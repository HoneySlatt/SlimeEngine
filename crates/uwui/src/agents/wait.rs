//! Waits on a target's elements: a condition on their role, their name and their states, present
//! or absent, kept across frames and checked after each of its target's frames on the elements
//! that changed
//! (`docs/adr/0312-waits-on-elements-kept-by-uwui-a-reader-for-waits-reads-what-a-condition-reads.md`).
//!
//! The waits of one target share one reader of meaning, a reader for waits: it lists every element
//! marked for its meaning, but none whose bounds or path alone changed, and no modal root's whole
//! changes; and every element removed or moved to another target, which the interface tells it
//! alone. Each wait keeps the elements that it matched: an element listed leaves the wait's set
//! when it is gone, hidden or no longer matches.
//!
//! A wait may follow what changed since an answer before, rather than read the interface as it
//! stands: given the numbers of the elements that it gave then, its *seen* elements
//! (`docs/adr/0314-m6d9a2c4-cut-in-two-an-interfaces-wait-gives-the-elements-seen-as-after.md`),
//! a presence waits for an element that it did not see, and an absence waits for none to match once
//! one did.

use std::collections::{HashMap, HashSet};

use uwui_agent::Fault;

use super::State;
use crate::host::TargetId;
use crate::style::Display;
use crate::tree::{ElementId, MeaningReader, Role, Ui, WaitView};

/// Whether a wait waits for an element that its condition matches, or for none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Presence {
    /// Met once an element matches.
    Present,
    /// Met once none does.
    Absent,
}

/// What a wait asks of its target's elements: those drawn that mean something, as
/// `interface/elements` lists them, filtered as it filters them, and by their states.
///
/// A check reads an element's role, name and states, and whether it is drawn and means something.
/// A field that read its actions, bounds or path would need the marks that the readers for waits
/// pass over (ADR-0312).
#[derive(Clone, Debug, PartialEq)]
pub struct Condition {
    /// The role that an element has; none for any.
    pub role: Option<Role>,
    /// A text that its name holds, whatever its case; empty for any.
    pub name: String,
    /// The states that hold of it.
    pub holding: Vec<State>,
    /// The states that do not: `lacking: [Disabled]` waits for an enabled button, which differs
    /// from no disabled button when the button is gone.
    pub lacking: Vec<State>,
    /// Whether it waits for an element that matches, or for none.
    pub presence: Presence,
}

/// A wait held by a [`Waits`], which names no other wait of the same `Waits`; another `Waits`
/// gives the same names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WaitId(u64);

/// What [`Waits::wait`] did with a condition.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Waited {
    /// It holds now.
    Met {
        /// The elements that met it and were not seen, in the order of the tree; none for an
        /// absence.
        elements: Vec<ElementId>,
        /// The elements seen that still match, in the order of the tree; none for an absence.
        seen: Vec<ElementId>,
    },
    /// It is held, under this name, until a check finds it met or it is cancelled.
    Held(WaitId),
}

/// A wait that a check found met, and taken out of the waits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Met {
    /// The wait.
    pub wait: WaitId,
    /// The elements that met it and were not seen, in the order of the tree; none for an absence.
    pub elements: Vec<ElementId>,
    /// The elements seen that still match, in the order of the tree; none for an absence.
    pub seen: Vec<ElementId>,
}

/// What a check of a target's waits found, and what it cost.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaitCheck {
    /// The waits met, in the order that they were made.
    pub met: Vec<Met>,
    /// The elements that it read: each one that its reader listed since the last check, once
    /// each, or each one that a walk of the target reached, when the reader's marks turned whole.
    /// None in a frame where no element of the target was marked for its meaning, removed or
    /// moved away.
    pub visited: usize,
}

/// The waits held on an interface's elements, each on one target: a host keeps them beside its
/// [`Ui`], gives each new condition to [`Waits::wait`], and calls [`Waits::check`] after each
/// frame of a target that holds some.
///
/// While it holds a wait on a target, it holds one reader of meaning of that target, which costs
/// a push for each element marked there; the reader goes with the target's last wait. It bounds
/// nothing itself: the host bounds the waits that it asks.
///
/// It reads one interface while it holds waits: another one's is refused, and its checks and
/// cancels do nothing, until its last wait is met or cancelled, or [`Waits::clear`] lets them all
/// go. A `Waits` dropped while it holds waits leaves their readers in the interface, where each
/// takes one of its [`MEANING_READERS`](crate::tree::MEANING_READERS): clear it first.
#[derive(Debug, Default)]
pub struct Waits {
    /// The interface whose waits it holds ([`Ui::number_in_process`]); none while it holds none.
    interface: Option<u64>,
    /// Each target that holds waits, with its reader.
    targets: Vec<OnTarget>,
    /// The number of the next wait.
    next: u64,
    /// The elements that a check takes from a reader, which trades its list for this one: the
    /// reader keeps a list with room for its next marks, so that its pushes during the frames
    /// allocate only past the most that a check took.
    marked: Vec<ElementId>,
}

