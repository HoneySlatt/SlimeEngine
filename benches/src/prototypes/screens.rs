//! The screens of ADR-0202, each with its scripted interactions, the same in the four models, in
//! the order the bench plays them (`docs/adr/0202-how-m6d1-compares-ways-of-writing-an-application.md`).

use std::time::Instant;

use slime_input::NamedKey;

use super::host::{Find, Input, Interaction};

/// A screen in a model: its name, its style sheet, its program with its data made, and its
/// interactions.
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

/// A document whose line the bench changes from outside the screen, as an edit does: the
/// control's state, the signals' handle, the views' `View`, or the messages' application, which
/// sends `Message::Edit`.
pub(crate) trait Edit {
    fn edit(&mut self, line: usize, text: String);
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

pub(crate) const HELLO: (&str, (&str, &str)) = (
    "\"Hello, World!\"",
    ("hello.style", include_str!("sheets/hello.style")),
);

pub(crate) fn hello<P>(_: Scale) -> Vec<Interaction<P>> {
    vec![
        open(),
        Interaction {
            name: "its button pressed",
            frames: vec![vec![Input::PointerAt(Find::Class("primary")), Input::Click]],
        },
    ]
}

/// The logical pixels that a frame of the screens of reference scrolls.
const SCROLL: f32 = 16.0;

pub(crate) const LIBRARY: (&str, (&str, &str)) = (
    "Slatty's library",
    ("library.style", include_str!("sheets/library.style")),
);

pub(crate) fn library<P>(scale: Scale) -> Vec<Interaction<P>> {
    let frames = if scale == Scale::Whole { 1_200 } else { 60 };
    let mut scroll = vec![vec![
        Input::PointerAt(Find::Class("covers")),
        Input::Wheel(SCROLL),
    ]];
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
}

pub(crate) const SONGS: (&str, (&str, &str)) = (
    "Opium's songs",
    ("songs.style", include_str!("sheets/songs.style")),
);

pub(crate) fn songs<P>(scale: Scale) -> Vec<Interaction<P>> {
    // From the first song to the last, 52 logical pixels a row.
    let frames = if scale == Scale::Whole {
        (8_500.0 * 52.0 / SCROLL) as usize
    } else {
        60
    };
    let mut scroll = vec![vec![
        Input::PointerAt(Find::Class("table")),
        Input::Wheel(SCROLL),
    ]];
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
                Input::PointerAt(Find::Child("header", 0)),
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
}

pub(crate) const DOCUMENT: (&str, (&str, &str)) = (
    "A document of 100,000 lines",
    ("document.style", include_str!("sheets/document.style")),
);

pub(crate) fn document<P: Edit>(_: Scale) -> Vec<Interaction<P>> {
    vec![
        open(),
        Interaction {
            name: "a line that shows changed",
            frames: each([Input::Call(|document: &mut P| {
                document.edit(10, "a line changed".to_owned());
            })]),
        },
        Interaction {
            name: "a line that does not show changed",
            frames: each([Input::Call(|document: &mut P| {
                document.edit(50_000, "a line changed".to_owned());
            })]),
        },
    ]
}

/// A screen of `named` in a model, its program made by `make` and played by `script`.
pub(crate) const fn screen<P>(
    named: (&'static str, (&'static str, &'static str)),
    make: fn(Instant) -> P,
    script: fn(Scale) -> Vec<Interaction<P>>,
) -> Screen<P> {
    Screen {
        name: named.0,
        sheet: named.1,
        make,
        script,
    }
}

pub(crate) const SELECTION: (&str, (&str, &str)) = (
    "A list whose selection moves",
    ("mistake.style", include_str!("sheets/mistake.style")),
);

/// The script of the small cases of a selection that does not show: a list of 1,000 rows,
/// focused, its selection moved by an arrow.
pub(crate) fn selection<P>(_: Scale) -> Vec<Interaction<P>> {
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
}
