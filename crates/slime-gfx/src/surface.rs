//! Drawing into a window: its surface, and the image of each frame.

use std::fmt;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::render_target::{RenderTarget, TextureFormat};

/// What a window must provide to be drawn into: the `raw-window-handle` traits, and being usable
/// from any thread.
trait WindowHandles: HasWindowHandle + HasDisplayHandle + Send + Sync {}

impl<W: HasWindowHandle + HasDisplayHandle + Send + Sync> WindowHandles for W {}

/// How a window shows the images presented to it (`docs/adr/0021-frame-pacing.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PresentMode {
    /// Each image presented replaces the one waiting to be shown, which the display shows at its
    /// next refresh: presenting never waits, and images the display had no time to show are
    /// dropped. A window that does not offer it uses [`Fifo`](Self::Fifo) instead, which the log
    /// says. The default.
    #[default]
    Mailbox,
    /// Every image presented is shown, in order, one per refresh of the display: presenting waits
    /// while earlier images are still to be shown. Every window offers it. On compositors that show
    /// a hidden window's images only from time to time (once per second on niri), each frame then
    /// waits that long.
    Fifo,
}

/// How many losses in a row the surface is created again after; the next loss in a row, with no
/// image acquired in between, gives it up: a surface lost again at once every time will not come
/// back.
const MAX_LOSSES_IN_A_ROW: u32 = 3;

/// The usage of a window's images: the GPU draws into them, and nothing else. They can neither be
/// sampled nor read back (ADR-0033); a test of `bind_group.rs` relies on this very value.
pub(crate) const WINDOW_IMAGE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT;

/// The wgpu present mode for `requested`, among those the window offers. Every window offers
/// FIFO.
fn choose_present_mode(requested: PresentMode, offered: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    match requested {
        PresentMode::Mailbox if offered.contains(&wgpu::PresentMode::Mailbox) => {
            wgpu::PresentMode::Mailbox
        }
        PresentMode::Mailbox | PresentMode::Fifo => wgpu::PresentMode::Fifo,
    }
}

/// The link between the GPU and a window: the images the window shows, which the GPU draws into.
///
/// Each frame, [`acquire`](Self::acquire) gives the image to draw into, and
/// [`SurfaceFrame::present`] hands it to the window once drawn. The images are in an sRGB format
/// ([`format`](Self::format)), and are shown as the [`PresentMode`] chosen at creation says.
///
/// The surface keeps the window alive: drop it before the windowing system's connection closes.
pub struct Surface {
    surface: wgpu::Surface<'static>,
    instance: wgpu::Instance,
    /// Kept to create the surface again if it is lost.
    window: Arc<dyn WindowHandles>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Why the device was lost, once it has been: the [`Gpu`]'s record.
    device_loss: Arc<OnceLock<String>>,
    config: wgpu::SurfaceConfiguration,
    format: TextureFormat,
    /// The largest width or height the device accepts for an image (8192 with the default limits).
    max_dimension: u32,
    /// Whether the window is larger than that, so that its images are smaller than it.
    oversize: bool,
    /// Whether `config` must be applied before the next image is acquired.
    stale: bool,
    /// How many times in a row the surface was lost, with no image acquired in between.
    losses_in_a_row: u32,
    /// Why the surface is lost for good, once it is: it could not be created again, or was lost
    /// again each time it was.
    lost: Option<String>,
}

