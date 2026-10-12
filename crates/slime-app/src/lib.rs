#![doc = include_str!("../README.md")]

mod diagnostics;

pub use diagnostics::{LOG_ENV_VAR, init_diagnostics};
