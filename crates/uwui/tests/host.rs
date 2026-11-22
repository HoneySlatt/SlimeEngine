//! The host's events and outputs (`docs/adr/0249-the-hosts-api-its-items-and-their-table.md`,
//! `docs/adr/0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md`):
//! an event answered at once on its target, its time moving the interface's clock, an event for a
//! target that no root draws on left alone, and the outputs telling only what changed, the
//! pointer's shape, the input method's state and its caret's box.

use std::time::Duration;

use slime_input::{ButtonState, CursorShape, MouseButton, Pointer, PointerId};
use uwui::action::{Action, ActionKind};
use uwui::host::TargetSize;
use uwui::host::{Answer, Event, Input, InputMethod, Outputs, Person, TargetId};
use uwui::style::{Dimension, FlexDirection, StyleBuilder};
use uwui::text::Fonts;
use uwui::time::{EventTime, FrameShown, Instant};
use uwui::tree::{
    Element, ElementId, LayoutDirection, Notification, NotificationKind, RootSettings, Ui,
};

/// Writes 200 by 30 logical pixels, of the tests' font.
fn sized(s: StyleBuilder) -> StyleBuilder {
    s.width(Dimension::Pixels(200.0))
        .height(Dimension::Pixels(30.0))
        .fonts(vec!["Noto Sans".to_owned()])
}

/// An interface of the tests' Noto Sans whose root, of 400 by 300 pixels at scale 1, holds in a column a field of text,
/// a field of search, a field of several lines, a field of numbers and a button, each 200 by 30,
/// laid out: the interface and those five.
fn interface() -> (Ui, [ElementId; 5]) {
    let mut fonts = Fonts::new();
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    let mut ui = Ui::new(fonts);
    ui.set_target_size(
        TargetId::MAIN,
        TargetSize::new([400, 300], 1.0).expect("a size"),
    );
    let settings = RootSettings {
        direction: LayoutDirection::LeftToRight,
        modal: false,
        target: TargetId::MAIN,
    };
    let root = ui.add_root("main", settings, |s| s.direction(FlexDirection::Column));
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        let elements = [
            Element::text_field("text"),
            Element::search_field(""),
            Element::text_area("lines"),
            Element::number_field(1.0, 0.1),
            Element::button("go"),
        ];
        for (index, element) in elements.into_iter().enumerate() {
            ids.push(children.add(index, element.style(sized), |_| {}));
        }
    });
    ui.frame(TargetId::MAIN);
    (ui, ids.try_into().expect("five elements"))
}

/// `input`, come to `target` at `time`.
fn given(ui: &mut Ui, target: TargetId, time: Instant, input: Input<'_>) -> Answer {
    ui.event(Event::new(target, EventTime::received(time), input))
}

/// A press of the primary button.
const PRESS: Input<'static> = Input::PointerButton {
    pointer: Pointer::MOUSE,
    button: MouseButton::Left,
    state: ButtonState::Pressed,
};

#[test]
fn a_resume_reaches_the_programs_queue_alone_and_from_its_host_alone() {
    let (mut ui, [field, ..]) = interface();
    ui.set_focus(Some(field));
    ui.frame(TargetId::MAIN);
    let _ = ui.take_actions();
    let eight_hours = Duration::from_secs(8 * 3600);
    ui.resumed(eight_hours);
    assert_eq!(
        ui.take_actions(),
        [Action {
            element: None,
            kind: ActionKind::Resumed {
                suspended: eight_hours
            },
        }]
    );
    // The interface changes nothing: its focus stays, and it asks for no frame.
    assert_eq!(ui.focus(), Some(field));
    assert!(!ui.needs_frame(TargetId::MAIN));
    // An agent cannot tell one.
    ui.act(Action {
        element: None,
        kind: ActionKind::Resumed {
            suspended: eight_hours,
        },
    });
    assert_eq!(ui.take_actions(), []);
}

