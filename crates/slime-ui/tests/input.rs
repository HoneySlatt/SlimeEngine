//! A person's input and an agent's actions, without a GPU: the pointer that presses a button and
//! holds it until let go, the wheel that scrolls, the focus moved by Tab and by directions within
//! its scope, a modal root that keeps it and gives it back, shortcuts read by the key's meaning in
//! their scopes, and the queue of actions that the program reads.

use slime_input::{
    ButtonState, Key, KeyCode, KeyEvent, Modifiers, MouseButton, NamedKey, ScrollDelta,
};
use slime_ui::action::{Action, ActionKind, Intent};
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::{Dimension, Display, FlexDirection, Style, Track};
use slime_ui::text::{Alignment, Fonts};
use slime_ui::theme::{StyleSheet, Theme};
use slime_ui::tree::{Element, ElementId, ElementState, LayoutDirection, RootSettings, Ui};

/// Settings of a root of `width` by `height` logical pixels at `origin`, at scale 1.
fn settings(width: f32, height: f32, origin: [f32; 2], modal: bool) -> RootSettings {
    RootSettings {
        size: [width, height],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin,
        modal,
    }
}

/// A style of `width` by `height` logical pixels.
fn sized(width: f32, height: f32) -> Style {
    let mut style = Style::default();
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
    style
}

/// A button of 100 by 40 logical pixels, of no text.
fn button() -> Element {
    Element::button("", sized(100.0, 40.0))
}

/// An interface whose root holds three buttons in a row, at 0, 100 and 200.
fn row() -> (Ui, Vec<ElementId>) {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut buttons = Vec::new();
    ui.build(root, 0, |children| {
        for index in 0..3_usize {
            buttons.push(children.add(index, button(), |_| {}));
        }
    });
    ui.frame();
    (ui, buttons)
}

/// A press of the key that means `key`, at the position `code`.
fn press(code: KeyCode, key: Key) -> KeyEvent {
    KeyEvent {
        physical: code,
        logical: key,
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    }
}

fn named(key: NamedKey) -> KeyEvent {
    press(KeyCode::Unidentified, Key::Named(key))
}

fn press_action(element: ElementId) -> Action {
    Action {
        element: Some(element),
        kind: ActionKind::Press,
    }
}

#[test]
fn a_button_pressed_and_let_go_over_it_gives_a_press_and_holds_the_pointer_meanwhile() {
    let (mut ui, buttons) = row();
    ui.pointer_moved(150.0, 20.0);
    assert!(ui.state(buttons[1]).unwrap().hovered);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    assert!(ui.state(buttons[1]).unwrap().pressed);
    assert_eq!(ui.focus(), Some(buttons[1]));
    // Over another button while it holds the pointer: neither is hovered but it, when over it.
    ui.pointer_moved(250.0, 20.0);
    assert!(!ui.state(buttons[2]).unwrap().hovered);
    assert!(!ui.state(buttons[1]).unwrap().hovered);
    ui.pointer_moved(150.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(ui.take_actions(), [press_action(buttons[1])]);
    assert!(!ui.state(buttons[1]).unwrap().pressed);
    // Let go elsewhere: no press.
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(390.0, 290.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert!(ui.take_actions().is_empty());
    // A disabled button takes nothing, from the pointer or from an agent.
    let mut state = ui.state(buttons[0]).unwrap();
    state.disabled = true;
    ui.set_state(buttons[0], state);
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.act(press_action(buttons[0]));
    assert!(ui.take_actions().is_empty());
    assert_eq!(ui.element_at(50.0, 20.0), Some(buttons[0]));
}

#[test]
fn an_agent_s_actions_reach_the_queue_as_a_person_s() {
    let (mut ui, buttons) = row();
    ui.act(press_action(buttons[2]));
    ui.act(Action {
        element: None,
        kind: ActionKind::Intent(Intent::Next),
    });
    assert_eq!(ui.focus(), Some(buttons[0]));
    ui.act(Action {
        element: None,
        kind: ActionKind::Intent(Intent::Accept),
    });
    ui.act(Action {
        element: None,
        kind: ActionKind::Intent(Intent::Cancel),
    });
    assert_eq!(
        ui.take_actions(),
        [
            press_action(buttons[2]),
            press_action(buttons[0]),
            Action {
                element: None,
                kind: ActionKind::Intent(Intent::Cancel),
            },
        ]
    );
}

#[test]
fn tab_moves_the_focus_in_the_order_of_the_tree_and_the_arrows_to_the_nearest_on_screen() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut grid = Style::default();
    grid.layout.display = Display::Grid;
    grid.layout.columns = vec![Track::Pixels(100.0); 3];
    grid.layout.rows = vec![Track::Pixels(40.0); 2];
    let mut cells = Vec::new();
    ui.build(root, 0, |children| {
        children.add("grid", Element::container(grid), |children| {
            for index in 0..6_usize {
                cells.push(children.add(index, button(), |_| {}));
            }
        });
    });
    ui.frame();
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(cells[0]));
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(cells[1]));
    // Down, to the cell below, not the next in the tree.
    ui.key(&named(NamedKey::ArrowDown));
    assert_eq!(ui.focus(), Some(cells[4]));
    ui.key(&named(NamedKey::ArrowLeft));
    assert_eq!(ui.focus(), Some(cells[3]));
    ui.key(&named(NamedKey::ArrowLeft));
    assert_eq!(ui.focus(), Some(cells[3]), "nothing further left");
    let mut shift = Modifiers::default();
    shift.shift = true;
    ui.modifiers(shift);
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(cells[2]));
    assert!(ui.state(cells[2]).unwrap().focused);
    assert!(!ui.state(cells[3]).unwrap().focused);
}

