//! The host's API: what the program that owns the event loop and the platform's objects gives uwui,
//! and what it reads back (`docs/adr/0249-the-hosts-api-its-items-and-their-table.md`,
//! `docs/adr/0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md`).
//!
//! A *host*, the engine's `slime-app` or a platform's own, drives an interface through three
//! channels and one callback:
//!
//! - *events*, given one at a time by [`Ui::event`](crate::tree::Ui::event), each with its target
//!   and its time, each answered at once ([`Answer`]);
//! - *questions* that the host asks and that uwui answers from its live state, the edits of the
//!   same step included, as an input method asks them in the middle of a keystroke
//!   ([`Ui::text_around`](crate::tree::Ui::text_around),
//!   [`Ui::range_box`](crate::tree::Ui::range_box),
//!   [`Ui::character_at`](crate::tree::Ui::character_at), [`Ui::covers`](crate::tree::Ui::covers));
//! - *outputs*, which the host reads for each target after each frame and after each event that
//!   the interface took, inside the platform's handler of that event, only what changed since it
//!   last read them ([`Ui::outputs`](crate::tree::Ui::outputs)): the web lets a page write the
//!   clipboard only during the gesture's own event;
//! - the *waker*, which any thread calls to ask the host for a step
//!   ([`Ui::set_waker`](crate::tree::Ui::set_waker)).
//!
//! A *target* is where roots draw and where input comes from: a window, a region of the host's
//! image, a texture. Every event and output names its target. Today every root draws on
//! [`TargetId::MAIN`]: an event for another target is answered [`Answer::NotTaken`] and changes
//! nothing, and another target's outputs are empty.
//!
//! Positions are in the target's physical pixels, from its top left corner, as the platform's
//! pointer gives them; a host whose platform counts in logical pixels multiplies by the scale.
//! Places in a field's text are counted in the unit that the host names ([`TextUnit`]): uwui holds
//! text in UTF-8 and converts.
//!
//! The clipboard is the host's. uwui asks it to write a text and to read one through the outputs,
//! and takes each read's answer as an event, [`Input::ClipboardText`], so that a host whose
//! platform reads it later, the web's or Wayland's, serves it as well as one that reads it at
//! once.

use std::ops::Range;

use slime_input::{ButtonState, CursorShape, Ime, KeyEvent, Modifiers, MouseButton, ScrollDelta};

use crate::Rect;
use crate::time::EventTime;

/// A target of roots, named by the host: a window, a region of its image, a texture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TargetId(pub u32);

impl TargetId {
    /// The target that every root draws on.
    pub const MAIN: Self = Self(0);
}

/// An input event that the host gives ([`Ui::event`](crate::tree::Ui::event)): where it came, when,
/// and what it is.
#[derive(Clone, Copy, Debug)]
pub struct Event<'a> {
    /// The target that it came to.
    pub target: TargetId,
    /// When it came, on the host's clock, and where that time comes from. The interface's time
    /// moves to it, and never back: an event older than the latest time given moves nothing.
    pub time: EventTime,
    /// What it is.
    pub input: Input<'a>,
}

impl<'a> Event<'a> {
    /// `input`, come to `target` at `time`.
    pub const fn new(target: TargetId, time: EventTime, input: Input<'a>) -> Self {
        Self {
            target,
            time,
            input,
        }
    }
}

