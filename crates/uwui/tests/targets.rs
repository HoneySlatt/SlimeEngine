//! Roots on several targets, each with its own input and outputs
//! (`docs/adr/0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md`):
//! two targets with a button at the same place, each target's hit test and display list, a layer
//! over another target's root, a modal root that blocks its own target alone, each target's
//! focus and outputs, the active target, the clipboard across targets, and a root moved from one
//! target to another.

use slime_input::{ButtonState, CursorShape, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use uwui::action::{Action, ActionKind};
use uwui::host::{Answer, Event, Input, InputMethod, TargetId, TextQuery, TextUnit};
use uwui::style::{Dimension, FlexDirection, StyleBuilder};
use uwui::text::Fonts;
use uwui::time::{EventTime, Instant};
use uwui::tree::{
    Along, Element, ElementId, Layer, LayoutDirection, PaletteCommand, Place, RootSettings, Side,
    Ui,
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
    ui.frame();
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
    let (main, _) = ui.frame();
    assert!(main.is_empty(), "no root on the main target");
    let lists = [ui.display_list(A), ui.display_list(B)];
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
    ui.frame();
    assert_eq!(ui.display_list(A).len(), lists[0].len());
    assert!(ui.display_list(B).len() > lists[1].len());
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
    let before = ui.display_list(A).len();
    ui.frame();
    // It draws in `b`'s list, below `b`'s button, and not in `a`'s.
    assert_eq!(ui.display_list(A).len(), before);
    let rect = ui.rect(item).expect("laid out");
    let (x, y) = (rect.x + 10.0, rect.y + 10.0);
    assert_eq!(ui.element_at(B, x, y), Some(item));
    assert_ne!(ui.element_at(A, x, y), Some(item));
    // A press at the same place on `a` reaches `a`'s own and leaves the layer open.
    assert!(click(&mut ui, A, 50.0, 20.0));
    assert_eq!(ui.take_actions(), [press(a)]);
    ui.frame();
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
    ui.frame();
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
    ui.frame();
    // On the main target, it takes the input; `a` keeps its own.
    assert_eq!(ui.element_at(TargetId::MAIN, 50.0, 20.0), Some(buttons[1]));
    assert_eq!(ui.element_at(B, 50.0, 20.0), None);
    assert!(click(&mut ui, TargetId::MAIN, 50.0, 60.0));
    assert_eq!(ui.focus_in(TargetId::MAIN), Some(fields[1]));
    // Moved again with its focused field: the main target's input ends, told once.
    let _ = ui.outputs(TargetId::MAIN);
    let alone = ui.display_list(A).len();
    ui.set_root(roots[1], settings(A));
    assert_eq!(ui.focus_in(TargetId::MAIN), None);
    assert_eq!(
        ui.outputs(TargetId::MAIN).input_method,
        Some(InputMethod::Off)
    );
    ui.frame();
    assert!(ui.display_list(A).len() > alone, "`a` draws it now");
    assert!(ui.display_list(TargetId::MAIN).is_empty());
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
    ui.frame();
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
    ui.frame();
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
    ui.frame();
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
    ui.frame();
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
    ui.frame();
    let ok = ok.expect("built");
    assert_eq!(ui.focus_in(B), Some(ok));
    ui.set_root(dialog, modal(A));
    assert_eq!(ui.focus_in(B), Some(buttons[1]), "`b`'s focus given back");
    ui.frame();
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
    ui.frame();
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
    ui.frame();
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
    ui.frame();
    ui.set_root(late, settings(A));
    ui.frame();
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
fn the_meaning_and_the_program_follow_the_active_target() {
    let Two { mut ui, fields, .. } = two_targets();
    assert!(click(&mut ui, A, 50.0, 60.0));
    assert!(click(&mut ui, B, 50.0, 60.0));
    let number = |ui: &mut Ui, id: ElementId| {
        ui.take_meaning_changes(true)
            .nodes
            .iter()
            .find(|node| node.meaning.id == id)
            .map(|node| node.number)
    };
    let b = number(&mut ui, fields[1]);
    assert_eq!(ui.take_meaning_changes(true).focus, b);
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
    ui.frame();
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
