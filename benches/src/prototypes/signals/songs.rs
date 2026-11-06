//! Opium's songs with signals: 8,500 songs in a table that builds only the rows that show, a
//! column sorted by its header, the selection moved by the arrows, a song played by Enter, its
//! wave of progress moving in the bar below at each frame, and the theme changed.
//!
//! The model's concepts:
//! - `View`, what a view's function gives
//! - the rule: a view's function runs once; a closure that reads a signal runs again when it
//!   changes, and nothing else does
//! - `RwSignal`, a `Copy` handle to a value: `get`, `set`, which tells only of a value that
//!   changed, `update` and `with`
//! - `Memo`, a value worked out from signals, again when they change
//! - `Stored`, a `Copy` handle to a value that no closure tracks
//! - `Signal`, a `Copy` handle to a value that it only reads
//! - `frame_time`, the signal of each frame's time, which asks for frames while a closure that
//!   the view tracks reads it
//! - a closure given where a value would be, `move || …`, which the view tracks
//! - `on_press`, `on_step`, `on_accept`, `on_command`: closures that events run
//! - the rule: a signal of a view disposed panics when it is read
//! - the rule: a signal read outside a closure that the view tracks is read once
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

#[derive(Clone, Copy, PartialEq)]
enum Column {
    Title,
    Album,
    Length,
}

type Sort = Option<(Column, bool)>;

pub fn songs(songs: Vec<Song>) -> impl View {
    let songs = Stored::new(songs);
    // The column sorted, and whether it descends.
    let sort = RwSignal::new(None);
    // The songs in the table's order, by their places in `songs`.
    let order = Memo::new(move || songs.with(|songs| sorted(songs, sort.get())));
    // The place in `order` of the song selected.
    let selected = RwSignal::new(0);
    // The song playing, and when it started.
    let playing = RwSignal::new(None::<(usize, Instant)>);
    let dark = RwSignal::new(true);
    let clock = frame_time();
    column((
        row([Column::Title, Column::Album, Column::Length].map(|column| header(column, sort)))
            .class("header"),
        list(
            move || order.with(Vec::len),
            ROW,
            move |place| song_row(songs, place, order, playing),
        )
        .class("table")
        .selection(move || selected.get())
        .on_step(move |place| selected.set(place))
        .on_accept(move || {
            let song = order.with(|order| order[selected.get()]);
            playing.set(Some((song, clock.get())));
        }),
        toolbar(songs, playing, clock),
    ))
    .theme(move || if dark.get() { "dark" } else { "light" })
    .on_command("toggle-theme", move || dark.update(|dark| *dark = !*dark))
}

fn header(column: Column, sort: RwSignal<Sort>) -> impl View {
    button(move || header_text(column, sort.get()))
        .on_press(move || sort.update(|sort| *sort = next_sort(*sort, column)))
}

fn song_row(
    songs: Stored<Vec<Song>>,
    place: usize,
    order: Memo<Vec<usize>>,
    playing: RwSignal<Option<(usize, Instant)>>,
) -> impl View {
    let song = move || order.with(|order| order[place]);
    row((
        label((place + 1).to_string()).class("number"),
        image(move || songs.with(|songs| songs[song()].thumbnail)).class("thumbnail"),
        column((
            label(move || songs.with(|songs| songs[song()].title.clone())),
            label(move || songs.with(|songs| songs[song()].artist.clone())).class("muted"),
        ))
        .class("title"),
        label(move || songs.with(|songs| songs[song()].album.clone())).class("album"),
        label(move || songs.with(|songs| length(songs[song()].length))).class("length"),
    ))
    .class("song")
    .class_if("playing", move || {
        playing.with(|playing| playing.map(|(song, _)| song)) == Some(song())
    })
}

fn toolbar(
    songs: Stored<Vec<Song>>,
    playing: RwSignal<Option<(usize, Instant)>>,
    clock: Signal<Instant>,
) -> impl View {
    let title = move || {
        let song = playing.get().map(|(song, _)| song);
        song.map(|song| songs.with(|songs| songs[song].title.clone()))
            .unwrap_or_default()
    };
    let shown = move || match playing.get() {
        Some((song, started)) => {
            songs.with(|songs| progress(clock.get() - started, songs[song].length))
        }
        None => (0.0, 0.0),
    };
    row((label(title), wave(shown))).class("toolbar")
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
