//! Keyboard and mouse input, and its translation from winit's types.

use winit::event::{ElementState, MouseScrollDelta};
use winit::keyboard::{
    Key as WinitKey, KeyCode as WinitKeyCode, NamedKey as WinitNamedKey, PhysicalKey,
};

/// Whether a key or a mouse button went down or up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ButtonState {
    /// The key or button went down.
    Pressed,
    /// The key or button went up.
    Released,
}

/// A key going down or up.
///
/// A key is described twice, because games need both: [`physical`](Self::physical) says where the
/// key is, whatever the keyboard layout, which suits movement keys; [`logical`](Self::logical)
/// says what the key means with the current layout, which suits shortcuts and text. On a French
/// AZERTY keyboard, the key at the position of Q on a US keyboard has the physical code
/// [`KeyCode::KeyQ`] and the logical value `Key::Character("a")`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// The position of the key on the keyboard, independent of the layout.
    pub physical: KeyCode,
    /// What the key means with the current layout and modifiers.
    pub logical: Key,
    /// The text the key produces when pressed, if any. A character key gives its character, with
    /// dead keys applied: pressing `^` then `e` on a French layout gives the text `"ê"` on the
    /// second press. Four named keys give a control character: Enter `"\r"`, Tab `"\t"` (with
    /// Shift too), Backspace `"\u{8}"` and Escape `"\u{1b}"`. Space gives `" "`, or after a dead
    /// key the dead key's character (`"^"` for `^` then Space). The other named keys, such as
    /// Delete, the arrows and the modifiers, give `None`, as does a release. This holds on every
    /// platform.
    pub text: Option<String>,
    /// Whether the key went down or up.
    pub state: ButtonState,
    /// Whether this press comes from the key being held down (keyboard auto-repeat) rather than
    /// from a new press.
    pub repeat: bool,
}

/// What a key means with the current keyboard layout.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A key that produces a character: the character with the current layout and the modifiers
    /// that change characters (Shift, AltGr), but not Control. For example `"a"`, `"A"` or `"é"`.
    Character(String),
    /// A key with a name rather than a character.
    Named(NamedKey),
    /// A dead key, which modifies the next key pressed (for example `^` before `e`). It holds the
    /// character the dead key would add on its own, when the platform knows it.
    Dead(Option<char>),
    /// A key that the engine does not name. Use [`KeyEvent::physical`] to identify it.
    Unidentified,
}

