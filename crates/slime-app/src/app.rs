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
/// Only `init` returns an error. An application that hits an error it cannot handle in another
/// method passes it to [`Context::exit_with_error`]: the run stops as after an exit request, and
/// [`run`] returns the error.
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
    /// time on Vulkan (see [`Surface::acquire`]; Direct3D 12 waits for it instead, and on Windows
    /// with Vulkan, wgpu treats a wait of more than one second as a lost device, which stops the
    /// run). The fixed steps and the update of such frames still run.
    /// A frame that draws nothing into `target`, as this default does, shows it cleared to black.
    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let _ = (ctx, target);
    }

    /// Called between frames when the GPU device was lost and the run is about to open a new one,
    /// as an application asks with [`AppConfig::restore_lost_device`]: the application drops every
    /// GPU object it made with the lost device, which [`Context::gpu`] still is, and every clone of
    /// that `Gpu`, its loads in progress included, whatever their state. The run then releases its
    /// own and opens the new device, which Direct3D 12 does only once nothing holds the lost one
    /// (ADR-0075). Objects kept past this call make the new device open on another adapter, if
    /// any, and panic if used with it. The default does nothing, which suits an application that
    /// keeps no GPU object between frames.
    fn gpu_lost(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }

    /// Called between frames once the run opened a new GPU device after [`gpu_lost`](Self::gpu_lost):
    /// [`Context::gpu`] is the new device, and [`Context::surface_format`] the format of the
    /// window's images, which another adapter may change. The application creates its GPU objects
    /// again here, as in [`init`](Self::init). The default does nothing.
    ///
    /// The run does not restore, and stops as without the setting, once it has opened three devices
    /// in a row without a frame shown in between, when the device was lost because memory ran out,
    /// when only a CPU adapter is left where the lost device was a GPU's, or when no device opens
    /// ([`Surface::restore`]); nor when the run is stopping anyway, or the window's surface was lost
    /// for good. When no device opens after `gpu_lost`, [`Context::gpu`] is `None` from then on,
    /// [`App::shutdown`] included.
    ///
    /// # Errors
    ///
    /// An application returns an error when it cannot go on with the new device. The run then stops
    /// as after [`Context::exit_with_error`], and [`run`] returns [`RunError::App`] with this error.
    fn gpu_restored(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let _ = ctx;
        Ok(())
    }

    /// Called once, after the last frame, however the run stopped, unless [`init`](Self::init)
    /// failed or the process ends before the run stops: on a second termination signal, when the
    /// console closes or Windows logs off or shuts down (see [`run`](crate::run)), or when the
    /// connection to the X server is lost. With a window, the window and the GPU still exist, but for
    /// a GPU lost that no new one replaced after [`gpu_lost`](Self::gpu_lost); an application that
    /// keeps a clone of the window should drop it here.
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
    /// With a window, whether the run opens a new GPU device when it loses its own, and lets the
    /// application create its GPU objects again ([`App::gpu_restored`]), rather than stopping
    /// (ADR-0075). Headless runs ignore it: they open no GPU. Default: `false`, so that an
    /// application that does not create its objects again stops cleanly, as the engine did
    /// before.
    pub restore_lost_device: bool,
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
            restore_lost_device: false,
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
    /// The error passed to `exit_with_error`, which `run` returns.
    error: Option<AppError>,
    fixed_step_alpha: f64,
}

impl Context {
    /// Asks the engine to stop. The current frame finishes, then [`App::shutdown`] is called and
    /// [`run`] returns. Asking more than once has no further effect.
    pub fn request_exit(&mut self) {
        self.exit_because(ExitReason::Requested);
    }

    /// Stops the run because of `error`, which [`run`] then returns as [`RunError::App`]: this is
    /// how an application fails in a method that returns no error. The run stops as after
    /// [`request_exit`](Self::request_exit): the current frame finishes, then [`App::shutdown`]
    /// is called. `shutdown` can call it too, and so fail a run that stopped for another reason.
    ///
    /// Only the first error is returned; a later one is logged as an error. When the GPU device or
    /// the window's surface is lost as well, `run` returns the application's error, and the loss is
    /// in the log, where the engine writes it when it happens.
    pub fn exit_with_error(&mut self, error: impl Into<AppError>) {
        let error = error.into();
        if self.error.is_some() {
            tracing::error!("the application failed again: {error}");
        } else {
            self.error = Some(error);
        }
        self.exit_because(ExitReason::Failed);
    }

