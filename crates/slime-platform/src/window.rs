//! The application window.

use std::sync::Arc;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

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
/// It cannot yet go fullscreen, grab or hide the cursor, change its title once created, or report
/// the mouse's raw motion, which games that turn a camera with the mouse need.
///
/// It implements the [`raw-window-handle`](raw_window_handle) traits, through which graphics
/// libraries draw into a window without knowing the windowing library that created it.
#[derive(Clone, Debug)]
pub struct Window {
    inner: Arc<winit::window::Window>,
}

impl Window {
    pub(crate) fn new(window: winit::window::Window) -> Self {
        Self {
            inner: Arc::new(window),
        }
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

    /// Asks the windowing system for a chance to draw: the event loop then calls
    /// [`EventHandler::redraw_requested`](crate::EventHandler::redraw_requested), usually during
    /// its next iteration. Requests made before that call are merged into one.
    ///
    /// The windowing system also asks for redraws on its own, for example while the window is being
    /// resized.
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
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
