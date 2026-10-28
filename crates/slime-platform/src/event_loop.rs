//! The event loop: creating the window and calling the handler back when something happens.

use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::error::EventLoopError;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::error::PlatformError;
use crate::event::{Event, Translator};
use crate::window::{Window, WindowConfig};

/// The code that [`run_event_loop`] calls back. It decides what happens in the window and when
/// the loop ends.
///
/// The calls come in this order: [`window_created`](Self::window_created) once; then
/// [`event`](Self::event), [`redraw_requested`](Self::redraw_requested) and
/// [`about_to_wait`](Self::about_to_wait) as events arrive and time passes; then
/// [`exiting`](Self::exiting) once. None of them is called if the window could not be created.
pub trait EventHandler {
    /// The window now exists. Called once, before any other method.
    fn window_created(&mut self, window: &Window);

    /// Something happened to the window.
    fn event(&mut self, event: Event);

    /// The window should be drawn now: the handler asked for it with [`Window::request_redraw`],
    /// or the windowing system did, for example because the window was resized or uncovered.
    ///
    /// On Windows, while the user resizes or moves the window, the system runs a loop of its own
    /// in which this is the only call the handler receives besides [`event`](Self::event):
    /// [`about_to_wait`](Self::about_to_wait) is not called until the user releases the window.
    fn redraw_requested(&mut self);

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
    /// Wait for new events, however long they take: an event of the window, or a [`Waker`]'s.
    Wait,
    /// End the loop: [`EventHandler::exiting`] is called next.
    Exit,
}

/// What reaches the event loop from other threads: a wake ([`Waker::wake`]), and, with the
/// `accessibility` feature, what screen readers ask, through AccessKit's adapter.
#[derive(Debug)]
pub(crate) enum UserEvent {
    Wake,
    /// The answer of the system's dialog ([`Window::choose_file`](crate::Window::choose_file)).
    FileChosen {
        dialog: crate::DialogId,
        file: Option<std::path::PathBuf>,
    },
    #[cfg(feature = "accessibility")]
    Accessibility(accesskit_winit::Event),
}

#[cfg(feature = "accessibility")]
impl From<accesskit_winit::Event> for UserEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::Accessibility(event)
    }
}

/// Wakes the event loop from any thread, so that [`EventHandler::about_to_wait`] runs again: what
/// another thread finished, a request that came, can then be seen. Clones wake the same loop; a
/// loop that ended wakes for nothing. Built on winit's user events, the one way to wake its loop in
/// winit 0.30.
#[derive(Clone, Debug)]
pub struct Waker {
    pub(crate) proxy: winit::event_loop::EventLoopProxy<UserEvent>,
}

impl Waker {
    /// Wakes the loop. Wakes asked for before the loop runs again are merged into one.
    pub fn wake(&self) {
        // A loop that has ended has nothing to wake.
        let _ = self.proxy.send_event(UserEvent::Wake);
    }
}

/// The largest width or height a window can be asked for, in logical pixels: larger than any
/// display. Times the scale factor, it can still exceed the 65 535 physical pixels that X11
/// allows, so `resumed` also gives winit a maximum size there.
const MAX_WINDOW_SIZE: u32 = 16_384;

