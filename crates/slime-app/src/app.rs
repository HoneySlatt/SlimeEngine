//! The application lifecycle: the `App` trait that a game implements, and `run`, which drives it.

use std::time::Duration;

use slime_gfx::{Gpu, PresentMode, RenderTarget, Surface, TextureFormat};
use slime_platform::{Event, Window, WindowConfig};

use crate::diagnostics::init_diagnostics;
use crate::error::{AppError, RunError};
use crate::signal;
use crate::time::{FixedStep, FrameClock, FrameTime};
use crate::windowed::run_windowed;

/// An engine application: the code a game provides, which the engine's main loop calls.
///
/// Every method has a default implementation that does nothing, so an application implements
/// only the ones it needs. During [`run`], the engine calls:
///
/// 1. [`init`](Self::init), once;
/// 2. for each frame, [`fixed_update`](Self::fixed_update) once per fixed step due in that frame
///    (zero or more times), then [`update`](Self::update) once, then, with a window,
///    [`render`](Self::render) once if the window can show a new image;
/// 3. [`shutdown`](Self::shutdown), once, after the last frame, if `init` succeeded.
///
/// With a window, [`on_event`](Self::on_event) is called between frames, as events arrive.
///
/// Only `init` can return an error. An application that hits an error it cannot handle in
/// another method logs it and calls [`Context::request_exit`].
pub trait App {
    /// Called once, before the first frame. With a window, the window and the GPU already exist
    /// ([`Context::window`], [`Context::gpu`]): this is where an application creates its GPU
    /// objects, for targets of [`Context::surface_format`].
    ///
    /// # Errors
    ///
    /// An application returns an error when it cannot start, for example because a shader does not
    /// compile. The run then stops: no frame runs, [`shutdown`](Self::shutdown) is not called, and
    /// [`run`] returns [`RunError::Init`] with this error as its source.
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let _ = ctx;
        Ok(())
    }

    /// Called for each event concerning the window, between frames. Headless runs have no
    /// events.
    ///
    /// When the event is [`Event::CloseRequested`], the engine stops after this call, as if the
    /// application had called [`Context::request_exit`].
    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        let _ = (ctx, event);
    }

    /// Called once per fixed step. `step` is the duration of a step; it is the same for the
    /// whole run ([`AppConfig::fixed_step`]).
    fn fixed_update(&mut self, ctx: &mut Context, step: Duration) {
        let _ = (ctx, step);
    }

    /// Called once per frame, after the frame's fixed steps.
    fn update(&mut self, ctx: &mut Context, frame: &FrameTime) {
        let _ = (ctx, frame);
    }

    /// Called once per frame, after [`update`](Self::update), to draw the frame into `target`, the
    /// window's image, with the GPU of [`Context::gpu`]. The engine shows the image once this
    /// method returns.
    ///
    /// Only windowed runs call it, and only in frames where the window can show a new image: not
    /// while the window has a size of zero (minimized), nor when its next image is not ready in
    /// time on Vulkan (see [`Surface::acquire`]; Direct3D 12 waits for it instead). The fixed
    /// steps and the update of such frames still run.
    /// A frame that draws nothing into `target`, as this default does, shows it cleared to black.
    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let _ = (ctx, target);
    }

    /// Called once, after the last frame, however the run stopped, unless [`init`](Self::init)
    /// failed. With a window, the window and the GPU still exist; an application that keeps a
    /// clone of the window should drop it here.
    fn shutdown(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }
}

/// How [`run`] runs an application.
///
/// The fields are public so that a configuration is written as a struct literal, starting from
/// the defaults:
///
/// ```
/// use slime_platform::WindowConfig;
///
/// let config = slime_app::AppConfig {
///     window: Some(WindowConfig {
///         title: "My game".to_owned(),
///         ..WindowConfig::default()
///     }),
///     ..slime_app::AppConfig::default()
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppConfig {
    /// The window to open, or `None` to run headless. Default: `None`, so that the default
    /// configuration runs anywhere, including on machines without a display.
    pub window: Option<WindowConfig>,
    /// Stop after this many frames. `None`, the default, runs until the application requests an
    /// exit or, with a window, until the window is closed.
    pub frame_limit: Option<u64>,
    /// The duration of one fixed step. Default: 1/60 s, rounded to 16 666 667 ns.
    pub fixed_step: Duration,
    /// The largest number of fixed steps run in one frame; the rest is dropped. Default: 8.
    pub max_fixed_steps_per_frame: u32,
    /// The longest time one frame can represent. Default: 250 ms.
    pub max_frame_delta: Duration,
    /// With a window, the highest average number of frames per second that the engine asks for:
    /// frames are scheduled at this rate and the loop sleeps between them. This schedule paces the
    /// frames (`docs/adr/0021-frame-pacing.md`); the windowing system can ask for more, for
    /// example while the window is being resized, and with [`PresentMode::Fifo`] the display can
    /// slow them down. Headless runs ignore it. Default: 60.
    pub max_frame_rate: u32,
    /// With a window, how it shows the images drawn by [`App::render`]. Headless runs ignore it.
    /// Default: [`PresentMode::Mailbox`], which never makes a frame wait for the display, and is
    /// replaced by [`PresentMode::Fifo`] on windows that do not offer it.
    pub present_mode: PresentMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window: None,
            frame_limit: None,
            fixed_step: Duration::from_nanos(16_666_667),
            max_fixed_steps_per_frame: 8,
            max_frame_delta: Duration::from_millis(250),
            max_frame_rate: 60,
            present_mode: PresentMode::default(),
        }
    }
}

