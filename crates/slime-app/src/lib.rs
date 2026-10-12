#![doc = include_str!("../README.md")]

mod app;
mod diagnostics;
pub mod time;
mod windowed;

pub use app::{App, AppConfig, Context, run};
pub use diagnostics::{LOG_ENV_VAR, init_diagnostics};
