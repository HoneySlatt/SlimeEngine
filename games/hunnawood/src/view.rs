//! What the player sees: the zone on an overcast day, drawn by the engine's renderer from the
//! game's state. The GPU objects are made once, when the window opens, and kept for every frame and
//! every new game.
//!
//! Until the zone is cooked from its sources (M4a.6), its ground and colliders are drawn with shapes
//! made in code, and the hero with a stand-in: a hooded body and a face that shows which way it
//! looks.

use std::f32::consts::TAU;

use slime_gfx::{Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::glam::{Mat4, Quat, Vec2, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, Instance, Light, Material, MaterialData, Mesh, MeshData,
    RenderError, RenderStats, Renderer, Scene, Sky,
};

use crate::rules::{Game, HERO_RADIUS};
use crate::zone::Shape;

/// The sun behind the clouds: a light of 12 000 lux, a fifth of a clear day's direct sunlight,
/// from the south-west, 48° high, which casts soft-edged shadows up to 40 m from the camera.
const SUN: DirectionalLight = DirectionalLight {
    direction: Vec3::new(0.5, -0.8, -0.5),
    color: LinearColor::rgb(1.0, 0.97, 0.92),
    illuminance: 12_000.0,
    casts_shadows: true,
    shadow_distance: 40.0,
};
/// An overcast sky, in nits: grey, brighter overhead than at the horizon, which lights the ground
/// with about 5 000 lux; the sun shows no disc through the clouds. The ground below the horizon is
/// dark soil of albedo about 0.12, lit by the sun and the sky: 0.12 × (9 000 + 5 000) / π ≈ 530.
const OVERCAST: Sky = Sky {
    zenith: LinearColor::rgb(2000.0, 2100.0, 2300.0),
    horizon: LinearColor::rgb(1500.0, 1550.0, 1600.0),
    ground: LinearColor::rgb(540.0, 530.0, 500.0),
    sun: None,
};
/// What the scene's light is multiplied by: a white matte surface facing the sun, which receives
/// about 17 000 lux from it and the sky, shows at about 0.7, the light of a grey day.
const EXPOSURE: f32 = std::f32::consts::PI / 25_000.0;
/// The bloom's strength, as in the engine's samples.
const BLOOM: f32 = 0.04;

/// The hero's stand-in: a body as wide as the hero, its head, and its face.
const BODY_HEIGHT: f32 = 1.45;
const HEAD_HEIGHT: f32 = 1.62;
const HEAD_SIZE: f32 = 0.3;

/// The renderer and the GPU objects of the zone, for targets of one format.
pub struct View {
    renderer: Renderer,
    cube: Mesh,
    sphere: Mesh,
    cylinder: Mesh,
    materials: Materials,
    environment: Environment,
}

/// Every material of the game.
struct Materials {
    ground: Material,
    stone: Material,
    bark: Material,
    cloak: Material,
    face: Material,
}

/// A material of one `color`, without texture, of `roughness` from 0 to 1, not metallic.
fn plain(color: LinearColor, roughness: f32) -> MaterialData<'static> {
    MaterialData {
        base_color: color,
        roughness,
        metallic: 0.0,
        ..MaterialData::default()
    }
}

/// The transform of a shape one metre across, scaled to `size`, whose bottom stands on the ground
/// at `ground`, turned by `yaw` radians from +x towards +z.
fn standing(size: Vec3, ground: Vec2, yaw: f32) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        size,
        Quat::from_rotation_y(-yaw),
        Vec3::new(ground.x, size.y / 2.0, ground.y),
    )
}

