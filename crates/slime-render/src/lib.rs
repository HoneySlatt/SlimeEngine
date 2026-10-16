#![doc = include_str!("../README.md")]

mod bytes;
mod cache;
mod camera;
mod cooked;
mod environment;
mod error;
mod graph;
mod light;
mod material;
mod mesh;
mod pool;
mod renderer;
mod shaders;
mod shadow;
mod shapes;

/// The math crate whose vectors, matrices and quaternions the API uses, re-exported so that
/// callers use the same version (ADR-0027).
pub use glam;

pub use cache::AssetCache;
pub use camera::Camera;
pub use cooked::{LoadedScene, SceneError, SceneLoad, scene_lights};
pub use environment::{Environment, Sky};
pub use error::RenderError;
pub use light::{DirectionalLight, Light, MAX_LIGHTS, PointLight, SpotLight};
pub use material::{AlphaMode, Material, MaterialData, MaterialValues, SampledTexture};
pub use mesh::{Mesh, MeshData};
pub use renderer::{Instance, RenderStats, Renderer, Scene};
pub use shaders::{ShaderSources, ShaderWatch};

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}
    fn send<T: Send>() {}

    #[test]
    fn what_a_scene_is_made_of_can_be_shared_between_threads_and_a_renderer_moved() {
        send_and_sync::<Mesh>();
        send_and_sync::<MeshData>();
        send_and_sync::<Material>();
        send_and_sync::<LoadedScene>();
        send_and_sync::<AssetCache>();
        send_and_sync::<SceneLoad>();
        send_and_sync::<Environment>();
        send_and_sync::<Camera>();
        send_and_sync::<Light>();
        send_and_sync::<Scene<'static>>();
        send_and_sync::<Instance<'static>>();
        send_and_sync::<RenderStats>();
        send_and_sync::<RenderError>();
        // `render` takes the renderer mutably: one thread draws with it at a time.
        send::<Renderer>();
    }
}
