//! Errors of the renderer.

use slime_gfx::GfxError;

/// An error from the renderer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// The GPU layer failed: the graphics driver rejected the renderer's shader or pipeline, or
    /// the device was lost.
    #[error("the GPU layer failed")]
    Gfx(#[from] GfxError),
    /// Mesh data that cannot be drawn: mismatched vertex arrays, no triangle, or an index that
    /// names no vertex.
    #[error("mesh `{label}` is invalid: {reason}")]
    InvalidMesh {
        /// The label given to the mesh.
        label: String,
        /// What is wrong with the data.
        reason: String,
    },
}
