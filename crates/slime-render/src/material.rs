//! Materials: how a surface looks, with glTF's metallic-roughness model (ADR-0035), or unlit, its
//! color shown as it is (ADR-0129).

use std::sync::{Arc, Mutex, PoisonError};

use slime_color::LinearColor;
use slime_gfx::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, Gpu, Sampler, Texture,
    UniformBuffer,
};
use slime_reflect::{Reflect, Value};

use crate::bytes::push_f32s;
use crate::error::RenderError;
use crate::kind::{MaterialKind, PhysicallyBased, Unlit};

/// The size of a material's values as `MaterialUniform` in `shader.wgsl` lays them out: the base
/// color (16 bytes), the emission (12) and five numbers (20).
pub(crate) const MATERIAL_VALUES_SIZE: usize = 48;

/// The texture slots of a material, each a texture and its sampler: base color,
/// metallic-roughness, normal, occlusion, emissive (ADR-0035).
pub(crate) const MATERIAL_TEXTURE_SLOTS: u32 = 5;

/// The index of the physically based kind among a renderer's kinds ([`MaterialKind`]): the kind of
/// the materials that [`MaterialParts::create`] makes.
///
/// [`MaterialKind`]: crate::kind::MaterialKind
pub(crate) const PHYSICALLY_BASED: usize = 0;

/// The index of the unlit kind among a renderer's kinds: the kind of the materials that
/// [`MaterialParts::create_unlit`] makes.
pub(crate) const UNLIT: usize = 1;

/// The size of an unlit material's values as `MaterialUniform` in `unlit.wgsl` lays them out: the
/// color (16 bytes), the alpha cutoff and whether it blends (8), and padding to a multiple of 16.
pub(crate) const UNLIT_VALUES_SIZE: usize = 32;

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

/// How a material uses the alpha of its color (the factor's times the texture's): glTF's alpha
/// modes. The physically based kind refuses blending (`BLEND`) until Stage 9a sorts transparent
/// surfaces by their depth; the unlit kind draws it (ADR-0129).
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(name = "AlphaMode")]
pub enum AlphaMode {
    /// The alpha is ignored: the surface is drawn wherever it is (glTF's `OPAQUE`).
    #[default]
    Opaque,
    /// glTF's `MASK`: where the alpha is below `cutoff`, nothing is drawn, not even depth;
    /// elsewhere the surface is drawn opaque. glTF's default cutoff is 0.5.
    Mask {
        /// The smallest alpha that is drawn: 0 or more, and above 1, nothing is drawn.
        #[reflect(range(min = 0.0))]
        cutoff: f32,
    },
    /// glTF's `BLEND`: the surface is laid over what is behind it by its alpha, and does not write
    /// its depth, so that what is drawn after it behind it shows too. Blended surfaces are drawn
    /// after the others, in no particular order among themselves: where two overlap, either may lie
    /// over the other.
    Blend,
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
    /// Each texture slot, in the order of the material's group: its name, its texture, and whether
    /// it holds colors, in an sRGB format, rather than data.
    fn slots(&self) -> [(&'static str, Option<SampledTexture<'_>>, bool); 5] {
        [
            ("base color", self.base_color_texture, true),
            ("metallic-roughness", self.metallic_roughness_texture, false),
            ("normal", self.normal_texture, false),
            ("occlusion", self.occlusion_texture, false),
            ("emissive", self.emissive_texture, true),
        ]
    }

    /// Why a texture's format does not suit its slot, if one does not: read in the other kind of
    /// format, its values would be wrong.
    pub(crate) fn misplaced_texture(&self) -> Option<String> {
        self.slots()
            .into_iter()
            .find_map(|(slot, sampled, colors)| {
                let format = sampled?.texture.format();
                (format.is_srgb() != colors).then(|| {
                    format!(
                        "its {slot} texture is {format:?}, but the {slot} slot reads {}",
                        if colors {
                            "colors, from an sRGB format"
                        } else {
                            "data, from Rgba8Unorm"
                        }
                    )
                })
            })
    }

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
///
/// They are the parameters of the physically based kind of material (ADR-0111), described by
/// reflection, so that an editor shows and edits them by their names and ranges
/// ([`Material::parameters`]).
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "MaterialValues", check = MaterialValues::check)]
pub struct MaterialValues {
    /// The color a dielectric diffuses and a metal reflects, times its texture, from 0 to 1.
    pub base_color: LinearColor,
    /// 0 for a dielectric, 1 for a metal, times its texture.
    #[reflect(range(min = 0.0, max = 1.0))]
    pub metallic: f32,
    /// From 0, a mirror, to 1: the rougher, the wider and dimmer the highlights.
    #[reflect(range(min = 0.0, max = 1.0))]
    pub roughness: f32,
    /// Multiplies the x and y of the normal texture's normals: 0 keeps the mesh's normal.
    pub normal_scale: f32,
    /// How much the occlusion texture counts, from 0, not at all, to 1.
    #[reflect(range(min = 0.0, max = 1.0))]
    pub occlusion_strength: f32,
    /// The light the surface emits, from 0 to 1 in each channel; its alpha is ignored.
    pub emissive: LinearColor,
    /// Multiplies the emission: above 1 for a surface brighter than white.
    #[reflect(range(min = 0.0))]
    pub emissive_strength: f32,
    /// How the alpha of the base color is used. A material keeps the mode it was made with: its
    /// cutoff can change, but not whether it has one.
    pub alpha_mode: AlphaMode,
}

