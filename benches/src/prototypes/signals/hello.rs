//! "Hello, World!" with signals: the view's function runs once, and what reads a signal runs
//! again when it changes; this screen holds none.
//!
//! The model's concepts:
//! - `View`, what a view's function gives
//! - the rule: a view's function runs once
//! - `on_press`: a closure that an event runs
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `column`, `label`, `button`
//! - a class, which the style sheet styles

use super::prelude::*;

pub fn hello() -> impl View {
    column((
        label("Hello, World!"),
        button("Let's Go!")
            .class("primary")
            .on_press(|| println!("Clicked!")),
    ))
    .class("centered")
}
