//! The glTF scene that the `scene` example and its image test draw: the asset of
//! `examples/assets/scene.glb` by default, or any GLB file the loader reads, under one light.

#[path = "gltf.rs"]
mod gltf;

use slime_gfx::{Color, Gpu, RenderTarget, TextureFormat};
use slime_render::glam::Vec3;
use slime_render::{Camera, DirectionalLight, RenderStats, Renderer, Scene};

pub use gltf::{Bounds, LoadError};

/// The sample asset, made in Blender by `examples/assets/make_scene.py`.
pub const ASSET: &[u8] = include_bytes!("../assets/scene.glb");

/// The color behind the scene.
pub const BACKGROUND: Color = Color {
    r: 0.05,
    g: 0.06,
    b: 0.08,
    a: 1.0,
};

/// A slightly warm light from above, with a slightly cold ambient light.
const LIGHT: DirectionalLight = DirectionalLight {
    direction: Vec3::new(-0.5, -1.0, -0.6),
    color: Vec3::new(1.0, 0.98, 0.9),
    ambient: Vec3::new(0.18, 0.19, 0.23),
};

/// A loaded glTF scene and the renderer that draws it, for targets of one format.
pub struct SampleScene {
    renderer: Renderer,
    loaded: gltf::LoadedScene,
}

impl SampleScene {
    /// Loads the GLB file `bytes` for targets of `format`.
    pub fn new(gpu: &Gpu, format: TextureFormat, bytes: &[u8]) -> Result<Self, LoadError> {
        let renderer =
            Renderer::new(gpu, format).map_err(|error| LoadError::new(error.to_string()))?;
        let loaded = gltf::load(gpu, &renderer, bytes)?;
        Ok(Self { renderer, loaded })
    }

    /// The smallest box around the scene, which the camera orbits.
    pub fn bounds(&self) -> &Bounds {
        &self.loaded.bounds
    }

    /// Draws the scene into `target` as `camera` sees it.
    pub fn render(&mut self, gpu: &Gpu, target: &RenderTarget, camera: Camera) -> RenderStats {
        let instances = self.loaded.instances();
        self.renderer.render(
            gpu,
            target,
            &Scene {
                camera,
                light: LIGHT,
                clear: BACKGROUND,
                instances: &instances,
            },
        )
    }
}