impl MaterialValues {
    /// Accepts the values if they lie in their ranges, the colors' included, which ranges on fields
    /// cannot say: what reflection checks when it reads values (`#[reflect(check)]`).
    fn check(&self) -> Result<(), String> {
        self.invalid().map_or(Ok(()), Err)
    }

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
        if self.alpha_mode == AlphaMode::Blend {
            return Some(
                "its alpha mode is Blend, which only unlit materials draw until Stage 9a sorts \
                 transparent surfaces"
                    .to_owned(),
            );
        }
        let not_negative = [
            Some(("emissive strength", self.emissive_strength)),
            cutoff_of(self.alpha_mode).map(|cutoff| ("alpha cutoff", cutoff)),
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
        push_f32s(
            &mut bytes,
            &[
                self.metallic,
                self.roughness,
                normal_scale,
                self.occlusion_strength,
                cutoff_of(self.alpha_mode).unwrap_or(0.0),
            ],
        );
        bytes
    }
}

/// The cutoff of a masked alpha mode, below which nothing is drawn; `None` for the others, whose
/// shaders compare the alpha with 0 and keep every fragment.
fn cutoff_of(alpha_mode: AlphaMode) -> Option<f32> {
    match alpha_mode {
        AlphaMode::Mask { cutoff } => Some(cutoff),
        AlphaMode::Opaque | AlphaMode::Blend => None,
    }
}

/// The data of an unlit material (ADR-0129): a color shown as it is, whatever lights the scene and
/// whatever its exposure, as an editor's icons and handles and a stylized game's surfaces are.
///
/// Its color is written into the scene unexposed, then mapped to the display's range by the tone
/// curve as the scene's light is ([`Renderer::render`](crate::Renderer::render)): Khronos PBR
/// Neutral keeps a color whose channels lie below 0.76, but lowers it by up to 0.04. Its surfaces
/// cast no shadows. Its values must lie in the ranges that [`UnlitValues`] gives, or the material
/// is refused.
#[derive(Clone, Copy, Debug)]
pub struct UnlitData<'a> {
    /// The color shown, in linear values: red, green, blue and alpha each from 0 to 1. Alpha is
    /// used as `alpha_mode` says.
    pub color: LinearColor,
    /// Multiplies `color`, alpha included. An sRGB format.
    pub texture: Option<SampledTexture<'a>>,
    /// How the alpha of the color is used.
    pub alpha_mode: AlphaMode,
    /// Whether the back faces are drawn too; when false they are culled.
    pub double_sided: bool,
}

impl Default for UnlitData<'_> {
    /// Opaque white, single-sided, without a texture.
    fn default() -> Self {
        Self {
            color: LinearColor::WHITE,
            texture: None,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
        }
    }
}

/// The values of an unlit material that can change once it is made: all of [`UnlitData`] but its
/// texture and its sidedness, each meaning what the field of the same name says there. The color's
/// four channels lie from 0 to 1, and a cutoff of [`AlphaMode::Mask`] is not negative.
///
/// They are the parameters of the unlit kind of material, described by reflection
/// ([`Material::parameters`]).
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "UnlitValues", check = UnlitValues::check)]
pub struct UnlitValues {
    /// The color shown, times its texture, from 0 to 1.
    pub color: LinearColor,
    /// How the alpha of the color is used. A material keeps the mode it was made with: the cutoff
    /// of a mask can change, but not the mode.
    pub alpha_mode: AlphaMode,
}

impl UnlitValues {
    /// Accepts the values if they lie in their ranges (`#[reflect(check)]`).
    fn check(&self) -> Result<(), String> {
        self.invalid().map_or(Ok(()), Err)
    }

    /// Which value is outside its range, if one is.
    pub(crate) fn invalid(&self) -> Option<String> {
        let LinearColor { r, g, b, a } = self.color;
        [("red", r), ("green", g), ("blue", b), ("alpha", a)]
            .into_iter()
            .find(|(_, value)| !(0.0..=1.0).contains(value))
            .map(|(name, value)| format!("its color's {name} {value} is not from 0 to 1"))
            .or_else(|| {
                cutoff_of(self.alpha_mode)
                    .filter(|cutoff| !(cutoff.is_finite() && *cutoff >= 0.0))
                    .map(|cutoff| format!("its alpha cutoff {cutoff} is negative or not finite"))
            })
    }

