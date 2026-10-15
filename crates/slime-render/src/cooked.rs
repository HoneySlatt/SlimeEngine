//! Cooked scenes on the GPU: a scene that the asset pipeline cooked, its meshes, materials and
//! textures uploaded, its nodes flattened into instances and its lights placed in world space.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use glam::{Mat4, Vec3};
use slime_assets::{
    AssetId, CookedAssets, CookedScene, LightKind, LoadError, TextureFilter, TextureSampler,
    TextureWrap,
};
use slime_gfx::{AddressMode, Filter, Gpu, LinearColor, Sampler, SamplerDescriptor, Texture};

use crate::error::RenderError;
use crate::light::{DirectionalLight, Light, PointLight, SpotLight};
use crate::material::{AlphaMode, Material, MaterialData, SampledTexture};
use crate::mesh::{Mesh, MeshData};
use crate::renderer::{Instance, Renderer};

/// Why a cooked scene cannot be loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SceneError {
    /// An asset of the scene cannot be read from the cooked data.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// The renderer cannot create one of its objects: the GPU's memory is full, or the device was
    /// lost.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// A node places a mesh or a light where nothing can be drawn: at a point that is not finite,
    /// or a light without a direction.
    #[error("scene `{scene}`, node {node}: {reason}")]
    Placement {
        /// The scene's name.
        scene: String,
        /// The node's index among the scene's nodes.
        node: usize,
        /// What is wrong.
        reason: String,
    },
}

/// A placed mesh: indices into the scene's meshes and materials, its transform, and the node that
/// places it.
struct Placement {
    mesh: usize,
    material: usize,
    transform: Mat4,
    node: usize,
}

/// A cooked scene on the GPU ([`Renderer::load_scene`]): its meshes and materials, which keep
/// their textures and samplers, where its nodes place them, and its lights in world space.
pub struct LoadedScene {
    meshes: Vec<Mesh>,
    materials: Vec<Material>,
    placements: Vec<Placement>,
    /// The scene as it was cooked: its nodes, their names, transforms and `extras`, which a game
    /// reads its own data from.
    pub scene: CookedScene,
    /// The lights its nodes place, in world space, in the order of the nodes.
    pub lights: Vec<Light>,
    /// The smallest box around every mesh placed, its lowest corner then its highest, in world
    /// space; `None` for a scene that places no mesh.
    pub bounds: Option<(Vec3, Vec3)>,
}

impl LoadedScene {
    /// The instances to draw: one per mesh that a node places, casting shadows.
    pub fn instances(&self) -> Vec<Instance<'_>> {
        self.placements
            .iter()
            .map(|placement| {
                Instance::new(
                    &self.meshes[placement.mesh],
                    &self.materials[placement.material],
                    placement.transform,
                )
            })
            .collect()
    }

    /// The node that places each of [`LoadedScene::instances`], in the same order: an index into
    /// `scene.nodes`, so that a game can leave out or move what some nodes place, such as a door
    /// that opens.
    pub fn instance_nodes(&self) -> Vec<usize> {
        self.placements
            .iter()
            .map(|placement| placement.node)
            .collect()
    }
}

