//! The family's waits, `interface/wait`, read into a [`Waits`] and answered: what every host that
//! holds them does alike
//! (`docs/adr/0314-m6d9a2c4-cut-in-two-an-interfaces-wait-gives-the-elements-seen-as-after.md`).
//!
//! A wait's `after` is a handle written as a list's cursor is ([`Listing`]): bound to its process,
//! its interface and its condition but its presence, and holding the numbers of the elements seen
//! ([`MeaningNode::number`](crate::tree::MeaningNode::number)), in order, then a sum of them, so that
//! a handle changed on its way back is refused rather than read as other elements.

use std::collections::BTreeSet;
use std::hash::{DefaultHasher, Hash, Hasher};

use uwui_agent::Fault;
use uwui_agent::interface::{MAX_SEEN, Reply, Request};
use uwui_agent::page::{Listing, Page, Pager};

use super::{Condition, Presence, State, Waited, Waits, summary};
use crate::host::TargetId;
use crate::tree::{ElementId, Meaning, Role, Ui};

/// What a host does with a wait of the family.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Waiting {
    /// It answers with this reply, the wait met at once.
    Reply(Reply),
    /// It holds the request, until [`Waits::check`] gives this wait met, which [`met`] answers, or
    /// its tool cancels it, which [`Waits::cancel`] lets go.
    Held(super::WaitId),
}

/// Reads `request`, an `interface/wait`, into `waits`, on `ui`: answers it when its condition
/// holds now, its reply's elements on a page bounded by `page`, narrowed by what it asks, and by
/// `time_up` ([`Pager`]); else holds it. A host that bounds its waits counts them first, or
/// cancels at once one held past its bound ([`Waits::wait`]).
///
/// # Errors
///
/// A [`Fault`] of the code `BAD_PARAMS` for a request that is no wait, a role or a state that the
/// family does not name, `GenericContainer`, a state both held and lacking, a target that the
/// interface does not have (not the main one, given no size and stood on by no root), an `after`
/// of another condition or that does not read, and more than [`MAX_SEEN`] elements to see;
/// `STALE` for an `after` of another process or interface; as [`Waits::wait`] for another
/// interface's waits or a full interface.
pub fn wait(
    ui: &mut Ui,
    waits: &mut Waits,
    request: &Request,
    page: Page,
    time_up: impl FnMut() -> bool,
) -> Result<Waiting, Fault> {
    let asked = Asked::read(request)?;
    if !has_target(ui, asked.target) {
        return Err(bad(format!(
            "the interface has no target {}: no root stands on it and its host gave it no size; \
             its window's is 0",
            asked.target.0
        )));
    }
    let after = asked.seen(ui, page.program)?;
    match waits.wait(ui, asked.target, asked.condition.clone(), after.as_deref())? {
        Waited::Met { elements, seen } => {
            answered(ui, &asked, &elements, &seen, page, time_up).map(Waiting::Reply)
        }
        Waited::Held(wait) => Ok(Waiting::Held(wait)),
    }
}

/// The reply to `request`, the wait that [`Waits::check`] gave met, `met`: its elements not seen,
/// on a page bounded by `page`, narrowed by what it asks, and by `time_up`, and its `after`.
///
/// # Errors
///
/// A [`Fault`] of the code `BAD_PARAMS` for a request that is no wait, or when more than
/// [`MAX_SEEN`] elements would be seen.
pub fn met(
    ui: &Ui,
    request: &Request,
    met: &super::Met,
    page: Page,
    time_up: impl FnMut() -> bool,
) -> Result<Reply, Fault> {
    let asked = Asked::read(request)?;
    answered(ui, &asked, &met.elements, &met.seen, page, time_up)
}

/// A wait as its request asks it.
struct Asked<'a> {
    target: TargetId,
    condition: Condition,
    /// What its handle is bound to: its condition but its presence, written once.
    filter: String,
    after: Option<&'a str>,
    limit: Option<u64>,
    bytes: Option<u64>,
}

