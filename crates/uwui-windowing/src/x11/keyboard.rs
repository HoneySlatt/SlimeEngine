//! The keyboard on X11
//! (`docs/adr/0313-x11s-keyboard-through-xinput2-and-libxkbcommon-x11-its-cursor-themed-its-scale-from-xft-dpi.md`):
//! the server's keymap, read through libxkbcommon-x11 and followed through XKB's notices; the
//! keys, through XInput2's events of the master keyboards; the focus, through XInput2's too.
//!
//! - *The keymap*: the core keyboard's, as the server holds it, read again when XKB says that it
//!   changed (`NewKeyboardNotify`, `MapNotify`), as `setxkbmap` or a tool that maps a keysym for a
//!   while changes it. Its keys are translated by Linux's code of the keyboard (`crate::xkb`),
//!   which counts from X11's keycode less 8.
//! - *The modifiers and the layout*: XKB's `StateNotify`, depressed, latched and locked apart.
//! - *The repeat*: the server's own. XInput2 marks a key held's repeats and sends no release
//!   between them, so the layer keeps no timer of its own, as it does on Wayland.
//! - *The focus*: the window holds it when the server's focus is the window itself, as XInput2's
//!   `FocusIn` and `FocusOut` tell; or when the server's focus follows the pointer
//!   (`PointerRoot`), as where no window manager gives it, and the pointer lies over the window:
//!   XInput2's `Enter` then says that the window has the focus, and `Leave` takes it back. Focus
//!   events of the detail `Pointer` tell the second way too.
//!
//! It knows no clock: the layer times the inputs that it gives.

use std::ptr::NonNull;

use slime_input::{ButtonState, Modifiers};
use x11rb::connection::RequestConnection;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::xcb_ffi::XCBConnection;

use crate::Input;
use crate::xkb::{self as linux, Keyboard, X11Connection};

/// X11's keycodes count from Linux's codes plus 8.
const KEYCODE_OFFSET: u32 = 8;

/// The keyboard of the window: its keymap and state, the modifiers last told, the focus.
pub(super) struct Keys {
    /// None where libxkbcommon or libxkbcommon-x11 does not load, or the server gives no XKB: the
    /// window takes no key, but its focus.
    keyboard: Option<Keyboard>,
    /// XKB's id of the server's core keyboard, which every master keyboard's keys follow.
    device: i32,
    modifiers: Modifiers,
    /// Whether the server's focus is the window.
    window_focus: bool,
    /// Whether the server's focus follows the pointer and the pointer lies over the window.
    pointer_focus: bool,
}

/// What told of the focus, as XInput2's events give it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Focus {
    /// `FocusIn` or `FocusOut` on the window: the server's focus given or taken; `pointer` for the
    /// detail `Pointer`, the server's focus following the pointer into the window or out of it.
    Changed { given: bool, pointer: bool },
    /// The pointer entered the window, the server saying whether that gives it the keyboard.
    Entered { focus: bool },
    /// The pointer left the window.
    Left,
}

impl Keys {
    /// The keyboard of the server that `connection` reaches: its keymap read, XKB's notices of
    /// its changes selected. Without libxkbcommon, libxkbcommon-x11 or XKB, a keyboard that gives
    /// no key, which a warning tells.
    pub(super) fn new(connection: &XCBConnection) -> Self {
        Self::open(connection).unwrap_or_else(|error| {
            tracing::warn!(%error, "the window takes no key");
            Self::without_keyboard()
        })
    }

