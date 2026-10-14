#![doc = include_str!("../README.md")]

mod error;
mod event;
mod event_loop;
mod input;
mod window;

pub use error::PlatformError;
pub use event::Event;
pub use event_loop::{EventHandler, LoopControl, run_event_loop};
pub use input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey, ScrollDelta};
pub use window::{PhysicalSize, Window, WindowConfig};

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    #[test]
    fn windows_and_events_can_be_shared_between_threads() {
        send_and_sync::<Window>();
        send_and_sync::<Event>();
        send_and_sync::<PlatformError>();
    }
}
