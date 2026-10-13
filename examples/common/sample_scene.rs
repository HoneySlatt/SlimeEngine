//! The glTF scene that the `scene` example and its image test draw: the asset of
//! `examples/assets/scene.glb` by default, or any GLB file the loader reads, lit by the lights the
//! file places, or by the sample's sun when it places none, and by its sky, in physical units
//! (ADR-0037, ADR-0039).

#[path = "gltf.rs"]
mod gltf;

use std::error::Error;
use std::f32::consts::PI;
use std::time::{Duration, Instant};

use slime_gfx::{Gpu, RenderTarget, TextureFormat};
use slime_render::glam::Vec3;
use slime_render::{
    Camera, DirectionalLight, Environment, Light, RenderStats, Renderer, Scene, Sky,
};

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

/// A clear day's sky around the sun, in nits: a blue zenith of luminance 3 900 and a whiter horizon
/// of 9 400, and dark soil below them, of albedo about 0.1, lit by the sun 52° high and by the sky:
/// 0.1 × (100 000 × sin 52° + 18 000) / π ≈ 3 100. Once exposed: (0.053, 0.123, 0.314),
/// (0.239, 0.302, 0.383) and (0.097, 0.093, 0.085). Without its sun, which [`SampleScene`] gives
/// it from the file's lights.
pub const SKY: Sky = Sky {
    zenith: Vec3::new(1700.0, 3900.0, 10_000.0),
    horizon: Vec3::new(7600.0, 9600.0, 12_200.0),
    ground: Vec3::new(3100.0, 2950.0, 2700.0),
    sun: None,
};

/// The sun that lights a file without lights: slightly warm, from above. The asset's sun is the
/// same light.
const SUN: DirectionalLight = DirectionalLight {
    direction: Vec3::new(-0.5, -1.0, -0.6),
    color: Vec3::new(1.0, 0.98, 0.9),
    illuminance: SUN_ILLUMINANCE,
    casts_shadows: true,
};

/// A loaded glTF scene, its sky and the renderer that draws them, for targets of one format.
pub struct SampleScene {
    renderer: Renderer,
    loaded: gltf::LoadedScene,
    environment: Environment,
    environment_time: Duration,
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
        // The sky's sun is the scene's: the file's first directional light, or the sample's sun
        // for a file without lights, so that its disc and glow lie where the light and its shadows
        // come from.
        let sun = loaded
            .lights
            .iter()
            .find_map(|light| match light {
                Light::Directional(light) => Some(*light),
                _ => None,
            })
            .or(loaded.lights.is_empty().then_some(SUN));
        let started = Instant::now();
        let environment = renderer.create_environment(gpu, &Sky { sun, ..SKY });
        let environment_time = started.elapsed();
        tracing::info!(?environment_time, "the sky's environment is computed");
        Ok(Self {
            renderer,
            loaded,
            environment,
            environment_time,
        })
    }

    /// How long the CPU took to compute the sky's environment, when the scene was loaded.
    pub fn environment_time(&self) -> Duration {
        self.environment_time
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
        let sun = [Light::Directional(SUN)];
        let lights = if self.loaded.lights.is_empty() {
            &sun[..]
        } else {
            &self.loaded.lights[..]
        };
        self.renderer.render(
            gpu,
            target,
            &Scene {
                camera,
                lights,
                environment: Some(&self.environment),
                exposure,
                instances: &instances,
            },
        )
    }
}