    fn open(connection: &XCBConnection) -> Result<Self, String> {
        let mut keyboard = Keyboard::new(&linux::locale()).map_err(|error| error.to_string())?;
        let raw = raw_connection(connection)?;
        let device = linux::x11_core_keyboard(raw).map_err(|error| error.to_string())?;
        // libxkbcommon-x11 asked for XKB through libxcb alone: x11rb learns its events' numbers
        // here, without which it would not read them.
        if connection
            .extension_information(xkb::X11_EXTENSION_NAME)
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Err("the X server gives no XKB".to_owned());
        }
        let parts = xkb::MapPart::KEY_TYPES
            | xkb::MapPart::KEY_SYMS
            | xkb::MapPart::MODIFIER_MAP
            | xkb::MapPart::EXPLICIT_COMPONENTS
            | xkb::MapPart::KEY_ACTIONS
            | xkb::MapPart::KEY_BEHAVIORS
            | xkb::MapPart::VIRTUAL_MODS
            | xkb::MapPart::VIRTUAL_MOD_MAP;
        connection
            .xkb_select_events(
                xkb::DeviceSpec::try_from(device).map_err(|error| error.to_string())?,
                xkb::EventType::from(0_u16),
                xkb::EventType::NEW_KEYBOARD_NOTIFY
                    | xkb::EventType::MAP_NOTIFY
                    | xkb::EventType::STATE_NOTIFY,
                parts,
                parts,
                &xkb::SelectEventsAux::new(),
            )
            .map_err(|error| error.to_string())?;
        keyboard
            .set_x11_keymap(raw, device)
            .map_err(|error| error.to_string())?;
        Ok(Self::with_keyboard(keyboard, device))
    }

    /// A keyboard of `keyboard`, the server's core keyboard `device`.
    fn with_keyboard(keyboard: Keyboard, device: i32) -> Self {
        let modifiers = keyboard.modifiers();
        Self {
            keyboard: Some(keyboard),
            device,
            modifiers,
            window_focus: false,
            pointer_focus: false,
        }
    }

    fn without_keyboard() -> Self {
        Self {
            keyboard: None,
            device: -1,
            modifiers: Modifiers::default(),
            window_focus: false,
            pointer_focus: false,
        }
    }

    /// Whether XKB's notice of device `device` is the core keyboard's, which this one follows.
    pub(super) fn follows(&self, device: u8) -> bool {
        self.keyboard.is_some() && i32::from(device) == self.device
    }

    /// Reads the keymap of the server's core keyboard again, which XKB said changed: the modifiers
    /// held, if they changed with it. A keymap that cannot be read leaves the last.
    pub(super) fn keymap_changed(&mut self, connection: &XCBConnection) -> Option<Input> {
        let keyboard = self.keyboard.as_mut()?;
        let read = raw_connection(connection).and_then(|raw| {
            keyboard
                .set_x11_keymap(raw, self.device)
                .map_err(|error| error.to_string())
        });
        if let Err(error) = read {
            tracing::warn!(%error, "the X server's new keymap is not read: keys keep the last");
            return None;
        }
        let held = keyboard.modifiers();
        self.modifiers_now(held)
    }

    /// XKB's state of the modifiers and the layout: `modifiers` and `layouts` depressed, latched
    /// and locked. The modifiers held, if they changed.
    pub(super) fn state(&mut self, modifiers: [u32; 3], layouts: [u32; 3]) -> Option<Input> {
        let held = self.keyboard.as_mut()?.set_modifiers(modifiers, layouts);
        self.modifiers_now(held)
    }

    fn modifiers_now(&mut self, held: Modifiers) -> Option<Input> {
        (held != self.modifiers).then(|| {
            self.modifiers = held;
            Input::Modifiers(held)
        })
    }

    /// The key of X11's keycode `keycode` going down, up or repeating, as XInput2 gives it.
    /// Nothing without a keymap, or for a keycode below X11's first, 8, which no key has.
    pub(super) fn key(&mut self, keycode: u32, state: ButtonState, repeat: bool) -> Option<Input> {
        let code = keycode.checked_sub(KEYCODE_OFFSET)?;
        let event = self.keyboard.as_mut()?.key(code, state, repeat);
        Some(Input::Key(event))
    }

    /// What `event` makes of the window's keyboard focus: told once for each change. A sequence
    /// of dead keys in progress is forgotten with the focus.
    pub(super) fn focus(&mut self, event: Focus) -> Option<Input> {
        let before = self.focused();
        match event {
            Focus::Changed {
                given,
                pointer: true,
            } => self.pointer_focus = given,
            Focus::Changed { given, .. } => self.window_focus = given,
            Focus::Entered { focus } => self.pointer_focus = focus && !self.window_focus,
            Focus::Left => self.pointer_focus = false,
        }
        let given = self.focused();
        if given == before {
            return None;
        }
        if !given && let Some(keyboard) = &mut self.keyboard {
            keyboard.forget_sequence();
        }
        Some(Input::Focus(given))
    }

    fn focused(&self) -> bool {
        self.window_focus || self.pointer_focus
    }
}

