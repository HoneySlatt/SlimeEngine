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
//!   host's clock (`crate::anchor`); entering and leaving with their reception's. Its shape, through
//!   `wp_cursor_shape_v1`, where the compositor gives it: elsewhere its own cursor shows.
//! - *The keyboard*: its keys through libxkbcommon (`crate::xkb`), their times anchored as the
//!   pointer's; their repeat, the host's own (`repeat`); the focus and the modifiers.
//! - *The scale*: `wp_fractional_scale_v1` with `wp_viewporter`, the image
//!   `round(logical × scale)` pixels and the viewport the window's logical size; else the surface's
//!   preferred integer scale; else the largest integer scale of the outputs that it lies on;
//!   else 1.
//! - *Activation*: the start's `XDG_ACTIVATION_TOKEN`, and the application's requests, through
//!   `xdg_activation_v1`.
//!
//! What waits for its milestone: the presentations' times (M6d.7a2c).

mod protocol;
mod repeat;
mod wait;

use std::error::Error;
use std::fmt::{self, Write as _};
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::Duration;

use uwui::host::{Event, Input, TargetId, TargetSize};
use uwui::time::EventTime;
use wayland_client::globals::{GlobalList, registry_queue_init};
use wayland_client::protocol::{wl_compositor, wl_output, wl_seat, wl_surface};
use wayland_client::{Connection, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1;
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::app::{Activation, App, Look, Window, WindowSettings};
use crate::clock::{self, Suspends};
use crate::gpu::Gpu;
use crate::pacing::{Next, Pacing};
use crate::session::{Session, StartError};
use crate::wake::{self, Asks};
use crate::{EXIT_HOST_FAILED, EXIT_START_FAILED};

use protocol::{Cursor, Kept, Told};
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
    /// The surface's fractional scale and its viewport, where the compositor gives both.
    fractional: Option<(
        wp_fractional_scale_v1::WpFractionalScaleV1,
        wp_viewport::WpViewport,
    )>,
    activation: Option<xdg_activation_v1::XdgActivationV1>,
    seat: Option<wl_seat::WlSeat>,
    // Kept so that their objects live as long as the window's.
    _outputs: Vec<wl_output::WlOutput>,
    _compositor: wl_compositor::WlCompositor,
    _base: xdg_wm_base::XdgWmBase,
}

/// The scale that the window draws at, as the compositor last preferred it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Scale {
    /// `wp_fractional_scale_v1`'s, in 120ths, which the viewport carries.
    fractional: Option<u32>,
    /// The surface's preferred integer scale, which its buffer scale carries.
    integer: Option<i32>,
}

/// How a scale and a logical size are drawn: the image's size, the physical pixels per logical one,
/// and the buffer scale and the viewport's destination asked of the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Drawn {
    pixels: [u32; 2],
    scale: f64,
    buffer_scale: i32,
    destination: Option<[u32; 2]>,
}

impl Scale {
    /// How the window of `logical` pixels draws at this scale; the fractional one only where the
    /// surface has a viewport.
    fn drawn(self, logical: [u32; 2], viewport: bool) -> Drawn {
        let logical = logical.map(|side| side.max(1));
        match (self.fractional.filter(|_| viewport), self.integer) {
            (Some(fraction), _) if fraction > 0 => {
                let scale = f64::from(fraction) / 120.0;
                Drawn {
                    // Rounded half away from zero, as the protocol asks of a toplevel.
                    pixels: logical.map(|side| pixels(f64::from(side) * scale)),
                    scale,
                    buffer_scale: 1,
                    destination: Some(logical),
                }
            }
            (_, Some(integer)) if integer > 1 => Drawn {
                pixels: logical.map(|side| side.saturating_mul(integer.unsigned_abs())),
                scale: f64::from(integer),
                buffer_scale: integer,
                destination: None,
            },
            _ => Drawn {
                pixels: logical,
                scale: 1.0,
                buffer_scale: 1,
                destination: None,
            },
        }
    }
}

