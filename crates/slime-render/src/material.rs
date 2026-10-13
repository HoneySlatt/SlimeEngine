//! Materials: how a surface looks, with glTF's metallic-roughness model (ADR-0035).

use slime_gfx::{BindGroup, Sampler, Texture};

use crate::bytes::push_f32s;

/// The size of a material's values as `MaterialUniform` in `shader.wgsl` lays them out: the base
/// color (16 bytes), the emission (12) and five numbers (20).
pub(crate) const MATERIAL_VALUES_SIZE: usize = 48;

/// A texture and the sampler that reads it.
#[derive(Clone, Copy, Debug)]
pub struct SampledTexture<'a> {
    /// The texture.
    pub texture: &'a Texture,
    /// How it is read: its filters and what lies beyond its edges.
    pub sampler: &'a Sampler,
}

/// How a material uses the alpha of its base color (the factor's times the texture's): glTF's
/// alpha modes, but for blending (`BLEND`), which the renderer does not do.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AlphaMode {
    /// The alpha is ignored: the surface is drawn wherever it is (glTF's `OPAQUE`).
    #[default]
    Opaque,
    /// glTF's `MASK`: where the alpha is below `cutoff`, nothing is drawn, not even depth;
    /// elsewhere the surface is drawn opaque. glTF's default cutoff is 0.5.
    Mask {
        /// The smallest alpha that is drawn, from 0 to 1.
        cutoff: f32,
    },
}

/// The data of a material: glTF's metallic-roughness model (ADR-0035).
///
/// Each texture is read at the mesh's texture coordinates
/// ([`MeshData::uvs`](crate::MeshData::uvs)) and multiplies the factor of its slot, the normal
/// texture excepted. Textures of colors (base color, emissive) must have an sRGB format, which the
/// GPU decodes to linear values; textures of data (metallic-roughness, normal, occlusion) must have
/// a format without sRGB encoding, `Rgba8Unorm`, read as stored
/// ([`Renderer::create_material`](crate::Renderer::create_material) panics otherwise). A slot left
/// empty reads 1 in every channel: its factor alone counts.
#[derive(Clone, Copy, Debug)]
pub struct MaterialData<'a> {
    /// Red, green, blue and alpha, linear, each from 0 to 1: the color a dielectric diffuses and a
    /// metal reflects. Alpha is compared with the cutoff of [`AlphaMode::Mask`], and otherwise
    /// ignored.
    pub base_color: [f32; 4],
    /// Multiplies `base_color`, alpha included. An sRGB format.
    pub base_color_texture: Option<SampledTexture<'a>>,
    /// 0 for a dielectric (plastic, wood, stone), 1 for a metal; clamped to 0 to 1 once multiplied
    /// by the texture.
    pub metallic: f32,
    /// From 0, a mirror, to 1: the rougher, the wider and dimmer the highlights. Shaded no smoother
    /// than 0.045, and clamped to at most 1 once multiplied by the texture.
    pub roughness: f32,
    /// Its green channel multiplies `roughness`, its blue channel `metallic`; red and alpha are not
    /// read (glTF's layout, which lets one image hold the occlusion in red too). `Rgba8Unorm`.
    pub metallic_roughness_texture: Option<SampledTexture<'a>>,
    /// Normals in tangent space ([`MeshData::tangents`](crate::MeshData::tangents)), which replace
    /// the mesh's: red for x, along the tangent; green for y, along the bitangent, up the texture's
    /// image; blue for z, along the mesh's normal; each from −1 to 1 stored as 0 to 1 (glTF).
    /// `Rgba8Unorm`.
    pub normal_texture: Option<SampledTexture<'a>>,
    /// Multiplies the x and y of the normal texture's normals: 0 keeps the mesh's normal. Not used
    /// without a normal texture.
    pub normal_scale: f32,
    /// Its red channel is how much of the ambient light reaches the surface, from 0 (none) to 1
    /// (all); the directional light is not affected. `Rgba8Unorm`.
    pub occlusion_texture: Option<SampledTexture<'a>>,
    /// How much the occlusion texture counts, from 0 (not at all) to 1. Not used without one.
    pub occlusion_strength: f32,
    /// The light the surface emits, linear, each channel from 0 to 1 (glTF's emissive factor),
    /// added to the light it reflects.
    pub emissive: [f32; 3],
    /// Multiplies `emissive`; not negative, and above 1 for a surface brighter than white
    /// (glTF's `KHR_materials_emissive_strength`).
    pub emissive_strength: f32,
    /// Multiplies `emissive`. An sRGB format.
    pub emissive_texture: Option<SampledTexture<'a>>,
    /// How the alpha of the base color is used.
    pub alpha_mode: AlphaMode,
    /// Whether the back faces are drawn too, shaded with their normals turned around (glTF);
    /// when false they are culled.
    pub double_sided: bool,
}

