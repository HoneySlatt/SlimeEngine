//! The guarded door through which the engine lends its wgpu device to a library that draws with
//! wgpu itself, uwui's renderer (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`),
//! under the engine's policy: a lost device and memory that runs out are errors that the program
//! recovers from, and any other error of wgpu is a bug, which panics.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::GfxError;
use crate::gpu::{Gpu, GpuId, submit, within_error_scopes};
use crate::render_target::{RenderTarget, TextureFormat};
use crate::texture::Texture;

/// What [`Gpu::with_wgpu`] lends its closure: the wgpu device behind the engine's, its queue, the
/// adapter's downlevel capabilities, an encoder that the door submits once the closure returns,
/// and the wgpu objects behind the engine's targets and textures ([`EngineObjects`]).
///
/// What the closure creates belongs to the device, and must be dropped before
/// [`Gpu::restore`] or [`Surface::restore`](crate::Surface::restore) replaces a lost one, as the
/// engine's own objects must. What the closure must not do with what is lent:
/// [`Gpu::with_wgpu`].
#[derive(Debug)]
pub struct WgpuLoan<'a> {
    /// The device.
    pub device: &'a wgpu::Device,
    /// Its queue. What it writes reaches the GPU before the encoder's commands.
    pub queue: &'a wgpu::Queue,
    /// What the adapter can do below WebGPU's full feature set, as a library that also runs on
    /// WebGL2 reads it.
    pub downlevel: &'a wgpu::DownlevelCapabilities,
    /// The encoder that the closure records into, submitted by the door after it returns.
    pub encoder: &'a mut wgpu::CommandEncoder,
    /// The wgpu objects behind the engine's, each checked to be of this device.
    pub objects: EngineObjects<'a>,
}

impl WgpuLoan<'_> {
    /// The wgpu format of the engine's `format`.
    pub fn texture_format(format: TextureFormat) -> wgpu::TextureFormat {
        format.to_wgpu()
    }
}

/// The wgpu objects behind the engine's targets and textures, lent by [`Gpu::with_wgpu`], each
/// checked to be of the device that the door lends: wgpu, each of whose instances numbers its
/// objects from zero, would not tell another instance's object from its own
/// (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`).
#[derive(Debug)]
pub struct EngineObjects<'a> {
    /// The device's identity, which every object lent is checked against.
    gpu: GpuId,
    /// The flags of the targets lent to be drawn into, set once the encoder is submitted.
    drawn: &'a mut Vec<Arc<AtomicBool>>,
}

impl EngineObjects<'_> {
    /// The texture behind `target`, to be drawn into: the target counts as drawn once the door has
    /// submitted the encoder, as a pass of [`Gpu::render`] makes it count, so that a window's image
    /// is presented without an empty submission of its own.
    ///
    /// An sRGB target can be viewed without its encoding, its format without sRGB
    /// ([`TextureFormat::without_srgb`]), as uwui's canvas draws into it.
    ///
    /// # Panics
    ///
    /// If `target` belongs to another `Gpu` than this one and its clones.
    #[track_caller]
    pub fn target<'t>(&mut self, target: &'t RenderTarget) -> &'t wgpu::Texture {
        self.gpu.owns(target.gpu, "a render target");
        self.drawn.push(Arc::clone(&target.drawn));
        &target.texture
    }

    /// The view of `target`, to be sampled: a target that an earlier pass drew into, as a view
    /// that an editor's panel shows. A window's image cannot be sampled: wgpu refuses it, a bug
    /// that the door panics on.
    ///
    /// # Panics
    ///
    /// If `target` belongs to another `Gpu` than this one and its clones.
    #[track_caller]
    pub fn target_view<'t>(&self, target: &'t RenderTarget) -> &'t wgpu::TextureView {
        self.gpu.owns(target.gpu, "a render target");
        &target.view
    }

    /// The view of `texture`, to be sampled.
    ///
    /// # Panics
    ///
    /// If `texture` belongs to another `Gpu` than this one and its clones.
    #[track_caller]
    pub fn texture_view<'t>(&self, texture: &'t Texture) -> &'t wgpu::TextureView {
        self.gpu.owns(texture.gpu, "a texture");
        &texture.view
    }
}

