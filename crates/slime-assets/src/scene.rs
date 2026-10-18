//! Cooked scenes: the nodes of a glTF scene, the meshes and lights they place, and the custom
//! values they carry.

use crate::AssetId;
use crate::bytes::{Reader, Writer};
use crate::error::CookedError;

const MAGIC: &[u8; 4] = b"SLSC";
/// The version of the format of cooked scenes; a change of format changes it.
const VERSION: u32 = 2;

/// A custom value of a node: one of glTF's `extras`, as Blender writes a custom property.
#[derive(Clone, Debug, PartialEq, slime_reflect::Reflect)]
#[reflect(name = "Extra")]
pub enum Extra {
    /// A boolean.
    Bool(bool),
    /// A number.
    Number(f64),
    /// A text.
    Text(String),
}

/// What a light is, by `KHR_lights_punctual`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightKind {
    /// Light from a direction, its node's −Z axis: its intensity is an illuminance, in lux.
    Directional,
    /// Light from its node's origin in every direction: its intensity is in candelas.
    Point,
    /// Light from its node's origin within a cone along its −Z axis, in candelas, full within the
    /// inner angle and fading out to the outer one, both from the axis, in radians.
    Spot {
        /// Where the light starts to fade.
        inner_cone_angle: f32,
        /// Where it ends.
        outer_cone_angle: f32,
    },
}

/// A light a node places, by `KHR_lights_punctual`, in the node's own space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneLight {
    /// What the light is.
    pub kind: LightKind,
    /// Its color, linear red, green and blue.
    pub color: [f32; 3],
    /// Its intensity: lux for a directional light, candelas otherwise.
    pub intensity: f32,
    /// How far it reaches, in metres, for a point or spot light; `None` for no limit.
    pub range: Option<f32>,
}

/// A node of a scene.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneNode {
    /// Its name, as the file gives it; empty if it has none.
    pub name: String,
    /// The index of its parent among the scene's nodes, which comes before it; `None` for a node at
    /// the root.
    pub parent: Option<u32>,
    /// Its transform relative to its parent: a 4 × 4 matrix of 16 numbers, column after column.
    pub transform: [f32; 16],
    /// The cooked meshes it places, one per primitive of its glTF mesh.
    pub meshes: Vec<AssetId>,
    /// The light it places, if any.
    pub light: Option<SceneLight>,
    /// Its custom values, by name, in the order of their names, but its typed components: the engine
    /// passes them on without reading them.
    pub extras: Vec<(String, Extra)>,
    /// Its typed components, which glTF's `extras` give as `slime:<type name>` properties: the
    /// engine's binary encoding of a document whose keys are their types' names, as `slime-cook`
    /// encodes them, which a world reads through its registry when it spawns the scene
    /// (`docs/adr/0089-scenes-and-prefabs.md`); empty for none.
    pub components: Vec<u8>,
}

/// A cooked scene: its nodes, each after its parent.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CookedScene {
    /// Its name, for labels and messages.
    pub name: String,
    /// Its nodes, each after its parent: the hierarchy walked depth first, in the file's order.
    pub nodes: Vec<SceneNode>,
}

/// The product of two 4 × 4 matrices stored column after column.
fn multiply(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut product = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            product[column * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[column * 4 + k]).sum();
        }
    }
    product
}

