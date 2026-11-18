//! An application's run, without a platform: its interface, its window and the calls that the host
//! makes of it, in their order. Part of the host's core, tested without a window; a platform's
//! layer drives it.

use std::error::Error;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::task::Waker;

use uwui::DocumentErrors;
use uwui::host::TargetId;
use uwui::time::{FrameShown, FrameSource, Instant};
use uwui::tree::Ui;

use crate::app::{App, Close, Look, Window};

/// Why an application did not start.
#[derive(Debug)]
pub(crate) enum StartError {
    /// Its look's documents do not read.
    Look(DocumentErrors),
    /// Its [`App::start`] failed.
    App(Box<dyn Error>),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Look(errors) => write!(f, "{errors}"),
            Self::App(error) => write!(f, "{error}"),
        }
    }
}

impl Error for StartError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Look(_) => None,
            Self::App(error) => error.source(),
        }
    }
}

/// A value that a panic's unwinding forgets rather than drops: an application's state after a
/// panic, whose `Drop` is its code too, which may write the state that panicked
/// (`docs/adr/0306-uwuis-crash-reports-a-marker-held-by-a-lock-hours-counted-a-panic-ends-with-101.md`).
/// Dropped as any value otherwise.
pub(crate) struct Unwound<T>(Option<T>);

impl<T> Unwound<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(Some(value))
    }

    /// The value, which leaves the guard.
    pub(crate) fn into_inner(mut self) -> T {
        self.0
            .take()
            .expect("an unwound value is held until it leaves")
    }
}

impl<T> Deref for Unwound<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
            .as_ref()
            .expect("an unwound value is held until it leaves")
    }
}

impl<T> DerefMut for Unwound<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0
            .as_mut()
            .expect("an unwound value is held until it leaves")
    }
}

impl<T> Drop for Unwound<T> {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(value) = self.0.take()
        {
            std::mem::forget(value);
        }
    }
}

/// An application's run: its interface, its window and the application, until it ends.
pub(crate) struct Session<A> {
    pub(crate) ui: Ui,
    pub(crate) window: Window,
    app: A,
    /// The window closes: the application allowed it, or closed it itself.
    ending: bool,
}

impl<A: App> Session<A> {
    /// Makes the interface in `look`, given `waker` and the host's time `now`, for `app` in
    /// `window`, which [`start`](Self::start) then starts: the host holds the session before the
    /// application's first call, and a panic as the interface is made forgets the application,
    /// so that a panic never drops it.
    pub(crate) fn new(
        look: Look,
        window: Window,
        app: A,
        waker: Waker,
        now: Instant,
    ) -> Result<Self, StartError> {
        let app = Unwound::new(app);
        let Look {
            fonts,
            sheets,
            themes,
        } = look;
        let mut ui = Ui::new(fonts);
        ui.set_style(&sheets, &themes).map_err(StartError::Look)?;
        ui.set_waker(waker);
        // What the application adds as it starts, a notification, waits from the host's time,
        // not from the clock's zero.
        ui.set_frame_shown(
            TargetId::MAIN,
            FrameShown {
                at: now,
                source: FrameSource::Started,
            },
        );
        Ok(Self {
            ui,
            window,
            app: app.into_inner(),
            ending: false,
        })
    }

    /// Starts the application: its first call.
    pub(crate) fn start(&mut self) -> Result<(), StartError> {
        self.app
            .start(&mut self.ui, &mut self.window)
            .map_err(StartError::App)
    }

    /// The application's step, after the events that came and before a frame.
    pub(crate) fn step(&mut self) {
        self.app.step(&mut self.ui, &mut self.window);
        self.ending |= self.window.closing();
    }

    /// The person or the system asked the window to close: the application chooses.
    pub(crate) fn close_requested(&mut self) {
        if self.ending {
            return;
        }
        if self.app.close_requested(&mut self.ui, &mut self.window) == Close::Allow {
            self.ending = true;
        }
        self.ending |= self.window.closing();
    }

    /// Whether the run ends.
    pub(crate) fn ending(&self) -> bool {
        self.ending
    }

    /// Ends the run: the application's last call.
    pub(crate) fn stop(&mut self) {
        self.app.stop(&mut self.ui);
    }
}

#[cfg(test)]
mod tests {
    use uwui::text::Fonts;
    use uwui::theme::StyleSheet;

    use super::*;
    use crate::app::WindowSettings;

    /// An application that writes down each call made of it.
    #[derive(Default)]
    struct Told {
        calls: Vec<&'static str>,
        fails: bool,
        keeps: bool,
        closes_in_step: bool,
    }

    impl App for &mut Told {
        fn start(&mut self, _: &mut Ui, window: &mut Window) -> Result<(), Box<dyn Error>> {
            self.calls.push("start");
            window.set_title("started");
            if self.fails {
                return Err("no data".into());
            }
            Ok(())
        }