/// A target's waits and the reader that they share.
#[derive(Debug)]
struct OnTarget {
    target: TargetId,
    reader: MeaningReader,
    waits: Vec<Wait>,
}

/// A wait held.
#[derive(Debug)]
struct Wait {
    id: WaitId,
    /// Its condition, its name in small letters.
    condition: Condition,
    /// The elements that match it now.
    matched: HashSet<ElementId>,
    /// The elements of `matched` that were seen: those that the answer before gave and that
    /// matched as the wait started, less each one that stopped matching since. A presence is met
    /// by another one.
    seen: HashSet<ElementId>,
    /// Whether an element matched since the answer before, which an absence waits for: from its
    /// start when it reads the interface as it stands, or when the answer before gave any.
    armed: bool,
    /// The numbers of the elements seen, while its first walk waits for its target's frame; none
    /// once walked.
    unwalked: Option<HashSet<u64>>,
}

impl Wait {
    /// A wait of `condition`, whose name is in small letters, given the numbers of the elements
    /// seen, `after`: none to read the interface as it stands.
    fn new(id: WaitId, condition: Condition, after: Option<&[u64]>) -> Self {
        Self {
            id,
            condition,
            matched: HashSet::new(),
            seen: HashSet::new(),
            armed: after.is_none_or(|after| !after.is_empty()),
            unwalked: Some(after.unwrap_or_default().iter().copied().collect()),
        }
    }

    /// Takes the first walk's matches, `matched`: the elements seen are those of them whose
    /// numbers the answer before gave.
    fn walked(&mut self, ui: &Ui, matched: HashSet<ElementId>) {
        let numbers = self.unwalked.take().unwrap_or_default();
        self.seen = matched
            .iter()
            .copied()
            .filter(|&id| {
                ui.node(id)
                    .is_some_and(|node| numbers.contains(&node.number))
            })
            .collect();
        self.matched = matched;
        self.armed |= !self.matched.is_empty();
    }

    /// Whether it is met by what it matched.
    fn met(&self) -> bool {
        match self.condition.presence {
            Presence::Present => self.matched.len() > self.seen.len(),
            Presence::Absent => self.armed && self.matched.is_empty(),
        }
    }

    /// What it gives once met: the elements that met it and were not seen, and those seen, each
    /// in the order of the tree.
    fn answer(&self, ui: &Ui) -> (Vec<ElementId>, Vec<ElementId>) {
        let new: HashSet<ElementId> = self.matched.difference(&self.seen).copied().collect();
        (in_tree_order(ui, &new), in_tree_order(ui, &self.seen))
    }
}

