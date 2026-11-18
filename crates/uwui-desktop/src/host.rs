//! The host above the platform layer (`uwui-windowing`): the application's run, the pacing of its
//! frames and its device, called back by the layer at each turn of the platform's loop
//! (`docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md`,
//! `docs/adr/0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md`).
//!
//! At each turn, in this order: what came from outside asks a frame even while the window holds
//! its frames (`docs/adr/0187-a-hidden-window-at-rest.md`); a closing asked goes to the
//! application; the platform's signal paces the frames; the window's size and scale reach the
//! device; the person's input reaches the interface, the pointer's moves gathered; the application
//! steps and makes its requests; a device lost is replaced and a surface given up made again,
//! within their budget (`recovery`); a frame is drawn if the pacing says so, the signal asked just
//! before its present. Where a trace is asked (`crate::trace`), each frame and each input to the
//! present of the image after it are written.
//!
//! A panic in any call of the layer to the host, in the application's code, uwui's or the host's,
//! is caught before it leaves the call: the application is called no more and never dropped, the
//! device and the window still let go of, and the process ends with `EXIT_PANICKED`
//! (`docs/adr/0298-a-panic-ends-a-run-without-stop-the-same-rule-in-both-hosts.md`). The run's
//! crash report and marker are `crate::crash`'s.

use std::error::Error;
use std::fmt::{self, Write as _};
use std::panic::{self, AssertUnwindSafe};
use std::time::SystemTime;

use uwui::host::{Event, Input, TargetId, TargetSize};
use uwui::time::{EventTime, Instant};
use uwui::tree::Ui;
use uwui_windowing::{Control, Drawn, Handler, Happened, Opening, Stopped, Window as Platform};

use crate::app::{App, Look, Window, WindowSettings};
use crate::crash::{self, Crashes};
use crate::gpu::{Gpu, Loss};
use crate::pacing::{Next, Pacing};
use crate::recovery::{Budget, MAX_RECOVERIES, Remake, WINDOW};
use crate::session::{Session, StartError, Unwound};
use crate::trace::Trace;
use crate::{EXIT_HOST_FAILED, EXIT_PANICKED, EXIT_START_FAILED};

/// Why the host stopped.
#[derive(Debug)]
struct HostError(String);

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for HostError {}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Device(error) | Self::Run(error) => write!(f, "{error}"),
            Self::Start(error) => write!(f, "the application did not start: {error}"),
            Self::Panicked(message) => write!(f, "a panic: {message}"),
        }
    }
}

/// `what` failed, by `cause`.
fn failed(what: &str, cause: impl fmt::Display) -> HostError {
    HostError(format!("{what}: {cause}"))
}

/// What ended the run, from the host's side.
#[derive(Debug)]
enum Failure {
    /// The window's device could not be made.
    Device(HostError),
    /// The application did not start.
    Start(StartError),
    /// The host failed during the run.
    Run(HostError),
    /// A panic on the host's thread, with its message.
    Panicked(String),
}

