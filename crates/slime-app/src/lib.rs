#![doc = include_str!("../README.md")]

mod app;
#[cfg(feature = "development")]
mod channel;
mod data;
mod diagnostics;
mod error;
mod game;
mod host;
mod interface;
#[cfg(feature = "development")]
mod log_lines;
mod signal;
pub mod time;
mod trace;
mod windowed;

pub use app::{AppConfig, run};
pub use data::{HeldFile, data_directory, set_aside};
pub use diagnostics::{DEFAULT_LOG_FILTER, LOG_ENV_VAR, error_chain, init_diagnostics};
pub use error::{AppError, RunError};
pub use game::{PickAnswer, Presentation, Reflected, ShownView, Simulation};
pub use host::{Context, Host, SimulationContext};
pub use trace::TRACE_ENV_VAR;

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    #[test]
    fn the_host_its_context_and_the_errors_can_be_shared_between_threads() {
        send_and_sync::<Host>();
        send_and_sync::<Context>();
        send_and_sync::<AppConfig>();
        send_and_sync::<RunError>();
    }
}
