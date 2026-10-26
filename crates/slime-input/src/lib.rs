#![doc = include_str!("../README.md")]

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
    ///
    /// The press of a character key right after a dead key reports what the two keys make
    /// together, but its release does not. When they combine, the press is the composed character
    /// (`"ê"` for `^` then `e` on a French layout) and the release the plain one (`"e"`). When
    /// they do not, the press is [`Key::Unidentified`] on Wayland and X11, and the plain
    /// character on Windows (`"q"` for `^` then `q`); the release is the plain character. The
    /// logical values of a press and of its release can therefore differ: use
    /// [`physical`](Self::physical) to track which keys are down.
    pub logical: Key,
    /// The text the key produces when pressed, if any. A character key gives its character, with
    /// dead keys applied: pressing `^` then `e` on a French layout gives the text `"ê"` on the
    /// second press. When the dead key and the next character key do not combine, that press gives
    /// no text on Wayland and X11, and both characters on Windows (`"^q"` for `^` then `q`). Four
    /// named keys give a control character: Enter `"\r"`, Tab `"\t"` (with Shift too), Backspace
    /// `"\u{8}"` and Escape `"\u{1b}"`. Space gives `" "`, or after a dead key the dead key's
    /// character (`"^"` for `^` then Space). The other named keys, such as Delete, the arrows and
    /// the modifiers, give `None`, as does a release. The text of named keys is the same on every
    /// platform, after a dead key too.
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
    /// A key that the engine does not name. Use [`KeyEvent::physical`] to identify it. On Wayland
    /// and X11, the press of a character key after a dead key that it does not combine with is
    /// reported so too (see [`KeyEvent::logical`]).
    Unidentified,
}

/// Declares a public enum of unit variants, plus an optional variant of its own, and its variants'
/// names as text and back. The variants are those that [`for_each_key_code`] or
/// [`for_each_named_key`] list, so that the platform layer translates its windowing library's
/// values without listing them again.
macro_rules! named_enum {
    (
        $(#[$meta:meta])*
        enum $engine:ident, variant doc $doc:literal;
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

        impl $engine {
            /// The variant's name, as the code writes it: how the tools of the development channel,
            /// and egui, name keys.
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant),)*
                    $(Self::$extra => stringify!($extra),)?
                }
            }

            /// The variant whose [`name`](Self::name) is `name`, if one is.
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $(stringify!($variant) => Some(Self::$variant),)*
                    $(stringify!($extra) => Some(Self::$extra),)?
                    _ => None,
                }
            }
        }
    };
}

