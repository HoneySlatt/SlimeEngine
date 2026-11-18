//! The host's own device and its window's surface, which uwui-render draws each frame into
//! (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`).
//!
//! The device is opened with wgpu's hint `MemoryHints::MemoryUsage`, whose allocator reserves
//! small blocks of video memory rather than the large ones of `Performance`, wgpu's default: an
//! application's interface never fills them. It asks for no optional feature: nothing reads the
//! GPU's times of its passes. The adapter is the one that uses the least power, an integrated GPU
//! where there is one, unless `WGPU_POWER_PREF` names another.
//!
//! A lost device is replaced by a new one on the same window ([`Gpu::restore`]), and a lost surface
//! made again ([`Gpu::make_surface_again`]); the host decides when, within its budget
//! (`crate::recovery`). Nothing of the interface lives on the device but what `uwui-render` makes
//! again at its first frame: the atlas of glyphs and the canvas's buffers. The program gives no
//! image yet; once it does, the host makes them again from their source
//! (`docs/adr/0305-uwuis-desktop-host-makes-its-device-and-surface-again-a-programs-images-made-again-from-their-source.md`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use uwui::display::DisplayList;
use uwui::text::Fonts;
use uwui_render::{DrawContext, UiRenderer};

/// Why the device or the surface failed.
#[derive(Debug)]
pub(crate) struct GpuError(String);

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GpuError {}

fn error(what: &str, cause: impl std::fmt::Display) -> GpuError {
    GpuError(format!("{what}: {cause}"))
}

/// How many times in a row the surface is lost and made again without an image between before it
/// is given up: a surface lost at every frame is otherwise made again, with a warning, at every
/// frame (`docs/adr/0025-giving-up-a-lost-surface.md`).
const MAX_LOSSES_IN_A_ROW: u32 = 3;

/// What keeps the window from having an image now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Loss {
    /// The device is lost: the host replaces it ([`Gpu::restore`]).
    Device,
    /// The surface is given up: the host makes it again after a wait
    /// ([`Gpu::make_surface_again`]).
    Surface,
}

/// The device, its queue, the window's surface and the renderer that draws into it.
pub(crate) struct Gpu {
    renderer: UiRenderer,
    config: wgpu::SurfaceConfiguration,
    // Dropped before the device, as wgpu asks.
    surface: wgpu::Surface<'static>,
    queue: wgpu::Queue,
    device: wgpu::Device,
    /// The instance that made the surface and the device, which makes their replacements.
    instance: wgpu::Instance,
    /// The window's raw handles, live while this value is, as [`Gpu::new`]'s caller promised.
    handles: (RawDisplayHandle, RawWindowHandle),
    /// Whether the adapter draws on the processor: a lost device of a GPU is never replaced by one
    /// on the processor, which would draw each frame many times slower.
    software: bool,
    /// Set when the device is lost, by its callback or on purpose.
    lost: Arc<AtomicBool>,
    /// Why the surface was given up, while it is.
    given_up: Option<String>,
    /// The surface's losses since it last gave an image.
    losses_in_a_row: u32,
    /// Whether the surface is configured again once the image held is presented: wgpu refuses to
    /// configure a surface while one of its images is held.
    stale: bool,
}

/// A frame's image, drawn, to be presented once the platform's signal is asked for
/// ([`Gpu::present`]).
pub(crate) struct Drawn(wgpu::SurfaceTexture);

impl Drawn {
    /// Drops the image without presenting it: the window keeps showing the image before.
    pub(crate) fn discard(self) {
        drop(self.0);
    }
}

