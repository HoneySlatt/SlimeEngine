//! Cooked textures: every level of a texture in the format the GPU samples, kept in a KTX2 file,
//! the Khronos Group's container for GPU textures.

use ktx2::dfd;

use crate::bytes::Reader;
use crate::error::CookedError;

/// The longest side of a texture, in texels: the largest that every GPU takes at the engine's
/// floor (`slime_gfx::Gpu::max_texture_dimension`). The cooker refuses a larger image, and
/// [`CookedTexture::from_ktx2`] a larger texture.
pub const MAX_SIDE: u32 = 8192;

/// The twelve bytes that a KTX2 file starts with.
const KTX2_IDENTIFIER: [u8; 12] = [
    0xab, 0x4b, 0x54, 0x58, 0x20, 0x32, 0x30, 0xbb, 0x0d, 0x0a, 0x1a, 0x0a,
];

/// How the texels of a cooked texture are stored, which sets how the GPU reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureEncoding {
    /// BC7 blocks of 4 × 4 texels, 16 bytes each, sRGB-encoded: colors, which the GPU decodes to
    /// linear values.
    Bc7Srgb,
    /// BC7 blocks, linear: data that is not a color, such as occlusion, roughness and metalness.
    Bc7,
    /// BC5 blocks, linear: two channels, the x and y of a normal map, whose z the renderer
    /// rebuilds.
    Bc5,
    /// Red, green, blue and alpha, a byte each, sRGB-encoded: colors of an image smaller than a
    /// block of 4 × 4 texels, along either side, which the cooker keeps so; it stretches others
    /// whose sides are not multiples of 4 to whole blocks
    /// (`docs/adr/0106-the-measured-quick-wins.md`).
    Rgba8Srgb,
    /// Red, green, blue and alpha, a byte each, linear: data or normals of an image smaller than a
    /// block, as [`TextureEncoding::Rgba8Srgb`] says.
    Rgba8,
}

impl TextureEncoding {
    /// Whether the texels are sRGB-encoded colors.
    pub fn is_srgb(self) -> bool {
        matches!(self, Self::Bc7Srgb | Self::Rgba8Srgb)
    }

    /// Whether the texels are stored in blocks of 4 × 4.
    pub fn is_block_compressed(self) -> bool {
        matches!(self, Self::Bc7Srgb | Self::Bc7 | Self::Bc5)
    }

    /// The bytes of a level of `width` × `height` texels: 16 per block of 4 × 4 texels, a block
    /// counted whole at the right and bottom edges, or 4 per texel.
    pub fn level_size(self, width: u32, height: u32) -> usize {
        if self.is_block_compressed() {
            width.div_ceil(4) as usize * height.div_ceil(4) as usize * 16
        } else {
            width as usize * height as usize * 4
        }
    }

    /// The Vulkan format that KTX2 files name the encoding by.
    fn format(self) -> ktx2::Format {
        match self {
            Self::Bc7Srgb => ktx2::Format::BC7_SRGB_BLOCK,
            Self::Bc7 => ktx2::Format::BC7_UNORM_BLOCK,
            Self::Bc5 => ktx2::Format::BC5_UNORM_BLOCK,
            Self::Rgba8Srgb => ktx2::Format::R8G8B8A8_SRGB,
            Self::Rgba8 => ktx2::Format::R8G8B8A8_UNORM,
        }
    }

    /// The encoding of the Vulkan format numbered `format`, if it is a cooked one.
    fn from_format(format: u32) -> Option<Self> {
        [
            Self::Bc7Srgb,
            Self::Bc7,
            Self::Bc5,
            Self::Rgba8Srgb,
            Self::Rgba8,
        ]
        .into_iter()
        .find(|encoding| encoding.format().value() == format)
    }
}

