//! The shapes that the image test `tests/scene.rs` draws (the `scene` example draws a glTF scene
//! instead): a unit cube and a smooth sphere, four materials, two of them with a checker texture,
//! four instances that overlap from most viewpoints, and in front of them six small spheres, a
//! row of dielectrics and a row of metals of increasing roughness (ADR-0035); a directional light,
//! and a camera that orbits them with time.

use std::f32::consts::PI;

use slime_color::LinearColor;
use slime_gfx::{Gpu, RenderTarget, SamplerDescriptor, TextureFormat};
use slime_render::glam::{DAffine3, DQuat, DVec3, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, Instance, Light, Material, MaterialData, Mesh, MeshData,
    RenderError, RenderStats, Renderer, SampledTexture, Scene, Sky,
};

/// A slightly warm light of π lux from above and from the side the camera starts on, under which a
/// white matte surface facing it reflects about 0.97 nits (ADR-0035), shown as they are at an
/// exposure of 1. Light in these units keeps the image of this test as it was before lights had
/// physical units: it checks the shapes and their materials, not the units. It casts shadows as far
/// as the shapes reach from the orbiting camera, about 7 m, so that the cascades cover them and
/// little else (ADR-0050).
const SUN: DirectionalLight = DirectionalLight {
    direction: Vec3::new(-0.6, -1.0, -0.5),
    color: LinearColor::rgb(1.0, 0.98, 0.9),
    illuminance: PI,
    casts_shadows: true,
    shadow_distance: 7.0,
};
/// The sample's clear sky (`sample_scene.rs`) at the scale of this light: its radiances times
/// π / 100 000, as it shows at the sample's exposure, around this light.
const SKY: Sky = Sky {
    zenith: LinearColor::rgb(0.0534, 0.1225, 0.3142),
    horizon: LinearColor::rgb(0.2388, 0.3016, 0.3833),
    ground: LinearColor::rgb(0.0974, 0.0927, 0.0848),
    sun: Some(SUN),
};

/// The strength of the bloom: the sample's (`sample_scene::BLOOM`), so that the image test sees
/// what the sample shows (ADR-0040).
const BLOOM: f32 = 0.04;

/// How far the camera orbits from the origin, and how high above it.
const ORBIT_RADIUS: f32 = 4.0;
const ORBIT_HEIGHT: f32 = 1.8;
/// How fast the camera orbits, in radians per second.
const ORBIT_SPEED: f32 = 0.5;

/// The color of the small spheres, and the roughness of each column of them.
const SPHERE_COLOR: LinearColor = LinearColor::rgb(0.9, 0.6, 0.3);
const SPHERE_ROUGHNESS: [f32; 3] = [0.3, 0.55, 0.8];

/// The side of the checker texture, in pixels, and of its squares.
const CHECKER_SIZE: u32 = 64;
const CHECKER_SQUARE: u32 = 8;

/// The renderer and the GPU objects of the shapes, for targets of one format.
pub struct Shapes {
    renderer: Renderer,
    cube: Mesh,
    sphere: Mesh,
    orange: Material,
    blue: Material,
    checkered: Material,
    checkered_green: Material,
    /// The small spheres' materials: a row of dielectrics, then a row of metals, each from the
    /// smoothest to the roughest.
    spheres: [Vec<Material>; 2],
    environment: Environment,
}

