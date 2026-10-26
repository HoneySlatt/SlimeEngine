//! The translation of winit's keyboard and mouse input into the engine's vocabulary of input,
//! `slime-input`.

use winit::event::{ElementState, MouseScrollDelta};
use winit::keyboard::{
    Key as WinitKey, KeyCode as WinitKeyCode, NamedKey as WinitNamedKey, PhysicalKey,
};

use slime_input::{
    ButtonState, CursorShape, Ime, Key, KeyCode, KeyEvent, Modifiers, MouseButton, NamedKey,
    ScrollDelta,
};

/// Declares `translate_key_code`, which translates winit's key code into the engine's, by the
/// names that `slime-input` lists once (`None` for a code the engine does not list).
macro_rules! translate_key_code {
    ($($variant:ident)*) => {
        fn translate_key_code(value: WinitKeyCode) -> Option<KeyCode> {
            match value {
                $(WinitKeyCode::$variant => Some(KeyCode::$variant),)*
                _ => None,
            }
        }
    };
}

slime_input::for_each_key_code!(translate_key_code);

/// Declares `translate_named_key`, as `translate_key_code` for the named keys.
macro_rules! translate_named_key {
    ($($variant:ident)*) => {
        fn translate_named_key(value: WinitNamedKey) -> Option<NamedKey> {
            match value {
                $(WinitNamedKey::$variant => Some(NamedKey::$variant),)*
                _ => None,
            }
        }
    };
}

slime_input::for_each_named_key!(translate_named_key);

pub(crate) fn key_event(event: winit::event::KeyEvent) -> KeyEvent {
    KeyEvent {
        physical: key_code(event.physical_key),
        logical: key(&event.logical_key),
        text: key_text(event.state, &event.logical_key, event.text.as_deref()),
        state: button_state(event.state),
        repeat: event.repeat,
    }
}

/// The text of a key event: what a press produces, nothing for a release. The windowing library
/// reports a release with the same text as the press on Wayland and X11, but not on Windows.
///
/// A named key other than Space gives the text of the windowing library's own table (Enter,
/// Tab, Backspace and Escape have one), which its Windows backend already uses when Ctrl is held,
/// rather than the platform's text. On Wayland and X11, that text comes from libxkbcommon, which
/// gives Delete `"\u{7f}"` and Clear `"\u{b}"` and gives Shift+Tab nothing, where Windows gives
/// Delete and Clear nothing and Shift+Tab `"\t"`. Space keeps the platform's text, because after
/// a dead key both platforms give the dead key's character rather than `" "`.
fn key_text(state: ElementState, logical: &WinitKey, text: Option<&str>) -> Option<String> {
    match (state, logical) {
        (ElementState::Released, _) => None,
        (ElementState::Pressed, WinitKey::Named(named)) if *named != WinitNamedKey::Space => {
            named.to_text().map(str::to_owned)
        }
        (ElementState::Pressed, _) => text.map(str::to_owned),
    }
}

fn key_code(key: PhysicalKey) -> KeyCode {
    match key {
        PhysicalKey::Code(code) => translate_key_code(code).unwrap_or(KeyCode::Unidentified),
        PhysicalKey::Unidentified(_) => KeyCode::Unidentified,
    }
}

fn key(key: &WinitKey) -> Key {
    match key {
        WinitKey::Character(character) => Key::Character(character.to_string()),
        WinitKey::Named(named) => translate_named_key(*named).map_or(Key::Unidentified, Key::Named),
        WinitKey::Dead(character) => Key::Dead(*character),
        WinitKey::Unidentified(_) => Key::Unidentified,
    }
}

pub(crate) fn button_state(state: ElementState) -> ButtonState {
    match state {
        ElementState::Pressed => ButtonState::Pressed,
        ElementState::Released => ButtonState::Released,
    }
}

pub(crate) fn modifiers(state: winit::keyboard::ModifiersState) -> Modifiers {
    // Field by field: the type may gain modifiers, so that no literal names all of them.
    let mut modifiers = Modifiers::default();
    modifiers.shift = state.shift_key();
    modifiers.control = state.control_key();
    modifiers.alt = state.alt_key();
    modifiers.super_key = state.super_key();
    modifiers
}

pub(crate) fn mouse_button(button: winit::event::MouseButton) -> MouseButton {
    use winit::event::MouseButton as Winit;
    match button {
        Winit::Left => MouseButton::Left,
        Winit::Right => MouseButton::Right,
        Winit::Middle => MouseButton::Middle,
        Winit::Back => MouseButton::Back,
        Winit::Forward => MouseButton::Forward,
        Winit::Other(number) => MouseButton::Other(number),
    }
}

pub(crate) fn scroll_delta(delta: MouseScrollDelta) -> ScrollDelta {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines { x, y },
        MouseScrollDelta::PixelDelta(position) => ScrollDelta::Pixels {
            x: position.x,
            y: position.y,
        },
    }
}

/// The icon that winit shows for the engine's cursor shape.
pub(crate) fn cursor_icon(shape: CursorShape) -> winit::window::CursorIcon {
    match shape {
        CursorShape::Default => winit::window::CursorIcon::Default,
        CursorShape::Text => winit::window::CursorIcon::Text,
        CursorShape::ResizeHorizontal => winit::window::CursorIcon::EwResize,
        CursorShape::ResizeVertical => winit::window::CursorIcon::NsResize,
    }
}

