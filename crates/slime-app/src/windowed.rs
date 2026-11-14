//! The windowed driver: frames run from the callbacks of the platform's event loop, each time the
//! windowing system lets the window draw.

use std::time::{Duration, Instant};

use slime_gfx::{Gpu, PresentMode, Surface};
use slime_platform::{Event, EventHandler, LoopControl, Window, WindowConfig};
use uwui::host::TargetId;
use uwui::time::{EventTime, FrameShown, FrameSource};

use crate::app::{AppConfig, Frames};
use crate::error::RunError;
use crate::game::{Presentation, Simulation};
use crate::host::{ExitReason, Host};
use crate::time::HostClock;

/// How many devices the run opens to replace lost ones within `RESTORE_WINDOW`: at the next loss
/// within it, it gives up, since a device lost again at once, or soon after each new one, will not
/// stay.
const MAX_RESTORES: usize = 3;
/// The time within which the run opens at most `MAX_RESTORES` devices.
const RESTORE_WINDOW: Duration = Duration::from_secs(5 * 60);
/// The longest that a frame asked for from outside the loop waits for the compositor's signal
/// (`docs/adr/0187-a-hidden-window-at-rest.md`): longer than a refresh of any display, so that a
/// window shown keeps its frames in step with it, and short enough that a request to a hidden
/// game, an agent's, is answered at once.
const SIGNAL_WAIT: Duration = Duration::from_millis(100);
/// The shortest time suspended that the log tells: what parts the two clocks while the machine is
/// awake is far below it.
const SUSPEND_TOLD: Duration = Duration::from_secs(1);
/// The first wait before a window's surface given up is made again; each attempt that fails
/// doubles it, up to `LONGEST_REMAKE_WAIT`.
const FIRST_REMAKE_WAIT: Duration = Duration::from_secs(1);
/// The longest wait between two attempts to make a surface given up again.
const LONGEST_REMAKE_WAIT: Duration = Duration::from_secs(16);

pub(crate) fn run_windowed(
    simulation: &mut impl Simulation,
    presentation: &mut impl Presentation,
    config: &AppConfig,
    window: &WindowConfig,
) -> Result<(), RunError> {
    assert!(
        config.max_frame_rate > 0,
        "the maximum frame rate must not be zero"
    );
    let now = Instant::now();
    let host = Host::new(config);
    let frames = Frames::new(config, &host.context);
    let ui_waker = host.context.waker().task_waker();
    let mut driver = WindowedDriver {
        simulation,
        presentation,
        host,
        frames,
        surface: None,
        present_mode: config.present_mode,
        restore_lost_device: config.restore_lost_device,
        restores: Vec::new(),
        frame_period: Duration::from_secs(1) / config.max_frame_rate,
        last_frame: now,
        clock: HostClock::new(),
        ui_waker,
        next_frame: now,
        redraw_asked: None,
        asked_from_outside: false,
        checked_hold: false,
        resting: false,
        inputs: InputWait::default(),
        input: crate::interface::WindowInput::default(),
        occluded: false,
        remake: Remake::default(),
        wanted: true,
        change_due: None,
        started: false,
        stop: None,
        error: None,
    };
    tracing::debug!(?config, "running with a window");
    slime_platform::run_event_loop(window, &mut driver)?;
    driver.error.map_or(Ok(()), Err)
}

