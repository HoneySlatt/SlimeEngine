//! The application's side of the host: its look, its window's settings and requests, and the calls
//! that the host makes of it (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`).

use std::error::Error;

use uwui::text::Fonts;
use uwui::theme::{StyleSheet, Theme};
use uwui::tree::Ui;
use uwui_windowing::Activation;

/// What an application draws in (`docs/adr/0226-an-application-started-in-one-call-each-root-given-its-look.md`):
/// its fonts, then its style sheets and its themes, in their order, which the host gives the
/// interface before [`App::start`]. An application that declares tokens of its own declares them
/// in `start` and sets its style again ([`Ui::set_style`]).
pub struct Look {
    /// The fonts that the interface lays its text out with.
    pub fonts: Fonts,
    /// The style sheets, the last one's rules winning.
    pub sheets: Vec<StyleSheet>,
    /// The themes, stacked in their order.
    pub themes: Vec<Theme>,
}

/// How the window opens: its title, its identity, its size, its minimum size and whether it fills
/// the screen; and the application's version, which its crash reports name. Sizes are in logical
/// pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowSettings {
    title: String,
    app_id: String,
    pub(crate) version: Option<String>,
    size: [u32; 2],
    min_size: Option<[u32; 2]>,
    fullscreen: bool,
}

impl WindowSettings {
    /// A window of 800 × 600 logical pixels, titled `title`, of the application named `app_id`.
    ///
    /// The identity names the application to the system, which groups its windows and finds its
    /// icon and its name by it: on Wayland, the window's app id, on X11 its class, which matches
    /// the application's desktop file, by reverse domain name by convention, `org.example.Notes`;
    /// Windows groups the windows by their program, and takes none.
    /// It names the
    /// application's crash directory too ([`run`](crate::run)).
    pub fn new(title: impl Into<String>, app_id: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            app_id: app_id.into(),
            version: None,
            size: [800, 600],
            min_size: None,
            fullscreen: false,
        }
    }

    /// The window's size as it opens, where the compositor leaves it to the application; a side of
    /// zero counts as one.
    #[must_use]
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.size = [width.max(1), height.max(1)];
        self
    }

    /// The smallest that the person may make the window.
    #[must_use]
    pub fn min_size(mut self, width: u32, height: u32) -> Self {
        self.min_size = Some([width, height]);
        self
    }

    /// Whether the window opens filling the screen, without borders.
    #[must_use]
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }

    /// The application's version, which its crash reports name, as Cargo gives it to the
    /// application's crate: `.version(env!("CARGO_PKG_VERSION"))`. None by default, which the
    /// reports say.
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

/// The application's window, its state as the compositor last gave it and the requests that the
/// application makes of it. A request is a wish: the compositor may refuse it, and the state shows
/// what it chose. The host makes the requests after each [`App::step`] and [`App::start`].
#[derive(Debug)]
pub struct Window {
    title: String,
    app_id: String,
    size: [u32; 2],
    min_size: Option<[u32; 2]>,
    fullscreen: bool,
    closing: bool,
    /// The requests made since the host last took them.
    requests: Requests,
}

/// The requests that the application made since the host last took them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Requests {
    /// The title changed.
    pub(crate) title: bool,
    /// The minimum size changed.
    pub(crate) min_size: bool,
    /// The application asked to fill the screen, or to stop.
    pub(crate) fullscreen: Option<bool>,
    /// The application asked the window to be brought to the front, or to ask for attention: the
    /// last request made.
    pub(crate) activation: Option<Activation>,
    /// The application asked the GPU's device lost, on purpose.
    pub(crate) lose_device: bool,
    /// The application asked the window's surface given up, on purpose.
    pub(crate) lose_surface: bool,
}

