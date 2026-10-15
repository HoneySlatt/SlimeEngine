//! Cooking textures: an image's levels of detail, filtered for what the texture is used for, and
//! compressed for the GPU.

use std::sync::LazyLock;
use std::thread;

use block_compression::encode::compress_rgba8;
use block_compression::{BC7Settings, CompressionVariant};
use slime_assets::{TextureData, TextureEncoding};

use crate::image::Image;

/// What a texture is used for, which sets how its levels are filtered and how it is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureRole {
    /// Colors, sRGB-encoded: a base color or an emission. Averaged in linear light; BC7 with sRGB.
    Color,
    /// Data that is not a color, such as occlusion, roughness and metalness. Averaged as stored;
    /// BC7 without sRGB.
    Data,
    /// A normal map in tangent space, x, y and z stored as red, green and blue from 0 to 255 for −1
    /// to 1. Averaged as vectors, then made of length one again; BC5, which keeps x and y, the
    /// renderer rebuilding z.
    Normal,
}

impl TextureRole {
    /// The byte that stands for the role in what a cooked texture's name is made from. Never
    /// changed.
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Color => 1,
            Self::Data => 2,
            Self::Normal => 3,
        }
    }
}

/// Cooks `image` into a texture for `role`: every level of detail down to one texel, each texel
/// the average of the texels it covers in the level before, as `slime_gfx::Gpu::create_texture`
/// filters them, but in the way the role asks; then compressed in BC7 or BC5 when both sides are
/// multiples of 4, which blocks need, and kept as 8-bit texels otherwise.
///
/// BC7 takes the "fast" settings of Intel's encoders, with alpha if a texel of the image is not
/// opaque: on a noisy test image of 2048 × 2048 texels, they made blocks as close to the image
/// as the "basic" settings (a peak signal-to-noise ratio of 55.3 dB) in less than half their
/// time, and every level of a texture of 4096 × 4096 texels takes about 5 s on the 16 threads of
/// the development machine (ADR-0071). The blocks of each level are encoded on every thread the
/// machine offers, each a band of rows; the result does not depend on their number. A level
/// smaller than a block repeats its last row and column to fill it.
pub fn cook_texture(image: &Image, role: TextureRole) -> TextureData {
    let levels = levels(image, role);
    let compressed = image.width.is_multiple_of(4) && image.height.is_multiple_of(4);
    let (encoding, variant) = match (role, compressed) {
        (TextureRole::Color, true) => {
            let opaque = image.rgba.chunks(4).all(|texel| texel[3] == 255);
            let settings = if opaque {
                BC7Settings::opaque_fast()
            } else {
                BC7Settings::alpha_fast()
            };
            (
                TextureEncoding::Bc7Srgb,
                Some(CompressionVariant::BC7(settings)),
            )
        }
        (TextureRole::Data, true) => (
            TextureEncoding::Bc7,
            Some(CompressionVariant::BC7(BC7Settings::opaque_fast())),
        ),
        (TextureRole::Normal, true) => (TextureEncoding::Bc5, Some(CompressionVariant::BC5)),
        (TextureRole::Color, false) => (TextureEncoding::Rgba8Srgb, None),
        (TextureRole::Data | TextureRole::Normal, false) => (TextureEncoding::Rgba8, None),
    };
    let levels = levels
        .into_iter()
        .map(|level| match variant {
            Some(variant) => encode(variant, &padded(&level)),
            None => level.rgba,
        })
        .collect();
    TextureData {
        width: image.width,
        height: image.height,
        encoding,
        levels,
    }
}

/// Every level of detail of `image`, from the image itself down to one texel.
fn levels(image: &Image, role: TextureRole) -> Vec<Image> {
    let count = u32::BITS - image.width.max(image.height).leading_zeros();
    let mut levels = vec![image.clone()];
    while levels.len() < count as usize {
        let next = halved(levels.last().expect("the full-size level is there"), role);
        levels.push(next);
    }
    levels
}

/// The next level of `level`: half its size, rounded down, at least one texel; each texel the
/// average of the texels of `level` it covers, each weighted by how much of it is covered (see
/// [`footprints`]). Colors are averaged in linear light, normals as vectors made of length one
/// again; alpha, and data, as stored.
fn halved(level: &Image, role: TextureRole) -> Image {
    let columns = footprints(level.width);
    let rows = footprints(level.height);
    let mut rgba = Vec::with_capacity(columns.len() * rows.len() * 4);
    for row in &rows {
        for column in &columns {
            let mut sum = [0.0f32; 4];
            for &(source_y, weight_y) in row {
                for &(source_x, weight_x) in column {
                    let weight = weight_x * weight_y;
                    let start = ((source_y * level.width + source_x) * 4) as usize;
                    let texel = &level.rgba[start..start + 4];
                    for channel in 0..4 {
                        let value = texel[channel];
                        sum[channel] += weight
                            * match role {
                                TextureRole::Color if channel < 3 => SRGB_TO_LINEAR[value as usize],
                                TextureRole::Normal if channel < 3 => {
                                    f32::from(value) / 255.0 * 2.0 - 1.0
                                }
                                _ => f32::from(value) / 255.0,
                            };
                    }
                }
            }
            match role {
                TextureRole::Color => {
                    rgba.extend(sum[..3].iter().map(|&value| linear_to_srgb(value)));
                }
                TextureRole::Data => rgba.extend(sum[..3].iter().map(|&value| unit_to_byte(value))),
                TextureRole::Normal => {
                    let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
                    // Normals that cancel out average to about nothing, less than a step of 8-bit
                    // storage, whose direction is noise: the surface's own normal stands.
                    let normal = if length > 0.01 {
                        [sum[0] / length, sum[1] / length, sum[2] / length]
                    } else {
                        [0.0, 0.0, 1.0]
                    };
                    rgba.extend(normal.iter().map(|&value| unit_to_byte(value * 0.5 + 0.5)));
                }
            }
            rgba.push(unit_to_byte(sum[3]));
        }
    }
    Image {
        width: columns.len() as u32,
        height: rows.len() as u32,
        rgba,
    }
}

