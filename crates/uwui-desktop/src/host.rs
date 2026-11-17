//! The host above the platform layer (`uwui-windowing`): the application's run, the pacing of its
//! frames and its device, called back by the layer at each turn of the platform's loop
//! (`docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md`,
//! `docs/adr/0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md`).
//!
//! At each turn, in this order: what came from outside asks a frame even while the window holds
//! its frames (`docs/adr/0187-a-hidden-window-at-rest.md`); a closing asked goes to the
//! application; the platform's signal paces the frames; the window's size and scale reach the
//! device; the person's input reaches the interface, the pointer's moves gathered; the application
//! steps and makes its requests; a frame is drawn if the pacing says so, the signal asked just
//! before its present.

use std::error::Error;
use std::fmt::{self, Write as _};

use uwui::host::{Event, Input, TargetId, TargetSize};
use uwui::time::{EventTime, Instant};
use uwui::tree::Ui;
use uwui_windowing::{Control, Drawn, Handler, Happened, Opening, Stopped, Window as Platform};

use crate::app::{App, Look, Window, WindowSettings};
use crate::gpu::Gpu;
use crate::pacing::{Next, Pacing};
use crate::session::{Session, StartError};
use crate::{EXIT_HOST_FAILED, EXIT_START_FAILED};

/// Why the host stopped.
#[derive(Debug)]
struct HostError(String);

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for HostError {}

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
}