impl<'a> Asked<'a> {
    /// The wait that `request` asks.
    fn read(request: &'a Request) -> Result<Self, Fault> {
        let Request::Wait {
            target,
            role,
            name,
            holding,
            lacking,
            absent,
            after,
            limit,
            bytes,
        } = request
        else {
            return Err(bad("the request is no wait"));
        };
        let role = if role.is_empty() {
            None
        } else {
            Some(role_named(role)?)
        };
        let holding = states_named(holding)?;
        let lacking = states_named(lacking)?;
        if let Some(both) = holding.iter().find(|state| lacking.contains(state)) {
            return Err(bad(format!(
                "`{}` is both held and lacking: no element would match",
                both.name()
            )));
        }
        let names = |states: &BTreeSet<State>| {
            states
                .iter()
                .map(|state| state.name())
                .collect::<Vec<_>>()
                .join(",")
        };
        let filter = format!(
            "{target}\n{}\n{}\n{}\n{}",
            role.map_or("", Role::name),
            name.to_lowercase(),
            names(&holding),
            names(&lacking),
        );
        Ok(Self {
            target: TargetId(*target),
            condition: Condition {
                role,
                name: name.clone(),
                holding: holding.into_iter().collect(),
                lacking: lacking.into_iter().collect(),
                presence: if *absent {
                    Presence::Absent
                } else {
                    Presence::Present
                },
            },
            filter,
            after: after.as_deref(),
            limit: *limit,
            bytes: *bytes,
        })
    }

    fn listing(&self) -> Listing<'_> {
        Listing {
            list: "wait",
            filter: &self.filter,
        }
    }

    /// The numbers of the elements that its `after` saw, none without one.
    fn seen(&self, ui: &Ui, program: u64) -> Result<Option<Vec<u64>>, Fault> {
        let Some(after) = self.after else {
            return Ok(None);
        };
        let not_this = || {
            bad(format!(
                "`{}` is not the `after` of a wait of this target, role, name and states",
                after.chars().take(64).collect::<String>()
            ))
        };
        // A cursor's faults speak of lists: a wait's say what to do with its handle.
        let mut place = self
            .listing()
            .place(after, program, ui.number_in_process())
            .map_err(|fault| {
                if fault.code == Fault::STALE {
                    Fault::new(
                        Fault::STALE,
                        "this `after` was given by another program or interface: wait again \
                         without `after`",
                    )
                } else {
                    not_this()
                }
            })?;
        let summed = place.pop();
        if summed != Some(sum(&place)) {
            return Err(not_this());
        }
        if place.len() > MAX_SEEN {
            return Err(too_many());
        }
        Ok(Some(place))
    }

    /// Its handle, of the elements of these numbers, in order.
    fn handle(&self, ui: &Ui, program: u64, numbers: &BTreeSet<u64>) -> String {
        let mut place: Vec<u64> = numbers.iter().copied().collect();
        place.push(sum(&place));
        self.listing()
            .cursor(program, ui.number_in_process(), &place)
    }
}

/// The reply of a wait met, `asked`: `elements`, those that met it and were not seen, on the page,
/// their numbers counted in its bytes, and its `after`, those of `seen` and those given.
fn answered(
    ui: &Ui,
    asked: &Asked<'_>,
    elements: &[ElementId],
    seen: &[ElementId],
    page: Page,
    time_up: impl FnMut() -> bool,
) -> Result<Reply, Fault> {
    if elements.len() + seen.len() > MAX_SEEN {
        return Err(too_many());
    }
    let number = |id: ElementId| ui.node(id).map(|node| node.number);
    let mut numbers: BTreeSet<u64> = seen.iter().copied().filter_map(number).collect();
    let mut page = page.within(asked.limit, asked.bytes);
    // The handle of what was seen takes its room in the page first.
    page.bytes = page
        .bytes
        .saturating_sub(asked.handle(ui, page.program, &numbers).len());
    let mut pager = Pager::new(page, time_up);
    let mut given = Vec::new();
    for &id in elements {
        if !pager.visit() {
            break;
        }
        let (Some(meaning), Some(number)) = (ui.meaning_with(id, String::new()), number(id)) else {
            continue;
        };
        let path = ui.path_text(id).unwrap_or_default();
        let element = summary(ui, Meaning { path, ..meaning }, page.text_bytes());
        // Counted with its number, which its handle writes.
        if !pager.take(&(&element, number)) {
            break;
        }
        numbers.insert(number);
        given.push(element);
    }
    Ok(Reply::Waited {
        more: given.len() < elements.len(),
        elements: given,
        after: asked.handle(ui, page.program, &numbers),
    })
}

