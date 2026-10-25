//! What a person or an agent does to an interface: *actions*, which the program reads from a queue
//! and turns into what it changes, a command of the editor's or a game's
//! (`docs/adr/0142-input-and-focus.md`).
//!
//! A person's input becomes actions ([`Ui::pointer_button`](crate::tree::Ui::pointer_button),
//! [`Ui::key`](crate::tree::Ui::key)); an agent sends the same ([`Ui::act`](crate::tree::Ui::act)).
//! The interface handles some itself, the intents that move the focus; the others wait in its
//! queue until the program takes them ([`Ui::take_actions`](crate::tree::Ui::take_actions)).

use crate::tree::ElementId;

/// An action: what was done, and the element it was done to, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    /// The element: the button pressed, the element that held the focus when a key was pressed;
    /// none when nothing held it.
    pub element: Option<ElementId>,
    /// What was done.
    pub kind: ActionKind,
}

/// What an action does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionKind {
    /// A button pressed: by the pointer, pressed and let go over it, by the keyboard, accepted
    /// while it held the focus, or by an agent.
    Press,
    /// A navigation intent.
    Intent(Intent),
    /// The command that a shortcut names ([`Shortcuts`](crate::shortcuts::Shortcuts)).
    Command(String),
}

/// A navigation intent: what a key, and later a gamepad, means to the interface whatever the
/// program. The interface moves the focus itself for the first six; `Accept` presses the button
/// that holds the focus, and reaches the program as an intent where none does; `Cancel` reaches the
/// program, which closes what it opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    /// The focus to the next element, in the order of the tree: Tab.
    Next,
    /// The focus to the element before: Shift+Tab.
    Previous,
    /// The focus to the nearest element above, on screen.
    Up,
    /// The focus to the nearest element below.
    Down,
    /// The focus to the nearest element on the left.
    Left,
    /// The focus to the nearest element on the right.
    Right,
    /// What holds the focus done: Enter or Space.
    Accept,
    /// What is open closed, or what is under way given up: Escape.
    Cancel,
}
