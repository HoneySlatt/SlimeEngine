//! Events delivered to the application, and their translation from winit's.

use winit::event::WindowEvent;

use crate::window::PhysicalSize;

/// Something that happened to the window.
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
}

/// Translates a winit window event, or returns `None` for events the engine does not handle.
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
        assert_eq!(translate(WindowEvent::RedrawRequested), None);
    }
}
