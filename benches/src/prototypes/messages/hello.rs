//! "Hello, World!" with messages, as in Elm: the interface is a function of the state, `view`,
//! which runs again after each message that `update` handles.
//!
//! The model's concepts:
//! - `Message`, the program's type of what can happen
//! - `update`, which changes the state for a message
//! - `view`, which describes the interface from the state, and runs again after each message
//! - `El`, what `view` gives, which borrows the state
//! - `on_press`: the message that an event sends
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `column`, `label`, `button`
//! - a class, which the style sheet styles

use super::prelude::*;

#[derive(Clone, Debug)]
pub enum Message {
    Go,
}

pub fn update(_: &mut (), message: Message) {
    match message {
        Message::Go => println!("Clicked!"),
    }
}

pub fn view(_: &()) -> El<'_, Message> {
    column((
        label("Hello, World!"),
        button("Let's Go!").class("primary").on_press(Message::Go),
    ))
    .class("centered")
    .into()
}
