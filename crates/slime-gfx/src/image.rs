//! Reading render targets back to the CPU.

use std::sync::mpsc;

use crate::error::GfxError;
use crate::float16::f16_to_f32;
use crate::gpu::Gpu;
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
    /// submitted before, so the image shows everything rendered into `target` so far.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]); [`GfxError::Readback`] if
    /// waiting for the GPU or reading its memory fails.
    ///
    /// # Panics
    ///
    /// If `target` is `Rgba16Float`, which [`Gpu::read_float_render_target`] reads; if `target` is
    /// the image of a window ([`SurfaceFrame::target`](crate::SurfaceFrame::target)): only targets
    /// created by [`Gpu::create_render_target`] can be read back. Also if the wait for the GPU is
    /// what finds the device lost: wgpu treats that as fatal.
    pub fn read_render_target(&self, target: &RenderTarget) -> Result<Image, GfxError> {
        assert!(
            target.format() != TextureFormat::Rgba16Float,
            "render target `{}` is Rgba16Float: read it with Gpu::read_float_render_target",
            target.label
        );
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
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]); [`GfxError::Readback`] if
    /// waiting for the GPU or reading its memory fails.
    ///
    /// # Panics
    ///
    /// If `target` is not `Rgba16Float` (the image of a window never is). Also if the wait for the
    /// GPU is what finds the device lost: wgpu treats that as fatal.
    pub fn read_float_render_target(&self, target: &RenderTarget) -> Result<FloatImage, GfxError> {
        assert!(
            target.format() == TextureFormat::Rgba16Float,
            "render target `{}` is {:?}: read it with Gpu::read_render_target",
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
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{} readback", target.label)),
            size: u64::from(padded_row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(&format!("{} readback", target.label)),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let (sender, receiver) = mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            // The receiver waits below; it cannot be gone.
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| readback_error(Box::new(error)))?;
        receiver
            .recv()
            .map_err(|error| readback_error(Box::new(error)))?
            .map_err(|error| readback_error(Box::new(error)))?;

        let mapped = buffer
            .get_mapped_range(..)
            .map_err(|error| readback_error(Box::new(error)))?;
        let mut texels = Vec::with_capacity(row_bytes as usize * height as usize);
        for row in mapped.chunks(padded_row_bytes as usize) {
            texels.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(texels)
    }
}
