#![doc = include_str!("../README.md")]

mod error;
mod message;
mod session;
mod typed;

pub use error::AgentError;
pub use message::{Fault, Frame, Limits, MAX_EXACT_INTEGER, Message, Reader, encode};
/// The JSON that a message holds, which a family reads into its own types.
pub use serde_json::Value;
pub use session::{
    Family, FromProgram, FromTool, HELLO, Hello, Named, ProgramSession, Refusal, ToolSession,
    Welcome,
};
pub use typed::{call, read, read_call, write};

/// The version of the core: its session and its messages' envelope. A family's own messages have
/// their own versions ([`Family`]). A tool and a program of other versions of the core refuse each
/// other at the hello, rather than misread each other.
pub const CORE_VERSION: u32 = 1;

/// Which types may cross threads: checked when the tests are compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    #[test]
    fn the_core_s_types_cross_threads() {
        send_and_sync::<Message>();
        send_and_sync::<Frame>();
        send_and_sync::<Reader>();
        send_and_sync::<ProgramSession>();
        send_and_sync::<ToolSession>();
        send_and_sync::<AgentError>();
    }
}