/// What winit's input method event is, in the engine's terms.
pub(crate) fn ime(ime: winit::event::Ime) -> Ime {
    match ime {
        winit::event::Ime::Enabled => Ime::Enabled,
        winit::event::Ime::Preedit(text, cursor) => Ime::Preedit { text, cursor },
        winit::event::Ime::Commit(text) => Ime::Commit(text),
        winit::event::Ime::Disabled => Ime::Disabled,
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::PhysicalPosition;
    use winit::keyboard::{NativeKey, NativeKeyCode, SmolStr};

    use super::*;

    #[test]
    fn physical_keys_keep_their_w3c_name() {
        assert_eq!(
            key_code(PhysicalKey::Code(WinitKeyCode::KeyQ)),
            KeyCode::KeyQ
        );
        assert_eq!(
            key_code(PhysicalKey::Code(WinitKeyCode::Escape)),
            KeyCode::Escape
        );
        assert_eq!(
            key_code(PhysicalKey::Code(WinitKeyCode::NumpadEnter)),
            KeyCode::NumpadEnter
        );
        assert_eq!(key_code(PhysicalKey::Code(WinitKeyCode::F35)), KeyCode::F35);
        assert_eq!(
            key_code(PhysicalKey::Unidentified(NativeKeyCode::Xkb(999))),
            KeyCode::Unidentified
        );
    }

    #[test]
    fn modifiers_are_translated_one_by_one() {
        use winit::keyboard::ModifiersState;
        let state = ModifiersState::SHIFT | ModifiersState::SUPER;
        let mut expected = Modifiers::default();
        expected.shift = true;
        expected.super_key = true;
        assert_eq!(modifiers(state), expected);
        let state = ModifiersState::CONTROL | ModifiersState::ALT;
        let mut expected = Modifiers::default();
        expected.control = true;
        expected.alt = true;
        assert_eq!(modifiers(state), expected);
    }

    #[test]
    fn logical_keys_are_translated() {
        assert_eq!(
            key(&WinitKey::Character(SmolStr::new("a"))),
            Key::Character("a".to_owned())
        );
        assert_eq!(
            key(&WinitKey::Named(WinitNamedKey::Enter)),
            Key::Named(NamedKey::Enter)
        );
        assert_eq!(key(&WinitKey::Dead(Some('^'))), Key::Dead(Some('^')));
        assert_eq!(
            key(&WinitKey::Unidentified(NativeKey::Xkb(999))),
            Key::Unidentified
        );
    }

    #[test]
    fn named_keys_the_engine_does_not_list_are_unidentified() {
        assert_eq!(
            key(&WinitKey::Named(WinitNamedKey::TVPower)),
            Key::Unidentified
        );
    }

    #[test]
    fn a_release_carries_no_text() {
        let a = WinitKey::Character(SmolStr::new("a"));
        assert_eq!(
            key_text(ElementState::Pressed, &a, Some("a")),
            Some("a".to_owned())
        );
        assert_eq!(key_text(ElementState::Released, &a, Some("a")), None);
        assert_eq!(
            key_text(
                ElementState::Released,
                &WinitKey::Named(WinitNamedKey::Enter),
                Some("\r")
            ),
            None
        );
        assert_eq!(key_text(ElementState::Pressed, &a, None), None);
    }

    #[test]
    fn named_keys_give_the_same_text_on_every_platform() {
        let press = |named, text| key_text(ElementState::Pressed, &WinitKey::Named(named), text);
        // The text Wayland and X11 report for Delete, Clear and Shift+Tab, where Windows reports
        // none, none and "\t".
        assert_eq!(press(WinitNamedKey::Delete, Some("\u{7f}")), None);
        assert_eq!(press(WinitNamedKey::Clear, Some("\u{b}")), None);
        assert_eq!(press(WinitNamedKey::Tab, None), Some("\t".to_owned()));
        assert_eq!(
            press(WinitNamedKey::Enter, Some("\r")),
            Some("\r".to_owned())
        );
        assert_eq!(press(WinitNamedKey::ArrowLeft, None), None);
        // Space keeps the platform's text: after a dead key, it gives the dead key's character.
        assert_eq!(press(WinitNamedKey::Space, Some(" ")), Some(" ".to_owned()));
        assert_eq!(press(WinitNamedKey::Space, Some("^")), Some("^".to_owned()));
    }

    #[test]
    fn mouse_buttons_and_states_are_translated() {
        use winit::event::MouseButton as Winit;
        assert_eq!(mouse_button(Winit::Left), MouseButton::Left);
        assert_eq!(mouse_button(Winit::Right), MouseButton::Right);
        assert_eq!(mouse_button(Winit::Middle), MouseButton::Middle);
        assert_eq!(mouse_button(Winit::Back), MouseButton::Back);
        assert_eq!(mouse_button(Winit::Forward), MouseButton::Forward);
        assert_eq!(mouse_button(Winit::Other(7)), MouseButton::Other(7));
        assert_eq!(button_state(ElementState::Pressed), ButtonState::Pressed);
        assert_eq!(button_state(ElementState::Released), ButtonState::Released);
    }

    #[test]
    fn scroll_deltas_keep_their_unit() {
        assert_eq!(
            scroll_delta(MouseScrollDelta::LineDelta(0.0, -1.0)),
            ScrollDelta::Lines { x: 0.0, y: -1.0 }
        );
        assert_eq!(
            scroll_delta(MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                3.5, 12.0
            ))),
            ScrollDelta::Pixels { x: 3.5, y: 12.0 }
        );
    }
}
