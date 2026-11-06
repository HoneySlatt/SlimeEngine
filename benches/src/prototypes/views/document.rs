//! The synthetic case with views that say they changed: a document of 100,000 lines in a list
//! that builds only the lines that show, one line of which changes from outside the screen, by
//! `edit`, which the bench calls on the view that it holds.
//!
//! The model's concepts:
//! - `Render`, the trait of a view: `render`, which gives its elements
//! - `Cx`, the context that a view renders and handles its events in: `notify`, and `processor`,
//!   a closure that gives a list its rows
//! - the rule: a view that changed without `notify` shows what it showed
//! - `IntoElement`, what `render` gives
//! - a key, `key`, by which an element keeps its state from one render to the next
//! - the rule: an element without a key is matched by its place among its siblings
//! - `View`, what `Cx::new` makes of a value: `update`, its value changed from outside it
//! - `App`, the context outside every view, which an edit from outside is given
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `label`
//! - `list`, which builds the rows that show
//! - a class, which the style sheet styles

use std::ops::Range;

use super::prelude::*;

/// A line's height, in logical pixels.
const LINE: f32 = 20.0;

#[derive(Clone)]
pub struct Document {
    pub lines: Vec<String>,
}

impl Document {
    /// Gives `line` of `document` the text `text`, as an edit from outside the screen would.
    pub fn edit(document: &View<Self>, line: usize, text: String, cx: &mut App) {
        document.update(cx, |document, cx| {
            document.lines[line] = text;
            cx.notify();
        });
    }
}

impl Render for Document {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement {
        let lines = cx.processor(|this, lines: Range<usize>, _| {
            lines
                .map(|line| label(this.lines[line].clone()).key(line).class("line"))
                .collect()
        });
        list(self.lines.len(), LINE, lines).class("document")
    }
}
