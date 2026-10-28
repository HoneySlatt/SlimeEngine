//! The application window.

use std::sync::{Arc, Mutex, PoisonError};

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use slime_input::CursorShape;
use winit::dpi::PhysicalPosition;

use crate::Waker;

/// The settings of the window that [`run_event_loop`](crate::run_event_loop) creates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowConfig {
    /// The title the compositor shows, for example in a title bar or a task switcher. It must not
    /// contain NUL bytes; on Wayland, only its first 1024 bytes are shown. Default:
    /// `"SlimeEngine"`.
    pub title: String,
    /// The requested width of the window's content area, in logical pixels, from 1 to 16 384: the
    /// compositor multiplies it by the display's scale factor, and does not limit the result to the
    /// display. On X11, the result is limited to 65 535 physical pixels, the most the X11 protocol
    /// allows; the height too. Compositors may choose another size; tiling compositors such as niri
    /// usually do. Default: 1280.
    pub width: u32,
    /// The requested height of the window's content area, in logical pixels, from 1 to 16 384.
    /// Default: 720.
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "SlimeEngine".to_owned(),
            width: 1280,
            height: 720,
        }
    }
}

/// Where the cursor last was over the window, and where the window holds it, if it does.
#[derive(Debug, Default)]
struct Cursor {
    position: Option<PhysicalPosition<f64>>,
    held: Option<Held>,
}

/// How the window holds the cursor ([`Window::set_cursor_held`]).
#[derive(Clone, Copy, Debug)]
struct Held {
    /// Where the cursor was as it was held, in whole physical pixels of the content area, so that
    /// a position put back is the one reported then.
    position: PhysicalPosition<f64>,
    /// Whether the cursor is put back there after each motion, and as it is let go, on X11, which
    /// cannot lock it.
    warp: bool,
}

/// A size in physical pixels, the pixels of the display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhysicalSize {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

/// The window of the application.
///
/// A `Window` is a cheap handle: clones refer to the same window, which is destroyed when the
/// last clone is dropped. The event loop drops its own handle when it ends, so an application
/// that keeps a clone, for example inside a drawing surface, should drop it when it shuts down.
/// On Windows, the windowing library destroys windows from inside the event loop, so the window
/// still open when the loop ends is hidden then, and released only when the process exits.
///
/// It cannot yet go fullscreen. It holds the cursor, hidden, while the mouse's raw motion turns a
/// camera ([`Window::set_cursor_held`]), opens the system's dialog to choose a file
/// ([`Window::choose_file`]), changes its title, sets the pointer's shape and places the input
/// method, which it allows while a field takes text, and gives a [`Waker`] of its event loop.
///
/// It implements the [`raw-window-handle`](raw_window_handle) traits, through which graphics
/// libraries draw into a window without knowing the windowing library that created it.
#[derive(Clone, Debug)]
pub struct Window {
    inner: Arc<winit::window::Window>,
    waker: Waker,
    /// Where the cursor last was, and whether the window holds it.
    cursor: Arc<Mutex<Cursor>>,
    /// The updates of the tree that screen readers read, which the event loop gives the adapter
    /// once the handler's call returns: the adapter stays on the thread of the event loop.
    #[cfg(feature = "accessibility")]
    accessibility: Arc<std::sync::Mutex<Vec<accesskit::TreeUpdate>>>,
}

impl Window {
    pub(crate) fn new(window: winit::window::Window, waker: Waker) -> Self {
        Self {
            inner: Arc::new(window),
            waker,
            cursor: Arc::default(),
            #[cfg(feature = "accessibility")]
            accessibility: Arc::default(),
        }
    }

    /// winit's window.
    #[cfg(feature = "accessibility")]
    pub(crate) fn winit(&self) -> &winit::window::Window {
        &self.inner
    }

    /// Gives the tree that screen readers read `update`, as the window's handler's call returns:
    /// the whole tree, with its [`TreeInfo`](accesskit::TreeInfo), once one was asked for
    /// ([`Accessibility::TreeRequested`](crate::Accessibility::TreeRequested)), then what changed.
    /// An update while no screen reader reads the window is dropped, and so is one of what
    /// changed given after a screen reader asked for the tree and before the whole tree, a tree
    /// of the window's node alone, numbered 0, standing in for it until the whole tree comes. An
    /// update that names a node that neither it nor the tree holds, or that puts the focus on one,
    /// makes AccessKit panic.
    #[cfg(feature = "accessibility")]
    pub fn update_accessibility(&self, update: accesskit::TreeUpdate) {
        self.accessibility
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(update);
    }