/// Runs `app` in a window of `settings`, in `look`, until it ends: the process's exit status.
pub(crate) fn run<A: App>(look: Look, settings: WindowSettings, app: A) -> i32 {
    let version = settings.version.clone();
    let mut window = Window::new(settings);
    // Before the window opens, so that a crash before the first frame is found too.
    let crashes = Crashes::start(window.app_id(), version.as_deref(), SystemTime::now());
    if crashes.is_some() {
        crash::install_hook();
    }
    // The settings' requests are the opening's.
    let requests = window.take_requests();
    let opening = Opening {
        title: window.title().to_owned(),
        app_id: window.app_id().to_owned(),
        size: window.size(),
        min_size: window.min_size(),
        fullscreen: requests.fullscreen == Some(true),
    };
    let host = Host {
        waiting: Some(Unwound::new((look, window, app))),
        session: None,
        crashes,
        gpu: None,
        pacing: Pacing::default(),
        drawn: Drawn {
            pixels: [1, 1],
            scale: 1.0,
        },
        held_moves: Vec::new(),
        devices: Budget::default(),
        remake: Remake::default(),
        trace: None,
        failure: None,
    };
    let (host, ran) = uwui_windowing::run(opening, host);
    let Host {
        waiting,
        session,
        crashes,
        mut failure,
        ..
    } = host;
    // The application's last call, after a failure too, so that it may keep its work: the window
    // is gone, its device perhaps lost. Never after a panic, when the session is gone.
    if let Some(mut session) = session
        && let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| session.stop()))
    {
        failure = Some(Failure::Panicked(crash::message(&*payload)));
        forget_panicked(session, payload);
    }
    match (crashes, &failure) {
        (Some(crashes), Some(Failure::Panicked(message))) => crashes.panicked(message),
        (Some(crashes), _) => crashes.end(),
        (None, _) => {}
    }
    match (failure, ran) {
        (Some(Failure::Device(error)), _) => {
            report("uwui's desktop host cannot start", &error);
            EXIT_HOST_FAILED
        }
        (Some(Failure::Start(error)), _) => {
            let what = match error {
                StartError::Look(_) => "the application's look does not read",
                StartError::App(_) => "the application cannot start",
            };
            // Its `Display` holds the cause alone.
            report(what, &error);
            EXIT_START_FAILED
        }
        (Some(Failure::Run(error)), _) => {
            report("uwui's desktop host stopped", &error);
            EXIT_HOST_FAILED
        }
        (Some(Failure::Panicked(message)), _) => {
            report("uwui's desktop host ended on a panic", &HostError(message));
            EXIT_PANICKED
        }
        // The layer failed before the window opened: the application never started.
        (None, Err(error)) if waiting.is_some() => {
            report("uwui's desktop host cannot start", &error);
            EXIT_HOST_FAILED
        }
        (None, Err(error)) => {
            report("uwui's desktop host stopped", &error);
            EXIT_HOST_FAILED
        }
        (None, Ok(())) => 0,
    }
}

/// Lets go of what a panic left without running any of its code: the application's value, which
/// panicked, is never dropped, nor the panic's payload, whose drop may panic again. The process ends
/// soon after.
fn forget_panicked<T>(value: T, payload: Box<dyn std::any::Any + Send>) {
    std::mem::forget(value);
    std::mem::forget(payload);
}

/// Tells that the trace stopped, `what`, by `error`, in the log and on the standard error, which a
/// tool that waits for the trace reads.
fn tell_trace_stopped(what: &str, error: &std::io::Error) {
    let told = format!("uwui's desktop host: {what}: {error}");
    tracing::warn!("{told}");
    crash::tell(&told);
}

/// Tells `error` and its causes on the standard error, which the application's log may not hold.
fn report(what: &str, error: &dyn Error) {
    let mut told = format!("{what}: {error}");
    let mut cause = error.source();
    while let Some(error) = cause {
        let _ = write!(told, ": {error}");
        cause = error.source();
    }
    tracing::error!("{told}");
    crash::tell(&told);
}

/// The host: the application, before and while it runs, its device and its frames' pacing.
///
/// The application, waiting or running, is held in an [`Unwound`], so that a panic that leaves a
/// call never drops it, one of the layer's own code included.
struct Host<A> {
    /// The look, the window and the application until the window opens.
    waiting: Option<Unwound<(Look, Window, A)>>,
    session: Option<Unwound<Session<A>>>,
    /// The run's crash directory, where the environment gives one.
    crashes: Option<Crashes>,
    /// The window's device, from its opening until it closes.
    gpu: Option<Gpu>,
    pacing: Pacing,
    /// How the window's images are drawn now.
    drawn: Drawn,
    /// Each pointer's last move, given to the interface before the next event or frame.
    held_moves: Vec<(EventTime, uwui_windowing::Input)>,
    /// The devices that replaced lost ones, within the budget's window.
    devices: Budget,
    /// When the surface given up is made again.
    remake: Remake,
    /// The trace that the environment asks for, from the window's opening.
    trace: Option<Trace>,
    failure: Option<Failure>,
}

impl<A: App> Handler for Host<A> {
    fn opened(&mut self, platform: &mut Platform<'_>, drawn: Drawn) -> Result<(), Stopped> {
        self.guarded(|host| host.open(platform, drawn))
            .unwrap_or(Err(Stopped))
    }

