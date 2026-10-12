#![doc = include_str!("../README.md")]

mod app;
mod diagnostics;
mod signal;
pub mod time;
mod windowed;

pub use app::{App, AppConfig, Context, run};
pub use diagnostics::{DEFAULT_LOG_FILTER, LOG_ENV_VAR, init_diagnostics};
