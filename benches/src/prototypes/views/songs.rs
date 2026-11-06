//! Opium's songs with views that say they changed: 8,500 songs in a table that builds only the
//! rows that show, a column sorted by its header, the selection moved by the arrows, a song
//! played by Enter, its wave of progress moving in the bar below at each frame, and the theme
//! changed. The bar is a view of its own, so that its frames render it alone.
//!
//! The model's concepts:
//! - `Render`, the trait of a view: `render`, which gives its elements
//! - `Cx`, the context that a view renders and handles its events in: `notify`, `listener`, a
//!   closure of an event given the view, `this`, `processor`, one that gives a list its rows,
//!   `request_frame`, which renders the view again at the next frame, and `frame_time`
//! - the rule: a view that changed without `notify` shows what it showed
//! - `IntoElement`, what `render` gives
//! - a key, `key`, by which an element keeps its state from one render to the next
//! - the rule: an element without a key is matched by its place among its siblings
//! - `on_press`, `on_step`, `on_accept`, `on_command`: closures that events run
//! - `View`, what `Cx::new` makes of a value, a view within a view: `update`, its value changed
//!   from another view
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

use std::ops::Range;
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

#[derive(Clone, Copy, PartialEq)]
enum Column {
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
    /// The song playing.
    playing: Option<usize>,
    dark: bool,
    toolbar: View<Toolbar>,
}

/// The bar below the table: the song playing, its title, its length and when it started.
#[derive(Clone)]
struct Toolbar {
    playing: Option<(String, Duration, Instant)>,
}

impl Songs {
    pub fn new(songs: Vec<Song>, cx: &mut Cx<Self>) -> Self {
        Self {
            order: (0..songs.len()).collect(),
            songs,
            sort: None,
            selected: 0,
            playing: None,
            dark: true,
            toolbar: cx.new(|_| Toolbar { playing: None }),
        }
    }

    fn sort_by(&mut self, column: Column, cx: &mut Cx<Self>) {
        self.sort = next_sort(self.sort, column);
        self.order = sorted(&self.songs, self.sort);
        cx.notify();
    }

    fn select(&mut self, place: usize, cx: &mut Cx<Self>) {
        self.selected = place;
        cx.notify();
    }

    fn play(&mut self, cx: &mut Cx<Self>) {
        let song = self.order[self.selected];
        self.playing = Some(song);
        let playing = (
            self.songs[song].title.clone(),
            self.songs[song].length,
            cx.frame_time(),
        );
        self.toolbar.update(cx, |toolbar, cx| {
            toolbar.playing = Some(playing);
            cx.notify();
        });
        cx.notify();
    }

    fn toggle_theme(&mut self, cx: &mut Cx<Self>) {
        self.dark = !self.dark;
        cx.notify();
    }

    fn song_row(&self, place: usize) -> impl IntoElement + use<> {
        let song = &self.songs[self.order[place]];
        row((
            label((place + 1).to_string()).class("number"),
            image(song.thumbnail).class("thumbnail"),
            column((
                label(song.title.clone()),
                label(song.artist.clone()).class("muted"),
            ))
            .class("title"),
            label(song.album.clone()).class("album"),
            label(length(song.length)).class("length"),
        ))
        .key(place)
        .class("song")
        .class_if("playing", self.playing == Some(self.order[place]))
    }
}

impl Render for Songs {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement {
        let headers = [Column::Title, Column::Album, Column::Length].map(|column| {
            button(header_text(column, self.sort))
                .key(column as u64)
                .on_press(cx.listener(move |this, _, cx| this.sort_by(column, cx)))
        });
        let rows = cx.processor(|this, places: Range<usize>, _| {
            places.map(|place| this.song_row(place)).collect()
        });
        let table = list(self.order.len(), ROW, rows)
            .class("table")
            .selection(self.selected)
            .on_step(cx.listener(|this, &place, cx| this.select(place, cx)))
            .on_accept(cx.listener(|this, _, cx| this.play(cx)));
        column((row(headers).class("header"), table, self.toolbar.clone()))
            .theme(if self.dark { "dark" } else { "light" })
            .on_command(
                "toggle-theme",
                cx.listener(|this, _, cx| this.toggle_theme(cx)),
            )
    }
}

impl Render for Toolbar {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement {
        let (title, shown) = match &self.playing {
            Some((title, length, started)) => {
                cx.request_frame();
                (title.clone(), progress(cx.frame_time() - *started, *length))
            }
            None => (String::new(), (0.0, 0.0)),
        };
        row((label(title), wave(shown))).class("toolbar")
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
