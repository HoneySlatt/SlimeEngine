//! The waits on elements ([`Waits`]): what they find, and what their checks read.

use super::*;
use crate::host::{Person, TargetSize};
use crate::style::{Dimension, FlexDirection, StyleBuilder};
use crate::text::Fonts;
use crate::tree::{Element, Layer, MEANING_READERS, Place, Role, RootSettings};
use slime_input::{ButtonState, MouseButton};

/// A wait that reads the interface as it stands: no elements seen.
trait Ask {
    fn ask(&mut self, ui: &mut Ui, target: TargetId, condition: Condition)
    -> Result<Waited, Fault>;
}

impl Ask for Waits {
    fn ask(
        &mut self,
        ui: &mut Ui,
        target: TargetId,
        condition: Condition,
    ) -> Result<Waited, Fault> {
        self.wait(ui, target, condition, None)
    }
}

/// The second target, beside [`TargetId::MAIN`].
const OTHER: TargetId = TargetId(2);

/// What [`build`] makes.
#[derive(Clone, Copy)]
struct Shape {
    /// The rows of the list of labels, `row 0` and on.
    rows: usize,
    /// Whether the box `extra`, which holds the button `inner`, is of no display.
    hidden: bool,
    /// Whether `inner` is there.
    inner: bool,
    /// Whether `status` is a button rather than a label, under the same key.
    status_button: bool,
    /// Whether the box `panel` is named, which makes it mean something.
    named_panel: bool,
}

const SHAPE: Shape = Shape {
    rows: 200,
    hidden: false,
    inner: true,
    status_button: false,
    named_panel: false,
};

fn sized(s: StyleBuilder, width: f32, height: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(height))
        .shrink(0.0)
}

/// An interface whose targets, the main one and [`OTHER`], are each 400 × 300 logical pixels.
fn interface() -> Ui {
    let mut ui = Ui::new(Fonts::new());
    for target in [TargetId::MAIN, OTHER] {
        ui.set_target_size(target, TargetSize::new([400, 300], 1.0).expect("a size"));
    }
    ui
}

/// Builds `root`: the buttons `open`, `save` and `close`, a box that scrolls of `shape.rows`
/// labels, each 20 pixels high in a box 100 high, the box `extra` holding `inner`, `status`, and
/// `panel`.
fn build(ui: &mut Ui, root: ElementId, revision: u64, shape: Shape) {
    ui.build(root, revision, |children| {
        for name in ["open", "save", "close"] {
            children.add(
                name,
                Element::button(name).style(|s| sized(s, 60.0, 20.0)),
                |_| {},
            );
        }
        let rows =
            Element::scroll().style(|s| sized(s, 200.0, 100.0).direction(FlexDirection::Column));
        children.add("rows", rows, |children| {
            for row in 0..shape.rows {
                let label = Element::label(format!("row {row}")).style(|s| sized(s, 200.0, 20.0));
                children.add(row, label, |_| {});
            }
        });
        let display = if shape.hidden {
            Display::None
        } else {
            Display::Flex
        };
        let extra = Element::container().style(|s| sized(s, 100.0, 20.0).display(display));
        children.add("extra", extra, |children| {
            if shape.inner {
                children.add(
                    "inner",
                    Element::button("inner").style(|s| sized(s, 60.0, 20.0)),
                    |_| {},
                );
            }
        });
        let status = if shape.status_button {
            Element::button("status")
        } else {
            Element::label("status")
        };
        children.add("status", status.style(|s| sized(s, 60.0, 20.0)), |_| {});
        let mut panel = Element::container().style(|s| sized(s, 20.0, 20.0));
        if shape.named_panel {
            panel = panel.named("panel");
        }
        children.add("panel", panel, |_| {});
    });
}

/// An interface whose main target holds a root `main` built in [`SHAPE`], drawn; and the root.
fn form() -> (Ui, ElementId) {
    let mut ui = interface();
    let root = ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| {
        s.direction(FlexDirection::Column)
    });
    build(&mut ui, root, 0, SHAPE);
    ui.frame(TargetId::MAIN);
    (ui, root)
}

fn condition(role: Option<Role>, name: &str, presence: Presence) -> Condition {
    Condition {
        role,
        name: name.to_owned(),
        holding: Vec::new(),
        lacking: Vec::new(),
        presence,
    }
}

