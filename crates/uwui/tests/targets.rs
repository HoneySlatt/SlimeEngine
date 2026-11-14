//! Roots on several targets, each with its own input and outputs
//! (`docs/adr/0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md`):
//! two targets with a button at the same place, each target's hit test and frame, a layer
//! over another target's root, a modal root that blocks its own target alone, each target's
//! focus and outputs, the active target, the clipboard across targets, a root moved from one
//! target to another; and each target's own frame and frame's time
//! (`docs/adr/0258-each-target-its-frame-and-its-time-a-change-asks-for-its-targets-frame.md`).

use std::time::Duration;

use slime_input::{ButtonState, CursorShape, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use uwui::action::{Action, ActionKind};
use uwui::display::DisplayList;
use uwui::host::{Answer, Event, Input, InputMethod, TargetId, TextQuery, TextUnit};
use uwui::preferences::Preferences;
use uwui::style::{Dimension, FlexDirection, StyleBuilder};
use uwui::text::Fonts;
use uwui::theme::{StyleSheet, Theme};
use uwui::time::{EventTime, FrameShown, Instant};
use uwui::tree::{
    Along, Element, ElementId, FrameStats, Layer, LayoutDirection, Notification, NotificationKind,
    PaletteCommand, Place, RootSettings, Side, Ui,
};

/// The two targets of the tests, neither of them the main one.
const A: TargetId = TargetId(1);
const B: TargetId = TargetId(2);

/// Writes `width` by 40 logical pixels, of the tests' font.
fn sized(s: StyleBuilder, width: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(40.0))
        .fonts(vec!["Noto Sans".to_owned()])
}

/// A root of 400 by 300 logical pixels on `target`, at scale 1.
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

/// An interface of the tests' Noto Sans without roots.
fn ui() -> Ui {
    let mut fonts = Fonts::new();
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    Ui::new(fonts)
}

/// Draws a frame of each target, the main one, `a` and `b`: each lays out and draws its own roots
/// alone.
fn frame_each(ui: &mut Ui) {
    for target in [TargetId::MAIN, A, B] {
        ui.frame(target);
    }
}

/// What two targets hold: on each, a root whose column holds a button of 100 by 40 logical pixels
/// at its top left corner, then a field of text of 180 by 40 below it.
struct Two {
    ui: Ui,
    roots: [ElementId; 2],
    buttons: [ElementId; 2],
    fields: [ElementId; 2],
}

/// `a`'s root added first, `b`'s over it, laid out.
fn two_targets() -> Two {
    let mut ui = ui();
    let (mut roots, mut buttons, mut fields) = (Vec::new(), Vec::new(), Vec::new());
    for (name, target) in [("a", A), ("b", B)] {
        let root = ui.add_root(name, settings(target), |s| {
            s.direction(FlexDirection::Column)
        });
        ui.build(root, 0, |children| {
            let button = Element::button("go").style(|s| sized(s, 100.0));
            buttons.push(children.add("button", button, |_| {}));
            let field = Element::text_field("").style(|s| sized(s, 180.0));
            fields.push(children.add("field", field, |_| {}));
        });
        roots.push(root);
    }
    frame_each(&mut ui);
    Two {
        ui,
        roots: roots.try_into().expect("two roots"),
        buttons: buttons.try_into().expect("two buttons"),
        fields: fields.try_into().expect("two fields"),
    }
}

/// `input`, come to `target`, at the interface's start.
fn give(ui: &mut Ui, target: TargetId, input: Input<'_>) -> Answer {
    ui.event(Event::new(
        target,
        EventTime::simulated(Instant::ZERO),
        input,
    ))
}

/// The primary button, `state`.
fn primary(state: ButtonState) -> Input<'static> {
    Input::PointerButton {
        button: MouseButton::Left,
        state,
    }
}

/// A press then a release of the primary button at `x` and `y` of `target`: whether the
/// interface took both.
fn click(ui: &mut Ui, target: TargetId, x: f32, y: f32) -> bool {
    let _ = give(ui, target, Input::PointerMoved { x, y });
    let pressed = give(ui, target, primary(ButtonState::Pressed)).taken();
    let released = give(ui, target, primary(ButtonState::Released)).taken();
    pressed && released
}

/// A key's press that types `text`, or a named key's.
fn key(logical: Key, text: Option<&str>) -> KeyEvent {
    KeyEvent {
        physical: KeyCode::Unidentified,
        logical,
        text: text.map(str::to_owned),
        state: ButtonState::Pressed,
        repeat: false,
    }
}

fn press(element: ElementId) -> Action {
    Action {
        element: Some(element),
        kind: ActionKind::Press,
    }
}

#[test]
fn a_press_on_one_target_reaches_its_own_button_at_the_same_place() {
    let Two {
        mut ui, buttons, ..
    } = two_targets();
    let [a, b] = buttons;
    assert_eq!(ui.rect(a), ui.rect(b), "both at the same place");
    // `b`'s root lies over `a`'s: without targets, the top root would take every press.
    assert!(click(&mut ui, A, 50.0, 20.0));
    assert_eq!(ui.take_actions(), [press(a)]);
    assert!(ui.state(a).expect("a").hovered);
    assert!(!ui.state(b).expect("b").hovered, "`b`'s pointer never came");
    assert!(click(&mut ui, B, 50.0, 20.0));
    assert_eq!(ui.take_actions(), [press(b)]);
    assert!(ui.state(a).expect("a").hovered, "`a`'s pointer stays");
    assert!(ui.state(b).expect("b").hovered);
    // Each target's hit test reaches its own roots alone.
    assert_eq!(ui.element_at(A, 50.0, 20.0), Some(a));
    assert_eq!(ui.element_at(B, 50.0, 20.0), Some(b));
    assert_eq!(ui.element_at(TargetId::MAIN, 50.0, 20.0), None);
}

