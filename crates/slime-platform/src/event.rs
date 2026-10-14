//! Events delivered to the application, and their translation from winit's.

use winit::event::WindowEvent;
use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};

use crate::input::{self, ButtonState, KeyEvent, Modifiers, MouseButton, ScrollDelta};
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
    /// release included, until it is pressed again. On X11 and Windows, "already down" is judged
    /// when the event loop handles the focus gain, which can be later than the focus changed:
    /// after a long frame, and on X11 after
    /// [`EventHandler::window_created`](crate::EventHandler::window_created) returns if the window
    /// gains the focus while it runs (`slime-app` initializes the application there). A key
    /// pressed in between is not reported either. On Wayland, a new window gains the focus only
    /// once it shows its first frame, so a key held while the application initializes is not
    /// reported there either. On Windows, Print Screen gives no event:
    /// Windows reports only its release. Windows also reports a single release for the two Shift
    /// keys: while both are down, the one let go of first is released only when the other goes
    /// up, and both are then released together. Windows 11 was seen to report the press of the
    /// second Shift as an auto-repeat, in which case neither that press nor its release is
    /// reported: the first Shift's press and its release, at the last key-up, are.
    Key(KeyEvent),
    /// The modifier keys held changed: the new state, as a whole.
    ///
    /// Only changes are reported: two of these events in a row hold different states. The modifiers
    /// held when the window gains the keyboard focus are reported, unlike the keys held then (see
    /// [`Key`](Self::Key)), next to [`Focused(true)`](Self::Focused). When the window loses the
    /// focus, modifiers still held are reported released, before the keys still down and
    /// [`Focused(false)`](Self::Focused).
    ModifiersChanged(Modifiers),
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
    /// The modifiers last delivered.
    modifiers: Modifiers,
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
                let event = input::key_event(event);
                // Windows sends one key-up for both Shift keys: see `release_other_shift`.
                if cfg!(windows) && !is_synthetic && event.state == ButtonState::Released {
                    self.release_other_shift(physical, &mut deliver);
                }
                match self.key(physical, event, is_synthetic) {
                    Some(key) => Event::Key(key),
                    None => return,
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let modifiers = input::modifiers(modifiers.state());
                if modifiers == self.modifiers {
                    return;
                }
                self.modifiers = modifiers;
                Event::ModifiersChanged(modifiers)
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

    /// Releases the other Shift key when `released`, the key of a release that Windows reports, is
    /// a Shift key.
    ///
    /// Windows gives both Shift keys one key state: while both are down, letting go of one sends
    /// no key-up, and letting go of the other then sends one key-up, for that other key only. So
    /// on Windows a Shift key-up means that both are up, whether or not the engine delivered the
    /// press of the one it names. On other platforms each Shift key has its own key-up.
    fn release_other_shift(&mut self, released: PhysicalKey, deliver: &mut impl FnMut(Event)) {
        let other = match released {
            PhysicalKey::Code(WinitKeyCode::ShiftLeft) => {
                PhysicalKey::Code(WinitKeyCode::ShiftRight)
            }
            PhysicalKey::Code(WinitKeyCode::ShiftRight) => {
                PhysicalKey::Code(WinitKeyCode::ShiftLeft)
            }
            _ => return,
        };
        if let Some(index) = self.pressed.iter().position(|(key, _)| *key == other) {
            let (_, press) = self.pressed.remove(index);
            deliver(Event::Key(release_of(press)));
        }
        self.held_before_focus.retain(|key| *key != other);
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

    /// Releases the modifiers, keys and mouse buttons still down, because the window is losing the
    /// focus.
    fn release_all(&mut self, deliver: &mut impl FnMut(Event)) {
        if self.modifiers != Modifiers::default() {
            self.modifiers = Modifiers::default();
            deliver(Event::ModifiersChanged(self.modifiers));
        }
        for (_, press) in self.pressed.drain(..) {
            deliver(Event::Key(release_of(press)));
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

/// The release of a key whose press was delivered, made up by the engine.
fn release_of(press: KeyEvent) -> KeyEvent {
    KeyEvent {
        physical: press.physical,
        logical: press.logical,
        text: None,
        state: ButtonState::Released,
        repeat: false,
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::{PhysicalPosition, PhysicalSize as WinitSize};
    use winit::keyboard::{KeyCode as WinitKeyCode, ModifiersState, NativeKeyCode};

    use super::*;
    use crate::input::{Key, KeyCode, NamedKey};

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
        // X11), then the release. The unmarked press may also be the real press of a key that
        // went down after the focus changed but before the loop handled it: it is dropped too.
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

    /// What `translate` delivers on Windows for a key event that winit reports. The cfg!(windows)
    /// branch of `translate` is reproduced here, so that the test runs on every platform.
    fn windows_key(
        translator: &mut Translator,
        physical: PhysicalKey,
        event: KeyEvent,
        synthetic: bool,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        if !synthetic && event.state == Released {
            translator.release_other_shift(physical, &mut |event| events.push(event));
        }
        events.extend(translator.key(physical, event, synthetic).map(Event::Key));
        events
    }

    #[test]
    fn on_windows_a_shift_key_up_releases_both_shift_keys() {
        let shift = |code, state, repeat| {
            let (physical, engine) = match code {
                WinitKeyCode::ShiftLeft => (PhysicalKey::Code(code), KeyCode::ShiftLeft),
                _ => (PhysicalKey::Code(code), KeyCode::ShiftRight),
            };
            let event = KeyEvent {
                physical: engine,
                logical: Key::Named(NamedKey::Shift),
                text: None,
                state,
                repeat,
            };
            (physical, event)
        };
        let (left, right) = (WinitKeyCode::ShiftLeft, WinitKeyCode::ShiftRight);
        // Left Shift, then Right Shift go down; Left Shift goes up first, with no key-up; Right
        // Shift goes up, with one key-up. Windows may mark the second press as a repeat, which
        // the engine drops, and then the release of Right Shift too.
        for second_press_repeats in [false, true] {
            let mut translator = Translator::default();
            let (physical, event) = shift(left, Pressed, false);
            assert_eq!(
                windows_key(&mut translator, physical, event.clone(), false),
                [Event::Key(event)]
            );
            let (physical, event) = shift(right, Pressed, second_press_repeats);
            windows_key(&mut translator, physical, event, false);
            let (physical, event) = shift(right, Released, false);
            let mut expected = vec![Event::Key(shift(left, Released, false).1)];
            if !second_press_repeats {
                expected.push(Event::Key(event.clone()));
            }
            assert_eq!(
                windows_key(&mut translator, physical, event, false),
                expected
            );
            assert_eq!(lose_focus(&mut translator), [Event::Focused(false)]);
        }
        // Left Shift was down before the window gained the focus; Right Shift goes down in it.
        let mut translator = Translator::default();
        let (physical, event) = shift(left, Pressed, false);
        assert_eq!(windows_key(&mut translator, physical, event, true), []);
        let (physical, event) = shift(right, Pressed, false);
        assert_eq!(
            windows_key(&mut translator, physical, event.clone(), false),
            [Event::Key(event)]
        );
        let (physical, event) = shift(right, Released, false);
        assert_eq!(
            windows_key(&mut translator, physical, event.clone(), false),
            [Event::Key(event)]
        );
        // Left Shift is up too: its next press is reported.
        let (physical, event) = shift(left, Pressed, false);
        assert_eq!(
            windows_key(&mut translator, physical, event.clone(), false),
            [Event::Key(event)]
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

    /// The events delivered for `event`, by `translator`.
    fn events(translator: &mut Translator, event: WindowEvent) -> Vec<Event> {
        let mut events = Vec::new();
        translator.translate(event, |event| events.push(event));
        events
    }

    fn changed(state: ModifiersState) -> WindowEvent {
        WindowEvent::ModifiersChanged(state.into())
    }

    fn held(shift: bool, control: bool) -> Event {
        Event::ModifiersChanged(Modifiers {
            shift,
            control,
            ..Modifiers::default()
        })
    }

    #[test]
    fn each_change_of_the_modifiers_is_reported_once() {
        let mut translator = Translator::default();
        let control = ModifiersState::CONTROL;
        assert_eq!(
            events(&mut translator, changed(control)),
            [held(false, true)]
        );
        // X11 reports the modifiers again when the window gains the focus, changed or not.
        assert_eq!(events(&mut translator, changed(control)), []);
        assert_eq!(
            events(&mut translator, changed(control | ModifiersState::SHIFT)),
            [held(true, true)]
        );
        assert_eq!(
            events(&mut translator, changed(ModifiersState::empty())),
            [held(false, false)]
        );
    }

    #[test]
    fn modifiers_still_held_are_released_when_the_focus_is_lost() {
        let mut translator = Translator::default();
        events(&mut translator, changed(ModifiersState::SHIFT));
        assert_eq!(
            events(&mut translator, WindowEvent::Focused(false)),
            [held(false, false), Event::Focused(false)]
        );
        // The release that winit reports itself on every platform then changes nothing.
        assert_eq!(
            events(&mut translator, changed(ModifiersState::empty())),
            []
        );
    }
}