struct WindowedDriver<'a, S, P> {
    simulation: &'a mut S,
    presentation: &'a mut P,
    host: Host,
    frames: Frames,
    /// The window's surface; the GPU that draws into it is in the host's context.
    surface: Option<Surface>,
    present_mode: PresentMode,
    /// Whether a lost device is replaced (`AppConfig::restore_lost_device`); cleared when the run
    /// gives up replacing it.
    restore_lost_device: bool,
    /// When the run opened the devices that replaced lost ones, within `RESTORE_WINDOW`.
    restores: Vec<Instant>,
    /// The interval at which frames are scheduled, from `AppConfig::max_frame_rate`.
    frame_period: Duration,
    last_frame: Instant,
    /// The host's clock, which gives the presentation's interface its times: each frame's start and
    /// each event's reception (`docs/adr/0227-the-time-that-motion-reads.md`). The loop's own
    /// pacing waits on `std::time::Instant`, as the platform's event loop does.
    clock: HostClock,
    /// The run's waker as the presentation's interface takes it (`Ui::set_waker`).
    ui_waker: std::task::Waker,
    /// When the next frame is due.
    next_frame: Instant,
    /// When the loop asked the window for a redraw that has not come: on Wayland, one that waits
    /// for the compositor's signal that the window can show a new image.
    redraw_asked: Option<Instant>,
    /// Whether something outside the loop asked for a frame since the last one ran: an event, a
    /// wake of the run's waker, which a thread, a termination signal, the tool's end and the
    /// development channel give, or a device restored. Unlike the program's own requests, these
    /// run a frame while the window holds its frames.
    asked_from_outside: bool,
    /// Whether the window, holding its frames since its last redraw, ran the one frame that checks
    /// that the image before reached the compositor (`held`).
    checked_hold: bool,
    /// Whether the loop waits, frames running on demand, with no frame asked for: the frame that
    /// ends the wait represents the time since what asked for it, not the wait, so that what
    /// moves at real time does not jump by the frame's longest time.
    resting: bool,
    /// The inputs that wait for an image, whose intervals of input to present the trace holds.
    inputs: InputWait,
    /// What the presentation's interface is given of the window's events: the pointer's moves of
    /// a frame as one.
    input: crate::interface::WindowInput,
    /// Whether the system says that the window is covered whole (`Event::Occluded`), on X11 and
    /// macOS: it then holds its frames, as a hidden one.
    occluded: bool,
    /// The window's surface given up, and when the run makes it again.
    remake: Remake,
    /// Whether something asked for a frame that has not run yet, when frames run on demand.
    wanted: bool,
    /// When the presentation's interface changes on its own, which a frame then shows, when frames
    /// run on demand: a tooltip that shows, a notification that ends (`Ui::next_frame_due`); or when
    /// the presentation asked for a frame (`Context::request_frame_at`), the earlier.
    change_due: Option<Instant>,
    /// Whether both `start` methods succeeded, which makes both `stop` methods due at the end.
    started: bool,
    /// Why the loop is ending, once it has been decided.
    stop: Option<&'static str>,
    /// Why the run failed, if it did.
    error: Option<RunError>,
}