/// The wait that `waited` holds.
fn held(waited: Result<Waited, Fault>) -> WaitId {
    match waited {
        Ok(Waited::Held(wait)) => wait,
        other => panic!("not held: {other:?}"),
    }
}

/// The element at `path`.
fn at(ui: &Ui, path: &str) -> ElementId {
    ui.element_by_path(path).expect("an element there")
}

/// A frame of `target`, then a check of its waits.
fn frame_and_check(ui: &mut Ui, waits: &mut Waits, target: TargetId) -> WaitCheck {
    ui.frame(target);
    waits.check(ui, target)
}

#[test]
fn a_condition_that_holds_is_met_at_once_with_its_elements_in_the_tree_s_order() {
    let (mut ui, _) = form();
    let mut waits = Waits::default();
    let buttons = waits
        .ask(
            &mut ui,
            TargetId::MAIN,
            condition(Some(Role::Button), "", Presence::Present),
        )
        .expect("an answer");
    let paths = ["main/open", "main/save", "main/close", "main/extra/inner"];
    assert_eq!(
        buttons,
        Waited::Met {
            elements: paths.iter().map(|path| at(&ui, path)).collect(),
            seen: Vec::new(),
        }
    );
    // The name is held whatever its case; none on the other target.
    let named = waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "ROW 19", Presence::Present),
    );
    assert_eq!(
        named,
        Ok(Waited::Met {
            elements: vec![
                at(&ui, "main/rows/#19"),
                at(&ui, "main/rows/#190"),
                at(&ui, "main/rows/#191"),
                at(&ui, "main/rows/#192"),
                at(&ui, "main/rows/#193"),
                at(&ui, "main/rows/#194"),
                at(&ui, "main/rows/#195"),
                at(&ui, "main/rows/#196"),
                at(&ui, "main/rows/#197"),
                at(&ui, "main/rows/#198"),
                at(&ui, "main/rows/#199")
            ],
            seen: Vec::new(),
        })
    );
    ui.frame(OTHER);
    let elsewhere = waits.ask(&mut ui, OTHER, condition(None, "", Presence::Absent));
    assert_eq!(
        elsewhere,
        Ok(Waited::Met {
            elements: Vec::new(),
            seen: Vec::new(),
        })
    );
    assert_eq!(ui.readers_held(), 0, "a wait met at once holds no reader");
}

#[test]
fn a_still_frame_reads_no_element_and_a_change_reads_what_changed_alone() {
    let (mut ui, _) = form();
    let mut waits = Waits::default();
    // An absence that keeps every label, and one that keeps one button.
    let labels = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Label), "", Presence::Absent),
    ));
    held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Button), "close", Presence::Absent),
    ));
    assert_eq!(ui.readers_held(), 1, "the target's waits share one reader");
    let still = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(still, WaitCheck::default(), "a still frame reads nothing");
    // One button disabled: that button alone, not the tree nor what the waits keep.
    let disabled = crate::tree::ElementState {
        disabled: true,
        ..Default::default()
    };
    ui.set_state(at(&ui, "main/save"), disabled);
    let changed = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(
        changed,
        WaitCheck {
            met: Vec::new(),
            visited: 1
        }
    );
    // The pointer over a button: that button.
    let open = ui.rect(at(&ui, "main/open")).expect("laid out");
    ui.pointer_moved(open.x + 1.0, open.y + 1.0);
    let hovered = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(hovered.visited, 1);
    assert!(waits.cancel(&mut ui, labels));
}

#[test]
fn a_scroll_and_a_target_resized_read_nothing_for_the_waits() {
    let (mut ui, _) = form();
    let mut waits = Waits::default();
    held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Label), "row", Presence::Absent),
    ));
    ui.scroll_to(at(&ui, "main/rows"), [0.0, 400.0]);
    let scrolled = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(scrolled.visited, 0, "rows that only moved");
    ui.set_target_size(
        TargetId::MAIN,
        TargetSize::new([380, 280], 1.0).expect("a size"),
    );
    let resized = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(resized.visited, 0, "elements whose bounds alone changed");
}