/// A cooked texture: its size, its encoding, and its levels of detail, each half the size of the
/// one before, rounded down and at least one texel, as the GPU sizes them, from the full size down
/// to one texel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CookedTexture {
    /// The width of the full-size level, in texels.
    pub width: u32,
    /// Its height, in texels.
    pub height: u32,
    /// How the texels are stored.
    pub encoding: TextureEncoding,
    /// Each level's texels or blocks, rows from top to bottom ([`TextureEncoding::level_size`]).
    pub levels: Vec<Vec<u8>>,
}

impl CookedTexture {
    /// The size of level `index`, in texels.
    pub fn level_dimensions(&self, index: usize) -> (u32, u32) {
        level_dimensions(self.width, self.height, index)
    }

    /// The texture as a KTX2 file: one image, no supercompression, a data format descriptor of the
    /// encoding, and the levels stored from the smallest to the largest, each at an offset that is
    /// a multiple of its blocks' or texels' size, as the KTX2 specification asks.
    ///
    /// # Panics
    ///
    /// If the levels are not those of [`CookedTexture`], each of [`TextureEncoding::level_size`],
    /// or if a side is longer than [`MAX_SIDE`]: [`CookedTexture::from_ktx2`] would refuse the
    /// file.
    pub fn to_ktx2(&self) -> Vec<u8> {
        self.check().unwrap_or_else(|reason| panic!("{reason}"));
        let format = self.encoding.format();
        let (basic, type_size) =
            dfd::Basic::from_format(format).expect("ktx2 describes every encoding");
        let block = dfd::Block::Basic(basic).to_vec();
        let level_count = self.levels.len();
        let dfd_offset = ktx2::Header::LENGTH + level_count * ktx2::LevelIndex::LENGTH;
        let dfd_length = 4 + block.len();
        let alignment = if self.encoding.is_block_compressed() {
            16
        } else {
            4
        };
        // Level data starts after the descriptor, smallest level first.
        let mut offset = dfd_offset + dfd_length;
        let mut placed = vec![0; level_count];
        for index in (0..level_count).rev() {
            offset = offset.next_multiple_of(alignment);
            placed[index] = offset;
            offset += self.levels[index].len();
        }
        let header = ktx2::Header {
            format: Some(format),
            type_size,
            pixel_width: self.width,
            pixel_height: self.height,
            pixel_depth: 0,
            layer_count: 0,
            face_count: 1,
            level_count: level_count as u32,
            supercompression_scheme: None,
            index: ktx2::Index {
                dfd_byte_offset: dfd_offset as u32,
                dfd_byte_length: dfd_length as u32,
                kvd_byte_offset: 0,
                kvd_byte_length: 0,
                sgd_byte_offset: 0,
                sgd_byte_length: 0,
            },
        };
        let mut file = Vec::with_capacity(offset);
        file.extend_from_slice(&header.as_bytes());
        for (level, &start) in self.levels.iter().zip(&placed) {
            let entry = ktx2::LevelIndex {
                byte_offset: start as u64,
                byte_length: level.len() as u64,
                uncompressed_byte_length: level.len() as u64,
            };
            file.extend_from_slice(&entry.as_bytes());
        }
        file.extend_from_slice(&(dfd_length as u32).to_le_bytes());
        file.extend_from_slice(&block);
        for index in (0..level_count).rev() {
            file.resize(placed[index], 0);
            file.extend_from_slice(&self.levels[index]);
        }
        file
    }

