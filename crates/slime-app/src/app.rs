//! Running a game: [`run`], the host of a game's program, and how it runs ([`AppConfig`]).

use std::time::{Duration, Instant};

use slime_gfx::{PresentMode, Surface};
use slime_platform::WindowConfig;

use crate::diagnostics::{error_chain, init_diagnostics};
use crate::error::RunError;
use crate::game::{Presentation, Simulation};
use crate::host::{Context, ExitReason, Host};
use crate::signal;
use crate::windowed::{next_due, run_windowed};

/// How [`run`] runs a game.
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
    /// Stop after this many frames. `None`, the default, runs until the game requests an exit or,
    /// with a window, until the window is closed.
    pub frame_limit: Option<u64>,
    /// The duration of one fixed step of game time, at every speed. Default: 1/60 s, rounded to
    /// 16 666 667 ns.
    pub fixed_step: Duration,
    /// The largest number of fixed steps run in one frame; the rest is dropped. Default: 8.
    pub max_fixed_steps_per_frame: u32,
    /// The longest time one frame can represent. Default: 250 ms.
    pub max_frame_delta: Duration,
    /// With a window, the highest average number of frames per second that the engine asks for:
    /// frames are scheduled at this rate and the loop sleeps between them. This schedule paces the
    /// frames (`docs/adr/0021-frame-pacing.md`); the windowing system can ask for more, for
    /// example while the window is being resized, and with [`PresentMode::Fifo`] the display can
    /// slow them down. Headless runs ignore it, but those that [`AppConfig::pace_headless`] paces.
    /// Default: 60.
    pub max_frame_rate: u32,
    /// With a window, how it shows the images drawn by [`Presentation::render`]. Headless runs
    /// ignore it. Default: [`PresentMode::Mailbox`], which never makes a frame wait for the
    /// display, and is replaced by [`PresentMode::Fifo`] on windows that do not offer it.
    pub present_mode: PresentMode,
    /// With a window, whether the run opens a new GPU device when it loses its own, and lets the
    /// presentation create its GPU objects again ([`Presentation::gpu_restored`]), rather than
    /// stopping (ADR-0075). Headless runs ignore it: they open no GPU. Default: `false`, so that a
    /// game that does not create its objects again stops cleanly, with
    /// [`GfxError::DeviceLost`](slime_gfx::GfxError::DeviceLost).
    pub restore_lost_device: bool,
    /// Without a window, whether frames are scheduled at [`AppConfig::max_frame_rate`], the loop
    /// sleeping between them, as a program that waits for tools does, an editor without a window
    /// (`docs/adr/0134-the-editors-core.md`); each frame still represents one fixed step. Windowed
    /// runs ignore it, which are paced anyway. Default: `false`, so that tests and benchmarks run
    /// their frames back to back.
    pub pace_headless: bool,
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
            pace_headless: false,
        }
    }
}