impl Gpu {
    /// Opens the most capable GPU that can draw into `window`, and creates the window's surface
    /// for images of `width` × `height` physical pixels (limited as [`Surface::resize`] says), shown
    /// as `present_mode` says.
    ///
    /// The GPU is chosen as by [`Gpu::new`], among the adapters that can present to the window.
    /// The surface uses the first sRGB format the window offers among the engine's
    /// [`TextureFormat`]s. The format and the present mode in use are logged at the `info` level.
    ///
    /// # Errors
    ///
    /// - [`GfxError::NoAdapter`] if no graphics API is available at all (no Vulkan driver on Linux)
    ///   or no adapter can present to the window, and [`GfxError::RequestDevice`] if the adapter
    ///   refuses to open a device.
    /// - [`GfxError::CreateSurface`] if the graphics API cannot draw into this window.
    /// - [`GfxError::SurfaceFormat`] if the window offers no sRGB format that the engine supports.
    pub fn new_for_window<W>(
        window: W,
        width: u32,
        height: u32,
        present_mode: PresentMode,
    ) -> Result<(Self, Surface), GfxError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let instance = crate::gpu::create_instance();
        let window: Arc<dyn WindowHandles> = Arc::new(window);
        let surface = match instance.create_surface(Arc::clone(&window)) {
            Ok(surface) => surface,
            Err(error) => {
                // Without any usable graphics API (no Vulkan driver, a `WGPU_BACKEND` this build
                // does not have), wgpu fails to create the surface with an empty list of causes;
                // asking for an adapter then gives the message that names what is missing.
                let options = wgpu::RequestAdapterOptions::default();
                pollster::block_on(instance.request_adapter(&options))
                    .map_err(|error| GfxError::NoAdapter(Box::new(error)))?;
                return Err(GfxError::CreateSurface(Box::new(error)));
            }
        };
        let (gpu, adapter) = Gpu::open(&instance, Some(&surface))?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .find_map(|&format| TextureFormat::from_wgpu(format).filter(|format| format.is_srgb()))
            .ok_or_else(|| GfxError::SurfaceFormat {
                offered: format!("{:?}", capabilities.formats),
            })?;
        let chosen_mode = choose_present_mode(present_mode, &capabilities.present_modes);
        tracing::info!(?format, present_mode = ?chosen_mode, width, height, "window surface");
        if present_mode == PresentMode::Mailbox && chosen_mode != wgpu::PresentMode::Mailbox {
            tracing::info!("the window does not offer the Mailbox present mode; Fifo is used");
        }
        tracing::debug!(
            formats = ?capabilities.formats,
            present_modes = ?capabilities.present_modes,
            alpha_modes = ?capabilities.alpha_modes,
            "what the window's surface supports"
        );
        let config = wgpu::SurfaceConfiguration {
            usage: WINDOW_IMAGE_USAGE,
            format: format.to_wgpu(),
            color_space: wgpu::SurfaceColorSpace::Auto,
            // Set by `resize` below, which limits the size to what the device accepts.
            width: 0,
            height: 0,
            present_mode: chosen_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
        };
        let mut surface = Surface {
            surface,
            instance,
            window,
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            device_loss: Arc::clone(&gpu.loss),
            config,
            format,
            max_dimension: gpu.device.limits().max_texture_dimension_2d,
            oversize: false,
            stale: true,
            losses_in_a_row: 0,
            lost: None,
        };
        surface.resize(width, height);
        Ok((gpu, surface))
    }
}

impl Surface {
    /// The format of the window's images: the format of every target that
    /// [`acquire`](Self::acquire) returns, for which pipelines must be created.
    pub fn format(&self) -> TextureFormat {
        self.format
    }

