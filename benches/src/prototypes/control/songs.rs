//! Opium's songs in the control, today's rebuild by keys: 8,500 songs in a table that builds only
//! the rows that show, a column sorted by its header, the selection moved by the arrows, a song
//! played by Enter, its wave of progress moving in the bar below at each frame, and the theme
//! changed.
//!
//! The model's concepts:
//! - `Program`, the trait that the host calls at each frame: `advance` with the frame's time,
//!   `act` for each action, then `build`
//! - `Ui`, the interface: `build`, which runs a builder when its revision moved, `set_theme`, and
//!   `request_frame`
//! - `Children`, what a builder is given: `add`, `add_slot`, and `rows`, the rows that show
//! - `ElementId`, what `add` returns, kept to read the actions
//! - `Action`, an element and what was done to it
//! - `ActionKind`: `Press`, `Step`, `Intent` and `Command`
//! - `Intent`, what a key means: `Accept`
//! - a key, given to each element added, by which it keeps its state from one build to the next
//! - the rule: each change moves the revision of each builder that reads what it changed
//! - the rule: an id kept is valid until the builder that made it runs again
//! - the rule: a frame is asked for while something moves
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

use uwui::prelude::*;

pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length: Duration,
    pub thumbnail: ImageKey,
}

/// A row's height, in logical pixels.
const ROW: f32 = 52.0;

#[derive(Clone, Copy, PartialEq)]
enum Column {
    Title,
    Album,
    Length,
}

type Sort = Option<(Column, bool)>;

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
    /// Moved when the sort, the selection or the song playing change.
    revision: u64,
    /// Moved when the rows show another order or song playing.
    rows: u64,
    /// Moved at each frame while a song plays.
    wave: u64,
    ids: Ids,
}

/// The elements that the actions name, from the builds that made them.
#[derive(Default)]
struct Ids {
    headers: Vec<(ElementId, Column)>,
    list: Option<ElementId>,
    wave: Option<ElementId>,
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
            revision: 0,
            rows: 0,
            wave: 0,
            ids: Ids::default(),
        }
    }

    fn sort_by(&mut self, column: Column) {
        self.sort = next_sort(self.sort, column);
        self.order = sorted(&self.songs, self.sort);
        self.revision += 1;
        self.rows += 1;
    }

    fn play(&mut self) {
        self.playing = Some((self.order[self.selected], self.now));
        self.revision += 1;
        self.rows += 1;
    }
}

impl Program for Songs {
    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        let ids = &mut self.ids;
        ui.build(root, self.revision, |children| {
            ids.headers.clear();
            children.add("songs", Element::column(), |children| {
                children.add("header", Element::row().class("header"), |children| {
                    for column in [Column::Title, Column::Album, Column::Length] {
                        let header = Element::button(header_text(column, self.sort));
                        ids.headers
                            .push((children.add(column as u64, header, |_| {}), column));
                    }
                });
                let list = Element::list(self.order.len(), ROW)
                    .class("table")
                    .selection(self.selected);
                ids.list = Some(children.add_slot("list", list));
                children.add("toolbar", Element::row().class("toolbar"), |children| {
                    let title = self
                        .playing
                        .map(|(song, _)| self.songs[song].title.as_str());
                    children.add("title", Element::label(title.unwrap_or("")), |_| {});
                    ids.wave = Some(children.add_slot("wave", Element::row()));
                });
            });
        });
        if let Some(list) = self.ids.list {
            ui.build(list, self.rows, |children| {
                for place in children.rows() {
                    let song = &self.songs[self.order[place]];
                    let playing = self.playing.map(|(song, _)| song) == Some(self.order[place]);
                    let row = Element::row().class("song").class_if("playing", playing);
                    children.add(place, row, |children| {
                        let number = Element::label((place + 1).to_string()).class("number");
                        children.add("number", number, |_| {});
                        let thumbnail = Element::image(song.thumbnail).class("thumbnail");
                        children.add("thumbnail", thumbnail, |_| {});
                        children.add("title", Element::column().class("title"), |children| {
                            children.add("title", Element::label(&song.title), |_| {});
                            let artist = Element::label(&song.artist).class("muted");
                            children.add("artist", artist, |_| {});
                        });
                        let album = Element::label(&song.album).class("album");
                        children.add("album", album, |_| {});
                        let length = Element::label(length(song.length)).class("length");
                        children.add("length", length, |_| {});
                    });
                }
            });
        }
        if let Some(wave) = self.ids.wave {
            ui.build(wave, self.wave, |children| {
                let (share, phase) = match self.playing {
                    Some((song, started)) => progress(self.now - started, self.songs[song].length),
                    None => (0.0, 0.0),
                };
                children.add("wave", Element::wave(share, phase), |_| {});
            });
        }
    }

    fn act(&mut self, ui: &mut Ui, action: Action) {
        let element = action.element;
        match action.kind {
            ActionKind::Press => {
                let header = self.ids.headers.iter().find(|(id, _)| Some(*id) == element);
                if let Some(&(_, column)) = header {
                    self.sort_by(column);
                }
            }
            ActionKind::Step(place) if element == self.ids.list => {
                self.selected = place;
                self.revision += 1;
            }
            ActionKind::Intent(Intent::Accept) if element == self.ids.list => self.play(),
            ActionKind::Command(command) if command == "toggle-theme" => {
                self.dark = !self.dark;
                ui.set_theme(if self.dark { "dark" } else { "light" });
            }
            _ => {}
        }
    }

    fn advance(&mut self, ui: &mut Ui, now: Instant) {
        self.now = now;
        if self.playing.is_some() {
            self.wave += 1;
            ui.request_frame();
        }
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

fn main() -> uwui::Result {
    uwui::run(Songs::new(uwui::sample::songs(8_500), Instant::now()))
}
