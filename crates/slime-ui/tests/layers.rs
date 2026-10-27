//! Layers, without a GPU (`docs/adr/0150-layers.md`): roots that the framework places beside an
//! element, turned and moved to stay within the window, or at a point; closed by a press outside
//! them and outside what opened them, or by Escape, the program told and the focus given back; the
//! layers that stand over what a layer holds closed with it; following their anchor as it moves.

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::action::ActionKind;
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{
    Along, Element, ElementId, Layer, LayoutDirection, Place, RootSettings, Side, Ui,
};

/// An interface of 400 by 300 logical pixels at scale 1, laid out in `direction`, its root a column
/// holding a button of 100 by 30 at `top` from its top: the interface, its root and the button.
fn interface(direction: LayoutDirection, top: f32) -> (Ui, ElementId, ElementId) {
    let mut ui = Ui::new(Fonts::new());
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction,
        origin: [0.0; 2],
        modal: false,
    };
    let mut column = Style::default();
    column.layout.direction = FlexDirection::Column;
    column.layout.padding.top = top;
    let root = ui.add_root(settings, column);
    let mut button = None;
    ui.build(root, 0, |children| {
        button = Some(children.add("open", Element::button("", sized(100.0, 30.0)), |_| {}));
    });
    ui.frame();
    (ui, root, button.expect("built"))
}

/// A style of `width` by `height` logical pixels, which does not shrink.
fn sized(width: f32, height: f32) -> Style {
    let mut style = Style::default();
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
    style.layout.shrink = 0.0;
    style
}

/// Opens a layer of `place` that closes as the user leaves it, holding a button of 80 by 120.
fn open(ui: &mut Ui, place: Place) -> (ElementId, ElementId) {
    let layer = ui.open_layer(
        Layer {
            place,
            modal: false,
            dismiss: true,
        },
        Style::default(),
    );
    let mut item = None;
    ui.build(layer, 0, |children| {
        item = Some(children.add("item", Element::button("", sized(80.0, 120.0)), |_| {}));
    });
    ui.frame();
    (layer, item.expect("built"))
}

/// A press and a release of the pointer's first button at `x` and `y`.
fn click(ui: &mut Ui, x: f32, y: f32) {
    ui.pointer_moved(x, y);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
}

/// A press of Escape.
fn escape() -> KeyEvent {
    KeyEvent {
        physical: KeyCode::Escape,
        logical: Key::Named(NamedKey::Escape),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    }
}

#[test]
fn a_layer_stands_beside_its_element_and_turns_or_moves_to_stay_in_the_window() {
    // Below the button, lined up with its start, four pixels apart.
    let (mut ui, _, button) = interface(LayoutDirection::LeftToRight, 0.0);
    let below = Place::Beside {
        element: button,
        side: Side::Below,
        along: Along::Start,
    };
    let (layer, _) = open(&mut ui, below);
    let rect = ui.rect(layer).expect("placed");
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (0.0, 34.0, 80.0, 120.0)
    );
    ui.remove(layer);
    // Near the bottom, it turns above the button.
    let (mut ui, _, button) = interface(LayoutDirection::LeftToRight, 250.0);
    let (layer, _) = open(
        &mut ui,
        Place::Beside {
            element: button,
            side: Side::Below,
            along: Along::Start,
        },
    );
    let rect = ui.rect(layer).expect("placed");
    assert_eq!(rect.y, 250.0 - 4.0 - 120.0);
    // From right to left, its start is the button's right edge; at the end, it keeps in the window.
    let (mut ui, _, button) = interface(LayoutDirection::RightToLeft, 0.0);
    let (layer, _) = open(
        &mut ui,
        Place::Beside {
            element: button,
            side: Side::Below,
            along: Along::Start,
        },
    );
    let rect = ui.rect(layer).expect("placed");
    assert_eq!((rect.x, rect.y), (400.0 - 80.0, 34.0));
    let (mut ui, _, button) = interface(LayoutDirection::LeftToRight, 0.0);
    let (layer, _) = open(
        &mut ui,
        Place::Beside {
            element: button,
            side: Side::Start,
            along: Along::Start,
        },
    );
    // No room before the button: after it, then.
    let rect = ui.rect(layer).expect("placed");
    assert_eq!((rect.x, rect.y), (104.0, 0.0));
}

