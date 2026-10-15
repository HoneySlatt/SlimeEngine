//! Cooked textures: every level of a texture in the format the GPU samples, kept in a KTX2 file,
//! the Khronos Group's container for GPU textures.

use ktx2::dfd;

use crate::error::CookedError;

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
    /// Red, green, blue and alpha, a byte each, sRGB-encoded: colors whose sides are not
    /// multiples of 4, which blocks cannot hold.
    Rgba8Srgb,
    /// Red, green, blue and alpha, a byte each, linear: data or normals whose sides are not
    /// multiples of 4.
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

    fn from_format(format: ktx2::Format) -> Option<Self> {
        [
            Self::Bc7Srgb,
            Self::Bc7,
            Self::Bc5,
            Self::Rgba8Srgb,
            Self::Rgba8,
        ]
        .into_iter()
        .find(|encoding| encoding.format() == format)
    }
}

/// A cooked texture: its size, its encoding, and its levels of detail, each half the size of the
/// one before, rounded down and at least one texel, as the GPU sizes them, from the full size down
/// to one texel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureData {
    /// The width of the full-size level, in texels.
    pub width: u32,
    /// Its height, in texels.
    pub height: u32,
    /// How the texels are stored.
    pub encoding: TextureEncoding,
    /// Each level's texels or blocks, rows from top to bottom ([`TextureEncoding::level_size`]).
    pub levels: Vec<Vec<u8>>,
}

impl TextureData {
    /// The size of level `index`, in texels.
    pub fn level_dimensions(&self, index: usize) -> (u32, u32) {
        ((self.width >> index).max(1), (self.height >> index).max(1))
    }

    /// The texture as a KTX2 file: one image, no supercompression, a data format descriptor of the
    /// encoding, and the levels stored from the smallest to the largest, each at an offset that is
    /// a multiple of its blocks' or texels' size, as the KTX2 specification asks.
    ///
    /// # Panics
    ///
    /// If the levels are not those of [`TextureData`], each of [`TextureEncoding::level_size`].
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

    /// The texture that a KTX2 file of [`TextureData::to_ktx2`] holds.
    ///
    /// # Errors
    ///
    /// [`CookedError::Malformed`] if the file is not such a KTX2 file: not KTX2, of another
    /// encoding, supercompressed, of several images or of a depth, or with levels of the wrong
    /// size.
    pub fn from_ktx2(bytes: &[u8]) -> Result<Self, CookedError> {
        let malformed = |reason: String| CookedError::Malformed {
            what: "texture",
            reason,
        };
        let reader = ktx2::Reader::new(bytes).map_err(|error| malformed(error.to_string()))?;
        let header = reader.header();
        let encoding = header
            .format
            .and_then(TextureEncoding::from_format)
            .ok_or_else(|| malformed(format!("format {:?} is not a cooked one", header.format)))?;
        if header.supercompression_scheme.is_some()
            || header.pixel_depth != 0
            || header.layer_count != 0
            || header.face_count != 1
        {
            return Err(malformed("not one plain two-dimensional image".to_owned()));
        }
        let texture = Self {
            width: header.pixel_width,
            height: header.pixel_height,
            encoding,
            levels: reader.levels().map(|level| level.data.to_vec()).collect(),
        };
        texture.check().map_err(malformed)?;
        Ok(texture)
    }

    /// Why the texture is not one the GPU can take, if it is not.
    fn check(&self) -> Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Err(format!("{}x{} texels", self.width, self.height));
        }
        let full_chain = (u32::BITS - self.width.max(self.height).leading_zeros()) as usize;
        if self.levels.is_empty() || self.levels.len() > full_chain {
            return Err(format!(
                "{} levels for {}x{} texels, which take 1 to {full_chain}",
                self.levels.len(),
                self.width,
                self.height
            ));
        }
        if self.encoding.is_block_compressed()
            && !(self.width.is_multiple_of(4) && self.height.is_multiple_of(4))
        {
            return Err(format!(
                "{}x{} texels in blocks of 4x4",
                self.width, self.height
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A texture of `encoding`, `width` × `height`, every level down to one texel filled with its
    /// index.
    fn texture(encoding: TextureEncoding, width: u32, height: u32) -> TextureData {
        let count = (u32::BITS - width.max(height).leading_zeros()) as usize;
        let mut texture = TextureData {
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
            assert_eq!(TextureData::from_ktx2(&file), Ok(original), "{encoding:?}");
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
            TextureData::from_ktx2(b"not a texture"),
            Err(CookedError::Malformed { .. })
        ));
        // A level of the wrong size.
        let mut wrong = texture(TextureEncoding::Rgba8, 4, 4);
        wrong.levels[1].pop();
        let mut file = texture(TextureEncoding::Rgba8, 4, 4).to_ktx2();
        assert!(TextureData::from_ktx2(&file).is_ok());
        // Cut short: the last level's bytes are gone.
        file.truncate(file.len() - 8);
        assert!(TextureData::from_ktx2(&file).is_err());
        assert!(wrong.check().is_err());
    }
}