/// A side in whole pixels, rounded half away from zero, one at least.
fn pixels(side: f64) -> u32 {
    let side = side.round();
    if side >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        (side as u32).max(1)
    }
}

impl Drop for Objects {
    fn drop(&mut self) {
        // The decoration goes before its toplevel, as xdg-decoration asks; the scale's objects
        // before their surface.
        if let Some(decoration) = self.decoration.take() {
            decoration.destroy();
        }
        if let Some((fractional, viewport)) = self.fractional.take() {
            fractional.destroy();
            viewport.destroy();
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
    /// The scale that the compositor prefers, and how the window draws now.
    scale: Scale,
    drawn: Drawn,
    /// The pointer's last move, given to the interface before the next event or frame.
    held_move: Option<(EventTime, Kept)>,
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
        told.cursor.manager = globals.bind(&handle, 1..=1, ()).ok();
        if told.cursor.manager.is_none() {
            tracing::info!(
                "the compositor gives no wp_cursor_shape_v1: its own cursor shows over the window"
            );
        }
        activate_at_start(&objects);
        // The first configure comes after the first commit, without an image.
        objects.surface.commit();
        while told.configure.is_none() {
            queue
                .blocking_dispatch(&mut told)
                .map_err(|cause| failed("the compositor's first configure", cause))?;
        }
        let logical = told
            .configure
            .and_then(|configure| configure.size)
            .unwrap_or(window.size());
        let scale = Scale::default();
        let drawn = scale.drawn(logical, objects.fractional.is_some());
        let size = drawn.pixels;
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
            scale,
            drawn,
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
            .bind(handle, 1..=6, ())
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
        let viewporter: Option<wp_viewporter::WpViewporter> = globals.bind(handle, 1..=1, ()).ok();
        let fractional_manager: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1> =
            globals.bind(handle, 1..=1, ()).ok();
        let activation: Option<xdg_activation_v1::XdgActivationV1> =
            globals.bind(handle, 1..=1, ()).ok();
        // Each output's scale, which a compositor without a scale for the surface gives (version
        // 2); outputs that come later are not followed.
        let outputs: Vec<wl_output::WlOutput> = globals.contents().with_list(|list| {
            list.iter()
                .filter(|global| global.interface == "wl_output" && global.version >= 2)
                .map(|global| {
                    globals
                        .registry()
                        .bind(global.name, global.version.min(4), handle, ())
                })
                .collect()
        });
        let surface = compositor.create_surface(handle, ());
        let fractional = match (&fractional_manager, &viewporter) {
            (Some(manager), Some(viewporter)) => Some((
                manager.get_fractional_scale(&surface, handle, ()),
                viewporter.get_viewport(&surface, handle, ()),
            )),
            _ => None,
        };
        // The managers are needed no more once their objects are made.
        if let Some(manager) = fractional_manager {
            manager.destroy();
        }
        if let Some(viewporter) = viewporter {
            viewporter.destroy();
        }
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
            fractional,
            activation,
            seat,
            _outputs: outputs,
            _compositor: compositor,
            _base: base,
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
            self.told.repeat(clock::now());
            for token in std::mem::take(&mut self.told.tokens) {
                if let Some(activation) = &self.objects.activation {
                    activation.activate(token, &self.objects.surface);
                }
            }
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
                // The commit that follows the acknowledgement carries the new size.
                self.pacing.host_changed();
            }
            let scale = Scale {
                fractional: self.told.fractional_scale,
                integer: self.told.integer_scale(),
            };
            if scale != self.scale {
                self.scale = scale;
                self.pacing.asked_from_outside();
            }
            if self.draw_at(session.window.size()) {
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
            // A key held wakes the loop at its next repeat.
            let deadline = match (deadline, self.told.keys.repeat.deadline()) {
                (Some(deadline), Some(repeat)) => Some(deadline.min(repeat)),
                (deadline, repeat) => deadline.or(repeat),
            };
            self.wait(deadline)?;
        }
        Ok(())
    }

