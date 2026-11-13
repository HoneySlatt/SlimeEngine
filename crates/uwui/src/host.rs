//! The host's API: what the program that owns the event loop and the platform's objects gives uwui,
//! and what it reads back (`docs/adr/0249-the-hosts-api-its-items-and-their-table.md`,
//! `docs/adr/0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md`).
//!
//! A *host*, the engine's `slime-app` or a platform's own, drives an interface through three
//! channels and one callback:
//!
//! - *events*, given one at a time by [`Ui::event`](crate::tree::Ui::event), each with its target
//!   and its time, each answered at once ([`Answer`]);
//! - *questions* that the host asks and that uwui answers from its live state, which come with the
//!   clipboard and input methods' questions;
//! - *outputs*, which the host reads for each target after each frame, only what changed since it
//!   last read them ([`Ui::outputs`](crate::tree::Ui::outputs));
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
}

impl Answer {
    /// Whether the interface took the event.
    pub const fn taken(self) -> bool {
        matches!(self, Self::Taken)
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
}