#[test]
fn a_layer_at_a_point_turns_where_it_lacks_room() {
    let (mut ui, root, _) = interface(LayoutDirection::LeftToRight, 0.0);
    let (layer, _) = open(
        &mut ui,
        Place::At {
            over: root,
            point: [380.0, 250.0],
        },
    );
    let rect = ui.rect(layer).expect("placed");
    assert_eq!((rect.x, rect.y), (300.0, 130.0));
}

#[test]
fn a_press_outside_closes_it_and_tells_the_program_but_one_on_its_opener_does_not() {
    let (mut ui, _, button) = interface(LayoutDirection::LeftToRight, 0.0);
    let place = Place::Beside {
        element: button,
        side: Side::Below,
        along: Along::Start,
    };
    let (layer, _) = open(&mut ui, place);
    // On the button that opened it: it stays, the button pressed.
    click(&mut ui, 50.0, 15.0);
    assert!(ui.rect(layer).is_some());
    let kinds: Vec<ActionKind> = ui.take_actions().into_iter().map(|a| a.kind).collect();
    assert_eq!(kinds, [ActionKind::Press]);
    // Inside it: it stays.
    click(&mut ui, 40.0, 100.0);
    assert!(ui.rect(layer).is_some());
    ui.take_actions();
    // Outside both: it closes, and the program reads that it did.
    click(&mut ui, 300.0, 250.0);
    assert!(ui.rect(layer).is_none());
    let actions = ui.take_actions();
    assert_eq!(actions.len(), 1);
    assert_eq!(
        (actions[0].element, &actions[0].kind),
        (Some(layer), &ActionKind::Dismissed)
    );
}

#[test]
fn escape_closes_the_top_layer_and_gives_the_focus_back() {
    let (mut ui, _, button) = interface(LayoutDirection::LeftToRight, 0.0);
    ui.set_focus(Some(button));
    let (layer, item) = open(
        &mut ui,
        Place::Beside {
            element: button,
            side: Side::Below,
            along: Along::Start,
        },
    );
    // Opened while the keyboard last acted: its first element holds the focus.
    ui.frame();
    assert_eq!(ui.focus(), Some(item));
    // A layer over what it holds, then Escape twice: the top first, then the first.
    let (above, _) = open(
        &mut ui,
        Place::Beside {
            element: item,
            side: Side::End,
            along: Along::Start,
        },
    );
    assert!(ui.key(&escape()));
    ui.frame();
    assert!(ui.rect(above).is_none() && ui.rect(layer).is_some());
    assert!(ui.key(&escape()));
    ui.frame();
    assert!(ui.rect(layer).is_none());
    assert_eq!(ui.focus(), Some(button), "back to what held it");
}

#[test]
fn a_layer_closes_with_its_anchor_and_takes_the_layers_over_it_along() {
    let (mut ui, root, button) = interface(LayoutDirection::LeftToRight, 0.0);
    let (layer, item) = open(
        &mut ui,
        Place::Beside {
            element: button,
            side: Side::Below,
            along: Along::Start,
        },
    );
    let (above, _) = open(
        &mut ui,
        Place::Beside {
            element: item,
            side: Side::End,
            along: Along::Start,
        },
    );
    // The program closes the first: the one over it goes too, without telling.
    ui.remove(layer);
    ui.frame();
    assert!(ui.rect(above).is_none());
    assert!(ui.take_actions().is_empty());
    // An anchor that goes closes its layer, which the program reads.
    let (layer, _) = open(
        &mut ui,
        Place::Beside {
            element: button,
            side: Side::Below,
            along: Along::Start,
        },
    );
    ui.build(root, 1, |_| {});
    ui.frame();
    assert!(ui.rect(layer).is_none());
    let kinds: Vec<ActionKind> = ui.take_actions().into_iter().map(|a| a.kind).collect();
    assert_eq!(kinds, [ActionKind::Dismissed]);
}