    fn turn(&mut self, platform: &mut Platform<'_>, happened: Happened) -> Control {
        self.guarded(|host| match host.turn_of(platform, happened) {
            Ok(control) => control,
            Err(error) => {
                host.failure = Some(Failure::Run(error));
                Control::Exit
            }
        })
        .unwrap_or(Control::Exit)
    }

    fn closing(&mut self) {
        // The device goes before the window's handles, after a panic too.
        let gpu = self.gpu.take();
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| drop(gpu))) {
            self.ended_on_panic(payload);
        }
    }
}

impl<A: App> Host<A> {
    /// What `call` gives, run on the host unless a panic ended the run; none when it panics, which
    /// ends the run: the application, waiting or running, is let go of without a call nor a drop.
    fn guarded<R>(&mut self, call: impl FnOnce(&mut Self) -> R) -> Option<R> {
        if matches!(self.failure, Some(Failure::Panicked(_))) {
            return None;
        }
        match panic::catch_unwind(AssertUnwindSafe(|| call(self))) {
            Ok(given) => Some(given),
            Err(payload) => {
                self.ended_on_panic(payload);
                None
            }
        }
    }

    /// A panic of `payload` ends the run: the application, waiting or running, is let go of
    /// without a call nor a drop. The first panic is the run's failure; a failure before it is
    /// told, since the panic's status replaces its own.
    fn ended_on_panic(&mut self, payload: Box<dyn std::any::Any + Send>) {
        match self.failure.take() {
            Some(Failure::Panicked(first)) => self.failure = Some(Failure::Panicked(first)),
            earlier => {
                if let Some(earlier) = earlier {
                    let told = format!("uwui's desktop host had failed before a panic: {earlier}");
                    tracing::error!("{told}");
                    crash::tell(&told);
                }
                self.failure = Some(Failure::Panicked(crash::message(&*payload)));
            }
        }
        forget_panicked((self.waiting.take(), self.session.take()), payload);
    }

    /// The window opened: its device made, the application started.
    fn open(&mut self, platform: &mut Platform<'_>, drawn: Drawn) -> Result<(), Stopped> {
        if self.waiting.is_none() {
            return Err(Stopped);
        }
        let (display, surface) = platform.raw_handles();
        #[expect(unsafe_code, reason = "the device draws on the window's raw handles")]
        // SAFETY: the layer keeps the handles live until `closing` returns, and drops the host
        // before them otherwise; `closing` drops the device.
        let gpu = unsafe { Gpu::new(display, surface, drawn.pixels) };
        let gpu = match gpu {
            Ok(gpu) => gpu,
            Err(cause) => {
                self.failure = Some(Failure::Device(failed("the window's device", cause)));
                return Err(Stopped);
            }
        };
        if let Some(crashes) = &mut self.crashes {
            crashes.set_gpu(gpu.described());
        }
        self.gpu = Some(gpu);
        self.drawn = drawn;
        let (waker, now) = (platform.waker(), platform.now());
        self.trace = match Trace::from_env(now) {
            Some(Ok(trace)) => Some(trace),
            Some(Err(error)) => {
                tell_trace_stopped("the trace cannot be written", &error);
                None
            }
            None => None,
        };
        let Some(waiting) = self.waiting.take() else {
            return Err(Stopped);
        };
        // The session guards the application as soon as it takes it.
        let (look, window, app) = waiting.into_inner();
        match Session::new(look, window, app, waker, now) {
            Ok(session) => self.session = Some(Unwound::new(session)),
            Err(error) => {
                self.failure = Some(Failure::Start(error));
                return Err(Stopped);
            }
        }
        // Held by the host before the application's first call, so that a panic in it never drops
        // the application.
        if let Some(Err(error)) = self.session.as_mut().map(|session| session.start()) {
            self.session = None;
            self.failure = Some(Failure::Start(error));
            return Err(Stopped);
        }
        // The first frame runs at once, the window's size given.
        self.pacing.host_changed();
        Ok(())
    }