impl Gpu {
    /// Lends the wgpu device behind this one to `lend`, under the engine's policy: the one door
    /// through which wgpu leaves this crate, for `slime-app`'s bridge to uwui's renderer and the
    /// engine's own tests (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`).
    ///
    /// It checks the loss first: on a lost device, `lend` does not run. It runs `lend` on the
    /// calling thread, inside wgpu's error scopes of memory, validation and internal errors, which
    /// belong to the thread; scopes that `lend` pushes itself catch what happens inside them, as a
    /// library that checks its shaders does. It then finishes the encoder, submits it under the
    /// lock that every submission of the engine takes, marks drawn the targets lent to be drawn
    /// into ([`EngineObjects::target`]), and asks whether the device is still valid, as the engine does
    /// after each creation, at the cost of one small buffer. `label` names the encoder in GPU
    /// capture tools, and [`GfxError::OutOfMemory`].
    ///
    /// `lend` must not set a callback of the device's loss nor a handler of its errors, which would
    /// replace the engine's; poll the device, on which wgpu panics once it is lost; submit work on
    /// the queue itself, outside the lock that a window's surface needs; destroy the device; keep
    /// clones of the device, the queue or the objects lent, which no later use checks; nor use them
    /// on other threads, whose errors no scope of the door's catches. The engine's tests alone
    /// break these rules, to lose the device behind the engine's back.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] if the device is lost before `lend` runs, or once the encoder is
    /// submitted, whether wgpu reported it or not yet: a loss comes first. Otherwise
    /// [`GfxError::OutOfMemory`] if memory ran out in what `lend` created, outside its own scopes.
    /// What `lend` returned is dropped then.
    ///
    /// # Panics
    ///
    /// On any other error that wgpu raised in what `lend` did, in its encoder or in its submission,
    /// with wgpu's message: a bug, as everywhere in the engine (`docs/adr/0016-gpu-layer-with-wgpu.md`).
    /// As [`EngineObjects`]'s methods, on an object of another device. If `lend` panics, the panic goes
    /// on, nothing submitted.
    pub fn with_wgpu<T>(
        &self,
        label: &str,
        lend: impl FnOnce(&mut WgpuLoan<'_>) -> T,
    ) -> Result<T, GfxError> {
        self.check()?;
        let memory = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let ((lent, valid), error) = within_error_scopes(&self.device, || {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
            let mut drawn = Vec::new();
            let lent = lend(&mut WgpuLoan {
                device: &self.device,
                queue: &self.queue,
                downlevel: &self.downlevel,
                encoder: &mut encoder,
                objects: EngineObjects {
                    gpu: self.id,
                    drawn: &mut drawn,
                },
            });
            // Finishing the encoder is where wgpu validates what was recorded: inside the scopes.
            submit(self, [encoder.finish()]);
            for flag in drawn {
                flag.store(true, Ordering::Relaxed);
            }
            (lent, self.still_valid())
        });
        let memory = pollster::block_on(memory.pop());
        self.check()?;
        if !valid {
            return Err(self.lost(&format!("`{label}`")));
        }
        if memory.is_some() {
            return Err(GfxError::OutOfMemory {
                label: label.to_owned(),
            });
        }
        if let Some(error) = error {
            panic!("wgpu error in `{label}`: {error}");
        }
        Ok(lent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window's image presented counts on it: one that the door lent to be drawn into is not
    /// given an empty submission of its own ([`crate::SurfaceFrame::present`]).
    #[test]
    fn a_target_lent_to_be_drawn_into_counts_as_drawn_once_submitted_one_sampled_does_not() {
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let target = |label| {
            gpu.create_render_target(label, 4, 4, TextureFormat::Rgba8UnormSrgb)
                .expect("the GPU has memory for it")
        };
        let (drawn, sampled) = (target("drawn"), target("sampled"));
        let is_drawn = |target: &RenderTarget| target.drawn.load(Ordering::Relaxed);
        gpu.with_wgpu("lent", |loan| {
            loan.objects.target(&drawn);
            loan.objects.target_view(&sampled);
            assert!(!is_drawn(&drawn), "lent, not submitted yet");
        })
        .expect("the device is there");
        assert!(is_drawn(&drawn), "submitted");
        assert!(!is_drawn(&sampled), "only sampled");
    }
}