    /// Draws the window of `logical` pixels at the scale that the compositor prefers: asks the
    /// surface's buffer scale or its viewport's destination, which the next present's commit
    /// carries, and sizes the window's images. Whether anything changed.
    fn draw_at(&mut self, logical: [u32; 2]) -> bool {
        let viewport = self
            .objects
            .fractional
            .as_ref()
            .map(|(_, viewport)| viewport);
        let drawn = self.scale.drawn(logical, viewport.is_some());
        if drawn == self.drawn {
            return false;
        }
        if drawn.buffer_scale != self.drawn.buffer_scale {
            self.objects.surface.set_buffer_scale(drawn.buffer_scale);
        }
        if let Some(viewport) = viewport
            && drawn.destination != self.drawn.destination
        {
            match drawn.destination {
                Some([width, height]) => viewport.set_destination(side(width), side(height)),
                // Unset: the surface's size is its image's, divided by its buffer scale.
                None => viewport.set_destination(-1, -1),
            }
        }
        self.drawn = drawn;
        self.told.scale = drawn.scale;
        self.gpu.resize(drawn.pixels);
        true
    }

    /// Gives the interface the person's input that came, in order; the pointer's last move held
    /// while the frame waits for its signal, as uwui asks the moves of a frame gathered.
    fn give_inputs<A: App>(&mut self, session: &mut Session<A>) {
        let inputs = std::mem::take(&mut self.told.inputs);
        for input in gather(inputs, &mut self.held_move, self.pacing.awaiting()) {
            give(session, &mut self.told.cursor, input);
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
        if let Some(activation) = requests.activation {
            self.ask_activation(activation, window.app_id());
        }
        // The minimum size is the surface's state, which the next commit applies.
        requests.min_size
    }

    /// Asks the compositor for a token that activates the window, which comes back to the loop
    /// (`Told::tokens`): bringing it to the front names the person's last press, asking for
    /// attention names none.
    fn ask_activation(&self, activation: Activation, app_id: &str) {
        let Some(manager) = &self.objects.activation else {
            // Told once: the next requests do nothing as well.
            static TOLD: std::sync::Once = std::sync::Once::new();
            TOLD.call_once(|| {
                tracing::info!(
                    "the compositor gives no xdg_activation_v1: the window's requests do nothing"
                );
            });
            return;
        };
        let token = manager.get_activation_token(&self.handle, ());
        if activation == Activation::Front
            && let (Some(serial), Some(seat)) = (self.told.serial, &self.objects.seat)
        {
            token.set_serial(serial, seat);
        }
        token.set_app_id(app_id.to_owned());
        token.set_surface(&self.objects.surface);
        token.commit();
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
            give(session, &mut self.told.cursor, held);
        }
        let ui = &mut session.ui;
        // Never refused: each side is one pixel at least, and the scale above zero.
        let drawn = self.drawn;
        if let Ok(size) = TargetSize::new(drawn.pixels, drawn.scale as f32) {
            ui.set_target_size(TargetId::MAIN, size);
        }
        ui.set_frame_shown(TargetId::MAIN, self.pacing.shown(now));
        let (list, _) = ui.frame(TargetId::MAIN);
        serve_outputs(ui, &mut self.told.cursor);
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
    inputs: Vec<(EventTime, Kept)>,
    held: &mut Option<(EventTime, Kept)>,
    awaits: bool,
) -> Vec<(EventTime, Kept)> {
    let mut given = Vec::with_capacity(inputs.len() + 1);
    for (time, input) in inputs {
        if let Kept::Other(Input::PointerMoved { .. }) = input {
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
fn give<A: App>(session: &mut Session<A>, cursor: &mut Cursor, (time, input): (EventTime, Kept)) {
    let ui = &mut session.ui;
    let answer = match &input {
        Kept::Key(key) => ui.event(Event::new(TargetId::MAIN, time, Input::Key(key))),
        Kept::Other(input) => ui.event(Event::new(TargetId::MAIN, time, *input)),
    };
    if answer.taken() {
        serve_outputs(ui, cursor);
    }
}

/// Reads the window's outputs and serves them: the cursor's shape; the clipboard's reads answered
/// with no text until the host has a clipboard, and the input method waiting for its own (M6d.8).
fn serve_outputs(ui: &mut uwui::tree::Ui, cursor: &mut Cursor) {
    let outputs = ui.outputs(TargetId::MAIN);
    if let Some(shape) = outputs.cursor {
        cursor.set(shape);
    }
    for read in outputs.clipboard_reads {
        let input = Input::ClipboardText { read, text: None };
        let _ = ui.event(Event::new(
            TargetId::MAIN,
            EventTime::received(clock::now()),
            input,
        ));
    }
}

/// Activates the window with the token that started the program, if the environment holds one:
/// the launcher that started it lets it take the focus so. The variable stays set: a token is
/// spent once, so a program that the application starts in turn and that reads it is refused.
fn activate_at_start(objects: &Objects) {
    let token = std::env::var("XDG_ACTIVATION_TOKEN").unwrap_or_default();
    if token.is_empty() {
        return;
    }
    match &objects.activation {
        Some(activation) => activation.activate(token, &objects.surface),
        None => tracing::info!(
            "the compositor gives no xdg_activation_v1: the start's token goes unused"
        ),
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

    fn moved(at: u64, x: f32) -> (EventTime, Kept) {
        (
            EventTime::received(Instant::from_nanos(at)),
            Kept::Other(Input::PointerMoved { x, y: 0.0 }),
        )
    }

    fn told(inputs: &[(EventTime, Kept)]) -> Vec<String> {
        inputs
            .iter()
            .map(|(time, input)| match input {
                Kept::Other(input) => format!("{} {input:?}", time.at.as_nanos()),
                Kept::Key(key) => format!("{} {key:?}", time.at.as_nanos()),
            })
            .collect()
    }

    #[test]
    fn the_pointers_moves_are_gathered_and_the_last_held_while_a_frame_waits() {
        let left = (
            EventTime::received(Instant::from_nanos(3)),
            Kept::Other(Input::PointerLeft),
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

    #[test]
    fn the_window_draws_at_the_fractional_scale_through_its_viewport() {
        let scale = Scale {
            fractional: Some(180),
            integer: Some(2),
        };
        // 1.5: 721 × 1.5 is 1081.5, rounded away from zero.
        assert_eq!(
            scale.drawn([721, 480], true),
            Drawn {
                pixels: [1082, 720],
                scale: 1.5,
                buffer_scale: 1,
                destination: Some([721, 480]),
            }
        );
        // Without a viewport, the integer scale, through the buffer's.
        assert_eq!(
            scale.drawn([721, 480], false),
            Drawn {
                pixels: [1442, 960],
                scale: 2.0,
                buffer_scale: 2,
                destination: None,
            }
        );
    }

    #[test]
    fn without_a_scale_or_with_a_wrong_one_the_window_draws_at_one() {
        let one = Drawn {
            pixels: [720, 480],
            scale: 1.0,
            buffer_scale: 1,
            destination: None,
        };
        assert_eq!(Scale::default().drawn([720, 480], true), one);
        let wrong = Scale {
            fractional: Some(0),
            integer: Some(-3),
        };
        assert_eq!(wrong.drawn([720, 480], true), one);
        // A side of zero counts as one.
        assert_eq!(Scale::default().drawn([0, 0], false).pixels, [1, 1]);
        assert_eq!(pixels(f64::MAX), u32::MAX);
        assert_eq!(pixels(0.2), 1);
    }
}