#[test]
fn each_target_has_its_own_display_list() {
    let Two { mut ui, .. } = two_targets();
    let (main, _) = ui.frame(TargetId::MAIN);
    assert!(main.is_empty(), "no root on the main target");
    let lists = [ui.frame(A).0, ui.frame(B).0];
    assert!(!lists[0].is_empty() && !lists[1].is_empty());
    assert_eq!(
        lists[0].len(),
        lists[1].len(),
        "the same roots, each its own"
    );
    // A third root on `b` draws in `b`'s list alone.
    let extra = ui.add_root("extra", settings(B), |s| s);
    ui.build(extra, 0, |children| {
        let label = Element::label("more").style(|s| sized(s, 100.0));
        children.add("label", label, |_| {});
    });
    assert_eq!(ui.frame(A).0.len(), lists[0].len());
    assert!(ui.frame(B).0.len() > lists[1].len());
}

#[test]
fn a_layer_over_another_targets_root_stands_on_that_target() {
    let Two {
        mut ui, buttons, ..
    } = two_targets();
    let [a, b] = buttons;
    let layer = Layer {
        place: Place::Beside {
            element: b,
            side: Side::Below,
            along: Along::Start,
        },
        modal: false,
        dismiss: true,
    };
    let menu = ui.open_layer("menu", layer, |s| s);
    let mut item = None;
    ui.build(menu, 0, |children| {
        let button = Element::button("item").style(|s| sized(s, 100.0));
        item = Some(children.add("item", button, |_| {}));
    });
    let item = item.expect("built");
    let before = ui.frame(A).0.len();
    frame_each(&mut ui);
    // It draws in `b`'s list, below `b`'s button, and not in `a`'s.
    assert_eq!(ui.frame(A).0.len(), before);
    let rect = ui.rect(item).expect("laid out");
    let (x, y) = (rect.x + 10.0, rect.y + 10.0);
    assert_eq!(ui.element_at(B, x, y), Some(item));
    assert_ne!(ui.element_at(A, x, y), Some(item));
    // A press at the same place on `a` reaches `a`'s own and leaves the layer open.
    assert!(click(&mut ui, A, 50.0, 20.0));
    assert_eq!(ui.take_actions(), [press(a)]);
    frame_each(&mut ui);
    assert!(ui.rect(item).is_some(), "still open");
    // On `b`, its item takes the press.
    assert!(click(&mut ui, B, x, y));
    assert_eq!(ui.take_actions(), [press(item)]);
    // A press outside it on `b` closes it.
    let _ = click(&mut ui, B, 390.0, 290.0);
    let actions = ui.take_actions();
    assert!(
        actions
            .iter()
            .any(|action| action.element == Some(menu) && action.kind == ActionKind::Dismissed),
        "{actions:?}"
    );
}

#[test]
fn a_modal_root_blocks_its_own_target_alone() {
    let Two {
        mut ui, buttons, ..
    } = two_targets();
    let [a, b] = buttons;
    let dialog = ui.add_root(
        "dialog",
        RootSettings {
            size: [100.0, 40.0],
            origin: [200.0, 200.0],
            modal: true,
            ..settings(B)
        },
        |s| s,
    );
    ui.build(dialog, 0, |children| {
        let ok = Element::button("ok").style(|s| sized(s, 100.0));
        children.add("ok", ok, |_| {});
    });
    frame_each(&mut ui);
    // `b`'s button below the dialog takes nothing; `a`'s goes on.
    let _ = click(&mut ui, B, 50.0, 20.0);
    assert!(ui.take_actions().is_empty(), "blocked on `b`");
    assert!(click(&mut ui, A, 50.0, 20.0));
    assert_eq!(ui.take_actions(), [press(a)]);
    // An agent's press on `b`'s button is kept out too, on `a`'s goes through.
    ui.act(press(b));
    ui.act(press(a));
    assert_eq!(ui.take_actions(), [press(a)]);
    // The dialog took `b`'s focus to its own; `a`'s focus is its own.
    let in_dialog = ui.focus_in(B).expect("the dialog's button");
    assert_ne!(Some(in_dialog), ui.focus_in(A));
}

#[test]
fn each_target_keeps_its_focus_and_the_keys_reach_their_targets() {
    let Two { mut ui, fields, .. } = two_targets();
    let [a, b] = fields;
    assert!(click(&mut ui, A, 50.0, 60.0));
    assert!(click(&mut ui, B, 50.0, 60.0));
    // Each holds its field; the active target is the last pressed.
    assert_eq!(ui.focus_in(A), Some(a));
    assert_eq!(ui.focus_in(B), Some(b));
    assert_eq!(ui.active_target(), B);
    assert_eq!(ui.focus(), Some(b));
    let x = key(Key::Character("x".to_owned()), Some("x"));
    let y = key(Key::Character("y".to_owned()), Some("y"));
    assert!(give(&mut ui, A, Input::Key(&x)).taken());
    assert!(give(&mut ui, B, Input::Key(&y)).taken());
    assert_eq!(ui.field_text(a), Some("x"));
    assert_eq!(ui.field_text(b), Some("y"));
    // A key makes its target the active one.
    assert_eq!(ui.active_target(), B);
    assert!(give(&mut ui, A, Input::Key(&x)).taken());
    assert_eq!(ui.active_target(), A);
    // Taking the focus without an element takes the active target's alone.
    ui.set_focus(None);
    assert_eq!(ui.focus_in(A), None);
    assert_eq!(ui.focus_in(B), Some(b));
    // Escape and Tab stay on their target: Tab with no focus enters `a`'s roots alone.
    ui.set_tab_enters(true);
    let tab = key(Key::Named(NamedKey::Tab), None);
    assert!(give(&mut ui, A, Input::Key(&tab)).taken());
    let first = ui.focus_in(A).expect("Tab entered `a`");
    assert_eq!(ui.element_at(A, 50.0, 20.0), Some(first));
    assert_eq!(ui.focus_in(B), Some(b), "`b` keeps its own");
}

