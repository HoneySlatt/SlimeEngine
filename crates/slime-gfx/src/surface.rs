//! Drawing into a window: its surface, and the image of each frame.

use std::fmt;
use std::sync::Arc;

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
    config: wgpu::SurfaceConfiguration,
    format: TextureFormat,
    /// Whether `config` must be applied before the next image is acquired.
    stale: bool,
}

impl Gpu {
    /// Opens the most capable GPU that can draw into `window`, and creates the window's surface
    /// for images of `width` × `height` physical pixels, shown as `present_mode` says.
    ///
    /// The GPU is chosen as by [`Gpu::new`], among the adapters that can present to the window.
    /// The surface uses the first sRGB format the window offers among the engine's
    /// [`TextureFormat`]s. The format and the present mode in use are logged at the `info` level.
    ///
    /// # Errors
    ///
    /// - [`GfxError::CreateSurface`] if the graphics API cannot draw into this window.
    /// - [`GfxError::NoAdapter`] if no adapter can present to the window, and
    ///   [`GfxError::RequestDevice`] if the adapter refuses to open a device.
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
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| GfxError::CreateSurface(Box::new(error)))?;
        let (gpu, adapter) = Gpu::open(&instance, Some(&surface))?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .find_map(|&format| TextureFormat::from_wgpu(format))
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: format.to_wgpu(),
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: chosen_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
        };
        let surface = Surface {
            surface,
            instance,
            window,
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            config,
            format,
            stale: true,
        };
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
    /// for example while the window is minimized, means there is nothing to draw.
    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.stale = true;
        }
    }

    /// The image to draw the next frame into, or `None` when the window cannot show a new image
    /// now; the frame is then not drawn, and the next call tries again. `label` names the image's
    /// target in error messages and in GPU capture tools, where it also names the render passes
    /// drawn into it; the engine passes `window, frame <index>`.
    ///
    /// It returns `None` while the window has a size of zero, when the next image is not ready
    /// within wgpu's timeout (one second), while the window is occluded (on platforms that report
    /// it), and after the surface was lost, in which case it is created again for the next call.
    /// These cases are logged at the `debug` level, except the loss of the surface (`warn`).
    ///
    /// It can block until an image is free. With [`PresentMode::Fifo`], it does in step with the
    /// display; with [`PresentMode::Mailbox`], it was measured not to (on niri with RADV, and in a
    /// headless Weston with lavapipe).
    pub fn acquire(&mut self, label: &str) -> Option<SurfaceFrame> {
        if self.config.width == 0 || self.config.height == 0 {
            return None;
        }
        if self.stale {
            self.surface.configure(&self.device, &self.config);
            self.stale = false;
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
                // Only reachable when an error scope caught the error; otherwise wgpu's error
                // handler has already panicked.
                tracing::error!("frame not drawn: wgpu rejected the acquisition of the image");
                return None;
            }
        };
        let target = RenderTarget::new(texture.texture.clone(), label, self.format);
        Some(SurfaceFrame {
            texture,
            target,
            queue: self.queue.clone(),
        })
    }

    fn recreate(&mut self) {
        match self.instance.create_surface(Arc::clone(&self.window)) {
            Ok(surface) => {
                tracing::warn!("the window's surface was lost; it was created again");
                self.surface = surface;
                self.stale = true;
            }
            Err(error) => {
                tracing::error!(
                    %error,
                    "the window's surface was lost and cannot be created again"
                );
            }
        }
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
    /// The target to draw into. It is valid until the frame is presented, and cannot be read back
    /// ([`Gpu::read_render_target`] panics on it).
    pub fn target(&self) -> &RenderTarget {
        &self.target
    }

    /// Hands the image to the window, to be shown at one of the display's next refreshes, as the
    /// surface's [`PresentMode`] says. Call it after the work that draws into the target has been
    /// submitted, as [`Gpu::render`] does. With [`PresentMode::Fifo`], it can wait for the window
    /// (one second per frame for a hidden window on niri).
    pub fn present(self) {
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