impl<S: Simulation, P: Presentation> WindowedDriver<'_, S, P> {
    /// Opens the GPU for the window, then starts the simulation and the presentation.
    fn start(&mut self, window: &Window) -> Result<(), RunError> {
        let size = window.inner_size();
        let (gpu, surface) =
            Gpu::new_for_window(window.clone(), size.width, size.height, self.present_mode)?;
        self.host.context.set_gpu(Some(gpu), Some(surface.format()));
        self.surface = Some(surface);
        self.host.start(self.simulation).map_err(RunError::Init)?;
        let host = &mut self.host;
        self.presentation
            .start(&mut host.context, &host.world)
            .map_err(RunError::Init)
    }

    /// Makes the frame that ends a hold of the window's frames represent one period of the schedule
    /// at most, so that game time barely moves while the window is hidden.
    fn end_hold(&mut self, now: Instant) {
        if let Some(start) = now.checked_sub(self.frame_period) {
            self.last_frame = self.last_frame.max(start);
        }
    }

    /// Ends the loop's rest, if it rested: the next frame's time starts now.
    fn end_rest(&mut self) {
        if std::mem::take(&mut self.resting) {
            self.last_frame = Instant::now();
        }
    }

    /// Runs a frame, unless the run is stopping, or ends with this frame: the redraw that the loop
    /// asked for, one that the windowing system asked for, or one that something outside the loop
    /// asked for while the window holds its frames.
    fn frame(&mut self) {
        if self.started {
            self.restore();
        }
        self.redraw_asked = None;
        self.asked_from_outside = false;
        if self.started {
            self.follow_surface(Instant::now());
        }
        if !self.started
            || self.stop.is_some()
            || self
                .frames
                .check_stop(&mut self.host.context, self.restore_lost_device)
                .is_some()
        {
            return;
        }
        let now = Instant::now();
        // The interface shows the frame at its start: the engine reads no time of presentation
        // yet (M6d.7).
        let shown = self.clock.now();
        if let Some(ui) = self.presentation.ui() {
            // The pointer's last move first, at its own time, before the frame moves the clock.
            self.input.give_moved(ui);
            ui.set_frame_shown(
                TargetId::MAIN,
                FrameShown {
                    at: shown,
                    source: FrameSource::Started,
                },
            );
            if !ui.waker().will_wake(&self.ui_waker) {
                ui.set_waker(self.ui_waker.clone());
            }
        }
        let presented = self.frames.run_one(
            &mut self.host,
            self.simulation,
            self.presentation,
            now - self.last_frame,
            self.surface.as_mut(),
        );
        if self.host.context.take_lose_surface()
            && let Some(surface) = &mut self.surface
        {
            surface.lose();
        }
        // What the interface asks of the window and the clipboard, as the frame left it, whether
        // or not it drew an image, a window hidden or covered included: the pointer's shape, the
        // input method, and the reads that the frame asked, the editor's paste among them,
        // answered at once, whose paste the next frame shows.
        let ui = self.presentation.ui();
        let answered = match self.host.context.window() {
            Some(window) => {
                let time = EventTime::received(self.clock.now());
                self.frames.shown.show(window, ui, time)
            }
            None => false,
        };
        if presented {
            self.after_present();
        }
        self.last_frame = now;
        self.next_frame = next_due(self.next_frame, now, self.frame_period);
        let ui = self.presentation.ui();
        // On the loop's clock, which the platform waits on, from the host's.
        let interface_due = ui
            .as_ref()
            .and_then(|ui| ui.next_frame_due(TargetId::MAIN))
            .map(|due| now + due.saturating_duration_since(shown));
        // The earlier of the interface's change and the time that the presentation asked for.
        self.change_due = match (interface_due, self.host.context.take_frame_due()) {
            (Some(interface), Some(asked)) => Some(interface.min(asked)),
            (due, None) | (None, due) => due,
        };
        self.wanted = answered
            || ui.is_some_and(|ui| interface_asks(ui, presented))
            || self.host.context.take_frame_request()
            || self.frames.waiting()
            || self.host.context.steps_asked() > 0;
    }

    /// What follows the present of an image: the interval from the oldest input that it follows,
    /// written into the trace, and the time of the first image, logged.
    fn after_present(&mut self) {
        let now = Instant::now();
        match self.inputs.presented() {
            Presented::First => tracing::info!(
                "the first image is presented, {:.1} ms after the start",
                crate::since_start().as_secs_f64() * 1e3
            ),
            Presented::After(input) => crate::trace::input_presented(input, now),
            Presented::Alone => {}
        }
    }

    /// Whether the window holds its frames whatever the compositor says: it has no area to draw
    /// into, as a minimized window on Windows; the system says that it is covered whole, on X11 and
    /// macOS; or its surface was given up, until it is made again.
    fn holds_frames(&self) -> bool {
        let empty = self.host.context.window().is_some_and(|window| {
            let size = window.inner_size();
            size.width == 0 || size.height == 0
        });
        empty || self.occluded || self.remake.waiting()
    }

    /// Notices a surface that `slime-gfx` gave up, and makes it again once due, rather than ending
    /// the run; stops the run once the surfaces given up, and the attempts that failed, pass the
    /// budget (`Remake`).
    fn follow_surface(&mut self, now: Instant) {
        let Some(surface) = &mut self.surface else {
            return;
        };
        if surface.check().is_ok() {
            return;
        }
        if !self.remake.waiting() {
            if !self.remake.given_up(now) {
                tracing::error!(
                    "the window's surface was given up again after {MAX_RESTORES} within {} \
                     minutes: the run stops",
                    RESTORE_WINDOW.as_secs() / 60
                );
                self.host.context.exit_because(ExitReason::SurfaceLost);
                return;
            }
            tracing::warn!("the window holds its frames until its surface is made again");
        }
        if !self.remake.due(now) {
            return;
        }
        match surface.make_again() {
            Ok(()) => {
                self.remake.made();
                // Its first frame runs even if the window holds its frames, as after a new device.
                self.asked_from_outside = true;
                self.wanted = true;
            }
            Err(error) => {
                tracing::warn!(%error, "the window's surface cannot be made again yet");
                if !self.remake.failed(now) {
                    tracing::error!("the run stops: the window's surface cannot be made again");
                    self.host.context.exit_because(ExitReason::SurfaceLost);
                }
            }
        }
    }

    /// What the loop does while the window holds its frames: a redraw asked for that has not come,
    /// as on Wayland until the compositor signals that the window can show a new image, which a
    /// hidden window may never get; or no area to draw into. The loop wakes on no schedule of its
    /// own meanwhile. What asks from outside runs a frame, once the compositor's signal is late
    /// (`held_due`); and so does each frame that the development channel still waits for.
    ///
    /// The first time a redraw waits that long, one frame runs too, whatever asks: an image
    /// presented to a device lost before the run knew it never reached the compositor, whose
    /// signal would then never come. That frame finds the loss, and the device restored draws.
    fn held(&mut self, now: Instant) -> LoopControl {
        let unchecked = self.redraw_asked.is_some() && !self.checked_hold;
        let Some(due) = held_due(
            self.asked_from_outside || self.frames.waiting() || unchecked,
            self.redraw_asked.unwrap_or(self.last_frame),
            self.next_frame,
        ) else {
            return LoopControl::Wait;
        };
        if now < due {
            return LoopControl::WaitUntil(due);
        }
        let pending = self.redraw_asked.is_some();
        self.checked_hold |= pending;
        self.end_hold(now);
        self.frame();
        // The redraw asked for still waits for the compositor's signal, from now on.
        if pending {
            self.redraw_asked = Some(Instant::now());
        }
        // The loop looks again at once, as after a redraw: the frame may have asked it to stop.
        LoopControl::WaitUntil(Instant::now())
    }

    /// When the loop wakes next, a surface given up apart: once the frame asked for is due, or
    /// when something asks while the window holds its frames; never while frames run on demand and
    /// nothing asks for one.
    fn next_wake(&mut self) -> LoopControl {
        let now = Instant::now();
        let woken = self.host.context.waker().take();
        self.asked_from_outside |= woken;
        if self.host.context.on_demand() {
            self.wanted |= woken
                | self.host.context.take_frame_request()
                | self.change_due.is_some_and(|due| due <= now);
            if !self.wanted {
                // An event, a wake, a signal, the tool's end or the interface's change runs this
                // again.
                self.resting = true;
                return self
                    .change_due
                    .map_or(LoopControl::Wait, LoopControl::WaitUntil);
            }
        }
        if self.redraw_asked.is_none() && !self.holds_frames() {
            if now < self.next_frame {
                return LoopControl::WaitUntil(self.next_frame);
            }
            // The frame runs when the redraw arrives, which wakes the loop. Nothing else needs a
            // deadline: a termination signal, the tool's end and the development channel wake it.
            if let Some(window) = self.host.context.window() {
                window.request_redraw();
                self.redraw_asked = Some(now);
            }
        }
        self.held(now)
    }

    /// When the GPU device is lost and the game chose to replace it, lets the presentation drop its
    /// objects of the lost device (`Presentation::gpu_lost`), releases the run's own, opens a new
    /// device for the window and lets the presentation create its objects again
    /// (`Presentation::gpu_restored`), between frames. Does nothing when the run is stopping
    /// anyway, or while the window's surface is given up. Gives up, leaving the loss to stop the
    /// run, once `MAX_RESTORES` devices were opened within `RESTORE_WINDOW`; and when no device
    /// opens, stops the run with that error.
    fn restore(&mut self) {
        if !self.restore_lost_device || self.stop.is_some() || self.host.context.exit_requested() {
            return;
        }
        let Some(Err(loss)) = self.host.context.gpu().map(Gpu::check) else {
            return;
        };
        if !self
            .surface
            .as_ref()
            .is_some_and(|surface| surface.check().is_ok())
        {
            return;
        }
        let now = Instant::now();
        self.restores
            .retain(|&opened| now.duration_since(opened) < RESTORE_WINDOW);
        if self.restores.len() >= MAX_RESTORES {
            tracing::error!(
                %loss,
                "the GPU device was lost again after {MAX_RESTORES} new ones within {} minutes: \
                 no new one replaces it",
                RESTORE_WINDOW.as_secs() / 60
            );
            self.restore_lost_device = false;
            return;
        }
        // Nothing may hold the lost device when the new one opens: Direct3D 12 would not open it on
        // the same adapter (ADR-0075). The application drops its objects, then the run its own.
        self.presentation.gpu_lost(&mut self.host.context);
        let lost = self
            .host
            .context
            .take_gpu()
            .expect("the device that was found lost");
        let surface = self.surface.take().expect("checked above");
        match surface.restore(lost) {
            Ok((gpu, surface)) => {
                self.restores.push(now);
                // An input's wait would hold the new device's opening, which is not a frame's.
                self.inputs.forget();
                // The new device's first frame runs even if the window holds its frames: a frame
                // that the lost device presented may never have reached the compositor, whose
                // signal would then never come.
                self.asked_from_outside = true;
                self.wanted = true;
                tracing::warn!(%loss, "the GPU device was lost; a new one replaces it");
                let host = &mut self.host;
                host.context.set_gpu(Some(gpu), Some(surface.format()));
                self.surface = Some(surface);
                // A loss of the new device that it returns is handled as any other
                // (`Context::exit_with_error`).
                if let Err(error) = self
                    .presentation
                    .gpu_restored(&mut host.context, &host.world)
                {
                    host.context.exit_with_error(error);
                }
            }
            Err(error) => {
                // The run has neither device nor surface left: it stops, with this error.
                tracing::error!(%error, "no new GPU device replaces the lost one");
                self.error.get_or_insert(RunError::Gfx(error));
                self.restore_lost_device = false;
                self.host.context.exit_because(ExitReason::DeviceLost);
            }
        }
    }
}

