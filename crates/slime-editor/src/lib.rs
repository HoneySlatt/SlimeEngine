#![doc = include_str!("../README.md")]

mod editor;
mod history;
mod journal;
mod level;
mod program;

pub use editor::{EditError, Editor};
pub use history::HISTORY_BUDGET;
pub use program::{Program, run};
