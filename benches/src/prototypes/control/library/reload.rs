//! The control's library reloaded: its entry point, `build`, called through a table that the test
//! swaps for a body whose placeholder differs; and its state written to a form and read back.

use super::*;
use crate::prototypes::data;
use crate::prototypes::host::Model;
use crate::prototypes::reload::{self, Field, Form};
use crate::prototypes::screens::{self, Screen, screen};

/// The library, its entry point called through a table, as `subsecond` calls it.
#[derive(Clone)]
pub(crate) struct Table {
    library: Library,
    build: fn(&mut Library, &mut Ui, ElementId),
}

impl Model for Table {
    fn advance(&mut self, ui: &mut Ui, now: std::time::Instant) {
        Program::advance(&mut self.library, ui, now);
    }

    fn act(&mut self, ui: &mut Ui, action: Action) {
        Program::act(&mut self.library, ui, action);
    }

    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        (self.build)(&mut self.library, ui, root);
    }

    fn fresh(&self) -> Self {
        self.clone()
    }
}

fn games() -> Vec<Game> {
    data::games(10_000, |title, cover, art| Game { title, cover, art })
}

const TABLE: Screen<Table> = screen(
    screens::LIBRARY,
    |_| Table {
        library: Library::new(games()),
        build: <Library as Program>::build,
    },
    screens::library,
);

/// `build`'s body, its placeholder changed.
fn build_patched(this: &mut Library, ui: &mut Ui, root: ElementId) {
    let ids = &mut this.ids;
    ui.build(root, this.revision, |children| {
        ids.close = None;
        children.add("screen", Element::stack(), |children| {
            children.add("library", Element::column(), |children| {
                let field = Element::search_field(&this.filter).placeholder("Find a game");
                ids.filter = Some(children.add("filter", field, |_| {}));
                let grid = Element::grid(this.shown.len(), CELL)
                    .class("covers")
                    .selection(this.selected);
                ids.grid = Some(children.add_slot("grid", grid));
            });
            if let Some(game) = this.page.map(|game| &this.games[game]) {
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
    let Some(grid) = this.ids.grid else {
        return;
    };
    let covers = &mut this.ids.covers;
    ui.build(grid, this.cells, |children| {
        covers.clear();
        for place in children.rows() {
            let game = this.shown[place];
            let cover = Element::pressable().class("cover");
            let id = children.add(game, cover, |children| {
                let image = Element::image(this.games[game].cover);
                children.add("image", image, |_| {});
                let title = Element::label(&this.games[game].title);
                children.add("title", title, |_| {});
            });
            covers.push((id, place));
        }
    });
}

/// The library's state, by the names of its fields; the ids and revisions are left out.
fn write(library: &Library) -> Form {
    Form::from([
        ("filter", Field::Text(library.filter.clone())),
        ("selected", Field::Number(library.selected as u64)),
        (
            "page",
            library
                .page
                .map_or(Field::Absent, |page| Field::Number(page as u64)),
        ),
        ("dark", Field::Number(u64::from(library.dark))),
    ])
}

/// A library of `games` in the state that `form` holds, what it does not hold as a new one's.
fn read(form: &Form, games: Vec<Game>) -> Library {
    let mut library = Library::new(games);
    if let Some(Field::Text(filter)) = form.get("filter") {
        library.set_filter(filter.clone());
    }
    if let Some(Field::Number(selected)) = form.get("selected") {
        library.select(*selected as usize);
    }
    if let Some(Field::Number(page)) = form.get("page") {
        library.page = Some(*page as usize);
    }
    if let Some(Field::Number(dark)) = form.get("dark") {
        library.dark = *dark == 1;
    }
    library
}

#[test]
fn a_body_shows_once_its_builders_are_forgotten_and_a_state_migrates() {
    let mut host = reload::played(&TABLE);
    let before = reload::kept(&host);
    host.program.build = build_patched;
    reload::settled(&mut host);
    assert_eq!(
        reload::kept(&host).placeholder,
        before.placeholder,
        "no revision moved: no builder runs the new body"
    );
    let root = host.root;
    host.ui.forget_builds(root);
    reload::settled(&mut host);
    assert_eq!(
        reload::kept(&host).placeholder.as_deref(),
        Some("Find a game")
    );
    reload::assert_kept(&host, &before);
    let form = write(&host.program.library);
    host.program.library = read(&form, games());
    // The revisions and ids, the program's state too, start again.
    host.ui.forget_builds(root);
    reload::settled(&mut host);
    reload::assert_kept(&host, &before);
}
