//! The synthetic case in the control, today's rebuild by keys: a document of 100,000 lines in a
//! list that builds only the lines that show, one line of which changes from outside the screen,
//! by `edit`, which the bench calls on the state that it holds.
//!
//! The model's concepts:
//! - `Program`, the trait that the host calls at each frame: `build`
//! - `Ui`, the interface: `build`, which runs a builder when its revision moved
//! - `Children`, what a builder is given: `add`, `add_slot`, and `rows`, the rows that show
//! - `ElementId`, what `add_slot` returns, kept for the list's builder
//! - a key, given to each element added, by which it keeps its state from one build to the next
//! - the rule: each change moves the revision of each builder that reads what it changed
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `label`
//! - `list`, which builds the rows that show
//! - a class, which the style sheet styles

use uwui::prelude::*;

/// A line's height, in logical pixels.
const LINE: f32 = 20.0;

#[derive(Clone)]
pub struct Document {
    lines: Vec<String>,
    /// Moved when a line changes.
    revision: u64,
    list: Option<ElementId>,
}

impl Document {
    pub fn new(lines: Vec<String>) -> Self {
        Self {
            lines,
            revision: 0,
            list: None,
        }
    }

    /// Gives `line` the text `text`, as an edit from outside the screen would.
    pub fn edit(&mut self, line: usize, text: String) {
        self.lines[line] = text;
        self.revision += 1;
    }
}

impl Program for Document {
    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        let list = &mut self.list;
        ui.build(root, 0, |children| {
            let lines = Element::list(self.lines.len(), LINE).class("document");
            *list = Some(children.add_slot("lines", lines));
        });
        let Some(list) = self.list else {
            return;
        };
        ui.build(list, self.revision, |children| {
            for line in children.rows() {
                let label = Element::label(&self.lines[line]).class("line");
                children.add(line, label, |_| {});
            }
        });
    }
}
