//! The keyboard on X11
//! (`docs/adr/0313-x11s-keyboard-through-xinput2-and-libxkbcommon-x11-its-cursor-themed-its-scale-from-xft-dpi.md`):
//! the server's keymap, read through libxkbcommon-x11 and followed through XKB's notices; the
//! keys, through XInput2's events of the master keyboards; the focus, through the core protocol's
//! focus events, the server's focus read at each, and the keys themselves.
//!
//! - *The keymap*: the core keyboard's, as the server holds it, read again when XKB says that it
//!   changed (`NewKeyboardNotify`, `MapNotify`), as `setxkbmap` or a tool that maps a keysym for a
//!   while changes it. Its keys are translated by Linux's code of the keyboard (`crate::xkb`),
//!   which counts from X11's keycode less 8.
//! - *The modifiers and the layout*: each key is translated with its own event's state, the
//!   modifiers and the layout depressed, latched and locked as XInput2 gives them with the key,
//!   from before it; the keyboard then returns to the state that the server last told. The
//!   modifiers told the host come from XKB's `StateNotify`, of those parts alone, and only while
//!   the window holds the focus, and from each key's own state around it: a key typed as the focus
//!   comes keeps its modifiers, and the last key leaves the server's told. A modifier held in
//!   another window, or a click anywhere, which changes the pointer's buttons in XKB's state, never
//!   wakes the loop. As the window takes the focus, the layer reads XKB's state once, and tells the
//!   modifiers held if they changed, as on Wayland; as it loses it, it tells them released.
//! - *The repeat*: the server's own. XInput2 marks a key held's repeats and sends no release
//!   between them, so the layer keeps no timer of its own, as it does on Wayland.
//! - *The focus*: the window holds it when the server's focus is the window itself, or lies above
//!   it, following the pointer (`PointerRoot`), as where no window manager gives it, or on an
//!   ancestor such as the root window, and the pointer lies over the window: the keys then go to
//!   the window under the pointer. At each of the core protocol's `FocusIn` and `FocusOut`, which
//!   the server sends where XInput2 sends none, as for a move to an ancestor, the layer reads where
//!   the server's focus now is, rather than reading it from the events' details, which mislead
//!   between an event out and an event in. Another client's grab of the keyboard, as a menu, a
//!   screen locker or a window manager's Alt and Tab, takes the focus until it ends, though the
//!   server's focus stays. A key that reaches the window gives it the focus, which an event read
//!   late may not have told, and ends another client's grab, whose end the server may have told
//!   no one: the server sends keys only to the window that holds them. XInput2's
//!   `Enter` says whether the focus lies on the window or above it, and `Leave` that the pointer
//!   left; another client's grab of the pointer moves neither.
//!
//! It knows no clock: the layer times the inputs that it gives.

use slime_input::{ButtonState, Modifiers};
use x11rb::connection::RequestConnection;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::ModMask;
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
    /// The modifiers last told the host.
    modifiers: Modifiers,
    /// Whether the server's focus is the window itself.
    window_focus: bool,
    /// Whether the server's focus lies above the window: following the pointer, or on an
    /// ancestor.
    focus_above: bool,
    /// Whether the pointer lies over the window.
    inside: bool,
    /// Whether another client grabs the keyboard, which takes its keys until it lets it go.
    grabbed: bool,
    /// Whether a key reached the window since the last event of the focus: it holds the keyboard.
    keyed: bool,
    /// XKB's state as the server last told it, through `StateNotify` or a read of it, which the
    /// keyboard returns to after each key, translated in its own event's state.
    server: Option<([u32; 3], [u32; 3])>,
}

