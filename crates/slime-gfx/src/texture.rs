//! Textures that shaders sample.

use std::borrow::Cow;
use std::sync::LazyLock;

use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::render_target::TextureFormat;

/// A texture that shaders sample through a bind group
/// ([`BindingResource::Texture`](crate::BindingResource::Texture)), with a
/// [`Sampler`](crate::Sampler): a two-dimensional image (`texture_2d<f32>` in WGSL), or a cube of
/// six square faces (`texture_cube<f32>`, created by [`Gpu::create_cube_texture`]) that shaders
/// read by direction.
///
/// It has levels of detail (mipmaps), each half the size of the one before: a texture minified on
/// screen is then read from the level that fits, instead of aliasing. [`Gpu::create_texture`]
/// computes every level down to one pixel; [`Gpu::create_texture_with_levels`] and
/// [`Gpu::create_cube_texture`] take the levels the caller gives.
///
/// Clones refer to the same texture, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct Texture {
    pub(crate) view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: TextureFormat,
}

impl Texture {
    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The format of the pixels.
    pub fn format(&self) -> TextureFormat {
        self.format
    }
}

impl Gpu {
    /// Creates a texture of `width` × `height` pixels holding `pixels`: rows from top to bottom,
    /// 4 bytes per pixel in the byte order of `format` (red, green, blue, alpha for the RGBA
    /// formats). Shaders read sRGB formats decoded to linear values, and linear formats as they
    /// are. The levels of detail are computed from `pixels`, each pixel the average of the pixels
    /// it covers in the level before, in linear light for the sRGB formats: two along a side of
    /// even length, three along a side of odd length (one along a side of one pixel), each
    /// weighted by how much of it the pixel covers. `label` names the texture in error messages
    /// and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, and [`GfxError::DeviceLost`]
    /// once the device is lost ([`Gpu::check`]): nothing is created then.
    ///
    /// # Panics
    ///
    /// If `format` is `Rgba16Float`, whose textures take levels the caller gives
    /// ([`Gpu::create_texture_with_levels`]); if `width` or `height` is
    /// zero or larger than the device allows ([`Gpu::max_texture_dimension`], 8192 with the
    /// default limits); or if `pixels` does not hold `width * height * 4` bytes.
    pub fn create_texture(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Result<Texture, GfxError> {
        assert!(
            format != TextureFormat::Rgba16Float,
            "texture `{label}`: Rgba16Float textures take levels given by the caller \
             (Gpu::create_texture_with_levels)"
        );
        let expected = width as usize * height as usize * 4;
        assert!(
            pixels.len() == expected,
            "texture `{label}` is {width}x{height} and needs {expected} bytes of pixels, but {} \
             were given",
            pixels.len()
        );
        // One level per halving of the larger side, down to one pixel.
        let count = u32::BITS - width.max(height).leading_zeros();
        let mut levels = vec![Level {
            width,
            height,
            pixels: Cow::Borrowed(pixels),
        }];
        while levels.len() < count as usize {
            let next = levels
                .last()
                .expect("the full-size level is there")
                .halved(format.is_srgb());
            levels.push(next);
        }
        let levels: Vec<&[u8]> = levels.iter().map(|level| &*level.pixels).collect();
        self.create_texture_from_levels(label, (width, height), 1, format, &levels)
    }

    /// Creates a two-dimensional texture of `width` × `height` pixels whose levels of detail the
    /// caller gives: `levels[0]` is the full-size image, each next one half the size of the one
    /// before, rounded down, and at least one pixel, as wgpu sizes levels; there may be fewer
    /// levels than down to one pixel, but at least one. Each level holds its rows from top to
    /// bottom, in the byte order of `format`: 4 bytes per pixel for the 8-bit formats, 8 for
    /// `Rgba16Float`, whose channels are half-precision floats, little-endian, as
    /// [`f32_to_f16`](crate::f32_to_f16) gives them. Sampled like the textures of
    /// [`Gpu::create_texture`]; a sampler reads no level beyond the last given.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, and [`GfxError::DeviceLost`]
    /// once the device is lost ([`Gpu::check`]): nothing is created then.
    ///
    /// # Panics
    ///
    /// If `width` or `height` is zero or larger than the device allows
    /// ([`Gpu::max_texture_dimension`]), if `levels` is empty or has more levels than down to one
    /// pixel, or if a level does not hold exactly its pixels' bytes.
    pub fn create_texture_with_levels(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
        levels: &[&[u8]],
    ) -> Result<Texture, GfxError> {
        self.create_texture_from_levels(label, (width, height), 1, format, levels)
    }

    /// Creates a cube texture, whose six faces are squares of `size` × `size` pixels, with the
    /// levels of detail the caller gives, as [`Gpu::create_texture_with_levels`] takes them: each
    /// level holds the six faces one after the other, in wgpu's order, +X, −X, +Y, −Y, +Z, −Z, each
    /// face its rows from top to bottom. Shaders read it with a direction (`texture_cube<f32>` in
    /// WGSL, bound at a [`BindingType::CubeTexture`](crate::BindingType::CubeTexture) binding): the
    /// face is the axis along which the direction is longest, and on it the pixel at column `s`
    /// and row `t` (from 0 at the left or top edge to 1 at the right or bottom one) lies in the
    /// direction, for `sc = 2s − 1` and `tc = 2t − 1`:
    ///
    /// | Face | Direction |
    /// |---|---|
    /// | +X | (1, −tc, −sc) |
    /// | −X | (−1, −tc, sc) |
    /// | +Y | (sc, 1, tc) |
    /// | −Y | (sc, −1, −tc) |
    /// | +Z | (sc, −tc, 1) |
    /// | −Z | (−sc, −tc, −1) |
    ///
    /// the convention of Vulkan, Direct3D and Metal, which wgpu follows on every graphics API.
    /// Filtering blends pixels across the edges between faces.
    ///
    /// # Errors
    ///
    /// As [`Gpu::create_texture_with_levels`].
    ///
    /// # Panics
    ///
    /// As [`Gpu::create_texture_with_levels`], for a level holding six faces.
    pub fn create_cube_texture(
        &self,
        label: &str,
        size: u32,
        format: TextureFormat,
        levels: &[&[u8]],
    ) -> Result<Texture, GfxError> {
        self.create_texture_from_levels(label, (size, size), 6, format, levels)
    }

    /// Creates a texture of `layers` images (1, or 6 for a cube) from the levels the caller
    /// gives, each level holding every image.
    fn create_texture_from_levels(
        &self,
        label: &str,
        (width, height): (u32, u32),
        layers: u32,
        format: TextureFormat,
        levels: &[&[u8]],
    ) -> Result<Texture, GfxError> {
        assert!(
            width > 0 && height > 0,
            "texture `{label}` is {width}x{height}: a texture has at least one pixel"
        );
        let full_chain = u32::BITS - width.max(height).leading_zeros();
        assert!(
            !levels.is_empty() && levels.len() <= full_chain as usize,
            "texture `{label}` is {width}x{height} and takes 1 to {full_chain} levels, but {} \
             were given",
            levels.len()
        );
        let texel_size = format.texel_size();
        let level_size = |index: usize| ((width >> index).max(1), (height >> index).max(1));
        for (index, bytes) in levels.iter().enumerate() {
            let (level_width, level_height) = level_size(index);
            let expected = (level_width * level_height * layers * texel_size) as usize;
            assert!(
                bytes.len() == expected,
                "level {index} of texture `{label}` is {level_width}x{level_height}x{layers} and \
                 needs {expected} bytes, but {} were given",
                bytes.len()
            );
        }
        let texture = self.create(label, || {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: layers,
                },
                mip_level_count: levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: format.to_wgpu(),
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        })?;
        for (index, bytes) in levels.iter().enumerate() {
            let (level_width, level_height) = level_size(index);
            // Unlike copies recorded in a command encoder, this write needs no row alignment.
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: index as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level_width * texel_size),
                    rows_per_image: Some(level_height),
                },
                wgpu::Extent3d {
                    width: level_width,
                    height: level_height,
                    depth_or_array_layers: layers,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(if layers == 6 {
                wgpu::TextureViewDimension::Cube
            } else {
                wgpu::TextureViewDimension::D2
            }),
            ..wgpu::TextureViewDescriptor::default()
        });
        // Memory that runs out while the levels are uploaded loses the device.
        self.check()?;
        // The view keeps the texture alive.
        Ok(Texture {
            view,
            width,
            height,
            format,
        })
    }
}

