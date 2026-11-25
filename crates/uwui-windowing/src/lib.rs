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
//! Its *core*, which knows no platform, is tested without a window on every platform: the run that
//! calls the handler back on a platform's window and lets go of both in order (`running`), the
//! anchor of the events' times (`anchor`), a touch screen's fingers each a pointer (`fingers`)
//! and the wakes of other threads and the termination signals (`wake`). Linux has two layers,
//! Wayland's (`wayland`) and X11's (`x11`), which share the loop's wait (`wait`), the host's clock
//! (`clock`) and the keyboard's code of libxkbcommon (`xkb`). Windows has its own, on Win32
//! (`windows`), which shares the clock. macOS has its own at M6d.7d, and [`run`] there says that it
//! has none.
//!
//! Its name is provisional, until M6d.10a names uwui's crates. It is public nowhere but to
//! `uwui-desktop`, which `scripts/check.sh` checks, with its other rule: it depends on neither
//! `uwui`, `uwui-render` nor wgpu. No type of a platform's library appears in its API. Its
//! enumerations are open to new cases, as uwui's are
//! (`docs/adr/0282-errors-logging-assertions-uwuis-public-enumerations-open-to-new-cases.md`).

// A public enumeration may gain cases without breaking its host (ADR-0282).
#![warn(clippy::exhaustive_enums)]

// The core, which Linux's layers and Windows' drive: elsewhere it is compiled and tested, and used
// by no layer.
#[cfg_attr(
    not(any(target_os = "linux", windows)),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod anchor;
#[cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "Windows' touch comes at M6d.7c2, macOS's layer at M6d.7d"
    )
)]
mod fingers;
#[cfg_attr(
    not(any(target_os = "linux", windows)),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod running;
#[cfg_attr(
    not(any(target_os = "linux", windows)),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod wake;

#[cfg(any(target_os = "linux", windows))]
mod clock;
#[cfg(target_os = "linux")]
mod wait;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;
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
    /// The application's identity, which the system groups its windows by: on Wayland and X11;
    /// Windows groups them by their program, and takes none.
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
    /// When its light left the screen, on the host's clock; none where the platform says that it
    /// never showed: a later image replaced it, or the compositor discarded it. A hidden window's
    /// image may be told much later, as the window shows again.
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
    /// A termination signal came: SIGTERM, SIGINT or SIGHUP, or on Windows the console's Ctrl+C,
    /// Ctrl+Break or close; the first alone, a second that insists ending the process
    /// ([`EXIT_TERMINATED`]).
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
    /// The platform lost what the window showed, as X11 does where nothing keeps a covered
    /// window's image: the next image must be drawn and presented again.
    pub exposed: bool,
    /// The system resumed since the last turn, after this long suspended, a second at least: told
    /// once, at the first turn after it. On Linux the kernel's notice of a resume turns the loop
    /// at once, the window at rest too, and the time suspended is measured from the host's clock
    /// against one that stops while the machine sleeps; a suspend to idle whose timekeeping keeps
    /// running, as without a driver of the processor's idle states, stops neither and is not told
    /// (`docs/adr/0315-a-resume-told-by-the-kernels-own-notice-and-given-to-the-program-as-an-action.md`).
    pub resumed: Option<Duration>,
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

/// The process's exit status when a second termination signal, SIGTERM, SIGINT or SIGHUP, or on
/// Windows the console's Ctrl+C, Ctrl+Break or close, half a second or more after the first, ends
/// it at once, without a call of the host: the status that a shell gives a process ended by Ctrl+C. The first is the host's, as [`Happened::terminated`].
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
    platform: &'a mut dyn running::Platform,
}