impl Renderer {
    /// Loads the cooked scene `id` from `assets` onto the GPU: every mesh its nodes place, every
    /// material those meshes are drawn with, every texture those materials read, each once, on the
    /// calling thread; and its lights, placed by their nodes in world space, at the node's origin,
    /// along its −Z axis whatever its scale. A glTF file's default scene has the file's ID.
    ///
    /// # Errors
    ///
    /// [`SceneError::Load`] if an asset cannot be read; [`SceneError::Render`] if the GPU's memory
    /// cannot hold an object, the device is lost, or the GPU cannot sample the cooked textures
    /// (`GfxError::UnsupportedTexture`); [`SceneError::Placement`] if a node places a mesh at a
    /// point that is not finite, which a transform that overflows gives, or a light at such a point
    /// or with no direction, as a transform that flattens its −Z axis gives.
    pub fn load_scene(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        id: AssetId,
    ) -> Result<LoadedScene, SceneError> {
        let scene = assets.scene(id)?;
        let world = scene.world_transforms();
        let mut textures: HashMap<AssetId, Texture> = HashMap::new();
        let mut samplers: HashMap<TextureSampler, Sampler> = HashMap::new();
        let mut material_of: HashMap<AssetId, usize> = HashMap::new();
        let mut mesh_of: HashMap<AssetId, (usize, usize, [[f32; 3]; 2])> = HashMap::new();
        let mut loaded = LoadedScene {
            meshes: Vec::new(),
            materials: Vec::new(),
            placements: Vec::new(),
            scene: CookedScene::default(),
            lights: Vec::new(),
            bounds: None,
        };
        let placement = |node: usize, reason: String| SceneError::Placement {
            scene: scene.name.clone(),
            node,
            reason,
        };
        for (index, (node, transform)) in scene.nodes.iter().zip(&world).enumerate() {
            let transform = Mat4::from_cols_array(transform);
            for &mesh_id in &node.meshes {
                let (mesh, material, bounds) = match mesh_of.get(&mesh_id) {
                    Some(&found) => found,
                    None => {
                        let cooked = assets.mesh(mesh_id)?;
                        let material = match material_of.get(&cooked.material) {
                            Some(&found) => found,
                            None => {
                                let material = self.load_material(
                                    gpu,
                                    assets,
                                    cooked.material,
                                    &mut textures,
                                    &mut samplers,
                                )?;
                                loaded.materials.push(material);
                                material_of.insert(cooked.material, loaded.materials.len() - 1);
                                loaded.materials.len() - 1
                            }
                        };
                        let bounds = cooked.bounds;
                        let data = MeshData {
                            positions: cooked.positions,
                            normals: cooked.normals,
                            uvs: cooked.uvs,
                            tangents: cooked.tangents,
                            indices: cooked.indices,
                        };
                        loaded
                            .meshes
                            .push(self.create_mesh(gpu, &cooked.name, &data)?);
                        let found = (loaded.meshes.len() - 1, material, bounds);
                        mesh_of.insert(mesh_id, found);
                        found
                    }
                };
                loaded.bounds = grow(loaded.bounds, transform, bounds)
                    .map_err(|reason| placement(index, reason))?;
                loaded.placements.push(Placement {
                    mesh,
                    material,
                    transform,
                    node: index,
                });
            }
            if let Some(light) = node.light {
                let position = transform.transform_point3(Vec3::ZERO);
                if !position.is_finite() {
                    return Err(placement(
                        index,
                        format!("its light is placed at {position}, which is not finite"),
                    ));
                }
                // Scale changes the length of the axis, not the direction it gives.
                let direction = transform
                    .transform_vector3(Vec3::NEG_Z)
                    .try_normalize()
                    .ok_or_else(|| {
                        placement(
                            index,
                            "its light has no direction: the node's transform flattens its −Z \
                             axis or is not finite"
                                .to_owned(),
                        )
                    })?;
                let [r, g, b] = light.color;
                let color = LinearColor::rgb(r, g, b);
                loaded.lights.push(match light.kind {
                    LightKind::Directional => Light::Directional(DirectionalLight {
                        direction,
                        color,
                        illuminance: light.intensity,
                        ..DirectionalLight::default()
                    }),
                    LightKind::Point => Light::Point(PointLight {
                        position,
                        color,
                        intensity: light.intensity,
                        range: light.range,
                    }),
                    LightKind::Spot {
                        inner_cone_angle,
                        outer_cone_angle,
                    } => Light::Spot(SpotLight {
                        position,
                        direction,
                        color,
                        intensity: light.intensity,
                        range: light.range,
                        inner_cone_angle,
                        outer_cone_angle,
                        casts_shadows: true,
                    }),
                });
            }
        }
        tracing::info!(
            scene = %scene.name,
            meshes = loaded.meshes.len(),
            materials = loaded.materials.len(),
            textures = textures.len(),
            instances = loaded.placements.len(),
            lights = loaded.lights.len(),
            "cooked scene loaded"
        );
        loaded.scene = scene;
        Ok(loaded)
    }

