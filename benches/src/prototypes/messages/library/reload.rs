//! Messages' library reloaded: its entry point, `view`, a function's address that the
//! application holds, swapped for a body whose placeholder differs; and its state written to a
//! form and read back.

use super::*;
use crate::prototypes::data;
use crate::prototypes::messages::LIBRARY;
use crate::prototypes::reload::{self, Field, Form};

/// `view`'s body, its placeholder changed.
fn view_patched(this: &Library) -> El<'_, Message> {
    let covers = grid(this.shown.len(), CELL, |place| this.cover(place))
        .class("covers")
        .selection(this.selected)
        .on_step(Message::Select)
        .on_accept(Message::Open(this.selected));
    stack((
        column((
            search_field(&this.filter)
                .placeholder("Find a game")
                .on_change(Message::Filter),
            covers,
        )),
        this.page.map(|game| this.game_page(game)),
    ))
    .theme(if this.dark { "dark" } else { "light" })
    .on_cancel(Message::Close)
    .on_command("toggle-theme", Message::ToggleTheme)
    .into()
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
        library.update(Message::Filter(filter.clone()));
    }
    if let Some(Field::Number(selected)) = form.get("selected") {
        library.update(Message::Select(*selected as usize));
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
fn a_body_shows_once_view_runs_and_a_state_migrates() {
    let mut host = reload::played(&LIBRARY);
    let before = reload::kept(&host);
    host.program.view = view_patched;
    // What a reload asks of the application: `view` again, whole.
    host.program.viewed = false;
    reload::settled(&mut host);
    assert_eq!(
        reload::kept(&host).placeholder.as_deref(),
        Some("Find a game")
    );
    reload::assert_kept(&host, &before);
    let form = write(&host.program.state);
    host.program.state = read(&form);
    host.program.viewed = false;
    reload::settled(&mut host);
    reload::assert_kept(&host, &before);
}
