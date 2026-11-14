//! The answers to the interface's family, from the tree: what an agent reads of an interface, and
//! does in it, as a person's input would
//! (`docs/adr/0143-what-agents-see-of-the-interface.md`,
//! `docs/adr/0259-the-interfaces-family-in-uwui-agent-answered-by-uwui-an-elements-style-by-its-own-request.md`).
//!
//! Compiled by the feature `agents`, it serves nothing by itself: a host carries the family's
//! messages, `uwui_agent::interface`, and gives each request to [`answer`] with the [`Ui`] that
//! answers it. A capture waits for its target's image: the host keeps it, then asks [`shown`] which
//! pixels of that image to cut. A refusal is a [`Fault`] of the code `REFUSED`, saying why.
//!
//! The elements are read by pages
//! (`docs/adr/0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md`):
//! the tree is walked in its order, its roots in the order of their numbers, which raising a root
//! does not change, and a page's cursor holds where the walk stands, each element from the root
//! down by its id and the place of the next child to visit. A page resumes there; at an element
//! gone, at the place among its siblings where it was.

use uwui_agent::Fault;
use uwui_agent::interface::{
    Action as AgentAction, ElementSummary, Reply, Request, StyleDeclaration, StylePlace,
};
use uwui_agent::page::{Listing, Page, Pager, cut};

use crate::Rect;
use crate::action::{Action, ActionKind, Intent};
use crate::host::TargetId;
use crate::style::Display;
use crate::theme::StateName;
use crate::tree::{ElementAction, ElementId, ElementState, Meaning, Ui};

/// What a host does with a request of the family.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    /// It answers with this reply.
    Reply(Reply),
    /// It answers with the pixels that an element covers in its target's image, which
    /// [`Reply::Image`] carries.
    Capture(Shown),
}

/// The whole pixels of a target's image that an element covers, at least one wide and high where
/// it shows at all: what a host cuts from that image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    /// The target, whose image is cut.
    pub target: TargetId,
    /// The left column, from the image's left.
    pub x: u32,
    /// The top row, from the image's top.
    pub y: u32,
    /// The width, in pixels.
    pub width: u32,
    /// The height, in pixels.
    pub height: u32,
}

/// The answer of `ui` to `request`, which an [`Action`] done changes as a person's input would, or
/// why it refuses it. A list of elements is a page bounded by `page`, the most that the host
/// gives, narrowed by what the request asks, and by `time_up`, which says whether the page's time
/// has passed ([`Pager`]); an element's texts are cut to fit the page.
///
/// A capture is answered by the pixels that the element covers now: a host that draws again
/// before it captures keeps the request, and asks [`shown`] once the target's image is drawn.
///
/// # Errors
///
/// A [`Fault`] of the code `REFUSED` for a path that names no element, an element that does not
/// show, an action that the element does not take now, as a disabled button's press, an action
/// that needs an element and names none, a navigation intent or a command that names one, an
/// offset that is not finite, and a place in a target of drops outside 0 to 1; of the code
/// `STALE` for a cursor of another process or interface, and `BAD_PARAMS` for one that does not
/// read, or of another list or filter.
pub fn answer(
    ui: &mut Ui,
    request: &Request,
    page: Page,
    time_up: impl FnMut() -> bool,
) -> Result<Answer, Fault> {
    let reply = match request {
        Request::Elements {
            role,
            name,
            cursor,
            limit,
            bytes,
        } => elements(
            ui,
            role,
            name,
            cursor.as_deref(),
            page.within(*limit, *bytes),
            time_up,
        )?,
        Request::ElementAt { x, y, target } => {
            let target = target.map_or_else(|| ui.active_target(), TargetId);
            // The middle of the pixel, as the pointer would be there.
            let found = ui.meaning_at(target, *x as f32 + 0.5, *y as f32 + 0.5);
            Reply::Elements {
                elements: found
                    .map(|meaning| summary(ui, meaning, page.text_bytes()))
                    .into_iter()
                    .collect(),
                next: None,
            }
        }
        Request::Act { element, action } => {
            act(ui, element, action).map_err(refused)?;
            Reply::Done
        }
        Request::CaptureElement { element } => return shown(ui, element).map(Answer::Capture),
        Request::Style { element } => Reply::Style {
            places: style(ui, element).map_err(refused)?,
        },
    };
    Ok(Answer::Reply(reply))
}

/// The pixels of its target's image that the element at `path` covers, as its last frame laid it
/// out.
///
/// # Errors
///
/// A [`Fault`] of the code `REFUSED` for a path that names no element, and an element that does
/// not show, as one that a box that scrolls cuts whole.
pub fn shown(ui: &Ui, path: &str) -> Result<Shown, Fault> {
    let id = element(ui, path).map_err(refused)?;
    let target = ui.input_target(id);
    ui.meaning_of(id)
        .and_then(|meaning| meaning.bounds)
        .map(|bounds| covering(target, bounds))
        .ok_or_else(|| refused(format!("the element `{path}` does not show")))
}

/// A page of the elements of `ui` whose role is `role`, whatever its case, and whose names hold
/// `name`, whatever its case, either empty for any, from `cursor`, none for the list's start.
fn elements(
    ui: &Ui,
    role: &str,
    name: &str,
    cursor: Option<&str>,
    page: Page,
    time_up: impl FnMut() -> bool,
) -> Result<Reply, Fault> {
    let (role, name) = (role.to_ascii_lowercase(), name.to_lowercase());
    let filter = format!("{role}\n{name}");
    let listing = Listing {
        list: "elements",
        filter: &filter,
    };
    let maker = ui.number_in_process();
    let mut walk = match cursor {
        None => Walk::start(ui),
        Some(cursor) => {
            Walk::resume(ui, &listing.place(cursor, page.program, maker)?).ok_or_else(|| {
                Fault::new(
                    Fault::BAD_PARAMS,
                    format!("`{cursor}` is not a cursor of this list"),
                )
            })?
        }
    };
    let mut pager = Pager::new(page, time_up);
    let mut elements = Vec::new();
    while pager.visit() {
        let Some(id) = walk.next(ui) else {
            return Ok(Reply::Elements {
                elements,
                next: None,
            });
        };
        let Some(meaning) = ui.meaning_with(id, String::new()) else {
            continue;
        };
        if !((role.is_empty() || meaning.role.name().eq_ignore_ascii_case(&role))
            && meaning.name.to_lowercase().contains(&name))
        {
            continue;
        }
        let path = ui.path_text(id).unwrap_or_default();
        let element = summary(ui, Meaning { path, ..meaning }, page.text_bytes());
        if !pager.take(&element) {
            // Left for the next page.
            walk.hold();
            break;
        }
        elements.push(element);
    }
    Ok(Reply::Elements {
        elements,
        next: walk
            .place(ui)
            .map(|place| listing.cursor(page.program, maker, &place)),
    })
}

