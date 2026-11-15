//! The host's Wayland layer, on `wayland-client` itself
//! (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`).
//!
//! It connects to the compositor that `WAYLAND_DISPLAY` names, through libwayland's own display,
//! loaded at run time, which Vulkan's driver shares: the driver commits the surface as it presents
//! each image. The host orders its own requests around those commits: the frame callback is asked
//! just before each present, so that it rides on the commit that shows the image.
//!
//! - *The window*: an xdg-shell toplevel, its title, its app id, its minimum size, filling the
//!   screen or not, its closing told to the application rather than done.
//! - *Its bar*: asked of the compositor through xdg-decoration, which Weston and Mutter do not
//!   give: there the window has none yet.
//! - *Its frames*: on the compositor's frame callback (`crate::pacing`).
//! - *The pointer*: its moves, buttons and wheel, each with the compositor's time anchored on the
//!   host's clock (`crate::anchor`); entering and leaving with their reception's.
//!
//! What waits for its milestone: the keyboard through libxkbcommon, the cursor's shapes, the
//! fractional scale, activation and the presentations' times (M6d.7a2). The window draws at scale
//! 1, which a compositor at another scale stretches.

mod protocol;
mod wait;

use std::error::Error;
use std::fmt::{self, Write as _};
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::Duration;

use uwui::host::{Event, Input, TargetId, TargetSize};
use uwui::time::EventTime;
use wayland_client::globals::{GlobalList, registry_queue_init};
use wayland_client::protocol::{wl_compositor, wl_seat, wl_surface};
use wayland_client::{Connection, EventQueue, Proxy, QueueHandle};
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::app::{App, Look, Window, WindowSettings};
use crate::clock::{self, Suspends};
use crate::gpu::Gpu;
use crate::pacing::{Next, Pacing};
use crate::session::{Session, StartError};
use crate::wake::{self, Asks};
use crate::{EXIT_HOST_FAILED, EXIT_START_FAILED};

use protocol::Told;
use wait::Waits;

/// The shortest time suspended after which the events' anchor is made again: what parts the two
/// clocks while the machine is awake is far below it.
const RESUMED: Duration = Duration::from_secs(1);

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