    /// A turn of the loop: the application's run, given what `happened`.
    fn turn_of(
        &mut self,
        platform: &mut Platform<'_>,
        mut happened: Happened,
    ) -> Result<Control, HostError> {
        // Before the signal, so that the frames that it lets run predict from the latest.
        self.presentations(std::mem::take(&mut happened.presentations));
        let (Some(session), Some(gpu)) = (&mut self.session, &mut self.gpu) else {
            return Ok(Control::Exit);
        };
        let Happened {
            inputs,
            configured,
            drawn,
            close_asked,
            terminated,
            woken,
            signal,
            presentations: _,
        } = happened;
        // An event of the window, a wake or a termination signal asks from outside: it runs a
        // frame even while the window holds its frames, once their signal is late (ADR-0187).
        if woken
            || terminated
            || close_asked
            || !inputs.is_empty()
            || configured.is_some()
            || drawn.is_some()
        {
            self.pacing.asked_from_outside();
        }
        if terminated || close_asked {
            session.close_requested();
            if session.ending() {
                return Ok(Control::Exit);
            }
        }
        if let Some(at) = signal {
            self.pacing.signalled(at);
        }
        let now = platform.now();
        // The inputs' own times, which an input that waited behind a long frame keeps.
        if let Some(trace) = &mut self.trace
            && let Some(oldest) = inputs.iter().map(|(time, _)| time.at).min()
        {
            let wheel = inputs
                .iter()
                .filter(|(_, input)| matches!(input, uwui_windowing::Input::Wheel(_)))
                .count();
            trace.input(oldest, u32::try_from(wheel).unwrap_or(u32::MAX));
        }
        if let Some(configured) = configured {
            session
                .window
                .configured(configured.size, configured.fullscreen);
            // The commit that follows the acknowledgement carries the new size.
            self.pacing.host_changed();
            self.remake.soon(now);
        }
        if let Some(drawn) = drawn {
            self.drawn = drawn;
            gpu.resize(drawn.pixels);
            self.pacing.host_changed();
            self.remake.soon(now);
        }
        for input in gather(inputs, &mut self.held_moves, self.pacing.awaiting()) {
            give(session, platform, now, input);
        }
        session.step();
        if session.ending() {
            return Ok(Control::Exit);
        }
        if make_requests(&mut session.window, platform, gpu) {
            self.pacing.host_changed();
        }
        let now = platform.now();
        self.recover(now)?;
        if let Some(remade) = self.remake.next() {
            // The window holds its frames until its surface is made again.
            return Ok(Control::Wait(Some(remade)));
        }
        let Some(session) = &self.session else {
            return Ok(Control::Exit);
        };
        let ui = &session.ui;
        let mut next = self.pacing.next(
            now,
            ui.needs_frame(TargetId::MAIN),
            ui.next_frame_due(TargetId::MAIN),
        );
        if next == Next::Frame {
            self.frame(platform, now)?;
            let Some(session) = &self.session else {
                return Ok(Control::Exit);
            };
            let ui = &session.ui;
            next = self.pacing.next(
                platform.now(),
                ui.needs_frame(TargetId::MAIN),
                ui.next_frame_due(TargetId::MAIN),
            );
        }
        // Inputs that the interface answered with no frame are no input to present; a pointer's
        // move held for the next frame, or a frame owed, still answers them.
        if next != Next::Frame
            && self.held_moves.is_empty()
            && !self.pacing.owes_frame()
            && let (Some(trace), Some(session)) = (&mut self.trace, &self.session)
            && !session.ui.needs_frame(TargetId::MAIN)
        {
            trace.unanswered();
        }
        Ok(Control::Wait(match next {
            Next::WaitUntil(deadline) => Some(deadline),
            Next::Wait => None,
            // A frame just drawn waits for its signal: never two in a turn.
            Next::Frame => Some(platform.now()),
        }))
    }