#[test]
fn the_outputs_are_each_targets_own() {
    let Two { mut ui, fields, .. } = two_targets();
    let _ = ui.outputs(A);
    let _ = ui.outputs(B);
    // Over `b`'s field: `b` shows the I-beam, `a` its arrow.
    let _ = give(&mut ui, B, Input::PointerMoved { x: 50.0, y: 60.0 });
    assert_eq!(ui.outputs(B).cursor, Some(CursorShape::Text));
    assert_eq!(ui.outputs(A).cursor, None);
    // `b`'s field takes the focus: `b`'s input method turns on, `a`'s stays off.
    assert!(click(&mut ui, B, 50.0, 60.0));
    let outputs = ui.outputs(B);
    assert_eq!(outputs.input_method, Some(InputMethod::On));
    assert!(outputs.input_area.is_some());
    assert_eq!(ui.outputs(A).input_method, None);
    // `b`'s field removed: `b` is told once that no field takes text.
    ui.remove(fields[1]);
    assert_eq!(ui.outputs(B).input_method, Some(InputMethod::Off));
    assert_eq!(ui.outputs(B).input_method, None);
    // A target that the interface does not know has none, and takes nothing.
    assert_eq!(
        give(&mut ui, TargetId(9), Input::PointerMoved { x: 1.0, y: 1.0 }),
        Answer::NotTaken
    );
    assert_eq!(ui.outputs(TargetId(9)).cursor, None);
}

#[test]
fn the_questions_and_covers_answer_for_their_own_target() {
    let Two { mut ui, fields, .. } = two_targets();
    assert!(click(&mut ui, B, 50.0, 60.0));
    let hello = key(Key::Character("hello".to_owned()), Some("hello"));
    assert!(give(&mut ui, B, Input::Key(&hello)).taken());
    let query = TextQuery {
        before: 10,
        after: 10,
        unit: TextUnit::Utf8,
        composition: true,
    };
    let around = ui.text_around(B, query).expect("`b`'s field");
    assert_eq!(around.text, "hello");
    assert!(ui.text_around(A, query).is_none(), "`a` holds no focus");
    assert_eq!(ui.field_text(fields[0]), Some(""));
    // Each target covers where its own roots draw.
    assert!(ui.covers(A, 50.0, 20.0));
    assert!(!ui.covers(TargetId(9), 50.0, 20.0));
}

#[test]
fn a_paste_waits_on_its_own_target_and_lands_whatever_target_answers() {
    let Two { mut ui, fields, .. } = two_targets();
    let [a, b] = fields;
    assert!(click(&mut ui, A, 50.0, 60.0));
    assert!(click(&mut ui, B, 50.0, 60.0));
    let paste = key(Key::Character("v".to_owned()), Some("v"));
    let _ = give(&mut ui, B, Input::Modifiers(control()));
    assert!(give(&mut ui, B, Input::Key(&paste)).taken());
    let _ = give(&mut ui, B, Input::Modifiers(Default::default()));
    let reads = ui.outputs(B).clipboard_reads;
    assert_eq!(reads.len(), 1, "asked on `b`");
    assert!(ui.outputs(A).clipboard_reads.is_empty());
    // `a`'s focus leaves its field: `b`'s read waits on.
    assert!(click(&mut ui, A, 50.0, 20.0));
    assert_ne!(ui.focus_in(A), Some(a));
    // The host answers on the main target: `b`'s field pastes.
    let answer = Input::ClipboardText {
        read: reads[0],
        text: Some("pasted"),
    };
    assert_eq!(give(&mut ui, TargetId::MAIN, answer), Answer::Taken);
    assert_eq!(ui.field_text(b), Some("pasted"));
    assert_eq!(ui.field_text(a), Some(""));
    // The program writes to the active target's outputs, where the person last pressed.
    ui.write_clipboard("program");
    assert_eq!(ui.outputs(B).clipboard_write, None);
    assert_eq!(ui.outputs(A).clipboard_write.as_deref(), Some("program"));
}

/// Control held, the primary modifier of the tests' platform on Linux and Windows.
fn control() -> slime_input::Modifiers {
    let mut modifiers = slime_input::Modifiers::default();
    modifiers.control = true;
    modifiers
}

#[test]
fn a_root_moved_to_another_target_lets_go_of_what_the_old_one_held() {
    let Two {
        mut ui,
        roots,
        buttons,
        fields,
    } = two_targets();
    // `b`'s button pressed and held.
    let _ = give(&mut ui, B, Input::PointerMoved { x: 50.0, y: 20.0 });
    assert!(give(&mut ui, B, primary(ButtonState::Pressed)).taken());
    assert!(ui.state(buttons[1]).expect("b").pressed);
    // `b`'s root goes to the main target.
    ui.set_root(roots[1], settings(TargetId::MAIN));
    assert!(!ui.state(buttons[1]).expect("b").pressed, "let go");
    assert!(!ui.state(buttons[1]).expect("b").hovered);
    // Its release on `b` presses nothing.
    let _ = give(&mut ui, B, primary(ButtonState::Released));
    assert!(ui.take_actions().is_empty());
    frame_each(&mut ui);
    // On the main target, it takes the input; `a` keeps its own.
    assert_eq!(ui.element_at(TargetId::MAIN, 50.0, 20.0), Some(buttons[1]));
    assert_eq!(ui.element_at(B, 50.0, 20.0), None);
    assert!(click(&mut ui, TargetId::MAIN, 50.0, 60.0));
    assert_eq!(ui.focus_in(TargetId::MAIN), Some(fields[1]));
    // Moved again with its focused field: the main target's input ends, told once.
    let _ = ui.outputs(TargetId::MAIN);
    let alone = ui.frame(A).0.len();
    ui.set_root(roots[1], settings(A));
    assert_eq!(ui.focus_in(TargetId::MAIN), None);
    assert_eq!(
        ui.outputs(TargetId::MAIN).input_method,
        Some(InputMethod::Off)
    );
    assert!(ui.frame(A).0.len() > alone, "`a` draws it now");
    assert!(ui.frame(TargetId::MAIN).0.is_empty());
}

