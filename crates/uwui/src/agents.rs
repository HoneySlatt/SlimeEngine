//! The answers to the interface's family, from the tree: what an agent reads of an interface, and
//! does in it, as a person's input would
//! (`docs/adr/0143-what-agents-see-of-the-interface.md`,
//! `docs/adr/0259-the-interfaces-family-in-uwui-agent-answered-by-uwui-an-elements-style-by-its-own-request.md`).
//!
//! Compiled by the feature `agents`, it serves nothing by itself: a host carries the family's
//! messages, `uwui_agent::interface`, and gives each request to [`answer`] with the [`Ui`] that
//! answers it. A capture waits for its target's image: the host keeps it, then asks [`shown`] which
//! pixels of that image to cut. A refusal is a [`Fault`] of the code `REFUSED`, saying why.

use uwui_agent::Fault;
use uwui_agent::interface::{
    Action as AgentAction, ElementSummary, MAX_ELEMENTS, Reply, Request, StyleDeclaration,
    StylePlace,
};

use crate::Rect;
use crate::action::{Action, ActionKind, Intent};
use crate::host::TargetId;
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
/// why it refuses it.
///
/// A capture is answered by the pixels that the element covers now: a host that draws again
/// before it captures keeps the request, and asks [`shown`] once the target's image is drawn.
///
/// # Errors
///
/// A [`Fault`] of the code `REFUSED` for a path that names no element, an element that does not
/// show, an action that the element does not take now, as a disabled button's press, an action
/// that needs an element and names none, a navigation intent or a command that names one, an
/// offset that is not finite, and a place in a target of drops outside 0 to 1.
pub fn answer(ui: &mut Ui, request: &Request) -> Result<Answer, Fault> {
    let reply = match request {
        Request::Elements { role, name } => elements(ui, role, name),
        Request::ElementAt { x, y, target } => {
            let target = target.map_or_else(|| ui.active_target(), TargetId);
            // The middle of the pixel, as the pointer would be there.
            let found = ui.meaning_at(target, *x as f32 + 0.5, *y as f32 + 0.5);
            Reply::Elements {
                elements: found
                    .map(|meaning| summary(ui, meaning))
                    .into_iter()
                    .collect(),
                more: 0,
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

/// The elements of `ui` whose role is `role`, whatever its case, and whose names hold `name`,
/// whatever its case; either empty for any.
fn elements(ui: &Ui, role: &str, name: &str) -> Reply {
    let name = name.to_lowercase();
    let mut found = ui.meaning().into_iter().filter(|meaning| {
        (role.is_empty() || meaning.role.name().eq_ignore_ascii_case(role))
            && meaning.name.to_lowercase().contains(&name)
    });
    let elements = found
        .by_ref()
        .take(MAX_ELEMENTS)
        .map(|meaning| summary(ui, meaning))
        .collect();
    Reply::Elements {
        elements,
        more: found.count() as u64,
    }
}

/// What `meaning` says, as the family gives it.
fn summary(ui: &Ui, meaning: Meaning) -> ElementSummary {
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
    ElementSummary {
        path: meaning.path,
        target: target.0,
        role: meaning.role.name().to_owned(),
        name: meaning.name,
        value: meaning.value,
        description: meaning.description,
        placeholder: meaning.placeholder,
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
        ui.frame();
        ui
    }

    fn reply(ui: &mut Ui, request: Request) -> Result<Reply, Fault> {
        match answer(ui, &request)? {
            Answer::Reply(reply) => Ok(reply),
            Answer::Capture(shown) => panic!("a capture: {shown:?}"),
        }
    }

    fn found(ui: &mut Ui, role: &str, name: &str) -> Vec<ElementSummary> {
        let request = Request::Elements {
            role: role.to_owned(),
            name: name.to_owned(),
        };
        let Ok(Reply::Elements { elements, more }) = reply(ui, request) else {
            panic!("no elements");
        };
        assert_eq!(more, 0);
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
            answer(&mut ui, &capture),
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
        ui.frame();
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
        ui.frame();
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
}
