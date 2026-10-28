//! What the editor's core and its window share: what the core writes for the window to show, and
//! the requests that the window makes of the core
//! (`docs/adr/0156-the-editors-window-and-its-operations.md`).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use slime_world::{Command, PersistentId};

/// The value that the core and the window share, on the thread that runs both.
pub(crate) type Shared = Rc<RefCell<State>>;

/// What the core shows the window, and what the window asks of the core.
#[derive(Debug, Default)]
pub(crate) struct State {
    /// The name of the editor's program, which the window's title says.
    pub(crate) program: &'static str,
    /// Whether the program has a window, which runs the window's operations.
    pub(crate) window: bool,
    /// The level's scene file, once it is open.
    pub(crate) level: Option<PathBuf>,
    /// How many levels were opened in place of the first, each in a world of its own, which the
    /// window's world panel follows.
    pub(crate) opened: u64,
    /// The labels of the steps done, the oldest first, and of the steps undone, the next to do
    /// again first.
    pub(crate) done: Vec<String>,
    pub(crate) undone: Vec<String>,
    /// How many steps the level's file does not hold ([`Editor::unwritten`](crate::Editor::unwritten)).
    pub(crate) unwritten: usize,
    /// What is selected.
    pub(crate) selection: Vec<PersistentId>,
    /// The levels opened last, the last first, by their whole paths.
    pub(crate) recent: Vec<PathBuf>,
    /// Counted up each time the core changes what the window shows.
    pub(crate) revision: u64,
    /// What the window tells the user, as the core said it: the first run's hint, a request that
    /// failed.
    pub(crate) notices: Vec<Notice>,
    /// The window's operations that the core was asked to run, which the window runs.
    pub(crate) window_operations: VecDeque<&'static str>,
    /// What the window asks of the core, which the core answers at its next update.
    pub(crate) requests: VecDeque<Request>,
}

/// Something that the window tells the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    /// Whether it tells of a failure, which stays until the user closes it.
    pub(crate) failure: bool,
    pub(crate) text: String,
}

/// What a person, through the window, or an agent, through the development channel, asks of the
/// core; one handler answers each ([`Session::handle`](crate::session::Session::handle)).
#[derive(Debug)]
pub(crate) enum Request {
    /// A step of commands given as text, whole or not at all, which selects `selection` after it
    /// if it is given.
    Step {
        label: String,
        commands: Vec<String>,
        selection: Option<Vec<PersistentId>>,
    },
    /// A step of commands given as values, whole or not at all: what the window's world panel
    /// changes.
    Apply {
        label: String,
        commands: Vec<Command>,
    },
    Undo,
    Redo,
    /// The level, the history and the selection.
    History,
    /// The text of the journal.
    Journal,
    /// What is selected, which is not a step.
    Select(Vec<PersistentId>),
    /// The level written back to its file.
    Write,
    /// The operations, listed.
    Operations,
    /// An operation run by its name, with its arguments as text.
    Run {
        name: String,
        arguments: Vec<String>,
    },
}
