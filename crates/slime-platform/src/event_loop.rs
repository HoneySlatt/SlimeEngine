//! The event loop: creating the window and calling the handler back when something happens.

use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::error::EventLoopError;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::error::PlatformError;
use crate::event::{Event, translate};
use crate::window::{Window, WindowConfig};

/// The code that [`run_event_loop`] calls back. It decides what happens in the window and when
/// the loop ends.
///
/// The calls come in this order: [`window_created`](Self::window_created) once; then
/// [`event`](Self::event) and [`about_to_wait`](Self::about_to_wait) as events arrive and time
/// passes; then [`exiting`](Self::exiting) once. None of them is called if the window could not
/// be created.
pub trait EventHandler {
    /// The window now exists. Called once, before any other method.
    fn window_created(&mut self, window: &Window);

    /// Something happened to the window.
    fn event(&mut self, event: Event);

    /// Every pending event has been delivered and the loop is about to wait for new ones. The
    /// handler does its periodic work here and says when the loop must wake up next, or that it
    /// must end.
    fn about_to_wait(&mut self) -> LoopControl;

    /// The loop is ending. Called once; the window still exists during the call.
    fn exiting(&mut self);
}

/// What the event loop does after [`EventHandler::about_to_wait`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopControl {
    /// Wait for new events, and wake up at this instant at the latest.
    WaitUntil(Instant),
    /// End the loop: [`EventHandler::exiting`] is called next.
    Exit,
}

/// Connects to the windowing system, creates a window, and runs the event loop until the
/// handler returns [`LoopControl::Exit`].
///
/// The windowing system is chosen from the environment, with no fallback from one to the other:
/// Wayland if `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set to a non-empty value, otherwise X11 if
/// `DISPLAY` is. The windowing library allows one attempt per process: whether it succeeded or
/// not, a second call returns [`PlatformError::SecondEventLoop`].
///
/// # Errors
///
/// - [`PlatformError::Connect`] if no windowing system can be reached.
/// - [`PlatformError::SecondEventLoop`] if this function was already called in this process.
/// - [`PlatformError::CreateWindow`] if the window cannot be created; the handler is then never
///   called.
/// - [`PlatformError::EventLoop`] if the event loop fails while running.
///
/// # Panics
///
/// If the event loop is created from a thread other than the main thread.
pub fn run_event_loop(
    config: &WindowConfig,
    handler: &mut impl EventHandler,
) -> Result<(), PlatformError> {
    let event_loop = EventLoop::new().map_err(|error| match error {
        EventLoopError::RecreationAttempt => PlatformError::SecondEventLoop,
        error => PlatformError::Connect(Box::new(error)),
    })?;
    let mut adapter = WinitAdapter {
        config,
        handler,
        window: None,
        error: None,
    };
    event_loop
        .run_app(&mut adapter)
        .map_err(|error| PlatformError::EventLoop(Box::new(error)))?;
    adapter.error.map_or(Ok(()), Err)
}

/// Adapts winit's callbacks to an [`EventHandler`].
struct WinitAdapter<'a, H> {
    config: &'a WindowConfig,
    handler: &'a mut H,
    window: Option<Window>,
    error: Option<PlatformError>,
}

impl<H: EventHandler> ApplicationHandler for WinitAdapter<'_, H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // winit sends `resumed` once at startup on desktop platforms; mobile platforms send it
        // again after a suspension, when the window already exists.
        if self.window.is_some() {
            return;
        }
        let attributes = winit::window::Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height));
        match event_loop.create_window(attributes) {
            Ok(window) => {
                let window = Window::new(window);
                let size = window.inner_size();
                tracing::info!(
                    backend = backend_name(event_loop),
                    width = size.width,
                    height = size.height,
                    scale_factor = window.scale_factor(),
                    "window created"
                );
                self.handler.window_created(&window);
                self.window = Some(window);
            }
            Err(error) => {
                self.error = Some(PlatformError::CreateWindow(Box::new(error)));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.window.is_none() {
            return;
        }
        if let Some(event) = translate(event) {
            self.handler.event(event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            return;
        }
        match self.handler.about_to_wait() {
            LoopControl::WaitUntil(instant) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(instant));
            }
            LoopControl::Exit => event_loop.exit(),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            self.handler.exiting();
        }
        // Drop the window while the connection to the windowing system is still open.
        self.window = None;
    }
}

#[cfg(target_os = "linux")]
fn backend_name(event_loop: &ActiveEventLoop) -> &'static str {
    use winit::platform::wayland::ActiveEventLoopExtWayland;
    if event_loop.is_wayland() {
        "wayland"
    } else {
        "x11"
    }
}

#[cfg(not(target_os = "linux"))]
fn backend_name(_event_loop: &ActiveEventLoop) -> &'static str {
    std::env::consts::OS
}
