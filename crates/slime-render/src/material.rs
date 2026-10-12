//! Materials: how a surface looks.

use slime_gfx::{BindGroup, Sampler, Texture};

/// A texture and the sampler that reads it.
#[derive(Clone, Copy, Debug)]
pub struct SampledTexture<'a> {
    /// The texture.
    pub texture: &'a Texture,
    /// How it is read: its filters and what lies beyond its edges.
    pub sampler: &'a Sampler,
}

/// The data of a material: a base color, and a texture that modulates it.
#[derive(Clone, Copy, Debug)]
pub struct MaterialData<'a> {
    /// The color of the surface: red, green, blue and alpha, linear (not sRGB-encoded), each
    /// from 0 to 1. Surfaces are drawn opaque: alpha is carried but not used yet.
    pub base_color: [f32; 4],
    /// A texture multiplied with the base color, read at each vertex's texture coordinates
    /// ([`MeshData::uvs`](crate::MeshData::uvs)): an sRGB format for an image of colors. `None`
    /// leaves the base color alone.
    pub base_color_texture: Option<SampledTexture<'a>>,
}

impl Default for MaterialData<'_> {
    /// Opaque white, without a texture.
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
        }
    }
}

/// A material on the GPU, created by [`Renderer::create_material`](crate::Renderer::create_material).
/// It keeps its texture and sampler alive.
///
/// Clones refer to the same material.
#[derive(Clone, Debug)]
pub struct Material {
    pub(crate) group: BindGroup,
}
