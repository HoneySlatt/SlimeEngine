//! A *blank window* on the host's platform layer alone, `uwui-windowing`, and wgpu: it clears its
//! window to one colour and presents it, and at each input of the person changes the colour and
//! presents again; nothing else, and nothing at rest. `scripts/host-cost.py` measures it beside
//! the same program on winit, `uwui-blank-winit`, and the `application` example: what the host
//! and the interface cost above it is theirs
//! (`docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md`,
//! `docs/adr/0307-m6d7a2c-cut-in-two-the-host-measured-first-the-blank-window-on-winit-in-a-package-of-its-own.md`).
//!
//! ```sh
//! cargo run -p uwui-desktop --example blank
//! cargo run -p uwui-desktop --example blank -- --fullscreen  # filling its display, without borders
//! ```
//!
//! It opens its device as the host does: the adapter that uses the least power, unless
//! `WGPU_POWER_PREF` names another; the adapter's limits; wgpu's hint `MemoryUsage`; an sRGB image,
//! presented in Mailbox where the surface offers it, else Fifo. Its frames follow the host's main
//! rule (`docs/adr/0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md`): the
//! first after a rest at once, then each on the platform's signal of the image before; it has not
//! the host's frame for a signal that never comes, and draws nothing just in time: it is the plain
//! program that the host's pacing is measured against. It answers the pointer, its buttons, the wheel
//! and the keys, not the focus nor the modifiers, as the same program on winit. It makes nothing
//! again after a loss: a lost device or surface ends its run. Where `UWUI_TRACE` names a file, it
//! writes the host's trace there, with the host's own writer (`uwui_desktop::TRACE_ENV_VAR`).
//! Closing the window, or a termination signal, ends it with status 0.

use std::time::Duration;

use uwui_windowing::{Control, Drawn, Handler, Happened, Input, Opening, Stopped, Window};

// The host's trace writer, the same file: the blank window depends on nothing else of the host's.
#[expect(
    dead_code,
    reason = "the blank window answers every input it takes, and predicts no time that it could miss"
)]
#[path = "../src/trace.rs"]
mod trace;

use trace::Trace;

// The host's instance of wgpu, the same file: its graphics API and, on Direct3D 12, its shader
// compiler.
#[path = "../src/instance.rs"]
mod instance;

/// How long a frame that found no image waits before it tries again: the host's wait for a late
/// signal.
const SIGNAL_WAIT: Duration = Duration::from_millis(100);

/// The device, its queue and the window's surface.
struct Gpu {
    // Dropped before the device, as wgpu asks.
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    queue: wgpu::Queue,
    device: wgpu::Device,
}

/// The program: its device, the colour it shows, and when it draws.
#[derive(Default)]
struct Blank {
    gpu: Option<Gpu>,
    trace: Option<Trace>,
    /// The inputs taken, which choose the colour.
    inputs: u32,
    /// Whether a new image is wanted.
    wanted: bool,
    /// Whether the last image waits for the platform's signal.
    awaiting: bool,
    /// Why the run failed, if it did.
    failure: Option<String>,
}

impl Handler for Blank {
    fn opened(&mut self, window: &mut Window<'_>, drawn: Drawn) -> Result<(), Stopped> {
        self.trace = match Trace::from_env(window.now()) {
            Some(Ok(trace)) => Some(trace),
            Some(Err(error)) => {
                eprintln!("the blank window: the trace cannot be written: {error}");
                None
            }
            None => None,
        };
        let (display, surface) = window.raw_handles();
        #[expect(unsafe_code, reason = "the device draws on the window's raw handles")]
        // SAFETY: the layer keeps the handles live until `closing` returns, and drops the program
        // before them otherwise; `closing` drops the device.
        let gpu = unsafe { Gpu::new(display, surface, drawn.pixels) };
        match gpu {
            Ok(gpu) => self.gpu = Some(gpu),
            Err(error) => {
                self.failure = Some(error);
                return Err(Stopped);
            }
        }
        self.wanted = true;
        Ok(())
    }