impl<S: Simulation, P: Presentation> EventHandler for WindowedDriver<'_, S, P> {
    fn window_created(&mut self, window: &Window) {
        self.host.context.set_window(Some(window.clone()));
        self.host.context.waker().set_window(window.waker());
        match self.start(window) {
            Ok(()) => {
                self.started = true;
                let now = Instant::now();
                self.last_frame = now;
                self.next_frame = now;
            }
            Err(error) => {
                self.error = Some(error);
                self.stop = Some("the run failed to start");
            }
        }
    }

    fn event(&mut self, event: Event) {
        if !self.started {
            return;
        }
        if is_input(&event) {
            self.inputs.arrived(Instant::now());
        }
        self.end_rest();
        tracing::trace!(?event, "platform event");
        // Whatever the window tells may change what it shows.
        self.wanted = true;
        self.asked_from_outside = true;
        if let (Event::Resized(size), Some(surface)) = (&event, &mut self.surface) {
            surface.resize(size.width, size.height);
        }
        if let Event::Occluded(occluded) = event {
            self.occluded = occluded;
        }
        // What may change the surface makes a surface given up again at once.
        if matches!(
            event,
            Event::Resized(_)
                | Event::ScaleFactorChanged { .. }
                | Event::Focused(true)
                | Event::Occluded(false)
        ) {
            self.remake.soon(Instant::now());
        }
        // What a screen reader asks reaches the interface alone.
        #[cfg(feature = "accessibility")]
        if let Event::Accessibility(asked) = &event {
            if let Some(window) = self.host.context.window() {
                self.frames
                    .screen_readers
                    .asked(window, self.presentation.ui(), asked);
            }
            return;
        }
        // The interface first; what it takes, neither the presentation nor the game sees. Its
        // time stamped as it came, since winit gives none, so that what it starts, a pointer's
        // rest, starts then, not when the last frame ran, which waiting may have left long ago.
        let time = EventTime::received(self.clock.now());
        let taken_by_interface = match self.presentation.ui() {
            Some(ui) => {
                let taken = self.input.give(ui, &event, time);
                // What the event asked of the window and the clipboard, at once: a paste lands
                // in the step of its key.
                if taken && let Some(window) = self.host.context.window() {
                    self.frames.shown.show(window, Some(ui), time);
                }
                taken
            }
            None => false,
        };
        let taken =
            taken_by_interface || self.presentation.on_event(&mut self.host.context, &event);
        self.host.input_mut().handle(&event, taken);
        if event == Event::CloseRequested {
            self.host.context.exit_because(ExitReason::WindowClosed);
        }
    }

    fn redraw_requested(&mut self) {
        self.end_rest();
        // The compositor's signal came.
        self.checked_hold = false;
        let now = Instant::now();
        self.remake.soon(now);
        if self
            .redraw_asked
            .is_some_and(|asked| now - asked >= SIGNAL_WAIT)
        {
            self.end_hold(now);
        }
        self.frame();
    }

    fn about_to_wait(&mut self) -> LoopControl {
        // The loop left a wait: whether the machine slept meanwhile. The host's clock counted it,
        // so that what the interface waits for is due on time; the loop's waits end late by the
        // sleep's length, on a clock that stops during a suspend on Linux and as relative waits
        // that do not count one on Windows (M6d.7).
        let suspended = self.clock.take_suspended();
        if suspended >= SUSPEND_TOLD {
            tracing::info!(?suspended, "the system was suspended");
        }
        self.end_rest();
        if self.started {
            self.restore();
            self.follow_surface(Instant::now());
        }
        if self.stop.is_none() {
            self.stop = self
                .frames
                .check_stop(&mut self.host.context, self.restore_lost_device);
        }
        if self.stop.is_some() {
            return LoopControl::Exit;
        }
        // A surface given up wakes the loop to be made again.
        let control = self.next_wake();
        sooner(control, self.remake.next)
    }

    fn exiting(&mut self) {
        if self.started {
            let reason = self.stop.unwrap_or("the event loop ended");
            tracing::info!(frames = self.frames.count(), "stopping: {reason}");
            self.presentation.stop(&mut self.host.context);
            self.host.stop(self.simulation);
        }
        // The game's error comes first, before the error of a restore that failed; not before the
        // error of a `start`, which `run` returns rather than any later one.
        if let Some(error) = self.host.context.take_error()
            && !matches!(self.error, Some(RunError::Init(_)))
        {
            self.error = Some(RunError::App(error));
        }
        if let Some(Err(error)) = self.host.context.gpu().map(Gpu::check) {
            self.error.get_or_insert(RunError::Gfx(error));
        }
        // A surface given up is the run's error only when it stopped the run: one that waits to be
        // made again as the run stops for another reason is not.
        if self.host.context.exit_reason() == Some(ExitReason::SurfaceLost)
            && let Some(Err(error)) = self.surface.as_ref().map(Surface::check)
        {
            self.error.get_or_insert(RunError::Gfx(error));
        }
        // The surface keeps the window alive: release it, the GPU and the window while the
        // connection to the windowing system is still open.
        self.surface = None;
        self.host.context.set_gpu(None, None);
        self.host.context.set_window(None);
    }
}

