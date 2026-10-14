//! Translating the engine's events into egui's.

use egui::{Modifiers, MouseWheelUnit, PointerButton, Pos2, pos2, vec2};
use slime_platform::{
    ButtonState, Event, Key, KeyCode, KeyEvent, MouseButton, NamedKey, ScrollDelta,
};

/// The events of the window since the last frame, as egui reads them, and what they need: the
/// scale factor and where the cursor is.
pub(crate) struct Input {
    /// The window's physical pixels per logical pixel.
    scale_factor: f32,
    /// Where the cursor was last seen, in physical pixels.
    pointer: Pos2,
    /// The Ctrl keys that are down, by position, as their own presses and releases say: the engine
    /// does not report the state of the modifier keys.
    control_keys: Vec<KeyCode>,
    /// Whether the window has the keyboard focus, as its last focus change said: egui stops a
    /// text field's caret blinking while it does not.
    focused: bool,
    events: Vec<egui::Event>,
}

impl Input {
    pub(crate) fn new(scale_factor: f64) -> Self {
        Self {
            scale_factor: scale_factor as f32,
            pointer: Pos2::ZERO,
            control_keys: Vec::new(),
            // egui's own assumption until the window says otherwise.
            focused: true,
            events: Vec::new(),
        }
    }

    pub(crate) fn focused(&self) -> bool {
        self.focused
    }

    pub(crate) fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// The events gathered since the last call.
    pub(crate) fn take_events(&mut self) -> Vec<egui::Event> {
        std::mem::take(&mut self.events)
    }