/// Runs a game, its `simulation` and its `presentation`, until it stops, then returns: the host of
/// a game's program (`docs/adr/0107-the-execution-model.md`), which drives a [`Host`] with the
/// window's events and draws its frames. A program with no rules passes `&mut ()` as its
/// simulation, and one that shows nothing `&mut ()` as its presentation.
///
/// It first calls [`init_diagnostics`], so logging works from the first line of a `start`, and
/// installs a handler for termination signals. It then calls [`Simulation::start`] and, with a
/// window, [`Presentation::start`], runs frames until the game requests an exit, the window is
/// closed, the process receives a termination signal, the tool that started it with a token ends
/// (with the `development` feature, below) or [`AppConfig::frame_limit`] is reached, and calls
/// [`Presentation::stop`] then [`Simulation::stop`]. An exit requested during a `start`
/// runs no frame; an error returned by a `start` runs no frame and no `stop`.
///
/// Termination signals are SIGINT (Ctrl+C in a terminal), SIGTERM (a service manager stopping the
/// process) and SIGHUP (the terminal closing). The first one stops the run cleanly, like an exit
/// request; a second one, half a second or more after the first, ends the process at once with exit
/// code 130, for a game that no longer finishes its frames, while one that comes sooner, as when a
/// closing terminal sends SIGHUP twice, is ignored. A signal received once stops every later run in
/// the process too. The handler stays installed after `run` returns, and a signal then no longer
/// ends the process by itself: if none was received yet, the first one only stops later runs; the
/// next one, half a second or more later, ends the process with exit code 130. The handler
/// replaces whatever the process had set for these signals, including another library's handlers
/// and an inherited "ignore" (as under `nohup`). The only exception is a handler installed earlier
/// through the `ctrlc` crate, for example by the game: it is kept, and signals then do not stop
/// runs cleanly.
///
/// On Windows, Ctrl+C and Ctrl+Break in a console stop the run the same way. Closing the console
/// window ends the process at once, without the `stop` methods. Logging off and shutting down end
/// it without them too: Windows tells a window of them with messages that the windowing library
/// does not report.
///
/// - **Headless** ([`AppConfig::window`] is `None`): frames run back to back, as fast as the
///   machine allows, or at [`AppConfig::max_frame_rate`] when [`AppConfig::pace_headless`] says,
///   with simulated time: each frame represents exactly one fixed step, so it runs
///   exactly one fixed step at a speed of 1, and every run gives the same times (as long as the
///   fixed step is not longer than the maximum frame delta, which would limit it). The windowing
///   system is never touched, and the presentation never called.
/// - **With a window**: the window and the GPU that draws into it are opened before the `start`
///   methods. Frames represent the real time measured between them. The engine asks for a frame
///   on average at most [`AppConfig::max_frame_rate`] times per second, and a frame runs each time
///   the windowing system lets the window draw: when the engine asked, and when the system asks
///   on its own, for example while the window is being resized. Each frame runs the host's phases
///   ([`Host::frame`]), then [`Presentation::extract`] and, when the window can show a new image,
///   [`Presentation::render`], whose image the engine then shows as [`AppConfig::present_mode`]
///   says. Between frames, the loop sleeps until the next frame is due or an event arrives; each
///   event is passed to the presentation's interface ([`Presentation::ui`]), then, unless the
///   interface took it, to [`Presentation::on_event`], then to the host's input. After each frame
///   drawn, the window shows the pointer's shape that the interface asks, and allows the input
///   method while one of its fields takes text.
///
/// Each frame is recorded as a `trace`-level profiling span named `frame`, containing one
/// `fixed_step` span per fixed step, one `update` span and, with a window, an `extract` span, an
/// `acquire` span and, when the frame is drawn, a `render` span (containing `submit`, recorded by
/// `slime-gfx`) and a `present` span.
///
/// # Errors
///
/// - [`RunError::Init`] if a `start` returns an error.
/// - [`RunError::App`] if the game passed an error to [`Context::exit_with_error`] or
///   [`SimulationContext::exit_with_error`](crate::SimulationContext::exit_with_error), from any
///   method; it is returned even when the GPU device or the window's surface was lost too, but for
///   an error that the device's loss causes, which the run handles as its own loss.
/// - With a window, [`RunError::Platform`]: no windowing system can be reached, the window cannot
///   be created, a windowed run was already attempted in this process (the windowing library
///   allows one attempt per process, even a failed one), or the event loop fails.
/// - With a window, [`RunError::Gfx`]: no GPU can draw into the window, the GPU device was lost
///   during the run (`GfxError::DeviceLost`: the driver crashed or was reset, the GPU was removed,
///   or it ran out of memory), or the window's surface was lost for good (`GfxError::SurfaceLost`:
///   it could not be created again after a loss, or was lost again each time). After a loss, the
///   run stops like after an exit request: the frame in progress finishes, without drawing, and
///   the `stop` methods are called. A game that sets [`AppConfig::restore_lost_device`] goes on
///   with a new device instead ([`Presentation::gpu_lost`], [`Presentation::gpu_restored`]): its
///   run returns `GfxError::DeviceLost` at a loss that comes after three new devices within five
///   minutes, and `GfxError::CannotRestore`, or the error of opening a device, when none replaces
///   the lost one.
///
/// When the window or the GPU could not be opened, neither `start` nor `stop` is called. Headless
/// runs fail only when the game does. A failed run is logged at the `error` level, with the message
/// of each of the error's causes in turn, as [`error_chain`] gives them (`the run failed: cannot
/// draw into the window: no GPU adapter is available: ...`), so that a caller only has to exit with
/// a failure.
///
/// Built with the `development` feature, a run whose environment gives a token listens for the
/// tools that drive it, before its `start` methods, and answers them at the start of each frame
/// (the crate's documentation says what they ask). It stops, as at a termination signal, once its
/// standard input closes, which the tool that started it holds as a pipe, unless that input is a
/// terminal: an input that is empty or missing, as `/dev/null`, stops it at once
/// (`slime_channel::TOKEN_ENV_VAR`).
///
/// # Panics
///
/// If `config.fixed_step`, `config.max_fixed_steps_per_frame` or `config.max_frame_delta` is
/// zero. With a window, or [`AppConfig::pace_headless`], also if `config.max_frame_rate` is zero.
/// With a window, also if the window's width or height is zero or above 16 384 logical pixels, if
/// its title contains a NUL byte, or if `run` is not called from the main thread. On Linux, also
/// when the windowing library cannot load libxkbcommon, or libxkbcommon-x11 on X11 (see
/// [`slime_platform::run_event_loop`]); on Wayland, this can happen during the run, when a keyboard
/// first appears.
pub fn run(
    simulation: &mut impl Simulation,
    presentation: &mut impl Presentation,
    config: &AppConfig,
) -> Result<(), RunError> {
    init_diagnostics();
    signal::install_handler();
    let result = match &config.window {
        None => run_headless(simulation, config),
        Some(window) => run_windowed(simulation, presentation, config, window),
    };
    if let Err(error) = &result {
        tracing::error!("the run failed: {}", error_chain(error));
    }
    result
}

