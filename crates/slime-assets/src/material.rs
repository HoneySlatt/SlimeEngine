//! Cooked materials: glTF's metallic-roughness model, its textures named by ID.

use crate::AssetId;
use crate::bytes::{Reader, Writer};
use crate::error::CookedError;

const MAGIC: &[u8; 4] = b"SLMT";
/// The version of the format of cooked materials; a change of format changes it.
const VERSION: u32 = 1;

/// How a texture is filtered between its texels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureFilter {
    /// The nearest texel.
    Nearest,
    /// A blend of the nearest texels.
    Linear,
}

/// What lies beyond a texture's edges, along one direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureWrap {
    /// The texture again.
    Repeat,
    /// The texture again, mirrored every other time.
    MirroredRepeat,
    /// The edge's texels, stretched.
    ClampToEdge,
}

/// How a material reads one of its textures: glTF's sampler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureSampler {
    /// When the texture is magnified.
    pub mag_filter: TextureFilter,
    /// When it is minified, which also says how its levels are read: from the nearest level, or a
    /// blend of two.
    pub min_filter: TextureFilter,
    /// Beyond its left and right edges.
    pub wrap_u: TextureWrap,
    /// Beyond its top and bottom edges.
    pub wrap_v: TextureWrap,
}

/// One texture of a material: the cooked texture, by ID, and how it is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialTexture {
    /// The cooked texture: of colors for the base color and emission, of data for the
    /// metallic-roughness and occlusion, of normals for the normal texture.
    pub texture: AssetId,
    /// How it is read.
    pub sampler: TextureSampler,
}

/// A cooked material: glTF's metallic-roughness model, as `slime_render::MaterialData` takes it,
/// its five textures named by the IDs of cooked textures.
#[derive(Clone, Debug, PartialEq)]
pub struct CookedMaterial {
    /// Its name, for labels and messages.
    pub name: String,
    /// The base color, linear red, green, blue and alpha.
    pub base_color: [f32; 4],
    /// Multiplies the base color.
    pub base_color_texture: Option<MaterialTexture>,
    /// 0 for a dielectric, 1 for a metal.
    pub metallic: f32,
    /// From 0, a mirror, to 1.
    pub roughness: f32,
    /// Green multiplies the roughness, blue the metalness.
    pub metallic_roughness_texture: Option<MaterialTexture>,
    /// Normals in tangent space.
    pub normal_texture: Option<MaterialTexture>,
    /// Multiplies the normal texture's x and y.
    pub normal_scale: f32,
    /// Red is how much of the environment's light reaches the surface.
    pub occlusion_texture: Option<MaterialTexture>,
    /// How much the occlusion texture counts.
    pub occlusion_strength: f32,
    /// The light emitted, linear red, green and blue.
    pub emissive: [f32; 3],
    /// Multiplies the emission (`KHR_materials_emissive_strength`).
    pub emissive_strength: f32,
    /// Multiplies the emission.
    pub emissive_texture: Option<MaterialTexture>,
    /// glTF's `MASK` alpha mode and its cutoff: nothing is drawn where the base color's alpha is
    /// below it. `None` for `OPAQUE`.
    pub alpha_cutoff: Option<f32>,
    /// Whether back faces are drawn too.
    pub double_sided: bool,
}

impl CookedMaterial {
    /// Its textures, in the renderer's order: base color, metallic-roughness, normal, occlusion,
    /// emissive.
    pub fn textures(&self) -> [Option<MaterialTexture>; 5] {
        [
            self.base_color_texture,
            self.metallic_roughness_texture,
            self.normal_texture,
            self.occlusion_texture,
            self.emissive_texture,
        ]
    }

    /// The IDs of the cooked textures it reads, in the renderer's order, each once.
    pub fn needs(&self) -> Vec<AssetId> {
        let mut needs = Vec::new();
        for slot in self.textures().into_iter().flatten() {
            if !needs.contains(&slot.texture) {
                needs.push(slot.texture);
            }
        }
        needs
    }

    /// The material's file, which [`CookedMaterial::from_bytes`] reads back: the format's four
    /// bytes, `SLMT`, and version, then its name and its values in the order of its fields, a
    /// texture as a byte, 0 for none or 1, then its ID and its sampler's four bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::new(MAGIC, VERSION);
        writer.text(&self.name);
        writer.f32s(&self.base_color);
        texture(&mut writer, self.base_color_texture);
        writer.f32(self.metallic);
        writer.f32(self.roughness);
        texture(&mut writer, self.metallic_roughness_texture);
        texture(&mut writer, self.normal_texture);
        writer.f32(self.normal_scale);
        texture(&mut writer, self.occlusion_texture);
        writer.f32(self.occlusion_strength);
        writer.f32s(&self.emissive);
        writer.f32(self.emissive_strength);
        texture(&mut writer, self.emissive_texture);
        match self.alpha_cutoff {
            None => writer.u8(0),
            Some(cutoff) => {
                writer.u8(1);
                writer.f32(cutoff);
            }
        }
        writer.u8(u8::from(self.double_sided));
        writer.finish()
    }

    /// The material that `bytes`, a file of [`CookedMaterial::to_bytes`], holds.
    ///
    /// # Errors
    ///
    /// [`CookedError::Version`] if another version of the format wrote it, and
    /// [`CookedError::Malformed`] if it is not a material, or is cut short.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CookedError> {
        let mut reader = Reader::open(bytes, MAGIC, VERSION, "material")?;
        let material = Self {
            name: reader.text()?,
            base_color: reader.f32s()?,
            base_color_texture: read_texture(&mut reader)?,
            metallic: reader.f32()?,
            roughness: reader.f32()?,
            metallic_roughness_texture: read_texture(&mut reader)?,
            normal_texture: read_texture(&mut reader)?,
            normal_scale: reader.f32()?,
            occlusion_texture: read_texture(&mut reader)?,
            occlusion_strength: reader.f32()?,
            emissive: reader.f32s()?,
            emissive_strength: reader.f32()?,
            emissive_texture: read_texture(&mut reader)?,
            alpha_cutoff: match reader.u8()? {
                0 => None,
                1 => Some(reader.f32()?),
                _ => return Err(reader.malformed("its alpha mode is unknown")),
            },
            double_sided: match reader.u8()? {
                0 => false,
                1 => true,
                _ => return Err(reader.malformed("its sidedness is unknown")),
            },
        };
        reader.finish()?;
        Ok(material)
    }
}

