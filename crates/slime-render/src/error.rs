//! Errors of the renderer.

use slime_gfx::GfxError;

/// An error from the renderer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// The GPU layer failed: the graphics driver rejected one of the renderer's three shaders or
    /// nine pipelines (see [`Renderer`](crate::Renderer)), the GPU's memory cannot hold an object
    /// being created (`GfxError::OutOfMemory`), or the device was lost.
    #[error("the GPU layer failed")]
    Gfx(#[from] GfxError),
    /// Mesh data that cannot be drawn, for one of the reasons that
    /// [`Renderer::create_mesh`](crate::Renderer::create_mesh) lists; `reason` says which.
    #[error("mesh `{label}` is invalid: {reason}")]
    InvalidMesh {
        /// The label given to the mesh.
        label: String,
        /// What is wrong with the data.
        reason: String,
    },
    /// Material values out of the ranges that [`MaterialValues`](crate::MaterialValues) gives,
    /// which [`Renderer::create_material`](crate::Renderer::create_material) and
    /// [`Material::set_values`](crate::Material::set_values) refuse, or a change of alpha mode,
    /// which `set_values` refuses; `reason` says which.
    #[error("material `{label}` is invalid: {reason}")]
    InvalidMaterial {
        /// The label the material was given.
        label: String,
        /// What is wrong with its values.
        reason: String,
    },
}
