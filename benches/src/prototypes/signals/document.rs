//! The synthetic case with signals: a document of 100,000 lines in a list that builds only the
//! lines that show, one line of which changes from outside the screen, by `edit`, which the bench
//! calls on the handle that it holds. Each line is a signal of its own, so that a change runs
//! again the one label that reads it.
//!
//! The model's concepts:
//! - `View`, what a view's function gives
//! - the rule: a view's function runs once; a closure that reads a signal runs again when it
//!   changes, and nothing else does
//! - `RwSignal`, a `Copy` handle to a value: `get`, `set`, which tells only of a value that
//!   changed
//! - `Stored`, a `Copy` handle to a value that no closure tracks
//! - `Stored::kept`, a value named in its scope, whose value a reload keeps through `serde`, the
//!   signals that it holds too; its first value given by a closure
//! - a closure given where a value would be, `move || …`, which the view tracks
//! - the rule: a signal read outside a closure that the view tracks is read once
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `label`
//! - `list`, which builds the rows that show
//! - a class, which the style sheet styles

use super::prelude::*;

/// A line's height, in logical pixels.
const LINE: f32 = 20.0;

/// The document's lines, which the program changes from outside the screen.
#[derive(Clone, Copy)]
pub struct Document {
    lines: Stored<Vec<RwSignal<String>>>,
}

impl Document {
    pub fn new(lines: impl FnOnce() -> Vec<String>) -> Self {
        let lines = Stored::kept("lines", || lines().into_iter().map(RwSignal::new).collect());
        Self { lines }
    }

    /// Gives `line` the text `text`, as an edit from outside the screen would.
    pub fn edit(self, line: usize, text: String) {
        self.lines.with(|lines| lines[line].set(text));
    }

    pub fn view(self) -> impl View {
        let count = self.lines.with(Vec::len);
        list(count, LINE, move |line| {
            let text = self.lines.with(|lines| lines[line]);
            label(move || text.get()).class("line")
        })
        .class("document")
    }
}
