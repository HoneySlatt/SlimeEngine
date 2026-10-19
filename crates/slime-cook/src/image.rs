//! Importing images: PNG and JPEG files decoded into texels of 8-bit red, green, blue and alpha.

use std::io::Cursor;

use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

/// The longest side of an image the cooker takes, in texels: the largest texture the GPU takes with
/// the engine's default limits. Checked from the file's header, before its texels are allocated,
/// since a header can claim gigabytes.
pub const MAX_SIDE: u32 = 8192;

/// An image decoded into texels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Its width, in texels.
    pub width: u32,
    /// Its height, in texels.
    pub height: u32,
    /// Its texels, rows from top to bottom, each red, green, blue and alpha, a byte each, as the
    /// file stores them: sRGB-encoded for an image of colors.
    pub rgba: Vec<u8>,
}

/// Why an image cannot be imported.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ImageError {
    /// The file is not a PNG or JPEG image the decoder can read.
    #[error("cannot be decoded: {0}")]
    Decode(String),
    /// The image is larger than the GPU takes.
    #[error("is {width}x{height} texels; the cooker takes images up to {MAX_SIDE} on a side")]
    TooLarge {
        /// Its width, from its header.
        width: u32,
        /// Its height.
        height: u32,
    },
}

/// The kinds of image file the cooker imports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    /// PNG, of any color type and depth: palettes expanded, 16-bit channels cut to their most
    /// significant byte, gray spread to red, green and blue, a missing alpha opaque.
    Png,
    /// JPEG, baseline or progressive, in YCbCr, gray, CMYK or YCCK, converted to RGB by the decoder,
    /// alpha opaque.
    Jpeg,
}

impl ImageKind {
    /// The kind of the file of this extension, without regard to case.
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }
}

/// Decodes `bytes`, an image file of `kind`.
///
/// # Errors
///
/// [`ImageError::Decode`] if the file is not a valid image of its kind, and
/// [`ImageError::TooLarge`] if a side is longer than [`MAX_SIDE`].
pub fn decode(kind: ImageKind, bytes: &[u8]) -> Result<Image, ImageError> {
    match kind {
        ImageKind::Png => decode_png(bytes),
        ImageKind::Jpeg => decode_jpeg(bytes),
    }
}

fn check_size(width: u32, height: u32) -> Result<(), ImageError> {
    if width.max(height) > MAX_SIDE {
        Err(ImageError::TooLarge { width, height })
    } else {
        Ok(())
    }
}

fn decode_png(bytes: &[u8]) -> Result<Image, ImageError> {
    let error = |what: &dyn std::fmt::Display| ImageError::Decode(what.to_string());
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    // 8 bits per channel, palettes expanded, whatever the file stores.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| error(&e))?;
    let (width, height) = reader.info().size();
    check_size(width, height)?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| error(&"it is too large"))?;
    let mut buffer = vec![0; size];
    let info = reader.next_frame(&mut buffer).map_err(|e| error(&e))?;
    buffer.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buffer
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        png::ColorType::Indexed => return Err(error(&"its palette was not expanded")),
    };
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}