/// What an application can ask from the engine while it runs. Each [`App`] method receives it.
#[derive(Debug, Default)]
pub struct Context {
    window: Option<Window>,
    gpu: Option<Gpu>,
    surface_format: Option<TextureFormat>,
    exit: Option<ExitReason>,
}

impl Context {
    /// Asks the engine to stop. The current frame finishes, then [`App::shutdown`] is called and
    /// [`run`] returns. Asking more than once has no further effect.
    pub fn request_exit(&mut self) {
        self.exit_because(ExitReason::Requested);
    }

    /// Whether the run is going to stop: the application requested an exit, the window was closed,
    /// the process received a termination signal, the GPU device was lost, or the window's surface
    /// was lost for good.
    pub fn exit_requested(&self) -> bool {
        self.exit.is_some()
    }

    /// The window of the application, or `None` when it runs headless.
    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }

    /// The GPU that draws into the window, or `None` when the application runs headless: headless
    /// runs open no GPU. An application that needs one there opens its own with [`Gpu::new`].
    pub fn gpu(&self) -> Option<&Gpu> {
        self.gpu.as_ref()
    }

    /// The format of the window's images, which [`App::render`] draws into, or `None` when the
    /// application runs headless. Pipelines that draw into the window are created for it.
    pub fn surface_format(&self) -> Option<TextureFormat> {
        self.surface_format
    }

    pub(crate) fn set_window(&mut self, window: Option<Window>) {
        self.window = window;
    }

    pub(crate) fn set_gpu(&mut self, gpu: Option<Gpu>, surface_format: Option<TextureFormat>) {
        self.gpu = gpu;
        self.surface_format = surface_format;
    }

    /// Records why the run stops; the first reason is kept.
    pub(crate) fn exit_because(&mut self, reason: ExitReason) {
        self.exit.get_or_insert(reason);
    }
}

/// Why a run stops before reaching its frame limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExitReason {
    Requested,
    WindowClosed,
    Signal,
    DeviceLost,
    SurfaceLost,
}

/// Runs `app` until it stops, then returns.
///
/// It first calls [`init_diagnostics`], so logging works from the first line of `init`, and
/// installs a handler for termination signals. It then calls [`App::init`], runs frames until the
/// application requests an exit, the window is closed, the process receives a termination signal
/// or [`AppConfig::frame_limit`] is reached, and calls [`App::shutdown`]. An exit requested during
/// `init` runs no frame; an error returned by `init` runs no frame and no `shutdown`.
///
/// Termination signals are SIGINT (Ctrl+C in a terminal), SIGTERM (a service manager stopping the
/// process) and SIGHUP (the terminal closing). The first one stops the run cleanly, like an exit
/// request; a second one ends the process at once with exit code 130, for an application that no
/// longer finishes its frames. A signal received once stops every later run in the process too.
/// The handler stays installed after `run` returns, and a signal then no longer ends the process
/// by itself: if none was received yet, the first one only stops later runs; the next one ends
/// the process with exit code 130. The handler replaces whatever the process had set for these
/// signals, including another library's handlers and an inherited "ignore" (as under `nohup`). The
/// only exception is a handler installed earlier through the `ctrlc` crate, for example by the
/// game: it is kept, and signals then do not stop runs cleanly.
///
/// On Windows, Ctrl+C and Ctrl+Break in a console stop the run the same way. Closing the console
/// window ends the process at once, without `shutdown`.
///
/// - **Headless** ([`AppConfig::window`] is `None`): frames run back to back, as fast as the
///   machine allows, with simulated time: each frame represents exactly one fixed step, so it runs
///   exactly one fixed step, and every run gives the same times (as long as the fixed step is not
///   longer than the maximum frame delta, which would limit it). The windowing system is never
///   touched.
/// - **With a window**: the window and the GPU that draws into it are opened before `init`.
///   Frames represent the real time measured between them. The engine asks for a frame on
///   average at most [`AppConfig::max_frame_rate`] times per second, and a frame runs each time
///   the windowing system lets the window draw: when the engine asked, and when the system asks
///   on its own, for example while the window is being resized. Each frame ends with
///   [`App::render`], whose image the engine then shows as [`AppConfig::present_mode`] says.
///   Between frames, the loop sleeps until the next frame is due or an event arrives, and each
///   event is passed to [`App::on_event`].
///
/// Each frame is recorded as a `trace`-level profiling span named `frame`, containing one
/// `fixed_update` span per fixed step, one `update` span and, with a window, an `acquire` span
/// and, when the frame is drawn, a `render` span (containing `submit`, recorded by `slime-gfx`)
/// and a `present` span.
///
/// # Errors
///
/// - [`RunError::Init`] if [`App::init`] returns an error.
/// - With a window, [`RunError::Platform`]: no windowing system can be reached, the window cannot
///   be created, a windowed run was already attempted in this process (the windowing library
///   allows one attempt per process, even a failed one), or the event loop fails.
/// - With a window, [`RunError::Gfx`]: no GPU can draw into the window, the GPU device was lost
///   during the run (`GfxError::DeviceLost`: the driver crashed or was reset, the GPU was removed,
///   or it ran out of memory), or the window's surface was lost for good (`GfxError::SurfaceLost`:
///   it could not be created again after a loss, or was lost again each time). After a loss, the
///   run stops like after an exit request: the frame in progress finishes, without drawing, and
///   `shutdown` is called.
///
/// When the window or the GPU could not be opened, neither `init` nor `shutdown` is called.
/// Headless runs fail only when `init` does.
///
/// # Panics
///
/// If `config.fixed_step`, `config.max_fixed_steps_per_frame` or `config.max_frame_delta` is
/// zero. With a window, also if `config.max_frame_rate` is zero, if the window's width or height
/// is zero or above 16 384 logical pixels, if its title contains a NUL byte, or if `run` is not
/// called from the main thread.
pub fn run(app: &mut impl App, config: &AppConfig) -> Result<(), RunError> {
    init_diagnostics();
    signal::install_handler();
    match &config.window {
        None => run_headless(app, config),
        Some(window) => run_windowed(app, config, window),
    }
}