#[test]
fn an_element_removed_ends_a_wait_for_its_absence_reading_what_went() {
    let (mut ui, root) = form();
    let mut waits = Waits::default();
    let inner = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Button), "inner", Presence::Absent),
    ));
    build(
        &mut ui,
        root,
        1,
        Shape {
            inner: false,
            ..SHAPE
        },
    );
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(
        check.met,
        vec![Met {
            wait: inner,
            elements: Vec::new(),
            seen: Vec::new(),
        }]
    );
    assert_eq!(
        check.visited, 3,
        "what went, the box that held it, and the root above it"
    );
    assert_eq!(ui.readers_held(), 0, "the reader goes with the last wait");
}

#[test]
fn a_dialog_closed_ends_a_wait_for_its_absence_and_one_opened_reads_its_own_elements() {
    let (mut ui, _) = form();
    let mut waits = Waits::default();
    let modal = RootSettings {
        modal: true,
        ..RootSettings::on(TargetId::MAIN)
    };
    // A focused button, which the dialog takes the focus from.
    let open = at(&ui, "main/open");
    ui.set_focus(Some(open));
    ui.frame(TargetId::MAIN);
    let unfocused = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        Condition {
            lacking: vec![State::Focused],
            ..condition(Some(Role::Button), "open", Presence::Present)
        },
    ));
    let opened = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Dialog), "confirm", Presence::Present),
    ));
    let dialog = ui.add_root("dialog", modal, |s| s);
    ui.build(dialog, 0, |children| {
        let body = Element::container()
            .role(Role::Dialog)
            .named("confirm")
            .style(|s| sized(s, 100.0, 50.0));
        children.add("body", body, |children| {
            children.add(
                "yes",
                Element::button("yes").style(|s| sized(s, 40.0, 20.0)),
                |_| {},
            );
        });
    });
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    let body = at(&ui, "dialog/body");
    assert_eq!(
        check.met,
        vec![
            Met {
                wait: unfocused,
                elements: vec![open],
                seen: Vec::new(),
            },
            Met {
                wait: opened,
                elements: vec![body],
                seen: Vec::new(),
            },
        ],
        "the focus that the dialog took is seen, though its target's other readers turn whole"
    );
    assert_eq!(
        check.visited, 4,
        "the dialog's three elements and the focus taken"
    );
    let closed = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Dialog), "", Presence::Absent),
    ));
    ui.remove(dialog);
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(
        check.met,
        vec![Met {
            wait: closed,
            elements: Vec::new(),
            seen: Vec::new(),
        }]
    );
    assert_eq!(
        check.visited, 4,
        "the dialog's three elements and the focus given back"
    );
}

#[test]
fn a_box_hidden_and_shown_again_ends_waits_for_what_it_holds() {
    let (mut ui, root) = form();
    let mut waits = Waits::default();
    let gone = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    ));
    build(
        &mut ui,
        root,
        1,
        Shape {
            hidden: true,
            ..SHAPE
        },
    );
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(
        check.met,
        vec![Met {
            wait: gone,
            elements: Vec::new(),
            seen: Vec::new(),
        }]
    );
    let back = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "inner", Presence::Present),
    ));
    build(&mut ui, root, 2, SHAPE);
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    let inner = at(&ui, "main/extra/inner");
    assert_eq!(
        check.met,
        vec![Met {
            wait: back,
            elements: vec![inner],
            seen: Vec::new(),
        }]
    );
}

#[test]
fn states_held_and_lacking_name_and_kind_changed_under_a_key_are_seen() {
    let (mut ui, root) = form();
    let mut waits = Waits::default();
    let save = at(&ui, "main/save");
    let disabled = crate::tree::ElementState {
        disabled: true,
        ..Default::default()
    };
    ui.set_state(save, disabled);
    ui.frame(TargetId::MAIN);
    let enabled = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        Condition {
            lacking: vec![State::Disabled],
            ..condition(Some(Role::Button), "save", Presence::Present)
        },
    ));
    let label = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Label), "status", Presence::Absent),
    ));
    let button = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Button), "status", Presence::Present),
    ));
    let group = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Group), "panel", Presence::Present),
    ));
    assert_eq!(
        frame_and_check(&mut ui, &mut waits, TargetId::MAIN).met,
        Vec::new()
    );
    ui.set_state(save, crate::tree::ElementState::default());
    build(
        &mut ui,
        root,
        1,
        Shape {
            status_button: true,
            named_panel: true,
            ..SHAPE
        },
    );
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    let (status, panel) = (at(&ui, "main/status"), at(&ui, "main/panel"));
    assert_eq!(
        check.met,
        vec![
            Met {
                wait: enabled,
                elements: vec![save],
                seen: Vec::new(),
            },
            Met {
                wait: label,
                elements: Vec::new(),
                seen: Vec::new(),
            },
            Met {
                wait: button,
                elements: vec![status],
                seen: Vec::new(),
            },
            Met {
                wait: group,
                elements: vec![panel],
                seen: Vec::new(),
            },
        ]
    );
}