/// One level of detail of a texture being created.
struct Level<'a> {
    width: u32,
    height: u32,
    pixels: Cow<'a, [u8]>,
}

impl Level<'_> {
    /// The next level: half the size, rounded down, as wgpu sizes levels; each pixel the average
    /// of the pixels of this level it covers, each counted by how much of it is covered (see
    /// [`footprints`]), so that every pixel of this level counts. sRGB values are averaged in
    /// linear light.
    fn halved(&self, srgb: bool) -> Level<'static> {
        let columns = footprints(self.width);
        let rows = footprints(self.height);
        let mut pixels = Vec::with_capacity(columns.len() * rows.len() * 4);
        for row in &rows {
            for column in &columns {
                let mut sum = [0.0f32; 4];
                for &(source_y, weight_y) in row {
                    for &(source_x, weight_x) in column {
                        let weight = weight_x * weight_y;
                        let start = ((source_y * self.width + source_x) * 4) as usize;
                        let pixel = &self.pixels[start..start + 4];
                        for (total, &value) in sum.iter_mut().zip(pixel).take(3) {
                            *total += weight
                                * if srgb {
                                    SRGB_TO_LINEAR[value as usize]
                                } else {
                                    f32::from(value) / 255.0
                                };
                        }
                        sum[3] += weight * f32::from(pixel[3]) / 255.0;
                    }
                }
                for &average in &sum[..3] {
                    pixels.push(if srgb {
                        linear_to_srgb(average)
                    } else {
                        unit_to_byte(average)
                    });
                }
                pixels.push(unit_to_byte(sum[3]));
            }
        }
        Level {
            width: columns.len() as u32,
            height: rows.len() as u32,
            pixels: Cow::Owned(pixels),
        }
    }
}

