//! The control's own mistake, a revision not moved: a list whose selection moves by the arrows,
//! but whose `act` forgets `self.revision += 1`. It compiles; the list keeps showing the row
//! selected before, which the bench finds by comparing its display list with that of a new
//! interface built from the same state (`docs/ideal-code/README.md` in uwui's repository,
//! "Mistakes that the compiler lets through").

use uwui::prelude::*;

/// A row's height, in logical pixels.
const ROW: f32 = 32.0;

#[derive(Clone)]
pub struct Forgotten {
    selected: usize,
    /// Moved when the selection changes, but `act` forgets to.
    revision: u64,
    list: Option<ElementId>,
}

impl Forgotten {
    pub fn new() -> Self {
        Self {
            selected: 0,
            revision: 0,
            list: None,
        }
    }
}

impl Program for Forgotten {
    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        let list = &mut self.list;
        ui.build(root, self.revision, |children| {
            let rows = Element::list(1_000, ROW)
                .class("table")
                .selection(self.selected);
            *list = Some(children.add_slot("rows", rows));
        });
        let Some(list) = self.list else {
            return;
        };
        ui.build(list, 0, |children| {
            for row in children.rows() {
                let label = Element::label(format!("Row {row}")).class("row");
                children.add(row, label, |_| {});
            }
        });
    }

    fn act(&mut self, _: &mut Ui, action: Action) {
        if let ActionKind::Step(place) = action.kind
            && action.element == self.list
        {
            // The mistake: the selection changes, its revision does not.
            self.selected = place;
        }
    }
}

/// A cover's cell, in logical pixels.
#[cfg(test)]
const CELL: [f32; 2] = [150.0, 200.0];

/// A grid of six covers, each matched by its game, or, the mistake, by its place: its state, the
/// focus, passes to another game when one before it is removed.
#[cfg(test)]
#[derive(Clone)]
pub struct Covers {
    shown: Vec<usize>,
    keyed: bool,
    revision: u64,
    grid: Option<ElementId>,
}

#[cfg(test)]
impl Covers {
    pub fn new(keyed: bool) -> Self {
        Self {
            shown: (0..6).collect(),
            keyed,
            revision: 0,
            grid: None,
        }
    }

    pub fn remove_first(&mut self) {
        self.shown.remove(0);
        self.revision += 1;
    }
}

#[cfg(test)]
impl Program for Covers {
    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        let grid = &mut self.grid;
        ui.build(root, self.revision, |children| {
            let covers = Element::grid(self.shown.len(), CELL).class("table");
            *grid = Some(children.add_slot("grid", covers));
        });
        let Some(grid) = self.grid else {
            return;
        };
        ui.build(grid, self.revision, |children| {
            for place in children.rows() {
                let game = self.shown[place];
                let key = if self.keyed { game } else { place };
                children.add(key, Element::pressable().class("cover"), |children| {
                    let title = Element::label(format!("Game {game}"));
                    children.add("title", title, |_| {});
                });
            }
        });
    }
}