/// Where a walk of the tree stands: the root and the elements from it down whose children it
/// visits, each with the place of the next child to visit, and the roots after them.
struct Walk {
    /// The roots by their numbers, in that order.
    roots: Vec<(u64, ElementId)>,
    /// The place in `roots` of the next root to visit.
    next_root: usize,
    /// From the root down, the elements whose children the walk visits, each with the place of
    /// the next of them.
    stack: Vec<(ElementId, usize)>,
    /// Whether the element on top of `stack` was held back ([`Walk::hold`]): [`Walk::next`] gives
    /// it again.
    held: bool,
}

/// What a cursor writes as the place of the next child of an element held back, which the next page
/// gives first; and as both parts of the id of no element.
const HELD: u64 = u64::MAX;

/// What a cursor writes of each element from the root down: its place and its generation, the
/// place of its next child, and the place and generation of its next sibling.
const FRAME: usize = 5;

impl Walk {
    /// A walk from the first root.
    fn start(ui: &Ui) -> Self {
        let mut roots: Vec<(u64, ElementId)> = ui
            .roots()
            .iter()
            .filter_map(|&root| Some((ui.node(root)?.number, root)))
            .collect();
        roots.sort_unstable_by_key(|&(number, _)| number);
        Self {
            roots,
            next_root: 0,
            stack: Vec::new(),
            held: false,
        }
    }

    /// The walk where `place` left it ([`Walk::place`]), in `ui` as it is now; none for a place
    /// that does not read. Each element of the place still there resumes after its place among its
    /// siblings now, so that siblings added or removed before it shift nothing; one gone, or moved
    /// to another parent, resumes at its next sibling of then, where that sibling stands now, or
    /// at its own place of then; one hidden since is passed.
    fn resume(ui: &Ui, place: &[u64]) -> Option<Self> {
        let mut walk = Self::start(ui);
        let (&root, frames) = place.split_first()?;
        if !frames.len().is_multiple_of(FRAME) {
            return None;
        }
        // The first root not before the root of the place.
        walk.next_root = walk.roots.partition_point(|&(number, _)| number < root);
        let id = |index: u64, generation: u64| {
            Some(ElementId::from_parts((
                u32::try_from(index).ok()?,
                u32::try_from(generation).ok()?,
            )))
        };
        let place = |next: u64| usize::try_from(next).unwrap_or(usize::MAX);
        let mut frames = frames.as_chunks::<FRAME>().0.iter().peekable();
        let Some(&[index, generation, next, _, _]) = frames.next() else {
            return Some(walk);
        };
        match walk.roots.get(walk.next_root) {
            // The root of the place, still there: its walk resumes.
            Some(&(number, root_id))
                if number == root && Some(root_id) == id(index, generation) =>
            {
                walk.next_root += 1;
                if !displayed(ui, root_id) {
                    return Some(walk);
                }
                walk.stack.push((root_id, place(next)));
            }
            // It is gone: the walk goes on with the next root.
            _ => return Some(walk),
        }
        while let Some(&[index, generation, next, sibling, sibling_generation]) = frames.next() {
            let element = id(index, generation)?;
            let &mut (parent, ref mut parent_next) = walk.stack.last_mut()?;
            let at = |child: ElementId| {
                ui.node(parent)
                    .and_then(|node| node.children.iter().position(|&each| each == child))
            };
            let Some(now_at) = at(element) else {
                // Gone, or moved to another parent: its next sibling of then comes next, where it
                // stands now, or what took its place.
                *parent_next = id(sibling, sibling_generation)
                    .and_then(at)
                    .unwrap_or_else(|| parent_next.saturating_sub(1));
                break;
            };
            if next == HELD && frames.peek().is_none() {
                // Held back: the next page gives it first.
                *parent_next = now_at;
                break;
            }
            *parent_next = now_at + 1;
            if !displayed(ui, element) {
                // Hidden since: what it holds is not walked.
                break;
            }
            walk.stack.push((element, place(next)));
        }
        Some(walk)
    }

    /// The next element of the walk that is laid out, in the order of the tree; none at its end.
    /// What an element of no display holds is not walked.
    fn next(&mut self, ui: &Ui) -> Option<ElementId> {
        loop {
            let element = match self.stack.last_mut() {
                None => {
                    let &(_, root) = self.roots.get(self.next_root)?;
                    self.next_root += 1;
                    root
                }
                Some((parent, next)) => {
                    let child = ui
                        .node(*parent)
                        .and_then(|node| node.children.get(*next).copied());
                    let Some(child) = child else {
                        self.stack.pop();
                        continue;
                    };
                    *next += 1;
                    child
                }
            };
            if displayed(ui, element) {
                self.stack.push((element, 0));
                return Some(element);
            }
        }
    }

    /// Holds back the last element that [`Walk::next`] gave, which the page leaves: the cursor
    /// names it, by its id, for the next page to give first.
    fn hold(&mut self) {
        self.held = true;
    }

    /// Where the walk stands, for a cursor: the number of its root, then each element from the
    /// root down, its place, its generation, the place of its next child, [`HELD`] for one held
    /// back, and the id of its next sibling, [`HELD`] twice for none; none at its end.
    fn place(&self, ui: &Ui) -> Option<Vec<u64>> {
        if self.stack.is_empty() {
            let &(number, _) = self.roots.get(self.next_root)?;
            return Some(vec![number]);
        }
        let (root, _) = self.stack[0];
        let mut place = vec![ui.node(root)?.number];
        let top = self.stack.len() - 1;
        for (at, &(id, next)) in self.stack.iter().enumerate() {
            let (index, generation) = id.parts();
            let next = if self.held && at == top {
                HELD
            } else {
                next as u64
            };
            // The parent's next child is this element's next sibling: the walk passed this one.
            let sibling = at
                .checked_sub(1)
                .and_then(|parent| {
                    let (parent, parent_next) = self.stack[parent];
                    ui.node(parent)?.children.get(parent_next).copied()
                })
                .map_or([HELD; 2], |sibling| {
                    let (index, generation) = sibling.parts();
                    [u64::from(index), u64::from(generation)]
                });
            place.extend([u64::from(index), u64::from(generation), next]);
            place.extend(sibling);
        }
        Some(place)
    }
}

