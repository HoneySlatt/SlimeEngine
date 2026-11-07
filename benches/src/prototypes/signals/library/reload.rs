//! Signals' library reloaded: its view's function run again for a body whose placeholder differs,
//! then for changed types, each kept signal taking the value that the one of its name held. A
//! signal added before the others, which shifted their order on M6d.1b's bench, keeps them; a
//! signal renamed, and a kept value that no longer reads as its type, start again, and say so.

use super::*;
use crate::prototypes::data;
use crate::prototypes::reload;
use crate::prototypes::signals::LIBRARY;

fn games() -> Vec<Game> {
    data::games(10_000, |title, cover, art| Game { title, cover, art })
}

/// The library's view, its placeholder `placeholder`, its selection kept as `selected`, its page
/// as `page`, and a signal added before the selection if `added`.
fn changed(
    games: Vec<Game>,
    placeholder: &'static str,
    selected: &'static str,
    page: RwSignal<Option<usize>>,
    added: bool,
) -> impl View {
    let games = Stored::new(games);
    let filter = RwSignal::kept("filter", String::new);
    let shown = Memo::new(move || games.with(|games| matching(games, &filter.get())));
    if added {
        // A signal that the changed type adds, before those that it keeps.
        let _columns = RwSignal::kept("columns", || 4_usize);
    }
    let selected = RwSignal::kept(selected, || 0);
    let dark = RwSignal::kept("dark", || true);
    let open = move |place: usize| page.set(shown.with(|shown| shown.get(place).copied()));
    stack((
        column((
            search_field(filter)
                .placeholder(placeholder)
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

/// The library's page, kept as `page`, as the screen has it.
fn page() -> RwSignal<Option<usize>> {
    RwSignal::kept("page", || None)
}

#[test]
fn a_body_shows_and_the_state_is_kept() {
    let mut host = reload::played(&LIBRARY);
    let before = reload::kept(&host);
    host.program.reload(|| {
        (
            (),
            changed(games(), "Find a game", "selected", page(), false),
        )
    });
    reload::settled(&mut host);
    assert_eq!(
        reload::kept(&host).placeholder.as_deref(),
        Some("Find a game")
    );
    reload::assert_kept(&host, &before);
    assert_eq!(host.program.notes(), Vec::<String>::new());
}

#[test]
fn a_signal_added_before_the_others_keeps_their_state() {
    let mut host = reload::played(&LIBRARY);
    let before = reload::kept(&host);
    host.program.reload(|| {
        (
            (),
            changed(games(), "Find a game", "selected", page(), true),
        )
    });
    reload::settled(&mut host);
    reload::assert_kept(&host, &before);
    assert_eq!(host.program.notes(), Vec::<String>::new());
}

#[test]
fn a_signal_renamed_starts_again_and_says_so() {
    let mut host = reload::played(&LIBRARY);
    let before = reload::kept(&host);
    host.program
        .reload(|| ((), changed(games(), "Find a game", "cursor", page(), false)));
    reload::settled(&mut host);
    let after = reload::kept(&host);
    assert_eq!(after.selection, Some(0), "the selection started again");
    assert_eq!(after.field, before.field, "the filter kept");
    assert!(after.page, "the page kept");
    assert_eq!(
        host.program.notes(),
        ["`selected` at [] taken by no signal"]
    );
}

#[test]
fn a_value_whose_type_no_longer_reads_starts_again_and_says_so() {
    let mut host = reload::played(&LIBRARY);
    let before = reload::kept(&host);
    // The page now held as the game's title, which a place written as a number does not read as.
    host.program.reload(|| {
        let titled = RwSignal::kept("page", || None::<String>);
        let page = RwSignal::new(None);
        let _ = titled;
        ((), changed(games(), "Find a game", "selected", page, false))
    });
    reload::settled(&mut host);
    let after = reload::kept(&host);
    assert!(!after.page, "the page started again");
    assert_eq!(after.selection, before.selection, "the selection kept");
    let notes = host.program.notes();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with("`page` started again: invalid type: integer"),
        "{notes:?}"
    );
}
