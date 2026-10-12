#![doc = include_str!("../README.md")]

mod bytes;
mod camera;
mod error;
mod light;
mod material;
mod mesh;
mod renderer;

/// The math crate whose vectors, matrices and quaternions the API uses, re-exported so that
/// callers use the same version (ADR-0027).
pub use glam;

pub use camera::Camera;
pub use error::RenderError;
pub use light::DirectionalLight;
pub use material::{Material, MaterialData, SampledTexture};
pub use mesh::{Mesh, MeshData};
pub use renderer::{Instance, RenderStats, Renderer, Scene};
