#![doc = include_str!("../README.md")]

pub mod action;
pub mod display;
pub mod dock;
mod document;
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
pub mod tree;

pub use document::DocumentError;
pub use rect::Rect;