/// Calls the macro `$callback` with the names of the [`KeyCode`]s but
/// [`KeyCode::Unidentified`], as `$callback! { Backquote Backslash ... }`: the enum is declared
/// from them, and the platform layer translates its windowing library's codes into them by name,
/// the list written once.
#[macro_export]
macro_rules! for_each_key_code {
    ($callback:ident) => {
        $callback! {
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
    };
}

/// Calls the macro `$callback` with the names of the [`NamedKey`]s, as
/// `$callback! { Alt AltGraph ... }`, as [`for_each_key_code`] does for the key codes.
#[macro_export]
macro_rules! for_each_named_key {
    ($callback:ident) => {
        $callback! {
            Alt AltGraph CapsLock Control Fn FnLock NumLock ScrollLock Shift Super Meta Hyper Enter Tab
            Space ArrowDown ArrowLeft ArrowRight ArrowUp End Home PageDown PageUp Backspace Delete Insert
            Escape ContextMenu Pause PrintScreen Compose Copy Cut Paste Undo Redo AudioVolumeDown
            AudioVolumeUp AudioVolumeMute MediaPlayPause MediaStop MediaTrackNext MediaTrackPrevious F1 F2
            F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 F13 F14 F15 F16 F17 F18 F19 F20 F21 F22 F23 F24
        }
    };
}

macro_rules! key_code_enum {
    ($($variant:ident)*) => {
        named_enum! {
            /// The position of a key on the keyboard, independent of the layout.
            ///
            /// Variant names follow the `code` values of the
            /// [W3C UI Events](https://www.w3.org/TR/uievents-code/) specification, as the
            /// windowing library, winit, reports them: they name a key by the character it produces
            /// at that position on a US keyboard (`KeyQ`, `Digit1`, `Semicolon`) or by its function
            /// (`Escape`, `ShiftLeft`, `Numpad0`). Three differences come from winit: the keys the
            /// specification calls `MetaLeft` and `MetaRight` are `SuperLeft` and `SuperRight`
            /// here, the key it calls `Super` is reported as `Unidentified`, and `Meta` is a winit
            /// addition that the specification does not define.
            enum KeyCode, variant doc "The key named ";
            variants { $($variant)* }
            extra {
                /// A key that the platform reports without a code the engine knows.
                Unidentified
            }
        }
    };
}

for_each_key_code!(key_code_enum);

macro_rules! named_key_enum {
    ($($variant:ident)*) => {
        named_enum! {
            /// A key that has a name rather than a character, as seen through the current layout.
            ///
            /// Only the named keys of usual keyboards are listed; other named keys, such as those
            /// of remote controls, are reported as [`Key::Unidentified`], and
            /// [`KeyEvent::physical`] still identifies them.
            enum NamedKey, variant doc "The named key ";
            variants { $($variant)* }
        }
    };
}

for_each_named_key!(named_key_enum);

/// The modifier keys held, as a whole: each is held when either of its keys, left or right, is
/// down, or a key that the keyboard layout turns into it. AltGr counts as neither Control nor Alt.
///
/// A shortcut such as Ctrl+S is a press of S while the modifiers last reported (by
/// `slime_platform::Event::ModifiersChanged`) hold Control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Modifiers {
    /// Shift.
    pub shift: bool,
    /// Control (Ctrl).
    pub control: bool,
    /// Alt.
    pub alt: bool,
    /// The key with the system's logo: the Windows key, or Super on Linux.
    pub super_key: bool,
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
///
/// The unit depends on the platform as much as on the device. Wayland reports the notches of a
/// wheel in [`Lines`](Self::Lines), and touchpads and the movement of a high-resolution wheel
/// between two notches in [`Pixels`](Self::Pixels). X11 and Windows report every device in lines,
/// touchpads included.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
    /// In lines (or rows and columns). A notch of a usual mouse wheel is one line. The value can
    /// be a fraction: on X11 and Windows, a touchpad or a high-resolution wheel reports many small
    /// fractions of a line per movement, so one event is not one notch.
    Lines {
        /// Horizontal lines.
        x: f32,
        /// Vertical lines.
        y: f32,
    },
    /// In physical pixels. Only Wayland reports them.
    Pixels {
        /// Horizontal pixels.
        x: f64,
        /// Vertical pixels.
        y: f64,
    },
}

/// What an input method does while the user composes text through it, as Chinese, Japanese and
/// Korean are typed: the keys that it reads then give no text of their own.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ime {
    /// The input method is on for the window: compositions may follow.
    Enabled,
    /// The text being composed, not yet committed, which replaces the one before: empty when the
    /// composition ends without a commit.
    Preedit {
        /// The text.
        text: String,
        /// Where the method shows its own cursor, as bytes of the text, its start and its end;
        /// none to hide it.
        cursor: Option<(usize, usize)>,
    },
    /// Text committed, in place of the text being composed.
    Commit(String),
    /// The input method is off for the window.
    Disabled,
}

/// The shape of the pointer over the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorShape {
    /// The system's arrow.
    #[default]
    Default,
    /// Over text that can be selected or edited: the I-beam.
    Text,
    /// Over what is dragged left and right, as a field of numbers.
    ResizeHorizontal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_found_again_by_its_name() {
        for key in [
            KeyCode::KeyW,
            KeyCode::Escape,
            KeyCode::F35,
            KeyCode::Unidentified,
        ] {
            assert_eq!(KeyCode::from_name(key.name()), Some(key));
        }
        assert_eq!(KeyCode::KeyW.name(), "KeyW");
        assert_eq!(KeyCode::from_name("W"), None);
        assert_eq!(NamedKey::from_name("ArrowUp"), Some(NamedKey::ArrowUp));
        assert_eq!(NamedKey::Delete.name(), "Delete");
    }
}