    /// The texture that a KTX2 file of [`CookedTexture::to_ktx2`] holds, read by the engine's own
    /// reader of that layout: its header and its index of levels are checked first, its sides
    /// against [`MAX_SIDE`], its count of levels against its sides, and each level's place and size
    /// against the file, before any level is copied. The texture takes at most the file's size in
    /// memory, and a time linear in it. The data format descriptor, which says the format again,
    /// and the key-value data are not read.
    ///
    /// # Errors
    ///
    /// [`CookedError::Malformed`] if the file is not such a KTX2 file: not KTX2, cut short, of
    /// another encoding, supercompressed, of several images or of a depth, larger than
    /// [`MAX_SIDE`], or with levels that are not those of its size, or that lie outside the file.
    pub fn from_ktx2(bytes: &[u8]) -> Result<Self, CookedError> {
        let mut reader = Reader::new(bytes, "texture");
        if reader.take(12)? != KTX2_IDENTIFIER {
            return Err(reader.malformed("it does not start with the KTX2 identifier"));
        }
        let format = reader.u32()?;
        let encoding = TextureEncoding::from_format(format).ok_or_else(|| {
            reader.malformed(&format!("Vulkan format {format} is not a cooked one"))
        })?;
        let _type_size = reader.u32()?;
        let width = reader.u32()?;
        let height = reader.u32()?;
        let depth = reader.u32()?;
        let layers = reader.u32()?;
        let faces = reader.u32()?;
        let level_count = reader.u32()? as usize;
        let supercompression = reader.u32()?;
        if supercompression != 0 || depth != 0 || layers != 0 || faces != 1 {
            return Err(reader.malformed("it is not one plain two-dimensional image"));
        }
        check_size(width, height, encoding, level_count)
            .map_err(|reason| reader.malformed(&reason))?;
        // The offsets and lengths of the descriptors, which are not read.
        reader.take(32)?;
        let mut ranges = Vec::with_capacity(level_count);
        let mut total = 0_usize;
        for index in 0..level_count {
            let start = reader.u64()?;
            let length = reader.u64()?;
            let _uncompressed = reader.u64()?;
            let (level_width, level_height) = level_dimensions(width, height, index);
            let expected = encoding.level_size(level_width, level_height);
            let within = start
                .checked_add(length)
                .is_some_and(|end| end <= bytes.len() as u64);
            if length != expected as u64 || !within {
                return Err(reader.malformed(&format!(
                    "level {index}, of {level_width}x{level_height} texels, takes {length} bytes \
                     from byte {start}, where it needs {expected} bytes within the file's {}",
                    bytes.len()
                )));
            }
            total += expected;
            ranges.push(start as usize..(start + length) as usize);
        }
        if total > bytes.len() {
            return Err(reader.malformed(&format!(
                "its levels take {total} bytes, more than the file's {}",
                bytes.len()
            )));
        }
        Ok(Self {
            width,
            height,
            encoding,
            levels: ranges
                .into_iter()
                .map(|range| bytes[range].to_vec())
                .collect(),
        })
    }

    /// Why the texture is not one the GPU can take, if it is not.
    fn check(&self) -> Result<(), String> {
        check_size(self.width, self.height, self.encoding, self.levels.len())?;
        for (index, level) in self.levels.iter().enumerate() {
            let (width, height) = self.level_dimensions(index);
            let expected = self.encoding.level_size(width, height);
            if level.len() != expected {
                return Err(format!(
                    "level {index}, of {width}x{height} texels, holds {} bytes instead of \
                     {expected}",
                    level.len()
                ));
            }
        }
        Ok(())
    }
}

/// The size of level `index` of a texture of `width` × `height` texels, in texels.
fn level_dimensions(width: u32, height: u32, index: usize) -> (u32, u32) {
    let shift = |side: u32| {
        u32::try_from(index)
            .ok()
            .and_then(|index| side.checked_shr(index))
            .unwrap_or(0)
            .max(1)
    };
    (shift(width), shift(height))
}

