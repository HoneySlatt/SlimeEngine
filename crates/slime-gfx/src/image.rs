//! Reading render targets back to the CPU.

use std::sync::mpsc;

use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::render_target::{RenderTarget, TextureFormat};

/// An image in CPU memory: `width` × `height` pixels, rows from top to bottom, 4 bytes per pixel
/// in the order red, green, blue, alpha, whatever the byte order of the texture it was read from,
/// and encoded like that texture (sRGB for the engine's formats).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

impl Gpu {
    /// Copies the content of `target` to the CPU. It waits for the GPU to finish the work
    /// submitted before, so the image shows everything rendered into `target` so far.
    ///
    /// # Errors
    ///
    /// [`GfxError::Readback`] if waiting for the GPU or reading its memory fails, for example
    /// because the device was lost.
    ///
    /// # Panics
    ///
    /// If `target` is the image of a window ([`SurfaceFrame::target`](crate::SurfaceFrame::target)):
    /// only targets created by [`Gpu::create_render_target`] can be read back.
    pub fn read_render_target(&self, target: &RenderTarget) -> Result<Image, GfxError> {
        let readback_error =
            |source: Box<dyn std::error::Error + Send + Sync>| GfxError::Readback {
                label: target.label.clone(),
                source,
            };
        let (width, height) = (target.width(), target.height());
        let row_bytes = width * 4;
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
        let mut pixels = Vec::with_capacity(row_bytes as usize * height as usize);
        for row in mapped.chunks(padded_row_bytes as usize) {
            pixels.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        if target.format() == TextureFormat::Bgra8UnormSrgb {
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        Ok(Image {
            width,
            height,
            pixels,
        })
    }
}