#[test]
fn a_target_s_check_reads_its_own_waits_and_a_root_moved_away_leaves_them() {
    let (mut ui, root) = form();
    let mut waits = Waits::default();
    let side = ui.add_root("side", RootSettings::on(OTHER), |s| s);
    ui.frame(OTHER);
    let there = held(waits.ask(
        &mut ui,
        OTHER,
        condition(Some(Role::Button), "open", Presence::Present),
    ));
    let gone = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Button), "open", Presence::Absent),
    ));
    assert_eq!(ui.readers_held(), 2, "one reader for each target");
    // The main root moves to the other target: an absence on the main one, a presence there.
    ui.set_root(root, RootSettings::on(OTHER));
    let main = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(main.visited, 209, "every element of the root, leaving");
    assert_eq!(
        main.met,
        vec![Met {
            wait: gone,
            elements: Vec::new(),
            seen: Vec::new(),
        }]
    );
    let other = frame_and_check(&mut ui, &mut waits, OTHER);
    assert_eq!(other.visited, 209, "every element of the root, joining");
    assert_eq!(
        other.met,
        vec![Met {
            wait: there,
            elements: vec![at(&ui, "main/open")],
            seen: Vec::new(),
        }]
    );
    assert_eq!(ui.readers_held(), 0);
    ui.remove(side);
}

#[test]
fn a_reader_past_its_bound_walks_its_target_whole() {
    let (mut ui, root) = form();
    let mut waits = Waits::default();
    let wait = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "row 250", Presence::Present),
    ));
    // Rows made and removed, never checked: 600 made, then removed, are 1 200 marks, past twice
    // the tree of 609 elements.
    for revision in 1..4 {
        let rows = if revision % 2 == 0 { 0 } else { 600 };
        build(&mut ui, root, revision, Shape { rows, ..SHAPE });
    }
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(
        check.visited, 609,
        "the walk of the target: its root, three buttons, the box of 600 rows, and four more"
    );
    assert_eq!(
        check.met,
        vec![Met {
            wait,
            elements: vec![at(&ui, "main/rows/#250")],
            seen: Vec::new()
        }]
    );
}

#[test]
fn cancels_a_wait_once_and_lets_its_reader_go() {
    let (mut ui, root) = form();
    let mut waits = Waits::default();
    let wait = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    ));
    assert_eq!(ui.readers_held(), 1);
    assert!(waits.cancel(&mut ui, wait));
    assert!(!waits.cancel(&mut ui, wait), "cancelled already");
    assert_eq!(ui.readers_held(), 0);
    // A wait met is no longer held.
    let met = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    ));
    build(
        &mut ui,
        root,
        1,
        Shape {
            inner: false,
            ..SHAPE
        },
    );
    assert_eq!(
        frame_and_check(&mut ui, &mut waits, TargetId::MAIN)
            .met
            .len(),
        1
    );
    assert!(!waits.cancel(&mut ui, met));
}

#[test]
fn another_interface_is_refused_and_a_full_one_is_busy() {
    let (mut ui, _) = form();
    let (mut other, _) = form();
    let mut waits = Waits::default();
    let wait = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    ));
    let refused = waits.ask(
        &mut other,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    );
    assert_eq!(refused.map_err(|fault| fault.code), Err(Fault::REFUSED));
    assert_eq!(
        waits.check(&mut other, TargetId::MAIN),
        WaitCheck::default()
    );
    assert!(!waits.cancel(&mut other, wait));
    assert!(waits.cancel(&mut ui, wait));
    // Once it holds none, another interface's waits are taken; one past its readers is busy,
    // unless it is met at once.
    while other.add_meaning_reader(OTHER).is_some() {}
    let busy = waits.ask(
        &mut other,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    );
    assert_eq!(busy.map_err(|fault| fault.code), Err(Fault::BUSY));
    let met = waits.ask(
        &mut other,
        TargetId::MAIN,
        condition(None, "inner", Presence::Present),
    );
    assert!(matches!(met, Ok(Waited::Met { elements, .. }) if elements.len() == 1));
    assert_eq!(MEANING_READERS, other.readers_held());
}

