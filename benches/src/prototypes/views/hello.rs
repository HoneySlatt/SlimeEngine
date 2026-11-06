//! "Hello, World!" with views that say they changed: a view is a value of the program's that
//! renders its elements, again when it says that it changed; this screen never changes.
//!
//! The model's concepts:
//! - `Render`, the trait of a view: `render`, which gives its elements
//! - `Cx`, the context that a view renders and handles its events in
//! - `IntoElement`, what `render` gives
//! - `on_press`: a closure that an event runs
//! - `View`, what `Cx::new` makes of a value
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `column`, `label`, `button`
//! - a class, which the style sheet styles

use super::prelude::*;

#[derive(Clone)]
pub struct Hello;

impl Render for Hello {
    fn render(&mut self, _: &mut Cx<Self>) -> impl IntoElement {
        column((
            label("Hello, World!"),
            button("Let's Go!")
                .class("primary")
                .on_press(|_, _| println!("Clicked!")),
        ))
        .class("centered")
    }
}
