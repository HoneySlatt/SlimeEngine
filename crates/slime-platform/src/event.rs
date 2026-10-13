//! Events delivered to the application, and their translation from winit's.

use winit::event::WindowEvent;
use winit::keyboard::PhysicalKey;

use crate::input::{self, ButtonState, KeyEvent, MouseButton, ScrollDelta};
use crate::window::PhysicalSize;

/// Something that happened to the window, or input it received.
///
/// Keys arrive while the window has the keyboard focus. Mouse input arrives while the cursor is
/// over the window, and outside it while a button pressed over the window is held (see
/// [`CursorMoved`](Self::CursorMoved)). The engine does not report yet: the state of modifier
/// keys as a whole (track the `Shift`, `Control`, `Alt` and `Super` keys instead; a modifier
/// already held when the window gains the focus is not seen until it is pressed again, as
/// [`Key`](Self::Key) says), text composed by input methods (used for Chinese, Japanese or
/// Korean), touch screens, touchpad gestures, or files dropped on the window.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// The user or the compositor asked to close the window, for example with a close button or a
    /// compositor shortcut.
    CloseRequested,
    /// The window's content area changed size.
    Resized(PhysicalSize),
    /// The number of physical pixels per logical pixel changed, for example because the window
    /// moved to another display.
    ScaleFactorChanged {
        /// The new number of physical pixels per logical pixel.
        scale_factor: f64,
    },
    /// The window gained (`true`) or lost (`false`) the keyboard focus. The keys and mouse buttons
    /// still down when the focus is lost are released just before: see [`Key`](Self::Key) and
    /// [`MouseButton`](Self::MouseButton).
    Focused(bool),
    /// A key went down or up.
    ///
    /// The events of a key come in pairs, the same way on every platform: a release always follows
    /// a press of the same key, and auto-repeated presses may come in between (see
    /// [`KeyEvent::repeat`]). A key still down when the window loses the keyboard focus is released
    /// just before [`Focused(false)`](Self::Focused), even though the user lets go of it later, in
    /// another window. A key already down when the window gains the focus is not reported, its
    /// release included, until it is pressed again. On Windows, Print Screen gives no event:
    /// Windows reports only its release.
    Key(KeyEvent),
    /// A mouse button went down over the window, or went up after going down there, possibly
    /// with the cursor outside the window by then.
    ///
    /// The events of a button come in pairs, as those of a key do: a release always follows a
    /// press of the same button, and a button is not pressed twice in a row. A button still down
    /// when the window loses the keyboard focus is released just before
    /// [`Focused(false)`](Self::Focused), even if the user lets go of it later, over the window or
    /// in another one; its real release is then not reported. One case is not covered: on
    /// Wayland, a compositor that takes the pointer away during a press without taking the
    /// keyboard focus leaves the button down until the button is next released over the window,
    /// or until the window loses the focus.
    MouseButton {
        /// Which button.
        button: MouseButton,
        /// Whether it went down or up.
        state: ButtonState,
    },
    /// The cursor moved over the window, or outside it while a button pressed over the window is
    /// held. On X11, it is also reported when the window gains the keyboard focus, wherever the
    /// cursor is. The position can therefore lie outside the content area: check it against the
    /// window's size before using it as the position of a pixel.
    CursorMoved {
        /// Distance from the left edge of the content area, in physical pixels: negative left of
        /// it, the content area's width or more right of it.
        x: f64,
        /// Distance from the top edge of the content area, in physical pixels: negative above it,
        /// the content area's height or more below it.
        y: f64,
    },
    /// The cursor entered the window.
    CursorEntered,
    /// The cursor left the window.
    CursorLeft,
    /// A mouse wheel or a touchpad scrolled while the cursor was over the window, or outside it
    /// while a button pressed over the window is held.
    MouseWheel(ScrollDelta),
}