#[test]
fn states_that_hold_a_popup_and_a_name_whatever_its_case_are_matched_as_elements_matches_them() {
    let mut ui = interface();
    let root = ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| s);
    let build = |ui: &mut Ui, revision: u64, agreed: bool| {
        ui.build(root, revision, |children| {
            let agree = Element::check_box("agree", agreed).style(|s| sized(s, 60.0, 20.0));
            children.add("agree", agree, |_| {});
            let pick = Element::choice(["one", "two"], 0).style(|s| sized(s, 60.0, 20.0));
            children.add("pick", pick, |_| {});
            children.add(
                "road",
                Element::label("ΟΔΟΣ").style(|s| sized(s, 60.0, 20.0)),
                |_| {},
            );
        });
    };
    build(&mut ui, 0, false);
    ui.frame(TargetId::MAIN);
    let mut waits = Waits::default();
    let checked = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        Condition {
            holding: vec![State::Checked],
            ..condition(Some(Role::CheckBox), "agree", Presence::Present)
        },
    ));
    let popup = waits.ask(
        &mut ui,
        TargetId::MAIN,
        Condition {
            holding: vec![State::HasPopup],
            ..condition(None, "", Presence::Present)
        },
    );
    assert_eq!(
        popup,
        Ok(Waited::Met {
            elements: vec![at(&ui, "main/pick")],
            seen: Vec::new(),
        })
    );
    // A final sigma, which `interface/elements` writes small as `ς`.
    let road = waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "οδος", Presence::Present),
    );
    assert_eq!(
        road,
        Ok(Waited::Met {
            elements: vec![at(&ui, "main/road")],
            seen: Vec::new(),
        })
    );
    let request = Request::Elements {
        role: String::new(),
        name: "οδος".to_owned(),
        cursor: None,
        limit: None,
        bytes: None,
    };
    let page = Page::new(1, usize::MAX);
    let Ok(Answer::Reply(Reply::Elements { elements, .. })) =
        answer(&mut ui, &request, page, || false)
    else {
        panic!("no elements");
    };
    assert_eq!(elements.len(), 1, "the same name for `interface/elements`");
    build(&mut ui, 1, true);
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    let agree = at(&ui, "main/agree");
    assert_eq!(
        check.met,
        vec![Met {
            wait: checked,
            elements: vec![agree],
            seen: Vec::new(),
        }]
    );
}

#[test]
fn an_element_marked_then_removed_is_read_once() {
    let (mut ui, root) = form();
    let mut waits = Waits::default();
    held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    ));
    let disabled = crate::tree::ElementState {
        disabled: true,
        ..Default::default()
    };
    ui.set_state(at(&ui, "main/extra/inner"), disabled);
    build(
        &mut ui,
        root,
        1,
        Shape {
            inner: false,
            ..SHAPE
        },
    );
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(check.met.len(), 1);
    assert_eq!(
        check.visited, 3,
        "the button once, its box and the root above it"
    );
}

#[test]
fn removals_past_the_bound_walk_the_target_whole() {
    let mut ui = interface();
    let root = ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| {
        s.direction(FlexDirection::Column)
    });
    build(
        &mut ui,
        root,
        0,
        Shape {
            rows: 1_200,
            ..SHAPE
        },
    );
    ui.frame(TargetId::MAIN);
    let mut waits = Waits::default();
    let wait = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "row", Presence::Absent),
    ));
    // The box of 1 200 rows removed whole: its parent is marked first, then each element freed
    // tells the reader, past twice what the tree keeps and past 1 024.
    ui.remove(at(&ui, "main/rows"));
    let check = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(
        check.met,
        vec![Met {
            wait,
            elements: Vec::new(),
            seen: Vec::new()
        }]
    );
    assert_eq!(
        check.visited, 8,
        "a walk of what is left, not the rows removed"
    );
}

