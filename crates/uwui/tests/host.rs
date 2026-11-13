//! The host's events and outputs (`docs/adr/0249-the-hosts-api-its-items-and-their-table.md`,
//! `docs/adr/0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md`):
//! an event answered at once on its target, its time moving the interface's clock, an event for a
//! target that no root draws on left alone, and the outputs telling only what changed, the
//! pointer's shape, the input method's state and its caret's box.

use std::time::Duration;

use slime_input::{ButtonState, CursorShape, MouseButton};
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
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
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
    ui.frame();
    (ui, ids.try_into().expect("five elements"))
}

/// `input`, come to `target` at `time`.
fn given(ui: &mut Ui, target: TargetId, time: Instant, input: Input<'_>) -> Answer {
    ui.event(Event::new(target, EventTime::received(time), input))
}

/// A press of the primary button.
const PRESS: Input<'static> = Input::PointerButton {
    button: MouseButton::Left,
    state: ButtonState::Pressed,
};

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
        Input::PointerMoved { x: 300.0, y: 280.0 },
    );
    assert_eq!(moved, Answer::NotTaken);
    assert_eq!(given(&mut ui, main, time, PRESS), Answer::NotTaken);
    // Over the button: taken.
    let moved = Input::PointerMoved { x: 10.0, y: 130.0 };
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
    let moved = Input::PointerMoved { x: 10.0, y: 10.0 };
    assert_eq!(given(&mut ui, other, at, moved), Answer::NotTaken);
    assert_eq!(given(&mut ui, other, at, PRESS), Answer::NotTaken);
    assert!(ui.state(field).is_some_and(|state| !state.hovered));
    assert_eq!(ui.focus(), None);
    assert_eq!(ui.outputs(other), Outputs::default());
    // The time moved to the events': a notification given now ends five seconds after them,
    // though the frame's time given since is earlier.
    ui.set_frame_shown(FrameShown::simulated(Instant::ZERO));
    ui.notify(Notification {
        kind: NotificationKind::Information,
        text: "Saved".to_owned(),
        ..Notification::default()
    });
    ui.frame();
    assert_eq!(ui.next_frame_due(), Some(at + Duration::from_secs(5)));
    let moved = Input::PointerMoved { x: 10.0, y: 10.0 };
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
    ui.frame();
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