impl Window {
    pub(crate) fn new(settings: WindowSettings) -> Self {
        let WindowSettings {
            title,
            app_id,
            version: _,
            size,
            min_size,
            fullscreen,
        } = settings;
        Self {
            title,
            app_id,
            size,
            min_size,
            fullscreen: false,
            closing: false,
            requests: Requests {
                fullscreen: fullscreen.then_some(true),
                ..Requests::default()
            },
        }
    }

    /// The window's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Gives the window `title`.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if title != self.title {
            self.title = title;
            self.requests.title = true;
        }
    }

    /// The application's identity, as [`WindowSettings::new`] gave it.
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// The window's size in logical pixels: the compositor's last choice, or the size that the
    /// settings gave until it chose.
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// The smallest that the person may make the window, in logical pixels.
    pub fn min_size(&self) -> Option<[u32; 2]> {
        self.min_size
    }

    /// Sets the smallest that the person may make the window, in logical pixels; none for no
    /// bound.
    pub fn set_min_size(&mut self, size: Option<[u32; 2]>) {
        if size != self.min_size {
            self.min_size = size;
            self.requests.min_size = true;
        }
    }

    /// Whether the window fills the screen, as the compositor last said.
    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Asks the compositor to fill the screen with the window, without borders, or to stop.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.requests.fullscreen = Some(fullscreen);
    }

    /// Asks the system to bring the window to the front and give it the focus. The system chooses:
    /// it may refuse a window that the person is not using, so that it does not take the keys from
    /// another program, and show it as asking for attention instead. On Wayland, the request names
    /// the person's last press of a key or a button in the window, a touch, or the keyboard's
    /// focus given to it, and does nothing where the compositor gives no `xdg_activation_v1`, as Weston.
    /// On X11, through `_NET_ACTIVE_WINDOW`, naming the person's last press of a button; nothing
    /// without a window manager. On Windows, through `SetForegroundWindow`, which Windows refuses
    /// to a program that the person did not last use: the window then flashes in the task bar.
    pub fn bring_to_front(&mut self) {
        self.requests.activation = Some(Activation::Front);
    }

    /// Asks the system to show that the window wants the person's attention, as its entry in a
    /// task bar marked, without taking the focus. On Wayland, through `xdg_activation_v1`, which
    /// some compositors read as a request to bring it to the front; nothing where the compositor
    /// does not give it. On X11, through `_NET_WM_STATE_DEMANDS_ATTENTION`. On Windows, through
    /// `FlashWindowEx`, its entry in the task bar flashing until the window comes to the front.
    pub fn request_attention(&mut self) {
        self.requests.activation = Some(Activation::Attention);
    }

    /// Loses the window's GPU device on purpose in the next frame, once drawn and before its
    /// submission, as a reset of the driver found there would: for the tests and the tools that
    /// check how an application goes on after a lost device. The host drops that frame's image,
    /// opens a new device and draws the window again, the application told nothing; the
    /// fourth device lost within five minutes ends the run with
    /// [`EXIT_HOST_FAILED`](crate::EXIT_HOST_FAILED)
    /// (`docs/adr/0305-uwuis-desktop-host-makes-its-device-and-surface-again-a-programs-images-made-again-from-their-source.md`).
    pub fn lose_device(&mut self) {
        self.requests.lose_device = true;
    }

    /// Gives the window's surface up on purpose after this step, as a surface lost each time it
    /// is made would be: for the tests and the tools that check how an application goes on after a
    /// lost surface. The window holds its frames, and the host makes the surface again a second
    /// later, or at once when the window is resized or its scale changes; the fourth surface given
    /// up within five minutes ends the run with [`EXIT_HOST_FAILED`](crate::EXIT_HOST_FAILED)
    /// (`docs/adr/0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md`).
    pub fn lose_surface(&mut self) {
        self.requests.lose_surface = true;
    }

    /// Closes the window, as the application chose: the run ends after this step, through
    /// [`App::stop`], without asking [`App::close_requested`].
    pub fn close(&mut self) {
        self.closing = true;
    }

    /// Whether the application closed the window.
    pub(crate) fn closing(&self) -> bool {
        self.closing
    }

    /// The requests made since the last call.
    pub(crate) fn take_requests(&mut self) -> Requests {
        std::mem::take(&mut self.requests)
    }

    /// The compositor's choice of the window's size and of its filling the screen.
    pub(crate) fn configured(&mut self, size: [u32; 2], fullscreen: bool) {
        self.size = size;
        self.fullscreen = fullscreen;
    }
}

