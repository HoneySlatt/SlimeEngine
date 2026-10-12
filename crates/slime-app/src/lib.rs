#![doc = include_str!("../README.md")]

mod diagnostics;
pub mod time;

pub use diagnostics::{LOG_ENV_VAR, init_diagnostics};
