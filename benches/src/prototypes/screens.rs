//! The screens of ADR-0202 in the control, each with its scripted interactions, in the order the
//! bench plays them (`docs/adr/0202-how-m6d1-compares-ways-of-writing-an-application.md`).

use std::time::Instant;

use slime_input::NamedKey;
use uwui::tree::Key;

use super::control::document::Document;
use super::control::hello::Hello;
use super::control::library::Library;
use super::control::mistake::Forgotten;
use super::control::songs::Songs;
use super::data;
use super::host::{Input, Interaction};

/// A screen: its name, its style sheet, its program with its data made, and its interactions.
pub(crate) struct Screen<P> {
    pub(crate) name: &'static str,
    pub(crate) sheet: (&'static str, &'static str),
    pub(crate) make: fn(Instant) -> P,
    pub(crate) script: fn(Scale) -> Vec<Interaction<P>>,
}

/// How long the long interactions run: whole, as the screens of reference run them, or cut short
/// for a test in a debug build.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scale {
    Whole,
    Short,
}

/// A path below a screen's root.
fn path<const N: usize>(keys: [Key; N]) -> Vec<Key> {
    keys.into()
}

fn name(name: &str) -> Key {
    Key::from(name)
}

/// One frame of each input of `inputs`.
fn each<P>(inputs: impl IntoIterator<Item = Input<P>>) -> Vec<Vec<Input<P>>> {
    inputs.into_iter().map(|input| vec![input]).collect()
}

/// The interaction that opens a screen: its first frames, until it is still.
fn open<P>() -> Interaction<P> {
    Interaction {
        name: "opened",
        frames: vec![Vec::new()],
    }
}

pub(crate) const HELLO: Screen<Hello> = Screen {
    name: "\"Hello, World!\"",
    sheet: ("hello.style", include_str!("sheets/hello.style")),
    make: |_| Hello::default(),
    script: |_| {
        vec![
            open(),
            Interaction {
                name: "its button pressed",
                frames: vec![vec![
                    Input::PointerAt(path([name("column"), name("go")])),
                    Input::Click,
                ]],
            },
        ]
    },
};

/// The logical pixels that a frame of the screens of reference scrolls.
const SCROLL: f32 = 16.0;

pub(crate) const LIBRARY: Screen<Library> = Screen {
    name: "Slatty's library",
    sheet: ("library.style", include_str!("sheets/library.style")),
    make: |_| Library::new(data::games(10_000)),
    script: |scale| {
        let grid = path([name("screen"), name("library"), name("grid")]);
        let frames = if scale == Scale::Whole { 1_200 } else { 60 };
        let mut scroll = vec![vec![Input::PointerAt(grid), Input::Wheel(SCROLL)]];
        scroll.extend((1..frames).map(|_| vec![Input::Wheel(SCROLL)]));
        vec![
            open(),
            Interaction {
                name: "scrolled, 16 logical pixels a frame",
                frames: scroll,
            },
            Interaction {
                name: "the filter focused by Tab",
                frames: each([Input::Key(NamedKey::Tab)]),
            },
            Interaction {
                name: "a filter typed, a character a frame",
                frames: each("star".chars().map(Input::Type)),
            },
            Interaction {
                name: "the grid focused by Tab",
                frames: each([Input::Key(NamedKey::Tab)]),
            },
            Interaction {
                name: "the selection moved by the arrows",
                frames: each([
                    Input::Key(NamedKey::ArrowRight),
                    Input::Key(NamedKey::ArrowRight),
                    Input::Key(NamedKey::ArrowDown),
                ]),
            },
            Interaction {
                name: "a game's page opened, its key art fading in",
                frames: each([Input::Key(NamedKey::Enter)]),
            },
            Interaction {
                name: "the page closed",
                frames: each([Input::Key(NamedKey::Escape)]),
            },
            Interaction {
                name: "the theme changed",
                frames: each([Input::Control('t')]),
            },
        ]
    },
};

pub(crate) const SONGS: Screen<Songs> = Screen {
    name: "Opium's songs",
    sheet: ("songs.style", include_str!("sheets/songs.style")),
    make: |now| Songs::new(data::songs(8_500), now),
    script: |scale| {
        let list = path([name("songs"), name("list")]);
        // From the first song to the last, 52 logical pixels a row.
        let frames = if scale == Scale::Whole {
            (8_500.0 * 52.0 / SCROLL) as usize
        } else {
            60
        };
        let mut scroll = vec![vec![Input::PointerAt(list), Input::Wheel(SCROLL)]];
        scroll.extend((1..frames).map(|_| vec![Input::Wheel(SCROLL)]));
        let playing = if scale == Scale::Whole { 240 } else { 10 };
        vec![
            open(),
            Interaction {
                name: "scrolled from end to end, 16 logical pixels a frame",
                frames: scroll,
            },
            Interaction {
                name: "a column sorted by a click on its header",
                frames: vec![vec![
                    Input::PointerAt(path([name("songs"), name("header"), Key::Index(0)])),
                    Input::Click,
                ]],
            },
            Interaction {
                name: "the table focused by Tab, from the header",
                frames: each([
                    Input::Key(NamedKey::Tab),
                    Input::Key(NamedKey::Tab),
                    Input::Key(NamedKey::Tab),
                ]),
            },
            Interaction {
                name: "the selection moved by the arrows",
                frames: each([
                    Input::Key(NamedKey::ArrowDown),
                    Input::Key(NamedKey::ArrowDown),
                    Input::Key(NamedKey::ArrowDown),
                ]),
            },
            Interaction {
                name: "the theme changed",
                frames: each([Input::Control('t')]),
            },
            Interaction {
                name: "a song played by Enter",
                frames: each([Input::Key(NamedKey::Enter)]),
            },
            Interaction {
                name: "each frame while a song plays, its wave moving",
                frames: (0..playing).map(|_| Vec::new()).collect(),
            },
        ]
    },
};

pub(crate) const DOCUMENT: Screen<Document> = Screen {
    name: "A document of 100,000 lines",
    sheet: ("document.style", include_str!("sheets/document.style")),
    make: |_| Document::new(data::lines(100_000)),
    script: |_| {
        vec![
            open(),
            Interaction {
                name: "a line that shows changed",
                frames: each([Input::Call(|document: &mut Document| {
                    document.edit(10, "a line changed".to_owned());
                })]),
            },
            Interaction {
                name: "a line that does not show changed",
                frames: each([Input::Call(|document: &mut Document| {
                    document.edit(50_000, "a line changed".to_owned());
                })]),
            },
        ]
    },
};

pub(crate) const MISTAKE: Screen<Forgotten> = Screen {
    name: "The control's mistake, a revision not moved",
    sheet: ("mistake.style", include_str!("sheets/mistake.style")),
    make: |_| Forgotten::new(),
    script: |_| {
        vec![
            open(),
            Interaction {
                name: "the list focused by Tab",
                frames: each([Input::Key(NamedKey::Tab)]),
            },
            Interaction {
                name: "the selection moved by an arrow",
                frames: each([Input::Key(NamedKey::ArrowDown)]),
            },
        ]
    },
};