/// Whether the element is laid out and drawn: not of no display.
fn displayed(ui: &Ui, id: ElementId) -> bool {
    ui.node(id)
        .is_some_and(|node| node.style.layout.display != Display::None)
}

/// What `meaning` says, as the family gives it, each of its texts cut to `text_bytes` of JSON.
fn summary(ui: &Ui, meaning: Meaning, text_bytes: usize) -> ElementSummary {
    // Each state named, so that one more does not compile until it is.
    let ElementState {
        hovered,
        pressed,
        disabled,
        focused,
        // Whether the focus shows is the look's, not what the element means.
        focus_visible: _,
        checked,
        expanded,
        selected,
    } = meaning.state;
    let states = [
        (hovered, "hovered"),
        (pressed, "pressed"),
        (focused, "focused"),
        (disabled, "disabled"),
        (checked, "checked"),
        (expanded, "expanded"),
        (selected, "selected"),
        (meaning.popup, "has_popup"),
    ];
    let target = ui.input_target(meaning.id);
    let Meaning {
        mut path,
        mut name,
        mut value,
        mut description,
        mut placeholder,
        ..
    } = meaning;
    let mut cut_fields = Vec::new();
    let texts = [
        ("path", Some(&mut path)),
        ("name", Some(&mut name)),
        ("value", value.as_mut()),
        ("description", description.as_mut()),
        ("placeholder", placeholder.as_mut()),
    ];
    for (field, text) in texts {
        if let Some(text) = text
            && cut(text, text_bytes)
        {
            cut_fields.push(field.to_owned());
        }
    }
    ElementSummary {
        path,
        target: target.0,
        role: meaning.role.name().to_owned(),
        name,
        value,
        description,
        placeholder,
        states: states
            .into_iter()
            .filter(|&(holds, _)| holds)
            .map(|(_, state)| state.to_owned())
            .collect(),
        actions: meaning
            .actions
            .iter()
            .map(|action| action.name().to_owned())
            .collect(),
        bounds: meaning.bounds.map(|bounds| {
            let part = covering(target, bounds);
            [part.x, part.y, part.width, part.height]
        }),
        cut: cut_fields,
    }
}

/// The whole pixels of `target` that `rect`, in pixels of the target, covers.
fn covering(target: TargetId, rect: Rect) -> Shown {
    let (left, top) = (rect.x.max(0.0).floor(), rect.y.max(0.0).floor());
    let right = (rect.x + rect.width).ceil().max(left);
    let bottom = (rect.y + rect.height).ceil().max(top);
    Shown {
        target,
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    }
}

/// The element at `path`, or why none is.
fn element(ui: &Ui, path: &str) -> Result<ElementId, String> {
    ui.element_by_path(path)
        .ok_or_else(|| format!("no element of the interface at `{path}`"))
}

/// The places of the calls that wrote the own style of the element at `path`, each with what it
/// wrote, in the order of the calls.
fn style(ui: &Ui, path: &str) -> Result<Vec<StylePlace>, String> {
    let id = element(ui, path)?;
    let own = &ui
        .node(id)
        .expect("an element found by its path")
        .element
        .style;
    let mut places: Vec<StylePlace> = own
        .places()
        .into_iter()
        .map(|place| StylePlace {
            file: place.file().to_owned(),
            line: place.line(),
            column: place.column(),
            declarations: Vec::new(),
        })
        .collect();
    for written in own.written() {
        let at = places
            .iter()
            .position(|place| {
                place.file == written.place.file()
                    && place.line == written.place.line()
                    && place.column == written.place.column()
            })
            .expect("every declaration's place is one of the style's places");
        places[at].declarations.push(StyleDeclaration {
            field: written.field,
            state: written.state.map(state_name),
            token: written.token.map(str::to_owned),
        });
    }
    Ok(places)
}

/// `state` as a style sheet names it: `Hovered`.
fn state_name(state: StateName) -> String {
    StateName::IDENTIFIERS
        .iter()
        .find(|&&(_, named)| named == state)
        .map_or("", |&(name, _)| name)
        .to_owned()
}

/// A refusal of the family, saying why.
fn refused(why: impl Into<String>) -> Fault {
    Fault::new(Fault::REFUSED, why)
}