/// What an input event is.
///
/// A host gives the pointer's moves gathered: the moves of a frame as one, the last, given before
/// any other event that follows them and before the frame, as the web's and Android's coalesced
/// events are; each move given makes uwui find what lies under the pointer.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Input<'a> {
    /// The pointer moved to `x` and `y`: the button or the field under it is hovered, the one
    /// before is no longer, an item of a menu that it comes over takes the focus, and what holds
    /// the pointer follows it, a field selecting or dragging its number, a slider, a splitter, a
    /// scroll bar's thumb, a floating panel, a drag. Never taken: a game follows the pointer too.
    PointerMoved {
        /// Across, from the target's left.
        x: f32,
        /// Down, from the target's top.
        y: f32,
    },
    /// The pointer left the target: nothing is hovered any more. Never taken.
    PointerLeft,
    /// A button of the mouse went down or up. The primary button pressed over a button presses
    /// it, which then holds the pointer: let go over it, it gives an
    /// [`ActionKind::Press`](crate::action::ActionKind::Press); let go elsewhere, nothing. A
    /// button that can hold the focus takes it. Pressed over a field, it gives the field the focus
    /// and places its caret, Shift held extending the selection, and the field holds the pointer,
    /// which selects as it moves; over a field of numbers that does not hold the focus, a move
    /// past a few pixels drags its number instead. The primary button pressed elsewhere takes the
    /// focus away, a field committing, unless a modal root is open. Taken: a press of any button
    /// over what the interface draws, a panel's box or a label's text as well as a button or a
    /// field, and every press while a modal root is open; and the release of a press that it
    /// took.
    PointerButton {
        /// Which button.
        button: MouseButton,
        /// Down or up.
        state: ButtonState,
    },
    /// The wheel scrolled: the box that scrolls or the list under the pointer scrolls, a line of
    /// the wheel some 40 logical pixels. Taken when such a box or list lies under the pointer,
    /// whether or not it had room to scroll, and, as a press, over what the interface draws or
    /// while a modal root is open.
    Wheel(ScrollDelta),
    /// The modifier keys held changed: shortcuts are read with them. Never taken.
    Modifiers(Modifiers),
    /// A key went down or up. A press, or a repeat, reaches first the field that holds the focus,
    /// which takes what it types and edits with; then it gives the command of a shortcut whose
    /// chord it is, in the scopes of the focus, innermost first, then `global`
    /// ([`Ui::set_shortcuts`](crate::tree::Ui::set_shortcuts)), a press alone, its repeats taken
    /// and giving none, so that a chord that toggles does not flicker
    /// (`docs/adr/0154-floating-panels-and-held-keys.md`); else the intent that its key means: Tab
    /// and Shift+Tab next and previous, the arrows a direction, Enter and Space accept, Escape
    /// cancel. A focus that a press of the pointer gave to anything but a field takes none of these
    /// keys, nor the widget's own, the arrows of a slider or a list to choose from, unless it lies
    /// in a layer that closes as the user leaves it or in a modal root, or its widget's layer is
    /// open: they reach the program, as with no focus
    /// (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`). Tab and Shift+Tab, with no
    /// focus that takes the keys, give the focus to the interface only where it lets them
    /// ([`Ui::set_tab_enters`](crate::tree::Ui::set_tab_enters)); F6 and Shift+F6 give it to the
    /// first element of the next or the previous root that holds one, the top root first, then to
    /// nothing (`docs/adr/0153-tab-and-f6-in-a-games-interface.md`). The interface takes what a
    /// field takes, a shortcut's key, the key of an intent while an element holds the focus and
    /// takes the keys or once it gives the focus, F6 while a root holds an element that can take
    /// the focus, and every key while a modal root is open; the intent of a key that it does not
    /// take still reaches its queue. Its releases are left to the game, which pairs them with its
    /// presses, but Alt's, which, pressed and let go alone, gives a bar of menus the focus, as F10
    /// does ([`Kind::MenuBar`](crate::tree::Kind::MenuBar)). A press hides the tooltip that shows,
    /// which Escape does alone then, and the element that it gives the focus shows its tooltip
    /// ([`Element::tooltip`](crate::tree::Element::tooltip)).
    Key(&'a KeyEvent),
    /// An input method turned on or off, composed or committed text: the field that holds the
    /// focus shows what it composes, under a line, in place of its selection, which a composition
    /// deletes as it starts, and takes what it commits as typed text. Taken when a field holds the
    /// focus.
    InputMethod(&'a Ime),
    /// The input method deletes text around the caret: `before` units before the selection, or
    /// before the composition while it composes, and `after` units after it, counted in `unit`.
    /// The selection and the composition stay; a count past the text's end stops there, and one
    /// that ends inside a character deletes up to that character alone, never half of it. One
    /// edit, which typing joins in one undo. Taken when a field holds the focus.
    DeleteAround {
        /// How many units before.
        before: usize,
        /// How many units after.
        after: usize,
        /// The unit counted.
        unit: TextUnit,
    },
    /// The platform's own copy: the web's `copy` event, the Edit menu's of macOS, UIKit's
    /// `copy(_:)`, Android's menu of the selection. Taken when a field holds the focus, which
    /// writes what it selects, if anything, to the outputs' [`Outputs::clipboard_write`], which
    /// the host reads before its handler ends: the web writes the clipboard during the gesture
    /// alone, and only when the page prevents the event's own copy.
    Copy,
    /// The platform's own cut: a [`Input::Copy`] whose field then deletes what it selected, one
    /// edit.
    Cut,
    /// The platform's own paste, carrying the clipboard's text: the web's `paste` event, the Edit
    /// menu's of macOS, UIKit's `paste(_:)`. Taken when a field holds the focus, which takes the
    /// text in place of its selection, a field of one line a line break as a space. While an
    /// input method composes, what it composed is kept as typed, its input ends, the host told
    /// [`InputMethod::On`] again, and the text is pasted after it.
    Paste(&'a str),
    /// The host's answer to a read of the clipboard that uwui asked ([`Outputs::clipboard_reads`]):
    /// its text, or none when the clipboard holds no text, cannot be read, the person refused, or
    /// the host has no clipboard. Taken when uwui awaits that read, which it then awaits no more;
    /// an answer to a read not awaited, given twice or dropped, is not taken and changes nothing.
    /// A field's read is pasted as [`Input::Paste`] is, if the field holds the focus and did not
    /// lose it since it asked; a program's read gives the program
    /// [`ActionKind::ClipboardText`](crate::action::ActionKind::ClipboardText).
    ClipboardText {
        /// The read that it answers.
        read: ReadId,
        /// The clipboard's text.
        text: Option<&'a str>,
    },
}

/// The unit in which a host counts text, as its platform does. uwui counts in UTF-8 bytes and
/// converts at the boundary, which counts from the text's start (`docs/measurements.md`,
/// M6d.5c2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextUnit {
    /// Bytes of UTF-8, as Wayland's text input and winit count.
    #[default]
    Utf8,
    /// Code units of UTF-16, as Android, Apple's systems, Windows and the web count.
    Utf16,
    /// Characters, Unicode's code points, as Android's deletions in code points count.
    CodePoint,
}

impl TextUnit {
    /// How many units `character` takes.
    pub(crate) fn of(self, character: char) -> usize {
        match self {
            Self::Utf8 => character.len_utf8(),
            Self::Utf16 => character.len_utf16(),
            Self::CodePoint => 1,
        }
    }

    /// How many units `text` takes.
    pub(crate) fn count(self, text: &str) -> usize {
        // A byte that starts a character counts one; one that starts a character of four bytes,
        // which UTF-16 writes in two units, one more. No decoding.
        let starts = |byte: &u8| byte & 0xC0 != 0x80;
        match self {
            Self::Utf8 => text.len(),
            Self::Utf16 => text
                .bytes()
                .map(|byte| usize::from(starts(&byte)) + usize::from(byte >= 0xF0))
                .sum(),
            Self::CodePoint => text.bytes().filter(starts).count(),
        }
    }

    /// The byte of `text` that `at` units from its start reach: the start of the character that
    /// they end inside, or its end when `up`; the text's end past it.
    pub(crate) fn byte(self, text: &str, at: usize, up: bool) -> usize {
        if self == Self::Utf8 {
            let mut byte = at.min(text.len());
            while !text.is_char_boundary(byte) {
                if up {
                    byte += 1;
                } else {
                    byte -= 1;
                }
            }
            return byte;
        }
        let mut counted = 0;
        for (start, character) in text.char_indices() {
            if counted >= at {
                return start;
            }
            counted += self.of(character);
            if counted > at {
                return if up {
                    start + character.len_utf8()
                } else {
                    start
                };
            }
        }
        text.len()
    }
}

/// A read of the clipboard that uwui asked its host for ([`Outputs::clipboard_reads`]), named so
/// that its answer finds it ([`Input::ClipboardText`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadId(pub(crate) u64);

/// What the host asks of the text around the caret ([`Ui::text_around`](crate::tree::Ui::text_around)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextQuery {
    /// How many units before the selection, and the composition when the query counts it, at most.
    pub before: usize,
    /// How many units after the selection, and the composition when the query counts it, at most.
    pub after: usize,
    /// The unit counted, of the counts and of the answer's places.
    pub unit: TextUnit,
    /// Whether the text that an input method composes is part of the text, as Android and macOS
    /// count it; Wayland's and winit's surrounding text leave it out.
    pub composition: bool,
}

