//! The platform layer of uwui's desktop host: a window, the person's input with its times, and the
//! platform's signal of frames, given to a host above that knows the interface and the GPU, which
//! this crate knows neither of
//! (`docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md`).
//!
//! The layer runs the platform's loop and calls the host back ([`run`], [`Handler`]), the shape that
//! Win32's modal loops and AppKit's run loop ask for as well as Wayland's and X11's
//! (`docs/adr/0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md`):
//!
//! 1. it opens the window, and calls [`Handler::opened`] once the platform sized it, with the
//!    window's raw handles, on which the host makes what draws into it;
//! 2. at each turn of its loop, it calls [`Handler::turn`] with what came since the last
//!    ([`Happened`]); the host steps, draws, makes its requests of the [`Window`], and says when to
//!    call it again ([`Control`]);
//! 3. as the window closes, it calls [`Handler::closing`], where the host lets go of what it made
//!    on the handles, then hands the host back.
//!
//! Its *core*, which knows no platform, is tested without a window on every platform: the anchor
//! of the events' times (`anchor`) and the wakes of other threads and the termination signals
//! (`wake`). Wayland's layer (`wayland`) is the only platform's yet: X11, Windows and macOS have
//! theirs at M6d.7's next parts, and [`run`] there says that it has none. The keyboard's code of
//! libxkbcommon (`xkb`) is Linux's, which X11's layer will share.
//!
//! Its name is provisional, until M6d.10a names uwui's crates. It is public nowhere but to
//! `uwui-desktop`, which `scripts/check.sh` checks, with its other rule: it depends on neither
//! `uwui`, `uwui-render` nor wgpu. No type of a platform's library appears in its API. Its
//! enumerations are open to new cases, as uwui's are
//! (`docs/adr/0282-errors-logging-assertions-uwuis-public-enumerations-open-to-new-cases.md`).

// A public enumeration may gain cases without breaking its host (ADR-0282).
#![warn(clippy::exhaustive_enums)]

// The core, which only Wayland's layer drives yet: elsewhere it is compiled and tested, and used by
// no layer.
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod anchor;
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod wake;

#[cfg(target_os = "linux")]
mod clock;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod xkb;

use std::fmt;
use std::task::Waker;
use std::time::Duration;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use slime_input::{
    ButtonState, CursorShape, KeyEvent, Modifiers, MouseButton, Pointer, ScrollDelta,
};
use uwui_time::{EventTime, Instant};

/// How the window opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opening {
    /// Its title.
    pub title: String,
    /// The application's identity, which the system groups its windows by.
    pub app_id: String,
    /// Its size in logical pixels, until the platform chooses one.
    pub size: [u32; 2],
    /// The smallest that the person may make it, in logical pixels.
    pub min_size: Option<[u32; 2]>,
    /// Whether it opens filling the screen, without borders.
    pub fullscreen: bool,
}

/// A person's input, in the window's physical pixels.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Input {
    /// A pointer moved to `x` and `y`, from the window's top left.
    PointerMoved {
        /// Which pointer.
        pointer: Pointer,
        /// Across.
        x: f32,
        /// Down.
        y: f32,
    },
    /// A pointer left the window, or a finger lifted.
    PointerLeft {
        /// Which pointer.
        pointer: Pointer,
    },
    /// A button went down or up; a finger touching or lifting is the primary button's.
    PointerButton {
        /// Which pointer.
        pointer: Pointer,
        /// Which button.
        button: MouseButton,
        /// Down or up.
        state: ButtonState,
    },
    /// The wheel scrolled, in the window's physical pixels where it gives pixels.
    Wheel(ScrollDelta),
    /// A key went down, up or repeated.
    Key(KeyEvent),
    /// The modifier keys held changed.
    Modifiers(Modifiers),
    /// The window took the keyboard's focus, or lost it.
    Focus(bool),
}

/// The platform's choice of the window's size and states, which the next image shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configured {
    /// The window's size, in logical pixels.
    pub size: [u32; 2],
    /// Whether it fills the screen.
    pub fullscreen: bool,
}

