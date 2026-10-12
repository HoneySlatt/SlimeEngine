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
    /// are. The levels of detail are computed from `pixels`, each pixel the average of four of the
    /// level before, in linear light for the sRGB formats. `label` names the texture in error
    /// messages and in GPU capture tools.
    ///
    /// # Panics
    ///
    /// If `width` or `height` is zero or larger than the device allows (8192 with the default
    /// limits), or if `pixels` does not hold `width * height * 4` bytes.
    pub fn create_texture(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Texture {
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
    /// The next level: half the size, each pixel the average of the four it replaces (of the
    /// pixel itself and its neighbours, at an odd edge). sRGB values are averaged in linear light.
    fn halved(&self, srgb: bool) -> Level<'static> {
        let (width, height) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let mut sum = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let source_x = (2 * x + dx).min(self.width - 1);
                    let source_y = (2 * y + dy).min(self.height - 1);
                    let start = ((source_y * self.width + source_x) * 4) as usize;
                    let pixel = &self.pixels[start..start + 4];
                    for (total, &value) in sum.iter_mut().zip(pixel).take(3) {
                        *total += if srgb {
                            SRGB_TO_LINEAR[value as usize]
                        } else {
                            f32::from(value) / 255.0
                        };
                    }
                    sum[3] += f32::from(pixel[3]) / 255.0;
                }
                for &total in &sum[..3] {
                    let average = total / 4.0;
                    pixels.push(if srgb {
                        linear_to_srgb(average)
                    } else {
                        unit_to_byte(average)
                    });
                }
                pixels.push(unit_to_byte(sum[3] / 4.0));
            }
        }
        Level {
            width,
            height,
            pixels: Cow::Owned(pixels),
        }
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
    fn an_odd_edge_repeats_its_last_pixel() {
        // Three pixels wide: the second output pixel averages the third pixel with itself.
        let level = Level {
            width: 3,
            height: 1,
            pixels: Cow::Borrowed(&[0, 0, 0, 255, 100, 100, 100, 255, 200, 200, 200, 255]),
        };
        let halved = level.halved(false);
        assert_eq!((halved.width, halved.height), (1, 1));
        assert_eq!(&*halved.pixels, &[50, 50, 50, 255]);
    }
}
