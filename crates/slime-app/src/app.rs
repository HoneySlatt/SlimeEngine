//! The application lifecycle: the `App` trait that a game implements, and `run`, which drives it.

use std::time::Duration;

use slime_platform::{Event, PlatformError, Window, WindowConfig};

use crate::diagnostics::init_diagnostics;
use crate::signal;
use crate::time::{FixedStep, FrameClock, FrameTime};
use crate::windowed::run_windowed;

/// An engine application: the code a game provides, which the engine's main loop calls.
///
/// Every method has an empty default implementation, so an application implements only the ones
/// it needs. During [`run`], the engine calls:
///
/// 1. [`init`](Self::init), once;
/// 2. for each frame, [`fixed_update`](Self::fixed_update) once per fixed step due in that frame
///    (zero or more times), then [`update`](Self::update) once;
/// 3. [`shutdown`](Self::shutdown), once, after the last frame.
///
/// With a window, [`on_event`](Self::on_event) is called between frames, as events arrive.
///
/// The methods cannot return errors. An application that hits an error it cannot handle logs it
/// and calls [`Context::request_exit`].
pub trait App {
    /// Called once, before the first frame. With a window, the window already exists
    /// ([`Context::window`]).
    fn init(&mut self, ctx: &mut Context) {
        let _ = ctx;
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

    /// Called once, after the last frame, however the run stopped. With a window, the window
    /// still exists; an application that keeps a clone of it should drop it here.
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
    /// With a window, the highest number of frames per second; the loop waits between frames to
    /// stay under it. Headless runs ignore it. Default: 60.
    ///
    /// This limiter is temporary: once frames are presented on screen (Stage 2), the display's
    /// refresh rate paces them.
    pub max_frame_rate: u32,
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
        }
    }
}

/// What an application can ask from the engine while it runs. Each [`App`] method receives it.
#[derive(Debug, Default)]
pub struct Context {
    window: Option<Window>,
    exit: Option<ExitReason>,
}

impl Context {
    /// Asks the engine to stop. The current frame finishes, then [`App::shutdown`] is called and
    /// [`run`] returns. Asking more than once has no further effect.
    pub fn request_exit(&mut self) {
        self.exit_because(ExitReason::Requested);
    }

    /// Whether the run is going to stop: the application requested an exit, the window was closed,
    /// or the process received a termination signal.
    pub fn exit_requested(&self) -> bool {
        self.exit.is_some()
    }

    /// The window of the application, or `None` when it runs headless.
    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }

    pub(crate) fn set_window(&mut self, window: Option<Window>) {
        self.window = window;
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
}

/// Runs `app` until it stops, then returns.
///
/// It first calls [`init_diagnostics`], so logging works from the first line of `init`, and installs
/// a handler for termination signals. It then calls [`App::init`], runs frames until the
/// application requests an exit, the window is closed, the process receives a termination signal
/// or [`AppConfig::frame_limit`] is reached, and calls [`App::shutdown`]. An exit requested during
/// `init` runs no frame.
///
/// Termination signals are SIGINT (Ctrl+C in a terminal), SIGTERM (a service manager stopping the
/// process) and SIGHUP (the terminal closing). The first one stops the run cleanly, like an exit
/// request; a second one ends the process at once with exit code 130, for an application that no
/// longer finishes its frames. A signal received once stops every later run in the process too.
///
/// - **Headless** ([`AppConfig::window`] is `None`): frames run back to back, as fast as the
///   machine allows, with simulated time: each frame represents exactly one fixed step, so it runs
///   exactly one fixed step, and every run gives the same times (as long as the fixed step is not
///   longer than the maximum frame delta, which would limit it). The windowing system is never
///   touched.
/// - **With a window**: the window is created before `init`, and frames represent the real time
///   measured between them, at most [`AppConfig::max_frame_rate`] per second. Between frames, the
///   loop sleeps until the next frame is due or an event arrives, and each event is passed to
///   [`App::on_event`].
///
/// Each frame is recorded as a `trace`-level profiling span named `frame`, containing one
/// `fixed_update` span per fixed step and one `update` span.
///
/// # Errors
///
/// With a window, any [`PlatformError`]: no windowing system can be reached, the window cannot
/// be created, an event loop already ran in this process (only one windowed run is possible per
/// process), or the event loop fails. When the window could not be created, neither `init` nor
/// `shutdown` is called. Headless runs do not fail.
///
/// # Panics
///
/// If `config.fixed_step`, `config.max_fixed_steps_per_frame` or `config.max_frame_delta` is
/// zero, or, with a window, if `config.max_frame_rate` is zero.
pub fn run(app: &mut impl App, config: &AppConfig) -> Result<(), PlatformError> {
    init_diagnostics();
    signal::install_handler();
    match &config.window {
        None => {
            run_headless(app, config);
            Ok(())
        }
        Some(window) => run_windowed(app, config, window),
    }
}

fn run_headless(app: &mut impl App, config: &AppConfig) {
    let mut frames = Frames::new(config);
    let mut ctx = Context::default();

    tracing::debug!(?config, "running headless");
    app.init(&mut ctx);
    let reason = loop {
        if let Some(reason) = frames.check_stop(&mut ctx) {
            break reason;
        }
        frames.run_one(app, &mut ctx, config.fixed_step);
    };
    tracing::info!(frames = frames.count(), "stopping: {reason}");
    app.shutdown(&mut ctx);
}

/// The frame logic shared by every driver: timing, fixed steps, update, and when to stop.
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
    /// last check is recorded in `ctx` first, so that the application sees it as an exit request.
    pub(crate) fn check_stop(&self, ctx: &mut Context) -> Option<&'static str> {
        if signal::received() {
            ctx.exit_because(ExitReason::Signal);
        }
        match ctx.exit {
            Some(ExitReason::Requested) => Some("the application requested an exit"),
            Some(ExitReason::WindowClosed) => Some("the window was closed"),
            Some(ExitReason::Signal) => Some("a termination signal was received"),
            None if self.limit.is_some_and(|limit| self.count >= limit) => {
                Some("the frame limit was reached")
            }
            None => None,
        }
    }

    /// Runs one frame that represents `raw_delta` of time.
    pub(crate) fn run_one(&mut self, app: &mut impl App, ctx: &mut Context, raw_delta: Duration) {
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
        self.count += 1;
    }
}
