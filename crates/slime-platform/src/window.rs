//! The application window.

use std::sync::Arc;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// The settings of the window that [`run_event_loop`](crate::run_event_loop) creates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowConfig {
    /// The title the compositor shows, for example in a title bar or a task switcher. Default:
    /// `"SlimeEngine"`.
    pub title: String,
    /// The requested width of the window's content area, in logical pixels: the compositor
    /// multiplies it by the display's scale factor. Compositors may choose another size; tiling
    /// compositors such as niri usually do. Default: 1280.
    pub width: u32,
    /// The requested height of the window's content area, in logical pixels. Default: 720.
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