    /// The values `unlit.wgsl` reads, packed as its `MaterialUniform` declares them.
    pub(crate) fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(UNLIT_VALUES_SIZE);
        let LinearColor { r, g, b, a } = self.color;
        push_f32s(&mut bytes, &[r, g, b, a]);
        let blended = if self.alpha_mode == AlphaMode::Blend {
            1.0
        } else {
            0.0
        };
        push_f32s(
            &mut bytes,
            &[cutoff_of(self.alpha_mode).unwrap_or(0.0), blended, 0.0, 0.0],
        );
        bytes
    }
}

/// A material on the GPU, created by
/// [`Renderer::create_material`](crate::Renderer::create_material) or
/// [`Renderer::create_unlit_material`](crate::Renderer::create_unlit_material). It keeps its
/// textures and samplers alive. Any renderer of the same `Gpu` draws with it.
///
/// Clones refer to the same material, through one counted reference: a change of values through
/// one of them ([`Material::set_values`]) changes how every instance drawn with any of them looks.
#[derive(Clone, Debug)]
pub struct Material {
    pub(crate) inner: Arc<MaterialInner>,
}

/// What a material and its clones share.
#[derive(Debug)]
pub(crate) struct MaterialInner {
    pub(crate) group: BindGroup,
    /// The buffer that the scene shader reads its parameters from.
    uniform: UniformBuffer,
    /// Its parameters, a value of the type that its kind describes, which `uniform` holds as its
    /// kind packs them.
    parameters: Mutex<Value>,
    /// The label it was created with: a cooked material's name.
    pub(crate) label: Box<str>,
    /// Whether it has a normal texture, without which its normal scale is not used.
    pub(crate) normal_texture: bool,
    /// Whether its draws cull no face (ADR-0035).
    pub(crate) double_sided: bool,
    /// Whether its shadow is cut out where it discards fragments (ADR-0038).
    pub(crate) masked: bool,
    /// Whether it is laid over what is behind it ([`AlphaMode::Blend`]): drawn after the others,
    /// without writing its depth.
    pub(crate) blended: bool,
    /// Its kind, and the kind's index among a renderer's kinds.
    of_kind: &'static dyn MaterialKind,
    pub(crate) kind: usize,
}

impl PartialEq for Material {
    /// Whether the two are one material: one is a clone of the other, and a change of values
    /// through either changes both.
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for Material {}

impl Material {
    /// What it and its clones share, by which the renderer tells materials apart: clones are one
    /// material, drawn together.
    pub(crate) fn identity(&self) -> *const () {
        Arc::as_ptr(&self.inner).cast()
    }

    /// The name of its kind, the way it lights its surface (ADR-0111): `"physically based"` or
    /// `"unlit"` (ADR-0129).
    pub fn kind(&self) -> &'static str {
        self.inner.of_kind.name()
    }

