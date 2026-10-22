#![doc = include_str!("../README.md")]

mod client;
mod error;
mod listener;
mod message;
mod token;
mod wire;

pub use client::{Client, Launched, launch};
pub use error::ChannelError;
pub use listener::{Incoming, Listener};
pub use message::{
    ComponentText, EntitySummary, GameState, Hello, LogLine, Reply, Request, ViewSummary,
};
pub use token::Token;

/// The version of the protocol: a tool and a game of other versions refuse each other, rather
/// than misread each other's messages.
pub const PROTOCOL_VERSION: u32 = 6;

/// The environment variable that gives a game its token: a game started without it does not
/// listen.
pub const TOKEN_ENV_VAR: &str = "SLIME_DEV_TOKEN";

/// What a game writes on its standard output, followed by the address it listens on, once it
/// listens: `slime-dev-channel 127.0.0.1:41234`.
pub const ANNOUNCEMENT: &str = "slime-dev-channel ";

/// The most entities that a [`Reply::Entities`] lists: it counts the others.
pub const MAX_ENTITIES: usize = 1000;

/// The most lines of its log that a game keeps, the oldest dropped first, and that a
/// [`Reply::Logs`] holds.
pub const MAX_LOG_LINES: usize = 1000;

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    fn send<T: Send>() {}

    #[test]
    fn the_channel_s_types_cross_threads() {
        send_and_sync::<Token>();
        send_and_sync::<Hello>();
        send_and_sync::<Request>();
        send_and_sync::<Reply>();
        send_and_sync::<ChannelError>();
        send::<Listener>();
        send::<Incoming>();
        send_and_sync::<Client>();
    }
}
