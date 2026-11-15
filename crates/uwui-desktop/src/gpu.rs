//! The host's own device and its window's surface, which uwui-render draws each frame into
//! (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`).
//!
//! The device is opened with wgpu's hint `MemoryHints::MemoryUsage`, whose allocator reserves
//! small blocks of video memory rather than the large ones of `Performance`, wgpu's default: an
//! application's interface never fills them. It asks for no optional feature: nothing reads the
//! GPU's times of its passes. The adapter is the one that uses the least power, an integrated GPU
//! where there is one, unless `WGPU_POWER_PREF` names another.

use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
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

/// The device, its queue, the window's surface and the renderer that draws into it.
pub(crate) struct Gpu {
    renderer: UiRenderer,
    config: wgpu::SurfaceConfiguration,
    // Dropped before the device, as wgpu asks.
    surface: wgpu::Surface<'static>,
    queue: wgpu::Queue,
    device: wgpu::Device,
    /// Set when the device is lost, by its callback.
    lost: Arc<AtomicBool>,
    /// Whether the surface is configured again once the image held is presented: wgpu refuses to
    /// configure a surface while one of its images is held.
    stale: bool,
}

/// A frame's image, drawn, to be presented once the platform's signal is asked for
/// ([`Gpu::present`]).
pub(crate) struct Drawn(wgpu::SurfaceTexture);

impl Gpu {
    /// Opens a device that can draw into the Wayland surface `surface` of the display `display`,
    /// and configures its surface at `size`, in physical pixels.
    ///
    /// # Safety
    ///
    /// `display` and `surface` are a live `wl_display` and `wl_surface` of it, which outlive the
    /// returned value.
    #[expect(unsafe_code, reason = "its caller keeps the raw handles live")]
    pub(crate) unsafe fn on_wayland(
        display: NonNull<std::ffi::c_void>,
        surface: NonNull<std::ffi::c_void>,
        size: [u32; 2],
    ) -> Result<Self, GpuError> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY);
        descriptor.flags = wgpu::InstanceFlags::from_build_config().with_env();
        let instance = wgpu::Instance::new(descriptor);
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                display,
            ))),
            raw_window_handle: RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)),
        };
        #[expect(
            unsafe_code,
            reason = "wgpu makes a surface on the platform's raw handles"
        )]
        // SAFETY: the caller keeps the display and the surface live until the value returned is
        // dropped, which drops this surface first.
        let window = unsafe { instance.create_surface_unsafe(target) }
            .map_err(|cause| error("the window's surface cannot be made", cause))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:
                wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower),
            compatible_surface: Some(&window),
            ..Default::default()
        }))
        .map_err(|cause| error("no GPU can draw into the window", cause))?;
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
        // wgpu's own handler panics; an error of the GPU is told, and the frame goes on.
        device.on_uncaptured_error(Arc::new(|error| {
            tracing::error!(%error, "an error of the GPU");
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
        let info = adapter.get_info();
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
            lost,
            stale: false,
        })
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
    /// frame then drawing nothing.
    ///
    /// # Errors
    ///
    /// When the device is lost, or the surface lost and not made again.
    pub(crate) fn acquire(&mut self) -> Result<Option<wgpu::SurfaceTexture>, GpuError> {
        if self.lost.load(Ordering::Acquire) {
            return Err(GpuError("the GPU's device is lost".to_owned()));
        }
        for _ in 0..2 {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => return Ok(Some(texture)),
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                    // Drawn as it is; the surface is configured again once it is presented.
                    self.stale = true;
                    return Ok(Some(texture));
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.surface.configure(&self.device, &self.config);
                }
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => return Ok(None),
            }
        }
        Err(GpuError(
            "the window's surface is lost and cannot be made again".to_owned(),
        ))
    }

    /// Draws `list`, laid out with `fonts`, into `image` over a clear one, and submits it.
    ///
    /// # Errors
    ///
    /// When the GPU's memory cannot hold what the frame draws: the image is presented as it is.
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