/// A modal layer at a point over `over`, holding one button: the layer and its button.
fn dialog_over(ui: &mut Ui, over: ElementId) -> (ElementId, ElementId) {
    let layer = Layer {
        place: Place::At {
            over,
            point: [200.0, 200.0],
        },
        modal: true,
        dismiss: false,
    };
    let dialog = ui.open_layer("dialog", layer, |s| s);
    let mut ok = None;
    ui.build(dialog, 0, |children| {
        let button = Element::button("ok").style(|s| sized(s, 100.0));
        ok = Some(children.add("ok", button, |_| {}));
    });
    frame_each(ui);
    (dialog, ok.expect("built"))
}

#[test]
fn a_key_on_one_target_stays_out_of_the_palette_open_on_another() {
    let Two { mut ui, fields, .. } = two_targets();
    // A press on `a` makes it active: the palette opens there, its field taking `a`'s focus.
    assert!(click(&mut ui, A, 50.0, 20.0));
    let _ = ui.take_actions();
    let commands = vec![PaletteCommand {
        name: "Save".to_owned(),
        command: "save".to_owned(),
    }];
    ui.open_palette("Type a command", commands);
    frame_each(&mut ui);
    assert!(ui.palette_open());
    // The person moves to `b`, whose field takes `b`'s focus; `a`'s layers stay.
    assert!(click(&mut ui, B, 50.0, 60.0));
    assert_eq!(ui.focus_in(B), Some(fields[1]));
    assert!(ui.palette_open());
    // Enter on `b` reaches `b`'s field, not `a`'s palette.
    let enter = key(Key::Named(NamedKey::Enter), None);
    let _ = give(&mut ui, B, Input::Key(&enter));
    let actions = ui.take_actions();
    assert!(ui.palette_open(), "{actions:?}");
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action.kind, ActionKind::Command(_))),
        "{actions:?}"
    );
}

#[test]
fn a_root_moved_with_its_modal_layer_leaves_nothing_of_it_in_the_old_targets_focus() {
    let Two {
        mut ui,
        roots,
        fields,
        ..
    } = two_targets();
    // `b`'s field holds `b`'s focus; a modal dialog over `b`'s root takes it.
    assert!(click(&mut ui, B, 50.0, 60.0));
    let _ = ui.outputs(B);
    let (_, ok) = dialog_over(&mut ui, roots[1]);
    assert_eq!(ui.focus_in(B), Some(ok));
    let _ = ui.outputs(B);
    // `b`'s root moves to `a`, its dialog with it.
    ui.set_root(roots[1], settings(A));
    frame_each(&mut ui);
    assert_eq!(ui.focus_in(A), Some(ok), "the dialog took `a`'s focus");
    assert_eq!(ui.focus_in(B), None, "nothing that left holds `b`'s focus");
    let x = key(Key::Character("x".to_owned()), Some("x"));
    let _ = give(&mut ui, B, Input::Key(&x));
    assert_eq!(ui.field_text(fields[1]), Some(""));
    assert_eq!(ui.outputs(B).input_method, None, "no input method on `b`");
}

#[test]
fn a_modal_layer_placed_over_another_targets_root_moves_its_modality_there() {
    let Two {
        mut ui,
        roots,
        buttons,
        ..
    } = two_targets();
    let (dialog, ok) = dialog_over(&mut ui, roots[0]);
    assert_eq!(ui.focus_in(A), Some(ok));
    ui.set_layer_place(
        dialog,
        Place::At {
            over: roots[1],
            point: [200.0, 200.0],
        },
    );
    frame_each(&mut ui);
    let rect = ui.rect(ok).expect("laid out");
    let (x, y) = (rect.x + 10.0, rect.y + 10.0);
    assert_eq!(ui.element_at(B, x, y), Some(ok), "on `b` now");
    assert_ne!(ui.element_at(A, x, y), Some(ok));
    // `a` is free again; `b` is blocked, its focus kept in the dialog.
    ui.act(press(buttons[0]));
    ui.act(press(buttons[1]));
    assert_eq!(ui.take_actions(), [press(buttons[0])]);
    assert_eq!(ui.focus_in(B), Some(ok));
    assert_ne!(ui.focus_in(A), Some(ok));
}

#[test]
fn a_layers_target_follows_the_root_it_opens_over_whatever_set_root_says() {
    let Two { mut ui, roots, .. } = two_targets();
    let (dialog, ok) = dialog_over(&mut ui, roots[1]);
    let rect = ui.rect(ok).expect("laid out");
    let (x, y) = (rect.x + 10.0, rect.y + 10.0);
    let current = ui.root_settings(dialog).expect("a root");
    ui.set_root(
        dialog,
        RootSettings {
            target: A,
            ..current
        },
    );
    assert_eq!(ui.element_at(B, x, y), Some(ok));
    assert_ne!(ui.element_at(A, x, y), Some(ok));
}

