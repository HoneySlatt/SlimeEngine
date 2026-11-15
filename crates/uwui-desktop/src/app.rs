//! The application's side of the host: its look, its window's settings and requests, and the calls
//! that the host makes of it (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`).

use std::error::Error;

use uwui::text::Fonts;
use uwui::theme::{StyleSheet, Theme};
use uwui::tree::Ui;

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
/// the screen. Sizes are in logical pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowSettings {
    title: String,
    app_id: String,
    size: [u32; 2],
    min_size: Option<[u32; 2]>,
    fullscreen: bool,
}

impl WindowSettings {
    /// A window of 800 × 600 logical pixels, titled `title`, of the application named `app_id`.
    ///
    /// The identity names the application to the system, which groups its windows and finds its
    /// icon and its name by it: on Wayland, the window's app id, which matches the application's
    /// desktop file, by reverse domain name by convention, `org.example.Notes`.
    pub fn new(title: impl Into<String>, app_id: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            app_id: app_id.into(),
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
}

impl Window {
    pub(crate) fn new(settings: WindowSettings) -> Self {
        let WindowSettings {
            title,
            app_id,
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
pub enum Close {
    /// It closes: the run ends, through [`App::stop`].
    #[default]
    Allow,
    /// It stays open, as an editor with unsaved work asks first.
    Keep,
}

/// An application on uwui, which the host runs ([`run`](crate::run)).
///
/// The host calls [`start`](Self::start) once its window and interface are made, then
/// [`step`](Self::step) each time it wakes, after the events that came and before a frame;
/// [`close_requested`](Self::close_requested) when the person or the system asks the window to
/// close; and [`stop`](Self::stop) once, as the run ends. Nothing runs while nothing happens: a step
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
    /// or a termination signal, SIGTERM, SIGINT or SIGHUP. By default, it closes.
    fn close_requested(&mut self, ui: &mut Ui, window: &mut Window) -> Close {
        let _ = (ui, window);
        Close::Allow
    }

    /// The run ends: the window is still open, and closes after it. By default, nothing.
    fn stop(&mut self, ui: &mut Ui) {
        let _ = ui;
    }
}
