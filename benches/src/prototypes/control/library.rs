//! Slatty's library in the control, today's rebuild by keys: 10,000 covers in a grid that builds
//! only the cells that show, a filter typed a character at a time, the selection moved by the
//! arrows, a game's page opened, its key art fading in, then closed, and the theme changed.
//!
//! The model's concepts:
//! - `Program`, the trait that the host calls at each frame: `advance`, `act` for each action,
//!   then `build`
//! - `Ui`, the interface: `build`, which runs a builder when its revision moved, and `set_theme`
//! - `Children`, what a builder is given: `add`, `add_slot`, and `rows`, the cells that show
//! - `ElementId`, what `add` returns, kept to read the actions
//! - `Action`, an element and what was done to it
//! - `ActionKind`: `Press`, `Changed`, `Step`, `Intent` and `Command`
//! - `FieldValue`, a field's value in an action
//! - `Intent`, what a key means: `Accept`, `Cancel`
//! - a key, given to each element added, by which it keeps its state from one build to the next
//! - the rule: each change moves the revision of each builder that reads what it changed
//! - the rule: an id kept is valid until the builder that made it runs again
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `stack`, `column`, `label`, `button`, `image`
//! - `search_field` and its placeholder
//! - `grid`, which builds the cells that show, shows the place selected and steps from it
//! - `pressable`, a box that takes presses
//! - a theme by its name
//! - a class, which the style sheet styles; the key art's fade is the style sheet's
//! - `ImageKey`

use uwui::prelude::*;

#[derive(Clone)]
pub struct Game {
    pub title: String,
    pub cover: ImageKey,
    pub art: ImageKey,
}

/// A cover's cell, in logical pixels: 3 by 4.
const CELL: [f32; 2] = [150.0, 200.0];

#[derive(Clone)]
pub struct Library {
    games: Vec<Game>,
    filter: String,
    /// The games that the filter keeps, by their places in `games`.
    shown: Vec<usize>,
    /// The place in `shown` of the game selected.
    selected: usize,
    /// The game whose page is open.
    page: Option<usize>,
    dark: bool,
    /// Moved when the filter, the games shown, the selection or the page change.
    revision: u64,
    /// Moved when the cells show other games.
    cells: u64,
    ids: Ids,
}

/// The elements that the actions name, from the builds that made them.
#[derive(Clone, Default)]
struct Ids {
    filter: Option<ElementId>,
    grid: Option<ElementId>,
    close: Option<ElementId>,
    covers: Vec<(ElementId, usize)>,
}

impl Library {
    pub fn new(games: Vec<Game>) -> Self {
        Self {
            shown: (0..games.len()).collect(),
            games,
            filter: String::new(),
            selected: 0,
            page: None,
            dark: true,
            revision: 0,
            cells: 0,
            ids: Ids::default(),
        }
    }

    fn set_filter(&mut self, filter: String) {
        self.shown = matching(&self.games, &filter);
        self.filter = filter;
        self.selected = 0;
        self.revision += 1;
        self.cells += 1;
    }

    fn select(&mut self, place: usize) {
        self.selected = place;
        self.revision += 1;
    }

    fn open(&mut self, place: usize) {
        self.page = self.shown.get(place).copied();
        self.revision += 1;
    }

    fn close(&mut self) {
        if self.page.take().is_some() {
            self.revision += 1;
        }
    }
}

impl Program for Library {
    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        let ids = &mut self.ids;
        ui.build(root, self.revision, |children| {
            ids.close = None;
            children.add("screen", Element::stack(), |children| {
                children.add("library", Element::column(), |children| {
                    let field = Element::search_field(&self.filter).placeholder("Search a game");
                    ids.filter = Some(children.add("filter", field, |_| {}));
                    let grid = Element::grid(self.shown.len(), CELL)
                        .class("covers")
                        .selection(self.selected);
                    ids.grid = Some(children.add_slot("grid", grid));
                });
                if let Some(game) = self.page.map(|game| &self.games[game]) {
                    children.add("page", Element::column().class("page"), |children| {
                        let close = Element::button("Library");
                        ids.close = Some(children.add("close", close, |_| {}));
                        let art = Element::image(game.art).class("key-art");
                        children.add("art", art, |_| {});
                        let title = Element::label(&game.title).class("title");
                        children.add("title", title, |_| {});
                    });
                }
            });
        });
        let Some(grid) = self.ids.grid else {
            return;
        };
        let covers = &mut self.ids.covers;
        ui.build(grid, self.cells, |children| {
            covers.clear();
            for place in children.rows() {
                let game = self.shown[place];
                let cover = Element::pressable().class("cover");
                let id = children.add(game, cover, |children| {
                    let image = Element::image(self.games[game].cover);
                    children.add("image", image, |_| {});
                    let title = Element::label(&self.games[game].title);
                    children.add("title", title, |_| {});
                });
                covers.push((id, place));
            }
        });
    }

    fn act(&mut self, ui: &mut Ui, action: Action) {
        let element = action.element;
        match action.kind {
            ActionKind::Changed(FieldValue::Text(text)) if element == self.ids.filter => {
                self.set_filter(text);
            }
            ActionKind::Step(place) if element == self.ids.grid => self.select(place),
            ActionKind::Intent(Intent::Accept) if element == self.ids.grid => {
                self.open(self.selected);
            }
            ActionKind::Press if element == self.ids.close => self.close(),
            ActionKind::Press => {
                let cover = self.ids.covers.iter().find(|(id, _)| Some(*id) == element);
                if let Some(&(_, place)) = cover {
                    self.open(place);
                }
            }
            ActionKind::Intent(Intent::Cancel) => self.close(),
            ActionKind::Command(command) if command == "toggle-theme" => {
                self.dark = !self.dark;
                ui.set_theme(if self.dark { "dark" } else { "light" });
            }
            _ => {}
        }
    }
}

/// The games whose titles hold `filter`, whatever their case.
fn matching(games: &[Game], filter: &str) -> Vec<usize> {
    let filter = filter.to_lowercase();
    (0..games.len())
        .filter(|&game| games[game].title.to_lowercase().contains(&filter))
        .collect()
}
