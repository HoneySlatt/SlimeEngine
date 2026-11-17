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
//! pacing of frames and the rest, the clock given to it (`pacing`), and the application's run
//! (`session`); `host` drives them at each turn, and `gpu` draws each frame into the window's image.
//! The layer gives Wayland's window alone yet: elsewhere [`run`] says that it has none and ends.
//!
//! No type of a platform's library appears in its public API.
// A public enumeration may gain cases without breaking a program; one closed by nature says why
// it is not marked (ADR-0282).
#![warn(clippy::exhaustive_enums)]

mod app;
mod gpu;
mod host;
mod pacing;
mod session;

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
    std::process::exit(host::run(look, window, app))
}