/// The role named `name`, whatever its case, as [`Role::name`] names it.
fn role_named(name: &str) -> Result<Role, Fault> {
    match Role::ALL
        .into_iter()
        .find(|role| role.name().eq_ignore_ascii_case(name))
    {
        Some(Role::GenericContainer) => Err(bad(
            "`GenericContainer` lays others out and never means something: no element would match",
        )),
        Some(role) => Ok(role),
        None => {
            let roles: Vec<&str> = Role::ALL
                .into_iter()
                .filter(|&role| role != Role::GenericContainer)
                .map(Role::name)
                .collect();
            Err(bad(format!(
                "no role `{}`: the roles are {}",
                name.chars().take(64).collect::<String>(),
                roles.join(", ")
            )))
        }
    }
}

/// The states named `names`, whatever their case, as [`State::name`] names them, in the order of
/// [`State::ALL`], so that a filter writes the same states however they were asked.
fn states_named(names: &[String]) -> Result<BTreeSet<State>, Fault> {
    names
        .iter()
        .map(|name| {
            State::ALL
                .into_iter()
                .find(|state| state.name().eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    let states: Vec<&str> = State::ALL.into_iter().map(State::name).collect();
                    bad(format!(
                        "no state `{}`: the states are {}",
                        name.chars().take(64).collect::<String>(),
                        states.join(", ")
                    ))
                })
        })
        .collect()
}

/// Whether `ui` has `target`: the main one, one that its host gave a size, or one that a root
/// stands on.
fn has_target(ui: &Ui, target: TargetId) -> bool {
    target == TargetId::MAIN
        || ui.target_size(target).is_some()
        || ui
            .roots()
            .iter()
            .any(|&root| ui.target_of(root) == Some(target))
}

/// A sum of `numbers`, which a handle ends with.
fn sum(numbers: &[u64]) -> u64 {
    let mut hasher = DefaultHasher::new();
    numbers.hash(&mut hasher);
    hasher.finish()
}

fn too_many() -> Fault {
    bad(format!(
        "more than {MAX_SEEN} elements would be seen: wait on a narrower condition"
    ))
}

fn bad(why: impl Into<String>) -> Fault {
    Fault::new(Fault::BAD_PARAMS, why)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::TargetSize;
    use crate::text::Fonts;

    #[test]
    fn a_handle_of_more_elements_than_its_most_is_refused() {
        let mut ui = Ui::new(Fonts::new());
        ui.set_target_size(
            TargetId::MAIN,
            TargetSize::new([400, 300], 1.0).expect("a size"),
        );
        ui.frame(TargetId::MAIN);
        let request = |after: Option<String>| Request::Wait {
            target: 0,
            role: String::new(),
            name: String::new(),
            holding: Vec::new(),
            lacking: Vec::new(),
            absent: true,
            after,
            limit: None,
            bytes: None,
        };
        let page = Page::new(1, usize::MAX);
        let asked = request(None);
        let asked = Asked::read(&asked).expect("a wait");
        for (count, refused) in [(MAX_SEEN, false), (MAX_SEEN + 1, true)] {
            let numbers: BTreeSet<u64> = (1..=count as u64).collect();
            let handle = asked.handle(&ui, page.program, &numbers);
            let mut waits = Waits::default();
            let answer = wait(&mut ui, &mut waits, &request(Some(handle)), page, || false);
            assert_eq!(
                matches!(&answer, Err(fault) if fault.message.contains("narrower")),
                refused,
                "{count}: {answer:?}"
            );
            waits.clear(Some(&mut ui));
        }
    }
}