impl Shapes {
    /// Creates the renderer, the meshes, the checker texture and the materials, for targets of
    /// `format`.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let renderer = Renderer::new(gpu, format)?;
        let cube = renderer.create_mesh(gpu, "cube", &MeshData::cube())?;
        let sphere = renderer.create_mesh(gpu, "sphere", &MeshData::sphere(16, 24))?;
        let checker = gpu.create_texture(
            "checker",
            CHECKER_SIZE,
            CHECKER_SIZE,
            TextureFormat::Rgba8UnormSrgb,
            &checker(),
        )?;
        let sampler = gpu.create_sampler("checker", &SamplerDescriptor::default())?;
        let checkered_by = Some(SampledTexture {
            texture: &checker,
            sampler: &sampler,
        });
        let material = |label: &str, base_color: LinearColor, base_color_texture| {
            renderer.create_material(
                gpu,
                label,
                &MaterialData {
                    base_color,
                    base_color_texture,
                    ..MaterialData::default()
                },
            )
        };
        let orange = material("orange", LinearColor::rgb(0.9, 0.45, 0.1), None)?;
        let blue = material("blue", LinearColor::rgb(0.15, 0.4, 0.9), None)?;
        let checkered = material("checkered", LinearColor::WHITE, checkered_by)?;
        let checkered_green = material(
            "checkered green",
            LinearColor::rgb(0.45, 0.9, 0.55),
            checkered_by,
        )?;
        let spheres = [0.0, 1.0].map(|metallic| {
            SPHERE_ROUGHNESS
                .iter()
                .map(|&roughness| {
                    renderer.create_material(
                        gpu,
                        &format!("sphere, metallic {metallic}, roughness {roughness}"),
                        &MaterialData {
                            base_color: SPHERE_COLOR,
                            metallic,
                            roughness,
                            ..MaterialData::default()
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        });
        let [dielectrics, metals] = spheres;
        let spheres = [dielectrics?, metals?];
        let environment = Environment::new(gpu, &SKY)?;
        Ok(Self {
            environment,
            renderer,
            cube,
            sphere,
            orange,
            blue,
            checkered,
            checkered_green,
            spheres,
        })
    }

    /// Draws the shapes into `target` as the camera sees them `time` seconds after the start: it
    /// has orbited them by then.
    ///
    /// # Errors
    ///
    /// As [`Renderer::render`].
    pub fn render(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        time: f32,
    ) -> Result<RenderStats, RenderError> {
        let angle = time * ORBIT_SPEED;
        let camera = Camera {
            eye: Vec3::new(
                ORBIT_RADIUS * angle.sin(),
                ORBIT_HEIGHT,
                ORBIT_RADIUS * angle.cos(),
            )
            .as_dvec3(),
            target: DVec3::ZERO,
            ..Camera::default()
        };
        let mut instances = vec![
            Instance::new(&self.cube, &self.checkered, DAffine3::IDENTITY),
            Instance::new(
                &self.cube,
                &self.blue,
                DAffine3::from_rotation_translation(
                    DQuat::from_rotation_y(35f64.to_radians()),
                    DVec3::new(1.2, 0.3, -0.6),
                ),
            ),
            Instance::new(
                &self.cube,
                &self.orange,
                DAffine3::from_scale_rotation_translation(
                    DVec3::splat(0.6),
                    DQuat::from_rotation_x(20f64.to_radians()),
                    DVec3::new(-1.3, -0.2, 0.4),
                ),
            ),
            // The sphere sits on the first cube.
            Instance::new(
                &self.sphere,
                &self.checkered_green,
                DAffine3::from_translation(DVec3::new(0.0, 1.0, 0.0)),
            ),
        ];
        // In front of the cubes, two rows of three, the metals nearer.
        for (row, materials) in self.spheres.iter().enumerate() {
            for (column, material) in materials.iter().enumerate() {
                instances.push(Instance::new(
                    &self.sphere,
                    material,
                    DAffine3::from_scale_rotation_translation(
                        DVec3::splat(0.4),
                        DQuat::IDENTITY,
                        DVec3::new(0.6 * column as f64 - 0.6, -0.6, 1.2 + 0.6 * row as f64),
                    ),
                ));
            }
        }
        let lights = [Light::Directional(SUN)];
        let mut scene = Scene::new(camera, instances.clone());
        scene.lights = lights.to_vec();
        scene.environment = Some(self.environment.clone());
        scene.bloom = BLOOM;
        self.renderer.render(gpu, target, &scene)
    }
}

/// A checker of light and dark squares, as sRGB-encoded pixels, rows from top to bottom.
fn checker() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((CHECKER_SIZE * CHECKER_SIZE * 4) as usize);
    for y in 0..CHECKER_SIZE {
        for x in 0..CHECKER_SIZE {
            let light = (x / CHECKER_SQUARE + y / CHECKER_SQUARE).is_multiple_of(2);
            pixels.extend(if light {
                [235, 225, 205, 255]
            } else {
                [95, 75, 55, 255]
            });
        }
    }
    pixels
}