    /// Sets the size of the window's images, in physical pixels; call it whenever the window is
    /// resized. The images change size from the next [`acquire`](Self::acquire). A size of zero,
    /// for example while the window is minimized, means there is nothing to draw. A width or
    /// height above what the device accepts (8192 with the default limits) is limited to it, and
    /// the log says so once: the images are then smaller than the window.
    pub fn resize(&mut self, width: u32, height: u32) {
        let oversize = width > self.max_dimension || height > self.max_dimension;
        if oversize && !self.oversize {
            tracing::warn!(
                width,
                height,
                max = self.max_dimension,
                "the window is larger than the GPU can draw into; its images are limited to the \
                 largest size the device accepts"
            );
        }
        self.oversize = oversize;
        let (width, height) = (
            width.min(self.max_dimension),
            height.min(self.max_dimension),
        );
        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.stale = true;
        }
    }

    /// The image to draw the next frame into, or `None` when the window cannot show a new image
    /// now; the frame is then not drawn, and the next call tries again. `label` names the image's
    /// target in error messages and, in GPU capture tools, the render passes drawn into the image
    /// and their command buffers; the image itself keeps the name that wgpu and the graphics API
    /// give a window's images. The engine passes `window, frame <index>`.
    ///
    /// It returns `None` while the window has a size of zero, when the next image is not ready
    /// within wgpu's timeout (one second) on Vulkan, while the window is occluded (on platforms
    /// that report it), when the surface is still outdated after being configured again, after the
    /// surface was lost, in which case it is created again for the next call, once the surface is
    /// lost for good ([`check`](Self::check)), and once the device is lost. These cases are logged
    /// at the `debug` level, except a size of zero and a surface lost for good (not logged again)
    /// and the loss of the surface (`warn`, or `error` for the loss that gives it up).
    ///
    /// When it returns `None`, it submits an empty piece of work to the GPU, as
    /// [`SurfaceFrame::present`] does for an image that nothing drew into: wgpu keeps each write
    /// to a buffer or a texture ([`Gpu::write_uniform_buffer`], [`Gpu::create_texture`]) until the
    /// next submission, so the writes of frames that get no image, for as long as the window is
    /// minimized for example, would otherwise pile up.
    ///
    /// It can block until an image is free. With [`PresentMode::Fifo`], it does in step with the
    /// display; with [`PresentMode::Mailbox`], it was measured not to (on niri with RADV, and in a
    /// headless Weston with lavapipe). On Direct3D 12, wgpu reports no timeout: it waits up to one
    /// second, then returns the image anyway, and the frame is drawn late rather than skipped,
    /// without a log: the wait shows only in the time this call takes.
    ///
    /// # Panics
    ///
    /// If the [`SurfaceFrame`] of the previous call has neither been presented nor dropped: wgpu
    /// treats acquiring a second image as a misuse (ADR-0016).
    pub fn acquire(&mut self, label: &str) -> Option<SurfaceFrame> {
        let image = self.next_image(label);
        if image.is_none() {
            // wgpu keeps each write to a buffer or a texture, with a staging buffer of its own,
            // until a submission. A frame that draws gives it one, as does presenting an image
            // that nothing drew into; a frame without an image must give it one too.
            self.queue.submit([]);
        }
        image
    }

    /// The image of [`acquire`](Self::acquire), without the submission it makes when there is
    /// none.
    fn next_image(&mut self, label: &str) -> Option<SurfaceFrame> {
        if self.lost.is_some() || self.config.width == 0 || self.config.height == 0 {
            return None;
        }
        if self.stale {
            self.surface.configure(&self.device, &self.config);
            self.stale = false;
        }
        // Configuring fails once the device is lost, and can be what finds the loss, since it waits
        // for the GPU. wgpu then treats the acquisition of an image from a surface never configured
        // (the first one, or one created again) as fatal, and panics.
        if self.device_loss.get().is_some() {
            tracing::debug!("frame not drawn: the GPU device is lost");
            return None;
        }
        let mut current = self.surface.get_current_texture();
        if matches!(current, wgpu::CurrentSurfaceTexture::Outdated) {
            // The window changed since the surface was configured, typically a resize that has
            // not been reported yet: configure again and retry once.
            self.surface.configure(&self.device, &self.config);
            current = self.surface.get_current_texture();
        }
        let texture = match current {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                // Still usable; configure again once it has been presented.
                self.stale = true;
                texture
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                tracing::debug!("frame not drawn: the window's next image was not ready in time");
                return None;
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                tracing::debug!("frame not drawn: the window is occluded");
                return None;
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                tracing::debug!("frame not drawn: the window's surface is outdated");
                return None;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.recreate();
                return None;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                // Reached when the device was lost, whose errors the engine's error handler
                // records instead of panicking (ADR-0022), or when an error scope caught the error.
                tracing::debug!("frame not drawn: wgpu rejected the acquisition of the image");
                return None;
            }
        };
        self.losses_in_a_row = 0;
        let target = RenderTarget::new(texture.texture.clone(), label, self.format);
        Some(SurfaceFrame {
            texture,
            target,
            queue: self.queue.clone(),
        })
    }

    /// Whether the surface can still show images. It cannot once it is lost for good: after a loss,
    /// it could not be created again, or it was lost again each time it was. [`acquire`](Self::acquire)
    /// then returns `None`, and a run built on `slime-app` stops like after a lost device
    /// (`docs/adr/0025-giving-up-a-lost-surface.md`).
    ///
    /// # Errors
    ///
    /// [`GfxError::SurfaceLost`] once the surface is lost for good, with the reason.
    pub fn check(&self) -> Result<(), GfxError> {
        match &self.lost {
            Some(message) => Err(GfxError::SurfaceLost {
                message: message.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Creates the surface again after a loss, or gives it up when that fails or when the losses
    /// follow one another without an image in between: a surface lost at every frame would
    /// otherwise be created again, with a warning, at every frame.
    fn recreate(&mut self) {
        self.losses_in_a_row += 1;
        if self.losses_in_a_row > MAX_LOSSES_IN_A_ROW {
            self.give_up(format!(
                "it was lost again {MAX_LOSSES_IN_A_ROW} times after being created again"
            ));
            return;
        }
        match self.instance.create_surface(Arc::clone(&self.window)) {
            Ok(surface) => {
                tracing::warn!("the window's surface was lost; it was created again");
                self.surface = surface;
                self.stale = true;
            }
            Err(error) => self.give_up(format!("it cannot be created again: {error}")),
        }
    }

    fn give_up(&mut self, message: String) {
        tracing::error!(%message, "the window's surface was lost for good");
        self.lost = Some(message);
    }
}

impl fmt::Debug for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Surface")
            .field("format", &self.format)
            .field("width", &self.config.width)
            .field("height", &self.config.height)
            .finish_non_exhaustive()
    }
}

/// The image of a window for one frame, from [`Surface::acquire`]. Draw into
/// [`target`](Self::target), then [`present`](Self::present) it; dropping it without presenting
/// discards it and the window keeps showing its previous image.
#[derive(Debug)]
pub struct SurfaceFrame {
    texture: wgpu::SurfaceTexture,
    target: RenderTarget,
    queue: wgpu::Queue,
}

impl SurfaceFrame {
    /// The target to draw into. It is valid until the frame is presented, and can neither be read
    /// back ([`Gpu::read_render_target`] panics on it) nor sampled ([`Gpu::create_bind_group`]
    /// panics on it).
    pub fn target(&self) -> &RenderTarget {
        &self.target
    }

    /// Hands the image to the window, to be shown at one of the display's next refreshes, as the
    /// surface's [`PresentMode`] says. Call it after the work that draws into the target has been
    /// submitted, as [`Gpu::render`] does; an image that nothing drew into is shown cleared to
    /// black. With [`PresentMode::Fifo`], it can wait for the window (one second per frame for a
    /// hidden window on niri).
    pub fn present(self) {
        if !self.target.drawn.load(Ordering::Relaxed) {
            // wgpu clears an image that nothing drew into when it is presented, and keeps that
            // work, with everything earlier frames used, until a submission lets it see what the
            // GPU has finished. A frame that draws nothing must give it one, or that work piles up
            // at every frame for as long as the run lasts.
            self.queue.submit([]);
        }
        self.queue.present(self.texture);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_is_used_when_the_window_offers_it() {
        let offered = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Mailbox];
        assert_eq!(
            choose_present_mode(PresentMode::Mailbox, &offered),
            wgpu::PresentMode::Mailbox
        );
        assert_eq!(
            choose_present_mode(PresentMode::Fifo, &offered),
            wgpu::PresentMode::Fifo
        );
    }

    #[test]
    fn fifo_replaces_mailbox_when_the_window_does_not_offer_it() {
        let offered = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate];
        assert_eq!(
            choose_present_mode(PresentMode::Mailbox, &offered),
            wgpu::PresentMode::Fifo
        );
    }
}