impl Gpu {
    /// Opens a device that can draw into the window of the raw handles `display` and `window`, and
    /// configures its surface at `size`, in physical pixels.
    ///
    /// # Safety
    ///
    /// `display` and `window` are a live display and a window of it, which outlive the returned
    /// value and every device that replaces it ([`Gpu::restore`]).
    #[expect(unsafe_code, reason = "its caller keeps the raw handles live")]
    pub(crate) unsafe fn new(
        display: RawDisplayHandle,
        window: RawWindowHandle,
        size: [u32; 2],
    ) -> Result<Self, GpuError> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY);
        descriptor.flags = wgpu::InstanceFlags::from_build_config().with_env();
        let instance = wgpu::Instance::new(descriptor);
        #[expect(unsafe_code, reason = "the device draws on the window's raw handles")]
        // SAFETY: the caller keeps the handles live while the value returned lives.
        unsafe {
            Self::open(instance, (display, window), size, None)
        }
    }

    /// Replaces this device, lost, with a new one that draws into the same window at the same size:
    /// the adapter that `new` would choose, the lost device's again after a reset of the driver.
    /// Every object of the lost device goes first, its renderer and its surface with it: Direct3D 12
    /// opens no device on an adapter while a removed one lives in the process.
    ///
    /// # Errors
    ///
    /// When no device opens or the window's surface cannot be made, and when the only adapter left
    /// draws on the processor where the lost device did not.
    pub(crate) fn restore(self) -> Result<Self, GpuError> {
        let Self {
            renderer,
            config,
            surface,
            queue,
            device,
            instance,
            handles,
            software,
            ..
        } = self;
        drop(renderer);
        drop(surface);
        drop(queue);
        drop(device);
        let size = [config.width, config.height];
        #[expect(unsafe_code, reason = "the device draws on the window's raw handles")]
        // SAFETY: `Gpu::new`'s caller keeps the handles live while this value and every device
        // that replaces it live.
        unsafe {
            Self::open(instance, handles, size, Some(software))
        }
    }

    /// Makes the surface of the window of `handles` with `instance`, opens the device that draws
    /// into it and configures it at `size`. A device that replaces a lost one, of which
    /// `replaces_software` says whether it drew on the processor, refuses an adapter on the
    /// processor after one that was not.
    ///
    /// # Safety
    ///
    /// As [`Gpu::new`]: `handles` outlive the returned value.
    #[expect(unsafe_code, reason = "its caller keeps the raw handles live")]
    unsafe fn open(
        instance: wgpu::Instance,
        handles: (RawDisplayHandle, RawWindowHandle),
        size: [u32; 2],
        replaces_software: Option<bool>,
    ) -> Result<Self, GpuError> {
        #[expect(
            unsafe_code,
            reason = "wgpu makes a surface on the window's raw handles"
        )]
        // SAFETY: the caller keeps the handles live while the value returned lives, which drops
        // this surface before its device.
        let window = unsafe { surface_of(&instance, handles) }?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:
                wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower),
            compatible_surface: Some(&window),
            ..Default::default()
        }))
        .map_err(|cause| error("no GPU can draw into the window", cause))?;
        let info = adapter.get_info();
        let software = info.device_type == wgpu::DeviceType::Cpu;
        if replaces_software == Some(false) && software {
            return Err(GpuError(format!(
                "the only adapter left, {}, draws on the processor, where the lost device did not",
                info.name
            )));
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uwui-desktop"),
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|cause| error("the GPU's device cannot be opened", cause))?;
        let lost = Arc::new(AtomicBool::new(false));
        device.set_device_lost_callback({
            let lost = Arc::clone(&lost);
            move |reason, message| {
                lost.store(true, Ordering::Release);
                tracing::error!(?reason, %message, "the GPU's device is lost");
            }
        });
        // wgpu's own handler panics; an error of the GPU is told, and the frame goes on. Once the
        // device is lost, wgpu refuses what uses it, which tells nothing more (ADR-0103).
        device.on_uncaptured_error(Arc::new({
            let lost = Arc::clone(&lost);
            move |error| {
                if lost.load(Ordering::Acquire) {
                    tracing::debug!(%error, "an error of the lost GPU device");
                } else {
                    tracing::error!(%error, "an error of the GPU");
                }
            }
        }));

        let capabilities = window.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .ok_or_else(|| {
                error(
                    "the window offers no sRGB format",
                    format!("{:?}", capabilities.formats),
                )
            })?;
        // The canvas blends through a view of the image without its encoding: Vulkan allows it
        // with `VK_KHR_swapchain_mutable_format`.
        if !adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS)
        {
            return Err(GpuError(
                "the GPU cannot view the window's image without its sRGB encoding".to_owned(),
            ));
        }
        // Presenting never waits under Mailbox, where the surface offers it (ADR-0021).
        let present_mode = if capabilities
            .present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        let alpha_mode = if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            wgpu::CompositeAlphaMode::Auto
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size[0].max(1),
            height: size[1].max(1),
            present_mode,
            color_space: wgpu::SurfaceColorSpace::Auto,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![format.remove_srgb_suffix()],
        };
        window.configure(&device, &config);
        let renderer = UiRenderer::new(&device, &queue, format)
            .map_err(|cause| error("the interface's renderer cannot be made", cause))?;
        tracing::info!(
            adapter = %info.name,
            api = ?info.backend,
            ?format,
            ?present_mode,
            "the window's device"
        );
        Ok(Self {
            renderer,
            config,
            surface: window,
            queue,
            device,
            instance,
            handles,
            software,
            lost,
            given_up: None,
            losses_in_a_row: 0,
            stale: false,
        })
    }

    /// Whether the device is lost, and must be replaced ([`Gpu::restore`]).
    pub(crate) fn device_lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    /// Whether the surface is given up, and must be made again ([`Gpu::make_surface_again`]).
    pub(crate) fn surface_given_up(&self) -> bool {
        self.given_up.is_some()
    }

    /// Loses the device on purpose, as a reset of the driver would: the door of the tests
    /// ([`Window::lose_device`](crate::Window::lose_device)). No image of it may be held.
    pub(crate) fn lose_device(&mut self) {
        self.lost.store(true, Ordering::Release);
        self.device.destroy();
    }

    /// Gives the surface up, for `reason`: it shows no image until it is made again.
    pub(crate) fn give_up_surface(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        tracing::error!(%reason, "the window's surface is given up");
        self.given_up = Some(reason);
    }

    /// Makes the surface given up again, on the same window, to be configured before its next
    /// image; nothing for a surface that is not given up.
    ///
    /// # Errors
    ///
    /// When the graphics API cannot make it: it stays given up.
    pub(crate) fn make_surface_again(&mut self) -> Result<(), GpuError> {
        if self.given_up.is_none() {
            return Ok(());
        }
        self.remake_surface()?;
        tracing::warn!("the window's surface, given up, is made again");
        self.given_up = None;
        self.losses_in_a_row = 0;
        Ok(())
    }

    /// Makes the window's surface again and configures it. The old one goes before the new one is
    /// configured: a window has one swap chain at a time on Vulkan.
    fn remake_surface(&mut self) -> Result<(), GpuError> {
        #[expect(
            unsafe_code,
            reason = "wgpu makes a surface on the window's raw handles"
        )]
        // SAFETY: `Gpu::new`'s caller keeps the handles live while this value lives, which drops
        // its surface first.
        let surface = unsafe { surface_of(&self.instance, self.handles) }?;
        self.surface = surface;
        self.surface.configure(&self.device, &self.config);
        self.stale = false;
        Ok(())
    }

    /// Sizes the window's images at `size`, in physical pixels, from the next frame.
    pub(crate) fn resize(&mut self, size: [u32; 2]) {
        let size = size.map(|side| side.max(1));
        if size != [self.config.width, self.config.height] {
            self.config.width = size[0];
            self.config.height = size[1];
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Takes the window's next image: none when the surface gives none now, covered or late, the
    /// frame then drawing nothing. A surface lost is made again at once, and given up at its
    /// fourth loss in a row without an image between.
    ///
    /// # Errors
    ///
    /// When the device is lost, or the surface given up: the host makes them again.
    pub(crate) fn acquire(&mut self) -> Result<Option<wgpu::SurfaceTexture>, Loss> {
        for _ in 0..2 {
            if self.device_lost() {
                return Err(Loss::Device);
            }
            if self.surface_given_up() {
                return Err(Loss::Surface);
            }
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => {
                    self.losses_in_a_row = 0;
                    return Ok(Some(texture));
                }
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                    // Drawn as it is; the surface is configured again once it is presented.
                    self.losses_in_a_row = 0;
                    self.stale = true;
                    return Ok(Some(texture));
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    self.surface.configure(&self.device, &self.config);
                }
                wgpu::CurrentSurfaceTexture::Lost => self.surface_lost(),
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => break,
            }
        }
        if self.device_lost() {
            return Err(Loss::Device);
        }
        Ok(None)
    }

    /// The surface was lost: it is made again, or given up when it cannot be, or when the losses
    /// follow one another without an image between.
    fn surface_lost(&mut self) {
        self.losses_in_a_row += 1;
        if self.losses_in_a_row > MAX_LOSSES_IN_A_ROW {
            self.give_up_surface(format!(
                "it was lost again {MAX_LOSSES_IN_A_ROW} times after being made again"
            ));
            return;
        }
        match self.remake_surface() {
            Ok(()) => tracing::warn!("the window's surface was lost; it is made again"),
            Err(cause) => self.give_up_surface(format!("it cannot be made again: {cause}")),
        }
    }

    /// Draws `list`, laid out with `fonts`, into `image` over a clear one, and submits it.
    ///
    /// # Errors
    ///
    /// When the GPU's memory cannot hold what the frame draws, or the device is lost: the image is
    /// drawn in part, and the host does not present it.
    pub(crate) fn draw(
        &mut self,
        image: wgpu::SurfaceTexture,
        fonts: &Fonts,
        list: &DisplayList,
    ) -> (Drawn, Result<(), GpuError>) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("uwui-desktop frame"),
            });
        {
            let view = image
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uwui-desktop clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        let mut context = DrawContext {
            device: &self.device,
            queue: &self.queue,
            encoder: &mut encoder,
        };
        let drawn = self
            .renderer
            .draw(&mut context, fonts, list, |_| None, &image.texture)
            .map(|_| ())
            .map_err(|cause| error("the frame cannot be drawn whole", cause));
        self.queue.submit([encoder.finish()]);
        (Drawn(image), drawn)
    }

    /// Presents a frame's image: on Wayland, the driver commits the surface. A surface that gave
    /// a suboptimal image is configured again then, once no image of it is held.
    pub(crate) fn present(&mut self, drawn: Drawn) {
        self.queue.present(drawn.0);
        if std::mem::take(&mut self.stale) {
            self.surface.configure(&self.device, &self.config);
        }
    }
}

/// Makes the surface of the window of `handles` with `instance`.
///
/// # Safety
///
/// `handles` are a live display and a window of it, which outlive the surface returned.
#[expect(unsafe_code, reason = "its caller keeps the raw handles live")]
unsafe fn surface_of(
    instance: &wgpu::Instance,
    (display, window): (RawDisplayHandle, RawWindowHandle),
) -> Result<wgpu::Surface<'static>, GpuError> {
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(display),
        raw_window_handle: window,
    };
    #[expect(
        unsafe_code,
        reason = "wgpu makes a surface on the platform's raw handles"
    )]
    // SAFETY: the caller keeps the handles live while the surface lives.
    unsafe { instance.create_surface_unsafe(target) }
        .map_err(|cause| error("the window's surface cannot be made", cause))
}
