//! A *blank window* on winit and wgpu: it clears its window to one colour and presents it, and at
//! each input of the person changes the colour and presents again; nothing else, and nothing at
//! rest. `scripts/host-cost.py` measures it beside the same program on uwui's platform layer,
//! `uwui-desktop`'s example `blank`, and beside the host's example `application`
//! (`docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md`,
//! `docs/adr/0307-m6d7a2c-cut-in-two-the-host-measured-first-the-blank-window-on-winit-in-a-package-of-its-own.md`).
//!
//! ```sh
//! cargo run -p uwui-blank-winit
//! ```
//!
//! It is written as winit asks of a program: its frames come on `RedrawRequested`, each asked by
//! `request_redraw`, and `pre_present_notify` before each present lets winit pace them on the
//! compositor's frame callback on Wayland. A frame that finds no image tries again 100 ms later, as
//! uwui's host does. It opens its device as uwui's host does: the adapter that uses the least
//! power, unless `WGPU_POWER_PREF` names another; the adapter's limits; wgpu's hint `MemoryUsage`;
//! an sRGB image, presented in Mailbox where the surface offers it, else Fifo. A lost device or
//! surface ends its run. Where `UWUI_TRACE` names a file, it writes uwui's desktop host's trace
//! there, the same events (`uwui_desktop::TRACE_ENV_VAR`): each frame with the notches of the wheel
//! it took, and each input to the present of the image after it, an input's time its reception,
//! since winit gives none. Closing its window ends it with status 0; it takes no termination
//! signal, which ends it as the system does.

use std::fs::File;
use std::io::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// The environment variable that names the trace's file, uwui's desktop host's own.
const TRACE_ENV_VAR: &str = "UWUI_TRACE";

/// How long a frame that found no image waits before it tries again: uwui's host's wait for a late
/// signal.
const RETRY: Duration = Duration::from_millis(100);

/// The device, its queue and the window's surface.
struct Gpu {
    // Dropped before the device, as wgpu asks.
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    queue: wgpu::Queue,
    device: wgpu::Device,
}

/// The program: its window and device, the colour it shows.
#[derive(Default)]
struct Blank {
    // Dropped before the window, whose surface it holds.
    gpu: Option<Gpu>,
    window: Option<Arc<Window>>,
    trace: Option<Trace>,
    /// The inputs taken, which choose the colour.
    inputs: u32,
    /// Why the run failed, if it did.
    failure: Option<String>,
}

impl ApplicationHandler for Blank {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        self.trace = Trace::from_env();
        let attributes = Window::default_attributes()
            .with_title("uwui blank window on winit")
            .with_inner_size(LogicalSize::new(720, 480));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => return self.fail(event_loop, format!("no window opens: {error}")),
        };
        let size = window.inner_size();
        match Gpu::new(Arc::clone(&window), [size.width, size.height]) {
            Ok(gpu) => self.gpu = Some(gpu),
            Err(error) => return self.fail(event_loop, error),
        }
        window.request_redraw();
        self.window = Some(window);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            self.retry(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let (Some(window), Some(gpu)) = (&self.window, &mut self.gpu) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                gpu.resize([size.width, size.height]);
                window.request_redraw();
            }
            WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::KeyboardInput { .. } => {
                if let Some(trace) = &mut self.trace {
                    let wheel = u32::from(matches!(event, WindowEvent::MouseWheel { .. }));
                    trace.input(Instant::now(), wheel);
                }
                self.inputs = self.inputs.wrapping_add(1);
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let start = Instant::now();
                match gpu.draw(self.inputs, || window.pre_present_notify()) {
                    Ok(true) => {
                        if let Some(trace) = &mut self.trace
                            && let Err(error) = trace.presented(start, Instant::now())
                        {
                            eprintln!("the blank window: the trace stops: {error}");
                            self.trace = None;
                        }
                    }
                    // No image now: asked again a wait later, rather than at once.
                    Ok(false) => {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + RETRY));
                    }
                    Err(error) => {
                        self.failure = Some(error);
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }
}

impl Blank {
    /// The wait for an image ended: the frame is asked again, and the loop waits with no deadline.
    fn retry(&self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Ends the run on `failure`.
    fn fail(&mut self, event_loop: &ActiveEventLoop, failure: String) {
        self.failure = Some(failure);
        event_loop.exit();
    }
}

impl Gpu {
    /// Opens a device that draws into `window`, at `size` in physical pixels.
    fn new(window: Arc<Window>, size: [u32; 2]) -> Result<Self, String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY);
        descriptor.flags = wgpu::InstanceFlags::from_build_config().with_env();
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(window)
            .map_err(|cause| format!("the window's surface cannot be made: {cause}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:
                wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower),
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|cause| format!("no GPU can draw into the window: {cause}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("blank on winit"),
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|cause| format!("the GPU's device cannot be opened: {cause}"))?;
        let config = configuration(&surface.get_capabilities(&adapter), size)?;
        surface.configure(&device, &config);
        Ok(Self {
            surface,
            config,
            queue,
            device,
        })
    }

