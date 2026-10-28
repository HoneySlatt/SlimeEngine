//! Drawing into a window: its surface, and the image of each frame.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, PoisonError, RwLock};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::error::GfxError;
use crate::gpu::{Gpu, Loss, Lost, within_error_scopes};
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

/// The usage of a window's images: the GPU draws into them, and they cannot be sampled
/// (ADR-0033); a test of `bind_group.rs` relies on this very value. Where the window lets them be
/// copied, they can be read back too ([`READ_BACK_USAGE`]).
pub(crate) const WINDOW_IMAGE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT;
/// What a window's images can do besides, where the window offers it: be copied, which reads them
/// back, for a capture of what a game shows (`docs/adr/0122-the-development-channel.md`).
const READ_BACK_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::COPY_SRC;

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
    /// The device that draws into it, whose identity its images carry.
    gpu: crate::gpu::GpuId,
    surface: wgpu::Surface<'static>,
    instance: wgpu::Instance,
    /// Kept to create the surface again if it is lost.
    window: Arc<dyn WindowHandles>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Why the device was lost, once it has been: the [`Gpu`]'s record.
    device_loss: Arc<OnceLock<Loss>>,
    /// The [`Gpu`]'s lock on its submissions, which a configuration takes alone.
    submissions: Arc<RwLock<()>>,
    config: wgpu::SurfaceConfiguration,
    /// The present mode asked for, which a surface created again for a new device asks for too.
    present_mode: PresentMode,
    format: TextureFormat,
    /// The largest width or height the device accepts for an image (8192 at least: the engine's
    /// floor).
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
    /// Whether the [`SurfaceFrame`] of the last image acquired still exists.
    frame_outstanding: Arc<AtomicBool>,
    /// The texture of the last image acquired, which [`Surface::release_image`] destroys before
    /// the window's swap chain is configured again or goes.
    image: Option<wgpu::Texture>,
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
    ///   or no adapter can present to the window; [`GfxError::BelowFloor`] if the adapter offers
    ///   less than the engine's floor of one of its limits ([`Gpu::new`]); and
    ///   [`GfxError::RequestDevice`] if the adapter refuses to open a device.
    /// - [`GfxError::CreateSurface`] if the graphics API cannot draw into this window.
    /// - [`GfxError::SurfaceFormat`] if the window offers no sRGB format that the engine supports.
    /// - [`GfxError::SurfaceWithoutEncoding`] if the window's images cannot be seen without their
    ///   sRGB encoding ([`RenderTarget::without_srgb`](crate::RenderTarget::without_srgb)).
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
        Surface::open(instance, window, (width, height), present_mode, None)
    }
}

impl Surface {
    /// Opens a device to replace `lost`, which drew into this surface's window, and creates the
    /// window's surface again for it, of the same size and present mode: the most capable adapter
    /// of the same wgpu instance that can draw into the window, the lost device's adapter again
    /// after a driver reset. It takes the lost `Gpu` and the lost device's surface, and releases
    /// both before the new device opens: a window has one swap chain at a time on Vulkan and on
    /// Direct3D 12, and Direct3D 12 opens no device on an adapter while a removed one still exists
    /// in the process. Every other clone of `lost`, and every object it created, must be dropped
    /// first, as [`Gpu::restore`] says, then created again with the new device, for targets of the
    /// new surface's [`format`](Self::format), which another adapter may change; using one of the
    /// lost device's with it panics. The size is the surface's last, limited to what the lost device
    /// accepted until the window is next resized. Applications built on `slime-app` choose with
    /// `AppConfig` whether the run does it for them (ADR-0075).
    ///
    /// # Errors
    ///
    /// [`GfxError::CannotRestore`] as [`Gpu::restore`] says, and as [`Gpu::new_for_window`] when
    /// no device or surface opens. The window then has no surface left.
    ///
    /// # Panics
    ///
    /// If `lost` is not lost ([`Gpu::check`]) or is not this surface's device, or a
    /// [`SurfaceFrame`] of this surface still exists.
    pub fn restore(self, lost: Gpu) -> Result<(Gpu, Surface), GfxError> {
        assert!(
            !self.frame_outstanding.load(Ordering::Relaxed),
            "the window's image is still held: present each SurfaceFrame before restoring the \
             device"
        );
        assert!(
            Arc::ptr_eq(&self.device_loss, &lost.loss),
            "the lost device given to Surface::restore is not the one that drew into the window"
        );
        let lost = Lost::of(lost)?;
        let (instance, window, size, present_mode) = (
            self.instance.clone(),
            Arc::clone(&self.window),
            (self.config.width, self.config.height),
            self.present_mode,
        );
        // Every reference to the lost device goes before the new one opens.
        drop(self);
        Surface::open(instance, window, size, present_mode, Some(&lost))
    }

