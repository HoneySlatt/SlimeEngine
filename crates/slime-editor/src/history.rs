//! The editor's history: the steps done, which undo undoes, and the steps undone, which redo does
//! again (`docs/adr/0134-the-editors-core.md`).

use std::collections::VecDeque;
use std::mem;

use slime_world::{Deleted, PersistentId};

/// The bytes of memory that an editor's history takes at most, unless the editor is given another
/// bound ([`Editor::set_history_budget`](crate::Editor::set_history_budget)): 256 MiB. Past it, the
/// steps undone furthest from the present are forgotten first, then the oldest steps done, but the
/// last step done, whatever its size.
pub const HISTORY_BUDGET: usize = 256 << 20;

/// A command that the history holds, which moves the world across a step: as its text, which names
/// entities by their persistent identities, or, for what a deletion took out, as the deletion holds
/// it, which no text holds.
#[derive(Debug)]
pub(crate) enum Held {
    Text(String),
    Restore(Deleted),
}

impl Held {
    /// About the bytes of memory that it takes.
    fn size(&self) -> usize {
        mem::size_of::<Self>()
            + match self {
                Self::Text(text) => text.len(),
                Self::Restore(deleted) => deleted.size(),
            }
    }
}

/// A step of the history: the commands that move the world across it, in the order in which they
/// are applied, and the selections before and after it.
pub(crate) struct Step {
    pub(crate) label: String,
    pub(crate) commands: Vec<Held>,
    pub(crate) before: Vec<PersistentId>,
    pub(crate) after: Vec<PersistentId>,
}

impl Step {
    /// About the bytes of memory that it takes.
    fn size(&self) -> usize {
        let selection = |ids: &[PersistentId]| {
            ids.iter()
                .map(|id| id.parts().iter().map(String::len).sum::<usize>())
                .sum::<usize>()
        };
        mem::size_of::<Self>()
            + self.label.len()
            + self.commands.iter().map(Held::size).sum::<usize>()
            + selection(&self.before)
            + selection(&self.after)
    }
}

/// The steps done, the oldest first, and the steps undone, the last undone last, within a bound
/// of memory.
pub(crate) struct History {
    done: VecDeque<(Step, usize)>,
    undone: VecDeque<(Step, usize)>,
    size: usize,
    budget: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            done: VecDeque::new(),
            undone: VecDeque::new(),
            size: 0,
            budget: HISTORY_BUDGET,
        }
    }
}

impl History {
    /// Adds a step done: a new one forgets the steps undone, one done again does not.
    pub(crate) fn push_done(&mut self, step: Step, new: bool) {
        if new {
            self.size -= self.undone.drain(..).map(|(_, size)| size).sum::<usize>();
        }
        let size = step.size();
        self.size += size;
        self.done.push_back((step, size));
        self.forget();
    }

    /// Adds a step undone.
    pub(crate) fn push_undone(&mut self, step: Step) {
        let size = step.size();
        self.size += size;
        self.undone.push_back((step, size));
        self.forget();
    }

    /// Takes the last step done.
    pub(crate) fn pop_done(&mut self) -> Option<Step> {
        let (step, size) = self.done.pop_back()?;
        self.size -= size;
        Some(step)
    }

    /// Takes the last step undone.
    pub(crate) fn pop_undone(&mut self) -> Option<Step> {
        let (step, size) = self.undone.pop_back()?;
        self.size -= size;
        Some(step)
    }

    /// Whether it holds no step, done or undone.
    pub(crate) fn is_empty(&self) -> bool {
        self.done.is_empty() && self.undone.is_empty()
    }

    /// The labels of the steps done, the oldest first.
    pub(crate) fn done(&self) -> impl ExactSizeIterator<Item = &str> {
        self.done.iter().map(|(step, _)| step.label.as_str())
    }

    /// The labels of the steps undone, the next to be done again first.
    pub(crate) fn undone(&self) -> impl ExactSizeIterator<Item = &str> {
        self.undone
            .iter()
            .rev()
            .map(|(step, _)| step.label.as_str())
    }

    /// Sets the bound, and forgets what goes past it.
    pub(crate) fn set_budget(&mut self, budget: usize) {
        self.budget = budget;
        self.forget();
    }

    /// Forgets, while the history goes past its bound, the steps undone furthest from the present,
    /// then the oldest steps done, but the last one.
    fn forget(&mut self) {
        while self.size > self.budget {
            let forgotten = match self.undone.pop_front() {
                Some(step) => step,
                None if self.done.len() > 1 => self.done.pop_front().expect("more than one step"),
                None => break,
            };
            self.size -= forgotten.1;
        }
    }
}
