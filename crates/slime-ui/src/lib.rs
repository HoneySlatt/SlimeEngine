#![doc = include_str!("../README.md")]

pub mod action;
pub mod display;
mod document;
mod layout;
mod paint;
mod rect;
pub mod shortcuts;
pub mod style;
pub mod text;
pub mod theme;
pub mod tree;

pub use document::DocumentError;
pub use rect::Rect;
