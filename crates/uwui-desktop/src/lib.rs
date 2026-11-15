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
//! The host is a *core*, which knows no platform and is tested without a window, under a layer for
//! each platform. The core holds the pacing of frames and the rest, the clock given to it
//! (`pacing`), the anchor of the events' times (`anchor`), the wakes of other threads and the
//! termination signals (`wake`), and the application's run (`session`). Wayland's layer
//! (`wayland`) is the only one: X11, Windows and macOS have theirs at M6d.7's next parts, and
//! [`run`] there says that it has none and ends.
//!
//! No type of a platform's library appears in its public API.

// The core and the application's side, which only Wayland's layer drives yet: elsewhere they are
// compiled and tested, and used by no layer.
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod anchor;
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod app;
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod pacing;
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod session;
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "no windowing layer drives the core here yet")
)]
mod wake;

#[cfg(target_os = "linux")]
mod clock;
#[cfg(target_os = "linux")]
mod gpu;
#[cfg(target_os = "linux")]
mod wayland;

pub use app::{App, Close, Look, Window, WindowSettings};

/// The process's exit status when the host cannot start, a compositor or a GPU missing, or stops
/// on a failure of its own: the connection to the compositor broken, the device lost. What failed
/// is told on the standard error.
pub const EXIT_HOST_FAILED: i32 = 1;

/// The process's exit status when the application's look does not read or its
/// [`App::start`] fails, which is told on the standard error.
pub const EXIT_START_FAILED: i32 = 2;

/// Runs `app` in a window opened with `window`, in `look`, and ends the process when the window
/// closes, with status 0; never returns, on any platform.
///
/// The window closes when the application allows a closing that the person or the system asks
/// ([`App::close_requested`]), a termination signal among them, or closes it itself
/// ([`Window::close`]). A failure ends the process with [`EXIT_HOST_FAILED`] or
/// [`EXIT_START_FAILED`].
///
/// On Linux, it needs a Wayland compositor, which `WAYLAND_DISPLAY` names, and a GPU with Vulkan;
/// elsewhere, it has no windowing layer yet, and ends with [`EXIT_HOST_FAILED`].
pub fn run(look: Look, window: WindowSettings, app: impl App) -> ! {
    #[cfg(target_os = "linux")]
    let status = wayland::run(look, window, app);
    #[cfg(not(target_os = "linux"))]
    let status = {
        drop((look, window, app));
        eprintln!("uwui's desktop host has no windowing layer for this system yet");
        EXIT_HOST_FAILED
    };
    std::process::exit(status)
}
