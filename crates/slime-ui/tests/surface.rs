//! A surface, without a GPU (`docs/adr/0158-the-viewport.md`): the pointer's presses, moves, wheel
//! and releases over it given to the program, a button pressed over it holding the pointer until
//! it is let go, the keys without Control, Alt or Super reaching the program meanwhile, typing in no
//! field and running no shortcut, and its image drawn into its box; and the chords of the mouse's
//! buttons that a program reads from a table of shortcuts, with more modifiers held, the most named
//! winning (`docs/adr/0160-two-viewports-and-blenders-way.md`).

use slime_input::{
    ButtonState, Key, KeyCode, KeyEvent, Modifiers, MouseButton, NamedKey, ScrollDelta,
};
use slime_ui::action::{ActionKind, SurfaceInput};
use slime_ui::display::{ImageKey, Primitive};
use slime_ui::shortcuts::Shortcuts;
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{Element, ElementId, ItemMark, LayoutDirection, RootSettings, Ui};

/// An interface of 400 by 300 logical pixels at scale 2, a column holding a field of 400 by 40,
/// then a surface that fills the rest, with the shortcuts T and Ctrl+S: the interface, the field
/// and the surface.
fn interface() -> (Ui, ElementId, ElementId) {
    let mut ui = Ui::new(Fonts::new());
    let table = "[global]\nT = \"theme\"\n\"Ctrl+S\" = \"save\"\n";
    ui.set_shortcuts(&[Shortcuts::from_text("keys", table)])
        .expect("a table");
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 2.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let mut column = Style::default();
    column.layout.direction = FlexDirection::Column;
    let root = ui.add_root("main", settings, column);
    let (mut field, mut surface) = (None, None);
    ui.build(root, 0, |children| {
        let mut sized = Style::default();
        sized.layout.size = [Dimension::Pixels(400.0), Dimension::Pixels(40.0)];
        field = Some(children.add("field", Element::text_field("", sized), |_| {}));
        let mut fill = Style::default();
        fill.layout.grow = 1.0;
        surface = Some(children.add("view", Element::surface(ImageKey(7), fill), |_| {}));
    });
    ui.frame();
    (ui, field.expect("built"), surface.expect("built"))
}

/// The actions of a surface that the interface queued since they were last taken.
fn surface_inputs(ui: &mut Ui) -> Vec<SurfaceInput> {
    ui.take_actions()
        .into_iter()
        .filter_map(|action| match action.kind {
            ActionKind::Surface(input) => Some(input),
            _ => None,
        })
        .collect()
}

/// A press of the key that types `text`.
fn typed(text: &str) -> KeyEvent {
    KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Character(text.to_owned()),
        text: Some(text.to_owned()),
        state: ButtonState::Pressed,
        repeat: false,
    }
}

#[test]
fn a_surface_draws_its_image_into_its_box() {
    let (mut ui, _, surface) = interface();
    let rect = ui.rect(surface).expect("laid out");
    // Below the field, filling the rest, in pixels of the screen at scale 2.
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (0.0, 80.0, 800.0, 520.0)
    );
    let (list, _) = ui.frame();
    let drawn = list
        .items()
        .iter()
        .any(|item| matches!(&item.primitive, Primitive::Image(image) if image.image == ImageKey(7) && image.rect == rect));
    assert!(drawn);
}