/// The text around the caret of the field that holds the focus
/// ([`Ui::text_around`](crate::tree::Ui::text_around)): an excerpt, where it starts in the field's
/// whole text, and the selection and the composition within the excerpt, as Wayland and Android
/// count them; each place in the query's unit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextAround {
    /// The text before the selection, up to the count asked, never cutting a character; the
    /// selection and the composition that the query counts, whole, which the host cuts where its
    /// platform bounds the whole, as Wayland's 4,000 bytes do; then the text after them, up to the
    /// count asked.
    pub text: String,
    /// Where `text` starts in the field's whole text.
    pub start: usize,
    /// The selection's side that stays as it grows, in `text`; for a caret, the caret.
    pub anchor: usize,
    /// The selection's side that moves, in `text`; for a caret, the caret.
    pub focus: usize,
    /// The text that the input method composes, in `text`, while it composes and the query counts
    /// it.
    pub composition: Option<Range<usize>>,
}

/// The box of the first line of a range of a field's text
/// ([`Ui::range_box`](crate::tree::Ui::range_box)).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeBox {
    /// The box, in the target's physical pixels; a caret's, one pixel wide, for an empty range.
    pub rect: Rect,
    /// Where the part of the range that the box holds ends, in the unit asked: the range's end
    /// when it lies on one line, else the end of its first line, from which the host asks again.
    pub end: usize,
}