/// A cylinder one metre across and one metre high, centred on the origin, its axis along y, of
/// `segments` sides, closed at both ends: a stand-in for trunks and pillars, which the engine's
/// shapes do not include. The texture wraps around the side once, u growing from +x towards +z, and
/// lies flat on each end. (`segments` + 1) × 2 + 2 × (`segments` + 1) vertices, 4 × `segments`
/// triangles.
fn cylinder(segments: u32) -> MeshData {
    let mut data = MeshData::default();
    let ring = |index: u32| {
        let angle = TAU * index as f32 / segments as f32;
        Vec2::from_angle(angle)
    };
    // The side: a pair of vertices, bottom and top, at each angle, the first repeated at the end
    // with u = 1.
    for index in 0..=segments {
        let around = ring(index);
        let normal = [around.x, 0.0, around.y];
        // Along increasing u; seen from outside the texture is mirrored, so the handedness is −1,
        // the bitangent, normal × tangent, pointing down.
        let tangent = [-around.y, 0.0, around.x, -1.0];
        let u = index as f32 / segments as f32;
        for (y, v) in [(-0.5, 1.0), (0.5, 0.0)] {
            data.positions.push([around.x * 0.5, y, around.y * 0.5]);
            data.normals.push(normal);
            data.uvs.push([u, v]);
            data.tangents.push(tangent);
        }
    }
    for index in 0..segments {
        let (bottom, top) = (2 * index, 2 * index + 1);
        let (next_bottom, next_top) = (bottom + 2, top + 2);
        // Counter-clockwise seen from outside.
        data.indices
            .extend([bottom, top, next_top, bottom, next_top, next_bottom]);
    }
    // The ends: a center and a ring each, the texture flat on them with its top towards −z on the
    // top end and +z on the bottom one, as on the engine's cube.
    for (y, normal, v_along_z) in [(0.5, 1.0, 1.0), (-0.5, -1.0, -1.0)] {
        let center = data.positions.len() as u32;
        data.positions.push([0.0, y, 0.0]);
        data.normals.push([0.0, normal, 0.0]);
        data.uvs.push([0.5, 0.5]);
        data.tangents.push([1.0, 0.0, 0.0, 1.0]);
        for index in 0..segments {
            let around = ring(index);
            data.positions.push([around.x * 0.5, y, around.y * 0.5]);
            data.normals.push([0.0, normal, 0.0]);
            data.uvs
                .push([0.5 + around.x * 0.5, 0.5 + v_along_z * around.y * 0.5]);
            data.tangents.push([1.0, 0.0, 0.0, 1.0]);
        }
        for index in 0..segments {
            let (this, next) = (center + 1 + index, center + 1 + (index + 1) % segments);
            // Counter-clockwise seen from outside: from above, the angle turns clockwise.
            if normal > 0.0 {
                data.indices.extend([center, next, this]);
            } else {
                data.indices.extend([center, this, next]);
            }
        }
    }
    data
}