/// Translates winit's window events into the engine's.
///
/// It keeps track of the keys and mouse buttons that are down, so that their events pair up the
/// same way on every platform, as [`Event::Key`] and [`Event::MouseButton`] say. winit leaves
/// focus changes to each platform: on X11 and Windows, it reports the keys already down when the
/// window gains the focus as new presses (marked as synthetic, a mark the engine does not pass
/// on), and releases the keys still down when the window loses the focus; on Wayland, it does
/// neither, so a key let go of in another window would stay down. It releases no mouse button on
/// any platform, and the release of a button let go of in another window never comes on Windows,
/// nor on Wayland when the compositor takes the pointer away during the press.
#[derive(Debug, Default)]
pub(crate) struct Translator {
    /// The keys whose press was delivered and whose release was not, with that press, in the
    /// order they went down.
    pressed: Vec<(PhysicalKey, KeyEvent)>,
    /// The keys already down when the window gained the focus. Their events are dropped until
    /// they are released.
    held_before_focus: Vec<PhysicalKey>,
    /// The mouse buttons whose press was delivered and whose release was not, in the order they
    /// went down.
    buttons: Vec<MouseButton>,
}

impl Translator {
    /// Translates `event` and passes the result to `deliver`: nothing for the events the engine
    /// does not handle, and several events when the window loses the focus while keys are down.
    /// Redraw requests are not events: the event loop passes them to
    /// [`EventHandler::redraw_requested`](crate::EventHandler::redraw_requested).
    pub(crate) fn translate(&mut self, event: WindowEvent, mut deliver: impl FnMut(Event)) {
        let event = match event {
            WindowEvent::CloseRequested => Event::CloseRequested,
            WindowEvent::Resized(size) => Event::Resized(PhysicalSize {
                width: size.width,
                height: size.height,
            }),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                Event::ScaleFactorChanged { scale_factor }
            }
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.release_all(&mut deliver);
                }
                Event::Focused(focused)
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                let physical = event.physical_key;
                match self.key(physical, input::key_event(event), is_synthetic) {
                    Some(key) => Event::Key(key),
                    None => return,
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = input::mouse_button(button);
                let state = input::button_state(state);
                if !self.mouse_button(button, state) {
                    return;
                }
                Event::MouseButton { button, state }
            }
            WindowEvent::CursorMoved { position, .. } => Event::CursorMoved {
                x: position.x,
                y: position.y,
            },
            WindowEvent::CursorEntered { .. } => Event::CursorEntered,
            WindowEvent::CursorLeft { .. } => Event::CursorLeft,
            WindowEvent::MouseWheel { delta, .. } => Event::MouseWheel(input::scroll_delta(delta)),
            _ => return,
        };
        deliver(event);
    }

    /// The key event to deliver for one that winit reports, if any. `physical` is winit's code of
    /// the key, which tells apart the keys that [`KeyEvent::physical`] reports as
    /// [`KeyCode::Unidentified`](crate::KeyCode::Unidentified).
    fn key(&mut self, physical: PhysicalKey, event: KeyEvent, synthetic: bool) -> Option<KeyEvent> {
        if synthetic {
            // A synthetic press is a key already down when the window gains the focus. A
            // synthetic release is a key still down when the window loses it, which
            // `release_all` releases on every platform.
            if event.state == ButtonState::Pressed && !self.held_before_focus.contains(&physical) {
                self.held_before_focus.push(physical);
            }
            return None;
        }
        let delivered = self.pressed.iter().position(|(key, _)| *key == physical);
        match (event.state, delivered) {
            (ButtonState::Pressed, Some(_)) => Some(event),
            // The key went down before the window gained the focus. On X11, its first
            // auto-repeat after that is not marked as a repeat.
            (ButtonState::Pressed, None)
                if event.repeat || self.held_before_focus.contains(&physical) =>
            {
                None
            }
            (ButtonState::Pressed, None) => {
                self.pressed.push((physical, event.clone()));
                Some(event)
            }
            (ButtonState::Released, Some(index)) => {
                self.pressed.remove(index);
                Some(event)
            }
            // A key whose press was not delivered: it went down before the window gained the
            // focus (on Wayland, nothing marks it), or `release_all` released it already.
            (ButtonState::Released, None) => {
                self.held_before_focus.retain(|key| *key != physical);
                None
            }
        }
    }

    /// Whether to deliver a mouse button event that winit reports: a press of a button not down
    /// already, or a release of a button whose press was delivered.
    fn mouse_button(&mut self, button: MouseButton, state: ButtonState) -> bool {
        let down = self.buttons.iter().position(|held| *held == button);
        match (state, down) {
            (ButtonState::Pressed, None) => self.buttons.push(button),
            (ButtonState::Released, Some(index)) => {
                self.buttons.remove(index);
            }
            // A second press would break the pairing. A release of a button that is not down
            // comes after `release_all` released it: on X11, the grab that a press gives the
            // window can outlast the focus, so the real release still arrives.
            (ButtonState::Pressed, Some(_)) | (ButtonState::Released, None) => return false,
        }
        true
    }

    /// Releases the keys and mouse buttons still down, because the window is losing the focus.
    fn release_all(&mut self, deliver: &mut impl FnMut(Event)) {
        for (_, press) in self.pressed.drain(..) {
            deliver(Event::Key(KeyEvent {
                physical: press.physical,
                logical: press.logical,
                text: None,
                state: ButtonState::Released,
                repeat: false,
            }));
        }
        self.held_before_focus.clear();
        for button in self.buttons.drain(..) {
            deliver(Event::MouseButton {
                button,
                state: ButtonState::Released,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::{PhysicalPosition, PhysicalSize as WinitSize};
    use winit::keyboard::{KeyCode as WinitKeyCode, NativeKeyCode};

    use super::*;
    use crate::input::{Key, KeyCode};

    /// Translates `event` with a translator that has seen nothing before, which gives at most one
    /// event.
    fn translate(event: WindowEvent) -> Option<Event> {
        let mut events = Vec::new();
        Translator::default().translate(event, |event| events.push(event));
        assert!(events.len() <= 1, "{events:?}");
        events.pop()
    }

    /// The events delivered when the window loses the focus.
    fn lose_focus(translator: &mut Translator) -> Vec<Event> {
        let mut events = Vec::new();
        translator.translate(WindowEvent::Focused(false), |event| events.push(event));
        events
    }

    /// winit's code of the W key, and the engine's event for it, as `Translator::key` receives it.
    const W: PhysicalKey = PhysicalKey::Code(WinitKeyCode::KeyW);
    fn w(state: ButtonState, repeat: bool) -> KeyEvent {
        KeyEvent {
            physical: KeyCode::KeyW,
            logical: Key::Character("w".to_owned()),
            text: (state == ButtonState::Pressed).then(|| "w".to_owned()),
            state,
            repeat,
        }
    }

    use ButtonState::{Pressed, Released};

    #[test]
    fn keys_still_down_are_released_when_the_focus_is_lost() {
        // On Wayland, winit releases nothing when the focus is lost.
        let mut translator = Translator::default();
        assert_eq!(
            translator.key(W, w(Pressed, false), false),
            Some(w(Pressed, false))
        );
        assert_eq!(
            lose_focus(&mut translator),
            [Event::Key(w(Released, false)), Event::Focused(false)]
        );
        // On X11 and Windows, winit releases the key itself first; the key is released once.
        assert_eq!(
            translator.key(W, w(Pressed, false), false),
            Some(w(Pressed, false))
        );
        assert_eq!(translator.key(W, w(Released, false), true), None);
        assert_eq!(
            lose_focus(&mut translator),
            [Event::Key(w(Released, false)), Event::Focused(false)]
        );
    }

    #[test]
    fn released_keys_and_auto_repeats_pass_through() {
        let mut translator = Translator::default();
        for (event, synthetic) in [
            (w(Pressed, false), false),
            (w(Pressed, true), false),
            (w(Released, false), false),
        ] {
            assert_eq!(translator.key(W, event.clone(), synthetic), Some(event));
        }
        assert_eq!(lose_focus(&mut translator), [Event::Focused(false)]);
    }

    #[test]
    fn keys_already_down_when_the_focus_is_gained_are_not_reported() {
        let mut translator = Translator::default();
        // X11 and Windows: a synthetic press, auto-repeats (the first one not marked as such on
        // X11), then the release.
        for (event, synthetic) in [
            (w(Pressed, false), true),
            (w(Pressed, false), false),
            (w(Pressed, true), false),
            (w(Released, false), false),
        ] {
            assert_eq!(translator.key(W, event, synthetic), None);
        }
        // Wayland: only the release.
        assert_eq!(translator.key(W, w(Released, false), false), None);
        // The next press is reported.
        assert_eq!(
            translator.key(W, w(Pressed, false), false),
            Some(w(Pressed, false))
        );
    }

    #[test]
    fn mouse_buttons_still_down_are_released_when_the_focus_is_lost() {
        use winit::event::{DeviceId, ElementState, MouseButton as WinitButton};

        let left = |state| WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: WinitButton::Left,
        };
        let mut translator = Translator::default();
        let mut events = Vec::new();
        translator.translate(left(ElementState::Pressed), |event| events.push(event));
        // A second press of a button already down breaks the pairing: it is dropped.
        translator.translate(left(ElementState::Pressed), |event| events.push(event));
        assert_eq!(
            events,
            [Event::MouseButton {
                button: MouseButton::Left,
                state: Pressed,
            }]
        );
        assert_eq!(
            lose_focus(&mut translator),
            [
                Event::MouseButton {
                    button: MouseButton::Left,
                    state: Released,
                },
                Event::Focused(false)
            ]
        );
        // On X11, the real release can still come after the focus is lost; it is dropped.
        let mut events = Vec::new();
        translator.translate(left(ElementState::Released), |event| events.push(event));
        assert_eq!(events, []);
        assert_eq!(lose_focus(&mut translator), [Event::Focused(false)]);
    }

    #[test]
    fn keys_the_engine_does_not_name_are_told_apart() {
        let unnamed = |state| KeyEvent {
            physical: KeyCode::Unidentified,
            logical: Key::Unidentified,
            text: None,
            state,
            repeat: false,
        };
        let first = PhysicalKey::Unidentified(NativeKeyCode::Xkb(200));
        let second = PhysicalKey::Unidentified(NativeKeyCode::Xkb(201));
        let mut translator = Translator::default();
        assert!(translator.key(first, unnamed(Pressed), false).is_some());
        assert!(translator.key(second, unnamed(Pressed), false).is_some());
        assert!(translator.key(first, unnamed(Released), false).is_some());
        assert_eq!(
            lose_focus(&mut translator),
            [Event::Key(unnamed(Released)), Event::Focused(false)]
        );
    }

    #[test]
    fn window_events_are_translated() {
        assert_eq!(
            translate(WindowEvent::CloseRequested),
            Some(Event::CloseRequested)
        );
        assert_eq!(
            translate(WindowEvent::Resized(WinitSize::new(800, 600))),
            Some(Event::Resized(PhysicalSize {
                width: 800,
                height: 600
            }))
        );
        assert_eq!(
            translate(WindowEvent::Focused(false)),
            Some(Event::Focused(false))
        );
    }

    /// Keyboard and scale-factor events cannot be built outside winit: their payloads have no
    /// public constructor. What happens to key events is tested above through `Translator::key`,
    /// and their conversion piece by piece in `input.rs`; the scale-factor arm, a copy of the
    /// value, is not tested.
    #[test]
    fn mouse_events_are_translated() {
        use winit::event::{
            DeviceId, ElementState, MouseButton as WinitButton, MouseScrollDelta, TouchPhase,
        };

        let device_id = DeviceId::dummy();
        assert_eq!(
            translate(WindowEvent::MouseInput {
                device_id,
                state: ElementState::Pressed,
                button: WinitButton::Left,
            }),
            Some(Event::MouseButton {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            })
        );
        assert_eq!(
            translate(WindowEvent::CursorMoved {
                device_id,
                position: PhysicalPosition::new(3.5, 12.0),
            }),
            Some(Event::CursorMoved { x: 3.5, y: 12.0 })
        );
        assert_eq!(
            translate(WindowEvent::CursorEntered { device_id }),
            Some(Event::CursorEntered)
        );
        assert_eq!(
            translate(WindowEvent::CursorLeft { device_id }),
            Some(Event::CursorLeft)
        );
        assert_eq!(
            translate(WindowEvent::MouseWheel {
                device_id,
                delta: MouseScrollDelta::LineDelta(0.0, -1.0),
                phase: TouchPhase::Moved,
            }),
            Some(Event::MouseWheel(ScrollDelta::Lines { x: 0.0, y: -1.0 }))
        );
    }

    #[test]
    fn events_the_engine_does_not_handle_are_dropped() {
        assert_eq!(
            translate(WindowEvent::Moved(PhysicalPosition::new(10, 20))),
            None
        );
    }
}