#[test]
fn an_event_answers_at_once_whether_the_interface_took_it() {
    let (mut ui, [field, .., button]) = interface();
    let time = Instant::ZERO;
    let main = TargetId::MAIN;
    // Over nothing: left to the game, and so is a move.
    let moved = given(
        &mut ui,
        main,
        time,
        Input::PointerMoved {
            pointer: Pointer::MOUSE,
            x: 300.0,
            y: 280.0,
        },
    );
    assert_eq!(moved, Answer::NotTaken);
    assert_eq!(given(&mut ui, main, time, PRESS), Answer::NotTaken);
    // Over the button: taken.
    let moved = Input::PointerMoved {
        pointer: Pointer::MOUSE,
        x: 10.0,
        y: 130.0,
    };
    assert!(!given(&mut ui, main, time, moved).taken());
    assert!(ui.state(button).is_some_and(|state| state.hovered));
    assert!(given(&mut ui, main, time, PRESS).taken());
    assert!(ui.focus() != Some(field));
}

#[test]
fn an_event_for_a_target_that_no_root_draws_on_changes_nothing_but_the_time() {
    let (mut ui, [field, ..]) = interface();
    let other = TargetId(7);
    let at = Instant::ZERO + Duration::from_secs(3);
    let moved = Input::PointerMoved {
        pointer: Pointer::MOUSE,
        x: 10.0,
        y: 10.0,
    };
    assert_eq!(given(&mut ui, other, at, moved), Answer::NotTaken);
    assert_eq!(given(&mut ui, other, at, PRESS), Answer::NotTaken);
    assert!(ui.state(field).is_some_and(|state| !state.hovered));
    assert_eq!(ui.focus(), None);
    assert_eq!(ui.outputs(other), Outputs::default());
    // The time moved to the events': a notification given now ends five seconds after them,
    // though the frame's time given since is earlier.
    ui.set_frame_shown(TargetId::MAIN, FrameShown::simulated(Instant::ZERO));
    ui.notify(Notification {
        kind: NotificationKind::Information,
        text: "Saved".to_owned(),
        ..Notification::default()
    });
    ui.frame(TargetId::MAIN);
    assert_eq!(
        ui.next_frame_due(TargetId::MAIN),
        Some(at + Duration::from_secs(5))
    );
    let moved = Input::PointerMoved {
        pointer: Pointer::MOUSE,
        x: 10.0,
        y: 10.0,
    };
    let _ = given(&mut ui, TargetId::MAIN, Instant::ZERO, moved);
    assert!(
        ui.state(field).is_some_and(|state| state.hovered),
        "an event older than the clock is still handled"
    );
}

#[test]
fn the_outputs_tell_the_cursor_only_when_it_changed() {
    let (mut ui, [_, _, _, number, button]) = interface();
    let main = TargetId::MAIN;
    assert_eq!(
        ui.outputs(main),
        Outputs::default(),
        "the arrow, as a host starts"
    );
    ui.pointer_moved(10.0, 10.0);
    assert_eq!(ui.outputs(main).cursor, Some(CursorShape::Text));
    ui.pointer_moved(20.0, 15.0);
    assert_eq!(ui.outputs(main).cursor, None, "still the I-beam");
    // Over the field of numbers that does not hold the focus: the arrows of its drag.
    let number_at = ui.rect(number).expect("laid out");
    ui.pointer_moved(number_at.x + 5.0, number_at.y + 5.0);
    assert_eq!(ui.outputs(main).cursor, Some(CursorShape::ResizeHorizontal));
    // A change and its undoing between two readings tell nothing.
    let button_at = ui.rect(button).expect("laid out");
    ui.pointer_moved(button_at.x + 5.0, button_at.y + 5.0);
    ui.pointer_moved(number_at.x + 5.0, number_at.y + 5.0);
    assert_eq!(ui.outputs(main).cursor, None);
    ui.pointer_moved(button_at.x + 5.0, button_at.y + 5.0);
    assert_eq!(ui.outputs(main).cursor, Some(CursorShape::Default));
}