    /// Creates the surface of `window` with `instance`, and opens the most capable GPU that can
    /// draw into it, as [`Gpu::new_for_window`] says, to replace `lost` if it is given.
    fn open(
        instance: wgpu::Instance,
        window: Arc<dyn WindowHandles>,
        (width, height): (u32, u32),
        present_mode: PresentMode,
        lost: Option<&Lost>,
    ) -> Result<(Gpu, Surface), GfxError> {
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
        let (gpu, adapter) = Gpu::open(&instance, Some(&surface), lost)?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .find_map(|&format| TextureFormat::from_wgpu(format).filter(|format| format.is_srgb()))
            .ok_or_else(|| GfxError::SurfaceFormat {
                offered: format!("{:?}", capabilities.formats),
            })?;
        // The canvas blends through the view of a window's image without its encoding: Direct3D 12
        // always allows it, Vulkan with `VK_KHR_swapchain_mutable_format`.
        if !adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS)
        {
            return Err(GfxError::SurfaceWithoutEncoding);
        }
        let chosen_mode = choose_present_mode(present_mode, &capabilities.present_modes);
        tracing::info!(?format, present_mode = ?chosen_mode, width, height, "window surface");
        if present_mode == PresentMode::Mailbox && chosen_mode != wgpu::PresentMode::Mailbox {
            tracing::info!("the window does not offer the Mailbox present mode; Fifo is used");
        }
        tracing::debug!(
            formats = ?capabilities.formats,
            present_modes = ?capabilities.present_modes,
            alpha_modes = ?capabilities.alpha_modes,
            usages = ?capabilities.usages,
            "what the window's surface supports"
        );
        let usage = WINDOW_IMAGE_USAGE | (capabilities.usages & READ_BACK_USAGE);
        let config = wgpu::SurfaceConfiguration {
            usage,
            format: format.to_wgpu(),
            color_space: wgpu::SurfaceColorSpace::Auto,
            // Set by `resize` below, which limits the size to what the device accepts.
            width: 0,
            height: 0,
            present_mode: chosen_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: format
                .without_srgb()
                .map(TextureFormat::to_wgpu)
                .into_iter()
                .collect(),
        };
        let mut surface = Surface {
            gpu: gpu.id,
            surface,
            instance,
            window,
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            device_loss: Arc::clone(&gpu.loss),
            submissions: Arc::clone(&gpu.submissions),
            config,
            present_mode,
            format,
            max_dimension: gpu.max_texture_dimension(),
            oversize: false,
            stale: true,
            losses_in_a_row: 0,
            lost: None,
            frame_outstanding: Arc::new(AtomicBool::new(false)),
            image: None,
        };
        surface.resize(width, height);
        Ok((gpu, surface))
    }

    /// The format of the window's images: the format of every target that
    /// [`acquire`](Self::acquire) returns, for which pipelines must be created.
    pub fn format(&self) -> TextureFormat {
        self.format
    }

    /// Sets the size of the window's images, in physical pixels; call it whenever the window is
    /// resized. The images change size from the next [`acquire`](Self::acquire). A size of zero,
    /// for example while the window is minimized, means there is nothing to draw. A width or height
    /// above what the device accepts (8192 at least: the engine's floor) is limited to it, and the
    /// log says so once: the images are then smaller than the window.
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
    /// within wgpu's timeout (one second) on Vulkan (on Windows, see below), while the window is
    /// occluded (on platforms that report it), when the surface is still outdated after being
    /// configured again, after the surface was lost or could not be configured (the graphics API
    /// refused the window, which it does when the window or the display goes away), in which case
    /// it is created again for the next call (`docs/adr/0025-giving-up-a-lost-surface.md`), once
    /// the surface is lost for good ([`check`](Self::check)), and once the device is lost. These
    /// cases are logged at the `debug` level, except a size of zero and a surface lost for good
    /// (not logged again) and the loss of the surface (`warn`, or `error` for the loss that gives
    /// it up).
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
    /// without a log: the wait shows only in the time this call takes. On Windows with Vulkan
    /// (`WGPU_BACKEND=vulkan`), wgpu also waits, once the driver has handed out an image, until
    /// that image is free, and treats a wait longer than one second as the loss of the device
    /// rather than as a timeout: this call returns `None` and the device is lost
    /// ([`Gpu::check`]), with a reason that blames the driver, so a run built on `slime-app`
    /// stops instead of skipping the frame. A GPU frame that takes more than about one second, or
    /// a compositor that holds the window's images that long, would cause it; neither was
    /// measured.
    ///
    /// # Panics
    ///
    /// If the [`SurfaceFrame`] of the previous call still exists: acquiring a second image, or
    /// configuring the surface, while one is held is a misuse (ADR-0016). Present each frame before
    /// the next call; dropping it instead is a misuse too ([`SurfaceFrame`] says why).
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
        assert!(
            !self.frame_outstanding.load(Ordering::Relaxed),
            "the window's previous image ({label} asked for the next one) is still held: present \
             each SurfaceFrame before acquiring the next image"
        );
        if self.lost.is_some() || self.config.width == 0 || self.config.height == 0 {
            return None;
        }
        if self.stale && !self.configure() {
            return None;
        }
        // Acquiring an image once the device is lost can be fatal in wgpu, as for configuring.
        if self.device_loss.get().is_some() {
            tracing::debug!("frame not drawn: the GPU device is lost");
            return None;
        }
        let mut current = self.surface.get_current_texture();
        if matches!(current, wgpu::CurrentSurfaceTexture::Outdated) {
            // The window changed since the surface was configured, typically a resize that has
            // not been reported yet: configure again and retry once.
            self.stale = true;
            if !self.configure() {
                return None;
            }
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
                self.recreate(None);
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
        let target = RenderTarget::new(self.gpu, texture.texture.clone(), label, self.format);
        self.frame_outstanding.store(true, Ordering::Relaxed);
        self.image = Some(texture.texture.clone());
        Some(SurfaceFrame {
            texture,
            target,
            queue: self.queue.clone(),
            _outstanding: Outstanding(Arc::clone(&self.frame_outstanding)),
        })
    }

    /// Lets the window's swap chain go, or be configured again, with the last image acquired, which
    /// may still hold its synchronization: wgpu keeps an image acquired when the device refused to
    /// present it, lost before the engine knew, and work still in flight holds an image that a
    /// panic left unpresented. On Vulkan, wgpu panics when it destroys a swap chain whose
    /// synchronization an image holds, and during a panic, that aborts the process. Destroying the
    /// image's texture releases it, and does nothing to an image already presented; dropping the
    /// texture lets Direct3D 12 resize the swap chain's images, which it refuses while one is
    /// referred to.
    fn release_image(&mut self) {
        if let Some(image) = self.image.take() {
            image.destroy();
        }
    }

    /// Applies `config` to the surface, and whether an image can be acquired from it next. Not
    /// when the device is lost: configuring then fails, and can be what finds the loss, since it
    /// waits for the GPU, and wgpu treats acquiring an image from a surface never configured (the
    /// first one, or one created again) as fatal. Not either when the graphics API refuses the
    /// configuration, which the window's environment causes rather than the engine (the window or
    /// the display went away, a swap chain that cannot be created): the surface then counts as
    /// lost, and is created again, or given up (`docs/adr/0025-giving-up-a-lost-surface.md`).
    fn configure(&mut self) -> bool {
        // On Direct3D 12, the swap chain's images cannot be resized while a texture of one exists:
        // the last image goes first.
        self.release_image();
        // wgpu waits for the GPU to finish its work, then fails if work came meanwhile: no thread
        // submits until the surface is configured. The thread that owns the surface makes its own
        // submissions, and those of the frames, outside of this call.
        let alone = self
            .submissions
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let ((), error) = within_error_scopes(&self.device, || {
            self.surface.configure(&self.device, &self.config);
        });
        drop(alone);
        if self.device_loss.get().is_some() {
            tracing::debug!("frame not drawn: the GPU device is lost");
            return false;
        }
        if let Some(error) = error {
            tracing::warn!(%error, "the window's surface cannot be configured");
            self.recreate(Some(error.to_string()));
            return false;
        }
        self.stale = false;
        true
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
    /// otherwise be created again, with a warning, at every frame. `refusal` is wgpu's message
    /// when the loss is a configuration that the graphics API refused.
    fn recreate(&mut self, refusal: Option<String>) {
        self.losses_in_a_row += 1;
        if self.losses_in_a_row > MAX_LOSSES_IN_A_ROW {
            let mut message =
                format!("it was lost again {MAX_LOSSES_IN_A_ROW} times after being created again");
            if let Some(refusal) = refusal {
                message.push_str(&format!(
                    ", the last time because it cannot be configured: {refusal}"
                ));
            }
            self.give_up(message);
            return;
        }
        match self.instance.create_surface(Arc::clone(&self.window)) {
            Ok(surface) => {
                tracing::warn!("the window's surface was lost; it was created again");
                self.release_image();
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

impl Drop for Surface {
    fn drop(&mut self) {
        self.release_image();
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
/// [`target`](Self::target), then [`present`](Self::present) it.
///
/// Every frame acquired must be presented, even one that nothing drew into (it is shown cleared
/// to black); to keep the window's current image, do not acquire a new one. Dropping a frame
/// without presenting it is a misuse that nothing reports, but for a frame that a panic drops: on
/// Vulkan, wgpu cannot give such an image back to the window (its discard does nothing), so each
/// drop takes one of the window's images until the surface is configured again, and leaves wgpu's
/// synchronization of that image in a state that Vulkan forbids. Once the window has no image
/// left, [`acquire`](Surface::acquire) waits one second at every call and returns `None`. The
/// [`Surface`] releases the last image when it is configured again, created again, dropped or
/// restored, so that a panic that drops a frame ends as a panic does, not as an abort.
#[derive(Debug)]
pub struct SurfaceFrame {
    texture: wgpu::SurfaceTexture,
    target: RenderTarget,
    queue: wgpu::Queue,
    /// Tells the surface that the frame is gone, when it is presented or dropped.
    _outstanding: Outstanding,
}

/// The surface's flag that a [`SurfaceFrame`] exists, cleared when the frame is presented or
/// dropped: [`Surface::acquire`] panics while it is set, rather than let wgpu refuse to configure
/// the surface, which it would then take for a surface the environment broke.
#[derive(Debug)]
struct Outstanding(Arc<AtomicBool>);

impl Drop for Outstanding {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

impl SurfaceFrame {
    /// The target to draw into. It is valid until the frame is presented, and cannot be sampled
    /// ([`Gpu::create_bind_group`] panics on it). It can be read back before it is presented
    /// ([`Gpu::read_render_target`]) where the window lets the GPU copy its images, as Vulkan and
    /// Direct3D 12 usually do: a capture of what a game shows reads it
    /// (`docs/adr/0122-the-development-channel.md`).
    pub fn target(&self) -> &RenderTarget {
        &self.target
    }

    /// Hands the image to the window, to be shown at one of the display's next refreshes, as the
    /// surface's [`PresentMode`] says. Call it after the work that draws into the target has been
    /// submitted, as [`Gpu::render`] does; an image that nothing drew into is shown cleared to
    /// black. With [`PresentMode::Fifo`], it can wait for the window (one second per frame for a
    /// hidden window on niri). Once the GPU device is lost, nothing is shown: wgpu 30 refuses to
    /// present the image, silently, and keeps it acquired until the [`Surface`] releases it, when
    /// it is configured again, dropped or restored at the latest.
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
