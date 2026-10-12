//! The application lifecycle: the `App` trait that a game implements, and `run`, which drives it.

use std::time::Duration;

use crate::diagnostics::init_diagnostics;
use crate::time::{FixedStep, FrameClock, FrameTime};

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
/// The methods cannot return errors. An application that hits an error it cannot handle logs it
/// and calls [`Context::request_exit`].
pub trait App {
    /// Called once, before the first frame.
    fn init(&mut self, ctx: &mut Context) {
        let _ = ctx;
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

    /// Called once, after the last frame, even when the run was stopped by an exit request.
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
/// let config = slime_app::AppConfig {
///     frame_limit: Some(600),
///     ..Default::default()
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppConfig {
    /// Stop after this many frames. `None`, the default, runs until the application requests an
    /// exit.
    pub frame_limit: Option<u64>,
    /// The duration of one fixed step. Default: 1/60 s, rounded to 16 666 667 ns.
    pub fixed_step: Duration,
    /// The largest number of fixed steps run in one frame; the rest is dropped. Default: 8.
    pub max_fixed_steps_per_frame: u32,
    /// The longest time one frame can represent. Default: 250 ms.
    pub max_frame_delta: Duration,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            frame_limit: None,
            fixed_step: Duration::from_nanos(16_666_667),
            max_fixed_steps_per_frame: 8,
            max_frame_delta: Duration::from_millis(250),
        }
    }
}

/// What an application can ask from the engine while it runs. Each [`App`] method receives it.
#[derive(Debug, Default)]
pub struct Context {
    exit_requested: bool,
}

impl Context {
    /// Asks the engine to stop. The current frame finishes, then [`App::shutdown`] is called and
    /// [`run`] returns. Asking more than once has no further effect.
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    /// Whether an exit has been requested during this run.
    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }
}

/// Runs `app` until it stops, then returns.
///
/// It first calls [`init_diagnostics`], so logging works from the first line of `init`. It then
/// calls [`App::init`], runs frames until the application requests an exit or
/// [`AppConfig::frame_limit`] is reached, and calls [`App::shutdown`]. An exit requested during
/// `init` runs no frame.
///
/// The application runs headless (without a window). Frames run back to back, as fast as the
/// machine allows, with simulated time: each frame represents exactly one fixed step, so it runs
/// exactly one fixed step, and every run gives the same times (as long as the fixed step is not
/// longer than the maximum frame delta, which would limit it).
///
/// Each frame is recorded as a `trace`-level profiling span named `frame`, containing one
/// `fixed_update` span per fixed step and one `update` span.
///
/// # Panics
///
/// If `config.fixed_step`, `config.max_fixed_steps_per_frame` or `config.max_frame_delta` is
/// zero.
pub fn run(app: &mut impl App, config: &AppConfig) {
    init_diagnostics();
    let mut frames = Frames::new(config);
    let mut ctx = Context::default();

    tracing::debug!(?config, "running headless");
    app.init(&mut ctx);
    let reason = loop {
        if ctx.exit_requested() {
            break "the application requested an exit";
        }
        if config
            .frame_limit
            .is_some_and(|limit| frames.count >= limit)
        {
            break "the frame limit was reached";
        }
        frames.run_one(app, &mut ctx, config.fixed_step);
    };
    tracing::info!(frames = frames.count, "stopping: {reason}");
    app.shutdown(&mut ctx);
}

/// The frame logic shared by every way of running an application: timing, fixed steps, update.
struct Frames {
    clock: FrameClock,
    fixed: FixedStep,
    count: u64,
}

impl Frames {
    fn new(config: &AppConfig) -> Self {
        Self {
            clock: FrameClock::new(config.max_frame_delta),
            fixed: FixedStep::new(config.fixed_step, config.max_fixed_steps_per_frame),
            count: 0,
        }
    }

    /// Runs one frame that represents `raw_delta` of time.
    fn run_one(&mut self, app: &mut impl App, ctx: &mut Context, raw_delta: Duration) {
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