impl Waits {
    /// Waits on `target`'s elements for `condition`: met at once when it holds on the tree as it
    /// stands, which costs a walk of the target's elements, else held until a check finds it met.
    /// `after` names the elements seen, by their numbers ([`MeaningNode::number`]): a presence
    /// waits for another one, an absence for none to match once one did; none to read the
    /// interface as it stands. While `target` holds a change that its frame has not styled, the
    /// wait reads nothing and is held, walked by the first check after that frame, so that an
    /// element just built is not read before its style hides it.
    ///
    /// A host that bounds its waits counts them before it asks, or cancels at once one held past
    /// its bound ([`Waits::cancel`]): a wait held stays held until it is met, cancelled or
    /// cleared.
    ///
    /// [`MeaningNode::number`]: crate::tree::MeaningNode::number
    ///
    /// # Errors
    ///
    /// A [`Fault`] of the code `REFUSED` while it holds waits of another interface, and `BUSY`
    /// when the interface holds [`MEANING_READERS`](crate::tree::MEANING_READERS) readers and no
    /// wait holds one of `target` yet.
    pub fn wait(
        &mut self,
        ui: &mut Ui,
        target: TargetId,
        mut condition: Condition,
        after: Option<&[u64]>,
    ) -> Result<Waited, Fault> {
        if self
            .interface
            .is_some_and(|interface| interface != ui.number_in_process())
        {
            return Err(Fault::new(
                Fault::REFUSED,
                "these waits hold another interface's elements",
            ));
        }
        condition.name = condition.name.to_lowercase();
        let mut wait = Wait::new(WaitId(self.next), condition, after);
        if !ui.holds_change(target) {
            let mut matched = HashSet::new();
            walk(ui, target, |id, view| {
                if holds(&wait.condition, &view, &mut None) {
                    matched.insert(id);
                }
            });
            wait.walked(ui, matched);
            if wait.met() {
                let (elements, seen) = wait.answer(ui);
                return Ok(Waited::Met { elements, seen });
            }
        }
        let at = match self.targets.iter().position(|on| on.target == target) {
            Some(at) => at,
            None => {
                let reader = ui.add_wait_reader(target).ok_or_else(|| {
                    Fault::new(
                        Fault::BUSY,
                        "the interface holds as many readers of meaning as it can",
                    )
                })?;
                self.targets.push(OnTarget {
                    target,
                    reader,
                    waits: Vec::new(),
                });
                self.targets.len() - 1
            }
        };
        self.next += 1;
        self.interface = Some(ui.number_in_process());
        let id = wait.id;
        self.targets[at].waits.push(wait);
        Ok(Waited::Held(id))
    }

    /// Checks `target`'s waits, to be called after its [`Ui::frame`], which styles what changed:
    /// each element that its reader listed since the last check is read once, and the waits met
    /// are taken out and given. A frame where none was listed reads none. Where the reader's marks
    /// passed their bound ([`Ui::take_meaning_changes`]), or a wait was held unwalked, it walks the
    /// target whole. Nothing for a target that holds no wait, for another interface than the one
    /// whose waits it holds, and while the target holds a change that its frame has not styled,
    /// whose marks it keeps for the check after that frame: a host may check after each point
    /// where its interface may be framed.
    ///
    /// A target that runs no more frames, a window closed, has its waits checked no more: the host
    /// cancels them.
    pub fn check(&mut self, ui: &mut Ui, target: TargetId) -> WaitCheck {
        if self.interface != Some(ui.number_in_process()) {
            return WaitCheck::default();
        }
        let Some(at) = self.targets.iter().position(|on| on.target == target) else {
            return WaitCheck::default();
        };
        let Self {
            targets, marked, ..
        } = self;
        let on = &mut targets[at];
        if ui.holds_change(target) {
            return WaitCheck::default();
        }
        // Only this `Waits` lets its reader go: one that its interface no longer knows reads whole.
        let whole = ui
            .take_meaning_marks(on.reader, marked)
            .is_none_or(|taken| taken.whole)
            || on.waits.iter().any(|wait| wait.unwalked.is_some());
        let mut visited = 0;
        if whole {
            let mut matched = vec![HashSet::new(); on.waits.len()];
            visited = walk(ui, target, |id, view| {
                let mut lowered = None;
                for (wait, matched) in on.waits.iter().zip(&mut matched) {
                    if holds(&wait.condition, &view, &mut lowered) {
                        matched.insert(id);
                    }
                }
            });
            for (wait, matched) in on.waits.iter_mut().zip(matched) {
                if wait.unwalked.is_some() {
                    wait.walked(ui, matched);
                } else {
                    wait.seen.retain(|id| matched.contains(id));
                    wait.matched = matched;
                }
            }
        } else {
            for &id in marked.iter() {
                visited += 1;
                let view = (ui.target_of(id) == Some(target) && ui.drawn(id))
                    .then(|| ui.wait_view(id))
                    .flatten();
                let mut lowered = None;
                for wait in &mut on.waits {
                    if view
                        .as_ref()
                        .is_some_and(|view| holds(&wait.condition, view, &mut lowered))
                    {
                        wait.matched.insert(id);
                    } else {
                        wait.matched.remove(&id);
                        wait.seen.remove(&id);
                    }
                }
            }
        }
        let mut met = Vec::new();
        on.waits.retain_mut(|wait| {
            wait.armed |= !wait.matched.is_empty();
            if !wait.met() {
                return true;
            }
            let (elements, seen) = wait.answer(ui);
            met.push(Met {
                wait: wait.id,
                elements,
                seen,
            });
            false
        });
        if on.waits.is_empty() {
            self.forget_target(ui, at);
        }
        WaitCheck { met, visited }
    }

