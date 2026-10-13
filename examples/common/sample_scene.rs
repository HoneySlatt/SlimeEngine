//! The glTF scene that the `scene` example and its image test draw: the asset of
//! `examples/assets/scene.glb` by default, or any GLB file the loader reads, lit by the lights the
//! file places, or by the sample's sun when it places none, in physical units (ADR-0037).

#[path = "gltf.rs"]
mod gltf;

use std::error::Error;
use std::f32::consts::PI;

use slime_gfx::{Color, Gpu, RenderTarget, TextureFormat};
use slime_render::glam::Vec3;
use slime_render::{Camera, DirectionalLight, Light, RenderStats, Renderer, Scene};

pub use gltf::Bounds;

/// The sample asset, made in Blender by `examples/assets/make_scene.py`.
pub const ASSET: &[u8] = include_bytes!("../assets/scene.glb");

/// The illuminance of the sample's sun, in lux: direct sunlight. The asset's sun has it too
/// (`make_scene.py`).
const SUN_ILLUMINANCE: f32 = 100_000.0;

/// The exposure the sample is drawn at: under it, a white matte surface facing the sun reflects
/// 0.97 (ADR-0035), shown at sRGB 239. A photographer's exposure value of 14.7 at ISO 100, near the
/// 15 of the "sunny 16" rule for direct sunlight.
pub const EXPOSURE: f32 = PI / SUN_ILLUMINANCE;

/// The color behind the scene, in nits: a dark blue-grey, (0.05, 0.06, 0.08) once exposed. Not a
/// sky: image-based lighting brings one.
pub const BACKGROUND: Color = Color {
    r: 1592.0,
    g: 1910.0,
    b: 2546.0,
    a: 1.0,
};

/// A slightly cold ambient light, in nits: about the luminance of a clear sky, whose light from
/// every direction would give a surface π × 6 000 ≈ 19 000 lux; (0.18, 0.19, 0.23) once exposed.
const AMBIENT: Vec3 = Vec3::new(5730.0, 6048.0, 7321.0);

/// The sun that lights a file without lights: slightly warm, from above. The asset's sun is the
/// same light.
const SUN: Light = Light::Directional(DirectionalLight {
    direction: Vec3::new(-0.5, -1.0, -0.6),
    color: Vec3::new(1.0, 0.98, 0.9),
    illuminance: SUN_ILLUMINANCE,
});

/// A loaded glTF scene and the renderer that draws it, for targets of one format.
pub struct SampleScene {
    renderer: Renderer,
    loaded: gltf::LoadedScene,
}

impl SampleScene {
    /// Loads the GLB file `bytes` for targets of `format`.
    ///
    /// # Errors
    ///
    /// The renderer's error when it cannot be created, with its cause (the driver's message, or
    /// why the device was lost); the loader's when the file cannot be drawn.
    pub fn new(
        gpu: &Gpu,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let renderer = Renderer::new(gpu, format)?;
        let loaded = gltf::load(gpu, &renderer, bytes)?;
        Ok(Self { renderer, loaded })
    }

    /// The smallest box around the scene, which the camera orbits.
    pub fn bounds(&self) -> &Bounds {
        &self.loaded.bounds
    }

    /// Draws the scene into `target` as `camera` sees it, its light multiplied by `exposure`
    /// ([`EXPOSURE`] for the sample) before tone mapping.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        camera: Camera,
        exposure: f32,
    ) -> RenderStats {
        let instances = self.loaded.instances();
        let lights = if self.loaded.lights.is_empty() {
            &[SUN][..]
        } else {
            &self.loaded.lights[..]
        };
        self.renderer.render(
            gpu,
            target,
            &Scene {
                camera,
                lights,
                ambient: AMBIENT,
                clear: BACKGROUND,
                exposure,
                instances: &instances,
            },
        )
    }
}