impl View {
    /// Creates the renderer and the zone's meshes, materials and overcast sky, for targets of
    /// `format`.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let renderer = Renderer::new(gpu, format)?;
        let material =
            |label: &str, data: &MaterialData<'_>| renderer.create_material(gpu, label, data);
        let rgb = LinearColor::rgb;
        let materials = Materials {
            ground: material("ground", &plain(rgb(0.1, 0.12, 0.07), 0.95))?,
            stone: material("stone", &plain(rgb(0.3, 0.29, 0.27), 0.85))?,
            bark: material("bark", &plain(rgb(0.12, 0.09, 0.07), 0.9))?,
            cloak: material("cloak", &plain(rgb(0.08, 0.09, 0.08), 0.8))?,
            face: material("face", &plain(rgb(0.55, 0.42, 0.35), 0.6))?,
        };
        Ok(Self {
            cube: renderer.create_mesh(gpu, "cube", &MeshData::cube())?,
            sphere: renderer.create_mesh(gpu, "sphere", &MeshData::sphere(16, 24))?,
            cylinder: renderer.create_mesh(gpu, "cylinder", &cylinder(24))?,
            materials,
            environment: renderer.create_environment(gpu, &OVERCAST)?,
            renderer,
        })
    }

    /// Draws `game` into `target` as `camera` sees it, the hero `alpha` of the way between its last
    /// two fixed steps ([`Game::hero_between`]).
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        game: &Game,
        alpha: f32,
        camera: Camera,
    ) -> Result<RenderStats, RenderError> {
        let (cube, sphere, cylinder, materials) =
            (&self.cube, &self.sphere, &self.cylinder, &self.materials);
        let zone = game.zone();
        // The ground casts no shadow: it would only stretch the depths the cascades span
        // (ADR-0049).
        let ground_size = zone.half_size * 2.0;
        let mut ground = Instance::new(
            cube,
            &materials.ground,
            Mat4::from_scale_rotation_translation(
                Vec3::new(ground_size.x, 0.2, ground_size.y),
                Quat::IDENTITY,
                Vec3::new(0.0, -0.1, 0.0),
            ),
        );
        ground.casts_shadows = false;
        let mut instances = vec![ground];

        for collider in &zone.colliders {
            instances.push(match collider.shape {
                Shape::Box { half_size, yaw } => Instance::new(
                    cube,
                    &materials.stone,
                    standing(
                        Vec3::new(2.0 * half_size.x, collider.height, 2.0 * half_size.y),
                        collider.center,
                        yaw,
                    ),
                ),
                Shape::Cylinder { radius } => Instance::new(
                    cylinder,
                    // Pillars are stone, wider trunks bark.
                    if radius < 0.5 {
                        &materials.stone
                    } else {
                        &materials.bark
                    },
                    standing(
                        Vec3::new(2.0 * radius, collider.height, 2.0 * radius),
                        collider.center,
                        0.0,
                    ),
                ),
            });
        }

        // The hero: a body, a head under its hood, and a face on the side it looks to.
        let hero = game.hero_between(alpha);
        instances.push(Instance::new(
            cylinder,
            &materials.cloak,
            standing(
                Vec3::new(1.7 * HERO_RADIUS, BODY_HEIGHT, 1.7 * HERO_RADIUS),
                hero.position,
                hero.heading,
            ),
        ));
        let head = Vec3::new(hero.position.x, HEAD_HEIGHT, hero.position.y);
        instances.push(Instance::new(
            sphere,
            &materials.cloak,
            Mat4::from_scale_rotation_translation(Vec3::splat(HEAD_SIZE), Quat::IDENTITY, head),
        ));
        let facing = hero.facing();
        instances.push(Instance::new(
            cube,
            &materials.face,
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.05, 0.16, 0.14),
                Quat::from_rotation_y(-hero.heading),
                head + Vec3::new(facing.x, -0.02, facing.y) * (HEAD_SIZE / 2.0),
            ),
        ));

        let lights = [Light::Directional(SUN)];
        let mut scene = Scene::new(camera, &instances);
        scene.lights = &lights;
        scene.environment = Some(&self.environment);
        scene.exposure = EXPOSURE;
        scene.bloom = BLOOM;
        self.renderer.render(gpu, target, &scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cylinder_is_one_metre_across_and_high_with_outward_normals_and_faces() {
        let data = cylinder(12);
        assert_eq!(data.positions.len(), 13 * 2 + 2 * 13);
        assert_eq!(data.indices.len() / 3, 4 * 12);
        for (position, normal) in data.positions.iter().zip(&data.normals) {
            let position = Vec3::from_array(*position);
            assert!(Vec2::new(position.x, position.z).length() <= 0.5 + 1e-6);
            assert!(position.y.abs() <= 0.5 + 1e-6);
            let normal = Vec3::from_array(*normal);
            assert!((normal.length() - 1.0).abs() < 1e-5);
        }
        // Each triangle turns counter-clockwise seen from outside: its winding's normal points the
        // way its vertices' normals do, away from the center.
        for triangle in data.indices.chunks(3) {
            let [a, b, c] =
                [0, 1, 2].map(|i| Vec3::from_array(data.positions[triangle[i] as usize]));
            let winding = (b - a).cross(c - a);
            let normal = Vec3::from_array(data.normals[triangle[0] as usize]);
            assert!(winding.dot(normal) > 0.0, "{triangle:?}");
        }
        // Each tangent is of length one, perpendicular to its normal.
        for (tangent, normal) in data.tangents.iter().zip(&data.normals) {
            let tangent = Vec3::new(tangent[0], tangent[1], tangent[2]);
            assert!((tangent.length() - 1.0).abs() < 1e-5);
            assert!(tangent.dot(Vec3::from_array(*normal)).abs() < 1e-5);
        }
    }
}
