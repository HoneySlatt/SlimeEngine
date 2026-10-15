//! Materials: how a surface looks, with glTF's metallic-roughness model (ADR-0035).

use std::sync::{Arc, Mutex, PoisonError};

use slime_gfx::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, Gpu, LinearColor, Sampler,
    Texture, UniformBuffer,
};

use crate::bytes::push_f32s;
use crate::error::RenderError;

/// The size of a material's values as `MaterialUniform` in `shader.wgsl` lays them out: the base
/// color (16 bytes), the emission (12) and five numbers (20).
pub(crate) const MATERIAL_VALUES_SIZE: usize = 48;

/// A texture and the sampler that reads it, made with
/// [`Gpu::create_texture`](slime_gfx::Gpu::create_texture) and
/// [`Gpu::create_sampler`](slime_gfx::Gpu::create_sampler).
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
        /// The smallest alpha that is drawn: 0 or more, and above 1, nothing is drawn.
        cutoff: f32,
    },
}

/// The data of a material: glTF's metallic-roughness model (ADR-0035).
///
/// Each texture is read at the mesh's texture coordinates
/// ([`MeshData::uvs`](crate::MeshData::uvs)) and multiplies the factor of its slot, the normal
/// texture excepted. Textures of colors (base color, emissive) must have an sRGB format, which the
/// GPU decodes to linear values; textures of data (metallic-roughness, normal, occlusion) must have
/// a format without sRGB encoding, such as `Rgba8Unorm`, read as stored
/// ([`Renderer::create_material`](crate::Renderer::create_material) panics otherwise). A slot left
/// empty reads 1 in every channel: its factor alone counts.
///
/// Its values must lie in the ranges that [`MaterialValues`] gives, or the material is refused.
#[derive(Clone, Copy, Debug)]
pub struct MaterialData<'a> {
    /// The color a dielectric diffuses and a metal reflects: red, green and blue each from 0 to 1.
    /// Alpha, from 0 to 1, is compared with the cutoff of [`AlphaMode::Mask`], and otherwise
    /// ignored.
    pub base_color: LinearColor,
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
    /// image; each from −1 to 1 stored as 0 to 1 (glTF). Blue is not read: z, along the mesh's
    /// normal, is rebuilt from x and y, the normal being of length one, so that a two-channel
    /// `Bc5RgUnorm` texture, as the cooker makes normal maps, serves as well as `Rgba8Unorm`
    /// (ADR-0071).
    pub normal_texture: Option<SampledTexture<'a>>,
    /// Multiplies the x and y of the normal texture's normals: 0 keeps the mesh's normal. Not used
    /// without a normal texture.
    pub normal_scale: f32,
    /// Its red channel is how much of the environment's light reaches the surface, from 0 (none)
    /// to 1 (all); the lights are not affected. `Rgba8Unorm`.
    pub occlusion_texture: Option<SampledTexture<'a>>,
    /// How much the occlusion texture counts, from 0 (not at all) to 1. Not used without one.
    pub occlusion_strength: f32,
    /// The light the surface emits, added to the light it reflects: red, green and blue each from
    /// 0 to 1 (glTF's emissive factor). Alpha is ignored.
    pub emissive: LinearColor,
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
            base_color: LinearColor::WHITE,
            base_color_texture: None,
            metallic: 0.0,
            roughness: 1.0,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            occlusion_texture: None,
            occlusion_strength: 1.0,
            emissive: LinearColor::BLACK,
            emissive_strength: 1.0,
            emissive_texture: None,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
        }
    }
}

impl MaterialData<'_> {
    /// Its values that can change once the material is made.
    pub(crate) fn values(&self) -> MaterialValues {
        MaterialValues {
            base_color: self.base_color,
            metallic: self.metallic,
            roughness: self.roughness,
            normal_scale: self.normal_scale,
            occlusion_strength: self.occlusion_strength,
            emissive: self.emissive,
            emissive_strength: self.emissive_strength,
            alpha_mode: self.alpha_mode,
        }
    }
}

