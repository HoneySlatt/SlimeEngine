//! Slatty's library with views that say they changed: 10,000 covers in a grid that builds only
//! the cells that show, a filter typed a character at a time, the selection moved by the arrows,
//! a game's page opened, its key art fading in, then closed, and the theme changed.
//!
//! The model's concepts:
//! - `Render`, the trait of a view: `render`, which gives its elements
//! - `Cx`, the context that a view renders and handles its events in: `notify`, `listener`, a
//!   closure of an event given the view, `this`, and `processor`, one that gives a grid its cells
//! - the rule: a view that changed without `notify` shows what it showed
//! - `IntoElement`, what `render` gives
//! - a key, `key`, by which an element keeps its state from one render to the next
//! - the rule: an element without a key is matched by its place among its siblings
//! - `on_press`, `on_change`, `on_step`, `on_accept`, `on_cancel`, `on_command`: closures that
//!   events run
//! - `View`, what `Cx::new` makes of a value
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

use std::ops::Range;

use super::prelude::*;

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
        }
    }

    fn set_filter(&mut self, filter: &str, cx: &mut Cx<Self>) {
        self.shown = matching(&self.games, filter);
        self.filter = filter.to_owned();
        self.selected = 0;
        cx.notify();
    }

    fn select(&mut self, place: usize, cx: &mut Cx<Self>) {
        self.selected = place;
        cx.notify();
    }

    fn open(&mut self, place: usize, cx: &mut Cx<Self>) {
        self.page = self.shown.get(place).copied();
        cx.notify();
    }

    fn close(&mut self, cx: &mut Cx<Self>) {
        if self.page.take().is_some() {
            cx.notify();
        }
    }

    fn toggle_theme(&mut self, cx: &mut Cx<Self>) {
        self.dark = !self.dark;
        cx.notify();
    }

    fn cover(&self, place: usize, cx: &mut Cx<Self>) -> impl IntoElement + use<> {
        let game = self.shown[place];
        pressable((
            image(self.games[game].cover),
            label(self.games[game].title.clone()),
        ))
        .key(game)
        .class("cover")
        .on_press(cx.listener(move |this, _, cx| this.open(place, cx)))
    }

    fn game_page(&self, game: usize, cx: &mut Cx<Self>) -> impl IntoElement {
        column((
            button("Library").on_press(cx.listener(|this, _, cx| this.close(cx))),
            image(self.games[game].art).class("key-art"),
            label(self.games[game].title.clone()).class("title"),
        ))
        .class("page")
    }
}

impl Render for Library {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement {
        let filter = search_field(&self.filter)
            .placeholder("Search a game")
            .on_change(cx.listener(|this, filter: &String, cx| this.set_filter(filter, cx)));
        let covers = cx.processor(|this, places: Range<usize>, cx| {
            places.map(|place| this.cover(place, cx)).collect()
        });
        let grid = grid(self.shown.len(), CELL, covers)
            .class("covers")
            .selection(self.selected)
            .on_step(cx.listener(|this, &place, cx| this.select(place, cx)))
            .on_accept(cx.listener(|this, _, cx| this.open(this.selected, cx)));
        let page = self.page.map(|game| self.game_page(game, cx));
        stack((column((filter, grid)), page))
            .theme(if self.dark { "dark" } else { "light" })
            .on_cancel(cx.listener(|this, _, cx| this.close(cx)))
            .on_command(
                "toggle-theme",
                cx.listener(|this, _, cx| this.toggle_theme(cx)),
            )
    }
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