/// Along one side of a level, `size` texels long: for each texel of the next level, the texels of
/// this one it covers, with their weights, which add up to 1. The same footprints as
/// `slime_gfx::Gpu::create_texture`'s, so that a cooked texture's levels are those that the GPU
/// layer computes for a texture made in code: along an even side, pixel `i` of the next level is
/// the average of pixels `2i` and `2i + 1`; along an odd side of `2k + 1` pixels, it takes pixels
/// `2i`, `2i + 1` and `2i + 2`, weighted `k - i`, `k` and `i + 1` out of `2k + 1`.
fn footprints(size: u32) -> Vec<Vec<(u32, f32)>> {
    let half = size / 2;
    if size == 1 {
        vec![vec![(0, 1.0)]]
    } else if size.is_multiple_of(2) {
        (0..half)
            .map(|i| vec![(2 * i, 0.5), (2 * i + 1, 0.5)])
            .collect()
    } else {
        let weight = |share: u32| share as f32 / size as f32;
        (0..half)
            .map(|i| {
                vec![
                    (2 * i, weight(half - i)),
                    (2 * i + 1, weight(half)),
                    (2 * i + 2, weight(i + 1)),
                ]
            })
            .collect()
    }
}

/// The linear value of each sRGB-encoded byte.
static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|byte| {
        let encoded = byte as f32 / 255.0;
        if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    })
});

/// The sRGB encoding of a linear value between 0 and 1, as a byte.
fn linear_to_srgb(linear: f32) -> u8 {
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    unit_to_byte(encoded)
}

/// A value between 0 and 1 as a byte, rounded.
fn unit_to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// `level` with its last column and row repeated up to whole blocks of 4 × 4 texels.
fn padded(level: &Image) -> Image {
    let (width, height) = (
        level.width.next_multiple_of(4),
        level.height.next_multiple_of(4),
    );
    if (width, height) == (level.width, level.height) {
        return level.clone();
    }
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let start =
                4 * (y.min(level.height - 1) * level.width + x.min(level.width - 1)) as usize;
            rgba.extend_from_slice(&level.rgba[start..start + 4]);
        }
    }
    Image {
        width,
        height,
        rgba,
    }
}