    /// Whether the run is going to stop: the application requested an exit, the window was closed,
    /// the process received a termination signal, the GPU device was lost and not replaced
    /// ([`AppConfig::restore_lost_device`]), or the window's surface was lost for good.
    ///
    /// A termination signal shows here as soon as it is received, even during [`App::init`] or in
    /// the middle of a frame, so that a long loading or a long update that checks it can stop
    /// early. The loss of the GPU device or of the surface is noticed only between frames.
    pub fn exit_requested(&self) -> bool {
        self.exit.is_some() || signal::received()
    }

    /// The fraction of a fixed step that the frames so far accumulated but did not run, at least
    /// 0.0 and below 1.0: the interpolation factor, [`FixedStep::alpha`] of the run's fixed step.
    ///
    /// An application that moves things in [`App::fixed_update`] keeps their state of the last two
    /// steps and draws them this fraction of the way from the older state to the newer, so that
    /// motion looks smooth when the frame rate and the step rate differ. The value is set once per
    /// frame, after the frame's time is added and before its fixed steps run: it is the same in
    /// that frame's `fixed_update`, [`App::update`] and [`App::render`], and 0.0 before the first
    /// frame. Headless frames each represent one fixed step and read 0.0, unless the step is
    /// longer than [`AppConfig::max_frame_delta`].
    pub fn fixed_step_alpha(&self) -> f64 {
        self.fixed_step_alpha
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

    /// Takes the GPU out, with the format of the window's images.
    pub(crate) fn take_gpu(&mut self) -> Option<Gpu> {
        self.surface_format = None;
        self.gpu.take()
    }

    /// The error passed to `exit_with_error`, if there is one.
    pub(crate) fn take_error(&mut self) -> Option<AppError> {
        self.error.take()
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
    Failed,
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
/// request; a second one, half a second or more after the first, ends the process at once with exit
/// code 130, for an application that no longer finishes its frames, while one that comes sooner, as
/// when a closing terminal sends SIGHUP twice, is ignored. A signal received once stops every later
/// run in the process too. The handler stays installed after `run` returns, and a signal then no
/// longer ends the process by itself: if none was received yet, the first one only stops later
/// runs; the next one, half a second or more later, ends the process with exit code 130. The
/// handler replaces whatever the process had set for these signals, including another library's
/// handlers and an inherited "ignore" (as under `nohup`). The only exception is a handler installed
/// earlier through the `ctrlc` crate, for example by the game: it is kept, and signals then do not
/// stop runs cleanly.
///
/// On Windows, Ctrl+C and Ctrl+Break in a console stop the run the same way. Closing the console
/// window ends the process at once, without `shutdown`. Logging off and shutting down end it
/// without `shutdown` too: Windows tells a window of them with messages that the windowing library
/// does not report.
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
/// - [`RunError::App`] if the application passed an error to [`Context::exit_with_error`], from
///   any method; it is returned even when the GPU device or the window's surface was lost too.
/// - With a window, [`RunError::Platform`]: no windowing system can be reached, the window cannot
///   be created, a windowed run was already attempted in this process (the windowing library
///   allows one attempt per process, even a failed one), or the event loop fails.
/// - With a window, [`RunError::Gfx`]: no GPU can draw into the window, the GPU device was lost
///   during the run (`GfxError::DeviceLost`: the driver crashed or was reset, the GPU was removed,
///   or it ran out of memory), or the window's surface was lost for good (`GfxError::SurfaceLost`:
///   it could not be created again after a loss, or was lost again each time). After a loss, the
///   run stops like after an exit request: the frame in progress finishes, without drawing, and
///   `shutdown` is called. An application that sets [`AppConfig::restore_lost_device`] goes on
///   with a new device instead ([`App::gpu_lost`], [`App::gpu_restored`]): its run returns
///   `GfxError::DeviceLost` after three new devices in a row without a frame shown, and
///   `GfxError::CannotRestore`, or the error of opening a device, when none replaces the lost one.
///
/// When the window or the GPU could not be opened, neither `init` nor `shutdown` is called.
/// Headless runs fail only when the application does. A failed run is logged at the `error` level, with
/// the message of each of the error's causes in turn (`the run failed: cannot draw into the window:
/// no GPU adapter is available: ...`), so that a caller only has to exit with a failure.
///
/// # Panics
///
/// If `config.fixed_step`, `config.max_fixed_steps_per_frame` or `config.max_frame_delta` is
/// zero. With a window, also if `config.max_frame_rate` is zero, if the window's width or height
/// is zero or above 16 384 logical pixels, if its title contains a NUL byte, or if `run` is not
/// called from the main thread. On Linux, also when the windowing library cannot load
/// libxkbcommon, or libxkbcommon-x11 on X11 (see [`slime_platform::run_event_loop`]); on
/// Wayland, this can happen during the run, when a keyboard first appears.
pub fn run(app: &mut impl App, config: &AppConfig) -> Result<(), RunError> {
    init_diagnostics();
    signal::install_handler();
    let result = match &config.window {
        None => run_headless(app, config),
        Some(window) => run_windowed(app, config, window),
    };
    if let Err(error) = &result {
        // The error's own message is the most general one: the cause is in its sources.
        let mut message = error.to_string();
        let mut source = std::error::Error::source(error);
        while let Some(cause) = source {
            message.push_str(": ");
            message.push_str(&cause.to_string());
            source = cause.source();
        }
        tracing::error!("the run failed: {message}");
    }
    result
}

fn run_headless(app: &mut impl App, config: &AppConfig) -> Result<(), RunError> {
    let mut frames = Frames::new(config);
    let mut ctx = Context::default();

    tracing::debug!(?config, "running headless");
    app.init(&mut ctx).map_err(RunError::Init)?;
    let reason = loop {
        if let Some(reason) = frames.check_stop(&mut ctx, None, false) {
            break reason;
        }
        let _ = frames.run_one(app, &mut ctx, config.fixed_step, None);
    };
    tracing::info!(frames = frames.count(), "stopping: {reason}");
    app.shutdown(&mut ctx);
    ctx.take_error()
        .map_or(Ok(()), |error| Err(RunError::App(error)))
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
    /// as the reason the run stops; not the loss of the GPU when the run can still replace it, as
    /// `restorable` says, which it does before the next frame. From then on, the application also
    /// sees a loss as an exit request; it sees a signal as one as soon as it arrives
    /// (`Context::exit_requested`).
    pub(crate) fn check_stop(
        &self,
        ctx: &mut Context,
        surface: Option<&Surface>,
        restorable: bool,
    ) -> Option<&'static str> {
        if signal::received() {
            ctx.exit_because(ExitReason::Signal);
        }
        if !restorable && ctx.gpu().is_some_and(|gpu| gpu.check().is_err()) {
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
            Some(ExitReason::Failed) => Some("the application stopped with an error"),
            None if self.limit.is_some_and(|limit| self.count >= limit) => {
                Some("the frame limit was reached")
            }
            None => None,
        }
    }

    /// Runs one frame that represents `raw_delta` of time, and draws it into `surface` when there
    /// is one and it can show a new image. Returns whether the window showed the frame's image.
    pub(crate) fn run_one(
        &mut self,
        app: &mut impl App,
        ctx: &mut Context,
        raw_delta: Duration,
        surface: Option<&mut Surface>,
    ) -> bool {
        let frame = self.clock.advance(raw_delta);
        let _frame_span = tracing::trace_span!("frame", index = frame.index()).entered();
        let steps = self.fixed.advance(frame.delta());
        ctx.fixed_step_alpha = self.fixed.alpha();
        for _ in 0..steps {
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
        let mut shown = false;
        if let Some(image) = image {
            {
                let _span = tracing::trace_span!("render").entered();
                app.render(ctx, image.target());
            }
            let _span = tracing::trace_span!("present").entered();
            image.present();
            // Once the device is lost, the image is released rather than shown.
            shown = ctx.gpu().is_some_and(|gpu| gpu.check().is_ok());
        }
        self.count += 1;
        shown
    }
}