/// Declares a public enum whose variants have the same names as some of a winit enum's variants,
/// plus an optional variant of its own, and a function that translates winit's value into it
/// (`None` for a value the engine does not list).
macro_rules! mirror_enum {
    (
        $(#[$meta:meta])*
        enum $engine:ident mirrors $winit:ident, variant doc $doc:literal, fn $translate:ident;
        variants { $($variant:ident)* }
        $(extra { $(#[$extra_meta:meta])* $extra:ident })?
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $engine {
            $(
                #[doc = concat!($doc, "`", stringify!($variant), "`.")]
                $variant,
            )*
            $($(#[$extra_meta])* $extra,)?
        }

        fn $translate(value: $winit) -> Option<$engine> {
            match value {
                $($winit::$variant => Some($engine::$variant),)*
                _ => None,
            }
        }
    };
}

mirror_enum! {
    /// The position of a key on the keyboard, independent of the layout.
    ///
    /// Variant names follow the `code` values of the
    /// [W3C UI Events](https://www.w3.org/TR/uievents-code/) specification, as winit reports them:
    /// they name a key by the character it produces at that position on a US keyboard (`KeyQ`,
    /// `Digit1`, `Semicolon`) or by its function (`Escape`, `ShiftLeft`, `Numpad0`). Three
    /// differences come from winit: the keys the specification calls `MetaLeft` and `MetaRight`
    /// are `SuperLeft` and `SuperRight` here, the key it calls `Super` is reported as
    /// `Unidentified`, and `Meta` is a winit addition that the specification does not define.
    enum KeyCode mirrors WinitKeyCode, variant doc "The key named ",
    fn translate_key_code;
    variants {
        Backquote Backslash BracketLeft BracketRight Comma Digit0 Digit1 Digit2 Digit3 Digit4 Digit5
        Digit6 Digit7 Digit8 Digit9 Equal IntlBackslash IntlRo IntlYen KeyA KeyB KeyC KeyD KeyE KeyF
        KeyG KeyH KeyI KeyJ KeyK KeyL KeyM KeyN KeyO KeyP KeyQ KeyR KeyS KeyT KeyU KeyV KeyW KeyX KeyY
        KeyZ Minus Period Quote Semicolon Slash AltLeft AltRight Backspace CapsLock ContextMenu
        ControlLeft ControlRight Enter SuperLeft SuperRight ShiftLeft ShiftRight Space Tab Convert
        KanaMode Lang1 Lang2 Lang3 Lang4 Lang5 NonConvert Delete End Help Home Insert PageDown PageUp
        ArrowDown ArrowLeft ArrowRight ArrowUp NumLock Numpad0 Numpad1 Numpad2 Numpad3 Numpad4 Numpad5
        Numpad6 Numpad7 Numpad8 Numpad9 NumpadAdd NumpadBackspace NumpadClear NumpadClearEntry
        NumpadComma NumpadDecimal NumpadDivide NumpadEnter NumpadEqual NumpadHash NumpadMemoryAdd
        NumpadMemoryClear NumpadMemoryRecall NumpadMemoryStore NumpadMemorySubtract NumpadMultiply
        NumpadParenLeft NumpadParenRight NumpadStar NumpadSubtract Escape Fn FnLock PrintScreen
        ScrollLock Pause BrowserBack BrowserFavorites BrowserForward BrowserHome BrowserRefresh
        BrowserSearch BrowserStop Eject LaunchApp1 LaunchApp2 LaunchMail MediaPlayPause MediaSelect
        MediaStop MediaTrackNext MediaTrackPrevious Power Sleep AudioVolumeDown AudioVolumeMute
        AudioVolumeUp WakeUp Meta Hyper Turbo Abort Resume Suspend Again Copy Cut Find Open Paste Props
        Select Undo Hiragana Katakana F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 F13 F14 F15 F16 F17 F18
        F19 F20 F21 F22 F23 F24 F25 F26 F27 F28 F29 F30 F31 F32 F33 F34 F35
    }
    extra {
        /// A key that the platform reports without a code the engine knows.
        Unidentified
    }
}

mirror_enum! {
    /// A key that has a name rather than a character, as seen through the current layout.
    ///
    /// Only the named keys of usual keyboards are listed; other named keys, such as those of
    /// remote controls, are reported as [`Key::Unidentified`], and [`KeyEvent::physical`] still
    /// identifies them.
    enum NamedKey mirrors WinitNamedKey, variant doc "The named key ",
    fn translate_named_key;
    variants {
        Alt AltGraph CapsLock Control Fn FnLock NumLock ScrollLock Shift Super Meta Hyper Enter Tab
        Space ArrowDown ArrowLeft ArrowRight ArrowUp End Home PageDown PageUp Backspace Delete Insert
        Escape ContextMenu Pause PrintScreen Compose Copy Cut Paste Undo Redo AudioVolumeDown
        AudioVolumeUp AudioVolumeMute MediaPlayPause MediaStop MediaTrackNext MediaTrackPrevious F1 F2
        F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 F13 F14 F15 F16 F17 F18 F19 F20 F21 F22 F23 F24
    }
}

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// The primary button, usually the left one.
    Left,
    /// The secondary button, usually the right one.
    Right,
    /// The middle button, often the wheel.
    Middle,
    /// The "back" side button.
    Back,
    /// The "forward" side button.
    Forward,
    /// Another button, by the number the platform gives it.
    Other(u16),
}

/// How far a mouse wheel or a touchpad scrolled. Positive `y` scrolls up (content moves down),
/// positive `x` scrolls left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
    /// In lines (or rows and columns), as reported by most mouse wheels.
    Lines {
        /// Horizontal lines.
        x: f32,
        /// Vertical lines.
        y: f32,
    },
    /// In physical pixels, as reported by touchpads and some precise wheels.
    Pixels {
        /// Horizontal pixels.
        x: f64,
        /// Vertical pixels.
        y: f64,
    },
}

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
