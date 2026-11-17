//! uwui's platform layer on Wayland, on `wayland-client` itself
//! (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`).
//!
//! It connects to the compositor that `WAYLAND_DISPLAY` names, through libwayland's own display,
//! loaded at run time, which Vulkan's driver shares: the driver commits the surface as the host
//! presents each image. The host orders its own requests around those commits: the frame callback
//! is asked just before each present ([`crate::Window::ask_signal`]), so that it rides on the
//! commit that shows the image.
//!
//! - *The window*: an xdg-shell toplevel, its title, its app id, its minimum size, filling the
//!   screen or not, its closing told to the host rather than done.
//! - *Its bar*: asked of the compositor through xdg-decoration, which Weston and Mutter do not
//!   give: there the window has none yet.
//! - *Its frames' signal*: the compositor's frame callback.
//! - *The pointer*: its moves, buttons and wheel, each with the compositor's time anchored on the
//!   host's clock (`crate::anchor`); entering and leaving with their reception's. Its shape, through
//!   `wp_cursor_shape_v1`, where the compositor gives it: elsewhere its own cursor shows.
//! - *Touch*: each finger of `wl_touch` a pointer of its own, named anew at each touch.
//! - *The keyboard*: its keys through libxkbcommon (`crate::xkb`), their times anchored as the
//!   pointer's; their repeat, the layer's own (`repeat`); the focus and the modifiers.
//! - *The scale*: `wp_fractional_scale_v1` with `wp_viewporter`, the image
//!   `round(logical × scale)` pixels and the viewport the window's logical size; else the surface's
//!   preferred integer scale; else the largest integer scale of the outputs that it lies on;
//!   else 1.
//! - *Activation*: the start's `XDG_ACTIVATION_TOKEN`, and the host's requests, through
//!   `xdg_activation_v1`.
//!
//! What waits for its milestone: the presentations' times (M6d.7a2c).

mod protocol;
mod repeat;
mod wait;

use std::ptr::NonNull;
use std::sync::Arc;
use std::task::Waker;
use std::time::Duration;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use slime_input::CursorShape;
use uwui_time::Instant;
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

use crate::clock::{self, Suspends};
use crate::wake::{self, Asks};
use crate::{Activation, Configured, Control, Drawn, Error, Handler, Happened, Opening, Window};

use protocol::Told;
use wait::Waits;

/// The shortest time suspended after which the events' anchor is made again: what parts the two
/// clocks while the machine is awake is far below it.
const RESUMED: Duration = Duration::from_secs(1);

/// Runs the loop of a window opened as `opening` says, calling `handler` back, until it ends.
pub(crate) fn run<H: Handler>(opening: Opening, handler: H) -> (H, Result<(), Error>) {
    let (platform, drawn) = match Platform::open(opening) {
        Ok(opened) => opened,
        Err(error) => return (handler, Err(error)),
    };
    // The handler is dropped before the platform, a panic's unwinding included: what it made on
    // the window's handles goes before them.
    let mut running = Running { handler, platform };
    let ran = running.run(drawn);
    let Running { handler, platform } = running;
    drop(platform);
    (handler, ran)
}

/// The handler and the window that it draws into, in the order that they are dropped.
struct Running<H> {
    handler: H,
    platform: Platform,
}

impl<H: Handler> Running<H> {
    /// The loop, from the window's opening at `drawn` until a turn ends it or the connection
    /// fails; the handler told that the window closes in both cases.
    fn run(&mut self, drawn: Drawn) -> Result<(), Error> {
        let opened = self.handler.opened(
            &mut Window {
                platform: &mut self.platform,
            },
            drawn,
        );
        let ran = match opened {
            Ok(()) => self.turns(),
            Err(_) => Ok(()),
        };
        self.handler.closing();
        ran
    }

