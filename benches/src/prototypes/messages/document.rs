//! The synthetic case with messages, as in Elm: a document of 100,000 lines in a list that
//! builds only the lines that show, one line of which changes from outside the screen, by
//! `Message::Edit`, which the bench sends to the application.
//!
//! The model's concepts:
//! - `Message`, the program's type of what can happen
//! - `update`, which changes the state for a message
//! - `view`, which describes the interface from the state, and runs again after each message
//! - `El`, what `view` gives, which borrows the state
//! - a key, `key`, by which an element keeps its state from one `view` to the next
//! - the rule: an element without a key is matched by its place among its siblings
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `label`
//! - `list`, which builds the rows that show
//! - a class, which the style sheet styles

use super::prelude::*;

/// A line's height, in logical pixels.
const LINE: f32 = 20.0;

#[derive(Clone)]
pub struct Document {
    pub lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Message {
    /// A line, and its new text, as an edit from outside the screen would give it.
    Edit(usize, String),
}

impl Document {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Edit(line, text) => self.lines[line] = text,
        }
    }

    pub fn view(&self) -> El<'_, Message> {
        list(self.lines.len(), LINE, |line| {
            label(&self.lines[line]).key(line).class("line").into()
        })
        .class("document")
        .into()
    }
}