/// What told of the focus: the core protocol's focus events, with the server's focus read at each,
/// a key, and XInput2's crossings of the window's border.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Focus {
    /// Where the server's focus is, read at a `FocusIn` or `FocusOut` of the window's, outside a
    /// grab or at its end: the window itself, or above it, following the pointer or on an
    /// ancestor.
    Server { window: bool, above: bool },
    /// Another client grabbed the keyboard.
    Grabbed,
    /// A key reached the window, which no other client grabs the keyboard of then.
    Key,
    /// The pointer entered the window, the server saying whether its focus is the window or lies
    /// above it (XInput2's `focus`).
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
        let device = linux::x11_core_keyboard(X11Connection::new(connection))
            .map_err(|error| error.to_string())?;
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
                spec(device)?,
                xkb::EventType::from(0_u16),
                xkb::EventType::NEW_KEYBOARD_NOTIFY | xkb::EventType::MAP_NOTIFY,
                parts,
                parts,
                &xkb::SelectEventsAux::new(),
            )
            .map_err(|error| error.to_string())?;
        keyboard
            .set_x11_keymap(X11Connection::new(connection), device)
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
            focus_above: false,
            inside: false,
            grabbed: false,
            keyed: false,
            server: None,
        }
    }

    fn without_keyboard() -> Self {
        Self {
            keyboard: None,
            device: -1,
            modifiers: Modifiers::default(),
            window_focus: false,
            focus_above: false,
            inside: false,
            grabbed: false,
            keyed: false,
            server: None,
        }
    }

    /// Whether XKB's notice of device `device` is the core keyboard's, which this one follows.
    pub(super) fn follows(&self, device: u8) -> bool {
        self.keyboard.is_some() && i32::from(device) == self.device
    }

    /// Reads the keymap of the server's core keyboard again, which XKB said changed: the modifiers
    /// held, if they changed with it and the window holds the focus. A keymap that cannot be read
    /// leaves the last.
    pub(super) fn keymap_changed(&mut self, connection: &XCBConnection) -> Option<Input> {
        let keyboard = self.keyboard.as_mut()?;
        if let Err(error) = keyboard.set_x11_keymap(X11Connection::new(connection), self.device) {
            tracing::warn!(%error, "the X server's new keymap is not read: keys keep the last");
            return None;
        }
        self.modifiers_now()
    }

    /// Follows XKB's state of the modifiers and the layout while the window holds the focus, and
    /// only then: as it takes it, XKB's `StateNotify` is asked and the state read once, the
    /// modifiers held told if they changed meanwhile; as it loses it, the notices stop.
    pub(super) fn follow_state(&mut self, connection: &XCBConnection) -> Option<Input> {
        self.keyboard.as_ref()?;
        let focused = self.focused();
        let details = if focused { state_parts() } else { 0_u16.into() };
        let followed = spec(self.device).and_then(|device| {
            connection
                .xkb_select_events(
                    device,
                    xkb::EventType::from(0_u16),
                    xkb::EventType::from(0_u16),
                    0_u16.into(),
                    0_u16.into(),
                    &xkb::SelectEventsAux::new().state_notify(xkb::SelectEventsAuxStateNotify {
                        affect_state: state_parts(),
                        state_details: details,
                    }),
                )
                .map_err(|error| error.to_string())?;
            if !focused {
                return Ok(None);
            }
            let state = connection
                .xkb_get_state(device)
                .map_err(|error| error.to_string())?
                .reply()
                .map_err(|error| error.to_string())?;
            Ok(Some(state))
        });
        match followed {
            Ok(Some(state)) => {
                let (modifiers, layouts) = parts(
                    [state.base_mods, state.latched_mods, state.locked_mods],
                    [state.base_group, state.latched_group],
                    state.locked_group,
                );
                self.state(modifiers, layouts)
            }
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%error, "the keyboard's state is not followed: modifiers may lag");
                None
            }
        }
    }

    /// XKB's state of the modifiers and the layout as the server tells it: `modifiers` and
    /// `layouts` depressed, latched and locked. The modifiers held, if they changed and the window
    /// holds the focus.
    pub(super) fn state(&mut self, modifiers: [u32; 3], layouts: [u32; 3]) -> Option<Input> {
        self.server = Some((modifiers, layouts));
        self.keyboard.as_mut()?.set_modifiers(modifiers, layouts);
        self.modifiers_now()
    }

    /// The key of X11's keycode `keycode` going down, up or repeating, translated in `own`, the
    /// state that XInput2 gives with it, from before the key: the modifiers of that state told
    /// first if they changed, then the key, then the modifiers of the state that the server last
    /// told, which the keyboard returns to, if they differ. A key typed as the focus comes, before
    /// XKB's notices are asked, keeps its own modifiers; the last key's leaves none told held.
    pub(super) fn key_in(
        &mut self,
        keycode: u32,
        state: ButtonState,
        repeat: bool,
        (modifiers, layouts): ([u32; 3], [u32; 3]),
    ) -> Vec<Input> {
        let Some(keyboard) = self.keyboard.as_mut() else {
            return Vec::new();
        };
        keyboard.set_modifiers(modifiers, layouts);
        let mut told: Vec<Input> = self.modifiers_now().into_iter().collect();
        told.extend(self.key(keycode, state, repeat));
        if let (Some((modifiers, layouts)), Some(keyboard)) = (self.server, self.keyboard.as_mut())
        {
            keyboard.set_modifiers(modifiers, layouts);
            told.extend(self.modifiers_now());
        }
        told
    }

    /// The modifiers held, if the window holds the focus and they differ from those last told.
    fn modifiers_now(&mut self) -> Option<Input> {
        let held = self.keyboard.as_ref()?.modifiers();
        (self.focused() && held != self.modifiers).then(|| {
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

    /// What `event` makes of the window's keyboard focus: told once for each change, which
    /// [`Keys::follow_state`] must follow. A sequence of dead keys in progress is forgotten with
    /// the focus.
    pub(super) fn focus(&mut self, event: Focus) -> Option<Input> {
        let before = self.focused();
        match event {
            Focus::Server { window, above } => {
                self.window_focus = window;
                self.focus_above = above;
                self.grabbed = false;
                self.keyed = false;
            }
            Focus::Grabbed => {
                self.grabbed = true;
                self.keyed = false;
            }
            // A key reaches the window only while no other client grabs the keyboard: the
            // grab ended, though the server told it elsewhere, as under the pointer.
            Focus::Key => {
                self.keyed = true;
                self.grabbed = false;
            }
            Focus::Entered { focus } => {
                self.inside = true;
                self.focus_above = focus;
                self.keyed = false;
            }
            Focus::Left => {
                self.inside = false;
                self.keyed = false;
            }
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

    /// The modifiers told released, as the window lost the focus, if any was told held: the keys
    /// held go to another window from now on.
    pub(super) fn released(&mut self) -> Option<Input> {
        let released = Modifiers::default();
        (self.modifiers != released).then(|| {
            self.modifiers = released;
            Input::Modifiers(released)
        })
    }

    /// Whether the keys go to the window: its own focus, or the focus above it and the pointer
    /// over it, or a key that came; never while another client grabs the keyboard.
    pub(super) fn focused(&self) -> bool {
        !self.grabbed && (self.window_focus || (self.focus_above && self.inside) || self.keyed)
    }
}

/// The parts of XKB's state that the keys depend on: the modifiers and the layout, depressed,
/// latched and locked; not the pointer's buttons.
fn state_parts() -> xkb::StatePart {
    xkb::StatePart::MODIFIER_BASE
        | xkb::StatePart::MODIFIER_LATCH
        | xkb::StatePart::MODIFIER_LOCK
        | xkb::StatePart::GROUP_BASE
        | xkb::StatePart::GROUP_LATCH
        | xkb::StatePart::GROUP_LOCK
}

/// XKB's id of the keyboard `device` in its requests.
fn spec(device: i32) -> Result<xkb::DeviceSpec, String> {
    xkb::DeviceSpec::try_from(device).map_err(|error| error.to_string())
}

/// The modifiers' masks and the layouts of XKB's `state` notice, depressed, latched and locked,
/// as [`Keys::state`] takes them.
pub(super) fn state_of(state: &xkb::StateNotifyEvent) -> ([u32; 3], [u32; 3]) {
    parts(
        [state.base_mods, state.latched_mods, state.locked_mods],
        [state.base_group, state.latched_group],
        state.locked_group,
    )
}

/// The modifiers' masks and the layouts that XInput2 gives with a key, depressed, latched and
/// locked, as [`Keys::key_in`] takes them: the state that the key is translated in. A layout's byte
/// is XKB's signed group, a relative one negative, as `StateNotify`'s.
pub(super) fn key_state(
    modifiers: &x11rb::protocol::xinput::ModifierInfo,
    group: &x11rb::protocol::xinput::GroupInfo,
) -> ([u32; 3], [u32; 3]) {
    (
        [modifiers.base, modifiers.latched, modifiers.locked],
        [group.base, group.latched, group.locked]
            .map(|byte| layout(i16::from(i8::from_ne_bytes([byte])))),
    )
}

/// The masks of `modifiers` and the layouts of `groups`, then `locked`, as libxkbcommon takes them.
fn parts(
    modifiers: [ModMask; 3],
    [base, latched]: [i16; 2],
    locked: xkb::Group,
) -> ([u32; 3], [u32; 3]) {
    (
        modifiers.map(mask),
        [
            layout(base),
            layout(latched),
            layout(i16::from(u8::from(locked))),
        ],
    )
}

/// XKB's mask of modifiers as libxkbcommon takes it.
fn mask(modifiers: ModMask) -> u32 {
    u32::from(u16::from(modifiers))
}

/// XKB's layout, which a latch or a base may give as negative, relative, as libxkbcommon takes it:
/// the same bits, which it brings within the keymap's layouts itself.
fn layout(group: i16) -> u32 {
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

    /// The window's own focus given, as a window manager gives it.
    const GIVEN: Focus = Focus::Server {
        window: true,
        above: false,
    };
    /// The focus taken to another window.
    const TAKEN: Focus = Focus::Server {
        window: false,
        above: false,
    };
    /// The server's focus above the window: following the pointer, or on an ancestor.
    const ABOVE: Focus = Focus::Server {
        window: false,
        above: true,
    };

    #[test]
    fn xkbs_state_changes_the_modifiers_and_the_layout_told_once() {
        let mut keys = keys("us,fr");
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
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
        // The French layout locked, Shift released: `q`'s place types `a`.
        assert!(keys.state([0, 0, 0], [0, 0, 1]).is_some());
        assert_eq!(keys.state([0, 0, 0], [0, 0, 1]), None);
        assert_eq!(
            key(keys.key(24, ButtonState::Pressed, false)).2.as_deref(),
            Some("a")
        );
    }

    #[test]
    fn the_modifiers_are_told_only_while_focused() {
        let mut keys = keys("us");
        // A state given without the focus, as XKB's read as it comes: followed, not told.
        assert_eq!(keys.state([1, 0, 0], [0; 3]), None);
        assert_eq!(
            key(keys.key(38, ButtonState::Pressed, false)).2.as_deref(),
            Some("A"),
            "the state is followed"
        );
        // The focus given: the same state, read again as the window takes it, is told once.
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
        assert!(
            matches!(keys.state([1, 0, 0], [0; 3]), Some(Input::Modifiers(held)) if held.shift)
        );
        assert_eq!(keys.state([1, 0, 0], [0; 3]), None);
        // Taken away, then given back with the same modifiers: nothing more.
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
        assert_eq!(keys.state([1, 0, 0], [0; 3]), None);
    }

    #[test]
    fn xkbs_state_notice_gives_each_part_apart() {
        use x11rb::protocol::xproto::ModMask;
        let notice = xkb::StateNotifyEvent {
            base_mods: ModMask::SHIFT,
            latched_mods: ModMask::CONTROL,
            locked_mods: ModMask::LOCK,
            base_group: -1,
            latched_group: 2,
            locked_group: xkb::Group::from(1_u8),
            ..Default::default()
        };
        assert_eq!(state_of(&notice), ([1, 4, 2], [u32::MAX, 2, 1]));
    }

    #[test]
    fn the_modifiers_held_are_told_released_as_the_focus_leaves() {
        let mut keys = keys("us");
        keys.focus(GIVEN);
        assert!(keys.state([1, 0, 0], [0; 3]).is_some());
        keys.focus(TAKEN);
        assert_eq!(
            keys.released(),
            Some(Input::Modifiers(Modifiers::default()))
        );
        // Told once; none held, none told.
        assert_eq!(keys.released(), None);
    }

    #[test]
    fn a_grab_by_another_client_takes_the_focus_until_it_ends() {
        let mut keys = keys("us");
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
        assert_eq!(keys.focus(Focus::Grabbed), Some(Input::Focus(false)));
        // The server's focus read at the grab's end: still the window's.
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
        // A key that comes ends the grab, which the server told elsewhere.
        keys.focus(Focus::Grabbed);
        assert_eq!(keys.focus(Focus::Key), Some(Input::Focus(true)));
    }

    #[test]
    fn a_key_that_comes_gives_the_focus_until_the_next_event_of_the_focus() {
        let mut keys = keys("us");
        assert_eq!(keys.focus(Focus::Key), Some(Input::Focus(true)));
        assert_eq!(keys.focus(Focus::Key), None);
        // The server's focus read again, elsewhere: lost.
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        // A key, the server's focus read elsewhere before it, then the pointer gone: lost.
        keys.focus(Focus::Key);
        assert_eq!(keys.focus(Focus::Left), Some(Input::Focus(false)));
    }

    #[test]
    fn a_keys_own_state_is_read_as_xinput2_gives_it() {
        use x11rb::protocol::xinput::{GroupInfo, ModifierInfo};
        let modifiers = ModifierInfo {
            base: 1,
            latched: 4,
            locked: 2,
            effective: 7,
        };
        let group = GroupInfo {
            base: 0,
            latched: 1,
            locked: 3,
            effective: 0,
        };
        assert_eq!(key_state(&modifiers, &group), ([1, 4, 2], [0, 1, 3]));
    }

    #[test]
    fn a_key_is_translated_in_its_own_state_then_the_servers_comes_back() {
        let mut keys = keys("us");
        keys.focus(GIVEN);
        // The server's last word: nothing held.
        assert_eq!(keys.state([0; 3], [0; 3]), None);
        // `a` typed with Shift held, as its own event says, Shift let go since.
        let told = keys.key_in(38, ButtonState::Pressed, false, ([1, 0, 0], [0; 3]));
        assert_eq!(told.len(), 3, "{told:?}");
        assert!(matches!(told[0], Input::Modifiers(held) if held.shift));
        assert!(matches!(&told[1], Input::Key(event) if event.text.as_deref() == Some("A")));
        assert_eq!(told[2], Input::Modifiers(Modifiers::default()));
        // In the server's own state: the key alone.
        let told = keys.key_in(38, ButtonState::Released, false, ([0; 3], [0; 3]));
        assert_eq!(told.len(), 1, "{told:?}");
    }

    #[test]
    fn the_focus_is_told_at_each_change_alone() {
        let mut keys = keys("us");
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
        assert!(keys.focus(GIVEN).is_none());
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        assert!(keys.focus(TAKEN).is_none());
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
        // A window whose focus another holds: entering gives nothing.
        assert!(keys.focus(Focus::Entered { focus: false }).is_none());
        // The focus moving above the window while the pointer lies over it, and back.
        assert_eq!(keys.focus(ABOVE), Some(Input::Focus(true)));
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        // Above it, but the pointer gone: nothing.
        assert!(keys.focus(Focus::Left).is_none());
        assert!(keys.focus(ABOVE).is_none());
    }

    #[test]
    fn the_windows_own_focus_outlasts_the_pointer() {
        let mut keys = keys("us");
        assert_eq!(keys.focus(GIVEN), Some(Input::Focus(true)));
        // The pointer passing over the focused window changes nothing.
        assert!(keys.focus(Focus::Entered { focus: true }).is_none());
        assert!(keys.focus(Focus::Left).is_none());
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        // Focused by the pointer, then given the focus itself, then the pointer leaving: held.
        keys.focus(Focus::Entered { focus: true });
        assert!(keys.focus(GIVEN).is_none());
        assert!(keys.focus(Focus::Left).is_none());
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
        // The pointer entering the focused window, which then loses the focus to another while
        // the pointer stays: lost.
        keys.focus(GIVEN);
        assert!(keys.focus(Focus::Entered { focus: true }).is_none());
        assert_eq!(keys.focus(TAKEN), Some(Input::Focus(false)));
    }

    #[test]
    fn a_focus_given_to_an_ancestor_leaves_the_keys_to_the_window_under_the_pointer() {
        let mut keys = keys("us");
        keys.focus(GIVEN);
        keys.focus(Focus::Entered { focus: true });
        // The focus moves to the root window, above this one: the keys still come here.
        assert!(keys.focus(ABOVE).is_none());
        assert_eq!(keys.focus(Focus::Left), Some(Input::Focus(false)));
        assert_eq!(
            keys.focus(Focus::Entered { focus: true }),
            Some(Input::Focus(true))
        );
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
    fn a_keys_relative_layout_is_read_as_xkbs_notice_reads_it() {
        use x11rb::protocol::xinput::{GroupInfo, ModifierInfo};
        let modifiers = ModifierInfo {
            base: 0,
            latched: 0,
            locked: 0,
            effective: 0,
        };
        // XKB's base layout -1, relative, and the second locked: the first in effect. XInput2
        // gives each part in a byte.
        let group = GroupInfo {
            base: 0xff,
            latched: 0,
            locked: 1,
            effective: 0,
        };
        // On three layouts, the key of `q` types the first's.
        let mut keys = keys("us,fr,de");
        let (masks, layouts) = key_state(&modifiers, &group);
        keys.state(masks, layouts);
        assert_eq!(
            key(keys.key(24, ButtonState::Pressed, false)).2.as_deref(),
            Some("q"),
            "the layout in effect, from the key's own state {layouts:?}"
        );
        let notice = xkb::StateNotifyEvent {
            base_group: -1,
            locked_group: xkb::Group::from(1_u8),
            ..Default::default()
        };
        assert_eq!(key_state(&modifiers, &group), state_of(&notice));
    }

    #[test]
    fn the_focus_that_a_key_gave_ends_as_the_pointer_enters_where_the_focus_is_not() {
        let mut keys = keys("us");
        assert_eq!(keys.focus(Focus::Key), Some(Input::Focus(true)));
        // The pointer enters, the server saying that its focus lies neither on the window nor
        // above it: the keys go elsewhere.
        assert_eq!(
            keys.focus(Focus::Entered { focus: false }),
            Some(Input::Focus(false))
        );
    }

    #[test]
    fn a_relative_layout_keeps_its_bits() {
        assert_eq!(layout(1), 1);
        assert_eq!(layout(-1), u32::MAX);
        assert_eq!(mask(x11rb::protocol::xproto::ModMask::SHIFT), 1);
    }
}