#[test]
fn the_outputs_tell_the_input_method_on_as_each_fields_input_starts_and_its_caret_as_it_moves() {
    let (mut ui, [text, search, lines, number, _]) = interface();
    let main = TargetId::MAIN;
    ui.set_focus(Some(text));
    let outputs = ui.outputs(main);
    assert_eq!(outputs.input_method, Some(InputMethod::On));
    let area = outputs
        .input_area
        .expect("the caret told with the input's start");
    let field = ui.rect(text).expect("laid out");
    assert!(
        area.x >= field.x && area.y >= field.y && area.height > 0.0,
        "{area:?} in {field:?}"
    );
    assert_eq!(ui.outputs(main), Outputs::default(), "told once");
    // Each field that takes the focus starts an input of its own.
    for id in [search, lines, number] {
        ui.set_focus(Some(id));
        assert_eq!(ui.outputs(main).input_method, Some(InputMethod::On));
    }
    // The caret moved by typing: its box told once the frame laid it out, the input going on.
    ui.set_focus(Some(text));
    let before = ui.outputs(main).input_area.expect("told");
    ui.key(&typed("w"));
    ui.frame(TargetId::MAIN);
    let outputs = ui.outputs(main);
    assert_eq!(outputs.input_method, None, "the same input");
    assert!(
        outputs.input_area.expect("the caret moved").x > before.x,
        "the caret moved on, from {before:?}"
    );
    // The focus lost and taken back before the host reads: the composition ended, a new input.
    ui.set_focus(None);
    ui.set_focus(Some(text));
    assert_eq!(ui.outputs(main).input_method, Some(InputMethod::On));
    // No focus: off, once.
    ui.set_focus(None);
    assert_eq!(ui.outputs(main).input_method, Some(InputMethod::Off));
    assert_eq!(ui.outputs(main), Outputs::default());
}

/// A press of the key that types `text`.
fn typed(text: &str) -> slime_input::KeyEvent {
    slime_input::KeyEvent {
        physical: slime_input::KeyCode::KeyW,
        logical: slime_input::Key::Character(text.to_owned()),
        text: Some(text.to_owned()),
        state: ButtonState::Pressed,
        repeat: false,
    }
}

/// A finger's touch at `x` and `y` and its press.
fn touch(ui: &mut Ui, finger: Pointer, x: f32, y: f32) -> Answer {
    let _ = given(
        ui,
        TargetId::MAIN,
        Instant::ZERO,
        Input::PointerMoved {
            pointer: finger,
            x,
            y,
        },
    );
    given(
        ui,
        TargetId::MAIN,
        Instant::ZERO,
        Input::PointerButton {
            pointer: finger,
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        },
    )
}

#[test]
fn a_second_finger_is_not_followed_while_the_first_touches() {
    let (mut ui, [field, .., button]) = interface();
    let main = TargetId::MAIN;
    let first = Pointer::finger(PointerId(1));
    let second = Pointer::finger(PointerId(2));
    assert!(touch(&mut ui, first, 10.0, 130.0).taken());
    // The second, over the field, neither hovers nor presses it.
    assert_eq!(touch(&mut ui, second, 10.0, 10.0), Answer::NotTaken);
    assert_eq!(ui.focus(), Some(button), "the first's press");
    assert!(ui.state(field).is_some_and(|state| !state.hovered));
    assert!(ui.state(button).is_some_and(|state| state.hovered));
    let lifted = given(
        &mut ui,
        main,
        Instant::ZERO,
        Input::PointerLeft { pointer: second },
    );
    assert_eq!(lifted, Answer::NotTaken);
    assert!(ui.state(button).is_some_and(|state| state.hovered));
    // The first lifts over the button, which it pressed.
    let released = Input::PointerButton {
        pointer: first,
        button: MouseButton::Left,
        state: ButtonState::Released,
    };
    assert!(given(&mut ui, main, Instant::ZERO, released).taken());
    let _ = given(
        &mut ui,
        main,
        Instant::ZERO,
        Input::PointerLeft { pointer: first },
    );
    let actions: Vec<_> = ui
        .take_actions()
        .into_iter()
        .map(|action| (action.element, action.kind))
        .collect();
    assert_eq!(actions, [(Some(button), ActionKind::Press)]);
    // Once none is followed, the next finger is.
    assert!(touch(&mut ui, Pointer::finger(PointerId(3)), 10.0, 10.0).taken());
    assert_eq!(ui.focus(), Some(field));
}