/// Runs `app` in a window of `settings` on Wayland, in `look`, until it ends: the process's exit
/// status.
pub(crate) fn run<A: App>(look: Look, settings: WindowSettings, app: A) -> i32 {
    let mut window = Window::new(settings);
    let host = match Host::open(&mut window) {
        Ok(host) => host,
        Err(error) => {
            report("uwui's desktop host cannot start", &error);
            return EXIT_HOST_FAILED;
        }
    };
    let session = match Session::start(look, window, app, host.asks.waker(), clock::now()) {
        Ok(session) => session,
        Err(error) => {
            let what = match error {
                StartError::Look(_) => "the application's look does not read",
                StartError::App(_) => "the application cannot start",
            };
            // Its `Display` holds the cause alone.
            report(what, &error);
            return EXIT_START_FAILED;
        }
    };
    let mut host = host;
    let mut session = session;
    let ran = host.run(&mut session);
    // The application's last call, after a failure too, so that it may keep its work: the window
    // may be gone, its device lost.
    session.stop();
    match ran {
        Ok(()) => 0,
        Err(error) => {
            report("uwui's desktop host stopped", &error);
            EXIT_HOST_FAILED
        }
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

/// The window's objects of the protocol.
struct Objects {
    surface: wl_surface::WlSurface,
    xdg_surface: xdg_surface::XdgSurface,
    toplevel: xdg_toplevel::XdgToplevel,
    decoration: Option<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1>,
    // Kept so that their objects live as long as the window's.
    _compositor: wl_compositor::WlCompositor,
    _base: xdg_wm_base::XdgWmBase,
    _seat: Option<wl_seat::WlSeat>,
}

impl Drop for Objects {
    fn drop(&mut self) {
        // The decoration goes before its toplevel, as xdg-decoration asks.
        if let Some(decoration) = self.decoration.take() {
            decoration.destroy();
        }
        self.toplevel.destroy();
        self.xdg_surface.destroy();
        self.surface.destroy();
    }
}

/// The host on Wayland: its connection, the window, its device and its loop's state.
struct Host {
    // Dropped first: the device's surface stands on the window's.
    gpu: Gpu,
    objects: Objects,
    told: Told,
    queue: EventQueue<Told>,
    handle: QueueHandle<Told>,
    connection: Connection,
    waits: Waits,
    asks: Arc<Asks>,
    suspends: Suspends,
    pacing: Pacing,
    /// The window's size in physical pixels.
    size: [u32; 2],
    /// The pointer's last move, given to the interface before the next event or frame.
    held_move: Option<(EventTime, Input<'static>)>,
}

impl Host {
    /// Connects to the compositor, opens `window` and its device, and sets the termination's
    /// handler.
    fn open(window: &mut Window) -> Result<Self, HostError> {
        let waits = Waits::new().map_err(|cause| failed("the loop's waits", cause))?;
        let asks = Asks::new(waits.notifier());
        let connection = Connection::connect_to_env()
            .map_err(|cause| failed("no Wayland compositor answers", cause))?;
        let (globals, mut queue) = registry_queue_init::<Told>(&connection)
            .map_err(|cause| failed("the compositor's globals", cause))?;
        let handle = queue.handle();
        let mut told = Told::new();
        let objects = Self::window(&globals, &handle, window)?;
        // The first configure comes after the first commit, without an image.
        objects.surface.commit();
        while told.configure.is_none() {
            queue
                .blocking_dispatch(&mut told)
                .map_err(|cause| failed("the compositor's first configure", cause))?;
        }
        let size = told
            .configure
            .and_then(|configure| configure.size)
            .unwrap_or(window.size());
        let display = NonNull::new(connection.backend().display_ptr().cast());
        let surface = NonNull::new(objects.surface.id().as_ptr().cast());
        let (Some(display), Some(surface)) = (display, surface) else {
            return Err(HostError("the window's surface has no object".to_owned()));
        };
        #[expect(unsafe_code, reason = "the device draws on the window's raw surface")]
        // SAFETY: the display is the connection's, and the surface the window's, both held by the
        // host, which drops its device first.
        let gpu = unsafe { Gpu::on_wayland(display, surface, size) }
            .map_err(|cause| failed("the window's device", cause))?;
        wake::on_termination(&asks);
        Ok(Self {
            gpu,
            objects,
            told,
            queue,
            handle,
            connection,
            waits,
            asks,
            suspends: Suspends::new(),
            pacing: Pacing::default(),
            size,
            held_move: None,
        })
    }

    /// Makes the window of `window`'s settings, and asks for its bar.
    fn window(
        globals: &GlobalList,
        handle: &QueueHandle<Told>,
        window: &mut Window,
    ) -> Result<Objects, HostError> {
        let compositor: wl_compositor::WlCompositor = globals
            .bind(handle, 1..=4, ())
            .map_err(|cause| failed("the compositor gives no wl_compositor", cause))?;
        let base: xdg_wm_base::XdgWmBase = globals
            .bind(handle, 1..=6, ())
            .map_err(|cause| failed("the compositor gives no xdg_wm_base", cause))?;
        let seat: Option<wl_seat::WlSeat> = globals
            .bind(handle, 5..=7, ())
            .inspect_err(|error| tracing::warn!(%error, "no seat: the window takes no input"))
            .ok();
        let decorations: Option<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1> =
            globals.bind(handle, 1..=1, ()).ok();
        let surface = compositor.create_surface(handle, ());
        let xdg_surface = base.get_xdg_surface(&surface, handle, ());
        let toplevel = xdg_surface.get_toplevel(handle, ());
        toplevel.set_title(window.title().to_owned());
        toplevel.set_app_id(window.app_id().to_owned());
        let [width, height] = window.min_size().unwrap_or([0, 0]);
        toplevel.set_min_size(side(width), side(height));
        let requests = window.take_requests();
        if requests.fullscreen == Some(true) {
            toplevel.set_fullscreen(None);
        }
        let decoration = decorations.as_ref().map(|manager| {
            let decoration = manager.get_toplevel_decoration(&toplevel, handle, ());
            decoration.set_mode(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            decoration
        });
        if decoration.is_none() {
            tracing::info!("the compositor gives no xdg-decoration: the window has no bar");
        }
        Ok(Objects {
            surface,
            xdg_surface,
            toplevel,
            decoration,
            _compositor: compositor,
            _base: base,
            _seat: seat,
        })
    }

    /// The loop, until the application ends or the host fails.
    fn run<A: App>(&mut self, session: &mut Session<A>) -> Result<(), HostError> {
        // The first frame runs at once, the window's size given.
        self.pacing.host_changed();
        loop {
            // Before the events are timed: their anchor is made again after a suspend.
            if self.suspends.take() >= RESUMED {
                self.told.resumed();
            }
            self.queue
                .dispatch_pending(&mut self.told)
                .map_err(|cause| failed("the connection to the compositor", cause))?;
            let terminated = self.asks.take_terminated();
            let closed = std::mem::take(&mut self.told.close);
            // An event of the window, a wake or a termination signal asks from outside: it runs a
            // frame even while the window holds its frames, once their signal is late (ADR-0187).
            let woken = self.asks.take_woken();
            if woken
                || terminated
                || closed
                || !self.told.inputs.is_empty()
                || self.told.configure.is_some()
            {
                self.pacing.asked_from_outside();
            }
            if terminated || closed {
                session.close_requested();
                if session.ending() {
                    break;
                }
            }
            if let Some(at) = self.told.signal.take() {
                self.pacing.signalled(at);
            }
            if let Some(configure) = self.told.configure.take() {
                self.objects.xdg_surface.ack_configure(configure.serial);
                let size = configure.size.unwrap_or(session.window.size());
                session.window.configured(size, configure.fullscreen);
                // Scale 1 until the fractional scale (M6d.7a2).
                self.size = size;
                self.gpu.resize(size);
                // The commit that follows the acknowledgement carries the new size.
                self.pacing.host_changed();
            }
            self.give_inputs(session);
            session.step();
            if session.ending() {
                break;
            }
            if self.make_requests(&mut session.window) {
                self.pacing.host_changed();
            }
            let now = clock::now();
            let ui = &session.ui;
            let mut next = self.pacing.next(
                now,
                ui.needs_frame(TargetId::MAIN),
                ui.next_frame_due(TargetId::MAIN),
            );
            if next == Next::Frame {
                self.frame(session, now)?;
                let ui = &session.ui;
                next = self.pacing.next(
                    clock::now(),
                    ui.needs_frame(TargetId::MAIN),
                    ui.next_frame_due(TargetId::MAIN),
                );
            }
            let deadline = match next {
                Next::WaitUntil(deadline) => Some(deadline),
                Next::Wait => None,
                // A frame just drawn waits for its signal: never two in a turn.
                Next::Frame => Some(clock::now()),
            };
            self.wait(deadline)?;
        }
        Ok(())
    }

    /// Gives the interface the person's input that came, in order; the pointer's last move held
    /// while the frame waits for its signal, as uwui asks the moves of a frame gathered.
    fn give_inputs<A: App>(&mut self, session: &mut Session<A>) {
        let inputs = std::mem::take(&mut self.told.inputs);
        for input in gather(inputs, &mut self.held_move, self.pacing.awaiting()) {
            give(session, input);
        }
    }

    /// Makes the requests of the window that the application made: whether a commit must carry
    /// one.
    fn make_requests(&mut self, window: &mut Window) -> bool {
        let requests = window.take_requests();
        let toplevel = &self.objects.toplevel;
        if requests.title {
            toplevel.set_title(window.title().to_owned());
        }
        if requests.min_size {
            let [width, height] = window.min_size().unwrap_or([0, 0]);
            toplevel.set_min_size(side(width), side(height));
        }
        match requests.fullscreen {
            Some(true) => toplevel.set_fullscreen(None),
            Some(false) => toplevel.unset_fullscreen(),
            None => {}
        }
        // The minimum size is the surface's state, which the next commit applies.
        requests.min_size
    }

    /// Draws a frame, started at `now`, and presents it with the compositor's signal asked.
    fn frame<A: App>(
        &mut self,
        session: &mut Session<A>,
        now: uwui::time::Instant,
    ) -> Result<(), HostError> {
        let image = self
            .gpu
            .acquire()
            .map_err(|cause| failed("the window's image", cause))?;
        let Some(image) = image else {
            // No image now: tried again a signal's wait later, rather than at once.
            self.pacing.missed(now);
            return Ok(());
        };
        if let Some(held) = self.held_move.take() {
            give(session, held);
        }
        let ui = &mut session.ui;
        // Never refused: each side is one pixel at least.
        if let Ok(size) = TargetSize::new(self.size.map(|side| side.max(1)), 1.0) {
            ui.set_target_size(TargetId::MAIN, size);
        }
        ui.set_frame_shown(TargetId::MAIN, self.pacing.shown(now));
        let (list, _) = ui.frame(TargetId::MAIN);
        serve_outputs(ui);
        let (drawn, result) = self.gpu.draw(image, ui.fonts(), &list);
        if let Err(error) = result {
            tracing::warn!(%error, "a frame is drawn in part");
        }
        // Asked before the present, whose commit carries it.
        self.objects.surface.frame(&self.handle, ());
        self.gpu.present(drawn);
        self.pacing.drew(now);
        Ok(())
    }

    /// Waits for the compositor, a wake or `deadline`, then reads what the compositor sent.
    fn wait(&mut self, deadline: Option<uwui::time::Instant>) -> Result<(), HostError> {
        // Events already read, by Vulkan's driver among others, are dispatched first.
        let Some(guard) = self.queue.prepare_read() else {
            return Ok(());
        };
        // A full socket: the wait ends too once it can be written to, and the next turn sends
        // what waits.
        let full = match self.connection.flush() {
            Ok(()) => false,
            Err(wayland_client::backend::WaylandError::Io(error))
                if error.kind() == std::io::ErrorKind::WouldBlock =>
            {
                true
            }
            Err(cause) => return Err(failed("the connection to the compositor", cause)),
        };
        let readable = self
            .waits
            .wait(guard.connection_fd(), deadline, full)
            .map_err(|cause| failed("the loop's wait", cause))?;
        if readable {
            match guard.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(error))
                    if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(cause) => return Err(failed("the connection to the compositor", cause)),
            }
        }
        Ok(())
    }
}

/// The person's input in the order that the interface takes it: the pointer's moves gathered,
/// each move given only before the event that follows it, and the last one held in `held` while
/// the frame `awaits` its signal, to be given before the next event or frame.
fn gather(
    inputs: Vec<(EventTime, Input<'static>)>,
    held: &mut Option<(EventTime, Input<'static>)>,
    awaits: bool,
) -> Vec<(EventTime, Input<'static>)> {
    let mut given = Vec::with_capacity(inputs.len() + 1);
    for (time, input) in inputs {
        if let Input::PointerMoved { .. } = input {
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

/// Gives the interface one event on the window, then, if it took it, serves what it asks.
fn give<A: App>(session: &mut Session<A>, (time, input): (EventTime, Input<'static>)) {
    let ui = &mut session.ui;
    if ui.event(Event::new(TargetId::MAIN, time, input)).taken() {
        serve_outputs(ui);
    }
}

/// Reads the window's outputs and serves them: the clipboard's reads answered with no text until
/// the host has a clipboard (M6d.8); the cursor's shape and the input method wait for theirs.
fn serve_outputs(ui: &mut uwui::tree::Ui) {
    let outputs = ui.outputs(TargetId::MAIN);
    for read in outputs.clipboard_reads {
        let input = Input::ClipboardText { read, text: None };
        let _ = ui.event(Event::new(
            TargetId::MAIN,
            EventTime::received(clock::now()),
            input,
        ));
    }
}

/// A side of a size for the protocol, which counts in `i32`: zero for none.
fn side(pixels: u32) -> i32 {
    i32::try_from(pixels).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use uwui::time::Instant;

    use super::*;

    fn moved(at: u64, x: f32) -> (EventTime, Input<'static>) {
        (
            EventTime::received(Instant::from_nanos(at)),
            Input::PointerMoved { x, y: 0.0 },
        )
    }

    fn told(inputs: &[(EventTime, Input<'static>)]) -> Vec<String> {
        inputs
            .iter()
            .map(|(time, input)| format!("{} {input:?}", time.at.as_nanos()))
            .collect()
    }

    #[test]
    fn the_pointers_moves_are_gathered_and_the_last_held_while_a_frame_waits() {
        let left = (
            EventTime::received(Instant::from_nanos(3)),
            Input::PointerLeft,
        );
        let mut held = None;
        // Two moves then the pointer leaving: the last move before it, the first dropped.
        let given = gather(vec![moved(1, 1.0), moved(2, 2.0), left], &mut held, true);
        assert_eq!(
            told(&given),
            ["2 PointerMoved { x: 2.0, y: 0.0 }", "3 PointerLeft"]
        );
        // While a frame waits, the last move is held, then replaced by a later one.
        assert!(gather(vec![moved(4, 4.0)], &mut held, true).is_empty());
        assert!(gather(vec![moved(5, 5.0)], &mut held, true).is_empty());
        // Once nothing waits, the held move is given.
        let given = gather(Vec::new(), &mut held, false);
        assert_eq!(told(&given), ["5 PointerMoved { x: 5.0, y: 0.0 }"]);
        assert!(held.is_none());
    }
}