/// What an event answers at once, inside the platform's handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "an event that the interface took is the game's no more"]
#[non_exhaustive]
pub enum Answer {
    /// The interface left it: the program, a game, handles it.
    NotTaken,
    /// The interface took it: the program, a game, leaves it.
    Taken,
    /// The interface took it, and leaves the platform its own handling, which the host does not
    /// prevent: a key of copy, cut or paste in a field, where the platform fires its own events
    /// for them ([`Ui::set_clipboard_events`](crate::tree::Ui::set_clipboard_events)), which the
    /// host then gives as [`Input::Copy`], [`Input::Cut`] and [`Input::Paste`].
    TakenLeftToPlatform,
}

impl Answer {
    /// Whether the interface took the event, left to the platform or not.
    pub const fn taken(self) -> bool {
        matches!(self, Self::Taken | Self::TakenLeftToPlatform)
    }

    /// [`Answer::Taken`] when `taken`, else [`Answer::NotTaken`].
    pub(crate) const fn of(taken: bool) -> Self {
        if taken { Self::Taken } else { Self::NotTaken }
    }
}

/// What the host reads of a target after each frame ([`Ui::outputs`](crate::tree::Ui::outputs)):
/// each field what changed since it last read them, none where nothing did.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Outputs {
    /// The pointer's shape over the target.
    pub cursor: Option<CursorShape>,
    /// The input method's state: [`InputMethod::On`] each time a field's input starts.
    pub input_method: Option<InputMethod>,
    /// The caret's box of the field that takes text, beside which an input method places its
    /// window, in the target's physical pixels, as the field was last laid out: told with each
    /// [`InputMethod::On`], and when it moves.
    pub input_area: Option<Rect>,
    /// The text to put on the clipboard, the last written since the host read: a field's copy or
    /// cut, by its keys or the platform's [`Input::Copy`] and [`Input::Cut`], or the program's
    /// ([`Ui::write_clipboard`](crate::tree::Ui::write_clipboard)). The host writes it before it
    /// answers the reads beside it, so that a read asked after a write reads what it wrote.
    pub clipboard_write: Option<String>,
    /// The reads of the clipboard asked since the host read, oldest first: each answered once, by
    /// [`Input::ClipboardText`].
    pub clipboard_reads: Vec<ReadId>,
}

/// Whether the target takes text through an input method.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InputMethod {
    /// No field holds the focus: the host turns the input method off.
    #[default]
    Off,
    /// A field's input starts: another field took the focus, or the one that held it lost it and
    /// took it again, which ended what it composed. The host ends what the input method composed
    /// and turns it on, off then on where its platform has no other way, as winit has none.
    On,
}

#[cfg(feature = "test-input")]
/// A person's input on [`TargetId::MAIN`], each event given through
/// [`Ui::event`](crate::tree::Ui::event) at the interface's latest time, which it does not move:
/// what a test, a benchmark or a headless run gives an interface as a person would, where the time
/// that an event came does not matter. A test of time gives its events their time through
/// `Ui::event`. Each method answers whether the interface took the event, where an event can be
/// taken. Built with the feature `test-input` alone, which tests and benchmarks turn on, so that a
/// host has one way to give input
/// (`docs/adr/0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md`).
pub trait Person {
    /// The pointer moved to `x` and `y` ([`Input::PointerMoved`]).
    fn pointer_moved(&mut self, x: f32, y: f32);
    /// The pointer left the target ([`Input::PointerLeft`]).
    fn pointer_left(&mut self);
    /// A button of the mouse went down or up ([`Input::PointerButton`]).
    fn pointer_button(&mut self, button: MouseButton, state: ButtonState) -> bool;
    /// The wheel scrolled ([`Input::Wheel`]).
    fn wheel(&mut self, delta: ScrollDelta) -> bool;
    /// The modifier keys held changed ([`Input::Modifiers`]).
    fn modifiers(&mut self, modifiers: Modifiers);
    /// A key went down or up ([`Input::Key`]).
    fn key(&mut self, key: &KeyEvent) -> bool;
    /// An input method turned on or off, composed or committed ([`Input::InputMethod`]).
    fn ime(&mut self, ime: &Ime) -> bool;
    /// Serves the clipboard as a host would, from `board`, a test's clipboard: reads the main
    /// target's outputs, puts on `board` the text written, then answers each read with what
    /// `board` holds. Gives the outputs that it read, which the interface tells only once.
    fn serve_clipboard(&mut self, board: &mut Option<String>) -> Outputs;
}