#[test]
fn the_mouse_waits_while_a_finger_touches_and_comes_back_where_it_rests() {
    let (mut ui, [.., button]) = interface();
    let main = TargetId::MAIN;
    let mouse_at = |x, y| Input::PointerMoved {
        pointer: Pointer::MOUSE,
        x,
        y,
    };
    let release = |pointer| Input::PointerButton {
        pointer,
        button: MouseButton::Left,
        state: ButtonState::Released,
    };
    // The mouse rests over the button.
    let _ = given(&mut ui, main, Instant::ZERO, mouse_at(10.0, 130.0));
    assert!(ui.state(button).is_some_and(|state| state.hovered));
    // A finger touches elsewhere: the interface follows it alone, the mouse's press not taken.
    let finger = Pointer::finger(PointerId(1));
    assert_eq!(touch(&mut ui, finger, 300.0, 280.0), Answer::NotTaken);
    assert_eq!(given(&mut ui, main, Instant::ZERO, PRESS), Answer::NotTaken);
    let _ = given(&mut ui, main, Instant::ZERO, release(Pointer::MOUSE));
    let _ = given(&mut ui, main, Instant::ZERO, release(finger));
    let _ = given(
        &mut ui,
        main,
        Instant::ZERO,
        Input::PointerLeft { pointer: finger },
    );
    assert!(ui.take_actions().is_empty());
    // Once it lifts, the mouse is where it rested: its click, without a move, presses the button.
    assert!(ui.state(button).is_some_and(|state| state.hovered));
    assert!(given(&mut ui, main, Instant::ZERO, PRESS).taken());
    assert!(given(&mut ui, main, Instant::ZERO, release(Pointer::MOUSE)).taken());
    let actions: Vec<_> = ui
        .take_actions()
        .into_iter()
        .map(|action| (action.element, action.kind))
        .collect();
    assert_eq!(actions, [(Some(button), ActionKind::Press)]);
}

#[test]
fn a_finger_that_leaves_without_lifting_lets_go_of_its_press_without_an_action() {
    let (mut ui, [.., number, _]) = interface();
    let main = TargetId::MAIN;
    // A finger presses the field of numbers, then the platform cancels its touch.
    let finger = Pointer::finger(PointerId(1));
    assert!(touch(&mut ui, finger, 10.0, 100.0).taken());
    let _ = given(
        &mut ui,
        main,
        Instant::ZERO,
        Input::PointerLeft { pointer: finger },
    );
    assert!(ui.state(number).is_some_and(|state| !state.pressed));
    // The mouse's move drags nothing.
    let moved = Input::PointerMoved {
        pointer: Pointer::MOUSE,
        x: 120.0,
        y: 100.0,
    };
    let _ = given(&mut ui, main, Instant::ZERO, moved);
    assert!(ui.take_actions().is_empty());
}

#[test]
fn a_finger_that_leaves_before_it_came_changes_nothing() {
    let (mut ui, [.., button]) = interface();
    let main = TargetId::MAIN;
    let moved = Input::PointerMoved {
        pointer: Pointer::MOUSE,
        x: 10.0,
        y: 130.0,
    };
    let _ = given(&mut ui, main, Instant::ZERO, moved);
    let stray = Input::PointerLeft {
        pointer: Pointer::finger(PointerId(4)),
    };
    assert_eq!(given(&mut ui, main, Instant::ZERO, stray), Answer::NotTaken);
    assert!(ui.state(button).is_some_and(|state| state.hovered));
}