/// The oldest input that no image presented since has followed: the start of its interval of input
/// to present (`docs/adr/0189-first-measures-of-time.md`).
#[derive(Default)]
struct InputWait {
    /// When that input arrived.
    since: Option<Instant>,
    /// Whether the run presented an image yet.
    first_image_presented: bool,
}

/// What an image presented ends.
#[derive(Debug, PartialEq)]
enum Presented {
    /// The run's first image: an input before it waited for the run's start, not for a frame, and
    /// is forgotten.
    First,
    /// The wait of the input that arrived then.
    After(Instant),
    /// No input's wait.
    Alone,
}

impl InputWait {
    /// An input arrived at `now`: it starts a wait, unless an older one waits already.
    fn arrived(&mut self, now: Instant) {
        self.since.get_or_insert(now);
    }

    /// An image was presented: ends the wait.
    fn presented(&mut self) -> Presented {
        let since = self.since.take();
        if !std::mem::replace(&mut self.first_image_presented, true) {
            return Presented::First;
        }
        since.map_or(Presented::Alone, Presented::After)
    }

    /// Forgets the input that waits, whose wait would hold more than frames.
    fn forget(&mut self) {
        self.since = None;
    }
}

/// Whether `event` is a person's input, whose interval to the image after it the trace holds: a
/// key, a button, the pointer, the wheel or the input method; not what the window tells of itself.
fn is_input(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(_)
            | Event::MouseButton { .. }
            | Event::CursorMoved { .. }
            | Event::RawMotion { .. }
            | Event::MouseWheel(_)
            | Event::Ime(_)
    )
}