impl Default for MaterialData<'_> {
    /// An opaque, single-sided, white matte dielectric without textures: glTF's default material
    /// but for `metallic`, which glTF sets to 1 (a rough white metal). Scale and strengths 1, no
    /// emission.
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            base_color_texture: None,
            metallic: 0.0,
            roughness: 1.0,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            occlusion_texture: None,
            occlusion_strength: 1.0,
            emissive: [0.0; 3],
            emissive_strength: 1.0,
            emissive_texture: None,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
        }
    }
}

impl MaterialData<'_> {
    /// The values the scene shader reads for this material, packed as `MaterialUniform` declares
    /// them (ADR-0035).
    pub(crate) fn values(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(MATERIAL_VALUES_SIZE);
        push_f32s(&mut bytes, &self.base_color);
        // The emission times its strength fills a vec3<f32>; metallic completes its 16 bytes.
        push_f32s(
            &mut bytes,
            &self
                .emissive
                .map(|channel| channel * self.emissive_strength),
        );
        // A scale of 0 keeps the mesh's normals exactly; a cutoff of 0 keeps every fragment.
        let normal_scale = if self.normal_texture.is_some() {
            self.normal_scale
        } else {
            0.0
        };
        let alpha_cutoff = match self.alpha_mode {
            AlphaMode::Opaque => 0.0,
            AlphaMode::Mask { cutoff } => cutoff,
        };
        push_f32s(
            &mut bytes,
            &[
                self.metallic,
                self.roughness,
                normal_scale,
                self.occlusion_strength,
                alpha_cutoff,
            ],
        );
        bytes
    }
}

/// A material on the GPU, created by [`Renderer::create_material`](crate::Renderer::create_material).
/// It keeps its textures and samplers alive.
///
/// Clones refer to the same material.
#[derive(Clone, Debug)]
pub struct Material {
    pub(crate) group: BindGroup,
    /// Whether its draws cull no face (ADR-0035).
    pub(crate) double_sided: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floats(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect()
    }

    #[test]
    fn the_values_are_packed_as_the_shader_reads_them() {
        let data = MaterialData {
            base_color: [0.1, 0.2, 0.3, 0.4],
            metallic: 0.5,
            roughness: 0.6,
            // Without a normal texture, the scale is packed as 0.
            normal_scale: 2.0,
            occlusion_strength: 0.7,
            emissive: [1.0, 0.5, 0.25],
            emissive_strength: 4.0,
            alpha_mode: AlphaMode::Mask { cutoff: 0.3 },
            ..MaterialData::default()
        };
        let bytes = data.values();
        assert_eq!(bytes.len(), MATERIAL_VALUES_SIZE);
        assert_eq!(
            floats(&bytes),
            [0.1, 0.2, 0.3, 0.4, 4.0, 2.0, 1.0, 0.5, 0.6, 0.0, 0.7, 0.3]
        );
        let opaque = MaterialData {
            alpha_mode: AlphaMode::Opaque,
            ..data
        };
        assert_eq!(floats(&opaque.values())[11], 0.0);
    }
}