#[test]
fn a_modal_root_moved_gives_back_its_focus_and_blocks_its_new_target() {
    let Two {
        mut ui, buttons, ..
    } = two_targets();
    ui.set_focus(Some(buttons[1]));
    let modal = |target| RootSettings {
        size: [100.0, 40.0],
        origin: [200.0, 200.0],
        modal: true,
        ..settings(target)
    };
    let dialog = ui.add_root("dialog", modal(B), |s| s);
    let mut ok = None;
    ui.build(dialog, 0, |children| {
        let button = Element::button("ok").style(|s| sized(s, 100.0));
        ok = Some(children.add("ok", button, |_| {}));
    });
    frame_each(&mut ui);
    let ok = ok.expect("built");
    assert_eq!(ui.focus_in(B), Some(ok));
    ui.set_root(dialog, modal(A));
    assert_eq!(ui.focus_in(B), Some(buttons[1]), "`b`'s focus given back");
    frame_each(&mut ui);
    assert_eq!(ui.focus_in(A), Some(ok), "`a`'s focus kept in the dialog");
    ui.act(press(buttons[1]));
    ui.act(press(buttons[0]));
    assert_eq!(
        ui.take_actions(),
        [press(buttons[1])],
        "`b` free, `a` blocked"
    );
}

#[test]
fn a_root_moved_takes_the_layers_over_it_at_once() {
    let Two {
        mut ui,
        roots,
        buttons,
        ..
    } = two_targets();
    let layer = Layer {
        place: Place::Beside {
            element: buttons[1],
            side: Side::Below,
            along: Along::Start,
        },
        modal: false,
        dismiss: true,
    };
    let menu = ui.open_layer("menu", layer, |s| s);
    let mut item = None;
    ui.build(menu, 0, |children| {
        let button = Element::button("item").style(|s| sized(s, 100.0));
        item = Some(children.add("item", button, |_| {}));
    });
    frame_each(&mut ui);
    let item = item.expect("built");
    let rect = ui.rect(item).expect("laid out");
    let (x, y) = (rect.x + 10.0, rect.y + 10.0);
    assert_eq!(ui.element_at(B, x, y), Some(item));
    ui.set_root(roots[1], settings(A));
    assert_eq!(ui.element_at(A, x, y), Some(item));
    assert_ne!(ui.element_at(B, x, y), Some(item));
}

#[test]
fn a_slider_held_commits_as_its_root_moves() {
    let mut ui = ui();
    let root = ui.add_root("b", settings(B), |s| s.direction(FlexDirection::Column));
    let mut slider = None;
    ui.build(root, 0, |children| {
        let element = Element::slider(0.5, 0.0, 1.0, 0.1).style(|s| sized(s, 216.0));
        slider = Some(children.add("slider", element, |_| {}));
    });
    frame_each(&mut ui);
    let slider = slider.expect("built");
    let _ = give(&mut ui, B, Input::PointerMoved { x: 108.0, y: 10.0 });
    assert!(give(&mut ui, B, primary(ButtonState::Pressed)).taken());
    let _ = ui.take_actions();
    ui.set_root(root, settings(A));
    let actions = ui.take_actions();
    assert!(
        actions.iter().any(|action| action.element == Some(slider)
            && matches!(action.kind, ActionKind::Committed(_))),
        "{actions:?}"
    );
    // Its release on `b`, whose press `b` took, is taken, and changes nothing.
    assert!(give(&mut ui, B, primary(ButtonState::Released)).taken());
    assert!(ui.take_actions().is_empty());
}

#[test]
fn a_root_moved_onto_a_target_with_a_modal_root_stands_below_it() {
    let Two { mut ui, .. } = two_targets();
    let dialog = ui.add_root(
        "dialog",
        RootSettings {
            size: [100.0, 40.0],
            origin: [200.0, 200.0],
            modal: true,
            ..settings(A)
        },
        |s| s,
    );
    ui.build(dialog, 0, |children| {
        let ok = Element::button("ok").style(|s| sized(s, 100.0));
        children.add("ok", ok, |_| {});
    });
    let late = ui.add_root("late", settings(B), |s| s.direction(FlexDirection::Column));
    let mut button = None;
    ui.build(late, 0, |children| {
        let late = Element::button("late").style(|s| sized(s, 100.0));
        button = Some(children.add("go", late, |_| {}));
    });
    frame_each(&mut ui);
    ui.set_root(late, settings(A));
    frame_each(&mut ui);
    let button = button.expect("built");
    assert_ne!(ui.element_at(A, 50.0, 20.0), Some(button));
    ui.act(press(button));
    assert!(ui.take_actions().is_empty(), "kept from the input");
}

#[test]
fn a_known_target_without_roots_takes_a_release_whose_press_it_took_and_its_shortcuts() {
    let Two {
        mut ui,
        roots,
        buttons,
        ..
    } = two_targets();
    ui.set_shortcuts(&[uwui::shortcuts::Shortcuts::from_text(
        "test.keys.kdl",
        "scope global {\n    F2 rename\n}\n",
    )])
    .expect("a table");
    let _ = give(&mut ui, B, Input::PointerMoved { x: 50.0, y: 20.0 });
    assert!(give(&mut ui, B, primary(ButtonState::Pressed)).taken());
    ui.remove(roots[1]);
    assert!(ui.state(buttons[1]).is_none());
    assert!(give(&mut ui, B, primary(ButtonState::Released)).taken());
    let f2 = key(Key::Named(NamedKey::F2), None);
    assert!(give(&mut ui, B, Input::Key(&f2)).taken());
    let commands: Vec<ActionKind> = ui
        .take_actions()
        .into_iter()
        .map(|action| action.kind)
        .collect();
    assert_eq!(commands, [ActionKind::Command("rename".to_owned())]);
}

