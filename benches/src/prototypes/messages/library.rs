//! Slatty's library with messages, as in Elm: 10,000 covers in a grid that builds only the cells
//! that show, a filter typed a character at a time, the selection moved by the arrows, a game's
//! page opened, its key art fading in, then closed, and the theme changed.
//!
//! The model's concepts:
//! - `Message`, the program's type of what can happen
//! - `update`, which changes the state for a message
//! - `view`, which describes the interface from the state, and runs again after each message
//! - `El`, what `view` gives, which borrows the state
//! - a key, `key`, by which an element keeps its state from one `view` to the next
//! - the rule: an element without a key is matched by its place among its siblings
//! - `on_press`, `on_change`, `on_step`, `on_accept`, `on_cancel`, `on_command`: the message that
//!   each event sends
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

#[derive(Clone, Debug)]
pub enum Message {
    Filter(String),
    Select(usize),
    Open(usize),
    Close,
    ToggleTheme,
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

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Filter(filter) => {
                self.shown = matching(&self.games, &filter);
                self.filter = filter;
                self.selected = 0;
            }
            Message::Select(place) => self.selected = place,
            Message::Open(place) => self.page = self.shown.get(place).copied(),
            Message::Close => self.page = None,
            Message::ToggleTheme => self.dark = !self.dark,
        }
    }

    pub fn view(&self) -> El<'_, Message> {
        let covers = grid(self.shown.len(), CELL, |place| self.cover(place))
            .class("covers")
            .selection(self.selected)
            .on_step(Message::Select)
            .on_accept(Message::Open(self.selected));
        stack((
            column((
                search_field(&self.filter)
                    .placeholder("Search a game")
                    .on_change(Message::Filter),
                covers,
            )),
            self.page.map(|game| self.game_page(game)),
        ))
        .theme(if self.dark { "dark" } else { "light" })
        .on_cancel(Message::Close)
        .on_command("toggle-theme", Message::ToggleTheme)
        .into()
    }

    fn cover(&self, place: usize) -> El<'_, Message> {
        let game = &self.games[self.shown[place]];
        pressable((image(game.cover), label(&game.title)))
            .key(self.shown[place])
            .class("cover")
            .on_press(Message::Open(place))
            .into()
    }

    fn game_page(&self, game: usize) -> El<'_, Message> {
        let game = &self.games[game];
        column((
            button("Library").on_press(Message::Close),
            image(game.art).class("key-art"),
            label(&game.title).class("title"),
        ))
        .class("page")
        .into()
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