    /// Replaces a lost device and makes a surface given up again, at `now`, within their budget
    /// (`crate::recovery`): the first frame after either runs even while the window holds its
    /// frames, since the image that the lost device presented may never reach the compositor,
    /// whose signal would then never come.
    ///
    /// # Errors
    ///
    /// When the losses pass their budget, or no device replaces the lost one.
    fn recover(&mut self, now: Instant) -> Result<(), HostError> {
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        if gpu.device_lost() {
            if !self.devices.spend(now) {
                return Err(HostError(format!(
                    "the GPU's device was lost again after {MAX_RECOVERIES} new ones within {} \
                     minutes",
                    WINDOW.as_secs() / 60
                )));
            }
            let Some(lost) = self.gpu.take() else {
                return Ok(());
            };
            let gpu = lost
                .restore()
                .map_err(|cause| failed("no new GPU device replaces the lost one", cause))?;
            tracing::warn!("the GPU's device was lost; a new one replaces it");
            if let Some(crashes) = &mut self.crashes {
                crashes.set_gpu(gpu.described());
            }
            self.gpu = Some(gpu);
            // The new device's surface is new too.
            self.remake.made();
            self.redraw_whole();
            if let Some(trace) = &mut self.trace {
                trace.unanswered();
            }
            return Ok(());
        }
        if !gpu.surface_given_up() {
            return Ok(());
        }
        if self.remake.next().is_none() {
            if !self.remake.given_up(now) {
                return Err(HostError(format!(
                    "the window's surface was given up again after {MAX_RECOVERIES} within {} \
                     minutes",
                    WINDOW.as_secs() / 60
                )));
            }
            tracing::warn!("the window holds its frames until its surface is made again");
        }
        if !self.remake.due(now) {
            return Ok(());
        }
        match gpu.make_surface_again() {
            Ok(()) => {
                self.remake.made();
                self.redraw_whole();
                if let Some(trace) = &mut self.trace {
                    trace.unanswered();
                }
            }
            Err(cause) => {
                tracing::warn!(%cause, "the window's surface cannot be made again yet");
                if !self.remake.failed(now) {
                    return Err(failed("the window's surface cannot be made again", cause));
                }
            }
        }
        Ok(())
    }

    /// The images whose presentation the platform told: to the pacing, which predicts the next
    /// frames' times from them, and to the trace, with the frames that missed their refresh.
    fn presentations(&mut self, presentations: Vec<uwui_windowing::Presentation>) {
        for presentation in presentations {
            let missed =
                self.pacing
                    .presented(presentation.image, presentation.shown, presentation.refresh);
            if let Some(missed) = missed {
                tracing::debug!(
                    image = presentation.image,
                    late = ?missed.shown.saturating_duration_since(missed.given),
                    "a frame missed the refresh that it was drawn for"
                );
            }
            let Some(trace) = &mut self.trace else {
                continue;
            };
            let written = match (presentation.shown, missed) {
                (Some(shown), missed) => trace.shown(presentation.image, shown).and_then(|()| {
                    missed.map_or(Ok(()), |missed| {
                        trace.missed(presentation.image, missed.given, missed.shown)
                    })
                }),
                (None, _) => Ok(()),
            };
            if let Err(error) = written {
                tell_trace_stopped("the trace stops: its file refuses its writes", &error);
                self.trace = None;
            }
        }
    }

    /// The window's next image is drawn whole, even while the window holds its frames, once the
    /// signal of the image before is late if it has not come.
    fn redraw_whole(&mut self) {
        self.pacing.asked_from_outside();
        self.pacing.host_changed();
    }

    /// Draws a frame, started at `now`, and presents it with the platform's signal asked.
    fn frame(&mut self, platform: &mut Platform<'_>, now: Instant) -> Result<(), HostError> {
        let (Some(session), Some(gpu)) = (&mut self.session, &mut self.gpu) else {
            return Ok(());
        };
        let image = match gpu.acquire() {
            Ok(image) => image,
            // Replaced or made again before the next frame, which the interface's changes wait for.
            Err(Loss::Device | Loss::Surface) => {
                self.pacing.host_changed();
                return Ok(());
            }
        };
        let Some(image) = image else {
            // No image now: tried again a signal's wait later, rather than at once.
            self.pacing.missed(now);
            return Ok(());
        };
        for held in std::mem::take(&mut self.held_moves) {
            give(session, platform, now, held);
        }
        let ui = &mut session.ui;
        // Never refused: each side is one pixel at least, and the scale above zero.
        let drawn = self.drawn;
        if let Ok(size) = TargetSize::new(drawn.pixels, drawn.scale as f32) {
            ui.set_target_size(TargetId::MAIN, size);
        }
        ui.set_frame_shown(TargetId::MAIN, self.pacing.shown(now));
        let (list, _) = ui.frame(TargetId::MAIN);
        serve_outputs(ui, platform, now);
        let (drawn, result) = gpu.draw(image, ui.fonts(), &list);
        if gpu.device_lost() {
            // Replaced before the next frame, which draws the interface whole.
            gpu.discard(drawn);
            self.pacing.host_changed();
            return Ok(());
        }
        if let Err(error) = result {
            // The window keeps its last image whole rather than show this one, and draws again
            // later: the frame's changes are spent.
            tracing::warn!(%error, "a frame drawn in part is not presented");
            gpu.discard(drawn);
            self.pacing.not_presented(now);
            return Ok(());
        }
        // Asked before the present, whose commit carries it.
        let image = platform.ask_signal();
        gpu.present(drawn);
        let ready = platform.now();
        self.pacing.drew(now, ready, image);
        if let Some(trace) = &mut self.trace
            && let Err(error) = trace.presented(now, ready, image)
        {
            tell_trace_stopped("the trace stops: its file refuses its writes", &error);
            self.trace = None;
        }
        Ok(())
    }
}