/// The values of a material that can change once it is made, through [`Material::set_values`]:
/// all of [`MaterialData`] but its textures and its sidedness. Each means what the field of the same
/// name of `MaterialData` says.
///
/// Every value is finite; the base color's four channels, `metallic`, `roughness`,
/// `occlusion_strength` and the emission's red, green and blue lie from 0 to 1, as glTF keeps
/// them; `emissive_strength` and the cutoff of [`AlphaMode::Mask`] are not negative.
/// [`Renderer::create_material`](crate::Renderer::create_material) and [`Material::set_values`]
/// refuse values out of these ranges, which would draw nothing that a surface can look like: a
/// value that is not a number spreads through the bloom over the whole frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialValues {
    /// As [`MaterialData::base_color`].
    pub base_color: LinearColor,
    /// As [`MaterialData::metallic`].
    pub metallic: f32,
    /// As [`MaterialData::roughness`].
    pub roughness: f32,
    /// As [`MaterialData::normal_scale`].
    pub normal_scale: f32,
    /// As [`MaterialData::occlusion_strength`].
    pub occlusion_strength: f32,
    /// As [`MaterialData::emissive`].
    pub emissive: LinearColor,
    /// As [`MaterialData::emissive_strength`].
    pub emissive_strength: f32,
    /// As [`MaterialData::alpha_mode`]. A material keeps the mode it was made with: its cutoff can
    /// change, but not whether it has one.
    pub alpha_mode: AlphaMode,
}

impl MaterialValues {
    /// Which value is outside the range [`MaterialValues`] gives, if one is.
    pub(crate) fn invalid(&self) -> Option<String> {
        let LinearColor { r, g, b, a } = self.base_color;
        let emissive = self.emissive;
        let unit = [
            ("base color's red", r),
            ("base color's green", g),
            ("base color's blue", b),
            ("base color's alpha", a),
            ("metallic", self.metallic),
            ("roughness", self.roughness),
            ("occlusion strength", self.occlusion_strength),
            ("emission's red", emissive.r),
            ("emission's green", emissive.g),
            ("emission's blue", emissive.b),
        ];
        let not_negative = [
            Some(("emissive strength", self.emissive_strength)),
            match self.alpha_mode {
                AlphaMode::Opaque => None,
                AlphaMode::Mask { cutoff } => Some(("alpha cutoff", cutoff)),
            },
        ];
        // Comparisons with NaN are false: each test holds only for a number in its range.
        unit.into_iter()
            .find(|(_, value)| !(0.0..=1.0).contains(value))
            .map(|(name, value)| format!("its {name} {value} is not from 0 to 1"))
            .or_else(|| {
                not_negative
                    .into_iter()
                    .flatten()
                    .find(|(_, value)| !(value.is_finite() && *value >= 0.0))
                    .map(|(name, value)| format!("its {name} {value} is negative or not finite"))
            })
            .or_else(|| {
                (!self.normal_scale.is_finite())
                    .then(|| format!("its normal scale {} is not finite", self.normal_scale))
            })
    }