/// What the run does about a window's surface that `slime-gfx` gave up
/// (`docs/adr/0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md`):
/// it makes it again at the next event that may change it, or after a wait that doubles from
/// `FIRST_REMAKE_WAIT`, never at every frame; and stops the run at the surface given up, or the
/// attempt that failed, that passes `MAX_RESTORES` within `RESTORE_WINDOW`, the device's rule.
#[derive(Debug, Default)]
struct Remake {
    /// When surfaces were given up, or failed to be made again, within `RESTORE_WINDOW`.
    losses: Vec<Instant>,
    /// When the surface given up is made again next; none while the surface shows images.
    next: Option<Instant>,
    /// The wait after the last attempt.
    wait: Duration,
}

impl Remake {
    /// Counts a loss at `now`: whether the run goes on, the budget not spent.
    fn count(&mut self, now: Instant) -> bool {
        self.losses
            .retain(|&lost| now.duration_since(lost) < RESTORE_WINDOW);
        self.losses.push(now);
        self.losses.len() <= MAX_RESTORES
    }

    /// The surface was found given up at `now`: whether the run goes on. The first attempt to make
    /// it again comes `FIRST_REMAKE_WAIT` later.
    fn given_up(&mut self, now: Instant) -> bool {
        self.wait = FIRST_REMAKE_WAIT;
        self.next = Some(now + self.wait);
        self.count(now)
    }

    /// An attempt at `now` failed: whether the run goes on. The next waits twice as long.
    fn failed(&mut self, now: Instant) -> bool {
        self.wait = (self.wait * 2).min(LONGEST_REMAKE_WAIT);
        self.next = Some(now + self.wait);
        self.count(now)
    }

    /// The surface was made again.
    fn made(&mut self) {
        self.next = None;
    }

    /// An event at `now` that may change the surface, the window resized or shown again: the
    /// next attempt comes at once.
    fn soon(&mut self, now: Instant) {
        if let Some(next) = &mut self.next {
            *next = (*next).min(now);
        }
    }

    /// Whether the surface is given up, and waits to be made again.
    fn waiting(&self) -> bool {
        self.next.is_some()
    }

    /// Whether an attempt is due at `now`.
    fn due(&self, now: Instant) -> bool {
        self.next.is_some_and(|next| next <= now)
    }
}

/// `control`, woken at `at` at the latest, unless it ends the loop.
fn sooner(control: LoopControl, at: Option<Instant>) -> LoopControl {
    match (control, at) {
        (LoopControl::Exit, _) | (_, None) => control,
        (LoopControl::Wait, Some(at)) => LoopControl::WaitUntil(at),
        (LoopControl::WaitUntil(due), Some(at)) => LoopControl::WaitUntil(due.min(at)),
    }
}

