//! The keyboard through libxkbcommon, loaded at run time by `xkbcommon-dl`
//! (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`,
//! `docs/adr/0273-m6d7a2-cut-in-three-waylands-keyboard-cursor-scale-and-activation.md`): Linux's,
//! not Wayland's, so that X11's layer takes it too.
//!
//! A [`Keyboard`] holds the platform's keymap, the state of its modifiers and layout, and the
//! compose tables of the person's locale. It turns each key, by its Linux code, into
//! `slime-input`'s [`KeyEvent`], as that type's contract says:
//!
//! - the physical key from the Linux code, named as the W3C's `code` values name it;
//! - the logical key from the keysym that the keymap gives with the modifiers held, a named key
//!   first, else the keysym's character, Control never applied to it;
//! - dead keys and sequences composed: a dead key's press is [`Key::Dead`] with its character on
//!   its own, a press that ends a sequence the composed character, one that breaks it
//!   [`Key::Unidentified`] without text;
//! - the text of a press, which a release never has; the named keys' text the same on every
//!   platform: Enter `"\r"`, Tab `"\t"`, Backspace `"\u{8}"`, Escape `"\u{1b}"`, Space its own or the
//!   dead key's character, none for the others.
//!
//! A repeat is translated as a press, without feeding the compose tables: a key held repeats its
//! own character.

#![expect(
    unsafe_code,
    reason = "libxkbcommon's calls, through the functions that xkbcommon-dl loads"
)]

use std::ffi::{CString, c_char};
use std::fmt;
use std::ptr::NonNull;

use slime_input::{ButtonState, Key, KeyCode, KeyEvent, Modifiers, NamedKey};
use xkbcommon_dl::{
    XKB_MOD_NAME_ALT, XKB_MOD_NAME_CTRL, XKB_MOD_NAME_LOGO, XKB_MOD_NAME_SHIFT, XkbCommon,
    XkbCommonCompose, keysyms, xkb_compose_compile_flags, xkb_compose_feed_result,
    xkb_compose_state, xkb_compose_state_flags, xkb_compose_status, xkb_compose_table, xkb_context,
    xkb_context_flags, xkb_keymap, xkb_keymap_compile_flags, xkb_keymap_format, xkb_state,
    xkb_state_component,
};