/// Does `action` in `ui`, on the element at `path`, or on none for an empty path, as a person's
/// input would; or says why it cannot.
fn act(ui: &mut Ui, path: &str, action: &AgentAction) -> Result<(), String> {
    let id = match path {
        "" => None,
        path => Some(element(ui, path)?),
    };
    // The element of an action that needs one, which it takes now.
    let taking = |ui: &Ui, wanted: ElementAction| {
        let id = id.ok_or_else(|| format!("`{}` needs an element", wanted.name()))?;
        let meaning = ui.meaning_of(id).ok_or_else(|| {
            format!(
                "the element `{path}` takes nothing: it is hidden, or a box that only lays \
                     others out"
            )
        })?;
        if meaning.actions.contains(&wanted) {
            Ok(id)
        } else {
            let takes: Vec<_> = meaning.actions.iter().map(|action| action.name()).collect();
            Err(format!(
                "the element `{path}` takes no `{}` now; it takes {}",
                wanted.name(),
                if takes.is_empty() {
                    "none".to_owned()
                } else {
                    format!("`{}`", takes.join("`, `"))
                }
            ))
        }
    };
    let kind = match action {
        AgentAction::Press => {
            let id = taking(ui, ElementAction::Press)?;
            ui.act(Action {
                element: Some(id),
                kind: ActionKind::Press,
            });
            return Ok(());
        }
        AgentAction::Focus => {
            let id = taking(ui, ElementAction::Focus)?;
            ui.set_focus(Some(id));
            return Ok(());
        }
        AgentAction::Scroll { x, y } => {
            if !(x.is_finite() && y.is_finite()) {
                return Err(format!("({x}, {y}) is no offset: finite numbers are"));
            }
            let id = taking(ui, ElementAction::Scroll)?;
            ui.scroll_to(id, [*x, *y]);
            return Ok(());
        }
        AgentAction::SetValue { value } => {
            let id = taking(ui, ElementAction::SetValue)?;
            // A field takes what a person could type; a field of numbers, an expression of a
            // finite number; a slider and two panels, a finite number; a list, one of its options.
            if !ui.set_value(id, value) {
                return Err(format!(
                    "the element `{path}` reads `{value}` as nothing that it takes, and keeps its own"
                ));
            }
            return Ok(());
        }
        AgentAction::Drop { onto, at } => {
            if !(0.0..=1.0).contains(at) {
                return Err(format!(
                    "{at} is no place in a target of drops: a number from 0 to 1 is"
                ));
            }
            let id = taking(ui, ElementAction::Drop)?;
            let target = element(ui, onto)?;
            if !ui.drop(id, target, *at) {
                return Err(format!("the element `{onto}` takes no drop"));
            }
            return Ok(());
        }
        AgentAction::Next => ActionKind::Intent(Intent::Next),
        AgentAction::Previous => ActionKind::Intent(Intent::Previous),
        AgentAction::Up => ActionKind::Intent(Intent::Up),
        AgentAction::Down => ActionKind::Intent(Intent::Down),
        AgentAction::Left => ActionKind::Intent(Intent::Left),
        AgentAction::Right => ActionKind::Intent(Intent::Right),
        AgentAction::Accept => ActionKind::Intent(Intent::Accept),
        AgentAction::Cancel => ActionKind::Intent(Intent::Cancel),
        AgentAction::Command { name } => ActionKind::Command(name.clone()),
    };
    if id.is_some() {
        return Err(
            "a navigation intent or a command names no element: it reaches the focus, as a key does"
                .to_owned(),
        );
    }
    // As a key gives them, to the element that holds the active target's focus.
    let element = ui.focus();
    ui.act(Action { element, kind });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{Event, Input};
    use crate::style::{Dimension, FlexDirection, StyleBuilder};
    use crate::text::Fonts;
    use crate::theme::ACCENT;
    use crate::time::{EventTime, Instant};
    use crate::tree::{Element, LayoutDirection, RootSettings};
    use slime_input::{ButtonState, MouseButton};

    /// The settings of a root of 400 × 300 logical pixels at scale 1, on `target`.
    fn settings(target: TargetId) -> RootSettings {
        RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
            target,
        }
    }

    fn sized(s: StyleBuilder) -> StyleBuilder {
        s.width(Dimension::Pixels(100.0))
            .height(Dimension::Pixels(40.0))
            .shrink(0.0)
    }

    /// An interface of a row of three buttons, `open`, `save`, disabled, and `close`, then a box
    /// that scrolls, `notes`, of two buttons, the second hidden below the first; each 100 logical
    /// pixels wide and 40 high, laid out; and a second root, `side`, on target 2, of one button.
    fn panel() -> Ui {
        let mut ui = Ui::new(Fonts::new());
        let root = ui.add_root("main", settings(TargetId::MAIN), |s| s);
        let mut save = None;
        ui.build(root, 0, |children| {
            for name in ["open", "save", "close"] {
                let button = children.add(name, Element::button(name).style(sized), |_| {});
                if name == "save" {
                    save = Some(button);
                }
            }
            let notes = Element::scroll().style(|s| sized(s).direction(FlexDirection::Column));
            children.add("notes", notes, |children| {
                for name in ["top", "below"] {
                    children.add(name, Element::button(name).style(sized), |_| {});
                }
            });
        });
        let side = ui.add_root("side", settings(TargetId(2)), |s| s);
        ui.build(side, 0, |children| {
            children.add("help", Element::button("help").style(sized), |_| {});
        });
        let disabled = ElementState {
            disabled: true,
            ..ElementState::default()
        };
        ui.set_state(save.expect("built"), disabled);
        ui.frame(TargetId::MAIN);
        ui.frame(TargetId(2));
        ui
    }

    /// The number of the process, as a carrier would draw it.
    const PROGRAM: u64 = 0x5eed;

    /// A page of the most that a tool of a long line reads.
    fn page() -> Page {
        Page::new(PROGRAM, usize::MAX)
    }

    fn reply(ui: &mut Ui, request: Request) -> Result<Reply, Fault> {
        match answer(ui, &request, page(), || false)? {
            Answer::Reply(reply) => Ok(reply),
            Answer::Capture(shown) => panic!("a capture: {shown:?}"),
        }
    }

    fn found(ui: &mut Ui, role: &str, name: &str) -> Vec<ElementSummary> {
        let request = Request::Elements {
            role: role.to_owned(),
            name: name.to_owned(),
            cursor: None,
            limit: None,
            bytes: None,
        };
        let Ok(Reply::Elements { elements, next }) = reply(ui, request) else {
            panic!("no elements");
        };
        assert_eq!(next, None, "one page");
        elements
    }

    fn act(ui: &mut Ui, element: &str, action: AgentAction) -> Result<Reply, Fault> {
        let element = element.to_owned();
        reply(ui, Request::Act { element, action })
    }

    fn refusal(outcome: Result<impl std::fmt::Debug, Fault>) -> String {
        let fault = outcome.expect_err("refused");
        assert_eq!(fault.code, Fault::REFUSED, "{fault:?}");
        fault.message
    }

    /// A tool reads the elements of an interface, found by role and name, each with its target,
    /// finds where one shows, and acts on them as a person would, what an action needs checked.
    #[test]
    fn a_tool_reads_an_interface_and_acts_on_it() {
        let mut ui = panel();
        let buttons = found(&mut ui, "button", "");
        let paths: Vec<_> = buttons.iter().map(|button| button.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "main/open",
                "main/save",
                "main/close",
                "main/notes/top",
                "main/notes/below",
                "side/help",
            ]
        );
        let targets: Vec<_> = buttons.iter().map(|button| button.target).collect();
        assert_eq!(targets, [0, 0, 0, 0, 0, 2]);
        assert_eq!(buttons[1].states, ["disabled"]);
        assert!(buttons[1].actions.is_empty());
        assert_eq!(buttons[2].actions, ["press", "focus"]);
        assert_eq!(buttons[2].bounds, Some([200, 0, 100, 40]));
        let open = found(&mut ui, "", "PEN");
        assert_eq!(open.len(), 1, "{open:?}");
        assert_eq!(open[0].path, "main/open");
        let capture = Request::CaptureElement {
            element: "side/help".to_owned(),
        };
        assert_eq!(
            answer(&mut ui, &capture, page(), || false),
            Ok(Answer::Capture(Shown {
                target: TargetId(2),
                x: 0,
                y: 0,
                width: 100,
                height: 40
            }))
        );
        // The box that scrolls cuts its second button whole: it does not show, until scrolled to.
        for (path, problem) in [
            ("main/notes/below", "does not show"),
            ("main/nothing", "no element"),
        ] {
            assert!(refusal(shown(&ui, path)).contains(problem), "{path}");
        }
        let scroll = |x, y| AgentAction::Scroll { x, y };
        for (element, action, problem) in [
            ("main/save", AgentAction::Press, "takes no `press` now"),
            ("main/open", scroll(0.0, 40.0), "takes no `scroll` now"),
            ("main/notes", scroll(f32::NAN, 40.0), "is no offset"),
            ("main/nothing", AgentAction::Press, "no element"),
            ("", AgentAction::Focus, "needs an element"),
            ("main/open", AgentAction::Next, "names no element"),
        ] {
            let why = refusal(act(&mut ui, element, action));
            assert!(why.contains(problem), "{why}");
        }
        assert_eq!(
            act(&mut ui, "main/notes", scroll(0.0, 40.0)),
            Ok(Reply::Done)
        );
        assert_eq!(
            act(&mut ui, "main/open", AgentAction::Press),
            Ok(Reply::Done)
        );
        assert_eq!(
            act(&mut ui, "main/open", AgentAction::Focus),
            Ok(Reply::Done)
        );
        assert_eq!(act(&mut ui, "", AgentAction::Next), Ok(Reply::Done));
        let command = AgentAction::Command {
            name: "undo".to_owned(),
        };
        assert_eq!(act(&mut ui, "", command), Ok(Reply::Done));
        let kinds: Vec<_> = ui
            .take_actions()
            .into_iter()
            .map(|action| action.kind)
            .collect();
        assert_eq!(
            kinds,
            [ActionKind::Press, ActionKind::Command("undo".to_owned())]
        );
        ui.frame(TargetId::MAIN);
        let buttons = found(&mut ui, "Button", "");
        assert_eq!(buttons[2].states, ["focused"], "over the disabled button");
        let below = shown(&ui, "main/notes/below").expect("scrolled to");
        assert_eq!((below.x, below.y, below.height), (300, 0, 40));
        // The fraction of a pixel that a bound covers counts whole.
        let part = covering(TargetId::MAIN, Rect::new(10.5, 0.25, 20.0, 9.5));
        assert_eq!((part.x, part.y, part.width, part.height), (10, 0, 21, 10));
    }

    /// A pixel is read on the target that the request names, or on the active target.
    #[test]
    fn a_pixel_is_read_on_its_target_or_the_active_one() {
        let mut ui = panel();
        let at = |ui: &mut Ui, target| {
            let Ok(Reply::Elements { elements, .. }) = reply(
                ui,
                Request::ElementAt {
                    x: 50,
                    y: 20,
                    target,
                },
            ) else {
                panic!("no elements");
            };
            elements
                .into_iter()
                .map(|element| element.path)
                .collect::<Vec<_>>()
        };
        assert_eq!(at(&mut ui, Some(2)), ["side/help"]);
        assert_eq!(at(&mut ui, Some(0)), ["main/open"]);
        assert_eq!(
            at(&mut ui, None),
            ["main/open"],
            "the main target, active at the start"
        );
        assert!(at(&mut ui, Some(7)).is_empty(), "a target without roots");
        // A button let go on target 2 makes it the active target.
        let release = Input::PointerButton {
            button: MouseButton::Left,
            state: ButtonState::Released,
        };
        let time = EventTime::simulated(Instant::from_nanos(1));
        let _ = ui.event(Event::new(TargetId(2), time, release));
        assert_eq!(at(&mut ui, None), ["side/help"]);
    }

    /// An element's own style is read by the places of the calls that wrote it, each with the
    /// fields, the states and the tokens that it wrote.
    #[test]
    fn an_element_s_style_is_read_by_the_places_that_wrote_it() {
        let mut ui = Ui::new(Fonts::new());
        let root = ui.add_root("main", settings(TargetId::MAIN), |s| s);
        let (first, second) = (line!() + 3, line!() + 5);
        ui.build(root, 0, |children| {
            let button = Element::button("save")
                .style(|s| s.padding(4.0).hover(|s| s.background(ACCENT)))
                .class("primary")
                .style(|s| s.width(Dimension::Pixels(80.0)));
            children.add("save", button, |_| {});
            children.add("plain", Element::button("plain"), |_| {});
        });
        ui.frame(TargetId::MAIN);
        let style = |ui: &mut Ui, element: &str| {
            let element = element.to_owned();
            match reply(ui, Request::Style { element }) {
                Ok(Reply::Style { places }) => places,
                other => panic!("{other:?}"),
            }
        };
        let places = style(&mut ui, "main/save");
        let lines: Vec<_> = places.iter().map(|place| place.line).collect();
        assert_eq!(lines, [first, second]);
        assert!(
            places
                .iter()
                .all(|place| place.file == file!() && place.column > 0)
        );
        let declaration =
            |field: &str, state: Option<&str>, token: Option<&str>| StyleDeclaration {
                field: field.to_owned(),
                state: state.map(str::to_owned),
                token: token.map(str::to_owned),
            };
        assert_eq!(
            places[0].declarations,
            [
                declaration("layout.padding.start", None, None),
                declaration("layout.padding.end", None, None),
                declaration("layout.padding.top", None, None),
                declaration("layout.padding.bottom", None, None),
                declaration("paint.background", Some("Hovered"), Some("accent")),
            ]
        );
        assert_eq!(
            places[1].declarations,
            [declaration("layout.size[0]", None, None)]
        );
        assert!(style(&mut ui, "main/plain").is_empty(), "no own style");
        let missing = reply(
            &mut ui,
            Request::Style {
                element: "main/nothing".to_owned(),
            },
        );
        assert!(refusal(missing).contains("no element"));
    }

    /// A page of the elements of `ui` of `role` whose names hold `name`, from `cursor`, bounded by
    /// `page` and `time_up`: its elements and its `next`.
    fn page_of(
        ui: &mut Ui,
        (role, name): (&str, &str),
        cursor: Option<String>,
        page: Page,
        time_up: impl FnMut() -> bool,
    ) -> Result<(Vec<ElementSummary>, Option<String>), Fault> {
        let request = Request::Elements {
            role: role.to_owned(),
            name: name.to_owned(),
            cursor,
            limit: None,
            bytes: None,
        };
        match answer(ui, &request, page, time_up)? {
            Answer::Reply(Reply::Elements { elements, next }) => Ok((elements, next)),
            other => panic!("not elements: {other:?}"),
        }
    }

    /// Every page of the list, its elements' paths and how many pages gave none: read with `page`
    /// and `time_up`, `between` called with the interface after each page.
    fn by_pages(
        ui: &mut Ui,
        filter: (&str, &str),
        page: Page,
        mut time_up: impl FnMut() -> bool,
        mut between: impl FnMut(&mut Ui, usize),
    ) -> (Vec<String>, usize) {
        let (mut paths, mut empty, mut cursor) = (Vec::new(), 0, None);
        for count in 0..10_000 {
            let (elements, next) = page_of(ui, filter, cursor, page, &mut time_up).expect("a page");
            empty += usize::from(elements.is_empty());
            paths.extend(elements.into_iter().map(|element| element.path));
            let Some(next) = next else {
                return (paths, empty);
            };
            between(ui, count);
            cursor = Some(next);
        }
        panic!("a list that does not end");
    }

    /// A page of at most `limit` elements.
    fn limited(limit: u64) -> Page {
        Page::new(PROGRAM, usize::MAX).within(Some(limit), None)
    }

    /// An interface whose root `main` holds ten boxes, `g0` to `g9`, of ten buttons each, `g0 b0`
    /// to `g9 b9`, then a root `side` of one button, `help`, laid out; and the boxes' ids.
    fn tall() -> (Ui, Vec<ElementId>) {
        let mut ui = Ui::new(Fonts::new());
        let root = ui.add_root("main", settings(TargetId::MAIN), |s| s);
        let mut groups = Vec::new();
        ui.build(root, 0, |children| {
            for group in 0..10 {
                groups.push(children.add(
                    format!("g{group}").as_str(),
                    Element::container(),
                    |children| {
                        for button in 0..10 {
                            let name = format!("g{group} b{button}");
                            children.add(
                                format!("b{button}").as_str(),
                                Element::button(name),
                                |_| {},
                            );
                        }
                    },
                ));
            }
        });
        let side = ui.add_root("side", settings(TargetId(2)), |s| s);
        ui.build(side, 0, |children| {
            children.add("help", Element::button("help"), |_| {});
        });
        ui.frame(TargetId::MAIN);
        ui.frame(TargetId(2));
        (ui, groups)
    }

    /// A list read by pages, small or cut short by their time, is the list read in one: each
    /// element once, in its order; a rare match comes after pages that give none and go on.
    #[test]
    fn a_list_read_by_pages_is_the_list_read_whole() {
        let (mut ui, _) = tall();
        let whole: Vec<String> = found(&mut ui, "", "").into_iter().map(|e| e.path).collect();
        assert_eq!(whole.len(), 101, "{whole:?}");
        assert_eq!(whole[0], "main/g0/b0");
        assert_eq!(whole[100], "side/help");
        let (paged, _) = by_pages(&mut ui, ("", ""), limited(7), || false, |_, _| {});
        assert_eq!(paged, whole);
        let (paged, _) = by_pages(&mut ui, ("", ""), page(), || true, |_, _| {});
        assert_eq!(paged, whole, "pages cut short by their time");
        let small = Page::new(PROGRAM, usize::MAX).within(None, Some(400));
        let (paged, _) = by_pages(&mut ui, ("", ""), small, || false, |_, _| {});
        assert_eq!(paged, whole, "pages filled by their bytes");
        let (rare, empty) = by_pages(&mut ui, ("button", "G9 B9"), page(), || true, |_, _| {});
        assert_eq!(rare, ["main/g0/b0".replace("g0/b0", "g9/b9")]);
        assert!(empty >= 1, "a page that found none still gives the rest");
    }

    /// An element where a page ended, removed before the next, leaves the list going on: what was
    /// there throughout is listed once.
    #[test]
    fn an_element_gone_between_pages_leaves_the_list_going_on() {
        let (mut ui, groups) = tall();
        let removed = std::cell::Cell::new(false);
        let (paths, _) = by_pages(
            &mut ui,
            ("button", ""),
            limited(5),
            || false,
            |ui, _| {
                if !removed.replace(true) {
                    // The fifth button of `g0` ended the first page.
                    ui.build(groups[0], 1, |children| {
                        for button in (0..10).filter(|&button| button != 4) {
                            let name = format!("g0 b{button}");
                            children.add(
                                format!("b{button}").as_str(),
                                Element::button(name),
                                |_| {},
                            );
                        }
                    });
                }
            },
        );
        let mut expected: Vec<String> = (0..10)
            .flat_map(|group| (0..10).map(move |button| format!("main/g{group}/b{button}")))
            .collect();
        expected.push("side/help".to_owned());
        assert_eq!(paths.len(), 101);
        assert_eq!(paths[4], "main/g0/b4", "listed before it went");
        let mut rest = paths.clone();
        rest.remove(4);
        expected.remove(4);
        assert_eq!(rest, expected, "the rest once each");
    }

    /// Builds `main` of [`tall`] again, at `revision`: first a box of one button for each of
    /// `before`, then the ten boxes of ten buttons.
    fn tall_again(ui: &mut Ui, revision: u64, before: &[String]) {
        let root = ui.roots()[0];
        ui.build(root, revision, |children| {
            for key in before {
                children.add(key.as_str(), Element::container(), |children| {
                    children.add("new", Element::button(format!("{key} new")), |_| {});
                });
            }
            for group in 0..10 {
                children.add(
                    format!("g{group}").as_str(),
                    Element::container(),
                    |children| {
                        for button in 0..10 {
                            let name = format!("g{group} b{button}");
                            children.add(
                                format!("b{button}").as_str(),
                                Element::button(name),
                                |_| {},
                            );
                        }
                    },
                );
            }
        });
        ui.frame(TargetId::MAIN);
    }

    /// The buttons of [`tall`] that were there from the first page to the last, each listed once,
    /// in their order, among `paths`.
    fn each_once(paths: &[String]) {
        let mut kept: Vec<&String> = paths
            .iter()
            .filter(|path| !path.ends_with("/new"))
            .collect();
        let mut expected: Vec<String> = (0..10)
            .flat_map(|group| (0..10).map(move |button| format!("main/g{group}/b{button}")))
            .collect();
        expected.push("side/help".to_owned());
        assert_eq!(kept.len(), expected.len(), "{paths:?}");
        kept.dedup();
        assert_eq!(
            kept,
            expected.iter().collect::<Vec<_>>(),
            "each once, in order"
        );
    }

    /// Siblings added before where a page ended, of its element or of one above it, shift nothing:
    /// what was there throughout is listed once, and a list that grows at its top at each page
    /// ends.
    #[test]
    fn elements_added_before_where_a_page_ended_shift_nothing() {
        // A box added first in `main` after the first page, inside `g0`.
        let (mut ui, _) = tall();
        let (paths, _) = by_pages(
            &mut ui,
            ("button", ""),
            limited(5),
            || false,
            |ui, page| {
                if page == 0 {
                    tall_again(ui, 1, &["first".to_owned()]);
                }
            },
        );
        each_once(&paths);
        // A button added first in `g0`, the box of the page's last element.
        let (mut ui, groups) = tall();
        let (paths, _) = by_pages(
            &mut ui,
            ("button", ""),
            limited(5),
            || false,
            |ui, page| {
                if page == 0 {
                    ui.build(groups[0], 1, |children| {
                        children.add("new", Element::button("g0 new"), |_| {});
                        for button in 0..10 {
                            let name = format!("g0 b{button}");
                            children.add(
                                format!("b{button}").as_str(),
                                Element::button(name),
                                |_| {},
                            );
                        }
                    });
                    ui.frame(TargetId::MAIN);
                }
            },
        );
        each_once(&paths);
        // A box added first in `main` at every page, as a list shows its newest row on top, the
        // pages filled by their bytes so that an element is held back for the next page.
        let (mut ui, _) = tall();
        let small = Page::new(PROGRAM, usize::MAX).within(None, Some(400));
        let mut added = Vec::new();
        let (paths, _) = by_pages(
            &mut ui,
            ("button", ""),
            small,
            || false,
            |ui, page| {
                added.insert(0, format!("new{page}"));
                tall_again(ui, page as u64 + 1, &added);
            },
        );
        each_once(&paths);
    }

    /// Boxes removed above where a page ended, the page's own among them: the list goes on at the
    /// next box of then, and lists all that follows.
    #[test]
    fn elements_removed_above_where_a_page_ended_leave_the_rest_listed() {
        let (mut ui, _) = tall();
        let (paths, _) = by_pages(
            &mut ui,
            ("button", ""),
            limited(55),
            || false,
            |ui, page| {
                if page == 0 {
                    let root = ui.roots()[0];
                    ui.build(root, 1, |children| {
                        for group in 6..10 {
                            children.add(
                                format!("g{group}").as_str(),
                                Element::container(),
                                |children| {
                                    for button in 0..10 {
                                        let name = format!("g{group} b{button}");
                                        children.add(
                                            format!("b{button}").as_str(),
                                            Element::button(name),
                                            |_| {},
                                        );
                                    }
                                },
                            );
                        }
                    });
                    ui.frame(TargetId::MAIN);
                }
            },
        );
        let mut expected: Vec<String> =
            (0..6)
                .flat_map(|group| (0..10).map(move |button| format!("main/g{group}/b{button}")))
                .take(55)
                .chain((6..10).flat_map(|group| {
                    (0..10).map(move |button| format!("main/g{group}/b{button}"))
                }))
                .collect();
        expected.push("side/help".to_owned());
        assert_eq!(paths, expected);
    }

    /// A section folded between pages: nothing of what it holds is listed after.
    #[test]
    fn a_section_folded_between_pages_lists_nothing_of_it() {
        let mut ui = Ui::new(Fonts::new());
        let root = ui.add_root("main", settings(TargetId::MAIN), |s| s);
        let mut section = None;
        ui.build(root, 0, |children| {
            section =
                Some(
                    children.add("section", Element::section("Section", true), |children| {
                        children.add("inner", Element::container(), |children| {
                            for item in 0..6 {
                                let name = format!("x{item}");
                                children.add(name.as_str(), Element::button(name.clone()), |_| {});
                            }
                        });
                    }),
                );
            children.add("after", Element::button("after"), |_| {});
        });
        ui.frame(TargetId::MAIN);
        let section = section.expect("built");
        let (paths, _) = by_pages(
            &mut ui,
            ("button", "x"),
            limited(2),
            || false,
            |ui, page| {
                if page == 0 {
                    ui.set_open(section, false);
                    ui.frame(TargetId::MAIN);
                }
            },
        );
        assert_eq!(paths, ["main/section/inner/x0", "main/section/inner/x1"]);
        let (after, _) = by_pages(&mut ui, ("button", ""), page(), || false, |_, _| {});
        assert!(after.iter().all(|path| !path.contains("/x")), "{after:?}");
    }

    /// A root moved below a modal root between pages, so that the roots are drawn in another
    /// order: each root's elements are listed once, as the roots go by their numbers.
    #[test]
    fn roots_reordered_between_pages_are_each_listed_once() {
        let mut ui = Ui::new(Fonts::new());
        let modal = ui.add_root(
            "dialog",
            RootSettings {
                modal: true,
                ..settings(TargetId(2))
            },
            |s| s,
        );
        ui.build(modal, 0, |children| {
            for item in 0..5 {
                let name = format!("m{item}");
                children.add(name.as_str(), Element::button(name.clone()), |_| {});
            }
        });
        let main = ui.add_root("main", settings(TargetId::MAIN), |s| s);
        ui.build(main, 0, |children| {
            for item in 0..5 {
                let name = format!("a{item}");
                children.add(name.as_str(), Element::button(name.clone()), |_| {});
            }
        });
        ui.frame(TargetId::MAIN);
        ui.frame(TargetId(2));
        let (paths, _) = by_pages(
            &mut ui,
            ("button", ""),
            limited(3),
            || false,
            |ui, page| {
                if page == 0 {
                    ui.set_root(main, settings(TargetId(2)));
                    assert_eq!(ui.roots(), [main, modal], "below the modal root");
                }
            },
        );
        let expected: Vec<String> = (0..5)
            .map(|item| format!("dialog/m{item}"))
            .chain((0..5).map(|item| format!("main/a{item}")))
            .collect();
        assert_eq!(paths, expected);
    }

    /// Keys that a path cannot read back end pages, and the list goes on past them.
    #[test]
    fn keys_that_a_path_cannot_read_back_end_pages_and_the_list_goes_on() {
        let mut ui = Ui::new(Fonts::new());
        let root = ui.add_root("main", settings(TargetId::MAIN), |s| s);
        ui.build(root, 0, |children| {
            children.add("a/b", Element::button("slash"), |_| {});
            children.add("#12", Element::button("number"), |_| {});
            children.add(crate::tree::Key::Index(12), Element::button("row"), |_| {});
            children.add("last", Element::button("last"), |_| {});
        });
        ui.frame(TargetId::MAIN);
        let (paths, _) = by_pages(&mut ui, ("button", ""), limited(1), || false, |_, _| {});
        assert_eq!(paths, ["main/a/b", "main/#12", "main/#12", "main/last"]);
    }

    /// An element whose texts pass a page is cut to fit, marked, and the list goes past it, its
    /// pages within the tool's line.
    #[test]
    fn a_huge_text_is_cut_marked_and_passed() {
        let mut ui = Ui::new(Fonts::new());
        let root = ui.add_root("main", settings(TargetId::MAIN), |s| s);
        let huge = "\"".repeat(1 << 20);
        ui.build(root, 0, |children| {
            children.add(
                huge.as_str(),
                Element::button("huge").named(huge.clone()),
                |_| {},
            );
            children.add("after", Element::button("after"), |_| {});
        });
        ui.frame(TargetId::MAIN);
        let line = 64 << 10;
        let page = Page::new(PROGRAM, line);
        let (elements, next) =
            page_of(&mut ui, ("button", ""), None, page, || false).expect("a page");
        let reply = Reply::Elements {
            elements: elements.clone(),
            next,
        };
        assert!(uwui_agent::json_size(&reply) <= line, "within the line");
        let mut all = elements;
        if let Reply::Elements {
            next: Some(next), ..
        } = reply
        {
            all.extend(
                page_of(&mut ui, ("button", ""), Some(next), page, || false)
                    .expect("a page")
                    .0,
            );
        }
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].cut, ["path", "name"]);
        assert!(huge.starts_with(&all[0].name) && !all[0].name.is_empty());
        assert_eq!((all[1].path.as_str(), all[1].cut.len()), ("main/after", 0));
    }

    /// A cursor of another interface or process is stale; one that does not read, or given with
    /// another filter, is a bad parameter.
    #[test]
    fn a_cursor_of_another_interface_is_stale_and_one_garbled_is_refused() {
        let (mut ui, _) = tall();
        let (mut other, _) = tall();
        let (_, next) = page_of(&mut ui, ("", ""), None, limited(3), || false).expect("a page");
        let next = next.expect("more");
        let code = |ui: &mut Ui, filter, cursor: &str, page| {
            page_of(ui, filter, Some(cursor.to_owned()), page, || false)
                .expect_err("refused")
                .code
        };
        assert_eq!(code(&mut other, ("", ""), &next, limited(3)), Fault::STALE);
        let elsewhere = Page::new(PROGRAM + 1, usize::MAX).within(Some(3), None);
        assert_eq!(code(&mut ui, ("", ""), &next, elsewhere), Fault::STALE);
        assert_eq!(
            code(&mut ui, ("", "g"), &next, limited(3)),
            Fault::BAD_PARAMS
        );
        let garbled = format!("{next}.1");
        assert_eq!(
            code(&mut ui, ("", ""), &garbled, limited(3)),
            Fault::BAD_PARAMS
        );
        assert!(page_of(&mut ui, ("", ""), Some(next), limited(3), || false).is_ok());
    }

    /// A tree of ten thousand elements, of a thousand that a page gives.
    fn ten_thousand() -> Ui {
        let mut ui = Ui::new(Fonts::new());
        let root = ui.add_root("main", settings(TargetId::MAIN), |s| s);
        ui.build(root, 0, |children| {
            for group in 0..100 {
                let key = format!("g{group}");
                children.add(key.as_str(), Element::container(), |children| {
                    for button in 0..99 {
                        let name = format!("g{group} b{button}");
                        children.add(format!("b{button}").as_str(), Element::button(name), |_| {});
                    }
                });
            }
        });
        ui.frame(TargetId::MAIN);
        ui
    }

    /// A page of a thousand buttons of a tree of ten thousand elements, its median of 21
    /// (`docs/measurements.md`, M6d.9a1b3b1).
    #[test]
    #[ignore = "a measure, run by hand at rest"]
    fn a_page_of_a_thousand_elements() {
        let mut ui = ten_thousand();
        let request = Request::Elements {
            role: "button".to_owned(),
            name: String::new(),
            cursor: None,
            limit: None,
            bytes: None,
        };
        let mut times: Vec<std::time::Duration> = (0..21)
            .map(|_| {
                let started = std::time::Instant::now();
                let answered = answer(&mut ui, &request, page(), || false).expect("a page");
                let took = started.elapsed();
                let Answer::Reply(Reply::Elements { elements, .. }) = answered else {
                    panic!("elements");
                };
                assert_eq!(elements.len(), 1000);
                took
            })
            .collect();
        times.sort_unstable();
        println!("a page of a thousand elements: median {:?}", times[10]);
    }
}