    /// Takes `event`, with its positions in egui's points at egui's `zoom` factor: egui draws a
    /// point as `zoom` times the scale factor in physical pixels.
    pub(crate) fn handle(&mut self, event: &Event, zoom: f32) {
        // The engine does not report the state of the modifier keys yet.
        let modifiers = Modifiers::default();
        let pixels_per_point = self.scale_factor * zoom;
        match event {
            Event::ScaleFactorChanged { scale_factor } => {
                self.scale_factor = *scale_factor as f32;
            }
            Event::CursorMoved { x, y } => {
                self.pointer = pos2(*x as f32, *y as f32);
                self.events
                    .push(egui::Event::PointerMoved(self.pointer / pixels_per_point));
            }
            Event::CursorLeft => self.events.push(egui::Event::PointerGone),
            Event::MouseButton { button, state } => {
                if let Some(button) = pointer_button(*button) {
                    self.events.push(egui::Event::PointerButton {
                        pos: self.pointer / pixels_per_point,
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
                        vec2(*x as f32, *y as f32) / pixels_per_point,
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
            Event::Focused(focused) => {
                self.focused = *focused;
                self.events.push(egui::Event::WindowFocused(*focused));
            }
            _ => {}
        }
    }

    /// A key, by the name egui gives it, and the text it produces when pressed.
    fn key(&mut self, event: &KeyEvent, modifiers: Modifiers) {
        let pressed = event.state == ButtonState::Pressed;
        // A press is Ctrl by its meaning, so that a key the layout turns into Ctrl counts, and the
        // release of the same key ends it whatever that release means: on Linux, winit gives a
        // release the meaning the key has with the modifiers still down, and with the layout
        // options that switch groups with Ctrl, such as `grp:ctrl_shift_toggle`, Ctrl released
        // while Shift is held means "next group".
        if !pressed {
            self.control_keys.retain(|key| *key != event.physical);
        } else if event.logical == Key::Named(NamedKey::Control)
            && !self.control_keys.contains(&event.physical)
        {
            self.control_keys.push(event.physical);
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
        if let (true, Some(text)) = (pressed && self.control_keys.is_empty(), &event.text)
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

    #[test]
    fn cursor_positions_are_in_logical_points() {
        let mut input = Input::new(2.0);
        input.handle(&Event::CursorMoved { x: 100.0, y: 50.0 }, 1.0);
        input.handle(
            &Event::MouseButton {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            },
            1.0,
        );
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
    fn positions_follow_egui_zoom() {
        let mut input = Input::new(2.0);
        input.handle(&Event::CursorMoved { x: 100.0, y: 60.0 }, 1.0);
        // egui is zoomed in after the cursor moved: a click lands where the cursor is now drawn.
        let button = Event::MouseButton {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        };
        input.handle(&button, 2.0);
        input.handle(
            &Event::MouseWheel(ScrollDelta::Pixels { x: 0.0, y: 40.0 }),
            2.0,
        );
        let events = input.take_events();
        assert!(matches!(
            events[1],
            egui::Event::PointerButton { pos, .. } if pos == pos2(25.0, 15.0)
        ));
        assert!(matches!(
            events[2],
            egui::Event::MouseWheel { unit: MouseWheelUnit::Point, delta, .. }
                if delta == vec2(0.0, 10.0)
        ));
    }

    #[test]
    fn keys_give_egui_keys_and_text() {
        let mut input = Input::new(1.0);
        input.handle(
            &Event::Key(KeyEvent {
                physical: KeyCode::KeyA,
                logical: Key::Character("a".to_owned()),
                text: Some("a".to_owned()),
                state: ButtonState::Pressed,
                repeat: false,
            }),
            1.0,
        );
        input.handle(
            &Event::Key(KeyEvent {
                physical: KeyCode::Enter,
                logical: Key::Named(NamedKey::Enter),
                text: Some("\r".to_owned()),
                state: ButtonState::Pressed,
                repeat: false,
            }),
            1.0,
        );
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

    use ButtonState::{Pressed, Released};
    use KeyCode::{ControlLeft, ControlRight};

    const CONTROL: Key = Key::Named(NamedKey::Control);

    /// The key at `physical` going down or up, meaning `logical` then.
    fn modifier(physical: KeyCode, logical: Key, state: ButtonState) -> Event {
        Event::Key(KeyEvent {
            physical,
            logical,
            text: None,
            state,
            repeat: false,
        })
    }

    /// Presses C and returns the text it types. winit gives the text of a key pressed with Ctrl
    /// without Ctrl applied, so the press always carries some.
    fn type_c(input: &mut Input) -> Vec<String> {
        input.handle(
            &Event::Key(KeyEvent {
                physical: KeyCode::KeyC,
                logical: Key::Character("c".to_owned()),
                text: Some("c".to_owned()),
                state: Pressed,
                repeat: false,
            }),
            1.0,
        );
        input
            .take_events()
            .into_iter()
            .filter_map(|event| match event {
                egui::Event::Text(text) => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn keys_pressed_while_control_is_held_give_no_text() {
        let mut input = Input::new(1.0);
        input.handle(&modifier(ControlLeft, CONTROL, Pressed), 1.0);
        assert!(type_c(&mut input).is_empty(), "Ctrl+C types no text");
        input.handle(&modifier(ControlLeft, CONTROL, Released), 1.0);
        assert_eq!(type_c(&mut input), ["c"], "C types once Ctrl is released");
    }

    #[test]
    fn control_ends_with_the_release_of_its_key_whatever_that_release_means() {
        // On Linux, winit gives a release the meaning the key has with the modifiers still down.
        // With the layout option `grp:ctrl_shift_toggle`, Ctrl released while Shift is held means
        // "next group", which the engine does not name.
        let mut input = Input::new(1.0);
        input.handle(&modifier(ControlLeft, CONTROL, Pressed), 1.0);
        input.handle(&modifier(ControlLeft, Key::Unidentified, Released), 1.0);
        assert_eq!(type_c(&mut input), ["c"]);
    }

    #[test]
    fn control_lasts_while_either_control_key_is_held() {
        let mut input = Input::new(1.0);
        input.handle(&modifier(ControlLeft, CONTROL, Pressed), 1.0);
        input.handle(&modifier(ControlRight, CONTROL, Pressed), 1.0);
        input.handle(&modifier(ControlLeft, CONTROL, Released), 1.0);
        assert!(type_c(&mut input).is_empty(), "the right Ctrl is held");
        input.handle(&modifier(ControlRight, CONTROL, Released), 1.0);
        assert_eq!(type_c(&mut input), ["c"]);
    }
}