/// Along one side of a level, `size` pixels long: for each pixel of the next level, the pixels of
/// this one it covers, with their weights, which add up to 1. The next level has `size / 2`
/// pixels, and at least one.
///
/// A pixel of the next level spans `size / (size / 2)` pixels of this one, each weighted by the
/// part of the span it fills. Along an even side the span is two pixels: pixel `i` is the average
/// of pixels `2i` and `2i + 1`. Along an odd side of `2k + 1` pixels it is `2 + 1 / k` pixels:
/// pixel `i` takes pixels `2i`, `2i + 1` and `2i + 2`, weighted `k - i`, `k` and `i + 1` out of
/// `2k + 1`, so that the last pixel counts as much as the others. Along a side of one pixel, it
/// is that pixel.
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
pub(crate) static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_averages_four_pixels_in_linear_light() {
        // Black and white alternating: a linear average of 0.5, which sRGB encodes as 188.
        let level = Level {
            width: 2,
            height: 2,
            pixels: Cow::Borrowed(&[
                0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
            ]),
        };
        assert_eq!(&*level.halved(true).pixels, &[188, 188, 188, 255]);
        assert_eq!(&*level.halved(false).pixels, &[128, 128, 128, 255]);
    }

    #[test]
    fn an_odd_side_is_filtered_over_all_its_pixels() {
        // Three pixels wide, one high: the one pixel of the next level covers all three, a third
        // each.
        let level = Level {
            width: 3,
            height: 1,
            pixels: Cow::Borrowed(&[0, 0, 0, 255, 100, 100, 100, 255, 200, 200, 200, 255]),
        };
        let halved = level.halved(false);
        assert_eq!((halved.width, halved.height), (1, 1));
        assert_eq!(&*halved.pixels, &[100, 100, 100, 255]);
    }

    #[test]
    fn the_last_row_and_column_of_an_odd_level_count() {
        // 3 × 3, white but for a black last column and last row: four ninths of white, 113.
        let mut pixels = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                let value = if x < 2 && y < 2 { 255 } else { 0 };
                pixels.extend([value, value, value, 255]);
            }
        }
        let level = Level {
            width: 3,
            height: 3,
            pixels: Cow::Owned(pixels),
        };
        let halved = level.halved(false);
        assert_eq!((halved.width, halved.height), (1, 1));
        assert_eq!(&*halved.pixels, &[113, 113, 113, 255]);
    }

    #[test]
    fn each_pixel_of_an_odd_side_counts_by_how_much_of_it_is_covered() {
        // Five pixels wide: the two pixels of the next level cover two and a half each. The
        // first takes the first two pixels wholly and half of the third; the second, the other
        // half and the last two. Weights 2/5, 2/5, 1/5 and 1/5, 2/5, 2/5.
        let values = [0, 50, 100, 150, 250];
        let pixels: Vec<u8> = values
            .iter()
            .flat_map(|&value| [value, value, value, 255])
            .collect();
        let level = Level {
            width: 5,
            height: 1,
            pixels: Cow::Owned(pixels),
        };
        let halved = level.halved(false);
        assert_eq!((halved.width, halved.height), (2, 1));
        // (0 * 2 + 50 * 2 + 100) / 5 = 40 and (100 + 150 * 2 + 250 * 2) / 5 = 180.
        assert_eq!(&*halved.pixels, &[40, 40, 40, 255, 180, 180, 180, 255]);
    }
}
