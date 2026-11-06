//! The control, today's rebuild by keys, prototyped from its ideal code
//! (`docs/ideal-code/control/` in uwui's repository): each file that code, changed only where the
//! prototype forced it, and the control's own mistake.

pub mod document;
pub mod hello;
pub mod library;
pub mod mistake;
pub mod songs;

use super::data;
use super::screens::{self, Edit, Screen, screen};

pub(crate) const HELLO: Screen<hello::Hello> =
    screen(screens::HELLO, |_| hello::Hello::default(), screens::hello);

pub(crate) const LIBRARY: Screen<library::Library> = screen(
    screens::LIBRARY,
    |_| {
        let games = data::games(10_000, |title, cover, art| library::Game {
            title,
            cover,
            art,
        });
        library::Library::new(games)
    },
    screens::library,
);

pub(crate) const SONGS: Screen<songs::Songs> = screen(
    screens::SONGS,
    |now| {
        let songs = data::songs(8_500, |(title, artist, album, length, thumbnail)| {
            songs::Song {
                title,
                artist,
                album,
                length,
                thumbnail,
            }
        });
        songs::Songs::new(songs, now)
    },
    screens::songs,
);

pub(crate) const DOCUMENT: Screen<document::Document> = screen(
    screens::DOCUMENT,
    |_| document::Document::new(data::lines(100_000)),
    screens::document,
);

impl Edit for document::Document {
    fn edit(&mut self, line: usize, text: String) {
        document::Document::edit(self, line, text);
    }
}

/// The control's own mistake, a revision not moved.
pub(crate) const REVISION_NOT_MOVED: Screen<mistake::Forgotten> = screen(
    screens::small("A revision not moved"),
    |_| mistake::Forgotten::new(),
    screens::selection,
);

/// A grid of covers, matched by their games or, the mistake, by their places.
#[cfg(test)]
pub(crate) const COVERS: [Screen<mistake::Covers>; 2] = [
    screen(
        screens::SELECTION,
        |_| mistake::Covers::new(true),
        screens::removed,
    ),
    screen(
        screens::SELECTION,
        |_| mistake::Covers::new(false),
        screens::removed,
    ),
];

#[cfg(test)]
impl screens::Removes for mistake::Covers {
    fn remove_first(&mut self) {
        mistake::Covers::remove_first(self);
    }
}
