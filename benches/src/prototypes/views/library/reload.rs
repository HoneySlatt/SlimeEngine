//! Views' library reloaded: its entry point, `render`, called through a table of bodies by the
//! view's type, which the test gives a body whose placeholder differs; and its state written to a
//! form and read back.

use super::*;
use crate::prototypes::data;
use crate::prototypes::reload::{self, Field, Form};
use crate::prototypes::views::{El, LIBRARY};

/// `render`'s body, its placeholder changed.
fn render_patched(this: &mut Library, cx: &mut Cx<Library>) -> El {
    let filter = search_field(&this.filter)
        .placeholder("Find a game")
        .on_change(cx.listener(|this, filter: &String, cx| this.set_filter(filter, cx)));
    let covers = cx.processor(|this, places: Range<usize>, cx| {
        places.map(|place| this.cover(place, cx)).collect()
    });
    let grid = grid(this.shown.len(), CELL, covers)
        .class("covers")
        .selection(this.selected)
        .on_step(cx.listener(|this, &place, cx| this.select(place, cx)))
        .on_accept(cx.listener(|this, _, cx| this.open(this.selected, cx)));
    let page = this.page.map(|game| this.game_page(game, cx));
    stack((column((filter, grid)), page))
        .theme(if this.dark { "dark" } else { "light" })
        .on_cancel(cx.listener(|this, _, cx| this.close(cx)))
        .on_command(
            "toggle-theme",
            cx.listener(|this, _, cx| this.toggle_theme(cx)),
        )
}

/// The library's state, by the names of its fields.
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

/// A library in the state that `form` holds, what it does not hold as a new one's.
fn read(form: &Form) -> Library {
    let games = data::games(10_000, |title, cover, art| Game { title, cover, art });
    let mut library = Library::new(games);
    if let Some(Field::Text(filter)) = form.get("filter") {
        library.shown = matching(&library.games, filter);
        library.filter = filter.clone();
    }
    if let Some(Field::Number(selected)) = form.get("selected") {
        library.selected = *selected as usize;
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
fn a_body_shows_once_every_view_renders_and_a_state_migrates() {
    let mut host = reload::played(&LIBRARY);
    let before = reload::kept(&host);
    host.program.app.patch(render_patched);
    reload::settled(&mut host);
    assert_eq!(
        reload::kept(&host).placeholder.as_deref(),
        Some("Find a game")
    );
    reload::assert_kept(&host, &before);
    let root = host.program.root.clone();
    let form = root.update(&mut host.program.app, |library, _| write(library));
    root.update(&mut host.program.app, |library, cx| {
        *library = read(&form);
        cx.notify();
    });
    reload::settled(&mut host);
    reload::assert_kept(&host, &before);
}