/// libxcb's connection under x11rb's, for libxkbcommon-x11.
fn raw_connection(connection: &XCBConnection) -> Result<X11Connection, String> {
    NonNull::new(connection.get_raw_xcb_connection())
        .map(X11Connection)
        .ok_or_else(|| "libxcb's connection is null".to_owned())
}

/// XKB's mask of modifiers as libxkbcommon takes it.
pub(super) fn mask(modifiers: x11rb::protocol::xproto::ModMask) -> u32 {
    u32::from(u16::from(modifiers))
}

/// XKB's layout, which a latch or a base may give as negative, relative, as libxkbcommon takes it:
/// the same bits, which it brings within the keymap's layouts itself.
pub(super) fn layout(group: i16) -> u32 {
    i32::from(group).cast_unsigned()
}

#[cfg(test)]
mod tests {
    use slime_input::{Key, KeyCode};

    use super::*;

    /// A keyboard of the XKB rules' layout `layout`, as the server's core keyboard 3.
    fn keys(layout: &str) -> Keys {
        let mut keyboard = Keyboard::new("C").expect("libxkbcommon loads");
        keyboard
            .set_test_layout(layout)
            .expect("the layout compiles");
        Keys::with_keyboard(keyboard, 3)
    }

    fn key(input: Option<Input>) -> (KeyCode, Key, Option<String>, bool) {
        match input {
            Some(Input::Key(event)) => (event.physical, event.logical, event.text, event.repeat),
            other => panic!("not a key: {other:?}"),
        }
    }

    #[test]
    fn a_keycode_is_linuxs_code_plus_eight() {
        let mut keys = keys("us");
        // X11's keycode 38 is Linux's 30, the key A.
        assert_eq!(
            key(keys.key(38, ButtonState::Pressed, false)),
            (
                KeyCode::KeyA,
                Key::Character("a".to_owned()),
                Some("a".to_owned()),
                false
            )
        );
        assert!(
            key(keys.key(38, ButtonState::Pressed, true)).3,
            "a repeat that XInput2 marks is told as one"
        );
        // Below 8, no key.
        assert_eq!(keys.key(7, ButtonState::Pressed, false), None);
    }

    #[test]
    fn xkbs_state_changes_the_modifiers_and_the_layout_told_once() {
        let mut keys = keys("us,fr");
        // Shift, the keymap's first modifier.
        let Some(Input::Modifiers(held)) = keys.state([1, 0, 0], [0; 3]) else {
            panic!("Shift is not told");
        };
        assert!(held.shift);
        assert_eq!(keys.state([1, 0, 0], [0; 3]), None, "told twice");
        assert_eq!(
            key(keys.key(38, ButtonState::Pressed, false)).2.as_deref(),
            Some("A")
        );
        // The French layout locked: no modifier changes, but `q`'s place types `a`.
        assert!(keys.state([0, 0, 0], [0, 0, 1]).is_some());
        assert_eq!(keys.state([0, 0, 0], [0, 0, 1]), None);
        assert_eq!(
            key(keys.key(24, ButtonState::Pressed, false)).2.as_deref(),
            Some("a")
        );
    }

    const GIVEN: Focus = Focus::Changed {
        given: true,
        pointer: false,
    };
    const TAKEN: Focus = Focus::Changed {
        given: false,
        pointer: false,
    };

