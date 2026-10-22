//! Reading render targets back to the CPU.

use std::sync::mpsc;

use crate::error::GfxError;
use crate::float16::f16_to_f32;
use crate::gpu::{Gpu, LARGEST_STAGING, submit};
use crate::render_target::{RenderTarget, TextureFormat};

/// An image in CPU memory: `width` × `height` pixels, rows from top to bottom, 4 bytes per pixel
/// in the order red, green, blue, alpha, whatever the byte order of the texture it was read from,
/// and encoded as that texture's format says: sRGB-encoded for the sRGB formats, linear for
/// `Rgba8Unorm`. `Rgba16Float` targets are read as a [`FloatImage`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

/// An image of floating-point values in CPU memory, read from an `Rgba16Float` render target
/// ([`Gpu::read_float_render_target`]): `width` × `height` pixels, rows from top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` values: the red, green, blue and alpha of each pixel, each exactly the
    /// half-precision value the target holds (`f32` represents every one, subnormals, infinities
    /// and NaN included).
    pub pixels: Vec<f32>,
}

impl Gpu {
    /// Copies the content of an 8-bit `target` to the CPU. It waits for the GPU to finish the work
    /// submitted before, as [`Gpu::wait_idle`] waits, so the image shows everything rendered into
    /// `target` so far.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]), before or while it waits
    /// for the GPU; [`GfxError::OutOfMemory`] if the GPU's memory cannot hold the buffer the pixels
    /// are copied into; [`GfxError::Readback`] if reading the GPU's memory fails otherwise;
    /// [`GfxError::NotReadable`] for the image of a window whose platform does not let it be copied
    /// ([`SurfaceFrame::target`](crate::SurfaceFrame::target)).
    ///
    /// # Panics
    ///
    /// If `target` is `Rgba16Float`, which [`Gpu::read_float_render_target`] reads, or is
    /// multisampled.
    pub fn read_render_target(&self, target: &RenderTarget) -> Result<Image, GfxError> {
        assert!(
            target.format() != TextureFormat::Rgba16Float,
            "render target `{}` is Rgba16Float: read it with Gpu::read_float_render_target",
            target.label
        );
        assert!(
            target.texture.sample_count() == 1,
            "render target `{}` is multisampled: read the target that a pass resolves it into",
            target.label
        );
        if !target.readable() {
            return Err(GfxError::NotReadable {
                label: target.label.clone(),
            });
        }
        let mut pixels = self.read_texels(target, 4)?;
        if target.format() == TextureFormat::Bgra8UnormSrgb {
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        Ok(Image {
            width: target.width(),
            height: target.height(),
            pixels,
        })
    }

    /// Copies the content of an `Rgba16Float` `target` to the CPU, as
    /// [`read_render_target`](Self::read_render_target) does for 8-bit targets: it waits for the
    /// GPU to finish the work submitted before. The half-precision values are converted to `f32`
    /// exactly.
    ///
    /// # Errors
    ///
    /// As [`read_render_target`](Self::read_render_target).
    ///
    /// # Panics
    ///
    /// If `target` is not `Rgba16Float` (the image of a window never is), or is multisampled: only
    /// targets created by [`Gpu::create_render_target`] can be read back.
    pub fn read_float_render_target(&self, target: &RenderTarget) -> Result<FloatImage, GfxError> {
        assert!(
            target.format() == TextureFormat::Rgba16Float,
            "render target `{}` is {:?}, not Rgba16Float",
            target.label,
            target.format()
        );
        let bytes = self.read_texels(target, 8)?;
        let pixels = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&half| f16_to_f32(u16::from_le_bytes(half)))
            .collect();
        Ok(FloatImage {
            width: target.width(),
            height: target.height(),
            pixels,
        })
    }

    /// Copies the texels of `target`, `bytes_per_pixel` each, to the CPU, rows from top to
    /// bottom, without the padding that GPU copies add to rows.
    fn read_texels(
        &self,
        target: &RenderTarget,
        bytes_per_pixel: u32,
    ) -> Result<Vec<u8>, GfxError> {
        self.read_texels_in_bands(target, bytes_per_pixel, LARGEST_STAGING)
    }

