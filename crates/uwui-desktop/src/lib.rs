//! uwui's desktop host: an application on uwui alone, in a window of its own, its frames drawn on
//! the platform's signal and none at rest
//! (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`,
//! `docs/adr/0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md`). Its name is
//! provisional, until uwui's crates are named.
//!
//! An application gives [`run`] its look, its window's settings and itself, an [`App`]; the host
//! opens the window and its device, gives the interface the person's input with its times, asks
//! the application to step, draws the interface's frames with `uwui-render`, and ends when the
//! window closes:
//!
//! ```no_run
//! use std::error::Error;
//!
//! use uwui::text::Fonts;
//! use uwui::tree::{Element, RootSettings, Ui};
//! use uwui::host::TargetId;
//! use uwui_desktop::{App, Look, Window, WindowSettings};
//!
//! struct Hello;
//!
//! impl App for Hello {
//!     fn start(&mut self, ui: &mut Ui, _: &mut Window) -> Result<(), Box<dyn Error>> {
//!         let root = ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| s);
//!         ui.build(root, 0, |children| {
//!             children.add("hello", Element::label("Hello, World!"), |_| {});
//!         });
//!         Ok(())
//!     }
//!
//!     fn step(&mut self, ui: &mut Ui, _: &mut Window) {
//!         for _action in ui.take_actions() {}
//!     }
//! }
//!
//! let look = Look { fonts: Fonts::with_system_fonts(), sheets: Vec::new(), themes: Vec::new() };
//! uwui_desktop::run(look, WindowSettings::new("Hello", "org.example.Hello"), Hello);
//! ```
//!
//! The host stands on a platform layer, `uwui-windowing`, which runs the platform's loop and calls
//! it back at each turn, and knows neither the interface nor the GPU
//! (`docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md`).
//! Above it, its *core* knows no platform and is tested without a window, on every platform: the
//! pacing of frames and the rest, the clock given to it (`pacing`), the budgets within which it
//! makes a lost device and a lost surface again (`recovery`), and the application's run
//! (`session`), and what it keeps of the runs that did not end well (`crash`); `host` drives them
//! at each turn, `gpu` draws each frame into the window's image, and `trace` writes the frames and
//! each input to present where [`TRACE_ENV_VAR`] asks.
//! The layer gives Wayland's window and X11's yet: elsewhere [`run`] says that it has none and
//! ends.
//!
//! No type of a platform's library appears in its public API.
// A public enumeration may gain cases without breaking a program; one closed by nature says why
// it is not marked (ADR-0282).
#![warn(clippy::exhaustive_enums)]

mod app;
mod crash;
mod gpu;
mod host;
mod instance;
mod pacing;
mod recovery;
mod session;
mod trace;

pub use app::{App, Close, Look, Window, WindowSettings};
pub use trace::TRACE_ENV_VAR;

/// The process's exit status when the host cannot start, a compositor or a GPU missing, or stops
/// on a failure of its own: the connection to the compositor broken, the device or the surface lost
/// more often than it makes them again. What failed is told on the standard error.
pub const EXIT_HOST_FAILED: i32 = 1;

/// The process's exit status when the application's look does not read or its
/// [`App::start`] fails, which is told on the standard error.
pub const EXIT_START_FAILED: i32 = 2;

/// The process's exit status when a panic ended the run, on the host's thread: in the application's
/// code, or in uwui's or the host's own. The application is called no more, [`App::stop`] included,
/// and the panic is told on the standard error and, where the user's state directory is given, in
/// a crash report (see [`run`]). The status of a panic that leaves `main`, as Rust gives it.
pub const EXIT_PANICKED: i32 = 101;

/// The process's exit status when a second termination signal, half a second or more after the
/// first, ended the run at once: an application that keeps its window, or does not return from
/// [`App::step`], still ends. The status that a shell gives a process ended by Ctrl+C.
pub const EXIT_TERMINATED: i32 = uwui_windowing::EXIT_TERMINATED;

/// Runs `app` in a window opened with `window`, in `look`, and ends the process when the window
/// closes, with status 0; never returns, on any platform.
///
/// The window closes when the application allows a closing that the person or the system asks
/// ([`App::close_requested`]), a termination signal among them, or closes it itself
/// ([`Window::close`]). A failure ends the process with [`EXIT_HOST_FAILED`] or
/// [`EXIT_START_FAILED`]; a panic on the host's thread with [`EXIT_PANICKED`], without
/// [`App::stop`]; a second termination signal with [`EXIT_TERMINATED`], at once.
///
/// What the host keeps of the runs that did not end well goes to the application's crash
/// directory, `crashes` in a directory named by its identity ([`WindowSettings::new`]) in the
/// user's state directory: `$XDG_STATE_HOME`, else `~/.local/state`; `%LOCALAPPDATA%` on Windows.
/// Nothing leaves the machine:
///
/// - The host installs a panic hook, then runs the one installed before it; the hook keeps what a
///   report needs of each panic on the host's thread. A panic that ends the run writes a *crash
///   report*, `crash-<time>-<pid>.txt`: the message, where it came and the backtrace, the
///   application's identity and version ([`WindowSettings::version`]), the host's version, the
///   system, the GPU and its driver, the running user's home directory written `~`. A panic that
///   the application catches itself writes none. The 20 newest are kept. The backtrace's paths of
///   the machine that built the program are written as they are: a program built for others
///   hides them with `--remap-path-prefix`.
/// - A run holds a *session's marker* from its start to its end. A start that finds one that no
///   live run holds tells on the standard error how that run ended: by a panic, with its report,
///   or without a word, an abort, the system's killer of memory, a power cut, a second
///   termination signal.
/// - The file `hours` counts the seconds that the runs lasted, the runs and the crashes, which the
///   quality bar's hours without a crash read.
///
/// A run ended by a panic or by a second termination signal leaves its marker: the next start
/// counts it as a crash. What fails in the crash directory is told on the standard error, and the
/// run goes on. A program run by a test or a tool is given its own `XDG_STATE_HOME`, or it writes
/// in the user's.
///
/// On Linux, it needs a Wayland compositor, which `WAYLAND_DISPLAY` names, else an X server, which
/// `DISPLAY` names, and a GPU with Vulkan; elsewhere, it has no windowing layer yet, and ends with
/// [`EXIT_HOST_FAILED`].
pub fn run(look: Look, window: WindowSettings, app: impl App) -> ! {
    std::process::exit(host::run(look, window, app))
}
