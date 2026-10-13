//! The glTF scene that the `scene` example and its image test draw: the asset of
//! `examples/assets/scene.glb` by default, or any GLB file the loader reads, lit by the lights the
//! file places, or by the sample's sun when it places none.

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

/// The color behind the scene.
pub const BACKGROUND: Color = Color {
    r: 0.05,
    g: 0.06,
    b: 0.08,
    a: 1.0,
};

/// The sun that lights a file without lights: slightly warm, from above, of π lux, under which a
/// white matte surface facing it reflects about 0.97 nits (ADR-0035).
const SUN: Light = Light::Directional(DirectionalLight {
    direction: Vec3::new(-0.5, -1.0, -0.6),
    color: Vec3::new(1.0, 0.98, 0.9),
    illuminance: PI,
});
/// A slightly cold ambient light, in nits.
const AMBIENT: Vec3 = Vec3::new(0.18, 0.19, 0.23);

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

    /// Draws the scene into `target` as `camera` sees it, its light multiplied by `exposure` (1
    /// leaves it as it is) before tone mapping.
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