#[test]
fn a_button_pressed_over_a_surface_holds_the_pointer_until_it_is_let_go() {
    let (mut ui, field, _) = interface();
    ui.set_focus(Some(field));
    // The right button over the surface, 10 and 20 logical pixels from its corner.
    ui.pointer_moved(20.0, 120.0);
    assert!(ui.pointer_button(MouseButton::Right, ButtonState::Pressed));
    // It moves out of the surface, over the field: the surface still holds it.
    ui.pointer_moved(30.0, 10.0);
    // The left button too, pressed over the field, goes to the surface that holds the pointer.
    assert!(ui.pointer_button(MouseButton::Left, ButtonState::Pressed));
    assert!(ui.pointer_button(MouseButton::Left, ButtonState::Released));
    assert!(ui.pointer_button(MouseButton::Right, ButtonState::Released));
    let actions = ui.take_actions();
    assert!(
        actions
            .iter()
            .all(|action| matches!(action.kind, ActionKind::Surface(_))),
        "no context menu, nor anything but the surface's: {actions:?}"
    );
    let inputs: Vec<SurfaceInput> = actions
        .into_iter()
        .filter_map(|action| match action.kind {
            ActionKind::Surface(input) => Some(input),
            _ => None,
        })
        .collect();
    assert_eq!(
        inputs,
        [
            SurfaceInput::Pressed {
                button: MouseButton::Right,
                at: [10.0, 20.0],
                modifiers: Modifiers::default(),
            },
            SurfaceInput::Moved { at: [15.0, -35.0] },
            SurfaceInput::Pressed {
                button: MouseButton::Left,
                at: [15.0, -35.0],
                modifiers: Modifiers::default(),
            },
            SurfaceInput::Released {
                button: MouseButton::Left,
                at: [15.0, -35.0],
            },
            SurfaceInput::Released {
                button: MouseButton::Right,
                at: [15.0, -35.0],
            },
        ]
    );
    // The press took the focus from the field.
    assert_eq!(ui.focus(), None);
    // Let go, the pointer's moves no longer reach the surface, nor does a press over the field.
    ui.pointer_moved(40.0, 10.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert!(surface_inputs(&mut ui).is_empty());
}

#[test]
fn the_keys_that_fly_through_a_surface_reach_the_program_while_it_holds_the_pointer() {
    let (mut ui, field, _) = interface();
    ui.pointer_moved(20.0, 120.0);
    ui.pointer_button(MouseButton::Right, ButtonState::Pressed);
    // The focus given back to the field meanwhile: T neither types in it nor runs its shortcut.
    ui.set_focus(Some(field));
    assert!(!ui.key(&typed("t")));
    assert_eq!(ui.field_text(field), Some(""));
    assert!(
        ui.take_actions()
            .iter()
            .all(|action| action.kind != ActionKind::Command("theme".to_owned()))
    );
    // A chord with Control still runs its shortcut.
    let mut control = Modifiers::default();
    control.control = true;
    ui.modifiers(control);
    assert!(ui.key(&typed("s")));
    ui.modifiers(Modifiers::default());
    assert!(
        ui.take_actions()
            .iter()
            .any(|action| action.kind == ActionKind::Command("save".to_owned()))
    );
    // Let go, T types in the field again.
    ui.pointer_button(MouseButton::Right, ButtonState::Released);
    assert!(ui.key(&typed("t")));
    assert_eq!(ui.field_text(field), Some("t"));
}

#[test]
fn a_surface_lets_the_pointer_go_when_its_button_is_let_go_after_the_pointer_left() {
    let (mut ui, field, _) = interface();
    ui.pointer_moved(20.0, 120.0);
    ui.pointer_button(MouseButton::Right, ButtonState::Pressed);
    ui.pointer_moved(30.0, 140.0);
    // The pointer leaves the window, then the button is let go, as the platform lets it go when
    // the window loses the focus.
    ui.pointer_left();
    assert!(ui.pointer_button(MouseButton::Right, ButtonState::Released));
    assert_eq!(
        surface_inputs(&mut ui).last(),
        Some(&SurfaceInput::Released {
            button: MouseButton::Right,
            at: [15.0, 30.0],
        })
    );
    // Let go: the keys type in the field again.
    ui.set_focus(Some(field));
    assert!(ui.key(&typed("t")));
    assert_eq!(ui.field_text(field), Some("t"));
}

#[test]
fn alt_pressed_while_a_surface_holds_the_pointer_opens_no_menu() {
    let (mut ui, _, _) = interface();
    // A bar of menus in a root of its own, which Alt let go alone gives the focus.
    let settings = RootSettings {
        size: [400.0, 30.0],
        scale: 2.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0, 300.0],
        modal: false,
    };
    let root = ui.add_root("bar", settings, Style::default());
    ui.build(root, 0, |children| {
        children.add("menus", Element::menu_bar(Style::default()), |menus| {
            let item = Element::menu_item("File", "", ItemMark::Submenu, Style::default());
            menus.add("File", item, |_| {});
        });
    });
    ui.frame();
    let alt = |state| KeyEvent {
        physical: KeyCode::AltLeft,
        logical: Key::Named(NamedKey::Alt),
        text: None,
        state,
        repeat: false,
    };
    // During a gesture, Alt is the gesture's.
    ui.pointer_moved(20.0, 120.0);
    ui.pointer_button(MouseButton::Right, ButtonState::Pressed);
    ui.key(&alt(ButtonState::Pressed));
    assert!(!ui.key(&alt(ButtonState::Released)));
    assert_eq!(ui.focus(), None);
    // Let go, Alt alone gives the bar of menus the focus, as it does.
    ui.pointer_button(MouseButton::Right, ButtonState::Released);
    ui.key(&alt(ButtonState::Pressed));
    assert!(ui.key(&alt(ButtonState::Released)));
    assert!(ui.focus().is_some());
}

