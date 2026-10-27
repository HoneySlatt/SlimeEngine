//! What a person or an agent does to an interface: *actions*, which the program reads from a queue
//! and turns into what it changes, a command of the editor's or a game's
//! (`docs/adr/0142-input-and-focus.md`).
//!
//! A person's input becomes actions ([`Ui::pointer_button`](crate::tree::Ui::pointer_button),
//! [`Ui::key`](crate::tree::Ui::key)); an agent sends the same ([`Ui::act`](crate::tree::Ui::act)).
//! The interface handles some itself, the intents that move the focus; the others wait in its
//! queue until the program takes them ([`Ui::take_actions`](crate::tree::Ui::take_actions)).

use slime_color::LinearColor;

use crate::tree::ElementId;

/// An action: what was done, and the element it was done to, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    /// The element: the button pressed, the element that held the focus when a key was pressed;
    /// none when nothing held it.
    pub element: Option<ElementId>,
    /// What was done.
    pub kind: ActionKind,
}

/// What an action does.
#[derive(Clone, Debug, PartialEq)]
pub enum ActionKind {
    /// A button pressed: by the pointer, pressed and let go over it, by the keyboard, accepted
    /// while it held the focus, or by an agent.
    Press,
    /// A navigation intent.
    Intent(Intent),
    /// The command that a shortcut names ([`Shortcuts`](crate::shortcuts::Shortcuts)).
    Command(String),
    /// What a field holds changed, as the user edits it: at each edit, the composition of an
    /// input method once committed, and each step of a drag of a field of numbers or of a slider;
    /// a check box or a button that toggles turned, a list to choose from or a control of segments
    /// given another choice. A field of numbers whose text reads as no number gives none.
    Changed(FieldValue),
    /// What a field holds, given by the user: by Enter in a field of one line, Ctrl+Enter in a
    /// field of several, the focus leaving the field, the release of a drag of a field of numbers,
    /// or an agent's `Accept` while it holds the focus, once the user changed it since it took the
    /// focus or since its last commit; a check box or a button that toggles turned, and a list to
    /// choose from or a control of segments given another choice, after their change; the release
    /// of a slider's drag, and each step of its keys, after its change. A field that the user did not
    /// change gives none, so that a value that the program holds is written only when the user
    /// wrote it. A field of numbers whose text reads as no number gives none, and shows again the
    /// number that the program gave or that it last committed, the later of the two.
    Committed(FieldValue),
    /// A context menu asked for at `at`, in logical pixels of the surface of the action's element's
    /// root: a press of the pointer's second button on what the interface draws, the element under
    /// it the action's; or the key that opens a context menu, or Shift+F10, while an element holds
    /// the focus, at that element's bottom start corner. The program opens its menu there
    /// (`docs/adr/0150-layers.md`).
    ContextMenu {
        /// Where the menu goes.
        at: [f32; 2],
    },
    /// A layer that the program opened closed without the program: a press outside it and outside
    /// the element that it stands beside, or Escape, for one that closes as the user leaves it; a
    /// press on an item of a menu, for the menus that led to it; the element that it stands beside
    /// or over removed, for any layer. The action's element is the layer's root, which no longer
    /// exists (`docs/adr/0150-layers.md`).
    Dismissed,
}

/// What an element that the user gives a value holds: a field's text or number, a check box's or a
/// button that toggles' state, a slider's number, the choice of a list or of segments, a color
/// well's color.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// A field of text's text.
    Text(String),
    /// A field of numbers' number, a slider's.
    Number(f64),
    /// Whether a check box is checked, or a button that toggles on.
    Checked(bool),
    /// The place of a list to choose from's chosen option among its options, or of a control of
    /// segments' chosen segment among its segments.
    Chosen(usize),
    /// A color well's color.
    Color(LinearColor),
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