    /// Its parameters, through reflection, as an editor shows them: a value of the type that its
    /// kind describes ([`Renderer::describe_materials`](crate::Renderer::describe_materials)),
    /// [`MaterialValues`] for the physically based kind, [`UnlitValues`] for the unlit one.
    pub fn parameters(&self) -> Value {
        self.inner
            .parameters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Changes its parameters to those that `parameters` describes, as an editor does, through
    /// reflection; its textures and its sidedness stay as made. Its kind converts them into what
    /// its shader reads. The work submitted to the GPU after this call draws with the new
    /// parameters, the work submitted before with the old ones: called between two frames
    /// ([`Renderer::render`](crate::Renderer::render)), as a game does, it changes the next frame
    /// whole. Once the device is lost, it changes the parameters that [`Material::parameters`]
    /// gives, and nothing that is drawn.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMaterial`] if its kind refuses `parameters`: a value that is not of
    /// its parameters' type, that holds a number out of its field's range or breaks a rule of the
    /// type, or an alpha mode other than the one it was made with, but for a mask's cutoff. The
    /// material is not changed then.
    ///
    /// # Panics
    ///
    /// If `gpu` is not the device the material was created with, and is not lost: wgpu panics.
    pub fn set_parameters(&self, gpu: &Gpu, parameters: &Value) -> Result<(), RenderError> {
        let bytes = self
            .inner
            .of_kind
            .pack(&self.inner, parameters)
            .map_err(|reason| RenderError::InvalidMaterial {
                label: self.inner.label.to_string(),
                reason,
            })?;
        // A panic while the lock is held, when `gpu` is another device, leaves the parameters
        // whole: the old ones. Under the lock, so that the buffer holds what the clones read.
        let mut current = self
            .inner
            .parameters
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        gpu.write_uniform_buffer(&self.inner.uniform, &bytes);
        *current = parameters.clone();
        Ok(())
    }

    /// The values it is drawn with, the parameters of the physically based kind: those it was
    /// created with, or the last that [`Material::set_values`] or [`Material::set_parameters`]
    /// gave it, or one of its clones.
    ///
    /// # Panics
    ///
    /// If it is not of the physically based kind: an unlit material's parameters are
    /// [`UnlitValues`].
    pub fn values(&self) -> MaterialValues {
        MaterialValues::from_value(&self.parameters())
            .expect("a physically based material's parameters are its values")
    }

    /// Changes the values it is drawn with to `values`, as [`Material::set_parameters`] does.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMaterial`] if a value is out of the range that [`MaterialValues`]
    /// gives, or if `values.alpha_mode` adds a cutoff to an opaque material or removes the cutoff
    /// of a masked one. The material is not changed then.
    ///
    /// # Panics
    ///
    /// As [`Material::set_parameters`].
    pub fn set_values(&self, gpu: &Gpu, values: MaterialValues) -> Result<(), RenderError> {
        // Said as creating a material says it, rather than as reflection does.
        if let Some(reason) = values.invalid() {
            return Err(RenderError::InvalidMaterial {
                label: self.inner.label.to_string(),
                reason,
            });
        }
        self.set_parameters(gpu, &values.to_value())
    }
}

/// What creating a material takes besides its data: the layout of its group, and the white pixel
/// and its sampler that fill the slots it leaves empty. The renderer keeps them, and gives a clone
/// to the threads that load scenes, so that they create materials too.
#[derive(Clone, Debug)]
pub(crate) struct MaterialParts {
    /// The layout of a physically based material's group, which the scene pass's pipelines read it
    /// through.
    pub(crate) layout: BindGroupLayout,
    /// The layout of an unlit material's group.
    pub(crate) unlit_layout: BindGroupLayout,
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
        if let Some(reason) = data.misplaced_texture() {
            panic!("material `{label}`: {reason}");
        }
        let normal_texture = data.normal_texture.is_some();
        let uniform = gpu.create_uniform_buffer(label, &values.bytes(normal_texture))?;
        let mut entries = vec![BindGroupEntry {
            binding: 0,
            resource: BindingResource::UniformBuffer(&uniform),
        }];
        for (binding, (_, sampled, _)) in (1u32..).step_by(2).zip(data.slots()) {
            let (texture, sampler) = match sampled {
                Some(sampled) => (sampled.texture, sampled.sampler),
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
            inner: Arc::new(MaterialInner {
                group,
                uniform,
                parameters: Mutex::new(values.to_value()),
                label: label.into(),
                normal_texture,
                double_sided: data.double_sided,
                masked: matches!(data.alpha_mode, AlphaMode::Mask { .. }),
                blended: false,
                kind: PHYSICALLY_BASED,
                of_kind: &PhysicallyBased,
            }),
        })
    }

    /// Creates an unlit material, as
    /// [`Renderer::create_unlit_material`](crate::Renderer::create_unlit_material) says.
    pub(crate) fn create_unlit(
        &self,
        gpu: &Gpu,
        label: &str,
        data: &UnlitData<'_>,
    ) -> Result<Material, RenderError> {
        let values = UnlitValues {
            color: data.color,
            alpha_mode: data.alpha_mode,
        };
        if let Some(reason) = values.invalid() {
            return Err(RenderError::InvalidMaterial {
                label: label.to_owned(),
                reason,
            });
        }
        let (texture, sampler) = match data.texture {
            Some(sampled) => {
                let format = sampled.texture.format();
                assert!(
                    format.is_srgb(),
                    "material `{label}`: its texture is {format:?}, but an unlit material reads \
                     colors, from an sRGB format"
                );
                (sampled.texture, sampled.sampler)
            }
            None => (&self.white, &self.sampler),
        };
        let uniform = gpu.create_uniform_buffer(label, &values.bytes())?;
        let group = gpu.create_bind_group(
            label,
            &self.unlit_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::UniformBuffer(&uniform),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Texture(texture),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        )?;
        Ok(Material {
            inner: Arc::new(MaterialInner {
                group,
                uniform,
                parameters: Mutex::new(values.to_value()),
                label: label.into(),
                normal_texture: false,
                double_sided: data.double_sided,
                masked: matches!(data.alpha_mode, AlphaMode::Mask { .. }),
                blended: data.alpha_mode == AlphaMode::Blend,
                kind: UNLIT,
                of_kind: &Unlit,
            }),
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
