//! "Hello, World!" in the control, today's rebuild by keys: a builder that runs when its
//! revision moves, and actions read from a queue by the ids of the elements that the builder
//! made.
//!
//! The model's concepts:
//! - `Program`, the trait that the host calls at each frame: `act` for each action, then `build`
//! - `Ui`, the interface: `build`, which runs a builder when its revision moved
//! - `Children`, what a builder is given: `add`
//! - `ElementId`, what `add` returns, kept to read the actions
//! - `Action`, an element and what was done to it
//! - `ActionKind`: `Press`
//! - a key, given to each element added, by which it keeps its state from one build to the next
//! - the rule: each change moves the revision of each builder that reads what it changed
//! - the rule: an id kept is valid until the builder that made it runs again
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `column`, `label`, `button`
//! - a class, which the style sheet styles

use uwui::prelude::*;

#[derive(Clone, Default)]
pub struct Hello {
    go: Option<ElementId>,
}

impl Program for Hello {
    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        ui.build(root, 0, |children| {
            let column = Element::column().class("centered");
            children.add("column", column, |children| {
                children.add("text", Element::label("Hello, World!"), |_| {});
                let go = Element::button("Let's Go!").class("primary");
                self.go = Some(children.add("go", go, |_| {}));
            });
        });
    }

    fn act(&mut self, _: &mut Ui, action: Action) {
        if action.kind == ActionKind::Press && action.element == self.go {
            println!("Clicked!");
        }
    }
}