        fn step(&mut self, _: &mut Ui, window: &mut Window) {
            self.calls.push("step");
            if self.closes_in_step {
                window.close();
            }
        }

        fn close_requested(&mut self, _: &mut Ui, _: &mut Window) -> Close {
            self.calls.push("close_requested");
            if self.keeps {
                Close::Keep
            } else {
                Close::Allow
            }
        }

        fn stop(&mut self, _: &mut Ui) {
            self.calls.push("stop");
        }
    }

    fn look() -> Look {
        Look {
            fonts: Fonts::new(),
            sheets: Vec::new(),
            themes: Vec::new(),
        }
    }

    fn start(app: &mut Told) -> Result<Session<&mut Told>, StartError> {
        let mut session = Session::new(
            look(),
            Window::new(WindowSettings::new("a window", "org.example.Test")),
            app,
            Waker::noop().clone(),
            Instant::from_nanos(7),
        )?;
        session.start()?;
        Ok(session)
    }

    /// Writes down whether it was dropped.
    struct Dropped<'a>(&'a std::cell::Cell<bool>);

    impl Drop for Dropped<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    #[test]
    fn a_panics_unwinding_forgets_what_it_guards() {
        let dropped = std::cell::Cell::new(false);
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guarded = Unwound::new(Dropped(&dropped));
            panic!("unwinds");
        }));
        assert!(unwound.is_err());
        assert!(!dropped.get(), "a panic's unwinding dropped the value");
        drop(Unwound::new(Dropped(&dropped)));
        assert!(dropped.get(), "a value was not dropped without a panic");
    }

    #[test]
    fn a_start_that_fails_is_reported_and_never_stopped() {
        let mut app = Told {
            fails: true,
            ..Told::default()
        };
        let error = start(&mut app).err().expect("the start fails");
        assert!(matches!(error, StartError::App(_)));
        assert_eq!(error.to_string(), "no data");
        assert_eq!(app.calls, ["start"]);
    }

    #[test]
    fn a_look_that_does_not_read_fails_the_start_before_the_application() {
        let mut app = Told::default();
        let mut look = look();
        look.sheets
            .push(StyleSheet::from_text("broken.style.kdl", "button {"));
        let error = Session::new(
            look,
            Window::new(WindowSettings::new("a window", "org.example.Test")),
            &mut app,
            Waker::noop().clone(),
            Instant::from_nanos(7),
        )
        .err()
        .expect("the look does not read");
        assert!(matches!(error, StartError::Look(_)));
        assert!(app.calls.is_empty());
    }

    #[test]
    fn a_close_asked_is_the_applications_choice() {
        let mut app = Told {
            keeps: true,
            ..Told::default()
        };
        let mut session = start(&mut app).expect("it starts");
        session.close_requested();
        assert!(!session.ending());
        session.step();
        session.stop();
        assert_eq!(app.calls, ["start", "close_requested", "step", "stop"]);

        let mut app = Told::default();
        let mut session = start(&mut app).expect("it starts");
        session.close_requested();
        assert!(session.ending());
        // Asked again as it ends, the application is not asked twice.
        session.close_requested();
        session.stop();
        assert_eq!(app.calls, ["start", "close_requested", "stop"]);
    }

    #[test]
    fn an_application_that_closes_its_window_ends_without_being_asked() {
        let mut app = Told {
            closes_in_step: true,
            ..Told::default()
        };
        let mut session = start(&mut app).expect("it starts");
        session.step();
        assert!(session.ending());
        session.close_requested();
        session.stop();
        assert_eq!(app.calls, ["start", "step", "stop"]);
    }

    #[test]
    fn the_requests_of_the_start_wait_for_the_host() {
        let mut app = Told::default();
        let mut session = start(&mut app).expect("it starts");
        assert_eq!(session.window.title(), "started");
        let requests = session.window.take_requests();
        assert!(requests.title && !requests.min_size && requests.fullscreen.is_none());
        assert!(!session.window.take_requests().title);
    }

    #[test]
    fn the_windows_activation_is_asked_once_its_last_request_kept() {
        let mut window = Window::new(WindowSettings::new("a window", "org.example.Test"));
        assert_eq!(window.take_requests().activation, None);
        window.request_attention();
        window.bring_to_front();
        assert_eq!(
            window.take_requests().activation,
            Some(uwui_windowing::Activation::Front)
        );
        assert_eq!(window.take_requests().activation, None);
        window.request_attention();
        assert_eq!(
            window.take_requests().activation,
            Some(uwui_windowing::Activation::Attention)
        );
    }

    #[test]
    fn the_interface_has_the_hosts_time_before_the_application_starts() {
        let mut app = Told::default();
        let mut session = start(&mut app).expect("it starts");
        let (_, stats) = session.ui.frame(uwui::host::TargetId::MAIN);
        assert_eq!(
            stats.shown,
            uwui::time::FrameShown {
                at: Instant::from_nanos(7),
                source: uwui::time::FrameSource::Started,
            }
        );
    }
}
