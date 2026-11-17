#![doc = include_str!("../README.md")]
// A public enumeration may gain cases without breaking a program; one closed by nature says why
// it is not marked (ADR-0282).
#![warn(clippy::exhaustive_enums)]

pub mod action;
#[cfg(feature = "agents")]
pub mod agents;
pub mod display;
pub mod dock;
mod document;
pub mod host;
mod layout;
#[cfg(feature = "editor-look")]
pub mod look;
mod paint;
pub mod preferences;
mod rect;
pub mod shortcuts;
pub mod style;
pub mod text;
pub mod theme;
/// The times that a host gives uwui, which the crate `uwui-time` holds, below uwui and its desktop
/// host's platform layer.
pub use uwui_time as time;
pub mod tree;
#[cfg(test)]
mod verdicts_tests;
#[cfg(test)]
mod versions_tests;

pub use document::{DocumentError, DocumentErrors};
pub use rect::Rect;