    /// [`read_texels`](Self::read_texels), copying the rows in bands, each into a buffer of at
    /// most `largest_buffer` bytes, so that any target can be read back: a float target of
    /// 8192 × 8192 pixels needs 512 MiB, twice the largest buffer of the engine's floor.
    fn read_texels_in_bands(
        &self,
        target: &RenderTarget,
        bytes_per_pixel: u32,
        largest_buffer: u64,
    ) -> Result<Vec<u8>, GfxError> {
        self.check()?;
        let readback_error =
            |source: Box<dyn std::error::Error + Send + Sync>| GfxError::Readback {
                label: target.label.clone(),
                source,
            };
        let (width, height) = (target.width(), target.height());
        let row_bytes = width * bytes_per_pixel;
        // Copies from a texture to a buffer need rows aligned to 256 bytes.
        let padded_row_bytes = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        // A row always fits: the widest, 16 384 half-float pixels on the GPUs the engine is checked
        // on, takes 128 KiB.
        let band_rows =
            (largest_buffer / u64::from(padded_row_bytes)).min(u64::from(height)) as u32;
        let buffer_label = format!("{} readback", target.label);
        let buffer = self.create(&buffer_label, || {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&buffer_label),
                size: u64::from(padded_row_bytes) * u64::from(band_rows),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        })?;

        let mut texels = Vec::with_capacity(row_bytes as usize * height as usize);
        for first_row in (0..height).step_by(band_rows as usize) {
            let rows = band_rows.min(height - first_row);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some(&format!("{} readback", target.label)),
                });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: first_row,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_row_bytes),
                        rows_per_image: Some(rows),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height: rows,
                    depth_or_array_layers: 1,
                },
            );
            submit(self, [encoder.finish()]);

            let (sender, receiver) = mpsc::channel();
            buffer.map_async(wgpu::MapMode::Read, .., move |result| {
                // The receiver waits below; it cannot be gone.
                let _ = sender.send(result);
            });
            if let Err(error) = self.wait_for(|| receiver.try_recv().ok())? {
                // The buffer is the readback's own: only the device's loss fails its mapping, which
                // wgpu may not have reported yet.
                return Err(if self.check().is_ok() && self.still_valid() {
                    readback_error(Box::new(error))
                } else {
                    self.lost(&format!("mapping `{buffer_label}`"))
                });
            }

            let mapped = buffer
                .get_mapped_range(..)
                .map_err(|error| readback_error(Box::new(error)))?;
            for row in mapped.chunks(padded_row_bytes as usize).take(rows as usize) {
                texels.extend_from_slice(&row[..row_bytes as usize]);
            }
            drop(mapped);
            buffer.unmap();
        }
        Ok(texels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CullMode, FragmentState, RenderPassDescriptor, RenderPipelineDescriptor};

    #[test]
    fn rows_read_in_bands_come_back_in_order() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let target = gpu
            .create_render_target("bands", 3, 5, TextureFormat::Rgba16Float)
            .expect("the GPU has memory for it");
        let shader = gpu
            .create_shader_module(
                "bands",
                "@vertex
                 fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                     return vec4<f32>(f32((i & 1u) * 4u) - 1.0, f32((i & 2u) * 2u) - 1.0, 0.0, 1.0);
                 }
                 @fragment
                 fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                     return vec4<f32>(p.x, p.y, 0.0, 1.0);
                 }",
            )
            .expect("the shader compiles");
        let pipeline = gpu
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: "bands",
                shader: &shader,
                vertex_entry_point: "vs_main",
                vertex_layout: None,
                fragment: Some(FragmentState {
                    entry_point: "fs_main",
                    target_format: Some(TextureFormat::Rgba16Float),
                    blend: None,
                }),
                bind_group_layouts: &[],
                depth: None,
                cull: CullMode::None,
                multisampled: false,
            })
            .expect("the pipeline is valid");
        let pass = RenderPassDescriptor {
            target: &target,
            clear: None,
            depth: None,
            timer: None,
            resolve: None,
        };
        gpu.render(&pass, |pass| {
            pass.set_pipeline(&pipeline);
            pass.draw(0..3);
        });
        let whole = gpu
            .read_texels(&target, 8)
            .expect("the target is read back");
        // Two padded rows of 256 bytes per band: bands of 2, 2 and 1 rows.
        let banded = gpu
            .read_texels_in_bands(&target, 8, 512)
            .expect("the target is read back");
        assert_eq!(banded, whole);
        let image = gpu
            .read_float_render_target(&target)
            .expect("the target is read back");
        assert_eq!(&image.pixels[..4], [0.5, 0.5, 0.0, 1.0]);
        assert_eq!(
            &image.pixels[image.pixels.len() - 4..],
            [2.5, 4.5, 0.0, 1.0]
        );
    }
}
