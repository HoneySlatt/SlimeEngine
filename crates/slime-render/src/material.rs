//! Materials: how a surface looks.

use slime_gfx::BindGroup;

/// The data of a material: a base color for now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialData {
    /// The color of the surface: red, green, blue and alpha, linear (not sRGB-encoded), each
    /// from 0 to 1. Surfaces are drawn opaque: alpha is carried but not used yet.
    pub base_color: [f32; 4],
}

impl Default for MaterialData {
    /// Opaque white.
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// A material on the GPU, created by [`Renderer::create_material`](crate::Renderer::create_material).
///
/// Clones refer to the same material.
#[derive(Clone, Debug)]
pub struct Material {
    pub(crate) group: BindGroup,
}
