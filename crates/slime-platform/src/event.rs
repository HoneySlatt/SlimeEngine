//! Events delivered to the application, and their translation from winit's.

use winit::event::WindowEvent;

use crate::input::{self, ButtonState, KeyEvent, MouseButton, ScrollDelta};
use crate::window::PhysicalSize;

/// Something that happened to the window, or input it received.
///
/// Input arrives only while the window has the keyboard focus (keys) or the cursor (mouse). The
/// engine does not report yet: the state of modifier keys as a whole (track the `Shift`,
/// `Control`, `Alt` and `Super` keys instead), text composed by input methods (used for Chinese,
/// Japanese or Korean), touch screens, touchpad gestures, or files dropped on the window.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// The user or the compositor asked to close the window, for example with a close button or a
    /// compositor shortcut.
    CloseRequested,
    /// The window's content area changed size.
    Resized(PhysicalSize),
    /// The number of physical pixels per logical pixel changed, for example because the window
    /// moved to another display.
    ScaleFactorChanged {
        /// The new number of physical pixels per logical pixel.
        scale_factor: f64,
    },
    /// The window gained (`true`) or lost (`false`) the keyboard focus.
    Focused(bool),
    /// A key went down or up.
    Key(KeyEvent),
    /// A mouse button went down or up while the cursor was over the window.
    MouseButton {
        /// Which button.
        button: MouseButton,
        /// Whether it went down or up.
        state: ButtonState,
    },
    /// The cursor moved over the window.
    CursorMoved {
        /// Distance from the left edge of the content area, in physical pixels.
        x: f64,
        /// Distance from the top edge of the content area, in physical pixels.
        y: f64,
    },
    /// The cursor entered the window.
    CursorEntered,
    /// The cursor left the window.
    CursorLeft,
    /// A mouse wheel or a touchpad scrolled while the cursor was over the window.
    MouseWheel(ScrollDelta),
}

/// Translates a winit window event, or returns `None` for events the engine does not handle.
/// Redraw requests are not events: the event loop passes them to
/// [`EventHandler::redraw_requested`](crate::EventHandler::redraw_requested).
pub(crate) fn translate(event: WindowEvent) -> Option<Event> {
    match event {
        WindowEvent::CloseRequested => Some(Event::CloseRequested),
        WindowEvent::Resized(size) => Some(Event::Resized(PhysicalSize {
            width: size.width,
            height: size.height,
        })),
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            Some(Event::ScaleFactorChanged { scale_factor })
        }
        WindowEvent::Focused(focused) => Some(Event::Focused(focused)),
        WindowEvent::KeyboardInput { event, .. } => Some(Event::Key(input::key_event(event))),
        WindowEvent::MouseInput { state, button, .. } => Some(Event::MouseButton {
            button: input::mouse_button(button),
            state: input::button_state(state),
        }),
        WindowEvent::CursorMoved { position, .. } => Some(Event::CursorMoved {
            x: position.x,
            y: position.y,
        }),
        WindowEvent::CursorEntered { .. } => Some(Event::CursorEntered),
        WindowEvent::CursorLeft { .. } => Some(Event::CursorLeft),
        WindowEvent::MouseWheel { delta, .. } => {
            Some(Event::MouseWheel(input::scroll_delta(delta)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::{PhysicalPosition, PhysicalSize as WinitSize};

    use super::*;

    #[test]
    fn window_events_are_translated() {
        assert_eq!(
            translate(WindowEvent::CloseRequested),
            Some(Event::CloseRequested)
        );
        assert_eq!(
            translate(WindowEvent::Resized(WinitSize::new(800, 600))),
            Some(Event::Resized(PhysicalSize {
                width: 800,
                height: 600
            }))
        );
        assert_eq!(
            translate(WindowEvent::Focused(false)),
            Some(Event::Focused(false))
        );
    }

    #[test]
    fn events_the_engine_does_not_handle_are_dropped() {
        assert_eq!(
            translate(WindowEvent::Moved(PhysicalPosition::new(10, 20))),
            None
        );
    }
}