    #[test]
    fn the_focus_is_told_at_each_change_alone() {
        let mut keys = keys("us");
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
        assert_eq!(keys.focus(GIVEN), None);
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        assert_eq!(keys.focus(TAKEN), None);
    }

    #[test]
    fn the_focus_that_follows_the_pointer_comes_and_goes_with_it() {
        let mut keys = keys("us");
        // The server's focus follows the pointer: entering gives the keyboard, leaving takes it.
        assert_eq!(
            keys.focus(Focus::Entered { focus: true }),
            Some(Input::Focus(true))
        );
        assert_eq!(keys.focus(Focus::Left), Some(Input::Focus(false)));
        // A window that another holds the focus of: entering gives nothing.
        assert_eq!(keys.focus(Focus::Entered { focus: false }), None);
        assert_eq!(keys.focus(Focus::Left), None);
        // Focus events of the detail `Pointer` tell the same.
        let pointer = |given| Focus::Changed {
            given,
            pointer: true,
        };
        assert_eq!(keys.focus(pointer(true)), Some(Input::Focus(true)));
        assert_eq!(keys.focus(pointer(false)), Some(Input::Focus(false)));
    }

    #[test]
    fn the_windows_own_focus_outlasts_the_pointer() {
        let mut keys = keys("us");
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
        // The pointer passing over the focused window changes nothing.
        assert_eq!(keys.focus(Focus::Entered { focus: true }), None);
        assert_eq!(keys.focus(Focus::Left), None);
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        // Focused by the pointer, then given the focus itself, then the pointer leaving: held.
        keys.focus(Focus::Entered { focus: true });
        assert_eq!(keys.focus(GIVEN), None);
        assert_eq!(keys.focus(Focus::Left), None);
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        // The pointer entering the focused window, which then loses the focus to another while
        // the pointer stays: lost, the server's focus not following the pointer.
        keys.focus(GIVEN);
        assert_eq!(keys.focus(Focus::Entered { focus: true }), None);
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
    }

    #[test]
    fn a_dead_key_composes_nothing_after_the_focus_left() {
        if !linux::composing_from_the_tests_file(
            "x11::keyboard::tests::a_dead_key_composes_nothing_after_the_focus_left",
        ) {
            return;
        }
        let mut keyboard = Keyboard::new("en_US.UTF-8").expect("libxkbcommon loads");
        keyboard.set_test_layout("fr").expect("the layout compiles");
        let mut keys = Keys::with_keyboard(keyboard, 3);
        keys.focus(GIVEN);
        // `^`, dead on a French keyboard, at X11's keycode 34; then `e`, at 26.
        assert_eq!(
            key(keys.key(34, ButtonState::Pressed, false)).1,
            Key::Dead(Some('^'))
        );
        assert_eq!(
            key(keys.key(26, ButtonState::Pressed, false)).2.as_deref(),
            Some("ê")
        );
        keys.key(34, ButtonState::Pressed, false);
        keys.focus(TAKEN);
        keys.focus(GIVEN);
        assert_eq!(
            key(keys.key(26, ButtonState::Pressed, false)).2.as_deref(),
            Some("e")
        );
    }

    #[test]
    fn without_a_keyboard_only_the_focus_is_told() {
        let mut keys = Keys::without_keyboard();
        assert!(!keys.follows(0));
        assert_eq!(keys.key(38, ButtonState::Pressed, false), None);
        assert_eq!(keys.state([1, 0, 0], [0; 3]), None);
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
    }

    #[test]
    fn xkbs_notices_are_followed_for_the_core_keyboard_alone() {
        let keys = keys("us");
        assert!(keys.follows(3));
        assert!(!keys.follows(4));
    }

    #[test]
    fn a_relative_layout_keeps_its_bits() {
        assert_eq!(layout(1), 1);
        assert_eq!(layout(-1), u32::MAX);
        assert_eq!(mask(x11rb::protocol::xproto::ModMask::SHIFT), 1);
    }
}