#[test]
fn each_tree_of_meaning_tells_its_targets_focus_and_the_program_follows_the_active_target() {
    let Two { mut ui, fields, .. } = two_targets();
    assert!(click(&mut ui, A, 50.0, 60.0));
    assert!(click(&mut ui, B, 50.0, 60.0));
    // Each window's tree tells its own focus, whichever target is active.
    for (target, field) in [(A, fields[0]), (B, fields[1])] {
        let reader = ui.add_meaning_reader(target).expect("a reader");
        let changes = ui.take_meaning_changes(reader, true).expect("its changes");
        let number = changes
            .nodes
            .iter()
            .find(|node| node.meaning.id == field)
            .map(|node| node.number);
        assert!(number.is_some());
        assert_eq!(changes.focus, number);
    }
    // The program's focus on `a`'s field does not make `a` active.
    ui.set_focus(Some(fields[0]));
    assert_eq!(ui.active_target(), B);
    assert_eq!(ui.focus(), Some(fields[1]));
    // Escape closes the layer of the key's target alone.
    let layer = |element| Layer {
        place: Place::Beside {
            element,
            side: Side::Below,
            along: Along::Start,
        },
        modal: false,
        dismiss: true,
    };
    let on_a = ui.open_layer("on a", layer(fields[0]), |s| s);
    let on_b = ui.open_layer("on b", layer(fields[1]), |s| s);
    frame_each(&mut ui);
    let escape = key(Key::Named(NamedKey::Escape), None);
    let _ = give(&mut ui, B, Input::Key(&escape));
    let dismissed: Vec<Option<ElementId>> = ui
        .take_actions()
        .into_iter()
        .filter(|action| action.kind == ActionKind::Dismissed)
        .map(|action| action.element)
        .collect();
    assert_eq!(dismissed, [Some(on_b)]);
    assert!(ui.root_settings(on_a).is_some(), "`a`'s layer stays open");
}

// Each target's own frame and time
// (`docs/adr/0258-each-target-its-frame-and-its-time-a-change-asks-for-its-targets-frame.md`).

/// Buttons black, white under the pointer, over 200 ms of linear motion.
const BUTTONS: &str = r##"
button {
    select kind=Button
    paint background="#000000"
    transition duration=(s)0.2 easing=Linear
}

button-hovered {
    select kind=Button { states Hovered; }
    paint background="#ffffff"
}
"##;

/// `ms` milliseconds after the clock's zero.
fn ms(ms: u64) -> Instant {
    Instant::ZERO + Duration::from_millis(ms)
}

/// Gives `target`'s next frame the time `at`, then draws it.
fn frame_at(ui: &mut Ui, target: TargetId, at: u64) -> (DisplayList, FrameStats) {
    ui.set_frame_shown(target, FrameShown::simulated(ms(at)));
    ui.frame(target)
}

/// The red of `id`'s fill, as it shows: 0 black, 1 white.
fn red(ui: &Ui, id: ElementId) -> f32 {
    ui.style(id).expect("styled").paint.background.r
}

/// Styles `ui` with [`BUTTONS`], each target's style shown at once, without a transition.
fn style_buttons(ui: &mut Ui) {
    let less = Preferences {
        reduced_motion: true,
        ..Preferences::default()
    };
    ui.set_preferences(less);
    ui.set_style(
        &[StyleSheet::from_text("buttons.style", BUTTONS)],
        &[Theme::from_text("empty.theme", "")],
    )
    .expect("the style sheet");
    frame_each(ui);
    ui.set_preferences(Preferences::default());
    frame_each(ui);
}

/// The pointer moving onto `b`'s button at `at`.
fn hover_b(ui: &mut Ui, at: u64) {
    let _ = ui.event(Event::new(
        B,
        EventTime::simulated(ms(at)),
        Input::PointerMoved { x: 50.0, y: 20.0 },
    ));
}

#[test]
fn a_frame_of_one_target_leaves_anothers_changes_for_its_own_frame() {
    let Two { mut ui, roots, .. } = two_targets();
    for target in [TargetId::MAIN, A, B] {
        assert!(!ui.needs_frame(target), "{target:?} drew everything");
    }
    // A build on `b` asks for `b`'s frame alone.
    ui.build(roots[1], 1, |children| {
        let label = Element::label("new").style(|s| sized(s, 100.0));
        children.add("label", label, |_| {});
    });
    assert!(ui.needs_frame(B));
    assert!(!ui.needs_frame(A));
    let (_, stats) = ui.frame(A);
    assert_eq!((stats.styled, stats.layouts, stats.painted), (0, 0, 0));
    assert!(ui.needs_frame(B), "`a`'s frame leaves `b`'s change");
    let (_, stats) = ui.frame(B);
    assert!(stats.styled > 0 && stats.layouts == 1 && stats.painted > 0);
    assert!(!ui.needs_frame(B));
    // A frame of a target that the interface does not know draws nothing and leaves the others.
    ui.build(roots[1], 2, |_| {});
    let (list, _) = ui.frame(TargetId(9));
    assert!(list.is_empty());
    assert!(ui.needs_frame(B));
}