/// Makes the requests of the window that the application made: whether an image must follow to
/// apply one, as the platform says. The losses asked on purpose are made on `gpu`: the surface's
/// at once, the device's in the next frame's draw.
fn make_requests(window: &mut Window, platform: &mut Platform<'_>, gpu: &mut Gpu) -> bool {
    let requests = window.take_requests();
    // A device is lost in a frame's draw, which must follow.
    let mut image = requests.lose_device;
    if requests.lose_device {
        gpu.lose_device_at_next_draw();
    }
    if requests.lose_surface {
        gpu.give_up_surface("the application gave it up on purpose");
    }
    if requests.title {
        platform.set_title(window.title());
    }
    if requests.min_size {
        image |= platform.set_min_size(window.min_size());
    }
    if let Some(fullscreen) = requests.fullscreen {
        platform.set_fullscreen(fullscreen);
    }
    if let Some(activation) = requests.activation {
        platform.activate(activation);
    }
    image
}

/// The person's input in the order that the interface takes it: the pointers' moves gathered,
/// each pointer's last move held in `held`, in the order that the pointers first moved, and given
/// only before the event that follows them; held while the frame `awaits` its signal, to be given
/// before the next event or frame.
fn gather(
    inputs: Vec<(EventTime, uwui_windowing::Input)>,
    held: &mut Vec<(EventTime, uwui_windowing::Input)>,
    awaits: bool,
) -> Vec<(EventTime, uwui_windowing::Input)> {
    let mut given = Vec::with_capacity(inputs.len() + held.len());
    for (time, input) in inputs {
        if let uwui_windowing::Input::PointerMoved { pointer, .. } = input {
            let same = held.iter_mut().find(|(_, before)| {
                matches!(before, uwui_windowing::Input::PointerMoved { pointer: other, .. }
                    if other.id == pointer.id)
            });
            match same {
                Some(before) => *before = (time, input),
                None => held.push((time, input)),
            }
            continue;
        }
        given.append(held);
        given.push((time, input));
    }
    if !awaits {
        given.append(held);
    }
    given
}

/// The interface's event for the layer's `input`; none for an input that the host does not know
/// yet, which it leaves.
fn translated(input: &uwui_windowing::Input) -> Option<Input<'_>> {
    use uwui_windowing::Input as Given;
    Some(match input {
        &Given::PointerMoved { pointer, x, y } => Input::PointerMoved { pointer, x, y },
        &Given::PointerLeft { pointer } => Input::PointerLeft { pointer },
        &Given::PointerButton {
            pointer,
            button,
            state,
        } => Input::PointerButton {
            pointer,
            button,
            state,
        },
        &Given::Wheel(delta) => Input::Wheel(delta),
        Given::Key(key) => Input::Key(key),
        &Given::Modifiers(modifiers) => Input::Modifiers(modifiers),
        &Given::Focus(given) => Input::Focus(given),
        _ => return None,
    })
}