#[test]
fn the_wheel_over_a_surface_reaches_it_and_elsewhere_does_not() {
    let (mut ui, _, _) = interface();
    ui.pointer_moved(20.0, 120.0);
    let delta = ScrollDelta::Lines { x: 0.0, y: 1.0 };
    assert!(ui.wheel(delta));
    assert_eq!(
        surface_inputs(&mut ui),
        [SurfaceInput::Wheel {
            delta,
            at: [10.0, 20.0]
        }]
    );
    ui.pointer_moved(20.0, 10.0);
    ui.wheel(delta);
    assert!(surface_inputs(&mut ui).is_empty());
}

#[test]
fn a_button_s_chord_holds_with_more_modifiers_held_and_the_most_named_wins() {
    let mut ui = Ui::new(Fonts::new());
    let table = "[viewport]\nLeftButton = \"select\"\n\"Alt+LeftButton\" = \"orbit\"\n\
                 MiddleButton = \"pan\"\n\"Shift+MiddleButton\" = \"slide\"\n";
    ui.set_shortcuts(&[Shortcuts::from_text("keys", table)])
        .expect("a table");
    let held = |shift: bool, alt: bool| {
        let mut modifiers = Modifiers::default();
        modifiers.shift = shift;
        modifiers.alt = alt;
        modifiers
    };
    let command = |button, modifiers| ui.button_command("viewport", button, modifiers);
    assert_eq!(
        command(MouseButton::Left, held(false, false)),
        Some("select")
    );
    // Shift held too, which no chord of the left button names: it still selects.
    assert_eq!(
        command(MouseButton::Left, held(true, false)),
        Some("select")
    );
    assert_eq!(command(MouseButton::Left, held(true, true)), Some("orbit"));
    assert_eq!(
        command(MouseButton::Middle, held(true, false)),
        Some("slide")
    );
    assert_eq!(command(MouseButton::Right, held(false, false)), None);
    assert_eq!(
        ui.button_command("global", MouseButton::Left, held(false, false)),
        None
    );
    // A later table wins for one chord, and the chords read as they are written.
    let blender = "[viewport]\nMiddleButton = \"orbit\"\n";
    ui.set_shortcuts(&[
        Shortcuts::from_text("keys", table),
        Shortcuts::from_text("blender", blender),
    ])
    .expect("tables");
    assert_eq!(
        ui.button_command("viewport", MouseButton::Middle, held(false, false)),
        Some("orbit")
    );
    let scopes = ui.shortcuts();
    assert!(
        scopes[0]
            .1
            .contains(&("Alt+LeftButton".to_owned(), "orbit".to_owned()))
    );
    // A button that no chord can name is refused.
    let wrong = Shortcuts::from_text("wrong", "[viewport]\nWheelButton = \"zoom\"\n");
    assert!(ui.set_shortcuts(&[wrong]).is_err());
}