    fn turns(&mut self) -> Result<(), Error> {
        loop {
            let happened = self.platform.happened()?;
            let control = self.handler.turn(
                &mut Window {
                    platform: &mut self.platform,
                },
                happened,
            );
            match control {
                Control::Exit => return Ok(()),
                Control::Wait(deadline) => self.platform.wait(deadline)?,
            }
        }
    }
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
struct Scaled {
    pixels: [u32; 2],
    scale: f64,
    buffer_scale: i32,
    destination: Option<[u32; 2]>,
}

impl Scaled {
    /// What the host is told of it.
    fn drawn(self) -> Drawn {
        Drawn {
            pixels: self.pixels,
            scale: self.scale,
        }
    }
}

impl Scale {
    /// How the window of `logical` pixels draws at this scale; the fractional one only where the
    /// surface has a viewport.
    fn drawn(self, logical: [u32; 2], viewport: bool) -> Scaled {
        let logical = logical.map(|side| side.max(1));
        match (self.fractional.filter(|_| viewport), self.integer) {
            (Some(fraction), _) if fraction > 0 => {
                let scale = f64::from(fraction) / 120.0;
                Scaled {
                    // Rounded half away from zero, as the protocol asks of a toplevel.
                    pixels: logical.map(|side| pixels(f64::from(side) * scale)),
                    scale,
                    buffer_scale: 1,
                    destination: Some(logical),
                }
            }
            (_, Some(integer)) if integer > 1 => Scaled {
                pixels: logical.map(|side| side.saturating_mul(integer.unsigned_abs())),
                scale: f64::from(integer),
                buffer_scale: integer,
                destination: None,
            },
            _ => Scaled {
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

/// The layer on Wayland: its connection, the window and its loop's state.
pub(crate) struct Platform {
    objects: Objects,
    told: Told,
    queue: EventQueue<Told>,
    handle: QueueHandle<Told>,
    connection: Connection,
    waits: Waits,
    asks: Arc<Asks>,
    suspends: Suspends,
    app_id: String,
    /// libwayland's display and the window's surface, which `connection` and `objects` keep live.
    handles: (NonNull<std::ffi::c_void>, NonNull<std::ffi::c_void>),
    /// The window's size in logical pixels, as the compositor last chose it.
    logical: [u32; 2],
    /// The scale that the compositor prefers, and how the window draws now.
    scale: Scale,
    scaled: Scaled,
}

impl Platform {
    /// Connects to the compositor, opens the window of `opening`, waits for its first configure,
    /// and sets the termination's handler: the platform, and how the window draws.
    fn open(opening: Opening) -> Result<(Self, Drawn), Error> {
        let waits = Waits::new().map_err(|cause| Error::new("the loop's waits", cause))?;
        let asks = Asks::new(waits.notifier());
        let connection = Connection::connect_to_env()
            .map_err(|cause| Error::new("no Wayland compositor answers", cause))?;
        let (globals, mut queue) = registry_queue_init::<Told>(&connection)
            .map_err(|cause| Error::new("the compositor's globals", cause))?;
        let handle = queue.handle();
        let mut told = Told::new();
        let objects = Self::window(&globals, &handle, &opening)?;
        told.cursor.manager = globals.bind(&handle, 1..=1, ()).ok();
        if told.cursor.manager.is_none() {
            tracing::info!(
                "the compositor gives no wp_cursor_shape_v1: its own cursor shows over the window"
            );
        }
        activate_at_start(&objects);
        // The first configure comes after the first commit, without an image; it is acknowledged
        // at the first turn, whose image shows it.
        objects.surface.commit();
        while told.configure.is_none() {
            queue
                .blocking_dispatch(&mut told)
                .map_err(|cause| Error::new("the compositor's first configure", cause))?;
        }
        let logical = told
            .configure
            .and_then(|configure| configure.size)
            .unwrap_or(opening.size);
        let scale = Scale::default();
        let scaled = scale.drawn(logical, objects.fractional.is_some());
        let display = NonNull::new(connection.backend().display_ptr().cast());
        let surface = NonNull::new(objects.surface.id().as_ptr().cast());
        let (Some(display), Some(surface)) = (display, surface) else {
            return Err(Error("the window's surface has no object".to_owned()));
        };
        wake::on_termination(&asks);
        let platform = Self {
            objects,
            told,
            queue,
            handle,
            connection,
            waits,
            asks,
            suspends: Suspends::new(),
            app_id: opening.app_id,
            handles: (display, surface),
            logical,
            scale,
            scaled,
        };
        Ok((platform, scaled.drawn()))
    }

    /// Makes the window of `opening`, and asks for its bar.
    fn window(
        globals: &GlobalList,
        handle: &QueueHandle<Told>,
        opening: &Opening,
    ) -> Result<Objects, Error> {
        let compositor: wl_compositor::WlCompositor = globals
            .bind(handle, 1..=6, ())
            .map_err(|cause| Error::new("the compositor gives no wl_compositor", cause))?;
        let base: xdg_wm_base::XdgWmBase = globals
            .bind(handle, 1..=6, ())
            .map_err(|cause| Error::new("the compositor gives no xdg_wm_base", cause))?;
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
        toplevel.set_title(opening.title.clone());
        toplevel.set_app_id(opening.app_id.clone());
        let [width, height] = opening.min_size.unwrap_or([0, 0]);
        toplevel.set_min_size(side(width), side(height));
        if opening.fullscreen {
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

    /// Reads what the compositor sent and what other threads asked since the last turn: the
    /// configure acknowledged, the scale applied to the surface, which the next present's commit
    /// carries.
    fn happened(&mut self) -> Result<Happened, Error> {
        // Before the events are timed: their anchor is made again after a suspend.
        if self.suspends.take() >= RESUMED {
            self.told.resumed();
        }
        self.queue
            .dispatch_pending(&mut self.told)
            .map_err(|cause| Error::new("the connection to the compositor", cause))?;
        self.told.repeat(clock::now());
        for token in std::mem::take(&mut self.told.tokens) {
            if let Some(activation) = &self.objects.activation {
                activation.activate(token, &self.objects.surface);
            }
        }
        let configured = self.told.configure.take().map(|configure| {
            self.objects.xdg_surface.ack_configure(configure.serial);
            if let Some(size) = configure.size {
                self.logical = size;
            }
            Configured {
                size: self.logical,
                fullscreen: configure.fullscreen,
            }
        });
        self.scale = Scale {
            fractional: self.told.fractional_scale,
            integer: self.told.integer_scale(),
        };
        let drawn = self.scale_surface().then(|| self.scaled.drawn());
        Ok(Happened {
            inputs: std::mem::take(&mut self.told.inputs),
            configured,
            drawn,
            close_asked: std::mem::take(&mut self.told.close),
            terminated: self.asks.take_terminated(),
            woken: self.asks.take_woken(),
            signal: self.told.signal.take(),
        })
    }

    /// Draws the window of its logical size at the scale that the compositor prefers: asks the
    /// surface's buffer scale or its viewport's destination, which the next present's commit
    /// carries. Whether anything changed.
    fn scale_surface(&mut self) -> bool {
        let viewport = self
            .objects
            .fractional
            .as_ref()
            .map(|(_, viewport)| viewport);
        let scaled = self.scale.drawn(self.logical, viewport.is_some());
        if scaled == self.scaled {
            return false;
        }
        if scaled.buffer_scale != self.scaled.buffer_scale {
            self.objects.surface.set_buffer_scale(scaled.buffer_scale);
        }
        if let Some(viewport) = viewport
            && scaled.destination != self.scaled.destination
        {
            match scaled.destination {
                Some([width, height]) => viewport.set_destination(side(width), side(height)),
                // Unset: the surface's size is its image's, divided by its buffer scale.
                None => viewport.set_destination(-1, -1),
            }
        }
        self.scaled = scaled;
        self.told.scale = scaled.scale;
        true
    }

    /// Waits for the compositor, a wake, `deadline` or the next repeat of a key held, then reads
    /// what the compositor sent.
    fn wait(&mut self, deadline: Option<Instant>) -> Result<(), Error> {
        let deadline = match (deadline, self.told.keys.repeat.deadline()) {
            (Some(deadline), Some(repeat)) => Some(deadline.min(repeat)),
            (deadline, repeat) => deadline.or(repeat),
        };
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
            Err(cause) => return Err(Error::new("the connection to the compositor", cause)),
        };
        let readable = self
            .waits
            .wait(guard.connection_fd(), deadline, full)
            .map_err(|cause| Error::new("the loop's wait", cause))?;
        if readable {
            match guard.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(error))
                    if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(cause) => return Err(Error::new("the connection to the compositor", cause)),
            }
        }
        Ok(())
    }

    /// The display's and the surface's pointers, which the platform keeps live until it is
    /// dropped.
    pub(crate) fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle) {
        let (display, surface) = self.handles;
        (
            RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display)),
            RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)),
        )
    }

    pub(crate) fn waker(&self) -> Waker {
        self.asks.waker()
    }

    pub(crate) fn set_title(&mut self, title: &str) {
        self.objects.toplevel.set_title(title.to_owned());
    }

    /// The minimum size is the surface's state, which the next commit applies: an image must
    /// follow.
    pub(crate) fn set_min_size(&mut self, size: Option<[u32; 2]>) -> bool {
        let [width, height] = size.unwrap_or([0, 0]);
        self.objects
            .toplevel
            .set_min_size(side(width), side(height));
        true
    }

    pub(crate) fn set_fullscreen(&mut self, fullscreen: bool) {
        if fullscreen {
            self.objects.toplevel.set_fullscreen(None);
        } else {
            self.objects.toplevel.unset_fullscreen();
        }
    }

    pub(crate) fn set_cursor(&mut self, shape: CursorShape) {
        self.told.cursor.set(shape);
    }

    pub(crate) fn ask_signal(&mut self) {
        self.objects.surface.frame(&self.handle, ());
    }

    /// Asks the compositor for a token that activates the window, which comes back to the loop
    /// (`Told::tokens`): bringing it to the front names the person's last press, asking for
    /// attention names none.
    pub(crate) fn activate(&mut self, activation: Activation) {
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
        token.set_app_id(self.app_id.clone());
        token.set_surface(&self.objects.surface);
        token.commit();
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
    use super::*;

    #[test]
    fn the_window_draws_at_the_fractional_scale_through_its_viewport() {
        let scale = Scale {
            fractional: Some(180),
            integer: Some(2),
        };
        // 1.5: 721 × 1.5 is 1081.5, rounded away from zero.
        assert_eq!(
            scale.drawn([721, 480], true),
            Scaled {
                pixels: [1082, 720],
                scale: 1.5,
                buffer_scale: 1,
                destination: Some([721, 480]),
            }
        );
        // Without a viewport, the integer scale, through the buffer's.
        assert_eq!(
            scale.drawn([721, 480], false),
            Scaled {
                pixels: [1442, 960],
                scale: 2.0,
                buffer_scale: 2,
                destination: None,
            }
        );
    }

    #[test]
    fn without_a_scale_or_with_a_wrong_one_the_window_draws_at_one() {
        let one = Scaled {
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