    /// Creates the material `id`, uploading the textures it reads that are not in `textures` yet,
    /// and creating the samplers that are not in `samplers`.
    fn load_material(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        id: AssetId,
        textures: &mut HashMap<AssetId, Texture>,
        samplers: &mut HashMap<TextureSampler, Sampler>,
    ) -> Result<Material, SceneError> {
        let cooked = assets.material(id)?;
        for slot in cooked.textures().into_iter().flatten() {
            if let Entry::Vacant(entry) = textures.entry(slot.texture) {
                let data = assets.texture(slot.texture)?;
                let label = format!("{} texture {}", cooked.name, slot.texture);
                entry.insert(self.create_texture(gpu, &label, &data)?);
            }
            if let Entry::Vacant(entry) = samplers.entry(slot.sampler) {
                let descriptor = sampler_descriptor(slot.sampler);
                let sampler = gpu
                    .create_sampler(&cooked.name, &descriptor)
                    .map_err(RenderError::from)?;
                entry.insert(sampler);
            }
        }
        let sampled = |slot: Option<slime_assets::MaterialTexture>| {
            slot.map(|slot| SampledTexture {
                texture: &textures[&slot.texture],
                sampler: &samplers[&slot.sampler],
            })
        };
        let [r, g, b, a] = cooked.base_color;
        let [er, eg, eb] = cooked.emissive;
        let data = MaterialData {
            base_color: LinearColor::rgba(r, g, b, a),
            base_color_texture: sampled(cooked.base_color_texture),
            metallic: cooked.metallic,
            roughness: cooked.roughness,
            metallic_roughness_texture: sampled(cooked.metallic_roughness_texture),
            normal_texture: sampled(cooked.normal_texture),
            normal_scale: cooked.normal_scale,
            occlusion_texture: sampled(cooked.occlusion_texture),
            occlusion_strength: cooked.occlusion_strength,
            emissive: LinearColor::rgb(er, eg, eb),
            emissive_strength: cooked.emissive_strength,
            emissive_texture: sampled(cooked.emissive_texture),
            alpha_mode: match cooked.alpha_cutoff {
                Some(cutoff) => AlphaMode::Mask { cutoff },
                None => AlphaMode::Opaque,
            },
            double_sided: cooked.double_sided,
        };
        Ok(self.create_material(gpu, &cooked.name, &data)?)
    }
}

/// The GPU layer's sampler for a cooked texture slot's.
fn sampler_descriptor(sampler: TextureSampler) -> SamplerDescriptor {
    let filter = |filter| match filter {
        TextureFilter::Nearest => Filter::Nearest,
        TextureFilter::Linear => Filter::Linear,
    };
    let address = |wrap| match wrap {
        TextureWrap::Repeat => AddressMode::Repeat,
        TextureWrap::MirroredRepeat => AddressMode::MirrorRepeat,
        TextureWrap::ClampToEdge => AddressMode::ClampToEdge,
    };
    SamplerDescriptor {
        mag_filter: filter(sampler.mag_filter),
        min_filter: filter(sampler.min_filter),
        address_u: address(sampler.wrap_u),
        address_v: address(sampler.wrap_v),
    }
}

/// `bounds` grown by the box `mesh`, its lowest and highest corners, placed by `transform`; refused
/// if a corner lands at a point that is not finite, which would spoil the box.
fn grow(
    bounds: Option<(Vec3, Vec3)>,
    transform: Mat4,
    mesh: [[f32; 3]; 2],
) -> Result<Option<(Vec3, Vec3)>, String> {
    let [low, high] = mesh.map(Vec3::from);
    let mut bounds = bounds;
    for corner in 0..8 {
        let pick = |bit: usize, low: f32, high: f32| if corner & bit == 0 { low } else { high };
        let point = transform.transform_point3(Vec3::new(
            pick(1, low.x, high.x),
            pick(2, low.y, high.y),
            pick(4, low.z, high.z),
        ));
        if !point.is_finite() {
            return Err(format!(
                "it places a mesh at {point}: its transform overflows"
            ));
        }
        bounds = Some(match bounds {
            Some((min, max)) => (min.min(point), max.max(point)),
            None => (point, point),
        });
    }
    Ok(bounds)
}