/// The focus that Tab moves out of a box that scrolls is scrolled into view, and a list shows a
/// row by its number, built or not (`docs/adr/0167-the-hierarchy.md`).
#[test]
fn the_focus_that_keys_move_and_a_row_asked_for_are_scrolled_into_view() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut column = sized(100.0, 100.0);
    column.layout.direction = FlexDirection::Column;
    let (mut scroll, mut buttons, mut list) = (None, Vec::new(), None);
    ui.build(root, 0, |children| {
        scroll = Some(children.add("scroll", Element::scroll(column), |children| {
            for index in 0..10_usize {
                let mut tall = sized(100.0, 40.0);
                tall.layout.shrink = 0.0;
                buttons.push(children.add(index, Element::button("", tall), |_| {}));
            }
        }));
        list = Some(children.add_slot("list", Element::list(1_000, 20.0, sized(100.0, 100.0))));
    });
    let (scroll, list) = (scroll.expect("built"), list.expect("built"));
    ui.frame();
    // Buttons 40 high in a box 100 high: the fourth is out of view until Tab reaches it.
    for _ in 0..4 {
        ui.key(&named(NamedKey::Tab));
    }
    assert_eq!(ui.focus(), Some(buttons[3]));
    assert_eq!(ui.scroll(scroll), Some([0.0, 4.0 * 40.0 - 100.0]));
    ui.frame();
    // Back to the first: scrolled to its top.
    let mut shift = Modifiers::default();
    shift.shift = true;
    ui.modifiers(shift);
    for _ in 0..3 {
        ui.key(&named(NamedKey::Tab));
    }
    assert_eq!(ui.focus(), Some(buttons[0]));
    assert_eq!(ui.scroll(scroll), Some([0.0, 0.0]));
    // A row far down, built or not: as little as shows it whole.
    ui.show_row(list, 500);
    assert_eq!(ui.scroll(list), Some([0.0, 501.0 * 20.0 - 100.0]));
    ui.frame();
    ui.show_row(list, 10);
    assert_eq!(ui.scroll(list), Some([0.0, 200.0]));
    ui.show_row(list, 12);
    assert_eq!(ui.scroll(list), Some([0.0, 200.0]), "it shows already");
    ui.show_row(list, 5_000);
    assert_eq!(ui.scroll(list), Some([0.0, 200.0]), "past the end");
}

/// At two pixels a logical pixel, an element is scrolled into view by logical pixels, and a list
/// not laid out yet shows a row at its top (`docs/adr/0167-the-hierarchy.md`).
#[test]
fn scrolling_into_view_counts_logical_pixels_and_a_list_not_laid_out_shows_a_row_at_its_top() {
    let mut ui = Ui::new(Fonts::new());
    let mut doubled = settings(400.0, 300.0, [0.0; 2], false);
    doubled.scale = 2.0;
    let root = ui.add_root("main", doubled, Style::default());
    let mut column = sized(100.0, 100.0);
    column.layout.direction = FlexDirection::Column;
    let (mut scroll, mut buttons, mut list) = (None, Vec::new(), None);
    ui.build(root, 0, |children| {
        scroll = Some(children.add("scroll", Element::scroll(column), |children| {
            for index in 0..10_usize {
                let mut tall = sized(100.0, 40.0);
                tall.layout.shrink = 0.0;
                buttons.push(children.add(index, Element::button("", tall), |_| {}));
            }
        }));
        list = Some(children.add_slot("list", Element::list(1_000, 20.0, sized(100.0, 100.0))));
    });
    let (scroll, list) = (scroll.expect("built"), list.expect("built"));
    // Before any layout: the row at the list's top.
    ui.show_row(list, 50);
    assert_eq!(ui.scroll(list), Some([0.0, 1_000.0]));
    ui.frame();
    ui.scroll_into_view(buttons[5]);
    assert_eq!(ui.scroll(scroll), Some([0.0, 6.0 * 40.0 - 100.0]));
}