#[test]
fn cleared_waits_let_their_readers_go_and_take_another_interface_s() {
    let (mut ui, _) = form();
    let mut waits = Waits::default();
    let first = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    ));
    let second = held(waits.ask(&mut ui, OTHER, condition(None, "inner", Presence::Present)));
    assert_eq!(ui.readers_held(), 2);
    assert_eq!(waits.clear(Some(&mut ui)), vec![first, second]);
    assert_eq!(ui.readers_held(), 0);
    // An interface gone: its waits cleared without it, and the next one's taken.
    held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    ));
    drop(ui);
    let (mut next, _) = form();
    assert_eq!(waits.clear(None).len(), 1);
    held(waits.ask(
        &mut next,
        TargetId::MAIN,
        condition(None, "inner", Presence::Absent),
    ));
}

#[test]
fn waits_met_and_cleared_come_in_the_order_that_they_were_made() {
    let (mut ui, root) = form();
    let mut waits = Waits::default();
    let made: Vec<WaitId> = [Some(Role::Button), None, Some(Role::Button)]
        .into_iter()
        .map(|role| {
            held(waits.ask(
                &mut ui,
                TargetId::MAIN,
                condition(role, "inner", Presence::Absent),
            ))
        })
        .collect();
    assert!(waits.cancel(&mut ui, made[0]));
    build(
        &mut ui,
        root,
        1,
        Shape {
            inner: false,
            ..SHAPE
        },
    );
    let met: Vec<WaitId> = frame_and_check(&mut ui, &mut waits, TargetId::MAIN)
        .met
        .into_iter()
        .map(|met| met.wait)
        .collect();
    assert_eq!(met, made[1..]);
    let on_main = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "status", Presence::Absent),
    ));
    let on_other = held(waits.ask(&mut ui, OTHER, condition(None, "status", Presence::Present)));
    let on_main_again = held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(None, "open", Presence::Absent),
    ));
    assert_eq!(
        waits.clear(Some(&mut ui)),
        vec![on_main, on_other, on_main_again]
    );
}

#[test]
fn paths_moved_panels_raised_and_another_target_s_removals_read_nothing_for_the_waits() {
    let (mut ui, root) = form();
    let below = ui.add_root("below", RootSettings::on(TargetId::MAIN), |s| s);
    ui.build(below, 0, |children| {
        children.add(
            "note",
            Element::label("note").style(|s| sized(s, 40.0, 20.0)),
            |_| {},
        );
    });
    // Two floating panels of 20 labels each, over the main root.
    for (key, origin) in [("first", [200.0, 100.0]), ("second", [250.0, 150.0])] {
        let place = Place::Floating {
            over: root,
            origin,
            size: [100.0, 100.0],
        };
        let layer = ui.open_layer(
            key,
            Layer {
                place,
                modal: false,
                dismiss: false,
            },
            |s| s,
        );
        ui.build(layer, 0, |children| {
            for label in 0..20_usize {
                children.add(
                    label,
                    Element::label(format!("{key} {label}")).style(|s| sized(s, 10.0, 4.0)),
                    |_| {},
                );
            }
        });
    }
    let side = ui.add_root("side", RootSettings::on(OTHER), |s| s);
    ui.build(side, 0, |children| {
        children.add(
            "help",
            Element::button("help").style(|s| sized(s, 40.0, 20.0)),
            |_| {},
        );
    });
    ui.frame(TargetId::MAIN);
    ui.frame(OTHER);
    let mut waits = Waits::default();
    held(waits.ask(
        &mut ui,
        TargetId::MAIN,
        condition(Some(Role::Label), "", Presence::Absent),
    ));
    // A root below the panels removed: their paths change, which the waits do not read.
    ui.remove(below);
    let removed = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    assert_eq!(
        removed.visited, 2,
        "the root removed and its label, not the panels above it"
    );
    // The first panel raised by a press on one of its labels, which take no state.
    ui.pointer_moved(201.0, 101.0);
    frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    let raised = frame_and_check(&mut ui, &mut waits, TargetId::MAIN);
    let top = ui.roots().last().copied();
    assert_eq!(top, ui.element_by_path("first"), "the first panel raised");
    assert_eq!(raised.visited, 0, "the panel raised, not its 21 elements");
    // An element removed on the other target.
    ui.remove(side);
    ui.frame(OTHER);
    assert_eq!(
        frame_and_check(&mut ui, &mut waits, TargetId::MAIN).visited,
        0
    );
}