/// Runs `app` in a window of `settings`, in `look`, until it ends: the process's exit status.
pub(crate) fn run<A: App>(look: Look, settings: WindowSettings, app: A) -> i32 {
    let mut window = Window::new(settings);
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
        waiting: Some((look, window, app)),
        session: None,
        gpu: None,
        pacing: Pacing::default(),
        drawn: Drawn {
            pixels: [1, 1],
            scale: 1.0,
        },
        held_move: None,
        failure: None,
    };
    let (host, ran) = uwui_windowing::run(opening, host);
    let Host {
        waiting,
        session,
        failure,
        ..
    } = host;
    // The application's last call, after a failure too, so that it may keep its work: the window
    // is gone, its device perhaps lost.
    if let Some(session) = session {
        session.stop();
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

/// Tells `error` and its causes on the standard error, which the application's log may not hold.
fn report(what: &str, error: &dyn Error) {
    let mut told = format!("{what}: {error}");
    let mut cause = error.source();
    while let Some(error) = cause {
        let _ = write!(told, ": {error}");
        cause = error.source();
    }
    tracing::error!("{told}");
    eprintln!("{told}");
}

/// The host: the application, before and while it runs, its device and its frames' pacing.
struct Host<A> {
    /// The look, the window and the application until the window opens.
    waiting: Option<(Look, Window, A)>,
    session: Option<Session<A>>,
    /// The window's device, from its opening until it closes.
    gpu: Option<Gpu>,
    pacing: Pacing,
    /// How the window's images are drawn now.
    drawn: Drawn,
    /// The pointer's last move, given to the interface before the next event or frame.
    held_move: Option<(EventTime, uwui_windowing::Input)>,
    failure: Option<Failure>,
}

impl<A: App> Handler for Host<A> {
    fn opened(&mut self, platform: &mut Platform<'_>, drawn: Drawn) -> Result<(), Stopped> {
        let Some((look, window, app)) = self.waiting.take() else {
            return Err(Stopped);
        };
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
        self.gpu = Some(gpu);
        self.drawn = drawn;
        match Session::start(look, window, app, platform.waker(), platform.now()) {
            Ok(session) => self.session = Some(session),
            Err(error) => {
                self.failure = Some(Failure::Start(error));
                return Err(Stopped);
            }
        }
        // The first frame runs at once, the window's size given.
        self.pacing.host_changed();
        Ok(())
    }

    fn turn(&mut self, platform: &mut Platform<'_>, happened: Happened) -> Control {
        match self.turn_of(platform, happened) {
            Ok(control) => control,
            Err(error) => {
                self.failure = Some(Failure::Run(error));
                Control::Exit
            }
        }
    }

    fn closing(&mut self) {
        self.gpu = None;
    }
}

impl<A: App> Host<A> {
    /// A turn of the loop: the application's run, given what `happened`.
    fn turn_of(
        &mut self,
        platform: &mut Platform<'_>,
        happened: Happened,
    ) -> Result<Control, HostError> {
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
        if let Some(configured) = configured {
            session
                .window
                .configured(configured.size, configured.fullscreen);
            // The commit that follows the acknowledgement carries the new size.
            self.pacing.host_changed();
        }
        if let Some(drawn) = drawn {
            self.drawn = drawn;
            gpu.resize(drawn.pixels);
            self.pacing.host_changed();
        }
        let now = platform.now();
        for input in gather(inputs, &mut self.held_move, self.pacing.awaiting()) {
            give(session, platform, now, input);
        }
        session.step();
        if session.ending() {
            return Ok(Control::Exit);
        }
        if make_requests(&mut session.window, platform) {
            self.pacing.host_changed();
        }
        let now = platform.now();
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
        Ok(Control::Wait(match next {
            Next::WaitUntil(deadline) => Some(deadline),
            Next::Wait => None,
            // A frame just drawn waits for its signal: never two in a turn.
            Next::Frame => Some(platform.now()),
        }))
    }

    /// Draws a frame, started at `now`, and presents it with the platform's signal asked.
    fn frame(&mut self, platform: &mut Platform<'_>, now: Instant) -> Result<(), HostError> {
        let (Some(session), Some(gpu)) = (&mut self.session, &mut self.gpu) else {
            return Ok(());
        };
        let image = gpu
            .acquire()
            .map_err(|cause| failed("the window's image", cause))?;
        let Some(image) = image else {
            // No image now: tried again a signal's wait later, rather than at once.
            self.pacing.missed(now);
            return Ok(());
        };
        if let Some(held) = self.held_move.take() {
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
        if let Err(error) = result {
            tracing::warn!(%error, "a frame is drawn in part");
        }
        // Asked before the present, whose commit carries it.
        platform.ask_signal();
        gpu.present(drawn);
        self.pacing.drew(now);
        Ok(())
    }
}

/// Makes the requests of the window that the application made: whether a commit must carry one.
fn make_requests(window: &mut Window, platform: &mut Platform<'_>) -> bool {
    let requests = window.take_requests();
    if requests.title {
        platform.set_title(window.title());
    }
    if requests.min_size {
        platform.set_min_size(window.min_size());
    }
    if let Some(fullscreen) = requests.fullscreen {
        platform.set_fullscreen(fullscreen);
    }
    if let Some(activation) = requests.activation {
        platform.activate(activation);
    }
    // The minimum size is the surface's state, which the next commit applies.
    requests.min_size
}

/// The person's input in the order that the interface takes it: the pointer's moves gathered,
/// each move given only before the event that follows it, and the last one held in `held` while
/// the frame `awaits` its signal, to be given before the next event or frame.
fn gather(
    inputs: Vec<(EventTime, uwui_windowing::Input)>,
    held: &mut Option<(EventTime, uwui_windowing::Input)>,
    awaits: bool,
) -> Vec<(EventTime, uwui_windowing::Input)> {
    let mut given = Vec::with_capacity(inputs.len() + 1);
    for (time, input) in inputs {
        if let uwui_windowing::Input::PointerMoved { .. } = input {
            *held = Some((time, input));
            continue;
        }
        given.extend(held.take());
        given.push((time, input));
    }
    if !awaits {
        given.extend(held.take());
    }
    given
}

/// The interface's event for the layer's `input`.
fn translated(input: &uwui_windowing::Input) -> Input<'_> {
    use uwui_windowing::Input as Given;
    match input {
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
    }
}

/// Gives the interface one event on the window, then, if it took it, serves what it asks; `now`
/// is the time of what it serves.
fn give<A: App>(
    session: &mut Session<A>,
    platform: &mut Platform<'_>,
    now: Instant,
    (time, input): (EventTime, uwui_windowing::Input),
) {
    let ui = &mut session.ui;
    let answer = ui.event(Event::new(TargetId::MAIN, time, translated(&input)));
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
            .map(|(time, input)| format!("{} {:?}", time.at.as_nanos(), translated(input)))
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
        let mut held = None;
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
        assert!(held.is_none());
    }
}