/// A draggable element pressed and moved past a few pixels is dragged rather than pressed, onto the
/// target of drops under the pointer, where in it as the pointer stands; released there, it drops;
/// Escape, or a release over nothing, gives it up; an agent drops it as a person would
/// (`docs/adr/0168-rearranging-the-hierarchy.md`).
#[test]
fn a_draggable_element_drops_on_a_target_and_agents_drop_it_too() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut column = Style::default();
    column.layout.direction = FlexDirection::Column;
    let (mut source, mut target) = (None, None);
    ui.build(root, 0, |children| {
        children.add("column", Element::container(column.clone()), |children| {
            source = Some(children.add(
                "source",
                Element::button("", sized(100.0, 40.0)).draggable(),
                |_| {},
            ));
            target = Some(
                children.add(
                    "target",
                    Element::container(sized(100.0, 40.0))
                        .named("target")
                        .drop_target(),
                    |_| {},
                ),
            );
        });
    });
    let (source, target) = (source.expect("built"), target.expect("built"));
    ui.frame();
    let dropped = |ui: &mut Ui| {
        ui.take_actions()
            .into_iter()
            .filter_map(|action| match action.kind {
                ActionKind::Dropped { source, at } => Some((action.element, source, at)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // Pressed and moved a pixel: no drag, and the release presses it.
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(51.0, 20.0);
    assert_eq!(ui.dragging(), None);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert!(
        ui.take_actions()
            .iter()
            .any(|action| action.kind == ActionKind::Press)
    );
    // Dragged onto the target's lower quarter, then let go there.
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(50.0, 70.0);
    let dragging = ui.dragging().expect("a drag");
    assert_eq!((dragging.source, dragging.target), (source, Some(target)));
    assert!((dragging.at - 0.75).abs() < 1e-3, "{}", dragging.at);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    let drops = dropped(&mut ui);
    assert_eq!(drops.len(), 1);
    assert_eq!((drops[0].0, drops[0].1), (Some(target), source));
    assert!(ui.take_actions().is_empty(), "no press");
    // Given up by Escape, and released over nothing: no drop.
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(50.0, 70.0);
    ui.key(&named(NamedKey::Escape));
    assert_eq!(ui.dragging(), None);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert!(dropped(&mut ui).is_empty());
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(300.0, 250.0);
    assert_eq!(ui.dragging().and_then(|dragging| dragging.target), None);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert!(dropped(&mut ui).is_empty());
    // An agent: the drop that the meaning offers.
    let meaning = ui.meaning_of(source).expect("it means something");
    assert!(
        meaning
            .actions
            .contains(&slime_ui::tree::ElementAction::Drop)
    );
    assert!(ui.drop(source, target, 0.5));
    assert_eq!(dropped(&mut ui).len(), 1);
    assert!(
        !ui.drop(target, source, 0.5),
        "neither draggable nor a target"
    );
    // Given up by Escape with the pointer out of the window: it does not come back with it.
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(50.0, 70.0);
    ui.pointer_left();
    ui.key(&named(NamedKey::Escape));
    ui.pointer_moved(50.0, 70.0);
    assert_eq!(ui.dragging(), None);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    // Its element built away during the drag, as a list's row scrolled out of sight: the drag goes
    // on and drops, the element named as it was; the next click presses as any.
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(50.0, 70.0);
    let mut again = None;
    ui.build(root, 1, |children| {
        children.add("column", Element::container(column.clone()), |children| {
            children.add("spacer", Element::container(sized(100.0, 40.0)), |_| {});
            again = Some(
                children.add(
                    "target",
                    Element::container(sized(100.0, 40.0))
                        .named("target")
                        .drop_target(),
                    |_| {},
                ),
            );
        });
    });
    ui.frame();
    assert_eq!(again, Some(target), "the same target");
    assert_eq!(ui.dragging().map(|dragging| dragging.source), Some(source));
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(dropped(&mut ui), [(Some(target), source, 0.75)]);
    let mut button = None;
    ui.build(root, 2, |children| {
        children.add("column", Element::container(column.clone()), |children| {
            button = Some(children.add("other", Element::button("", sized(100.0, 40.0)), |_| {}));
        });
    });
    ui.frame();
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(
        ui.take_actions()
            .iter()
            .filter(|action| action.kind == ActionKind::Press)
            .map(|action| action.element)
            .collect::<Vec<_>>(),
        [button]
    );
}

#[test]
fn a_modal_root_takes_the_pointer_and_the_focus_and_gives_the_focus_back_when_it_closes() {
    let (mut ui, buttons) = row();
    ui.set_focus(Some(buttons[1]));
    let modal = ui.add_root(
        "main",
        settings(200.0, 100.0, [100.0, 100.0], true),
        Style::default(),
    );
    let mut inside = Vec::new();
    ui.build(modal, 0, |children| {
        inside.push(children.add("ok", button(), |_| {}));
        inside.push(children.add("cancel", button(), |_| {}));
    });
    ui.frame();
    assert_eq!(ui.focus(), Some(inside[0]));
    assert_eq!(ui.rect(inside[1]).map(|rect| rect.x), Some(200.0));
    // The roots below take no pointer, and the focus stays inside.
    assert_eq!(ui.element_at(50.0, 20.0), None);
    assert_eq!(ui.element_at(150.0, 120.0), Some(inside[0]));
    ui.set_focus(Some(buttons[2]));
    assert_eq!(ui.focus(), Some(inside[0]));
    ui.key(&named(NamedKey::Tab));
    ui.key(&named(NamedKey::Tab));
    assert_eq!(
        ui.focus(),
        Some(inside[0]),
        "Tab cycles within the modal root"
    );
    // Escape asks the program to close it; closed, the focus comes back.
    ui.key(&named(NamedKey::Escape));
    assert_eq!(
        ui.take_actions(),
        [Action {
            element: Some(inside[0]),
            kind: ActionKind::Intent(Intent::Cancel),
        }]
    );
    ui.remove(modal);
    assert_eq!(ui.focus(), Some(buttons[1]));
    assert_eq!(ui.element_at(50.0, 20.0), Some(buttons[0]));
}

#[test]
fn the_wheel_scrolls_the_box_under_the_pointer() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut scroll = None;
    ui.build(root, 0, |children| {
        let mut style = sized(200.0, 100.0);
        style.layout.direction = FlexDirection::Column;
        scroll = Some(children.add("scroll", Element::scroll(style), |children| {
            for index in 0..10_usize {
                let mut item = sized(200.0, 40.0);
                item.layout.shrink = 0.0;
                children.add(index, Element::container(item), |_| {});
            }
        }));
    });
    ui.frame();
    ui.pointer_moved(50.0, 50.0);
    ui.wheel(ScrollDelta::Lines { x: 0.0, y: -2.0 });
    assert_eq!(ui.scroll(scroll.unwrap()), Some([0.0, 80.0]));
    ui.wheel(ScrollDelta::Pixels { x: 0.0, y: 30.0 });
    assert_eq!(ui.scroll(scroll.unwrap()), Some([0.0, 50.0]));
    // Outside, nothing scrolls.
    ui.pointer_moved(350.0, 250.0);
    ui.wheel(ScrollDelta::Lines { x: 0.0, y: -2.0 });
    assert_eq!(ui.scroll(scroll.unwrap()), Some([0.0, 50.0]));
}

#[test]
fn shortcuts_name_commands_by_the_key_s_meaning_in_the_scopes_of_the_focus() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut buttons = Vec::new();
    ui.build(root, 0, |children| {
        children.add(
            "panel",
            Element::container(Style::default()).class("hierarchy"),
            |children| {
                buttons.push(children.add("inside", button(), |_| {}));
            },
        );
        buttons.push(children.add("outside", button(), |_| {}));
    });
    ui.frame();
    let table = "[global]\n\"Ctrl+Z\" = \"undo\"\nDelete = \"delete everything\"\nT = \"theme\"\n\n\
                 [hierarchy]\nDelete = \"delete\"\n";
    ui.set_shortcuts(&[Shortcuts::from_text("editor.keys", table)])
        .expect("a table of shortcuts");
    let command = |ui: &mut Ui, event: KeyEvent| {
        ui.key(&event);
        ui.take_actions()
            .into_iter()
            .map(|action| match action.kind {
                ActionKind::Command(name) => name,
                other => format!("{other:?}"),
            })
            .collect::<Vec<_>>()
    };
    // Ctrl+Z is the key that means `z`: on an AZERTY keyboard, the key labelled Z, at the place of
    // a US keyboard's W.
    let mut control = Modifiers::default();
    control.control = true;
    ui.modifiers(control);
    let z = press(KeyCode::KeyW, Key::Character("z".to_owned()));
    assert_eq!(command(&mut ui, z.clone()), ["undo"]);
    // Held, it gives its command once: its repeats are taken, and give none.
    let held = KeyEvent { repeat: true, ..z };
    assert!(ui.key(&held));
    assert!(ui.take_actions().is_empty());
    let w = press(KeyCode::KeyZ, Key::Character("w".to_owned()));
    assert!(command(&mut ui, w).is_empty());
    ui.modifiers(Modifiers::default());
    assert_eq!(
        command(
            &mut ui,
            press(KeyCode::KeyT, Key::Character("T".to_owned()))
        ),
        ["theme"]
    );
    // The innermost scope of the focus first.
    ui.set_focus(Some(buttons[0]));
    assert_eq!(command(&mut ui, named(NamedKey::Delete)), ["delete"]);
    ui.set_focus(Some(buttons[1]));
    assert_eq!(
        command(&mut ui, named(NamedKey::Delete)),
        ["delete everything"]
    );
    // A later table wins for one chord in one scope; a wrong chord is refused by its place.
    let later = Shortcuts::from_text("game.keys", "[global]\nT = \"talk\"\n");
    ui.set_shortcuts(&[Shortcuts::from_text("editor.keys", table), later])
        .expect("two tables");
    assert_eq!(
        command(
            &mut ui,
            press(KeyCode::KeyT, Key::Character("t".to_owned()))
        ),
        ["talk"]
    );
    for (wrong, problem) in [
        ("[global]\n\"Cmd+Z\" = \"undo\"\n", "is not a modifier"),
        ("[global]\nDel = \"delete\"\n", "does not end with a key"),
        ("undo = \"Ctrl+Z\"\n", "is not a scope"),
        ("[global]\n\"Ctrl+Shift+1\" = \"first\"\n", "has no case"),
    ] {
        let error = ui
            .set_shortcuts(&[Shortcuts::from_text("wrong.keys", wrong)])
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("wrong.keys: line ") && error.contains(problem),
            "{error}"
        );
    }
}