    /// The values the scene shader reads, packed as `MaterialUniform` declares them (ADR-0035),
    /// for a material that has a normal texture if `normal_texture`.
    pub(crate) fn bytes(&self, normal_texture: bool) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(MATERIAL_VALUES_SIZE);
        let LinearColor { r, g, b, a } = self.base_color;
        push_f32s(&mut bytes, &[r, g, b, a]);
        // The emission times its strength fills a vec3<f32>; metallic completes its 16 bytes.
        let (emissive, strength) = (self.emissive, self.emissive_strength);
        push_f32s(
            &mut bytes,
            &[emissive.r, emissive.g, emissive.b].map(|channel| channel * strength),
        );
        // A scale of 0 keeps the mesh's normals exactly; a cutoff of 0 keeps every fragment.
        let normal_scale = if normal_texture {
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
/// It keeps its textures and samplers alive. Any renderer of the same `Gpu` draws with it.
///
/// Clones refer to the same material: a change of values through one of them
/// ([`Material::set_values`]) changes how every instance drawn with any of them looks.
#[derive(Clone, Debug)]
pub struct Material {
    pub(crate) group: BindGroup,
    /// The buffer that the scene shader reads its values from.
    uniform: UniformBuffer,
    /// The values in `uniform`, which its clones share.
    values: Arc<Mutex<MaterialValues>>,
    /// The label it was created with: a cooked material's name.
    pub(crate) label: Arc<str>,
    /// Whether it has a normal texture, without which its normal scale is not used.
    normal_texture: bool,
    /// Whether its draws cull no face (ADR-0035).
    pub(crate) double_sided: bool,
    /// Whether its shadow is cut out where it discards fragments (ADR-0038).
    pub(crate) masked: bool,
}

impl Material {
    /// The values it is drawn with: those it was created with, or the last that
    /// [`Material::set_values`] gave it, or one of its clones.
    pub fn values(&self) -> MaterialValues {
        *self.values.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Changes the values it is drawn with to `values`; its textures and its sidedness stay as
    /// made. The work submitted to the GPU after this call draws with the new values, the work
    /// submitted before with the old ones: called between two frames ([`Renderer::render`](crate::Renderer::render)), as a
    /// game does, it changes the next frame whole. Once the device is lost, it changes the values
    /// that [`Material::values`] gives, and nothing that is drawn.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMaterial`] if a value is out of the range that [`MaterialValues`]
    /// gives, or if `values.alpha_mode` adds a cutoff to an opaque material or removes the cutoff
    /// of a masked one. The material is not changed then.
    ///
    /// # Panics
    ///
    /// If `gpu` is not the device the material was created with, and is not lost: wgpu panics.
    pub fn set_values(&self, gpu: &Gpu, values: MaterialValues) -> Result<(), RenderError> {
        let invalid = |reason| RenderError::InvalidMaterial {
            label: self.label.to_string(),
            reason,
        };
        if let Some(reason) = values.invalid() {
            return Err(invalid(reason));
        }
        if matches!(values.alpha_mode, AlphaMode::Mask { .. }) != self.masked {
            return Err(invalid(format!(
                "its alpha mode stays {}",
                if self.masked { "a mask" } else { "opaque" }
            )));
        }
        // A panic while the lock is held, when `gpu` is another device, leaves the values whole: the
        // old ones. Under the lock, so that the buffer holds the values that the clones read.
        let mut current = self.values.lock().unwrap_or_else(PoisonError::into_inner);
        gpu.write_uniform_buffer(&self.uniform, &values.bytes(self.normal_texture));
        *current = values;
        Ok(())
    }
}

/// What creating a material takes besides its data: the layout of its group, and the white pixel
/// and its sampler that fill the slots it leaves empty. The renderer keeps them, and gives a clone
/// to the threads that load scenes, so that they create materials too.
#[derive(Clone, Debug)]
pub(crate) struct MaterialParts {
    /// The layout of a material's group, which the scene pass's pipelines read it through.
    pub(crate) layout: BindGroupLayout,
    /// One white pixel, bound in every texture slot that a material leaves empty: every slot reads
    /// it as 1 in each channel, since sRGB's 255 decodes to exactly 1.
    pub(crate) white: Texture,
    /// The sampler of that pixel.
    pub(crate) sampler: Sampler,
}

impl MaterialParts {
    /// Creates a material, as [`Renderer::create_material`](crate::Renderer::create_material)
    /// says.
    pub(crate) fn create(
        &self,
        gpu: &Gpu,
        label: &str,
        data: &MaterialData<'_>,
    ) -> Result<Material, RenderError> {
        let values = data.values();
        if let Some(reason) = values.invalid() {
            return Err(RenderError::InvalidMaterial {
                label: label.to_owned(),
                reason,
            });
        }
        let normal_texture = data.normal_texture.is_some();
        let uniform = gpu.create_uniform_buffer(label, &values.bytes(normal_texture))?;
        // Each slot: its name, its texture, and whether it holds colors (sRGB) rather than data.
        let slots = [
            ("base color", data.base_color_texture, true),
            ("metallic-roughness", data.metallic_roughness_texture, false),
            ("normal", data.normal_texture, false),
            ("occlusion", data.occlusion_texture, false),
            ("emissive", data.emissive_texture, true),
        ];
        let mut entries = vec![BindGroupEntry {
            binding: 0,
            resource: BindingResource::UniformBuffer(&uniform),
        }];
        for (binding, (slot, sampled, colors)) in (1u32..).step_by(2).zip(slots) {
            let (texture, sampler) = match sampled {
                Some(sampled) => {
                    let format = sampled.texture.format();
                    assert!(
                        format.is_srgb() == colors,
                        "material `{label}`: its {slot} texture is {format:?}, but the {slot} \
                             slot reads {}",
                        if colors {
                            "colors, from an sRGB format"
                        } else {
                            "data, from Rgba8Unorm"
                        }
                    );
                    (sampled.texture, sampled.sampler)
                }
                None => (&self.white, &self.sampler),
            };
            entries.push(BindGroupEntry {
                binding,
                resource: BindingResource::Texture(texture),
            });
            entries.push(BindGroupEntry {
                binding: binding + 1,
                resource: BindingResource::Sampler(sampler),
            });
        }
        let group = gpu.create_bind_group(label, &self.layout, &entries)?;
        Ok(Material {
            group,
            uniform,
            values: Arc::new(Mutex::new(values)),
            label: label.into(),
            normal_texture,
            double_sided: data.double_sided,
            masked: matches!(data.alpha_mode, AlphaMode::Mask { .. }),
        })
    }
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
            base_color: LinearColor::rgba(0.1, 0.2, 0.3, 0.4),
            metallic: 0.5,
            roughness: 0.6,
            // Without a normal texture, the scale is packed as 0.
            normal_scale: 2.0,
            occlusion_strength: 0.7,
            // Its alpha is ignored.
            emissive: LinearColor::rgba(1.0, 0.5, 0.25, 0.0),
            emissive_strength: 4.0,
            alpha_mode: AlphaMode::Mask { cutoff: 0.3 },
            ..MaterialData::default()
        };
        let bytes = data.values().bytes(false);
        assert_eq!(bytes.len(), MATERIAL_VALUES_SIZE);
        assert_eq!(
            floats(&bytes),
            [0.1, 0.2, 0.3, 0.4, 4.0, 2.0, 1.0, 0.5, 0.6, 0.0, 0.7, 0.3]
        );
        assert_eq!(floats(&data.values().bytes(true))[9], 2.0);
        let opaque = MaterialData {
            alpha_mode: AlphaMode::Opaque,
            ..data
        };
        assert_eq!(floats(&opaque.values().bytes(false))[11], 0.0);
    }

    #[test]
    fn values_out_of_their_ranges_are_named() {
        let valid = MaterialData {
            emissive_strength: 1000.0,
            normal_scale: -3.0,
            alpha_mode: AlphaMode::Mask { cutoff: 2.0 },
            ..MaterialData::default()
        }
        .values();
        assert_eq!(valid.invalid(), None);
        for (values, reason) in [
            (
                MaterialValues {
                    base_color: LinearColor::rgb(1.5, 0.0, 0.0),
                    ..valid
                },
                "its base color's red 1.5 is not from 0 to 1",
            ),
            (
                MaterialValues {
                    roughness: f32::NAN,
                    ..valid
                },
                "its roughness NaN is not from 0 to 1",
            ),
            (
                MaterialValues {
                    emissive: LinearColor::rgb(0.0, 0.0, -0.1),
                    ..valid
                },
                "its emission's blue -0.1 is not from 0 to 1",
            ),
            (
                MaterialValues {
                    emissive_strength: f32::INFINITY,
                    ..valid
                },
                "its emissive strength inf is negative or not finite",
            ),
            (
                MaterialValues {
                    alpha_mode: AlphaMode::Mask { cutoff: -0.5 },
                    ..valid
                },
                "its alpha cutoff -0.5 is negative or not finite",
            ),
            (
                MaterialValues {
                    normal_scale: f32::NEG_INFINITY,
                    ..valid
                },
                "its normal scale -inf is not finite",
            ),
        ] {
            assert_eq!(values.invalid().as_deref(), Some(reason));
        }
    }
}