/// The blocks of `level`, whose sides are multiples of 4, in `variant`: bands of block rows encoded
/// on as many threads as the machine offers.
fn encode(variant: CompressionVariant, level: &Image) -> Vec<u8> {
    let block_rows = (level.height / 4) as usize;
    let row_bytes = variant.bytes_per_row(level.width) as usize;
    let texel_row_bytes = level.width as usize * 4;
    let threads = thread::available_parallelism().map_or(1, |count| count.get());
    let rows_per_band = block_rows.div_ceil(threads.min(block_rows));
    let mut blocks = vec![0; variant.blocks_byte_size(level.width, level.height)];
    thread::scope(|scope| {
        for (band, output) in blocks.chunks_mut(rows_per_band * row_bytes).enumerate() {
            let rows = output.len() / row_bytes;
            let start = band * rows_per_band * 4 * texel_row_bytes;
            let texels = &level.rgba[start..start + rows * 4 * texel_row_bytes];
            let width = level.width;
            scope.spawn(move || {
                compress_rgba8(variant, texels, output, width, rows as u32 * 4, width * 4);
            });
        }
    });
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use block_compression::decode::decompress_blocks_as_rgba8;

    /// An image of `width` × `height` texels, each of `texel(x, y)`.
    fn image(width: u32, height: u32, texel: impl Fn(u32, u32) -> [u8; 4]) -> Image {
        let rgba = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| texel(x, y))
            .collect();
        Image {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn every_level_down_to_one_texel_is_cooked_in_the_role_s_encoding() {
        let picture = image(16, 8, |x, y| [(x * 16) as u8, (y * 32) as u8, 100, 255]);
        for (role, encoding) in [
            (TextureRole::Color, TextureEncoding::Bc7Srgb),
            (TextureRole::Data, TextureEncoding::Bc7),
            (TextureRole::Normal, TextureEncoding::Bc5),
        ] {
            let texture = cook_texture(&picture, role);
            assert_eq!(texture.encoding, encoding);
            assert_eq!(texture.levels.len(), 5);
            assert_eq!(TextureData::from_ktx2(&texture.to_ktx2()), Ok(texture));
        }
        // Sides that are not multiples of 4 keep their texels.
        let odd = image(6, 3, |_, _| [1, 2, 3, 4]);
        let texture = cook_texture(&odd, TextureRole::Color);
        assert_eq!(texture.encoding, TextureEncoding::Rgba8Srgb);
        assert_eq!(texture.levels[0], odd.rgba);
        assert_eq!(texture.levels.len(), 3);
        assert_eq!(
            cook_texture(&odd, TextureRole::Normal).encoding,
            TextureEncoding::Rgba8
        );
    }

    #[test]
    fn colors_are_averaged_in_linear_light_and_data_as_stored() {
        // Black and white alternating: a linear average of 0.5, which sRGB encodes as 188.
        let checker = image(2, 2, |x, y| {
            let value = if (x + y) % 2 == 0 { 0 } else { 255 };
            [value, value, value, 255]
        });
        assert_eq!(
            halved(&checker, TextureRole::Color).rgba,
            [188, 188, 188, 255]
        );
        assert_eq!(
            halved(&checker, TextureRole::Data).rgba,
            [128, 128, 128, 255]
        );
    }

    #[test]
    fn normals_are_averaged_as_vectors_of_length_one() {
        // Two normals tilted 45° to the left and to the right average straight out of the surface,
        // not to a shorter vector: (0, 0, 1), stored as (128, 128, 255).
        let tilted = |x: u32, _| {
            let side = if x == 0 { -1.0f32 } else { 1.0 };
            let half = std::f32::consts::FRAC_1_SQRT_2;
            let encode = |value: f32| ((value * 0.5 + 0.5) * 255.0).round() as u8;
            [encode(side * half), 128, encode(half), 255]
        };
        let level = halved(&image(2, 1, tilted), TextureRole::Normal);
        // 128 stands for 1/255, not 0, which made of length one again grows a little.
        let [x, y, z, _] = level.rgba[..] else {
            unreachable!()
        };
        assert!(
            x.abs_diff(128) <= 1 && y.abs_diff(128) <= 1 && z == 255,
            "{:?}",
            level.rgba
        );
        // Opposite normals cancel out: the surface's normal stands.
        let opposite = image(2, 1, |x, _| {
            if x == 0 {
                [0, 128, 128, 255]
            } else {
                [255, 128, 128, 255]
            }
        });
        let level = halved(&opposite, TextureRole::Normal);
        assert!(level.rgba[2] >= 254, "{:?}", level.rgba);
    }

    #[test]
    fn the_blocks_decode_close_to_the_image_and_do_not_depend_on_the_threads() {
        // A smooth gradient, which BC7 keeps within a few steps.
        let picture = image(64, 32, |x, y| [(x * 4) as u8, (y * 8) as u8, 200, 255]);
        let blocks = encode(
            CompressionVariant::BC7(BC7Settings::opaque_fast()),
            &picture,
        );
        let mut decoded = vec![0; picture.rgba.len()];
        decompress_blocks_as_rgba8(
            CompressionVariant::BC7(BC7Settings::opaque_fast()),
            64,
            32,
            &blocks,
            &mut decoded,
        );
        let largest = decoded
            .iter()
            .zip(&picture.rgba)
            .map(|(a, b)| a.abs_diff(*b))
            .max();
        assert!(largest <= Some(4), "{largest:?}");
        // One band encodes the same blocks as many.
        let mut single = vec![0; blocks.len()];
        compress_rgba8(
            CompressionVariant::BC7(BC7Settings::opaque_fast()),
            &picture.rgba,
            &mut single,
            64,
            32,
            64 * 4,
        );
        assert_eq!(single, blocks);
    }

    #[test]
    fn a_level_smaller_than_a_block_repeats_its_edges() {
        let small = image(2, 1, |x, _| [x as u8, 0, 0, 255]);
        let block = padded(&small);
        assert_eq!((block.width, block.height), (4, 4));
        let reds: Vec<u8> = block.rgba.chunks(4).map(|texel| texel[0]).collect();
        assert_eq!(reds, [0, 1, 1, 1].repeat(4));
    }

    /// The time BC7 takes on a texture of 4096 × 4096 texels, every level: run with
    /// `cargo test --release -p slime-cook -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, which takes seconds"]
    fn the_time_a_texture_of_4096_texels_takes() {
        let picture = image(4096, 4096, |x, y| {
            [(x ^ y) as u8, (x * 3) as u8, (y * 5) as u8, 255]
        });
        let start = std::time::Instant::now();
        let texture = cook_texture(&picture, TextureRole::Color);
        println!(
            "4096 x 4096, BC7 with sRGB, {} levels, on {} threads: {:?}",
            texture.levels.len(),
            thread::available_parallelism().map_or(1, |count| count.get()),
            start.elapsed()
        );
    }
}