#[test]
fn shift_is_left_out_of_a_character_without_case_and_a_letter_of_another_script_is_its_place() {
    let (mut ui, _) = row();
    let table = "[global]\n\"Ctrl++\" = \"zoom\"\n\"Ctrl+1\" = \"first\"\n\"Ctrl+Z\" = \"undo\"\n\
                 \"Ctrl+Shift+Z\" = \"redo\"\n";
    ui.set_shortcuts(&[Shortcuts::from_text("editor.keys", table)])
        .expect("a table of shortcuts");
    let command = |ui: &mut Ui, shift: bool, event: KeyEvent| {
        let mut modifiers = Modifiers::default();
        modifiers.control = true;
        modifiers.shift = shift;
        ui.modifiers(modifiers);
        ui.key(&event);
        ui.take_actions()
            .into_iter()
            .map(|action| format!("{:?}", action.kind))
            .collect::<Vec<_>>()
    };
    let character = |code: KeyCode, text: &str| press(code, Key::Character(text.to_owned()));
    // `+` with Shift on a US keyboard, `1` with Shift on an AZERTY one, and both without on the
    // keypad.
    for shift in [true, false] {
        assert_eq!(
            command(&mut ui, shift, character(KeyCode::Equal, "+")),
            [r#"Command("zoom")"#]
        );
        assert_eq!(
            command(&mut ui, shift, character(KeyCode::Digit1, "1")),
            [r#"Command("first")"#]
        );
    }
    // A letter's case is its own: Shift chooses between them.
    assert_eq!(
        command(&mut ui, true, character(KeyCode::KeyZ, "Z")),
        [r#"Command("redo")"#]
    );
    // A Cyrillic layout types `я` at the place of Z.
    assert_eq!(
        command(&mut ui, false, character(KeyCode::KeyZ, "я")),
        [r#"Command("undo")"#]
    );
    assert!(command(&mut ui, false, character(KeyCode::KeyX, "ч")).is_empty());
}

#[test]
fn the_focus_moves_within_its_root_and_starts_in_the_top_root_that_can_take_it() {
    let (mut ui, below) = row();
    let above = ui.add_root(
        "main",
        settings(400.0, 100.0, [0.0, 200.0], false),
        Style::default(),
    );
    let mut buttons = Vec::new();
    ui.build(above, 0, |children| {
        for index in 0..2_usize {
            buttons.push(children.add(index, button(), |_| {}));
        }
    });
    ui.frame();
    // Without a focus, Tab starts in the top root, and Shift+Tab from its last element.
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(buttons[0]));
    ui.set_focus(None);
    let mut shift = Modifiers::default();
    shift.shift = true;
    ui.modifiers(shift);
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(buttons[1]));
    ui.modifiers(Modifiers::default());
    // Within a root, around, without reaching another.
    ui.set_focus(Some(below[2]));
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(below[0]));
    ui.key(&named(NamedKey::ArrowDown));
    assert_eq!(
        ui.focus(),
        Some(below[0]),
        "the root above is another scope"
    );
    // A top root with nothing to focus leaves it to the one below.
    ui.remove(above);
    let empty = ui.add_root(
        "main",
        settings(100.0, 100.0, [0.0; 2], false),
        Style::default(),
    );
    ui.frame();
    ui.set_focus(None);
    ui.key(&named(NamedKey::Tab));
    assert_eq!(ui.focus(), Some(below[0]));
    assert!(ui.state(empty).is_some());
}

#[test]
fn a_disabled_or_hidden_button_is_not_pressed_and_loses_the_focus() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let build = |ui: &mut Ui, revision: u64, display: Display| {
        let mut buttons = Vec::new();
        let mut panel = Style::default();
        panel.layout.display = display;
        ui.build(root, revision, |children| {
            buttons.push(children.add("first", button(), |_| {}));
            children.add("panel", Element::container(panel), |children| {
                buttons.push(children.add("inside", button(), |_| {}));
            });
        });
        ui.frame();
        buttons
    };
    let buttons = build(&mut ui, 0, Display::Flex);
    let accept = Action {
        element: None,
        kind: ActionKind::Intent(Intent::Accept),
    };
    // Disabled while it holds the focus: Accept presses nothing, and the next frame drops it.
    ui.set_focus(Some(buttons[0]));
    ui.set_state(
        buttons[0],
        ElementState {
            disabled: true,
            ..ElementState::default()
        },
    );
    assert!(ui.state(buttons[0]).unwrap().focused, "the focus's own");
    ui.act(accept.clone());
    assert_eq!(
        ui.take_actions(),
        [Action {
            element: Some(buttons[0]),
            kind: ActionKind::Intent(Intent::Accept),
        }]
    );
    ui.frame();
    assert_eq!(ui.focus(), None);
    assert!(!ui.state(buttons[0]).unwrap().focused);
    // Hidden by an ancestor: neither the focus, nor an agent's press, nor Tab reach it.
    ui.set_focus(Some(buttons[1]));
    let buttons = build(&mut ui, 1, Display::None);
    assert_eq!(ui.focus(), None);
    ui.set_focus(Some(buttons[1]));
    ui.act(press_action(buttons[1]));
    ui.act(accept);
    assert_eq!(
        ui.take_actions(),
        [Action {
            element: None,
            kind: ActionKind::Intent(Intent::Accept),
        }]
    );
    ui.key(&named(NamedKey::Tab));
    assert_eq!(
        ui.focus(),
        None,
        "the first button disabled, the other hidden"
    );
}