/// How the window's images are drawn: their size in physical pixels, and the physical pixels per
/// logical one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    /// The image's size, one pixel at least on each side.
    pub pixels: [u32; 2],
    /// The scale, above zero.
    pub scale: f64,
}

/// When an image that the host presented showed, as the platform told it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presentation {
    /// The image's number, which [`Window::ask_signal`] gave as it was presented.
    pub image: u64,
    /// When its light left the screen, on the host's clock; none if it never showed: a later
    /// image replaced it, or the window did not show.
    pub shown: Option<Instant>,
    /// The display's period then, where the platform gives it: a display of variable refresh
    /// gives its shortest.
    pub refresh: Option<Duration>,
}

/// What came since the host's last turn.
#[derive(Debug, Default)]
pub struct Happened {
    /// The person's input, in order, each with its time.
    pub inputs: Vec<(EventTime, Input)>,
    /// The platform sized the window or changed its states: the next image must show it.
    pub configured: Option<Configured>,
    /// The window's images are drawn at another size or scale from now on.
    pub drawn: Option<Drawn>,
    /// The person asked the window to close.
    pub close_asked: bool,
    /// A termination signal came: SIGTERM, SIGINT or SIGHUP; the first alone, a second that
    /// insists ending the process ([`EXIT_TERMINATED`]).
    pub terminated: bool,
    /// Another thread woke the loop through [`Window::waker`].
    pub woken: bool,
    /// When the platform's signal of frames last came, if it came: the image before may be
    /// followed by the next.
    pub signal: Option<Instant>,
    /// When the images presented showed, in the order that they were presented, where the
    /// platform tells it (on Wayland, `wp_presentation`, which every compositor of the series
    /// gives). A platform may tell none for some images: the host never waits for one.
    pub presentations: Vec<Presentation>,
}

/// What the layer does after a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Control {
    /// Waits for the platform, a wake, or the time given, then turns again.
    Wait(Option<Instant>),
    /// Ends the run: the window closes.
    Exit,
}

/// What the application asks of the system for its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Activation {
    /// Brought to the front, given the focus.
    Front,
    /// Shown as asking for the person's attention, its focus left where it is.
    Attention,
}

/// The process's exit status when a second termination signal, SIGTERM, SIGINT or SIGHUP, half a
/// second or more after the first, ends it at once, without a call of the host: the status that a
/// shell gives a process ended by Ctrl+C. The first is the host's, as [`Happened::terminated`].
pub const EXIT_TERMINATED: i32 = 130;