    /// The updates given since they were last taken.
    #[cfg(feature = "accessibility")]
    pub(crate) fn take_accessibility(&self) -> Vec<accesskit::TreeUpdate> {
        std::mem::take(
            &mut self
                .accessibility
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// The current size of the window's content area, in physical pixels. It can be zero, for
    /// example while the window is minimized.
    pub fn inner_size(&self) -> PhysicalSize {
        let size = self.inner.inner_size();
        PhysicalSize {
            width: size.width,
            height: size.height,
        }
    }

    /// The number of physical pixels per logical pixel for this window, set by the display's
    /// scaling. It changes when the window moves to another display or the scaling changes.
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// The scheme of colors that the system prefers, dark or light, where the platform tells it:
    /// on Windows; none on Linux, where winit reads no desktop's settings
    /// (`docs/adr/0149-the-looks-foundations.md`). [`Event::ColorSchemeChanged`](crate::Event)
    /// says when it changes.
    pub fn color_scheme(&self) -> Option<ColorScheme> {
        self.inner.theme().map(ColorScheme::from)
    }

    /// Asks the windowing system for a chance to draw: the event loop then calls
    /// [`EventHandler::redraw_requested`](crate::EventHandler::redraw_requested), usually during
    /// its next iteration. Requests made before that call are merged into one.
    ///
    /// The windowing system also asks for redraws on its own, for example while the window is being
    /// resized.
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }

    /// Changes the title that the compositor shows, as [`WindowConfig::title`] says.
    ///
    /// # Panics
    ///
    /// If `title` contains a NUL byte.
    pub fn set_title(&self, title: &str) {
        assert!(
            !title.contains('\0'),
            "the window title must not contain NUL bytes"
        );
        self.inner.set_title(title);
    }

    /// A [`Waker`] of the event loop that runs the window, which another thread wakes it with.
    pub fn waker(&self) -> Waker {
        self.waker.clone()
    }

    /// Opens the system's dialog to choose a file to open, as `choice` says, the window its parent,
    /// on a thread of its own: the event loop goes on meanwhile, and the answer arrives as
    /// [`Event::FileChosen`](crate::Event::FileChosen) with the number given here. On Linux, the
    /// desktop's portal shows it, through D-Bus's library, which the process loads then; `zenity`
    /// shows it where no portal answers, and no dialog shows where neither does, which answers as
    /// a cancel, as a text of `choice` that holds a NUL character does. A dialog of the portal is modal to the window where the compositor gives Wayland's
    /// `xdg-foreign` protocol. On Windows, the system's own dialog shows it. The program cannot
    /// close the dialog: only the user does.
    pub fn choose_file(&self, choice: crate::FileChoice) -> crate::DialogId {
        crate::dialog::choose_file(self.clone(), choice, self.waker.proxy.clone())
    }

    /// Holds the cursor, hidden, where it is, so that the mouse's raw motion
    /// ([`Event::RawMotion`](crate::Event::RawMotion)) moves what the program moves with it, a
    /// camera, without the cursor meeting an edge; or lets it go, shown again where it was held.
    /// On Wayland and Windows the windowing system locks it; on X11, which cannot, it is confined to
    /// the window and put back where it was held after each motion, which then reports no
    /// [`Event::CursorMoved`](crate::Event::CursorMoved). Holding it again, or letting go of a
    /// cursor that is free, does nothing. A program lets it go as the button that holds it is let
    /// go; the window lets it go as it loses the focus, before
    /// [`Event::Focused`](crate::Event::Focused) reports it, so that it is not kept from the
    /// window that the user went to.
    ///
    /// # Errors
    ///
    /// [`PlatformError::HoldCursor`](crate::PlatformError::HoldCursor) when the windowing system
    /// refuses to hold it: the cursor stays free and shown, and the program can say so.
    pub fn set_cursor_held(&self, held: bool) -> Result<(), crate::PlatformError> {
        use winit::window::CursorGrabMode;

        // The lock is never held across winit's calls: on Windows, a call from another thread
        // waits for the event loop's thread, which may be waiting for the lock in `cursor_moved`.
        let (was, position) = {
            let mut cursor = self.lock_cursor();
            if held == cursor.held.is_some() {
                return Ok(());
            }
            (cursor.held.take(), cursor.position)
        };
        if let Some(at) = was {
            // Locked, it shows again where it was; confined, it is put back there.
            if at.warp {
                let _ = self.inner.set_cursor_position(at.position);
            }
            if let Err(error) = self.inner.set_cursor_grab(CursorGrabMode::None) {
                tracing::warn!(%error, "the cursor's hold cannot be let go");
            }
            self.inner.set_cursor_visible(true);
            return Ok(());
        }
        let position = position.unwrap_or_else(|| {
            let size = self.inner.inner_size();
            PhysicalPosition::new(f64::from(size.width) / 2.0, f64::from(size.height) / 2.0)
        });
        let position = PhysicalPosition::new(position.x.round(), position.y.round());
        let warp = match self.inner.set_cursor_grab(CursorGrabMode::Locked) {
            Ok(()) => false,
            // X11 cannot lock it: confined, and put back where it was held after each motion.
            Err(winit::error::ExternalError::NotSupported(_)) => {
                self.inner
                    .set_cursor_grab(CursorGrabMode::Confined)
                    .map_err(|error| crate::PlatformError::HoldCursor(Box::new(error)))?;
                true
            }
            Err(error) => return Err(crate::PlatformError::HoldCursor(Box::new(error))),
        };
        self.inner.set_cursor_visible(false);
        self.lock_cursor().held = Some(Held { position, warp });
        Ok(())
    }

    fn lock_cursor(&self) -> std::sync::MutexGuard<'_, Cursor> {
        self.cursor.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether the window holds the cursor ([`Window::set_cursor_held`]).
    pub fn cursor_held(&self) -> bool {
        self.lock_cursor().held.is_some()
    }

    /// Notes that the cursor moved to `position`, and gives whether the motion is reported: not
    /// while the window holds the cursor, where on X11 it is put back where it was held.
    pub(crate) fn cursor_moved(&self, position: PhysicalPosition<f64>) -> bool {
        let held = {
            let mut cursor = self.lock_cursor();
            if cursor.held.is_none() {
                cursor.position = Some(position);
            }
            cursor.held
        };
        let Some(held) = held else {
            return true;
        };
        if held.warp && position != held.position {
            let _ = self.inner.set_cursor_position(held.position);
        }
        false
    }

    /// Allows the input method, or forbids it: while it is allowed, the keys that it reads compose
    /// text, which arrives as [`Event::Ime`](crate::Event::Ime) rather than as keys' text. Allow
    /// it while a field of text holds the focus, and only then, since it takes keys that a game
    /// reads. It is forbidden when the window opens.
    pub fn set_ime_allowed(&self, allowed: bool) {
        self.inner.set_ime_allowed(allowed);
    }

    /// Tells the input method where the text being composed lies, the caret's box, in physical
    /// pixels from the content area's top left corner: it places its window of candidates beside
    /// it, not over it.
    pub fn set_ime_area(&self, x: f32, y: f32, width: f32, height: f32) {
        self.inner.set_ime_cursor_area(
            winit::dpi::PhysicalPosition::new(x, y),
            winit::dpi::PhysicalSize::new(width, height),
        );
    }

    /// Shows `shape` as the pointer over the window.
    pub fn set_cursor(&self, shape: CursorShape) {
        self.inner.set_cursor(crate::input::cursor_icon(shape));
    }

    #[cfg(windows)]
    pub(crate) fn hide(&self) {
        self.inner.set_visible(false);
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

/// The scheme of colors that the system prefers ([`Window::color_scheme`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorScheme {
    /// Light text on dark surfaces.
    Dark,
    /// Dark text on light surfaces.
    Light,
}

impl From<winit::window::Theme> for ColorScheme {
    fn from(theme: winit::window::Theme) -> Self {
        match theme {
            winit::window::Theme::Dark => Self::Dark,
            winit::window::Theme::Light => Self::Light,
        }
    }
}
