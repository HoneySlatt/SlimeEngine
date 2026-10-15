#![doc = include_str!("../README.md")]

mod app;
mod diagnostics;
mod error;
mod signal;
pub mod time;
mod windowed;

pub use app::{App, AppConfig, Context, run};
pub use diagnostics::{DEFAULT_LOG_FILTER, LOG_ENV_VAR, error_chain, init_diagnostics};
pub use error::{AppError, RunError};

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    #[test]
    fn the_context_and_the_errors_can_be_shared_between_threads() {
        send_and_sync::<Context>();
        send_and_sync::<AppConfig>();
        send_and_sync::<RunError>();
    }
}
