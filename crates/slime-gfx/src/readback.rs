//! Reading a render target back to the CPU without waiting for the GPU
//! (`docs/adr/0130-picking.md`).

use std::sync::mpsc;

use crate::error::GfxError;
use crate::gpu::{Gpu, submit};
use crate::render_target::RenderTarget;

/// The texels of a render target on their way to the CPU ([`Gpu::read_render_target_later`]): the
/// GPU copies them after the work submitted before, and wgpu maps them for the CPU once it sees the
/// copy done, which it checks whenever later work is submitted, the next frame's for example.
/// Asking for them never waits.
///
/// Dropping it before they come back gives up the copy.
#[derive(Debug)]
pub struct Readback {
    label: String,
    buffer: wgpu::Buffer,
    /// What wgpu said of the mapping, once it said it.
    mapped: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    /// Whether the mapping succeeded, once it is known.
    state: std::cell::Cell<Option<bool>>,
    /// The bytes of a row of texels, and of a row as the copy pads it, and the rows.
    row_bytes: u32,
    padded_row_bytes: u32,
    rows: u32,
}

impl Gpu {
    /// Starts copying the texels of `target` to the CPU, after the work submitted before, and
    /// returns at once: [`Readback::try_read`] gives them once they have come back, a frame or
    /// more later. `target` may be dropped meanwhile.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]); [`GfxError::OutOfMemory`]
    /// if the GPU's memory cannot hold the buffer the texels are copied into;
    /// [`GfxError::NotReadable`] for the image of a window whose platform does not let it be
    /// copied, as [`Gpu::read_render_target`] says.
    ///
    /// # Panics
    ///
    /// If `target` is multisampled, or if its texels, each row padded to 256 bytes, take more than
    /// the largest buffer the device allows (256 MiB at least: an `Rg32Uint` target of 4096 × 4096
    /// texels takes 128 MiB).
    pub fn read_render_target_later(&self, target: &RenderTarget) -> Result<Readback, GfxError> {
        assert!(
            target.texture.sample_count() == 1,
            "render target `{}` is multisampled: read the target that a pass resolves it into",
            target.label
        );
        self.check()?;
        if !target.readable() {
            return Err(GfxError::NotReadable {
                label: target.label.clone(),
            });
        }
        let (width, height) = (target.width(), target.height());
        let row_bytes = width * target.format().texel_size();
        // Copies from a texture to a buffer need rows aligned to 256 bytes.
        let padded_row_bytes = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let size = u64::from(padded_row_bytes) * u64::from(height);
        assert!(
            size <= self.max_buffer_size(),
            "render target `{}` takes {size} bytes to read back, more than the device's largest \
             buffer",
            target.label
        );
        let label = format!("{} readback", target.label);
        let buffer = self.create(&label, || {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&label),
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        })?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(&label),
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
        submit(self, [encoder.finish()]);
        let (sender, mapped) = mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            // The readback may be gone by then: nobody wants the texels.
            let _ = sender.send(result);
        });
        Ok(Readback {
            label: target.label.clone(),
            buffer,
            mapped,
            state: std::cell::Cell::new(None),
            row_bytes,
            padded_row_bytes,
            rows: height,
        })
    }
}

impl Readback {
    /// The texels, rows from top to bottom, without the padding that GPU copies add to rows, in
    /// the target's format, once they have come back; `None` until then. It never waits, nor polls
    /// the device: wgpu sees the copy done when later work is submitted. Once they have come back,
    /// each call gives them again.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost, whether the texels came back or not:
    /// nothing read from a lost device is trusted; [`GfxError::Readback`] if wgpu could not map
    /// them otherwise.
    pub fn try_read(&self, gpu: &Gpu) -> Option<Result<Vec<u8>, GfxError>> {
        if let Err(error) = gpu.check() {
            return Some(Err(error));
        }
        if self.state.get().is_none()
            && let Ok(result) = self.mapped.try_recv()
        {
            if let Err(error) = result {
                self.state.set(Some(false));
                return Some(Err(GfxError::Readback {
                    label: self.label.clone(),
                    source: Box::new(error),
                }));
            }
            self.state.set(Some(true));
        }
        match self.state.get()? {
            false => Some(Err(GfxError::Readback {
                label: self.label.clone(),
                source: "the copy could not be mapped".into(),
            })),
            true => Some(
                self.buffer
                    .get_mapped_range(..)
                    .map_err(|error| GfxError::Readback {
                        label: self.label.clone(),
                        source: Box::new(error),
                    })
                    .map(|mapped| {
                        mapped
                            .chunks(self.padded_row_bytes as usize)
                            .take(self.rows as usize)
                            .flat_map(|row| &row[..self.row_bytes as usize])
                            .copied()
                            .collect()
                    }),
            ),
        }
    }
}