impl CookedScene {
    /// Each node's transform in the scene's space: its parent's times its own.
    pub fn world_transforms(&self) -> Vec<[f32; 16]> {
        let mut world: Vec<[f32; 16]> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let transform = match node.parent {
                Some(parent) => multiply(&world[parent as usize], &node.transform),
                None => node.transform,
            };
            world.push(transform);
        }
        world
    }

    /// The cooked meshes its nodes place, each once, in the order they are first placed.
    pub fn needs(&self) -> Vec<AssetId> {
        let mut needs = Vec::new();
        for &mesh in self.nodes.iter().flat_map(|node| &node.meshes) {
            if !needs.contains(&mesh) {
                needs.push(mesh);
            }
        }
        needs
    }

    /// The scene's file, which [`CookedScene::from_bytes`] reads back: the format's four bytes,
    /// `SLSC`, and version, then its name and its nodes in order, each its name, its parent (4
    /// bytes, all set for none), its transform, its meshes, its light (a byte, 0 for none, then its
    /// kind, color, intensity, range and cone), its extras (each a name, a byte for its type, and its
    /// value), and its typed components (their length, then their bytes).
    ///
    /// # Panics
    ///
    /// If a node's parent does not come before it.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::new(MAGIC, VERSION);
        writer.text(&self.name);
        writer.count(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            writer.text(&node.name);
            match node.parent {
                Some(parent) => {
                    assert!(
                        (parent as usize) < index,
                        "scene `{}`: node {index}'s parent comes after it",
                        self.name
                    );
                    writer.u32(parent);
                }
                None => writer.u32(u32::MAX),
            }
            writer.f32s(&node.transform);
            writer.count(node.meshes.len());
            for &mesh in &node.meshes {
                writer.id(mesh);
            }
            match node.light {
                None => writer.u8(0),
                Some(light) => {
                    let (kind, cone) = match light.kind {
                        LightKind::Directional => (1, [0.0; 2]),
                        LightKind::Point => (2, [0.0; 2]),
                        LightKind::Spot {
                            inner_cone_angle,
                            outer_cone_angle,
                        } => (3, [inner_cone_angle, outer_cone_angle]),
                    };
                    writer.u8(kind);
                    writer.f32s(&light.color);
                    writer.f32(light.intensity);
                    // A range is positive: 0 stands for none.
                    writer.f32(light.range.unwrap_or(0.0));
                    writer.f32s(&cone);
                }
            }
            writer.count(node.extras.len());
            for (name, extra) in &node.extras {
                writer.text(name);
                match extra {
                    Extra::Bool(value) => {
                        writer.u8(0);
                        writer.u8(u8::from(*value));
                    }
                    Extra::Number(value) => {
                        writer.u8(1);
                        writer.f64(*value);
                    }
                    Extra::Text(value) => {
                        writer.u8(2);
                        writer.text(value);
                    }
                }
            }
            writer.bytes(&node.components);
        }
        writer.finish()
    }

    /// The scene that `bytes`, a file of [`CookedScene::to_bytes`], holds.
    ///
    /// # Errors
    ///
    /// [`CookedError::Version`] if another version of the format wrote it, and
    /// [`CookedError::Malformed`] if it is not a scene, is cut short, or has a node whose parent
    /// does not come before it.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CookedError> {
        let mut reader = Reader::open(bytes, MAGIC, VERSION, "scene")?;
        let name = reader.text()?;
        // A node takes at least 85 bytes.
        let count = reader.count(85)?;
        let mut nodes = Vec::with_capacity(count);
        for index in 0..count {
            let node_name = reader.text()?;
            let parent = match reader.u32()? {
                u32::MAX => None,
                parent if (parent as usize) < index => Some(parent),
                _ => return Err(reader.malformed("a node's parent does not come before it")),
            };
            let transform = reader.f32s()?;
            let meshes = (0..reader.count(16)?)
                .map(|_| reader.id())
                .collect::<Result<_, _>>()?;
            let light = match reader.u8()? {
                0 => None,
                kind @ 1..=3 => {
                    let color = reader.f32s()?;
                    let intensity = reader.f32()?;
                    let range = Some(reader.f32()?).filter(|&range| range > 0.0);
                    let [inner_cone_angle, outer_cone_angle] = reader.f32s()?;
                    Some(SceneLight {
                        kind: match kind {
                            1 => LightKind::Directional,
                            2 => LightKind::Point,
                            _ => LightKind::Spot {
                                inner_cone_angle,
                                outer_cone_angle,
                            },
                        },
                        color,
                        intensity,
                        range,
                    })
                }
                _ => return Err(reader.malformed("a light's kind is unknown")),
            };
            let mut extras = Vec::new();
            for _ in 0..reader.count(6)? {
                let extra_name = reader.text()?;
                let extra = match reader.u8()? {
                    0 => Extra::Bool(reader.u8()? != 0),
                    1 => Extra::Number(reader.f64()?),
                    2 => Extra::Text(reader.text()?),
                    _ => return Err(reader.malformed("an extra's type is unknown")),
                };
                extras.push((extra_name, extra));
            }
            let components = reader.bytes()?;
            nodes.push(SceneNode {
                name: node_name,
                parent,
                transform,
                meshes,
                light,
                extras,
                components,
            });
        }
        reader.finish()?;
        Ok(Self { name, nodes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A translation by (x, y, z), column after column.
    fn translation(x: f32, y: f32, z: f32) -> [f32; 16] {
        let mut matrix = [0.0; 16];
        for i in 0..4 {
            matrix[i * 5] = 1.0;
        }
        matrix[12..15].copy_from_slice(&[x, y, z]);
        matrix
    }

    fn scene() -> CookedScene {
        CookedScene {
            name: "zone".to_owned(),
            nodes: vec![
                SceneNode {
                    name: "ruin".to_owned(),
                    parent: None,
                    transform: translation(1.0, 0.0, 0.0),
                    meshes: vec![AssetId::from_u128(4), AssetId::from_u128(5)],
                    light: None,
                    extras: vec![
                        ("collider".to_owned(), Extra::Text("box".to_owned())),
                        ("height".to_owned(), Extra::Number(2.5)),
                    ],
                    components: vec![0x53, 0x4c, 0x44, 0x42],
                },
                SceneNode {
                    name: "lantern".to_owned(),
                    parent: Some(0),
                    transform: translation(0.0, 2.0, 0.0),
                    meshes: vec![AssetId::from_u128(5)],
                    light: Some(SceneLight {
                        kind: LightKind::Spot {
                            inner_cone_angle: 0.2,
                            outer_cone_angle: 0.5,
                        },
                        color: [1.0, 0.8, 0.6],
                        intensity: 40.0,
                        range: Some(8.0),
                    }),
                    extras: vec![("lit".to_owned(), Extra::Bool(true))],
                    components: Vec::new(),
                },
                SceneNode {
                    name: String::new(),
                    parent: None,
                    transform: translation(0.0, 0.0, 0.0),
                    meshes: Vec::new(),
                    light: Some(SceneLight {
                        kind: LightKind::Directional,
                        color: [1.0; 3],
                        intensity: 3.0,
                        range: None,
                    }),
                    extras: Vec::new(),
                    components: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn a_scene_is_read_back_from_its_file() {
        let scene = scene();
        assert_eq!(
            CookedScene::from_bytes(&scene.to_bytes()),
            Ok(scene.clone())
        );
        assert_eq!(scene.needs(), [4, 5].map(AssetId::from_u128));
    }

    #[test]
    fn a_child_is_placed_by_its_parent_s_transform_times_its_own() {
        let world = scene().world_transforms();
        assert_eq!(world[1][12..15], [1.0, 2.0, 0.0]);
        assert_eq!(world[0][12..15], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_damaged_file_is_refused() {
        let bytes = scene().to_bytes();
        for cut in [0, 8, 30, bytes.len() - 1] {
            assert!(
                CookedScene::from_bytes(&bytes[..cut]).is_err(),
                "cut at {cut}"
            );
        }
    }
}