/// Whether the window closes, when the person or the system asks ([`App::close_requested`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(
    clippy::exhaustive_enums,
    reason = "an application keeps its window open or lets it close"
)]
pub enum Close {
    /// It closes: the run ends, through [`App::stop`].
    #[default]
    Allow,
    /// It stays open, as an editor with unsaved work asks first. A termination signal asks once:
    /// any signal after it, half a second or more later, ends the process at once, without
    /// another call ([`EXIT_TERMINATED`](crate::EXIT_TERMINATED)).
    Keep,
}

/// An application on uwui, which the host runs ([`run`](crate::run)).
///
/// The host calls [`start`](Self::start) once its window and interface are made, then
/// [`step`](Self::step) each time it wakes, after the events that came and before a frame;
/// [`close_requested`](Self::close_requested) when the person or the system asks the window to
/// close; and [`stop`](Self::stop) once, as the run ends, after a failure of the host too. Nothing runs while nothing happens: a step
/// runs on an event, a wake of the interface's waker
/// ([`Ui::waker`](uwui::tree::Ui::waker)), a frame of the interface, or a time that it waits for.
pub trait App {
    /// Builds the application's roots and reads what it needs. The interface's style is the look's,
    /// and its target, [`TargetId::MAIN`](uwui::host::TargetId::MAIN), the window.
    ///
    /// # Errors
    ///
    /// What keeps the application from starting, which the host reports before it ends with
    /// [`EXIT_START_FAILED`](crate::EXIT_START_FAILED).
    fn start(&mut self, ui: &mut Ui, window: &mut Window) -> Result<(), Box<dyn Error>>;

    /// Reads the interface's actions ([`Ui::take_actions`]) and serves them, builds what changed,
    /// and makes the window's requests.
    fn step(&mut self, ui: &mut Ui, window: &mut Window);

    /// The person or the system asked the window to close: its button, a shortcut of the system,
    /// or a termination signal, SIGTERM, SIGINT or SIGHUP. By default, it closes. Only the first
    /// termination signal is asked: one after it, half a second or more later, however late, ends
    /// the process at once, so that an application that keeps its window or no longer returns
    /// still ends ([`EXIT_TERMINATED`](crate::EXIT_TERMINATED)).
    fn close_requested(&mut self, ui: &mut Ui, window: &mut Window) -> Close {
        let _ = (ui, window);
        Close::Allow
    }

    /// The run ends: the window closed, the application allowed it or closed it, or the host
    /// failed, its connection to the compositor broken, or its device or its surface lost more
    /// often than it makes them again, the window then perhaps gone. The last call, so that the
    /// application may keep its work; the interface is still whole. By default, nothing.
    ///
    /// Never called after a panic on the host's thread, in the application's code or in uwui's:
    /// the state that panicked is never handed back to be kept, where it could write over the last
    /// good copy of the person's work. The application is called no more, its value never dropped,
    /// and the process ends with [`EXIT_PANICKED`](crate::EXIT_PANICKED)
    /// (`docs/adr/0298-a-panic-ends-a-run-without-stop-the-same-rule-in-both-hosts.md`). What
    /// survives a panic is what the application kept before, as an editor's journal of its steps.
    /// Nor is it called when a second termination signal ends the process at once
    /// ([`EXIT_TERMINATED`](crate::EXIT_TERMINATED)).
    fn stop(&mut self, ui: &mut Ui) {
        let _ = ui;
    }
}