fn run_headless(simulation: &mut impl Simulation, config: &AppConfig) -> Result<(), RunError> {
    let mut frames = Frames::new(config);
    let mut host = Host::new(config);

    let period = config.pace_headless.then(|| {
        assert!(config.max_frame_rate > 0, "a paced run has a frame rate");
        Duration::from_secs(1) / config.max_frame_rate
    });
    tracing::debug!(?config, "running headless");
    host.start(simulation).map_err(RunError::Init)?;
    let mut due = Instant::now();
    let reason = loop {
        if let Some(reason) = frames.check_stop(&mut host.context, None, false) {
            break reason;
        }
        if let Some(period) = period {
            let now = Instant::now();
            if now < due {
                std::thread::sleep(due - now);
            }
            due = next_due(due, Instant::now(), period);
        }
        frames.run_one(&mut host, simulation, &mut (), config.fixed_step, None);
    };
    tracing::info!(frames = frames.count(), "stopping: {reason}");
    host.stop(simulation);
    host.context
        .take_error()
        .map_or(Ok(()), |error| Err(RunError::App(error)))
}

/// The frame logic shared by every driver: the frames counted, drawing, and when to stop.
pub(crate) struct Frames {
    count: u64,
    limit: Option<u64>,
    /// The development channel, when the environment gave a token.
    #[cfg(feature = "development")]
    channel: Option<crate::channel::DevChannel>,
    /// What the window shows of the presentation's interface.
    shown: crate::interface::Shown,
    /// What screen readers read of the presentation's interface.
    #[cfg(feature = "accessibility")]
    pub(crate) screen_readers: crate::accessibility::ScreenReaders,
}

impl Frames {
    pub(crate) fn new(config: &AppConfig) -> Self {
        Self {
            count: 0,
            limit: config.frame_limit,
            #[cfg(feature = "development")]
            channel: crate::channel::DevChannel::open(),
            shown: crate::interface::Shown::default(),
            #[cfg(feature = "accessibility")]
            screen_readers: crate::accessibility::ScreenReaders::new(
                config
                    .window
                    .as_ref()
                    .map_or("", |window| window.title.as_str()),
            ),
        }
    }

