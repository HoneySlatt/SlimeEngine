//! Signals' library reloaded: its entry point, the view's function, run again for a body whose
//! placeholder differs, its signals' values replayed in the order that they are made; and a
//! changed type, a signal added before the others, whose state that order does not keep.

use super::*;
use crate::prototypes::data;
use crate::prototypes::reload;
use crate::prototypes::signals::LIBRARY;

fn games() -> Vec<Game> {
    data::games(10_000, |title, cover, art| Game { title, cover, art })
}

/// The view's function, its placeholder changed.
fn library_patched(games: Vec<Game>) -> impl View {
    let games = Stored::new(games);
    let filter = RwSignal::new(String::new());
    // The games that the filter keeps, by their places in `games`.
    let shown = Memo::new(move || games.with(|games| matching(games, &filter.get())));
    // The place in `shown` of the game selected.
    let selected = RwSignal::new(0);
    // The game whose page is open.
    let page = RwSignal::new(None);
    let dark = RwSignal::new(true);
    let open = move |place: usize| page.set(shown.with(|shown| shown.get(place).copied()));
    stack((
        column((
            search_field(filter)
                .placeholder("Find a game")
                .on_change(move |_| selected.set(0)),
            grid(
                move || shown.get(),
                |&game| game,
                CELL,
                move |game| cover(games, game, page),
            )
            .class("covers")
            .selection(move || selected.get())
            .on_step(move |place| selected.set(place))
            .on_accept(move || open(selected.get())),
        )),
        dyn_view(move || page.get().map(|game| game_page(games, game, page))),
    ))
    .theme(move || if dark.get() { "dark" } else { "light" })
    .on_cancel(move || page.set(None))
    .on_command("toggle-theme", move || dark.update(|dark| *dark = !*dark))
}

/// The view's function, its placeholder changed and a signal added before `selected`.
fn library_v2(games: Vec<Game>) -> impl View {
    let games = Stored::new(games);
    let filter = RwSignal::new(String::new());
    // The games that the filter keeps, by their places in `games`.
    let shown = Memo::new(move || games.with(|games| matching(games, &filter.get())));
    // A signal that the changed type adds, before those that it keeps.
    let _columns = RwSignal::new(4_usize);
    // The place in `shown` of the game selected.
    let selected = RwSignal::new(0);
    // The game whose page is open.
    let page = RwSignal::new(None);
    let dark = RwSignal::new(true);
    let open = move |place: usize| page.set(shown.with(|shown| shown.get(place).copied()));
    stack((
        column((
            search_field(filter)
                .placeholder("Find a game")
                .on_change(move |_| selected.set(0)),
            grid(
                move || shown.get(),
                |&game| game,
                CELL,
                move |game| cover(games, game, page),
            )
            .class("covers")
            .selection(move || selected.get())
            .on_step(move |place| selected.set(place))
            .on_accept(move || open(selected.get())),
        )),
        dyn_view(move || page.get().map(|game| game_page(games, game, page))),
    ))
    .theme(move || if dark.get() { "dark" } else { "light" })
    .on_cancel(move || page.set(None))
    .on_command("toggle-theme", move || dark.update(|dark| *dark = !*dark))
}

#[test]
fn a_body_shows_once_the_view_runs_again_but_a_changed_type_loses_its_state() {
    let mut host = reload::played(&LIBRARY);
    let before = reload::kept(&host);
    host.program.reload(|| ((), library_patched(games())));
    reload::settled(&mut host);
    assert_eq!(
        reload::kept(&host).placeholder.as_deref(),
        Some("Find a game")
    );
    reload::assert_kept(&host, &before);
    host.program.reload(|| ((), library_v2(games())));
    reload::settled(&mut host);
    let after = reload::kept(&host);
    // The new signal takes the selection's value, the selection the page's, which its type
    // refuses: both start again.
    assert_eq!(after.field, before.field, "the filter, made before, kept");
    assert_eq!(after.selection, Some(0), "the selection lost");
    assert!(!after.page, "the page closed");
}
