//! Slatty's library with signals: 10,000 covers in a grid that builds only the cells that show, a
//! filter typed a character at a time, the selection moved by the arrows, a game's page opened,
//! its key art fading in, then closed, and the theme changed.
//!
//! The model's concepts:
//! - `View`, what a view's function gives
//! - the rule: a view's function runs once; a closure that reads a signal runs again when it
//!   changes, and nothing else does
//! - `RwSignal`, a `Copy` handle to a value: `get`, `set`, which tells only of a value that
//!   changed, `update` and `with`
//! - `RwSignal::kept`, a signal named in its view, whose value a reload keeps, its type changed
//!   too, through `serde`; its first value given by a closure
//! - the rule: a signal not named starts again at each reload
//! - `Memo`, a value worked out from signals, again when they change
//! - `Stored`, a `Copy` handle to a value that no closure tracks
//! - a closure given where a value would be, `move || …`, which the view tracks
//! - `dyn_view`, a view made again, and the one before disposed, when what it reads changes
//! - a key, the closure that gives each of the grid's items its own
//! - `on_press`, `on_change`, `on_step`, `on_accept`, `on_cancel`, `on_command`: closures that
//!   events run
//! - the rule: a signal of a view disposed panics when it is read
//! - the rule: a signal read outside a closure that the view tracks is read once
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

use super::prelude::*;

#[derive(Clone)]
pub struct Game {
    pub title: String,
    pub cover: ImageKey,
    pub art: ImageKey,
}

/// A cover's cell, in logical pixels: 3 by 4.
const CELL: [f32; 2] = [150.0, 200.0];

pub fn library(games: Vec<Game>) -> impl View {
    let games = Stored::new(games);
    let filter = RwSignal::kept("filter", String::new);
    // The games that the filter keeps, by their places in `games`.
    let shown = Memo::new(move || games.with(|games| matching(games, &filter.get())));
    // The place in `shown` of the game selected.
    let selected = RwSignal::kept("selected", || 0);
    // The game whose page is open.
    let page = RwSignal::kept("page", || None);
    let dark = RwSignal::kept("dark", || true);
    let open = move |place: usize| page.set(shown.with(|shown| shown.get(place).copied()));
    stack((
        column((
            search_field(filter)
                .placeholder("Search a game")
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

fn cover(games: Stored<Vec<Game>>, game: usize, page: RwSignal<Option<usize>>) -> impl View {
    games.with(|games| {
        pressable((image(games[game].cover), label(games[game].title.clone())))
            .class("cover")
            .on_press(move || page.set(Some(game)))
    })
}

fn game_page(games: Stored<Vec<Game>>, game: usize, page: RwSignal<Option<usize>>) -> impl View {
    games.with(|games| {
        column((
            button("Library").on_press(move || page.set(None)),
            image(games[game].art).class("key-art"),
            label(games[game].title.clone()).class("title"),
        ))
        .class("page")
    })
}

/// The games whose titles hold `filter`, whatever their case.
fn matching(games: &[Game], filter: &str) -> Vec<usize> {
    let filter = filter.to_lowercase();
    (0..games.len())
        .filter(|&game| games[game].title.to_lowercase().contains(&filter))
        .collect()
}

#[cfg(test)]
mod reload;