#[test]
fn modal_roots_closed_in_either_order_give_the_focus_back_to_what_it_was_before_them() {
    for below_first in [true, false] {
        let (mut ui, buttons) = row();
        ui.set_focus(Some(buttons[1]));
        let mut modals = Vec::new();
        for _ in 0..2 {
            let modal = ui.add_root(
                "main",
                settings(200.0, 100.0, [0.0; 2], true),
                Style::default(),
            );
            ui.build(modal, 0, |children| {
                children.add("ok", button(), |_| {});
            });
            ui.frame();
            modals.push(modal);
        }
        // Under a modal root, an agent presses nothing either.
        ui.act(press_action(buttons[0]));
        assert!(ui.take_actions().is_empty());
        let (first, second) = if below_first {
            (modals[0], modals[1])
        } else {
            (modals[1], modals[0])
        };
        ui.remove(first);
        ui.frame();
        ui.remove(second);
        ui.frame();
        assert_eq!(ui.focus(), Some(buttons[1]), "below first: {below_first}");
    }
}

#[test]
fn a_press_without_the_release_of_the_one_before_lets_go_of_what_that_one_held() {
    let (mut ui, buttons) = row();
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    // The release was lost, outside the window: the next press over another button takes it.
    ui.pointer_moved(150.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    assert!(!ui.state(buttons[0]).unwrap().pressed);
    assert!(ui.state(buttons[1]).unwrap().pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(ui.take_actions(), [press_action(buttons[1])]);
    // Over nothing, a press lets go of what the one before held, and presses nothing.
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(390.0, 290.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    assert!(!ui.state(buttons[1]).unwrap().pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert!(ui.take_actions().is_empty());
}

#[test]
fn the_hover_follows_what_moves_under_a_still_pointer() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut buttons = Vec::new();
    ui.build(root, 0, |children| {
        for index in 0..3_usize {
            buttons.push(children.add(index, button(), |_| {}));
        }
    });
    ui.frame();
    ui.pointer_moved(50.0, 20.0);
    assert!(ui.state(buttons[0]).unwrap().hovered);
    // The first button gone, the second takes its place under the pointer: the frame that lays it
    // out there shows it, the next finds it hovered.
    ui.build(root, 1, |children| {
        for index in 1..3_usize {
            children.add(index, button(), |_| {});
        }
    });
    ui.frame();
    assert!(!ui.state(buttons[1]).unwrap().hovered);
    ui.frame();
    assert!(ui.state(buttons[1]).unwrap().hovered);
}

#[test]
fn the_pointer_and_the_wheel_find_their_way_in_a_root_from_right_to_left() {
    let mut ui = Ui::new(Fonts::new());
    let mut root_settings = settings(300.0, 200.0, [100.0, 50.0], false);
    root_settings.direction = LayoutDirection::RightToLeft;
    let root = ui.add_root("main", root_settings, Style::default());
    let mut first = None;
    let mut scroll = None;
    let mut items = Vec::new();
    ui.build(root, 0, |children| {
        first = Some(children.add("first", button(), |_| {}));
        scroll = Some(
            children.add("scroll", Element::scroll(sized(150.0, 100.0)), |children| {
                for index in 0..4_usize {
                    let mut item = sized(100.0, 40.0);
                    item.layout.shrink = 0.0;
                    items.push(children.add(index, Element::container(item), |_| {}));
                }
            }),
        );
    });
    ui.frame();
    // The root starts at its origin, and its first child at its right.
    assert_eq!(ui.element_at(350.0, 70.0), first);
    // The scroll box lies left of the button, from 150 to 300; its first item at its right, and
    // the second cut by its clip at 150.
    assert_eq!(ui.element_at(250.0, 70.0), Some(items[0]));
    assert_eq!(ui.element_at(170.0, 70.0), Some(items[1]));
    assert_eq!(ui.element_at(120.0, 70.0), Some(root));
    // A wheel that moves the content right shows what lies left: from right to left, its end.
    ui.pointer_moved(250.0, 70.0);
    ui.wheel(ScrollDelta::Lines { x: 1.0, y: 0.0 });
    assert_eq!(ui.scroll(scroll.unwrap()), Some([40.0, 0.0]));
}

#[test]
fn a_rule_selects_the_element_that_holds_the_focus() {
    let (mut ui, buttons) = row();
    let sheet = "[focused]\nselect = { states = [\"Focused\"] }\nstyle.paint.border_width = 2\n";
    ui.set_style(
        &[StyleSheet::from_text("focus.style", sheet)],
        &[Theme::from_text("plain.theme", "")],
    )
    .expect("a style sheet");
    ui.set_focus(Some(buttons[1]));
    ui.frame();
    let border = |ui: &Ui, id: ElementId| ui.style(id).expect("an element").paint.border_width;
    assert_eq!(border(&ui, buttons[1]), 2.0);
    assert_eq!(border(&ui, buttons[0]), 0.0);
}

#[test]
fn a_press_on_what_the_interface_draws_is_its_own_and_a_modal_root_takes_every_press_and_key() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    // A panel with a fill, at the left, and a box without one, at the right: the game's.
    let mut panel = sized(200.0, 300.0);
    panel.paint.background = slime_color::LinearColor::rgba(0.1, 0.1, 0.1, 1.0);
    ui.build(root, 0, |children| {
        children.add("panel", Element::container(panel), |_| {});
        children.add("empty", Element::container(sized(200.0, 300.0)), |_| {});
    });
    ui.frame();
    let both = |ui: &mut Ui, button| {
        let pressed = ui.pointer_button(button, ButtonState::Pressed);
        let released = ui.pointer_button(button, ButtonState::Released);
        (pressed, released)
    };
    ui.pointer_moved(100.0, 100.0);
    assert_eq!(both(&mut ui, MouseButton::Left), (true, true), "the panel");
    assert_eq!(
        both(&mut ui, MouseButton::Right),
        (true, true),
        "any button"
    );
    assert!(ui.wheel(ScrollDelta::Lines { x: 0.0, y: 1.0 }));
    ui.pointer_moved(300.0, 100.0);
    assert_eq!(both(&mut ui, MouseButton::Left), (false, false), "no fill");
    assert!(!ui.wheel(ScrollDelta::Lines { x: 0.0, y: 1.0 }));
    // A press that the interface took keeps its release, wherever the pointer goes.
    ui.pointer_moved(100.0, 100.0);
    assert!(ui.pointer_button(MouseButton::Left, ButtonState::Pressed));
    ui.pointer_moved(300.0, 100.0);
    assert!(ui.pointer_button(MouseButton::Left, ButtonState::Released));
    // A modal root takes every press, every wheel and every key pressed, not their releases.
    let modal = ui.add_root(
        "main",
        settings(100.0, 100.0, [0.0; 2], true),
        Style::default(),
    );
    ui.frame();
    assert_eq!(both(&mut ui, MouseButton::Left), (true, true));
    assert!(ui.wheel(ScrollDelta::Lines { x: 0.0, y: 1.0 }));
    assert!(ui.key(&press(KeyCode::KeyW, Key::Character("w".to_owned()))));
    assert!(ui.key(&named(NamedKey::F5)));
    let mut release = named(NamedKey::F5);
    release.state = ButtonState::Released;
    assert!(!ui.key(&release));
    ui.remove(modal);
    ui.frame();
    assert_eq!(both(&mut ui, MouseButton::Left), (false, false));
    assert!(!ui.key(&press(KeyCode::KeyW, Key::Character("w".to_owned()))));
}

#[test]
fn without_a_focus_only_tab_enters_the_interface_and_the_arrows_are_the_game_s() {
    let (mut ui, buttons) = row();
    assert!(!ui.key(&named(NamedKey::ArrowRight)));
    assert!(!ui.key(&named(NamedKey::ArrowDown)));
    assert_eq!(ui.focus(), None, "an arrow gives no focus");
    assert!(!ui.key(&named(NamedKey::Space)), "nothing to accept");
    assert!(ui.key(&named(NamedKey::Tab)), "Tab gives the focus");
    assert_eq!(ui.focus(), Some(buttons[0]));
    assert!(ui.key(&named(NamedKey::ArrowRight)));
    assert_eq!(ui.focus(), Some(buttons[1]));
}

/// An intent of no element, as a key that the interface leaves to the program queues it.
fn intent(intent: Intent) -> Action {
    Action {
        element: None,
        kind: ActionKind::Intent(intent),
    }
}

/// A press and a release of the primary button at `x` and `y`.
fn click(ui: &mut Ui, x: f32, y: f32) {
    ui.pointer_moved(x, y);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
}

#[test]
fn a_focus_that_a_press_gave_leaves_the_keys_to_the_program_but_tab_goes_on_from_it() {
    let (mut ui, buttons) = row();
    click(&mut ui, 50.0, 20.0);
    assert_eq!(ui.take_actions(), [press_action(buttons[0])]);
    assert_eq!(ui.focus(), Some(buttons[0]));
    // The arrows, Enter and Escape are the program's: the button is not pressed again, and the
    // intents reach the queue as those of no element.
    assert!(!ui.key(&named(NamedKey::ArrowRight)));
    assert!(!ui.key(&named(NamedKey::Enter)));
    assert!(!ui.key(&named(NamedKey::Escape)));
    assert_eq!(ui.focus(), Some(buttons[0]));
    assert_eq!(
        ui.take_actions(),
        [intent(Intent::Accept), intent(Intent::Cancel)]
    );
    // Tab goes on from it, as the interface lets Tab in by default; the focus that it gave takes
    // the arrows.
    assert!(ui.key(&named(NamedKey::Tab)));
    assert_eq!(ui.focus(), Some(buttons[1]));
    assert!(ui.key(&named(NamedKey::ArrowRight)));
    assert_eq!(ui.focus(), Some(buttons[2]));
}

#[test]
fn a_game_s_interface_takes_tab_once_it_holds_the_focus_and_f6_moves_between_its_roots() {
    let (mut ui, below) = row();
    let above = ui.add_root(
        "main",
        settings(400.0, 100.0, [0.0, 200.0], false),
        Style::default(),
    );
    let mut buttons = Vec::new();
    ui.build(above, 0, |children| {
        for index in 0..2_usize {
            buttons.push(children.add(index, button(), |_| {}));
        }
    });
    ui.frame();
    ui.set_tab_enters(false);
    // Tab is the game's without a focus, and with the one that a press gave.
    assert!(!ui.key(&named(NamedKey::Tab)));
    assert_eq!(ui.focus(), None);
    click(&mut ui, 50.0, 20.0);
    ui.take_actions();
    assert!(!ui.key(&named(NamedKey::Tab)));
    assert_eq!(ui.focus(), Some(below[0]));
    // F6 gives the focus to the top root's first element, then the next root's, then to nothing;
    // the focus that it gave takes Tab, within its root.
    assert!(ui.key(&named(NamedKey::F6)));
    assert_eq!(ui.focus(), Some(buttons[0]));
    assert!(ui.key(&named(NamedKey::Tab)));
    assert_eq!(ui.focus(), Some(buttons[1]));
    assert!(ui.key(&named(NamedKey::F6)));
    assert_eq!(ui.focus(), Some(below[0]));
    assert!(ui.key(&named(NamedKey::F6)));
    assert_eq!(ui.focus(), None);
    // Shift+F6 goes the other way, the bottom root first.
    let mut shift = Modifiers::default();
    shift.shift = true;
    ui.modifiers(shift);
    assert!(ui.key(&named(NamedKey::F6)));
    assert_eq!(ui.focus(), Some(below[0]));
    assert!(ui.key(&named(NamedKey::F6)));
    assert_eq!(ui.focus(), Some(buttons[0]));
    ui.modifiers(Modifiers::default());
    // The focus that the program gives takes Tab too.
    ui.set_focus(Some(buttons[1]));
    assert!(ui.key(&named(NamedKey::Tab)));
    assert_eq!(ui.focus(), Some(buttons[0]));
    // A modal root takes Tab whatever the setting, and keeps the focus from F6.
    ui.set_focus(None);
    let modal = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], true),
        Style::default(),
    );
    let mut ok = None;
    ui.build(modal, 0, |children| {
        ok = Some(children.add("ok", button(), |_| {}));
    });
    ui.frame();
    assert!(ui.key(&named(NamedKey::Tab)));
    assert_eq!(ui.focus(), ok);
    assert!(ui.key(&named(NamedKey::F6)));
    assert_eq!(ui.focus(), ok);
    // With no root that holds an element to focus, F6 is the game's.
    let mut empty = Ui::new(Fonts::new());
    empty.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    empty.frame();
    assert!(!empty.key(&named(NamedKey::F6)));
}