#[test]
fn a_layer_follows_its_anchor_as_it_moves() {
    let (mut ui, root, button) = interface(LayoutDirection::LeftToRight, 0.0);
    let (layer, _) = open(
        &mut ui,
        Place::Beside {
            element: button,
            side: Side::Below,
            along: Along::Start,
        },
    );
    // An element built before the button pushes it down by 50: the layer comes along in the same
    // frame.
    ui.build(root, 2, |children| {
        children.add("space", Element::container(sized(100.0, 50.0)), |_| {});
        children.add("open", Element::button("", sized(100.0, 30.0)), |_| {});
    });
    ui.frame();
    let button_rect = ui.rect(button).expect("laid out");
    let rect = ui.rect(layer).expect("placed");
    assert_eq!(rect.y, button_rect.y + button_rect.height + 4.0);
}

#[test]
fn the_second_button_or_shift_f10_asks_for_a_context_menu() {
    let (mut ui, _, button) = interface(LayoutDirection::LeftToRight, 0.0);
    ui.pointer_moved(30.0, 10.0);
    assert!(ui.pointer_button(MouseButton::Right, ButtonState::Pressed));
    ui.pointer_button(MouseButton::Right, ButtonState::Released);
    let actions = ui.take_actions();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].element, Some(button));
    assert_eq!(
        actions[0].kind,
        ActionKind::ContextMenu { at: [30.0, 10.0] }
    );
    // Nothing drawn there: the game's.
    ui.pointer_moved(300.0, 250.0);
    assert!(!ui.pointer_button(MouseButton::Right, ButtonState::Pressed));
    // Shift+F10 with the button focused: at its bottom start corner.
    ui.set_focus(Some(button));
    let mut modifiers = slime_input::Modifiers::default();
    modifiers.shift = true;
    ui.modifiers(modifiers);
    let f10 = KeyEvent {
        physical: KeyCode::F10,
        logical: Key::Named(NamedKey::F10),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    };
    assert!(ui.key(&f10));
    let actions = ui.take_actions();
    assert_eq!(actions[0].kind, ActionKind::ContextMenu { at: [0.0, 30.0] });
}

#[test]
fn a_layer_beside_an_element_of_another_layer_stands_out_of_that_layer() {
    let (mut ui, _, button) = interface(LayoutDirection::LeftToRight, 0.0);
    let (layer, item) = open(
        &mut ui,
        Place::Beside {
            element: button,
            side: Side::Below,
            along: Along::Start,
        },
    );
    let (above, _) = open(
        &mut ui,
        Place::Beside {
            element: item,
            side: Side::End,
            along: Along::Start,
        },
    );
    // After the first layer's item, out of the first layer's box, within the window.
    let (first, second) = (
        ui.rect(layer).expect("placed"),
        ui.rect(above).expect("placed"),
    );
    assert_eq!((second.x, second.y), (first.x + 80.0 + 4.0, first.y));
}

#[test]
fn a_modal_layer_keeps_the_input_and_the_focus_until_it_closes() {
    let (mut ui, root, button) = interface(LayoutDirection::LeftToRight, 0.0);
    let dialog = ui.open_layer(
        Layer {
            place: Place::Floating {
                over: root,
                origin: [150.0, 100.0],
                size: [200.0, 120.0],
            },
            modal: true,
            dismiss: true,
        },
        Style::default(),
    );
    let mut ok = None;
    ui.build(dialog, 0, |children| {
        ok = Some(children.add("ok", Element::button("", sized(80.0, 30.0)), |_| {}));
    });
    ui.frame();
    let ok = ok.expect("built");
    // A press below it reaches nothing there, and does not close it; the focus stays within it.
    click(&mut ui, 50.0, 15.0);
    assert!(ui.rect(dialog).is_some());
    assert!(ui.take_actions().is_empty());
    ui.set_focus(Some(button));
    assert_ne!(ui.focus(), Some(button));
    ui.set_focus(Some(ok));
    assert_eq!(ui.focus(), Some(ok));
    // Escape closes it, which the program reads.
    assert!(ui.key(&escape()));
    ui.frame();
    assert!(ui.rect(dialog).is_none());
    let kinds: Vec<ActionKind> = ui.take_actions().into_iter().map(|a| a.kind).collect();
    assert_eq!(kinds, [ActionKind::Dismissed]);
    ui.set_focus(Some(button));
    assert_eq!(ui.focus(), Some(button), "the window takes the focus again");
}