/// When the frame after one that was due at `due` and started at `started` is due.
///
/// Frames stay on a regular schedule, so that a frame starting a little late does not lower the
/// average rate. The schedule restarts from the frame that just started in two cases: after
/// falling a whole period behind, since catching up would run frames back to back; and after a
/// frame that ran before it was due, because the windowing system asked for it (during a resize)
/// or the application did. Counting such a frame as the scheduled one would push the schedule one
/// period further away at each of them, and the loop would then sleep until the schedule caught
/// up, without running frames or noticing signals.
pub(crate) fn next_due(due: Instant, started: Instant, period: Duration) -> Instant {
    let next = due + period;
    if started < due || next <= started {
        started + period
    } else {
        next
    }
}

/// Whether the interface asks for the next frame after one that `presented` an image or not. What
/// it holds to draw waits for a frame that draws an image: a window that gave none, covered or
/// without an image ready, may not have run the interface's frame at all, as a game's overlay runs
/// it only into an image, and would otherwise ask again at every frame until the window shows. A
/// transition asks either way, as it always did
/// (`docs/adr/0258-each-target-its-frame-and-its-time-a-change-asks-for-its-targets-frame.md`).
fn interface_asks(ui: &uwui::tree::Ui, presented: bool) -> bool {
    if presented {
        ui.needs_frame(TargetId::MAIN)
    } else {
        ui.animating(TargetId::MAIN)
    }
}

