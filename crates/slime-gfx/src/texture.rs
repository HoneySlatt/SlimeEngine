//! Textures that shaders sample.

use std::borrow::Cow;
use std::sync::LazyLock;

use crate::gpu::Gpu;
use crate::render_target::TextureFormat;

/// A two-dimensional texture that shaders sample through a bind group
/// ([`BindingResource::Texture`](crate::BindingResource::Texture)), with a [`Sampler`](crate::Sampler).
///
/// It has mipmaps: levels of detail, each half the size of the one before, down to one pixel,
/// computed when the texture is created. A texture minified on screen is then read from the
/// level that fits, instead of aliasing.
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
    /// it covers in the level before, in linear light for the sRGB formats: four, or, where a side
    /// of that level has an odd length, three along that side, weighted by how much of each it
    /// covers. `label` names the texture in error messages and in GPU capture tools.
    ///
    /// # Panics
    ///
    /// If `format` is `Rgba16Float`, a format of render targets only; if `width` or `height` is
    /// zero or larger than the device allows ([`Gpu::max_texture_dimension`], 8192 with the
    /// default limits); or if `pixels` does not hold `width * height * 4` bytes.
    pub fn create_texture(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Texture {
        assert!(
            format != TextureFormat::Rgba16Float,
            "texture `{label}`: Rgba16Float is a format of render targets only"
        );
        let expected = width as usize * height as usize * 4;
        assert!(
            pixels.len() == expected,
            "texture `{label}` is {width}x{height} and needs {expected} bytes of pixels, but {} \
             were given",
            pixels.len()
        );
        // One level per halving of the larger side, down to one pixel.
        let levels = u32::BITS - width.max(height).leading_zeros();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.to_wgpu(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut level = Level {
            width,
            height,
            pixels: Cow::Borrowed(pixels),
        };
        for index in 0..levels {
            // Unlike copies recorded in a command encoder, this write needs no row alignment.
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: index,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &level.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level.width * 4),
                    rows_per_image: Some(level.height),
                },
                wgpu::Extent3d {
                    width: level.width,
                    height: level.height,
                    depth_or_array_layers: 1,
                },
            );
            if index + 1 < levels {
                level = level.halved(format.is_srgb());
            }
        }
        // The view keeps the texture alive.
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Texture {
            view,
            width,
            height,
            format,
        }
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