    fn resize(&mut self, size: [u32; 2]) {
        let size = size.map(|side| side.max(1));
        if size != [self.config.width, self.config.height] {
            [self.config.width, self.config.height] = size;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Clears the window's next image to the colour of `inputs` and presents it, `before_present`
    /// called just before: whether an image was presented. An image refused as outdated is taken
    /// again once the surface is configured again; one suboptimal is presented, the surface
    /// configured again after.
    fn draw(&mut self, inputs: u32, before_present: impl FnOnce()) -> Result<bool, String> {
        let mut configure = false;
        let mut image = None;
        for _ in 0..2 {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(taken) => image = Some(taken),
                wgpu::CurrentSurfaceTexture::Suboptimal(taken) => {
                    configure = true;
                    image = Some(taken);
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    self.surface.configure(&self.device, &self.config);
                    continue;
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {}
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Validation => {
                    return Err("the window's surface is lost".to_owned());
                }
            }
            break;
        }
        let Some(image) = image else {
            return Ok(false);
        };
        let view = image
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(colour(inputs)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.queue.submit([encoder.finish()]);
        before_present();
        self.queue.present(image);
        // Configured once no image of it is held.
        if configure {
            self.surface.configure(&self.device, &self.config);
        }
        Ok(true)
    }
}

/// The surface's configuration at `size`: an sRGB format, Mailbox where offered, else Fifo, as
/// uwui's desktop host's.
fn configuration(
    capabilities: &wgpu::SurfaceCapabilities,
    size: [u32; 2],
) -> Result<wgpu::SurfaceConfiguration, String> {
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .ok_or("the window offers no sRGB format")?;
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
    Ok(wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size[0].max(1),
        height: size[1].max(1),
        present_mode,
        color_space: wgpu::SurfaceColorSpace::Auto,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: Vec::new(),
    })
}

/// The colour after `inputs` inputs: a dark grey whose blue steps with each.
fn colour(inputs: u32) -> wgpu::Color {
    wgpu::Color {
        r: 0.05,
        g: 0.05,
        b: 0.05 + f64::from(inputs % 8) * 0.02,
        a: 1.0,
    }
}

/// uwui's desktop host's trace, written the same way: frames, and each input to the present of the
/// image after it, in microseconds since the Unix epoch, followed on Rust's `Instant`.
struct Trace {
    file: File,
    origin: Instant,
    origin_since_epoch: Duration,
    waiting: Option<Instant>,
    /// The notches of the wheel taken since the last image presented.
    wheel: u32,
    /// Whether an image was presented yet: inputs before the first are part of the start.
    shown: bool,
}

impl Trace {
    fn from_env() -> Option<Self> {
        let path = std::env::var_os(TRACE_ENV_VAR).filter(|path| !path.is_empty())?;
        let mut file = File::create(&path)
            .map_err(|error| eprintln!("the blank window: the trace cannot be written: {error}"))
            .ok()?;
        let header = "[\n\
            {\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":1,\"args\":{\"name\":\"frames\"}},\n\
            {\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":2,\"args\":{\"name\":\"input to present\"}},\n";
        file.write_all(header.as_bytes()).ok()?;
        let (origin, wall) = (Instant::now(), SystemTime::now());
        Some(Self {
            file,
            origin,
            origin_since_epoch: wall.duration_since(UNIX_EPOCH).ok()?,
            waiting: None,
            wheel: 0,
            shown: false,
        })
    }

    fn input(&mut self, received: Instant, wheel: u32) {
        self.wheel = self.wheel.saturating_add(wheel);
        if self.shown {
            self.waiting.get_or_insert(received);
        }
    }

    fn presented(&mut self, start: Instant, end: Instant) -> std::io::Result<()> {
        let wheel = std::mem::take(&mut self.wheel);
        let mut events = self.complete("frame", 1, start, end);
        let close = events.len() - "},\n".len();
        events.insert_str(close, &format!(",\"args\":{{\"wheel\":{wheel}}}"));
        self.shown = true;
        if let Some(input) = self.waiting.take() {
            events.push_str(&self.complete("input to present", 2, input, end));
        }
        self.file.write_all(events.as_bytes())
    }

    fn complete(&self, name: &str, track: u32, from: Instant, to: Instant) -> String {
        let ts = (self.origin_since_epoch + from.saturating_duration_since(self.origin)).as_nanos();
        let dur = to.saturating_duration_since(from).as_nanos();
        format!(
            "{{\"name\":\"{name}\",\"cat\":\"blank\",\"ph\":\"X\",\"ts\":{}.{:03},\"dur\":{}.{:03},\
             \"pid\":1,\"tid\":{track}}},\n",
            ts / 1000,
            ts % 1000,
            dur / 1000,
            dur % 1000,
        )
    }
}

fn main() {
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("the blank window cannot start: {error}");
            std::process::exit(1);
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut blank = Blank::default();
    let ran = event_loop.run_app(&mut blank);
    let failure = blank
        .failure
        .or_else(|| ran.err().map(|error| error.to_string()));
    if let Some(failure) = failure {
        eprintln!("the blank window stopped: {failure}");
        std::process::exit(1);
    }
}
