//! Opium's songs with messages, as in Elm: 8,500 songs in a table that builds only the rows that
//! show, a column sorted by its header, the selection moved by the arrows, a song played by
//! Enter, its wave of progress moving in the bar below at each frame, and the theme changed.
//!
//! The model's concepts:
//! - `Message`, the program's type of what can happen
//! - `update`, which changes the state for a message, given the time of the frame it arrives in
//! - `view`, which describes the interface from the state, and runs again after each message
//! - `El`, what `view` gives, which borrows the state
//! - a key, `key`, by which an element keeps its state from one `view` to the next
//! - the rule: an element without a key is matched by its place among its siblings
//! - `lazy`, a part of the interface described again only when the value that it is given changes
//! - the rule: a `lazy` part shows what it showed if its value leaves out something that it reads
//! - `Subscription` and `frames`, a message at each frame while the state asks for it
//! - the rule: a subscription is asked for again after each message, and stops when it is no
//!   longer given
//! - `on_press`, `on_step`, `on_accept`, `on_command`: the message that each event sends
//!
//! Shared by the four models:
//! - one call that starts the application
//! - `column`, `row`, `label`, `button`, `image`
//! - `list`, which builds the rows that show, shows the place selected and steps from it
//! - `wave`, the wave of progress
//! - a theme by its name
//! - a class, which the style sheet styles, and `class_if`
//! - each frame's time, set before its input is handled
//! - `ImageKey`

use std::time::{Duration, Instant};

use super::prelude::*;

#[derive(Clone)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length: Duration,
    pub thumbnail: ImageKey,
}

/// A row's height, in logical pixels.
const ROW: f32 = 52.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Column {
    Title,
    Album,
    Length,
}

type Sort = Option<(Column, bool)>;

#[derive(Clone)]
pub struct Songs {
    songs: Vec<Song>,
    /// The songs in the table's order, by their places in `songs`.
    order: Vec<usize>,
    /// The column sorted, and whether it descends.
    sort: Sort,
    /// The place in `order` of the song selected.
    selected: usize,
    /// The song playing, and when it started.
    playing: Option<(usize, Instant)>,
    now: Instant,
    dark: bool,
}

#[derive(Clone, Debug)]
pub enum Message {
    Sort(Column),
    Select(usize),
    Play,
    Frame,
    ToggleTheme,
}

impl Songs {
    pub fn new(songs: Vec<Song>, now: Instant) -> Self {
        Self {
            order: (0..songs.len()).collect(),
            songs,
            sort: None,
            selected: 0,
            playing: None,
            now,
            dark: true,
        }
    }

    pub fn update(&mut self, message: Message, now: Instant) {
        self.now = now;
        match message {
            Message::Sort(column) => {
                self.sort = next_sort(self.sort, column);
                self.order = sorted(&self.songs, self.sort);
            }
            Message::Select(place) => self.selected = place,
            Message::Play => self.playing = Some((self.order[self.selected], now)),
            Message::Frame => {}
            Message::ToggleTheme => self.dark = !self.dark,
        }
    }

    pub fn view(&self) -> El<'_, Message> {
        let headers = [Column::Title, Column::Album, Column::Length].map(|column| {
            button(header_text(column, self.sort))
                .key(column as u64)
                .on_press(Message::Sort(column))
        });
        // The table, described again only when its order, its selection or the song playing
        // change, and not at each frame of the wave.
        let playing = self.playing.map(|(song, _)| song);
        let table = lazy((self.sort, self.selected, playing), || {
            list(self.order.len(), ROW, |place| self.song_row(place))
                .class("table")
                .selection(self.selected)
                .on_step(Message::Select)
                .on_accept(Message::Play)
        });
        column((row(headers).class("header"), table, self.toolbar()))
            .theme(if self.dark { "dark" } else { "light" })
            .on_command("toggle-theme", Message::ToggleTheme)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.playing.is_some() {
            frames().map(|_| Message::Frame)
        } else {
            Subscription::none()
        }
    }

    fn song_row(&self, place: usize) -> El<'_, Message> {
        let song = &self.songs[self.order[place]];
        let playing = self.playing.map(|(song, _)| song) == Some(self.order[place]);
        row((
            label((place + 1).to_string()).class("number"),
            image(song.thumbnail).class("thumbnail"),
            column((label(&song.title), label(&song.artist).class("muted"))).class("title"),
            label(&song.album).class("album"),
            label(length(song.length)).class("length"),
        ))
        .key(place)
        .class("song")
        .class_if("playing", playing)
        .into()
    }

    fn toolbar(&self) -> El<'_, Message> {
        let (title, shown) = match self.playing {
            Some((song, started)) => {
                let song = &self.songs[song];
                (
                    song.title.as_str(),
                    progress(self.now - started, song.length),
                )
            }
            None => ("", (0.0, 0.0)),
        };
        row((label(title), wave(shown))).class("toolbar").into()
    }
}

/// A header's text: its column's name, and an arrow where it is sorted.
fn header_text(column: Column, sort: Sort) -> String {
    let name = match column {
        Column::Title => "Title",
        Column::Album => "Album",
        Column::Length => "Length",
    };
    match sort {
        Some((sorted, descending)) if sorted == column => {
            format!("{name} {}", if descending { "▼" } else { "▲" })
        }
        _ => name.to_owned(),
    }
}

/// The sort after a press on `column`'s header: ascending, descending, then none.
fn next_sort(sort: Sort, column: Column) -> Sort {
    match sort {
        Some((sorted, false)) if sorted == column => Some((column, true)),
        Some((sorted, true)) if sorted == column => None,
        _ => Some((column, false)),
    }
}

/// The songs' places in `songs`, in the order of `sort`, or of the library without one.
fn sorted(songs: &[Song], sort: Sort) -> Vec<usize> {
    let mut order: Vec<usize> = (0..songs.len()).collect();
    if let Some((column, descending)) = sort {
        order.sort_by(|&a, &b| {
            let (a, b) = (&songs[a], &songs[b]);
            let ordering = match column {
                Column::Title => a.title.cmp(&b.title),
                Column::Album => a.album.cmp(&b.album),
                Column::Length => a.length.cmp(&b.length),
            };
            if descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }
    order
}

/// A song's length, in minutes and seconds.
fn length(length: Duration) -> String {
    let seconds = length.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The wave's share played and its phase, `elapsed` into a song of `length`: it travels at 4.2
/// radians a second, as Opium's.
fn progress(elapsed: Duration, length: Duration) -> (f32, f32) {
    let share = (elapsed.as_secs_f32() / length.as_secs_f32()).min(1.0);
    (share, elapsed.as_secs_f32() * 4.2)
}
