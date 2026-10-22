#![doc = include_str!("../README.md")]

mod bytes;
mod cache;
mod camera;
mod cooked;
mod drawings;
mod environment;
mod error;
mod graph;
mod kind;
mod light;
mod loads;
mod material;
mod mesh;
mod pick;
mod renderer;
mod shaders;
mod shadow;
mod shapes;

/// The math crate whose vectors, matrices and quaternions the API uses, re-exported so that
/// callers use the same version (ADR-0027).
pub use glam;

pub use cache::{
    AssetCache, AssetHandle, AssetRequests, AssetState, Budgets, CacheReport, DEFAULT_MESH_BUDGET,
    DEFAULT_TEXTURE_BUDGET, KindReport, Priority, Request,
};
pub use camera::{Camera, ImagePart, Projection};
pub use cooked::{LoadedScene, SceneError, SceneLoad, scene_lights};
pub use drawings::{DrawList, Drawings, Line};
pub use environment::{Environment, Sky};
pub use error::RenderError;
pub use light::{DirectionalLight, Light, MAX_LIGHTS, PointLight, SpotLight};
pub use material::{
    AlphaMode, Material, MaterialData, MaterialValues, SampledTexture, UnlitData, UnlitValues,
};
pub use mesh::{Mesh, MeshData};
pub use pick::{Pick, Picked};
pub use renderer::{
    AntiAliasing, DebugMode, Instance, RESIZE_DELAY, RenderStats, Renderer, Scene, View, ViewDraw,
    ViewSettings, Viewport,
};
pub use shaders::{ShaderSources, ShaderWatch};

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}
    fn send<T: Send>() {}
    /// Owns what it holds: it borrows nothing, so that it can outlive what it was made from.
    fn owned<T: 'static>() {}

    #[test]
    fn what_a_scene_is_made_of_can_be_shared_between_threads_and_a_renderer_moved() {
        send_and_sync::<Mesh>();
        send_and_sync::<MeshData>();
        send_and_sync::<Material>();
        send_and_sync::<LoadedScene>();
        send_and_sync::<AssetCache>();
        send_and_sync::<AssetHandle>();
        send_and_sync::<AssetRequests>();
        send_and_sync::<Request>();
        send_and_sync::<CacheReport>();
        send_and_sync::<SceneLoad>();
        send_and_sync::<Environment>();
        send_and_sync::<Camera>();
        send_and_sync::<Light>();
        send_and_sync::<Scene>();
        send_and_sync::<Instance>();
        send_and_sync::<Drawings>();
        send_and_sync::<Pick>();
        send_and_sync::<Picked>();
        send_and_sync::<RenderStats>();
        send_and_sync::<ViewSettings>();
        send_and_sync::<RenderError>();
        send_and_sync::<Renderer>();
        // A view's timers read their times back through a cell; `render` takes the views mutably:
        // one thread draws with them at a time.
        send::<View>();
    }

    #[test]
    fn a_scene_owns_what_it_draws() {
        owned::<Scene>();
        owned::<Instance>();
    }
}