/// The host above the layer, which it calls back.
pub trait Handler {
    /// The window is open and sized, its images drawn at `drawn`. The host makes what draws into
    /// it on [`Window::raw_handles`]. An error ends the run: the layer calls
    /// [`closing`](Self::closing), then closes the window.
    ///
    /// # Errors
    ///
    /// The host's own, which [`run`] does not see.
    fn opened(&mut self, window: &mut Window<'_>, drawn: Drawn) -> Result<(), Stopped>;

    /// A turn of the loop, with what came since the last; the first follows
    /// [`opened`](Self::opened) at once.
    fn turn(&mut self, window: &mut Window<'_>, happened: Happened) -> Control;

    /// The window closes: the host lets go of what it made on the window's raw handles, which are
    /// destroyed once it returns. Called once after [`opened`](Self::opened), whatever it answered
    /// and however the run ends, but by a panic.
    fn closing(&mut self);
}

/// The host stopped the run from [`Handler::opened`]; it keeps why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stopped;

/// The window, during a call of the [`Handler`]: its handles, its clock, and the requests that the
/// host makes of it.
pub struct Window<'a> {
    #[cfg(target_os = "linux")]
    platform: &'a mut wayland::Platform,
    #[cfg(not(target_os = "linux"))]
    never: std::convert::Infallible,
    #[cfg(not(target_os = "linux"))]
    _platform: std::marker::PhantomData<&'a mut ()>,
}

impl Window<'_> {
    /// The window's raw handles, on which the host makes its surface. They stay valid until
    /// [`Handler::closing`] returns, and until the layer drops the handler, whichever comes first:
    /// the layer drops the handler before the window, a panic's unwinding included.
    pub fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle) {
        #[cfg(target_os = "linux")]
        return self.platform.raw_handles();
        #[cfg(not(target_os = "linux"))]
        match self.never {}
    }

    /// The host's clock now: one that counts the time that the machine spends suspended, which
    /// every time that the layer gives is on.
    pub fn now(&self) -> Instant {
        #[cfg(target_os = "linux")]
        return clock::now();
        #[cfg(not(target_os = "linux"))]
        match self.never {}
    }

    /// A waker that turns the loop from any thread, without blocking: wakes that come before the
    /// next turn turn it once ([`Happened::woken`]).
    pub fn waker(&self) -> Waker {
        #[cfg(target_os = "linux")]
        return self.platform.waker();
        #[cfg(not(target_os = "linux"))]
        match self.never {}
    }

    /// Gives the window `title`.
    pub fn set_title(&mut self, title: &str) {
        #[cfg(target_os = "linux")]
        self.platform.set_title(title);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = title;
            match self.never {}
        }
    }

    /// Sets the smallest that the person may make the window, in logical pixels; none for no
    /// bound. Whether an image must follow to apply it: on Wayland, the next image's commit does.
    #[must_use = "an image must follow where it says so"]
    pub fn set_min_size(&mut self, size: Option<[u32; 2]>) -> bool {
        #[cfg(target_os = "linux")]
        return self.platform.set_min_size(size);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = size;
            match self.never {}
        }
    }

    /// Asks the platform to fill the screen with the window, without borders, or to stop.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        #[cfg(target_os = "linux")]
        self.platform.set_fullscreen(fullscreen);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = fullscreen;
            match self.never {}
        }
    }

    /// Asks the system to activate the window as `activation` says; nothing where it gives no way.
    pub fn activate(&mut self, activation: Activation) {
        #[cfg(target_os = "linux")]
        self.platform.activate(activation);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = activation;
            match self.never {}
        }
    }

    /// Shows `shape` while a pointer lies in the window, where the platform shows shapes.
    pub fn set_cursor(&mut self, shape: CursorShape) {
        #[cfg(target_os = "linux")]
        self.platform.set_cursor(shape);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = shape;
            match self.never {}
        }
    }

    /// Asks the platform's signal of frames for the image presented next, and when it shows
    /// ([`Happened::presentations`]): called just before the present, so that both ride on the
    /// image that they follow. The image's number: 0 for the window's first, then one more at each
    /// call.
    pub fn ask_signal(&mut self) -> u64 {
        #[cfg(target_os = "linux")]
        return self.platform.ask_signal();
        #[cfg(not(target_os = "linux"))]
        match self.never {}
    }
}

/// Why the layer stopped: the window could not open, or the platform's connection broke.
#[derive(Debug)]
pub struct Error(String);

impl Error {
    #[cfg_attr(
        not(target_os = "linux"),
        allow(dead_code, reason = "no windowing layer fails here yet")
    )]
    fn new(what: &str, cause: impl fmt::Display) -> Self {
        Self(format!("{what}: {cause}"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Opens a window as `opening` says and runs the platform's loop, calling `handler` back, until a
/// turn ends it or the platform fails; hands `handler` back with how the run ended. The first
/// termination signal reaches the handler as [`Happened::terminated`]; a second, half a second or
/// more later, ends the process with [`EXIT_TERMINATED`].
///
/// On Linux, it needs a Wayland compositor, which `WAYLAND_DISPLAY` names; elsewhere, it has no
/// layer yet, and fails at once.
///
/// # Errors
///
/// When the window cannot open, before [`Handler::opened`], or when the platform fails during the
/// run, after [`Handler::closing`].
pub fn run<H: Handler>(opening: Opening, handler: H) -> (H, Result<(), Error>) {
    #[cfg(target_os = "linux")]
    return wayland::run(opening, handler);
    #[cfg(not(target_os = "linux"))]
    {
        drop(opening);
        (
            handler,
            Err(Error("no windowing layer for this system yet".to_owned())),
        )
    }
}