/// Why a texture of `width` × `height` texels in `encoding`, with `levels` levels, is not one the
/// GPU can take, if it is not.
fn check_size(
    width: u32,
    height: u32,
    encoding: TextureEncoding,
    levels: usize,
) -> Result<(), String> {
    if width == 0 || height == 0 || width.max(height) > MAX_SIDE {
        return Err(format!(
            "{width}x{height} texels, where a texture takes 1 to {MAX_SIDE} on a side"
        ));
    }
    let full_chain = (u32::BITS - width.max(height).leading_zeros()) as usize;
    if levels == 0 || levels > full_chain {
        return Err(format!(
            "{levels} levels for {width}x{height} texels, which take 1 to {full_chain}"
        ));
    }
    if encoding.is_block_compressed() && !(width.is_multiple_of(4) && height.is_multiple_of(4)) {
        return Err(format!("{width}x{height} texels in blocks of 4x4"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A texture of `encoding`, `width` × `height`, every level down to one texel filled with its
    /// index.
    fn texture(encoding: TextureEncoding, width: u32, height: u32) -> CookedTexture {
        let count = (u32::BITS - width.max(height).leading_zeros()) as usize;
        let mut texture = CookedTexture {
            width,
            height,
            encoding,
            levels: Vec::new(),
        };
        for index in 0..count {
            let (w, h) = texture.level_dimensions(index);
            texture
                .levels
                .push(vec![index as u8 + 1; encoding.level_size(w, h)]);
        }
        texture
    }

    #[test]
    fn a_texture_is_read_back_from_its_ktx2_file() {
        for (encoding, width, height) in [
            (TextureEncoding::Bc7Srgb, 16, 8),
            (TextureEncoding::Bc7, 4, 4),
            (TextureEncoding::Bc5, 8, 12),
            (TextureEncoding::Rgba8Srgb, 5, 3),
            (TextureEncoding::Rgba8, 1, 1),
        ] {
            let original = texture(encoding, width, height);
            let file = original.to_ktx2();
            assert_eq!(
                CookedTexture::from_ktx2(&file),
                Ok(original),
                "{encoding:?}"
            );
        }
    }

    #[test]
    fn the_file_follows_the_ktx2_layout() {
        let file = texture(TextureEncoding::Bc7Srgb, 16, 8).to_ktx2();
        let reader = ktx2::Reader::new(&file[..]).expect("valid KTX2");
        assert_eq!(
            reader.transfer_function(),
            Some(ktx2::TransferFunction::SRGB)
        );
        let header = reader.header();
        assert_eq!(
            (header.pixel_width, header.pixel_height, header.level_count),
            (16, 8, 5)
        );
        // Smallest level first, each at a multiple of 16.
        let offsets: Vec<usize> = reader
            .levels()
            .map(|level| level.data.as_ptr() as usize - file.as_ptr() as usize)
            .collect();
        assert!(
            offsets.windows(2).all(|pair| pair[0] > pair[1]),
            "{offsets:?}"
        );
        assert!(offsets.iter().all(|offset| offset % 16 == 0), "{offsets:?}");
    }

    #[test]
    fn a_file_that_is_not_a_cooked_texture_is_refused() {
        assert!(matches!(
            CookedTexture::from_ktx2(b"not a texture"),
            Err(CookedError::Malformed { .. })
        ));
        // A level of the wrong size.
        let mut wrong = texture(TextureEncoding::Rgba8, 4, 4);
        wrong.levels[1].pop();
        let mut file = texture(TextureEncoding::Rgba8, 4, 4).to_ktx2();
        assert!(CookedTexture::from_ktx2(&file).is_ok());
        // Cut short: the last level's bytes are gone.
        file.truncate(file.len() - 8);
        assert!(CookedTexture::from_ktx2(&file).is_err());
        assert!(wrong.check().is_err());
    }

    #[test]
    fn a_texture_longer_than_the_gpu_takes_is_refused() {
        assert!(texture(TextureEncoding::Rgba8, MAX_SIDE, 1).check().is_ok());
        let error = texture(TextureEncoding::Rgba8, MAX_SIDE * 2, 1)
            .check()
            .expect_err("refused");
        assert!(error.contains("8192"), "{error}");
        // A file that says it is twice as wide: refused for its width, before its levels are read.
        let mut file = texture(TextureEncoding::Rgba8, MAX_SIDE, 1).to_ktx2();
        file[20..24].copy_from_slice(&(MAX_SIDE * 2).to_le_bytes());
        let error = CookedTexture::from_ktx2(&file).expect_err("refused");
        assert!(error.to_string().contains("1 to 8192 on a side"), "{error}");
    }
}