fn run_headless(app: &mut impl App, config: &AppConfig) -> Result<(), RunError> {
    let mut frames = Frames::new(config);
    let mut ctx = Context::default();

    tracing::debug!(?config, "running headless");
    app.init(&mut ctx).map_err(RunError::Init)?;
    let reason = loop {
        if let Some(reason) = frames.check_stop(&mut ctx, None) {
            break reason;
        }
        frames.run_one(app, &mut ctx, config.fixed_step, None);
    };
    tracing::info!(frames = frames.count(), "stopping: {reason}");
    app.shutdown(&mut ctx);
    Ok(())
}

/// The frame logic shared by every driver: timing, fixed steps, update, drawing, and when to stop.
pub(crate) struct Frames {
    clock: FrameClock,
    fixed: FixedStep,
    count: u64,
    limit: Option<u64>,
}

impl Frames {
    pub(crate) fn new(config: &AppConfig) -> Self {
        Self {
            clock: FrameClock::new(config.max_frame_delta),
            fixed: FixedStep::new(config.fixed_step, config.max_fixed_steps_per_frame),
            count: 0,
            limit: config.frame_limit,
        }
    }

    /// The number of frames run so far.
    pub(crate) fn count(&self) -> u64 {
        self.count
    }

    /// Why no further frame must run, if that is the case. A termination signal received since the
    /// last check, or the loss of the GPU or of the window's `surface`, is recorded in `ctx` first,
    /// so that the application sees it as an exit request.
    pub(crate) fn check_stop(
        &self,
        ctx: &mut Context,
        surface: Option<&Surface>,
    ) -> Option<&'static str> {
        if signal::received() {
            ctx.exit_because(ExitReason::Signal);
        }
        if ctx.gpu().is_some_and(|gpu| gpu.check().is_err()) {
            ctx.exit_because(ExitReason::DeviceLost);
        }
        if surface.is_some_and(|surface| surface.check().is_err()) {
            ctx.exit_because(ExitReason::SurfaceLost);
        }
        match ctx.exit {
            Some(ExitReason::Requested) => Some("the application requested an exit"),
            Some(ExitReason::WindowClosed) => Some("the window was closed"),
            Some(ExitReason::Signal) => Some("a termination signal was received"),
            Some(ExitReason::DeviceLost) => Some("the GPU device was lost"),
            Some(ExitReason::SurfaceLost) => Some("the window's surface was lost"),
            None if self.limit.is_some_and(|limit| self.count >= limit) => {
                Some("the frame limit was reached")
            }
            None => None,
        }
    }

    /// Runs one frame that represents `raw_delta` of time, and draws it into `surface` when there
    /// is one and it can show a new image.
    pub(crate) fn run_one(
        &mut self,
        app: &mut impl App,
        ctx: &mut Context,
        raw_delta: Duration,
        surface: Option<&mut Surface>,
    ) {
        let frame = self.clock.advance(raw_delta);
        let _frame_span = tracing::trace_span!("frame", index = frame.index()).entered();
        for _ in 0..self.fixed.advance(frame.delta()) {
            let _span = tracing::trace_span!("fixed_update").entered();
            app.fixed_update(ctx, self.fixed.step());
        }
        {
            let _span = tracing::trace_span!("update").entered();
            app.update(ctx, &frame);
        }
        let image = surface.and_then(|surface| {
            let _span = tracing::trace_span!("acquire").entered();
            surface.acquire(&format!("window, frame {}", frame.index()))
        });
        if let Some(image) = image {
            {
                let _span = tracing::trace_span!("render").entered();
                app.render(ctx, image.target());
            }
            let _span = tracing::trace_span!("present").entered();
            image.present();
        }
        self.count += 1;
    }
}
