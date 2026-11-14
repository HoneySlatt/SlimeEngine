#![doc = include_str!("../README.md")]

mod client;
mod error;
mod listener;
mod message;
mod token;
mod wire;

pub use client::{Client, Launched, Notice, Pending, launch, launch_unless};
pub use error::ChannelError;
pub use listener::{Incoming, Listener};
pub use message::{
    ComponentText, Edit, EditorHistory, EntitySummary, Event, GameState, GameTime, LogLine,
    OperationSummary, Reply, Request, ViewSummary,
};
pub use token::Token;
/// Why a request failed, with its code: what [`Reply::Failed`] carries.
pub use uwui_agent::Fault;
pub use uwui_agent::Named;
/// The interface's family, whose requests and replies [`Request::Interface`] and
/// [`Reply::Interface`] carry: uwui's own, which `uwui` answers with its feature `agents`.
pub use uwui_agent::interface;
/// How many bytes of JSON a value writes: what a page of a list counts of its items.
pub use uwui_agent::json_size;
/// A long list read by pages: what bounds a page, its cursor, texts cut to fit
/// ([`Incoming::page`]).
pub use uwui_agent::page;

/// Locks `mutex`, through a panic of another thread that held it: what it guards stays whole, each
/// change being made under the lock in one step.
pub(crate) fn locked<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Waits on `condition` with `guard`, as [`locked`] does through a panic of another thread.
pub(crate) fn waited<'a, T>(
    condition: &std::sync::Condvar,
    guard: std::sync::MutexGuard<'a, T>,
) -> std::sync::MutexGuard<'a, T> {
    condition
        .wait(guard)
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The families of requests that the channel carries, each at its version, with the names of the
/// events that it declares ([`Event`]): a tool and a game that speak another version of one refuse
/// that family alone, and the rest is served
/// (`docs/adr/0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md`). The
/// version of the core's session is `uwui_agent::CORE_VERSION`.
pub const FAMILIES: [(&str, u32, &[&str]); 4] = [
    ("play", 1, &["time"]),
    ("world", 2, &[]),
    ("editing", 1, &[]),
    (
        uwui_agent::interface::FAMILY,
        uwui_agent::interface::VERSION,
        uwui_agent::interface::EVENTS,
    ),
];

/// [`FAMILIES`], as the core's session names them.
pub(crate) fn families() -> Vec<uwui_agent::Family> {
    FAMILIES
        .iter()
        .map(|&(name, version, _)| uwui_agent::Family::new(name, version))
        .collect()
}

/// The events that [`FAMILIES`] declare, as the core's session names them: `play/time`.
pub(crate) fn events() -> Vec<String> {
    FAMILIES
        .iter()
        .flat_map(|&(family, _, events)| events.iter().map(move |name| format!("{family}/{name}")))
        .collect()
}

/// The environment variable that gives a game its token: a game started without it does not
/// listen. A game started with it ends once its standard input closes, but when that input is a
/// terminal: the tool that started it holds a pipe there ([`launch`]), so that a tool that ends,
/// crashed or not, leaves no game behind. Whoever starts a game with a token without `launch` holds
/// such a pipe too: an input that is empty or missing, as `/dev/null`, ends the game at once.
pub const TOKEN_ENV_VAR: &str = "SLIME_DEV_TOKEN";

/// What a game writes on its standard output, followed by the address it listens on, once it
/// listens: `slime-dev-channel 127.0.0.1:41234`.
pub const ANNOUNCEMENT: &str = "slime-dev-channel ";

/// The most lines of its log that a game keeps, the oldest dropped first.
pub const MAX_LOG_LINES: usize = 1000;

/// The most bytes of a line of its log that a game keeps: a longer line is cut on a character's
/// boundary, and marked ([`LogLine::cut`]).
pub const MAX_LOG_LINE: usize = 16 << 10;

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
        send_and_sync::<Request>();
        send_and_sync::<Reply>();
        send_and_sync::<Event>();
        send_and_sync::<ChannelError>();
        send_and_sync::<Listener>();
        send_and_sync::<Incoming>();
        send::<crate::Pending>();
        send_and_sync::<Client>();
    }
}