/// When a frame runs while the window holds its frames, if something asks for one: once it is due
/// (`next_frame`), and `SIGNAL_WAIT` after `since`, when the loop asked for the redraw that waits
/// for the compositor's signal, or when the last frame ran for a window without area; `None` while
/// nothing asks.
fn held_due(asked: bool, since: Instant, next_frame: Instant) -> Option<Instant> {
    asked.then(|| next_frame.max(since + SIGNAL_WAIT))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD: Duration = Duration::from_millis(16);

    #[test]
    fn the_oldest_input_waits_until_an_image_is_presented() {
        let start = Instant::now();
        let at = |millis| start + Duration::from_millis(millis);
        let mut inputs = InputWait::default();
        // Before the first image, an input waits for the run's start: it is forgotten.
        inputs.arrived(at(0));
        assert_eq!(inputs.presented(), Presented::First);
        assert_eq!(inputs.presented(), Presented::Alone);
        // Of several inputs before an image, the oldest's wait.
        inputs.arrived(at(10));
        inputs.arrived(at(12));
        assert_eq!(inputs.presented(), Presented::After(at(10)));
        assert_eq!(inputs.presented(), Presented::Alone);
        // A wait forgotten, as over a new device's opening, ends with no interval.
        inputs.arrived(at(20));
        inputs.forget();
        assert_eq!(inputs.presented(), Presented::Alone);
    }

    #[test]
    fn a_persons_input_is_an_input_and_the_windows_own_events_are_not() {
        use slime_platform::{ButtonState, MouseButton, PhysicalSize, ScrollDelta};
        let inputs = [
            Event::CursorMoved { x: 1.0, y: 2.0 },
            Event::RawMotion { delta: [1.0, 0.0] },
            Event::MouseButton {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            },
            Event::MouseWheel(ScrollDelta::Lines { x: 0.0, y: 1.0 }),
        ];
        for event in &inputs {
            assert!(is_input(event), "{event:?}");
        }
        let own = [
            Event::CloseRequested,
            Event::Resized(PhysicalSize {
                width: 1,
                height: 1,
            }),
            Event::Focused(true),
            Event::CursorEntered,
            Event::CursorLeft,
        ];
        for event in &own {
            assert!(!is_input(event), "{event:?}");
        }
    }

    #[test]
    fn an_interface_asks_for_a_frame_to_draw_its_changes_only_after_one_that_drew_an_image() {
        use uwui::tree::{Element, LayoutDirection, RootSettings, Ui};
        let mut ui = Ui::new(uwui::text::Fonts::new());
        let settings = RootSettings {
            size: [200.0, 100.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
            target: TargetId::MAIN,
        };
        let root = ui.add_root("main", settings, |s| s);
        ui.frame(TargetId::MAIN);
        // A change that no frame drew: a window that showed no image, covered, leaves it until
        // it shows again, rather than asking at every frame.
        ui.build(root, 1, |children| {
            children.add("waiting", Element::progress(None), |_| {});
        });
        assert!(interface_asks(&ui, true));
        assert!(!interface_asks(&ui, false));
        // A waiting bar drawn moves on its own, and asks either way.
        ui.frame(TargetId::MAIN);
        assert!(interface_asks(&ui, true));
        assert!(interface_asks(&ui, false));
    }

    #[test]
    fn a_surface_given_up_is_made_again_after_waits_that_double_and_an_event_hastens_it() {
        let start = Instant::now();
        let mut remake = Remake::default();
        assert!(!remake.waiting());
        assert!(remake.given_up(start));
        assert!(remake.waiting());
        assert!(!remake.due(start));
        assert!(remake.due(start + FIRST_REMAKE_WAIT));
        // Each attempt that fails waits twice as long, up to the longest wait.
        let mut at = start + FIRST_REMAKE_WAIT;
        let mut waits = Vec::new();
        for _ in 0..6 {
            // Within the budget, whatever it holds.
            remake.losses.clear();
            assert!(remake.failed(at));
            let next = remake.next.expect("waiting");
            waits.push(next - at);
            at = next;
        }
        let seconds = waits.iter().map(Duration::as_secs).collect::<Vec<_>>();
        assert_eq!(seconds, [2, 4, 8, 16, 16, 16]);
        // An event that may change the surface makes the next attempt due at once.
        remake.soon(at);
        assert!(remake.due(at));
        remake.made();
        assert!(!remake.waiting());
        // Nothing to hasten once it shows images.
        remake.soon(at);
        assert!(!remake.waiting());
    }

    #[test]
    fn the_fourth_surface_given_up_within_five_minutes_stops_the_run() {
        let start = Instant::now();
        let mut remake = Remake::default();
        for loss in 0..3 {
            assert!(remake.given_up(start + Duration::from_secs(loss)), "{loss}");
            remake.made();
        }
        assert!(!remake.given_up(start + Duration::from_secs(3)));
        // Losses older than five minutes no longer count; a failed attempt counts as one.
        let later = start + RESTORE_WINDOW + Duration::from_secs(2);
        let mut remake = Remake::default();
        assert!(remake.given_up(start));
        assert!(remake.failed(start + Duration::from_secs(1)));
        assert!(remake.given_up(later));
        assert!(remake.failed(later));
        assert!(remake.failed(later));
        assert!(!remake.failed(later));
    }

    #[test]
    fn the_loop_wakes_to_make_the_surface_again() {
        let now = Instant::now();
        let soon = now + Duration::from_millis(5);
        let late = now + Duration::from_secs(1);
        assert_eq!(sooner(LoopControl::Wait, None), LoopControl::Wait);
        assert_eq!(
            sooner(LoopControl::Wait, Some(soon)),
            LoopControl::WaitUntil(soon)
        );
        assert_eq!(
            sooner(LoopControl::WaitUntil(late), Some(soon)),
            LoopControl::WaitUntil(soon)
        );
        assert_eq!(
            sooner(LoopControl::WaitUntil(soon), Some(late)),
            LoopControl::WaitUntil(soon)
        );
        assert_eq!(sooner(LoopControl::Exit, Some(soon)), LoopControl::Exit);
    }

    #[test]
    fn a_window_that_holds_its_frames_runs_one_only_when_something_asks() {
        let now = Instant::now();
        assert_eq!(held_due(false, now, now), None);
    }

    #[test]
    fn a_frame_asked_while_held_waits_signal_wait_and_until_it_is_due() {
        let since = Instant::now();
        let next_frame = since + PERIOD;
        assert_eq!(held_due(true, since, next_frame), Some(since + SIGNAL_WAIT));
        let late = since + SIGNAL_WAIT * 2;
        assert_eq!(held_due(true, since, late), Some(late));
    }

    #[test]
    fn a_frame_on_time_schedules_the_next_one_period_later() {
        let due = Instant::now();
        assert_eq!(next_due(due, due, PERIOD), due + PERIOD);
    }

    #[test]
    fn a_slightly_late_frame_keeps_the_schedule() {
        let due = Instant::now();
        let started = due + Duration::from_millis(5);
        assert_eq!(next_due(due, started, PERIOD), due + PERIOD);
    }

    #[test]
    fn a_frame_that_ran_before_it_was_due_restarts_the_schedule() {
        let started = Instant::now();
        let due = started + Duration::from_millis(10);
        assert_eq!(next_due(due, started, PERIOD), started + PERIOD);
    }

    #[test]
    fn a_frame_late_by_a_period_or_more_restarts_the_schedule() {
        let due = Instant::now();
        for late in [PERIOD, PERIOD * 3 + Duration::from_millis(2)] {
            let started = due + late;
            assert_eq!(next_due(due, started, PERIOD), started + PERIOD);
        }
    }
}