#[test]
fn each_kind_of_change_on_a_target_asks_for_its_frame_alone() {
    let Two { mut ui, fields, .. } = two_targets();
    let changed_b_alone = |ui: &mut Ui, what: &str| {
        assert!(ui.needs_frame(B), "{what} asks for `b`'s frame");
        assert!(!ui.needs_frame(A), "{what} asks nothing of `a`");
        frame_each(ui);
    };
    // A caret moved, which only draws again.
    assert!(click(&mut ui, B, 50.0, 60.0));
    let typed = key(Key::Character("a".into()), Some("a"));
    let _ = give(&mut ui, B, Input::Key(&typed));
    frame_each(&mut ui);
    assert_eq!(ui.focus_in(B), Some(fields[1]));
    let left = key(Key::Named(NamedKey::ArrowLeft), None);
    let _ = give(&mut ui, B, Input::Key(&left));
    changed_b_alone(&mut ui, "a caret moved");
    // A root added, which no builder filled yet.
    let extra = ui.add_root("extra", settings(B), |s| s);
    changed_b_alone(&mut ui, "a root added");
    // A bar's share, which changes what it draws and not its box.
    let mut scroll = None;
    let mut bar = |ui: &mut Ui, revision: u64, share: f32| {
        ui.build(extra, revision, |children| {
            let waiting = Element::progress(Some(share)).style(|s| sized(s, 100.0));
            children.add("bar", waiting, |_| {});
            let boxed = Element::scroll().style(|s| {
                s.direction(FlexDirection::Column)
                    .height(Dimension::Pixels(100.0))
            });
            scroll = Some(children.add("scroll", boxed, |children| {
                for index in 0..5_usize {
                    let item = Element::container().style(|s| sized(s, 100.0).shrink(0.0));
                    children.add(index, item, |_| {});
                }
            }));
        });
    };
    bar(&mut ui, 0, 0.2);
    frame_each(&mut ui);
    bar(&mut ui, 1, 0.6);
    changed_b_alone(&mut ui, "a bar's share");
    // A scroll.
    ui.scroll_to(scroll.expect("built"), [0.0, 30.0]);
    changed_b_alone(&mut ui, "a scroll");
}

#[test]
fn a_panel_raised_on_a_target_asks_for_its_frame() {
    let Two { mut ui, roots, .. } = two_targets();
    let panel = |ui: &mut Ui, origin: [f32; 2]| {
        let place = Place::Floating {
            over: roots[1],
            origin,
            size: [100.0, 100.0],
        };
        let layer = ui.open_layer(
            "panel",
            Layer {
                place,
                modal: false,
                dismiss: false,
            },
            |s| s,
        );
        ui.build(layer, 0, |children| {
            let boxed = Element::container().style(|s| sized(s, 100.0));
            children.add("box", boxed, |_| {});
        });
        layer
    };
    panel(&mut ui, [200.0, 100.0]);
    panel(&mut ui, [250.0, 150.0]);
    frame_each(&mut ui);
    // A press on the first panel's box, which holds no state, raises it over the second.
    let _ = give(&mut ui, B, Input::PointerMoved { x: 210.0, y: 110.0 });
    frame_each(&mut ui);
    let _ = give(&mut ui, B, primary(ButtonState::Pressed));
    assert!(ui.needs_frame(B));
    assert!(!ui.needs_frame(A));
}

#[test]
fn a_root_moved_or_removed_asks_for_the_frames_of_the_targets_it_left_and_joined() {
    let Two { mut ui, roots, .. } = two_targets();
    ui.set_root(roots[1], settings(A));
    assert!(ui.needs_frame(A), "`a` draws it now");
    assert!(ui.needs_frame(B), "`b` draws it no more");
    assert!(ui.frame(B).0.is_empty());
    frame_each(&mut ui);
    ui.remove(roots[0]);
    assert!(ui.needs_frame(A), "`a` draws one root fewer");
    assert!(!ui.needs_frame(B));
}

#[test]
fn two_targets_at_two_frame_times_step_their_motions_each_by_its_own() {
    let Two {
        mut ui, buttons, ..
    } = two_targets();
    style_buttons(&mut ui);
    hover_b(&mut ui, 10);
    // `a`'s frame, shown later, leaves `b`'s button as it was.
    frame_at(&mut ui, A, 100);
    assert_eq!(red(&ui, buttons[1]), 0.0);
    // `b`'s frames start the hover's motion at `b`'s time, and step it by `b`'s.
    let (_, stats) = frame_at(&mut ui, B, 12);
    assert_eq!(red(&ui, buttons[1]), 0.0);
    assert!(stats.animating && ui.animating(B) && !ui.animating(A));
    frame_at(&mut ui, B, 14);
    assert!((red(&ui, buttons[1]) - 0.01).abs() < 1e-4, "2 ms of 200");
    // `a`'s frame, later again, moves nothing of `b`'s.
    frame_at(&mut ui, A, 116);
    assert!((red(&ui, buttons[1]) - 0.01).abs() < 1e-4);
    frame_at(&mut ui, B, 112);
    assert!((red(&ui, buttons[1]) - 0.5).abs() < 1e-4, "100 ms of 200");
}

#[test]
fn a_targets_time_given_before_its_first_root_is_kept() {
    let mut ui = ui();
    ui.set_frame_shown(TargetId::MAIN, FrameShown::simulated(ms(100)));
    ui.set_frame_shown(B, FrameShown::simulated(ms(12)));
    let root = ui.add_root("b", settings(B), |s| s);
    let mut button = None;
    ui.build(root, 0, |children| {
        let go = Element::button("go").style(|s| sized(s, 100.0));
        button = Some(children.add("button", go, |_| {}));
    });
    let button = button.expect("built");
    style_buttons(&mut ui);
    hover_b(&mut ui, 12);
    frame_at(&mut ui, B, 12);
    frame_at(&mut ui, B, 14);
    assert!((red(&ui, button) - 0.01).abs() < 1e-4, "2 ms of 200");
}