impl Default for CookedMaterial {
    /// glTF's default material, which a primitive without one takes: white, fully metallic, fully
    /// rough, opaque, single-sided, without textures.
    fn default() -> Self {
        Self {
            name: "default".to_owned(),
            base_color: [1.0; 4],
            base_color_texture: None,
            metallic: 1.0,
            roughness: 1.0,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            occlusion_texture: None,
            occlusion_strength: 1.0,
            emissive: [0.0; 3],
            emissive_strength: 1.0,
            emissive_texture: None,
            alpha_cutoff: None,
            double_sided: false,
        }
    }
}

fn texture(writer: &mut Writer, texture: Option<MaterialTexture>) {
    let Some(MaterialTexture { texture, sampler }) = texture else {
        writer.u8(0);
        return;
    };
    writer.u8(1);
    writer.id(texture);
    let filter = |filter| match filter {
        TextureFilter::Nearest => 0,
        TextureFilter::Linear => 1,
    };
    let wrap = |wrap| match wrap {
        TextureWrap::Repeat => 0,
        TextureWrap::MirroredRepeat => 1,
        TextureWrap::ClampToEdge => 2,
    };
    writer.u8(filter(sampler.mag_filter));
    writer.u8(filter(sampler.min_filter));
    writer.u8(wrap(sampler.wrap_u));
    writer.u8(wrap(sampler.wrap_v));
}

fn read_texture(reader: &mut Reader<'_>) -> Result<Option<MaterialTexture>, CookedError> {
    match reader.u8()? {
        0 => return Ok(None),
        1 => {}
        _ => return Err(reader.malformed("a texture slot is neither empty nor full")),
    }
    let texture = reader.id()?;
    let mut filter = || match reader.u8()? {
        0 => Ok(TextureFilter::Nearest),
        1 => Ok(TextureFilter::Linear),
        _ => Err(reader.malformed("a filter is unknown")),
    };
    let (mag_filter, min_filter) = (filter()?, filter()?);
    let mut wrap = || match reader.u8()? {
        0 => Ok(TextureWrap::Repeat),
        1 => Ok(TextureWrap::MirroredRepeat),
        2 => Ok(TextureWrap::ClampToEdge),
        _ => Err(reader.malformed("a wrapping is unknown")),
    };
    let (wrap_u, wrap_v) = (wrap()?, wrap()?);
    Ok(Some(MaterialTexture {
        texture,
        sampler: TextureSampler {
            mag_filter,
            min_filter,
            wrap_u,
            wrap_v,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_material_is_read_back_from_its_file() {
        let sampler = TextureSampler {
            mag_filter: TextureFilter::Nearest,
            min_filter: TextureFilter::Linear,
            wrap_u: TextureWrap::MirroredRepeat,
            wrap_v: TextureWrap::ClampToEdge,
        };
        let slot = |id| {
            Some(MaterialTexture {
                texture: AssetId::from_u128(id),
                sampler,
            })
        };
        let material = CookedMaterial {
            name: "stone".to_owned(),
            base_color: [0.5, 0.4, 0.3, 1.0],
            base_color_texture: slot(1),
            metallic: 0.0,
            roughness: 0.8,
            metallic_roughness_texture: slot(2),
            normal_texture: slot(3),
            normal_scale: 0.7,
            occlusion_texture: slot(2),
            occlusion_strength: 0.9,
            emissive: [0.0, 0.1, 0.0],
            emissive_strength: 3.0,
            emissive_texture: None,
            alpha_cutoff: Some(0.5),
            double_sided: true,
        };
        assert_eq!(
            CookedMaterial::from_bytes(&material.to_bytes()),
            Ok(material.clone())
        );
        assert_eq!(
            material.needs(),
            [1, 2, 3].map(AssetId::from_u128),
            "the occlusion shares the metallic-roughness texture"
        );
        let default = CookedMaterial::default();
        assert_eq!(CookedMaterial::from_bytes(&default.to_bytes()), Ok(default));
    }

    #[test]
    fn a_damaged_file_is_refused() {
        let bytes = CookedMaterial::default().to_bytes();
        for cut in [0, 8, bytes.len() - 1] {
            assert!(
                CookedMaterial::from_bytes(&bytes[..cut]).is_err(),
                "cut at {cut}"
            );
        }
        let mut wrong = bytes.clone();
        let last = wrong.len() - 1;
        wrong[last] = 7;
        assert!(CookedMaterial::from_bytes(&wrong).is_err());
    }
}