/// Gives the interface one event on the window, then, if it took it, serves what it asks; `now`
/// is the time of what it serves.
fn give<A: App>(
    session: &mut Session<A>,
    platform: &mut Platform<'_>,
    now: Instant,
    (time, input): (EventTime, uwui_windowing::Input),
) {
    let Some(translated) = translated(&input) else {
        return;
    };
    let ui = &mut session.ui;
    let answer = ui.event(Event::new(TargetId::MAIN, time, translated));
    if answer.taken() {
        serve_outputs(ui, platform, now);
    }
}

/// Reads the window's outputs and serves them at `now`: the cursor's shape; the clipboard's reads
/// answered with no text until the host has a clipboard, and the input method waiting for its own
/// (M6d.8).
fn serve_outputs(ui: &mut Ui, platform: &mut Platform<'_>, now: Instant) {
    let outputs = ui.outputs(TargetId::MAIN);
    if let Some(shape) = outputs.cursor {
        platform.set_cursor(shape);
    }
    for read in outputs.clipboard_reads {
        let input = Input::ClipboardText { read, text: None };
        let _ = ui.event(Event::new(TargetId::MAIN, EventTime::received(now), input));
    }
}

#[cfg(test)]
mod tests {
    use slime_input::Pointer;
    use uwui_windowing::Input as Given;

    use super::*;

    fn moved(at: u64, x: f32) -> (EventTime, Given) {
        (
            EventTime::received(Instant::from_nanos(at)),
            Given::PointerMoved {
                pointer: Pointer::MOUSE,
                x,
                y: 0.0,
            },
        )
    }

    fn told(inputs: &[(EventTime, Given)]) -> Vec<String> {
        inputs
            .iter()
            .map(|(time, input)| format!("{} {:?}", time.at.as_nanos(), translated(input).unwrap()))
            .collect()
    }

    #[test]
    fn the_pointers_moves_are_gathered_and_the_last_held_while_a_frame_waits() {
        let left = (
            EventTime::received(Instant::from_nanos(3)),
            Given::PointerLeft {
                pointer: Pointer::MOUSE,
            },
        );
        let mut held = Vec::new();
        // Two moves then the pointer leaving: the last move before it, the first dropped.
        let given = gather(vec![moved(1, 1.0), moved(2, 2.0), left], &mut held, true);
        assert_eq!(
            told(&given),
            [
                "2 PointerMoved { pointer: Pointer { id: PointerId(0), device: Mouse }, x: 2.0, y: 0.0 }",
                "3 PointerLeft { pointer: Pointer { id: PointerId(0), device: Mouse } }"
            ]
        );
        // While a frame waits, the last move is held, then replaced by a later one.
        assert!(gather(vec![moved(4, 4.0)], &mut held, true).is_empty());
        assert!(gather(vec![moved(5, 5.0)], &mut held, true).is_empty());
        // Once nothing waits, the held move is given.
        let given = gather(Vec::new(), &mut held, false);
        assert_eq!(
            told(&given),
            [
                "5 PointerMoved { pointer: Pointer { id: PointerId(0), device: Mouse }, x: 5.0, y: 0.0 }"
            ]
        );
        assert!(held.is_empty());
    }

    #[test]
    fn each_pointers_last_move_is_held_apart() {
        let finger = |id, at: u64, x| {
            (
                EventTime::received(Instant::from_nanos(at)),
                Given::PointerMoved {
                    pointer: Pointer::finger(slime_input::PointerId(id)),
                    x,
                    y: 0.0,
                },
            )
        };
        let mut held = Vec::new();
        // One frame of touch: both fingers move, then the mouse; each keeps its last move.
        let given = gather(
            vec![
                finger(1, 1, 1.0),
                finger(2, 2, 50.0),
                finger(1, 3, 2.0),
                moved(4, 9.0),
            ],
            &mut held,
            true,
        );
        assert!(given.is_empty());
        let given = gather(Vec::new(), &mut held, false);
        assert_eq!(
            told(&given),
            [
                "3 PointerMoved { pointer: Pointer { id: PointerId(1), device: Finger }, x: 2.0, y: 0.0 }",
                "2 PointerMoved { pointer: Pointer { id: PointerId(2), device: Finger }, x: 50.0, y: 0.0 }",
                "4 PointerMoved { pointer: Pointer { id: PointerId(0), device: Mouse }, x: 9.0, y: 0.0 }",
            ]
        );
        assert!(held.is_empty());
    }
}