    fn turn(&mut self, window: &mut Window<'_>, happened: Happened) -> Control {
        // A frame's work counts from its turn's start, as the host's.
        let start = window.now();
        if happened.close_asked || happened.terminated {
            return Control::Exit;
        }
        if happened.signal.is_some() {
            self.awaiting = false;
        }
        if let Some(trace) = &mut self.trace {
            for presentation in &happened.presentations {
                if let Some(shown) = presentation.shown
                    && let Err(error) = trace.shown(presentation.image, shown)
                {
                    eprintln!("the blank window: the trace stops: {error}");
                    self.trace = None;
                    break;
                }
            }
        }
        if let Some(drawn) = happened.drawn
            && let Some(gpu) = &mut self.gpu
        {
            gpu.resize(drawn.pixels);
        }
        self.wanted |= happened.configured.is_some() || happened.drawn.is_some();
        // The person's input that it answers, as the program on winit does.
        let answered = happened.inputs.iter().filter(|(_, input)| {
            matches!(
                input,
                Input::PointerMoved { .. }
                    | Input::PointerButton { .. }
                    | Input::Wheel(_)
                    | Input::Key(_)
            )
        });
        if let Some(oldest) = answered.clone().map(|(time, _)| time.at).min() {
            self.inputs = self.inputs.wrapping_add(1);
            self.wanted = true;
            if let Some(trace) = &mut self.trace {
                let wheel = answered
                    .filter(|(_, input)| matches!(input, Input::Wheel(_)))
                    .count();
                trace.input(oldest, u32::try_from(wheel).unwrap_or(u32::MAX));
            }
        }
        if self.wanted && !self.awaiting {
            let Some(gpu) = &mut self.gpu else {
                return Control::Exit;
            };
            match gpu.draw(self.inputs, || window.ask_signal()) {
                Ok(Some(image)) => {
                    self.wanted = false;
                    self.awaiting = true;
                    if let Some(trace) = &mut self.trace
                        && let Err(error) = trace.presented(start, window.now(), image)
                    {
                        eprintln!("the blank window: the trace stops: {error}");
                        self.trace = None;
                    }
                }
                // No image now: tried again a signal's wait later, as the host does.
                Ok(None) => return Control::Wait(Some(window.now() + SIGNAL_WAIT)),
                Err(error) => {
                    self.failure = Some(error);
                    return Control::Exit;
                }
            }
        }
        Control::Wait(None)
    }

    fn closing(&mut self) {
        self.gpu = None;
    }
}

impl Gpu {
    /// Opens a device that draws into the window of `display` and `window`, at `size` in physical
    /// pixels.
    ///
    /// # Safety
    ///
    /// The handles outlive the value returned.
    #[expect(unsafe_code, reason = "its caller keeps the raw handles live")]
    unsafe fn new(
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
        size: [u32; 2],
    ) -> Result<Self, String> {
        let instance = instance::instance();
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(display),
            raw_window_handle: window,
        };
        #[expect(
            unsafe_code,
            reason = "wgpu makes a surface on the platform's raw handles"
        )]
        // SAFETY: the caller keeps the handles live while the surface lives.
        let surface = unsafe { instance.create_surface_unsafe(target) }
            .map_err(|cause| format!("the window's surface cannot be made: {cause}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:
                wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower),
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|cause| format!("no GPU can draw into the window: {cause}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("blank"),
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
    /// called just before, which gives its number: the number, if an image was presented. An image
    /// refused as outdated is taken again once the surface is configured again; one suboptimal is
    /// presented, the surface configured again after.
    fn draw(
        &mut self,
        inputs: u32,
        before_present: impl FnOnce() -> u64,
    ) -> Result<Option<u64>, String> {
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
            return Ok(None);
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
        let number = before_present();
        self.queue.present(image);
        // Configured once no image of it is held.
        if configure {
            self.surface.configure(&self.device, &self.config);
        }
        Ok(Some(number))
    }
}

/// The surface's configuration at `size`: an sRGB format, Mailbox where offered, else Fifo, as the
/// host's.
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

fn main() {
    // `--fullscreen` opens it filling its display, without borders, as the tests ask.
    let fullscreen = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--fullscreen") => true,
        Some(_) => {
            eprintln!("usage: blank [--fullscreen]");
            std::process::exit(2);
        }
    };
    let opening = Opening {
        title: "uwui blank window".to_owned(),
        app_id: "dev.uwui.Blank".to_owned(),
        size: [720, 480],
        min_size: None,
        fullscreen,
    };
    let (blank, ran) = uwui_windowing::run(opening, Blank::default());
    let failure = blank
        .failure
        .or_else(|| ran.err().map(|error| error.to_string()));
    if let Some(failure) = failure {
        eprintln!("the blank window stopped: {failure}");
        std::process::exit(1);
    }
}