/// Connects to the windowing system, creates a window, and runs the event loop until the
/// handler returns [`LoopControl::Exit`].
///
/// On Linux, the windowing system is chosen from the environment, with no fallback from one to the
/// other: Wayland if `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set to a non-empty value, otherwise
/// X11 if `DISPLAY` is. The windowing library allows one attempt per process: whether it succeeded
/// or not, a second call returns [`PlatformError::SecondEventLoop`].
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
/// If `config` asks for a width or height of zero or above 16 384 logical pixels, or for a title
/// containing a NUL byte: the windowing library would panic on them later, less clearly. Also if
/// the event loop is created from a thread other than the main thread.
///
/// On Linux, the windowing library also panics, instead of returning [`PlatformError::Connect`],
/// when it cannot load libxkbcommon. On Wayland, that happens when the compositor first reports
/// a keyboard, which can be after the window was created. On X11, it happens at once, and also
/// when libxkbcommon-x11 cannot be loaded or the X server does not set up its XKB extension.
pub fn run_event_loop(
    config: &WindowConfig,
    handler: &mut impl EventHandler,
) -> Result<(), PlatformError> {
    assert!(
        (1..=MAX_WINDOW_SIZE).contains(&config.width)
            && (1..=MAX_WINDOW_SIZE).contains(&config.height),
        "the window size must be between 1 and {MAX_WINDOW_SIZE} logical pixels in each \
         dimension, not {} by {}",
        config.width,
        config.height
    );
    assert!(
        !config.title.contains('\0'),
        "the window title must not contain NUL bytes"
    );
    let event_loop =
        EventLoop::<UserEvent>::with_user_event()
            .build()
            .map_err(|error| match error {
                EventLoopError::RecreationAttempt => PlatformError::SecondEventLoop,
                error => PlatformError::Connect(Box::new(error)),
            })?;
    let mut adapter = WinitAdapter {
        config,
        handler,
        window: None,
        translator: Translator::default(),
        error: None,
        proxy: event_loop.create_proxy(),
        #[cfg(feature = "accessibility")]
        accessibility: None,
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
    translator: Translator,
    error: Option<PlatformError>,
    /// Where other threads wake the loop, and AccessKit's adapter sends what screen readers ask.
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    /// AccessKit's adapter of the window, made before the window shows.
    #[cfg(feature = "accessibility")]
    accessibility: Option<crate::accessibility::Adapter>,
}

impl<H> WinitAdapter<'_, H> {
    /// Gives AccessKit's adapter the updates of the tree that the handler gave the window.
    fn update_accessibility(&mut self) {
        #[cfg(feature = "accessibility")]
        if let (Some(adapter), Some(window)) = (&mut self.accessibility, &self.window) {
            for update in window.take_accessibility() {
                adapter.update(update);
            }
        }
    }
}

