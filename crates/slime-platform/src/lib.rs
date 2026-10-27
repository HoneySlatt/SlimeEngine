#![doc = include_str!("../README.md")]

#[cfg(feature = "accessibility")]
mod accessibility;
mod clipboard;
mod error;
mod event;
mod event_loop;
mod held;
mod input;
mod window;

#[cfg(feature = "accessibility")]
pub use accessibility::Accessibility;
pub use clipboard::Clipboard;
pub use error::PlatformError;
pub use event::Event;
pub use event_loop::{EventHandler, LoopControl, run_event_loop};
pub use held::{Buttons, Input};
pub use slime_input::{
    ButtonState, CursorShape, Ime, Key, KeyCode, KeyEvent, Modifiers, MouseButton, NamedKey,
    ScrollDelta,
};
pub use window::{ColorScheme, PhysicalSize, Window, WindowConfig};

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    fn send<T: Send>() {}

    #[test]
    fn windows_events_and_input_can_be_shared_between_threads() {
        send_and_sync::<Window>();
        send_and_sync::<Event>();
        send_and_sync::<Input>();
        send_and_sync::<PlatformError>();
        // The clipboard goes with the interface it serves, which a presentation may move.
        send::<Clipboard>();
    }
}