#[test]
fn a_release_of_a_press_the_interface_did_not_take_is_not_taken_and_an_empty_label_takes_none() {
    let mut ui = Ui::new(Fonts::new());
    let root = ui.add_root(
        "main",
        settings(400.0, 300.0, [0.0; 2], false),
        Style::default(),
    );
    let mut panel = sized(200.0, 300.0);
    panel.paint.background = slime_color::LinearColor::rgba(0.1, 0.1, 0.1, 1.0);
    // A label of no text, stretched across the right half: it draws nothing.
    ui.build(root, 0, |children| {
        children.add("panel", Element::container(panel), |_| {});
        children.add("label", Element::label("", sized(200.0, 300.0)), |_| {});
    });
    ui.frame();
    // Pressed over nothing, let go over the panel: the game keeps both.
    ui.pointer_moved(300.0, 100.0);
    assert!(!ui.pointer_button(MouseButton::Left, ButtonState::Pressed));
    ui.pointer_moved(100.0, 100.0);
    assert!(!ui.pointer_button(MouseButton::Left, ButtonState::Released));
    // The empty label takes neither a press nor the wheel.
    ui.pointer_moved(300.0, 100.0);
    assert!(!ui.pointer_button(MouseButton::Left, ButtonState::Pressed));
    assert!(!ui.wheel(ScrollDelta::Lines { x: 0.0, y: 1.0 }));
}

#[test]
fn a_label_takes_the_presses_that_fall_on_its_glyphs_not_those_past_them() {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let font = std::fs::read(path).expect("the test font");
    // The glyphs of "ok" take some 20 of the box's 300 logical pixels: at its start, then its end.
    for (alignment, on, past) in [
        (Alignment::Start, 5.0, 200.0),
        (Alignment::End, 295.0, 100.0),
    ] {
        let mut fonts = Fonts::new();
        fonts.register(font.clone()).expect("a font");
        let mut ui = Ui::new(fonts);
        let root = ui.add_root(
            "main",
            settings(400.0, 300.0, [0.0; 2], false),
            Style::default(),
        );
        let mut style = sized(300.0, 20.0);
        style.text.style.fonts = vec!["Noto Sans".to_owned()];
        style.text.style.alignment = alignment;
        ui.build(root, 0, |children| {
            children.add("label", Element::label("ok", style), |_| {});
        });
        ui.frame();
        for (x, taken) in [(on, true), (past, false)] {
            ui.pointer_moved(x, 10.0);
            let pressed = ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
            ui.pointer_button(MouseButton::Left, ButtonState::Released);
            assert_eq!(pressed, taken, "{alignment:?} at {x}");
        }
    }
}