impl<H: EventHandler> ApplicationHandler<UserEvent> for WinitAdapter<'_, H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // winit sends `resumed` once at startup on desktop platforms; mobile platforms send it
        // again after a suspension, when the window already exists.
        if self.window.is_some() {
            return;
        }
        let attributes = winit::window::Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height));
        // AccessKit's adapter is made before the window shows, so that screen readers see it from
        // the start; Wayland shows a window with its first frame, whatever this says.
        #[cfg(feature = "accessibility")]
        let attributes = attributes.with_visible(false);
        // The X11 protocol limits a window to 65 535 physical pixels per side, and winit panics
        // when the requested size times the scale factor is larger, unless a maximum size makes
        // it limit the size first.
        #[cfg(target_os = "linux")]
        let attributes = {
            use winit::platform::wayland::ActiveEventLoopExtWayland;
            if event_loop.is_wayland() {
                attributes
            } else {
                let max = u32::from(u16::MAX);
                attributes.with_max_inner_size(winit::dpi::PhysicalSize::new(max, max))
            }
        };
        match event_loop.create_window(attributes) {
            Ok(window) => {
                #[cfg(feature = "accessibility")]
                {
                    self.accessibility = Some(crate::accessibility::Adapter::new(
                        event_loop,
                        &window,
                        self.proxy.clone(),
                    ));
                    window.set_visible(true);
                }
                let window = Window::new(
                    window,
                    Waker {
                        proxy: self.proxy.clone(),
                    },
                );
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
                end(event_loop);
            }
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = &self.window else {
            return;
        };
        #[cfg(feature = "accessibility")]
        if let Some(adapter) = &mut self.accessibility {
            adapter.process_event(window.winit(), &event);
        }
        // A held cursor reports its raw motion, not where it is, which on X11 it is put back to.
        if let WindowEvent::CursorMoved { position, .. } = event
            && !window.cursor_moved(position)
        {
            return;
        }
        // A window without the focus lets the cursor go: X11's grab and Windows' clip would keep
        // it, hidden, from the window that the user went to.
        // Letting go never fails: what winit refuses then is logged.
        if event == WindowEvent::Focused(false) {
            let _ = window.set_cursor_held(false);
        }
        if matches!(event, WindowEvent::RedrawRequested) {
            self.handler.redraw_requested();
        } else {
            self.translator
                .translate(event, |event| self.handler.event(event));
        }
        self.update_accessibility();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        // The mouse's raw motion, while the window holds the cursor alone: X11 and Windows send it
        // at every motion while the window has the focus, the cursor held or not.
        if self.window.as_ref().is_some_and(Window::cursor_held)
            && let Some(event) = crate::event::raw_motion(&event)
        {
            self.handler.event(event);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            // A wake has done its work: the loop runs `about_to_wait` again.
            UserEvent::Wake => {}
            UserEvent::FileChosen { dialog, file } => {
                if self.window.is_some() {
                    self.handler.event(Event::FileChosen { dialog, file });
                }
            }
            #[cfg(feature = "accessibility")]
            UserEvent::Accessibility(event) => {
                use crate::Accessibility;
                use accesskit_winit::WindowEvent as Asked;

                if self.window.is_none() {
                    return;
                }
                let asked = match event.window_event {
                    Asked::InitialTreeRequested => Accessibility::TreeRequested,
                    Asked::ActionRequested(request) => Accessibility::Action(request),
                    Asked::AccessibilityDeactivated => Accessibility::Deactivated,
                };
                self.handler.event(Event::Accessibility(asked));
                self.update_accessibility();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // On Windows, winit calls this once more after the loop was asked to end, just before
        // `exiting`.
        if self.window.is_none() || event_loop.exiting() {
            return;
        }
        let control = self.handler.about_to_wait();
        self.update_accessibility();
        match control {
            LoopControl::WaitUntil(instant) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(instant));
            }
            LoopControl::Wait => event_loop.set_control_flow(ControlFlow::Wait),
            LoopControl::Exit => end(event_loop),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            self.handler.exiting();
        }
        // On Windows, dropping the window only posts a message, and the window is destroyed when
        // the loop handles it; the loop handles no message after this call, so the window would
        // stay on screen, unresponsive, until the process exits. Hiding it takes effect at once.
        #[cfg(windows)]
        if let Some(window) = &self.window {
            window.hide();
        }
        // Drop the window while the connection to the windowing system is still open, after the
        // adapter that serves it.
        #[cfg(feature = "accessibility")]
        {
            self.accessibility = None;
        }
        self.window = None;
    }
}

/// Ends the event loop. On Windows, winit only notices the request after waiting as the control
/// flow says: `Poll` makes that wait return at once, instead of lasting until the last deadline,
/// or without limit before the first one.
fn end(event_loop: &ActiveEventLoop) {
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.exit();
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Nothing;

    impl EventHandler for Nothing {
        fn window_created(&mut self, _window: &Window) {}
        fn event(&mut self, _event: Event) {}
        fn redraw_requested(&mut self) {}
        fn about_to_wait(&mut self) -> LoopControl {
            LoopControl::Exit
        }
        fn exiting(&mut self) {}
    }

    // The configuration is checked before the event loop is created, so these tests need no
    // windowing system and leave the one attempt per process unused.
    #[test]
    #[should_panic(expected = "window size")]
    fn a_zero_size_is_rejected() {
        let config = WindowConfig {
            width: 0,
            ..WindowConfig::default()
        };
        let _ = run_event_loop(&config, &mut Nothing);
    }

    #[test]
    #[should_panic(expected = "window size")]
    fn an_absurd_size_is_rejected() {
        let config = WindowConfig {
            height: u32::MAX,
            ..WindowConfig::default()
        };
        let _ = run_event_loop(&config, &mut Nothing);
    }

    #[test]
    #[should_panic(expected = "NUL")]
    fn a_title_with_a_nul_byte_is_rejected() {
        let config = WindowConfig {
            title: "save \0 name".to_owned(),
            ..WindowConfig::default()
        };
        let _ = run_event_loop(&config, &mut Nothing);
    }
}