/// What libxkbcommon refused.
#[derive(Debug)]
pub(crate) struct KeyboardError(&'static str);

impl fmt::Display for KeyboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for KeyboardError {}

/// The keyboard's keymap, its state and its compose tables.
pub(crate) struct Keyboard {
    xkb: &'static XkbCommon,
    context: NonNull<xkb_context>,
    /// None until the platform gives its keymap.
    keymap: Option<Keymap>,
    /// None where the locale has no compose tables, or libxkbcommon none of its compose functions.
    compose: Option<Compose>,
}

/// A keymap and the state of its modifiers and layout.
struct Keymap {
    xkb: &'static XkbCommon,
    keymap: NonNull<xkb_keymap>,
    state: NonNull<xkb_state>,
}

/// The compose tables of a locale: one state follows the person's keys, the other finds a dead
/// key's character on its own, as the key pressed twice gives it.
struct Compose {
    xkb: &'static XkbCommonCompose,
    table: NonNull<xkb_compose_table>,
    state: NonNull<xkb_compose_state>,
    alone: NonNull<xkb_compose_state>,
}

/// What the compose tables made of a press.
#[derive(Debug, PartialEq, Eq)]
enum Composed {
    /// The key is not part of a sequence.
    Nothing,
    /// The key starts or goes on with a sequence, as a dead key does.
    Composing,
    /// The key ends a sequence, which gives this text.
    Text(String),
    /// The key breaks a sequence.
    Cancelled,
}

impl Keyboard {
    /// Loads libxkbcommon, and the compose tables of `locale`, as [`locale`] reads it.
    ///
    /// # Errors
    ///
    /// When libxkbcommon does not load, or makes no context: the keyboard then gives nothing.
    pub(crate) fn new(locale: &str) -> Result<Self, KeyboardError> {
        let xkb = xkbcommon_dl::xkbcommon_option()
            .ok_or(KeyboardError("libxkbcommon.so.0 does not load"))?;
        // SAFETY: a call with a flag of the enum; the context returned is checked for null.
        let context = unsafe { (xkb.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };
        let context =
            NonNull::new(context).ok_or(KeyboardError("libxkbcommon makes no context"))?;
        let compose = Compose::new(context, locale);
        if compose.is_none() {
            tracing::info!(
                locale,
                "no compose tables for the locale: dead keys and sequences compose nothing"
            );
        }
        Ok(Self {
            xkb,
            context,
            keymap: None,
            compose,
        })
    }

    /// Takes the keymap that the platform gives, in the text format of XKB, which may end with
    /// a nul.
    ///
    /// # Errors
    ///
    /// When libxkbcommon cannot compile it: the keyboard then keeps its last keymap.
    pub(crate) fn set_keymap(&mut self, text: &[u8]) -> Result<(), KeyboardError> {
        let text = text.strip_suffix(&[0]).unwrap_or(text);
        // SAFETY: the context is live; the buffer is `text`, of its length, which libxkbcommon
        // reads during the call alone; the keymap returned is checked for null.
        let keymap = unsafe {
            (self.xkb.xkb_keymap_new_from_buffer)(
                self.context.as_ptr(),
                text.as_ptr().cast::<c_char>(),
                text.len(),
                xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        let keymap = NonNull::new(keymap).ok_or(KeyboardError("the keymap does not compile"))?;
        self.replace_keymap(keymap)
    }

    /// Takes `keymap`, whose reference it holds, in place of the last one: a sequence in progress
    /// is forgotten, its keys' meaning gone.
    fn replace_keymap(&mut self, keymap: NonNull<xkb_keymap>) -> Result<(), KeyboardError> {
        self.keymap = Some(Keymap::new(self.xkb, keymap)?);
        self.forget_sequence();
        Ok(())
    }

    /// Forgets the sequence of dead keys in progress: the window lost the keyboard's focus, or
    /// the keymap changed, and a dead key pressed before composes with nothing after.
    pub(crate) fn forget_sequence(&mut self) {
        if let Some(compose) = &mut self.compose {
            compose.reset();
        }
    }

    /// Makes a keymap of the XKB rules' layout `layout`, as a test of Wayland's layer types on it.
    #[cfg(test)]
    pub(crate) fn set_test_layout(&mut self, layout: &str) -> Result<(), KeyboardError> {
        self.set_layout(layout, "")
    }

    /// Makes a keymap of the XKB rules' layout `layout` and variant `variant`, as a test types on
    /// it.
    #[cfg(test)]
    fn set_layout(&mut self, layout: &str, variant: &str) -> Result<(), KeyboardError> {
        let layout = CString::new(layout).map_err(|_| KeyboardError("a layout without nul"))?;
        let variant = CString::new(variant).map_err(|_| KeyboardError("a variant without nul"))?;
        let names = xkbcommon_dl::xkb_rule_names {
            rules: std::ptr::null(),
            model: std::ptr::null(),
            layout: layout.as_ptr(),
            variant: variant.as_ptr(),
            options: std::ptr::null(),
        };
        // SAFETY: the context is live, and the names' strings outlive the call; the keymap
        // returned is checked for null.
        let keymap = unsafe {
            (self.xkb.xkb_keymap_new_from_names)(
                self.context.as_ptr(),
                &raw const names,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        let keymap = NonNull::new(keymap).ok_or(KeyboardError("the layout does not compile"))?;
        self.replace_keymap(keymap)
    }

    /// Whether a keymap is set.
    #[cfg(test)]
    fn has_keymap(&self) -> bool {
        self.keymap.is_some()
    }

    /// Sets the modifiers and the layout that the platform reports, each a mask of the keymap's
    /// modifiers or a layout's index: the modifiers held afterwards. Nothing without a keymap.
    pub(crate) fn set_modifiers(
        &mut self,
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    ) -> Modifiers {
        let Some(keymap) = &self.keymap else {
            return Modifiers::default();
        };
        // SAFETY: the state is live; libxkbcommon takes any mask and index.
        unsafe {
            (self.xkb.xkb_state_update_mask)(
                keymap.state.as_ptr(),
                depressed,
                latched,
                locked,
                0,
                0,
                group,
            );
        }
        keymap.modifiers()
    }

    /// Whether the key of Linux's code `code` repeats while held, as the keymap says.
    pub(crate) fn repeats(&self, code: u32) -> bool {
        let Some(keymap) = &self.keymap else {
            return false;
        };
        // SAFETY: the keymap is live; libxkbcommon takes any keycode, unknown ones answering 0.
        unsafe { (self.xkb.xkb_keymap_key_repeats)(keymap.keymap.as_ptr(), xkb_code(code)) == 1 }
    }

    /// The key of Linux's code `code` going down, up or repeating: its press feeds the compose
    /// tables, its repeat and release do not.
    pub(crate) fn key(&mut self, code: u32, state: ButtonState, repeat: bool) -> KeyEvent {
        let physical = key_code(code);
        let Some(keymap) = &self.keymap else {
            return KeyEvent {
                physical,
                logical: Key::Unidentified,
                text: None,
                state,
                repeat,
            };
        };
        // SAFETY: the state is live; an unknown keycode or one of several keysyms answers
        // `NoSymbol`, zero.
        let keysym =
            unsafe { (self.xkb.xkb_state_key_get_one_sym)(keymap.state.as_ptr(), xkb_code(code)) };
        let pressed = state == ButtonState::Pressed;
        let composed = match &mut self.compose {
            Some(compose) if pressed && !repeat => compose.feed(keysym),
            _ => Composed::Nothing,
        };
        let plain = || utf8(self.xkb, keysym).filter(|text| !text.chars().any(char::is_control));
        let logical = match (named_key(keysym), &composed) {
            (Some(named), _) => Key::Named(named),
            (None, Composed::Composing) => Key::Dead(
                self.compose
                    .as_mut()
                    .and_then(|compose| compose.alone(keysym)),
            ),
            (None, Composed::Text(text)) => Key::Character(text.clone()),
            (None, Composed::Cancelled) => Key::Unidentified,
            (None, Composed::Nothing) => plain().map_or(Key::Unidentified, Key::Character),
        };
        let text = if pressed {
            match (&logical, composed) {
                (Key::Named(named), composed) => named_text(*named, composed, plain),
                (_, Composed::Text(text)) => Some(text),
                (_, Composed::Composing | Composed::Cancelled) => None,
                (_, Composed::Nothing) => plain(),
            }
        } else {
            None
        };
        KeyEvent {
            physical,
            logical,
            text,
            state,
            repeat,
        }
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        // The keymap and the compose tables hold references of the context of their own.
        self.keymap = None;
        self.compose = None;
        // SAFETY: the context is this keyboard's, never used again.
        unsafe { (self.xkb.xkb_context_unref)(self.context.as_ptr()) };
    }
}

impl Keymap {
    /// The keymap `keymap`, whose reference it takes, and a new state of it.
    fn new(xkb: &'static XkbCommon, keymap: NonNull<xkb_keymap>) -> Result<Self, KeyboardError> {
        // SAFETY: the keymap is live; the state returned, which holds a reference of the keymap,
        // is checked for null.
        let state = unsafe { (xkb.xkb_state_new)(keymap.as_ptr()) };
        let Some(state) = NonNull::new(state) else {
            // SAFETY: the keymap's reference is this function's, never used again.
            unsafe { (xkb.xkb_keymap_unref)(keymap.as_ptr()) };
            return Err(KeyboardError("libxkbcommon makes no state of the keymap"));
        };
        Ok(Self { xkb, keymap, state })
    }

    /// The modifiers held, as the state says.
    fn modifiers(&self) -> Modifiers {
        let held = |name: &[u8]| {
            // SAFETY: the state is live, and `name` one of xkbcommon-dl's names, nul-terminated.
            unsafe {
                (self.xkb.xkb_state_mod_name_is_active)(
                    self.state.as_ptr(),
                    name.as_ptr().cast::<c_char>(),
                    xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
                ) == 1
            }
        };
        let mut modifiers = Modifiers::default();
        modifiers.shift = held(XKB_MOD_NAME_SHIFT);
        modifiers.control = held(XKB_MOD_NAME_CTRL);
        modifiers.alt = held(XKB_MOD_NAME_ALT);
        modifiers.super_key = held(XKB_MOD_NAME_LOGO);
        modifiers
    }
}

impl Drop for Keymap {
    fn drop(&mut self) {
        // SAFETY: the state and the keymap's reference are this value's, never used again.
        unsafe {
            (self.xkb.xkb_state_unref)(self.state.as_ptr());
            (self.xkb.xkb_keymap_unref)(self.keymap.as_ptr());
        }
    }
}

impl Compose {
    /// The compose tables of `locale`, if libxkbcommon has its compose functions and the locale
    /// its tables.
    fn new(context: NonNull<xkb_context>, locale: &str) -> Option<Self> {
        let xkb = xkbcommon_dl::xkbcommon_compose_option()?;
        let locale = CString::new(locale).ok()?;
        // SAFETY: the context is live, and the locale's string outlives the call; the table
        // returned holds a reference of the context and is checked for null.
        let table = unsafe {
            (xkb.xkb_compose_table_new_from_locale)(
                context.as_ptr(),
                locale.as_ptr(),
                xkb_compose_compile_flags::XKB_COMPOSE_COMPILE_NO_FLAGS,
            )
        };
        let table = NonNull::new(table)?;
        let new_state = || {
            // SAFETY: the table is live; the state returned holds a reference of it.
            NonNull::new(unsafe {
                (xkb.xkb_compose_state_new)(
                    table.as_ptr(),
                    xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS,
                )
            })
        };
        let (state, alone) = (new_state(), new_state());
        let compose = |state, alone| Self {
            xkb,
            table,
            state,
            alone,
        };
        match (state, alone) {
            (Some(state), Some(alone)) => Some(compose(state, alone)),
            (state, alone) => {
                // SAFETY: each is this function's own, never used again; the table's reference
                // too.
                unsafe {
                    for state in [state, alone].into_iter().flatten() {
                        (xkb.xkb_compose_state_unref)(state.as_ptr());
                    }
                    (xkb.xkb_compose_table_unref)(table.as_ptr());
                }
                None
            }
        }
    }

    /// Forgets the sequence in progress.
    fn reset(&mut self) {
        // SAFETY: the state is live.
        unsafe { (self.xkb.xkb_compose_state_reset)(self.state.as_ptr()) };
    }

    /// Feeds a press's keysym to the sequence in progress.
    fn feed(&mut self, keysym: u32) -> Composed {
        // SAFETY: the state is live; libxkbcommon takes any keysym, ignoring the modifiers', and
        // returns one of `xkb_compose_feed_result`'s two values alone, as xkbcommon-dl's Rust enum
        // needs (libxkbcommon 1.x's `xkbcommon-compose.h`).
        let fed = unsafe { (self.xkb.xkb_compose_state_feed)(self.state.as_ptr(), keysym) };
        if fed == xkb_compose_feed_result::XKB_COMPOSE_FEED_IGNORED {
            return Composed::Nothing;
        }
        // SAFETY: the state is live; libxkbcommon returns one of `xkb_compose_status`'s four values
        // alone, as xkbcommon-dl's Rust enum needs.
        match unsafe { (self.xkb.xkb_compose_state_get_status)(self.state.as_ptr()) } {
            xkb_compose_status::XKB_COMPOSE_NOTHING => Composed::Nothing,
            xkb_compose_status::XKB_COMPOSE_COMPOSING => Composed::Composing,
            xkb_compose_status::XKB_COMPOSE_COMPOSED => {
                let text = composed_text(self.xkb, self.state);
                self.reset();
                // A sequence that gives a keysym without a character gives no text.
                text.map_or(Composed::Cancelled, Composed::Text)
            }
            xkb_compose_status::XKB_COMPOSE_CANCELLED => {
                self.reset();
                Composed::Cancelled
            }
        }
    }

    /// The character of a dead key on its own: what the key pressed twice composes.
    fn alone(&mut self, keysym: u32) -> Option<char> {
        // SAFETY: the state is live; each call takes any keysym.
        unsafe {
            (self.xkb.xkb_compose_state_reset)(self.alone.as_ptr());
            (self.xkb.xkb_compose_state_feed)(self.alone.as_ptr(), keysym);
            (self.xkb.xkb_compose_state_feed)(self.alone.as_ptr(), keysym);
        }
        let text = composed_text(self.xkb, self.alone);
        // SAFETY: the state is live.
        unsafe { (self.xkb.xkb_compose_state_reset)(self.alone.as_ptr()) };
        text?.chars().next()
    }
}

impl Drop for Compose {
    fn drop(&mut self) {
        // SAFETY: the states and the table's reference are this value's, never used again.
        unsafe {
            (self.xkb.xkb_compose_state_unref)(self.state.as_ptr());
            (self.xkb.xkb_compose_state_unref)(self.alone.as_ptr());
            (self.xkb.xkb_compose_table_unref)(self.table.as_ptr());
        }
    }
}

/// The compose tables' locale for the person: the first of `LC_ALL`, `LC_CTYPE` and `LANG` that is
/// set and not empty, as the C library's locale reads them, else `C`.
pub(crate) fn locale() -> String {
    locale_of(|name| std::env::var(name).ok())
}

/// The locale that the variables read by `variable` name, in the C library's order.
fn locale_of(variable: impl Fn(&str) -> Option<String>) -> String {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .filter_map(variable)
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| "C".to_owned())
}

/// XKB's keycode of Linux's code `code`, which XKB counts from 8, as X11 did.
fn xkb_code(code: u32) -> u32 {
    code.saturating_add(8)
}

/// The text that a compose state holds once a sequence ends, if it gives a character, however
/// long: a person's compose file may give a sequence a whole sentence.
fn composed_text(xkb: &XkbCommonCompose, state: NonNull<xkb_compose_state>) -> Option<String> {
    printed(|buffer| {
        // SAFETY: the state is live; libxkbcommon writes at most the buffer's length, nul
        // included.
        unsafe {
            (xkb.xkb_compose_state_get_utf8)(
                state.as_ptr(),
                buffer.as_mut_ptr().cast::<c_char>(),
                buffer.len(),
            )
        }
    })
}

/// The text that `print` writes as `snprintf` does: it answers the text's whole length, without
/// its nul, and writes what fits; a buffer too short is grown to the length and written again.
fn printed(mut print: impl FnMut(&mut [u8]) -> i32) -> Option<String> {
    let mut buffer = vec![0_u8; 64];
    let mut length = usize::try_from(print(&mut buffer)).ok()?;
    if length >= buffer.len() {
        buffer = vec![0; length.checked_add(1)?];
        length = usize::try_from(print(&mut buffer))
            .ok()?
            .min(buffer.len() - 1);
    }
    let text = std::str::from_utf8(&buffer[..length]).ok()?;
    (!text.is_empty()).then(|| text.to_owned())
}

/// The character of `keysym`, if it has one.
fn utf8(xkb: &XkbCommon, keysym: u32) -> Option<String> {
    let mut buffer = [0_u8; 16];
    // SAFETY: libxkbcommon writes at most the buffer's length, nul included.
    let written = unsafe {
        (xkb.xkb_keysym_to_utf8)(keysym, buffer.as_mut_ptr().cast::<c_char>(), buffer.len())
    };
    text_of(&buffer, written)
}

/// The text that libxkbcommon wrote in `buffer`: `written` bytes, its nul included; none for
/// zero, an error or too long a text.
fn text_of(buffer: &[u8], written: i32) -> Option<String> {
    let length = usize::try_from(written).ok()?.min(buffer.len());
    let text = buffer[..length].split(|&byte| byte == 0).next()?;
    let text = std::str::from_utf8(text).ok()?;
    (!text.is_empty()).then(|| text.to_owned())
}

/// The text of a named key's press, the same on every platform: Space's own, or the dead key's
/// character that it composes; four keys' control characters; none for the others.
fn named_text(
    named: NamedKey,
    composed: Composed,
    plain: impl FnOnce() -> Option<String>,
) -> Option<String> {
    match named {
        NamedKey::Space => match composed {
            Composed::Text(text) => Some(text),
            Composed::Composing | Composed::Cancelled => None,
            Composed::Nothing => plain().or_else(|| Some(" ".to_owned())),
        },
        NamedKey::Enter => Some("\r".to_owned()),
        NamedKey::Tab => Some("\t".to_owned()),
        NamedKey::Backspace => Some("\u{8}".to_owned()),
        NamedKey::Escape => Some("\u{1b}".to_owned()),
        _ => None,
    }
}

/// The named key of `keysym`, if the engine names it.
fn named_key(keysym: u32) -> Option<NamedKey> {
    use keysyms as k;
    let named = match keysym {
        k::BackSpace => NamedKey::Backspace,
        k::Tab | k::KP_Tab | k::ISO_Left_Tab => NamedKey::Tab,
        k::Return | k::KP_Enter | k::ISO_Enter => NamedKey::Enter,
        k::Pause | k::Break => NamedKey::Pause,
        k::Scroll_Lock => NamedKey::ScrollLock,
        k::Sys_Req | k::Print => NamedKey::PrintScreen,
        k::Escape => NamedKey::Escape,
        k::Delete | k::KP_Delete => NamedKey::Delete,
        k::Multi_key => NamedKey::Compose,
        k::Home | k::KP_Home => NamedKey::Home,
        k::End | k::KP_End => NamedKey::End,
        k::Left | k::KP_Left => NamedKey::ArrowLeft,
        k::Up | k::KP_Up => NamedKey::ArrowUp,
        k::Right | k::KP_Right => NamedKey::ArrowRight,
        k::Down | k::KP_Down => NamedKey::ArrowDown,
        k::Page_Up | k::KP_Page_Up => NamedKey::PageUp,
        k::Page_Down | k::KP_Page_Down => NamedKey::PageDown,
        k::Insert | k::KP_Insert => NamedKey::Insert,
        k::Undo => NamedKey::Undo,
        k::Redo => NamedKey::Redo,
        k::Menu => NamedKey::ContextMenu,
        k::Num_Lock => NamedKey::NumLock,
        k::space => NamedKey::Space,
        k::Shift_L | k::Shift_R => NamedKey::Shift,
        k::Control_L | k::Control_R => NamedKey::Control,
        k::Caps_Lock => NamedKey::CapsLock,
        k::Alt_L | k::Alt_R => NamedKey::Alt,
        k::Meta_L | k::Meta_R => NamedKey::Meta,
        k::Super_L | k::Super_R => NamedKey::Super,
        k::Hyper_L | k::Hyper_R => NamedKey::Hyper,
        k::ISO_Level3_Shift | k::ISO_Level3_Latch | k::ISO_Level3_Lock => NamedKey::AltGraph,
        k::XF86_Copy => NamedKey::Copy,
        k::XF86_Cut => NamedKey::Cut,
        k::XF86_Paste => NamedKey::Paste,
        k::XF86_AudioLowerVolume => NamedKey::AudioVolumeDown,
        k::XF86_AudioRaiseVolume => NamedKey::AudioVolumeUp,
        k::XF86_AudioMute => NamedKey::AudioVolumeMute,
        k::XF86_AudioPlay | k::XF86_AudioPause => NamedKey::MediaPlayPause,
        k::XF86_AudioStop => NamedKey::MediaStop,
        k::XF86_AudioNext => NamedKey::MediaTrackNext,
        k::XF86_AudioPrev => NamedKey::MediaTrackPrevious,
        k::KP_F1 => NamedKey::F1,
        k::KP_F2 => NamedKey::F2,
        k::KP_F3 => NamedKey::F3,
        k::KP_F4 => NamedKey::F4,
        k::F1..=k::F24 => FUNCTION_KEYS[usize::try_from(keysym - k::F1).ok()?],
        _ => return None,
    };
    Some(named)
}

/// F1 to F24, in their keysyms' order.
const FUNCTION_KEYS: [NamedKey; 24] = [
    NamedKey::F1,
    NamedKey::F2,
    NamedKey::F3,
    NamedKey::F4,
    NamedKey::F5,
    NamedKey::F6,
    NamedKey::F7,
    NamedKey::F8,
    NamedKey::F9,
    NamedKey::F10,
    NamedKey::F11,
    NamedKey::F12,
    NamedKey::F13,
    NamedKey::F14,
    NamedKey::F15,
    NamedKey::F16,
    NamedKey::F17,
    NamedKey::F18,
    NamedKey::F19,
    NamedKey::F20,
    NamedKey::F21,
    NamedKey::F22,
    NamedKey::F23,
    NamedKey::F24,
];

/// The physical key of Linux's code `code` (`linux/input-event-codes.h`), as the W3C's `code`
/// values name the key at that place.
fn key_code(code: u32) -> KeyCode {
    use KeyCode as C;
    match code {
        1 => C::Escape,
        2 => C::Digit1,
        3 => C::Digit2,
        4 => C::Digit3,
        5 => C::Digit4,
        6 => C::Digit5,
        7 => C::Digit6,
        8 => C::Digit7,
        9 => C::Digit8,
        10 => C::Digit9,
        11 => C::Digit0,
        12 => C::Minus,
        13 => C::Equal,
        14 => C::Backspace,
        15 => C::Tab,
        16 => C::KeyQ,
        17 => C::KeyW,
        18 => C::KeyE,
        19 => C::KeyR,
        20 => C::KeyT,
        21 => C::KeyY,
        22 => C::KeyU,
        23 => C::KeyI,
        24 => C::KeyO,
        25 => C::KeyP,
        26 => C::BracketLeft,
        27 => C::BracketRight,
        28 => C::Enter,
        29 => C::ControlLeft,
        30 => C::KeyA,
        31 => C::KeyS,
        32 => C::KeyD,
        33 => C::KeyF,
        34 => C::KeyG,
        35 => C::KeyH,
        36 => C::KeyJ,
        37 => C::KeyK,
        38 => C::KeyL,
        39 => C::Semicolon,
        40 => C::Quote,
        41 => C::Backquote,
        42 => C::ShiftLeft,
        43 => C::Backslash,
        44 => C::KeyZ,
        45 => C::KeyX,
        46 => C::KeyC,
        47 => C::KeyV,
        48 => C::KeyB,
        49 => C::KeyN,
        50 => C::KeyM,
        51 => C::Comma,
        52 => C::Period,
        53 => C::Slash,
        54 => C::ShiftRight,
        55 => C::NumpadMultiply,
        56 => C::AltLeft,
        57 => C::Space,
        58 => C::CapsLock,
        59 => C::F1,
        60 => C::F2,
        61 => C::F3,
        62 => C::F4,
        63 => C::F5,
        64 => C::F6,
        65 => C::F7,
        66 => C::F8,
        67 => C::F9,
        68 => C::F10,
        69 => C::NumLock,
        70 => C::ScrollLock,
        71 => C::Numpad7,
        72 => C::Numpad8,
        73 => C::Numpad9,
        74 => C::NumpadSubtract,
        75 => C::Numpad4,
        76 => C::Numpad5,
        77 => C::Numpad6,
        78 => C::NumpadAdd,
        79 => C::Numpad1,
        80 => C::Numpad2,
        81 => C::Numpad3,
        82 => C::Numpad0,
        83 => C::NumpadDecimal,
        85 => C::Lang5,
        86 => C::IntlBackslash,
        87 => C::F11,
        88 => C::F12,
        89 => C::IntlRo,
        90 => C::Lang3,
        91 => C::Lang4,
        92 => C::Convert,
        93 => C::KanaMode,
        94 => C::NonConvert,
        96 => C::NumpadEnter,
        97 => C::ControlRight,
        98 => C::NumpadDivide,
        99 => C::PrintScreen,
        100 => C::AltRight,
        102 => C::Home,
        103 => C::ArrowUp,
        104 => C::PageUp,
        105 => C::ArrowLeft,
        106 => C::ArrowRight,
        107 => C::End,
        108 => C::ArrowDown,
        109 => C::PageDown,
        110 => C::Insert,
        111 => C::Delete,
        113 => C::AudioVolumeMute,
        114 => C::AudioVolumeDown,
        115 => C::AudioVolumeUp,
        116 => C::Power,
        117 => C::NumpadEqual,
        119 => C::Pause,
        121 => C::NumpadComma,
        122 => C::Lang1,
        123 => C::Lang2,
        124 => C::IntlYen,
        125 => C::SuperLeft,
        126 => C::SuperRight,
        127 => C::ContextMenu,
        128 => C::BrowserStop,
        129 => C::Again,
        130 => C::Props,
        131 => C::Undo,
        132 => C::Select,
        133 => C::Copy,
        134 => C::Open,
        135 => C::Paste,
        136 => C::Find,
        137 => C::Cut,
        138 => C::Help,
        140 => C::LaunchApp2,
        142 => C::Sleep,
        143 => C::WakeUp,
        155 => C::LaunchMail,
        156 => C::BrowserFavorites,
        157 => C::LaunchApp1,
        158 => C::BrowserBack,
        159 => C::BrowserForward,
        161 => C::Eject,
        163 => C::MediaTrackNext,
        164 => C::MediaPlayPause,
        165 => C::MediaTrackPrevious,
        166 => C::MediaStop,
        172 => C::BrowserHome,
        173 => C::BrowserRefresh,
        179 => C::NumpadParenLeft,
        180 => C::NumpadParenRight,
        183 => C::F13,
        184 => C::F14,
        185 => C::F15,
        186 => C::F16,
        187 => C::F17,
        188 => C::F18,
        189 => C::F19,
        190 => C::F20,
        191 => C::F21,
        192 => C::F22,
        193 => C::F23,
        194 => C::F24,
        205 => C::Suspend,
        217 => C::BrowserSearch,
        226 => C::MediaSelect,
        464 => C::Fn,
        _ => C::Unidentified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Linux's codes of the keys that the tests press.
    const KEY_A: u32 = 30;
    const KEY_Q: u32 = 16;
    const KEY_E: u32 = 18;
    const KEY_SPACE: u32 = 57;
    const KEY_ENTER: u32 = 28;
    const KEY_TAB: u32 = 15;
    const KEY_DELETE: u32 = 111;
    const KEY_BACKSPACE: u32 = 14;
    const KEY_ESC: u32 = 1;
    const KEY_LEFTSHIFT: u32 = 42;
    /// The key right of P, `^` on a French keyboard, dead.
    const KEY_LEFTBRACE: u32 = 26;

    /// A keyboard of `layout`, composing as `en_US.UTF-8`.
    fn keyboard(layout: &str) -> Keyboard {
        let mut keyboard = Keyboard::new("en_US.UTF-8").expect("libxkbcommon loads");
        keyboard
            .set_layout(layout, "")
            .expect("the layout compiles");
        keyboard
    }

    fn press(keyboard: &mut Keyboard, code: u32) -> (Key, Option<String>) {
        let event = keyboard.key(code, ButtonState::Pressed, false);
        (event.logical, event.text)
    }

    fn character(text: &str) -> Key {
        Key::Character(text.to_owned())
    }

    /// The mask of Shift in the keymaps that XKB's rules compile: its first modifier.
    const SHIFT: u32 = 1;

    #[test]
    fn a_character_key_gives_its_character_and_its_release_no_text() {
        let mut keyboard = keyboard("us");
        let event = keyboard.key(KEY_A, ButtonState::Pressed, false);
        assert_eq!(
            event,
            KeyEvent {
                physical: KeyCode::KeyA,
                logical: character("a"),
                text: Some("a".to_owned()),
                state: ButtonState::Pressed,
                repeat: false,
            }
        );
        let release = keyboard.key(KEY_A, ButtonState::Released, false);
        assert_eq!((release.logical, release.text), (character("a"), None));
    }

    #[test]
    fn the_layout_gives_the_logical_key_and_the_place_its_physical_one() {
        let mut keyboard = keyboard("fr");
        // On a French keyboard, the key at the place of Q types `a`.
        let event = keyboard.key(KEY_Q, ButtonState::Pressed, false);
        assert_eq!(
            (event.physical, event.logical),
            (KeyCode::KeyQ, character("a"))
        );
    }

    #[test]
    fn the_modifiers_change_the_character_but_control_does_not() {
        let mut keyboard = keyboard("us");
        let held = keyboard.set_modifiers(SHIFT, 0, 0, 0);
        assert!(held.shift && !held.control && !held.alt && !held.super_key);
        assert_eq!(
            press(&mut keyboard, KEY_A),
            (character("A"), Some("A".to_owned()))
        );
        // Control, the third modifier of the rules' keymaps: no control character.
        let held = keyboard.set_modifiers(4, 0, 0, 0);
        assert!(held.control && !held.shift);
        assert_eq!(
            press(&mut keyboard, KEY_A),
            (character("a"), Some("a".to_owned()))
        );
    }

    #[test]
    fn named_keys_give_the_same_text_as_on_every_platform() {
        let mut keyboard = keyboard("us");
        assert_eq!(
            press(&mut keyboard, KEY_ENTER),
            (Key::Named(NamedKey::Enter), Some("\r".to_owned()))
        );
        assert_eq!(
            press(&mut keyboard, KEY_SPACE),
            (Key::Named(NamedKey::Space), Some(" ".to_owned()))
        );
        // Delete's keysym has a character, which its text never is.
        assert_eq!(
            press(&mut keyboard, KEY_DELETE),
            (Key::Named(NamedKey::Delete), None)
        );
        assert_eq!(
            press(&mut keyboard, KEY_BACKSPACE),
            (Key::Named(NamedKey::Backspace), Some("\u{8}".to_owned()))
        );
        assert_eq!(
            press(&mut keyboard, KEY_ESC),
            (Key::Named(NamedKey::Escape), Some("\u{1b}".to_owned()))
        );
        assert_eq!(
            press(&mut keyboard, KEY_LEFTSHIFT),
            (Key::Named(NamedKey::Shift), None)
        );
        // Shift and Tab: Tab still, its text a tab.
        keyboard.set_modifiers(SHIFT, 0, 0, 0);
        assert_eq!(
            press(&mut keyboard, KEY_TAB),
            (Key::Named(NamedKey::Tab), Some("\t".to_owned()))
        );
    }

    #[test]
    fn a_dead_key_composes_with_the_next() {
        let mut keyboard = keyboard("fr");
        assert_eq!(
            press(&mut keyboard, KEY_LEFTBRACE),
            (Key::Dead(Some('^')), None)
        );
        // Its release names no character, as the engine's layer on winit gives it.
        let release = keyboard.key(KEY_LEFTBRACE, ButtonState::Released, false);
        assert_eq!((release.logical, release.text), (Key::Unidentified, None));
        assert_eq!(
            press(&mut keyboard, KEY_E),
            (character("ê"), Some("ê".to_owned()))
        );
        // The release of `e` is the plain character.
        let release = keyboard.key(KEY_E, ButtonState::Released, false);
        assert_eq!(release.logical, character("e"));
        // Space after the dead key gives its character.
        press(&mut keyboard, KEY_LEFTBRACE);
        assert_eq!(
            press(&mut keyboard, KEY_SPACE),
            (Key::Named(NamedKey::Space), Some("^".to_owned()))
        );
        // `q`, at the place of A, does not combine: it breaks the sequence, without text, and
        // the next press types again.
        press(&mut keyboard, KEY_LEFTBRACE);
        assert_eq!(press(&mut keyboard, KEY_A), (Key::Unidentified, None));
        assert_eq!(
            press(&mut keyboard, KEY_A),
            (character("q"), Some("q".to_owned()))
        );
    }

    #[test]
    fn a_repeat_feeds_no_sequence() {
        let mut keyboard = keyboard("fr");
        press(&mut keyboard, KEY_LEFTBRACE);
        let repeat = keyboard.key(KEY_E, ButtonState::Pressed, true);
        assert_eq!(
            (repeat.logical, repeat.text, repeat.repeat),
            (character("e"), Some("e".to_owned()), true)
        );
        // The sequence still waits for its next press.
        assert_eq!(
            press(&mut keyboard, KEY_E),
            (character("ê"), Some("ê".to_owned()))
        );
    }

    #[test]
    fn keys_repeat_as_the_keymap_says() {
        let keyboard = keyboard("us");
        assert!(keyboard.repeats(KEY_A));
        assert!(!keyboard.repeats(KEY_LEFTSHIFT));
    }

    #[test]
    fn without_a_keymap_a_key_has_its_place_alone() {
        let mut keyboard = Keyboard::new("C").expect("libxkbcommon loads");
        assert!(!keyboard.has_keymap());
        assert_eq!(press(&mut keyboard, KEY_A), (Key::Unidentified, None));
        assert_eq!(keyboard.set_modifiers(SHIFT, 0, 0, 0), Modifiers::default());
        assert!(!keyboard.repeats(KEY_A));
    }

    #[test]
    fn a_keymap_given_as_text_compiles_and_a_broken_one_is_refused() {
        let mut keyboard = Keyboard::new("C").expect("libxkbcommon loads");
        assert!(keyboard.set_keymap(b"xkb_keymap {\0").is_err());
        assert!(!keyboard.has_keymap());
        let keymap = b"xkb_keymap {
            xkb_keycodes { include \"evdev+aliases(qwerty)\" };
            xkb_types { include \"complete\" };
            xkb_compat { include \"complete\" };
            xkb_symbols { include \"pc+us+inet(evdev)\" };
        };\0";
        keyboard.set_keymap(keymap).expect("the keymap compiles");
        assert_eq!(
            press(&mut keyboard, KEY_A),
            (character("a"), Some("a".to_owned()))
        );
    }

    #[test]
    fn the_locale_is_read_in_the_c_librarys_order() {
        let of = |set: &[(&str, &str)]| {
            let set: Vec<(String, String)> = set
                .iter()
                .map(|&(name, value)| (name.to_owned(), value.to_owned()))
                .collect();
            locale_of(|name| {
                set.iter()
                    .find(|(set, _)| set == name)
                    .map(|(_, value)| value.clone())
            })
        };
        assert_eq!(
            of(&[("LANG", "en_US.UTF-8"), ("LC_ALL", "fr_FR.UTF-8")]),
            "fr_FR.UTF-8"
        );
        assert_eq!(
            of(&[("LANG", "en_US.UTF-8"), ("LC_CTYPE", "de_DE.UTF-8")]),
            "de_DE.UTF-8"
        );
        // An empty variable counts as unset.
        assert_eq!(
            of(&[("LC_ALL", ""), ("LANG", "en_US.UTF-8")]),
            "en_US.UTF-8"
        );
        assert_eq!(of(&[]), "C");
    }

    /// `snprintf`'s way, over `text`.
    fn printing(text: &str) -> impl FnMut(&mut [u8]) -> i32 {
        let text = text.as_bytes().to_vec();
        move |buffer: &mut [u8]| {
            let fits = text.len().min(buffer.len().saturating_sub(1));
            buffer[..fits].copy_from_slice(&text[..fits]);
            buffer[fits] = 0;
            i32::try_from(text.len()).expect("a short text")
        }
    }

    #[test]
    fn a_composed_text_is_read_whole_however_long() {
        assert_eq!(printed(printing("ê")), Some("ê".to_owned()));
        let long = "é".repeat(40);
        assert_eq!(printed(printing(&long)), Some(long));
        assert_eq!(printed(printing("")), None);
        assert_eq!(printed(|_| -1), None);
    }

    #[test]
    fn a_new_keymap_forgets_the_sequence_in_progress() {
        let mut keyboard = keyboard("fr");
        press(&mut keyboard, KEY_LEFTBRACE);
        keyboard.set_layout("fr", "").expect("the layout compiles");
        assert_eq!(
            press(&mut keyboard, KEY_E),
            (character("e"), Some("e".to_owned()))
        );
    }

    #[test]
    fn linux_codes_name_their_keys_place() {
        assert_eq!(key_code(1), KeyCode::Escape);
        assert_eq!(key_code(30), KeyCode::KeyA);
        assert_eq!(key_code(125), KeyCode::SuperLeft);
        assert_eq!(key_code(194), KeyCode::F24);
        assert_eq!(key_code(0), KeyCode::Unidentified);
        assert_eq!(key_code(u32::MAX), KeyCode::Unidentified);
        assert_eq!(xkb_code(u32::MAX), u32::MAX);
    }

    #[test]
    fn libxkbcommons_text_is_read_to_its_nul() {
        assert_eq!(text_of(b"ab\0\0", 3), Some("ab".to_owned()));
        assert_eq!(text_of(b"\0", 1), None);
        assert_eq!(text_of(b"ab", -1), None);
        assert_eq!(text_of(b"\xff\0", 2), None);
    }
}