    /// The number of frames run so far.
    pub(crate) fn count(&self) -> u64 {
        self.count
    }

    /// Why no further frame must run, if that is the case. A termination signal received since the
    /// last check, or the loss of the GPU or of the window's `surface`, is recorded in `ctx` first,
    /// as the reason the run stops; not the loss of the GPU when the run can still replace it, as
    /// `restorable` says, which it does before the next frame. From then on, the game also sees a
    /// loss as an exit request; it sees a signal as one as soon as it arrives
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
        #[cfg(feature = "development")]
        if crate::channel::tool_ended() {
            ctx.exit_because(ExitReason::ToolEnded);
        }
        if !restorable && ctx.gpu().is_some_and(|gpu| gpu.check().is_err()) {
            ctx.exit_because(ExitReason::DeviceLost);
        }
        if surface.is_some_and(|surface| surface.check().is_err()) {
            ctx.exit_because(ExitReason::SurfaceLost);
        }
        match ctx.exit_reason() {
            Some(ExitReason::Requested) => Some("the application requested an exit"),
            Some(ExitReason::WindowClosed) => Some("the window was closed"),
            Some(ExitReason::Signal) => Some("a termination signal was received"),
            #[cfg(feature = "development")]
            Some(ExitReason::ToolEnded) => Some("the tool that started the game ended"),
            Some(ExitReason::DeviceLost) => Some("the GPU device was lost"),
            Some(ExitReason::SurfaceLost) => Some("the window's surface was lost"),
            Some(ExitReason::Failed) => Some("the application stopped with an error"),
            None if self.limit.is_some_and(|limit| self.count >= limit) => {
                Some("the frame limit was reached")
            }
            None => None,
        }
    }

    /// Runs one frame that represents `raw_delta` of time: the host's phases, then, with a
    /// window's `surface`, the presentation's extraction and, when the surface can show a new
    /// image, its rendering into it.
    pub(crate) fn run_one(
        &mut self,
        host: &mut Host,
        simulation: &mut impl Simulation,
        presentation: &mut impl Presentation,
        raw_delta: Duration,
        surface: Option<&mut Surface>,
    ) {
        // The host's clock numbers its frames as this count does: one each.
        let index = self.count;
        let _frame_span = tracing::trace_span!("frame", index).entered();
        #[cfg(feature = "development")]
        if let Some(channel) = &mut self.channel {
            channel.answer(
                host,
                simulation,
                presentation,
                index,
                raw_delta,
                surface.is_some(),
            );
        }
        host.frame(simulation, raw_delta);
        if let Some(surface) = surface {
            {
                let _span = tracing::trace_span!("extract").entered();
                presentation.extract(&mut host.context, &host.world);
            }
            let image = {
                let _span = tracing::trace_span!("acquire").entered();
                surface.acquire(&format!("window, frame {index}"))
            };
            if let Some(image) = image {
                {
                    let _span = tracing::trace_span!("render").entered();
                    presentation.render(&mut host.context, image.target());
                }
                // The pointer's shape and the input method, as the interface left them, or as the
                // window shows them without one.
                if let Some(window) = host.context.window() {
                    self.shown.update(window, presentation.ui().as_deref());
                    // What changed in what it means, while a screen reader reads the window.
                    #[cfg(feature = "accessibility")]
                    self.screen_readers.update(window, presentation.ui());
                }
                #[cfg(feature = "development")]
                if let (Some(channel), Some(gpu)) = (&mut self.channel, host.context.gpu())
                    && channel.wants_capture()
                {
                    let views = presentation.views();
                    channel.capture(gpu, image.target(), &views, presentation.ui().as_deref());
                }
                let _span = tracing::trace_span!("present").entered();
                image.present();
            }
        }
        self.count += 1;
    }
}
