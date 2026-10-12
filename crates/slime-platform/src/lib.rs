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
