//! Translating the engine's events into egui's.

use egui::{Modifiers, MouseWheelUnit, PointerButton, Pos2, pos2, vec2};
use slime_platform::{ButtonState, Event, Key, KeyEvent, MouseButton, NamedKey, ScrollDelta};

/// The events of the window since the last frame, as egui reads them, and what they need: the
/// scale factor and where the cursor is.
pub(crate) struct Input {
    pixels_per_point: f32,
    /// Where the cursor was last seen, in logical points.
    pointer: Pos2,
    /// Whether a Ctrl key is down, as its own presses and releases say: the engine does not report
    /// the state of the modifier keys.
    control_held: bool,
    events: Vec<egui::Event>,
}

impl Input {
    pub(crate) fn new(scale_factor: f64) -> Self {
        Self {
            pixels_per_point: scale_factor as f32,
            pointer: Pos2::ZERO,
            control_held: false,
            events: Vec::new(),
        }
    }

    pub(crate) fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    /// The events gathered since the last call.
    pub(crate) fn take_events(&mut self) -> Vec<egui::Event> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn handle(&mut self, event: &Event) {
        // The engine does not report the state of the modifier keys yet.
        let modifiers = Modifiers::default();
        match event {
            Event::ScaleFactorChanged { scale_factor } => {
                self.pixels_per_point = *scale_factor as f32;
            }
            Event::CursorMoved { x, y } => {
                self.pointer = pos2(
                    *x as f32 / self.pixels_per_point,
                    *y as f32 / self.pixels_per_point,
                );
                self.events.push(egui::Event::PointerMoved(self.pointer));
            }
            Event::CursorLeft => self.events.push(egui::Event::PointerGone),
            Event::MouseButton { button, state } => {
                if let Some(button) = pointer_button(*button) {
                    self.events.push(egui::Event::PointerButton {
                        pos: self.pointer,
                        button,
                        pressed: *state == ButtonState::Pressed,
                        modifiers,
                    });
                }
            }
            Event::MouseWheel(delta) => {
                let (unit, delta) = match delta {
                    ScrollDelta::Lines { x, y } => (MouseWheelUnit::Line, vec2(*x, *y)),
                    ScrollDelta::Pixels { x, y } => (
                        MouseWheelUnit::Point,
                        vec2(
                            *x as f32 / self.pixels_per_point,
                            *y as f32 / self.pixels_per_point,
                        ),
                    ),
                };
                // A wheel tick is one move of a scroll that neither starts nor ends.
                self.events.push(egui::Event::MouseWheel {
                    unit,
                    delta,
                    phase: egui::TouchPhase::Move,
                    modifiers,
                });
            }
            Event::Key(key) => self.key(key, modifiers),
            Event::Focused(focused) => self.events.push(egui::Event::WindowFocused(*focused)),
            _ => {}
        }
    }

    /// A key, by the name egui gives it, and the text it produces when pressed.
    fn key(&mut self, event: &KeyEvent, modifiers: Modifiers) {
        let pressed = event.state == ButtonState::Pressed;
        // By meaning rather than position, so that a key the layout turns into Ctrl counts.
        if event.logical == Key::Named(NamedKey::Control) {
            self.control_held = pressed;
        }
        let name = match &event.logical {
            Key::Character(character) => character.clone(),
            Key::Named(named) => format!("{named:?}"),
            Key::Dead(_) | Key::Unidentified => String::new(),
        };
        if let Some(key) = egui::Key::from_name(&name) {
            self.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: event.repeat,
                modifiers,
            });
        }
        // A key pressed with Ctrl gives its text without Ctrl applied, "c" for Ctrl+C, which a text
        // field would insert. It is dropped, as egui's own integration with winit does: the
        // shortcut then does nothing rather than type its letter.
        if let (true, Some(text)) = (pressed && !self.control_held, &event.text)
            && text.chars().all(|character| !character.is_control())
        {
            self.events.push(egui::Event::Text(text.clone()));
        }
    }
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Back => Some(PointerButton::Extra1),
        MouseButton::Forward => Some(PointerButton::Extra2),
        MouseButton::Other(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slime_platform::KeyCode;

    #[test]
    fn cursor_positions_are_in_logical_points() {
        let mut input = Input::new(2.0);
        input.handle(&Event::CursorMoved { x: 100.0, y: 50.0 });
        input.handle(&Event::MouseButton {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        });
        let events = input.take_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], egui::Event::PointerMoved(pos) if pos == pos2(50.0, 25.0)));
        assert!(matches!(
            events[1],
            egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. }
                if pos == pos2(50.0, 25.0)
        ));
        assert!(input.take_events().is_empty(), "events are taken once");
    }

    #[test]
    fn keys_give_egui_keys_and_text() {
        let mut input = Input::new(1.0);
        input.handle(&Event::Key(KeyEvent {
            physical: KeyCode::KeyA,
            logical: Key::Character("a".to_owned()),
            text: Some("a".to_owned()),
            state: ButtonState::Pressed,
            repeat: false,
        }));
        input.handle(&Event::Key(KeyEvent {
            physical: KeyCode::Enter,
            logical: Key::Named(NamedKey::Enter),
            text: Some("\r".to_owned()),
            state: ButtonState::Pressed,
            repeat: false,
        }));
        let events = input.take_events();
        assert!(matches!(
            events[0],
            egui::Event::Key {
                key: egui::Key::A,
                pressed: true,
                ..
            }
        ));
        assert!(matches!(&events[1], egui::Event::Text(text) if text == "a"));
        assert!(matches!(
            events[2],
            egui::Event::Key {
                key: egui::Key::Enter,
                ..
            }
        ));
        assert_eq!(events.len(), 3, "a control character is not text");
    }

    #[test]
    fn keys_pressed_while_control_is_held_give_no_text() {
        let control = |state| {
            Event::Key(KeyEvent {
                physical: KeyCode::ControlLeft,
                logical: Key::Named(NamedKey::Control),
                text: None,
                state,
                repeat: false,
            })
        };
        // winit gives the text of a key pressed with Ctrl without Ctrl applied.
        let c = Event::Key(KeyEvent {
            physical: KeyCode::KeyC,
            logical: Key::Character("c".to_owned()),
            text: Some("c".to_owned()),
            state: ButtonState::Pressed,
            repeat: false,
        });
        let mut input = Input::new(1.0);

        input.handle(&control(ButtonState::Pressed));
        input.handle(&c);
        let events = input.take_events();
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, egui::Event::Text(_))),
            "Ctrl+C types no text: {events:?}"
        );

        input.handle(&control(ButtonState::Released));
        input.handle(&c);
        let events = input.take_events();
        assert!(
            matches!(events.as_slice(), [.., egui::Event::Text(text)] if text == "c"),
            "C types \"c\" once Ctrl is released: {events:?}"
        );
    }
}