impl Window<'_> {
    /// The window's raw handles, on which the host makes its surface. They stay valid until
    /// [`Handler::closing`] returns, and until the layer drops the handler, whichever comes first:
    /// the layer drops the handler before the window, a panic's unwinding included.
    pub fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle) {
        self.platform.raw_handles()
    }

    /// The host's clock now: one that counts the time that the machine spends suspended, which
    /// every time that the layer gives is on.
    pub fn now(&self) -> Instant {
        self.platform.now()
    }

    /// A waker that turns the loop from any thread, without blocking: wakes that come before the
    /// next turn turn it once ([`Happened::woken`]).
    pub fn waker(&self) -> Waker {
        self.platform.waker()
    }

    /// Gives the window `title`.
    pub fn set_title(&mut self, title: &str) {
        self.platform.set_title(title);
    }

    /// Sets the smallest that the person may make the window, in logical pixels; none for no
    /// bound. Whether an image must follow to apply it: on Wayland, the next image's commit does;
    /// on X11 and Windows, none needs to.
    #[must_use = "an image must follow where it says so"]
    pub fn set_min_size(&mut self, size: Option<[u32; 2]>) -> bool {
        self.platform.set_min_size(size)
    }

    /// Asks the platform to fill the screen with the window, without borders, or to stop.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.platform.set_fullscreen(fullscreen);
    }

    /// Asks the system to activate the window as `activation` says; nothing where it gives no way.
    /// Windows may refuse the front to a program that the person did not last use: the window then
    /// asks for attention.
    pub fn activate(&mut self, activation: Activation) {
        self.platform.activate(activation);
    }

    /// Shows `shape` while a pointer lies in the window, where the platform shows shapes.
    pub fn set_cursor(&mut self, shape: CursorShape) {
        self.platform.set_cursor(shape);
    }

    /// Asks the platform's signal of frames for the image presented next, and when it shows
    /// ([`Happened::presentations`]): called just before the present, so that both ride on the
    /// image that they follow. The image's number: 0 for the window's first, then one more at each
    /// call.
    pub fn ask_signal(&mut self) -> u64 {
        self.platform.ask_signal()
    }
}

/// Why the layer stopped: the window could not open, or the platform's connection broke.
#[derive(Debug)]
pub struct Error(String);

impl Error {
    #[cfg_attr(
        not(any(target_os = "linux", windows)),
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
/// On Linux, it opens the window on the Wayland compositor that `WAYLAND_DISPLAY` or
/// `WAYLAND_SOCKET` names, and where none is named or the one named cannot open it, on the X server
/// that `DISPLAY` names (`docs/adr/0311-m6d7b-cut-in-three-x11s-window-frames-and-pointer-first-on-x11rb.md`).
/// On Windows, it opens a window of Win32
/// (`docs/adr/0317-m6d7c-cut-in-three-windows-on-windows-sys-its-frames-on-the-displays-vertical-blank-direct3d-12-by-default.md`).
/// Elsewhere, it has no layer yet, and fails at once.
///
/// # Errors
///
/// When the window cannot open, before [`Handler::opened`], or when the platform fails during the
/// run, after [`Handler::closing`].
pub fn run<H: Handler>(opening: Opening, handler: H) -> (H, Result<(), Error>) {
    #[cfg(target_os = "linux")]
    {
        let named = |variable| std::env::var_os(variable).is_some_and(|value| !value.is_empty());
        let mut failed: Option<Error> = None;
        if named("WAYLAND_DISPLAY") || named("WAYLAND_SOCKET") {
            match wayland::Platform::open(opening.clone()) {
                Ok(opened) => return running::run(Ok(opened), handler),
                Err(error) => {
                    tracing::warn!(%error, "the window could not open on Wayland");
                    failed = Some(error);
                }
            }
        }
        if named("DISPLAY") {
            match x11::Platform::open(opening) {
                Ok(opened) => return running::run(Ok(opened), handler),
                Err(error) => {
                    failed = Some(match failed {
                        Some(wayland) => Error(format!("{wayland}; then on X11, {error}")),
                        None => error,
                    });
                }
            }
        }
        let error = failed.unwrap_or_else(|| {
            Error(
                "no display: neither WAYLAND_DISPLAY, WAYLAND_SOCKET nor DISPLAY names one"
                    .to_owned(),
            )
        });
        (handler, Err(error))
    }
    #[cfg(windows)]
    {
        running::run(windows::Platform::open(opening), handler)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        drop(opening);
        (
            handler,
            Err(Error("no windowing layer for this system yet".to_owned())),
        )
    }
}