fn decode_jpeg(bytes: &[u8]) -> Result<Image, ImageError> {
    let error = |what: &dyn std::fmt::Display| ImageError::Decode(what.to_string());
    // RGB, then alpha added here: zune-jpeg 0.5.15 converts CMYK and YCCK to RGBA with the stride
    // of RGB, which shifts the colors of every texel; to RGB, it converts them right.
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGB)
        .set_max_width(MAX_SIDE as usize)
        .set_max_height(MAX_SIDE as usize);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().map_err(|e| error(&e))?;
    let info = decoder
        .info()
        .ok_or_else(|| error(&"it has no image header"))?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    check_size(width, height)?;
    let rgb = decoder.decode().map_err(|e| error(&e))?;
    if rgb.len() != width as usize * height as usize * 3 {
        return Err(error(&"the decoder gave texels of another size"));
    }
    Ok(Image {
        width,
        height,
        rgba: rgb
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_images_of_every_kind_are_decoded_into_rgba() {
        use png::{BitDepth, ColorType};

        // An image of two texels side by side, of `color` in `depth`, stored as `data`, with
        // `palette` and `transparency` chunks unless they are empty, decoded.
        let decoded = |color, depth, data: &[u8], palette: &[u8], transparency: &[u8]| {
            let mut bytes = Vec::new();
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(color);
            encoder.set_depth(depth);
            if !palette.is_empty() {
                encoder.set_palette(palette.to_vec());
            }
            if !transparency.is_empty() {
                encoder.set_trns(transparency.to_vec());
            }
            let mut writer = encoder.write_header().expect("the PNG header is written");
            writer
                .write_image_data(data)
                .expect("the PNG image is written");
            writer.finish().expect("the PNG image is finished");
            let image = decode(ImageKind::Png, &bytes).expect("the image is decoded");
            assert_eq!((image.width, image.height), (2, 1));
            image.rgba
        };
        let (eight, none) = (BitDepth::Eight, &[]);
        assert_eq!(
            decoded(
                ColorType::Rgba,
                eight,
                &[10, 20, 30, 40, 50, 60, 70, 80],
                none,
                none
            ),
            [10, 20, 30, 40, 50, 60, 70, 80]
        );
        let rgb = [10, 20, 30, 50, 60, 70];
        assert_eq!(
            decoded(ColorType::Rgb, eight, &rgb, none, none),
            [10, 20, 30, 255, 50, 60, 70, 255]
        );
        // The color of the transparency chunk, written in 16 bits per channel, is transparent.
        assert_eq!(
            decoded(ColorType::Rgb, eight, &rgb, none, &[0, 10, 0, 20, 0, 30]),
            [10, 20, 30, 0, 50, 60, 70, 255]
        );
        // 16 bits per channel, big-endian: the most significant byte is kept.
        assert_eq!(
            decoded(
                ColorType::Rgb,
                BitDepth::Sixteen,
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
                none,
                none
            ),
            [1, 3, 5, 255, 7, 9, 11, 255]
        );
        assert_eq!(
            decoded(ColorType::Grayscale, eight, &[10, 200], none, none),
            [10, 10, 10, 255, 200, 200, 200, 255]
        );
        // One bit per texel, from the most significant bit: white, then black.
        assert_eq!(
            decoded(
                ColorType::Grayscale,
                BitDepth::One,
                &[0b1000_0000],
                none,
                none
            ),
            [255, 255, 255, 255, 0, 0, 0, 255]
        );
        assert_eq!(
            decoded(
                ColorType::GrayscaleAlpha,
                eight,
                &[10, 40, 200, 80],
                none,
                none
            ),
            [10, 10, 10, 40, 200, 200, 200, 80]
        );
        // A palette of red and blue, without and with an alpha per entry.
        let palette = [255, 0, 0, 0, 0, 255];
        assert_eq!(
            decoded(ColorType::Indexed, eight, &[0, 1], &palette, none),
            [255, 0, 0, 255, 0, 0, 255, 255]
        );
        assert_eq!(
            decoded(ColorType::Indexed, eight, &[0, 1], &palette, &[128, 255]),
            [255, 0, 0, 128, 0, 0, 255, 255]
        );
    }

    #[test]
    fn a_jpeg_image_is_decoded_into_opaque_rgba() {
        // 8 × 4 texels of one blue, written by ImageMagick at quality 95: within a few steps of it.
        let bytes = include_bytes!("../tests/fixtures/blue.jpg");
        let image = decode(ImageKind::Jpeg, bytes).expect("the image is decoded");
        assert_eq!((image.width, image.height), (8, 4));
        for texel in image.rgba.chunks(4) {
            for (got, want) in texel.iter().zip([48, 128, 192, 255]) {
                assert!(got.abs_diff(want) <= 3, "{texel:?}");
            }
        }
    }

    #[test]
    fn a_jpeg_image_in_cmyk_is_decoded_into_its_colors() {
        // 48 × 8 texels, stripes of 8 of red, green, blue, white, grey and black, converted to CMYK
        // and written by Pillow at quality 100.
        let bytes = include_bytes!("../tests/fixtures/stripes-cmyk.jpg");
        let image = decode(ImageKind::Jpeg, bytes).expect("the image is decoded");
        assert_eq!((image.width, image.height), (48, 8));
        let stripes = [
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [255, 255, 255],
            [128, 128, 128],
            [0, 0, 0],
        ];
        for (index, texel) in image.rgba.as_chunks::<4>().0.iter().enumerate() {
            let want = stripes[index % 48 / 8];
            let near = texel[..3]
                .iter()
                .zip(want)
                .all(|(got, want)| got.abs_diff(want) <= 3);
            assert!(
                near && texel[3] == 255,
                "texel {index}: {texel:?}, not {want:?}"
            );
        }
    }

    #[test]
    fn an_image_larger_than_the_gpu_takes_is_refused_from_its_header() {
        // 8193 × 1 texels: the size is refused from the header, before the texels are read.
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, MAX_SIDE + 1, 1);
        encoder.set_color(png::ColorType::Grayscale);
        let mut writer = encoder.write_header().expect("the PNG header is written");
        writer
            .write_image_data(&vec![0; MAX_SIDE as usize + 1])
            .expect("the PNG image is written");
        writer.finish().expect("the PNG image is finished");
        assert_eq!(
            decode(ImageKind::Png, &bytes),
            Err(ImageError::TooLarge {
                width: MAX_SIDE + 1,
                height: 1
            })
        );
    }

    #[test]
    fn a_file_that_is_not_an_image_is_refused() {
        for kind in [ImageKind::Png, ImageKind::Jpeg] {
            assert!(matches!(
                decode(kind, b"not an image"),
                Err(ImageError::Decode(_))
            ));
        }
        assert_eq!(ImageKind::from_extension("JPEG"), Some(ImageKind::Jpeg));
        assert_eq!(ImageKind::from_extension("glb"), None);
    }
}