#[test]
fn a_target_given_no_time_reads_the_interfaces() {
    let Two {
        mut ui, buttons, ..
    } = two_targets();
    style_buttons(&mut ui);
    hover_b(&mut ui, 0);
    ui.set_frame_shown(TargetId::MAIN, FrameShown::simulated(ms(10)));
    ui.frame(B);
    ui.set_frame_shown(TargetId::MAIN, FrameShown::simulated(ms(110)));
    ui.frame(B);
    assert!((red(&ui, buttons[1]) - 0.5).abs() < 1e-4, "100 ms of 200");
}

#[test]
fn a_targets_time_never_goes_back_and_its_frame_tells_the_time_given() {
    let Two {
        mut ui, buttons, ..
    } = two_targets();
    style_buttons(&mut ui);
    hover_b(&mut ui, 0);
    frame_at(&mut ui, B, 50);
    let (_, stats) = frame_at(&mut ui, B, 40);
    assert_eq!(stats.shown, FrameShown::simulated(ms(40)));
    assert_eq!(red(&ui, buttons[1]), 0.0, "still at 50 ms");
    frame_at(&mut ui, B, 60);
    assert!((red(&ui, buttons[1]) - 0.05).abs() < 1e-4, "10 ms of 200");
}

#[test]
fn a_root_moved_to_another_target_ends_its_transitions() {
    let Two {
        mut ui,
        roots,
        buttons,
        ..
    } = two_targets();
    style_buttons(&mut ui);
    hover_b(&mut ui, 0);
    frame_at(&mut ui, B, 12);
    frame_at(&mut ui, B, 112);
    assert!((red(&ui, buttons[1]) - 0.5).abs() < 1e-4);
    // Placed on `b`'s time, which `a`'s frames do not read, the motion ends where it went.
    ui.set_root(roots[1], settings(A));
    assert_eq!(red(&ui, buttons[1]), 1.0);
}

#[test]
fn each_target_rests_and_spins_on_its_own_frames() {
    let mut ui = ui();
    let mut on = Vec::new();
    for (name, target) in [("a", A), ("b", B)] {
        let root = ui.add_root(name, settings(target), |s| s);
        ui.build(root, 0, |children| {
            let go = Element::button("go")
                .tooltip("Go")
                .style(|s| sized(s, 100.0));
            children.add("button", go, |_| {});
            let waiting = Element::progress(None).style(|s| sized(s, 100.0));
            on.push(children.add("waiting", waiting, |_| {}));
        });
    }
    frame_at(&mut ui, A, 0);
    let (shown, _) = frame_at(&mut ui, B, 0);
    // `b`'s waiting bar spins on `b`'s frames: `a`'s frame does not draw it.
    ui.remove(on[0]);
    let (_, stats) = frame_at(&mut ui, A, 0);
    assert!(!stats.animating && !ui.animating(A));
    assert!(ui.animating(B) && ui.needs_frame(B));
    // `b`'s pointer rests on its button: `b`'s tooltip is due, `a` has nothing due.
    hover_b(&mut ui, 0);
    assert_eq!(ui.next_frame_due(B), Some(ms(500)));
    assert_eq!(ui.next_frame_due(A), None);
    frame_at(&mut ui, A, 600);
    assert_eq!(
        ui.next_frame_due(B),
        Some(ms(600)),
        "`a`'s frame shows no tooltip of `b`'s"
    );
    let (tipped, _) = frame_at(&mut ui, B, 600);
    assert!(tipped.len() > shown.len(), "`b`'s frame shows its tooltip");
    assert_eq!(ui.next_frame_due(B), None);
}

#[test]
fn notifications_settle_in_the_main_targets_frame_with_or_without_its_roots() {
    let mut ui = ui();
    ui.notify(Notification {
        kind: NotificationKind::Information,
        text: "Saved".to_owned(),
        ..Notification::default()
    });
    assert_eq!(ui.next_frame_due(TargetId::MAIN), Some(ms(5_000)));
    assert_eq!(
        ui.next_frame_due(A),
        None,
        "the notifications stand on the main target"
    );
    frame_at(&mut ui, TargetId::MAIN, 6_000);
    assert_eq!(ui.next_frame_due(TargetId::MAIN), None, "its time ended");
}

#[test]
fn a_notification_given_or_closed_asks_for_the_main_targets_frame() {
    let mut ui = ui();
    ui.add_root("main", settings(TargetId::MAIN), |s| s);
    ui.frame(TargetId::MAIN);
    assert!(!ui.needs_frame(TargetId::MAIN), "drew everything");
    let error = ui.notify(Notification {
        kind: NotificationKind::Error,
        text: "A texture is missing".to_owned(),
        ..Notification::default()
    });
    assert_eq!(
        ui.next_frame_due(TargetId::MAIN),
        None,
        "an error waits for no time"
    );
    assert!(
        ui.needs_frame(TargetId::MAIN),
        "the error waits for the main target's frame"
    );
    ui.frame(TargetId::MAIN);
    assert!(!ui.needs_frame(TargetId::MAIN));
    assert!(ui.close_notification(error));
    assert!(
        ui.needs_frame(TargetId::MAIN),
        "the closed error shows until that frame"
    );
}

#[test]
fn less_motion_asked_during_a_transition_asks_for_its_targets_frame() {
    let Two {
        mut ui, buttons, ..
    } = two_targets();
    style_buttons(&mut ui);
    hover_b(&mut ui, 10);
    frame_at(&mut ui, B, 12);
    frame_at(&mut ui, B, 112);
    assert!(
        (red(&ui, buttons[1]) - 0.5).abs() < 1e-4,
        "half way, as drawn"
    );
    ui.set_preferences(Preferences {
        reduced_motion: true,
        ..Preferences::default()
    });
    assert_eq!(red(&ui, buttons[1]), 1.0, "ended at once");
    assert!(ui.needs_frame(B), "its end waits for `b`'s frame");
}