    /// Cancels `wait`, and gives whether it was held: its target's reader goes with its last wait.
    /// Nothing for another interface than the one whose waits it holds.
    pub fn cancel(&mut self, ui: &mut Ui, wait: WaitId) -> bool {
        if self.interface != Some(ui.number_in_process()) {
            return false;
        }
        for at in 0..self.targets.len() {
            let waits = &mut self.targets[at].waits;
            let Some(place) = waits.iter().position(|held| held.id == wait) else {
                continue;
            };
            waits.remove(place);
            if waits.is_empty() {
                self.forget_target(ui, at);
            }
            return true;
        }
        false
    }

    /// Lets every wait go, and gives them, in the order that they were made; it then takes
    /// another interface's waits. Their readers go with them when `ui` is the interface whose
    /// waits it holds; give none for an interface that is gone, whose readers went with it.
    pub fn clear(&mut self, mut ui: Option<&mut Ui>) -> Vec<WaitId> {
        let mut waits = Vec::new();
        for on in std::mem::take(&mut self.targets) {
            // Another interface lets go no reader of this one's.
            if let Some(ui) = ui.as_deref_mut() {
                ui.remove_meaning_reader(on.reader);
            }
            waits.extend(on.waits.iter().map(|wait| wait.id));
        }
        waits.sort_unstable();
        self.interface = None;
        waits
    }

    /// Lets go the target at `at`, which holds no wait, and its reader.
    fn forget_target(&mut self, ui: &mut Ui, at: usize) {
        let on = self.targets.swap_remove(at);
        ui.remove_meaning_reader(on.reader);
        if self.targets.is_empty() {
            self.interface = None;
        }
    }
}

/// Whether the element that `view` describes meets `condition`, whose name is in small letters;
/// `lowered` holds the element's name in small letters once a wait needed it, for the next.
fn holds(condition: &Condition, view: &WaitView<'_>, lowered: &mut Option<String>) -> bool {
    condition.role.is_none_or(|role| role == view.role)
        && condition
            .holding
            .iter()
            .all(|state| state.holds(view.state, view.popup))
        && !condition
            .lacking
            .iter()
            .any(|state| state.holds(view.state, view.popup))
        && (condition.name.is_empty()
            || lowered
                .get_or_insert_with(|| view.name.to_lowercase())
                .contains(&condition.name))
}

/// Walks the elements of `target`'s roots that are drawn and mean something, giving each to
/// `found`, and gives how many elements it reached: an element of no display is reached, what it
/// holds is not.
fn walk(ui: &Ui, target: TargetId, mut found: impl FnMut(ElementId, WaitView<'_>)) -> usize {
    let mut reached = 0;
    let mut stack: Vec<ElementId> = ui
        .roots()
        .iter()
        .rev()
        .copied()
        .filter(|&root| ui.target_of(root) == Some(target))
        .collect();
    while let Some(id) = stack.pop() {
        let Some(node) = ui.node(id) else {
            continue;
        };
        reached += 1;
        if node.style.layout.display == Display::None {
            continue;
        }
        if let Some(view) = ui.wait_view(id) {
            found(id, view);
        }
        stack.extend(node.children.iter().rev());
    }
    reached
}

/// `elements` in the order of the tree, as `interface/elements` lists them: their roots by their
/// numbers, then each element by its place among its siblings. The places of a parent's children
/// are found once, so that many elements of one parent cost its children once.
fn in_tree_order(ui: &Ui, elements: &HashSet<ElementId>) -> Vec<ElementId> {
    let mut places: HashMap<ElementId, u64> = HashMap::new();
    let mut place = |id: ElementId| {
        let mut path = Vec::new();
        let mut at = id;
        while let Some(parent) = ui.node(at).and_then(|node| node.parent) {
            if !places.contains_key(&at) {
                places.extend(ui.children(parent).iter().copied().zip(0..));
            }
            path.push(places.get(&at).copied().unwrap_or(0));
            at = parent;
        }
        path.push(ui.node(at).map_or(0, |root| root.number));
        path.reverse();
        path
    };
    let mut ordered: Vec<(Vec<u64>, ElementId)> =
        elements.iter().map(|&id| (place(id), id)).collect();
    ordered.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
    ordered.into_iter().map(|(_, id)| id).collect()
}
