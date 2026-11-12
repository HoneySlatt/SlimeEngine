//! Shortcuts: documents of KDL that name a command for a key and its modifiers, in a scope
//! (`docs/adr/0142-input-and-focus.md`,
//! `docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
//!
//! A table of shortcuts holds a node `scope` for each scope: `global`, or the class of an element,
//! whose shortcuts hold while the focus lies inside it. Each child of a scope is a *chord*,
//! modifiers and a key joined by `+`, and its one argument the name of the command that it gives
//! the program ([`ActionKind::Command`](crate::action::ActionKind::Command)):
//!
//! ```kdl
//! scope global {
//!     Primary+Z edit.undo
//!     Primary+Shift+Z edit.redo
//!     F2 rename
//! }
//! scope hierarchy {
//!     Delete delete
//! }
//! ```
//!
//! A chord names its key by what the key means with the keyboard's layout, its *logical* key: a
//! character, `Z`, which a letter of either case matches, or the name of a named key, `Delete`,
//! `F2`, `ArrowUp`; so Primary+Z is the key labelled Z, wherever the layout puts it. Its modifiers
//! are `Primary`, `Ctrl`, `Alt`, `Shift` and `Super`, all held and no other, and `+` is written
//! last, `Primary++`. `Primary` is the platform's own modifier of shortcuts ([`Platform`]): Ctrl on
//! Linux and Windows, ⌘ on macOS; `Ctrl` is the Control key everywhere, and `Super` the key with
//! the system's logo, which is ⌘ on macOS too. A chord that KDL does not take as a bare name, a
//! digit, `"1"`, or one holding `;`, `=`, `/`, `\`, `#`, `"`, a bracket or a brace, is written in
//! quotes: `"Primary+="`.
//!
//! Two cases bend the rule. A character without case, `+` or `1`, is typed with Shift on some
//! layouts and without on others: its chord does not name Shift, and matches either way. A layout
//! of another script than the Latin one types no Latin letter: there, a chord of a letter is the
//! key at that letter's position on a QWERTY keyboard.
//!
//! A chord may end with a button of the mouse in place of a key, `LeftButton`, `RightButton`,
//! `MiddleButton`, `BackButton` or `ForwardButton`: it names what a press of that button does in a
//! scope that the program reads itself ([`Ui::button_command`](crate::tree::Ui::button_command)),
//! as an editor's viewport reads which button orbits
//! (`docs/adr/0160-two-viewports-and-blenders-way.md`). A button's chord holds while the modifiers
//! that it names are held, whatever others are, and of those that hold, the one that names the most
//! wins, of two that name as many the later in the tables: `LeftButton` selects with Shift held
//! too, unless `Shift+LeftButton` names another command.
//!
//! ```kdl
//! scope viewport {
//!     MiddleButton orbit
//!     Shift+MiddleButton pan
//! }
//! ```
//!
//! A table is valid on every platform or on none: a scope given twice in a table is refused, and so
//! are two chords of a scope that are the same keys on some platform, `Primary+Z` beside `Ctrl+Z`,
//! the same on Linux and Windows, or `Primary+Q` beside `Super+Q`, the same on macOS. A later table
//! gives a chord of a scope again, as the platform resolves it, in place of an earlier one's. What
//! has no meaning yet is refused, to be given one later: an annotation, a property, a child, a
//! second argument, a command `#null`; so are a chord that ends with a modifier, `Primary+Shift`,
//! or joins none by a `+`, `+Z`, and an empty name. A scope's name, a chord or a command never
//! starts with `$`, which names a theme's value: the key `$` is bound with a modifier,
//! `Primary+$`, never alone.

use std::collections::HashMap;

use slime_input::{Key, KeyEvent, Modifiers, MouseButton, NamedKey};
use uwui_kdl::{Node, Value};

use crate::document::{DocumentError, DocumentErrors, Fault};
use crate::theme::values::shown as entry_text;

#[cfg(test)]
mod reflected_tests;
#[cfg(test)]
mod verdicts_tests;

/// A table of shortcuts, read when an interface takes it
/// ([`Ui::set_shortcuts`](crate::tree::Ui::set_shortcuts)).
#[derive(Clone, Debug)]
pub struct Shortcuts {
    name: String,
    text: String,
}

impl Shortcuts {
    /// The table that `text`, a document of KDL, holds, named `name` in its errors: its file,
    /// usually. A cooked table is its text too, its syntax checked by the cooker.
    pub fn from_text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
        }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The platform whose ways a program follows with keys: which modifier `Primary` is, which moves a
/// text field's caret by words, and how a shortcut is written for a person to read
/// ([`Ui::set_platform`](crate::tree::Ui::set_platform)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Linux, and every system but Windows and Apple's: `Primary` is Ctrl, a shortcut written
    /// `Ctrl+Shift+Z`.
    Linux,
    /// Windows: `Primary` is Ctrl, a shortcut written `Ctrl+Shift+Z`.
    Windows,
    /// macOS, and Apple's other systems: `Primary` is ⌘, a shortcut written `⇧⌘Z`.
    Mac,
}

impl Default for Platform {
    /// The platform that the program is built for, [`Platform::BUILT`].
    fn default() -> Self {
        Self::BUILT
    }
}

impl Platform {
    /// The platform that the program is built for: [`Platform::Mac`] for macOS and iOS,
    /// [`Platform::Windows`] for Windows, [`Platform::Linux`] for every other.
    pub const BUILT: Self = if cfg!(any(target_os = "macos", target_os = "ios")) {
        Self::Mac
    } else if cfg!(windows) {
        Self::Windows
    } else {
        Self::Linux
    };

    /// Every platform, as a table is checked on each.
    pub const ALL: [Self; 3] = [Self::Linux, Self::Windows, Self::Mac];

    /// Whether the platform's modifier of shortcuts, Ctrl or ⌘, is held, and no other but Shift:
    /// what a text field copies, pastes and undoes with.
    pub(crate) fn primary_held(self, modifiers: Modifiers) -> bool {
        match self {
            Self::Mac => modifiers.super_key && !modifiers.control && !modifiers.alt,
            Self::Linux | Self::Windows => {
                modifiers.control && !modifiers.alt && !modifiers.super_key
            }
        }
    }

    /// Whether the platform's modifier that moves a text field's caret by words is held, and no
    /// other but Shift: Option on macOS, Ctrl elsewhere.
    pub(crate) fn word_held(self, modifiers: Modifiers) -> bool {
        match self {
            Self::Mac => modifiers.alt && !modifiers.control && !modifiers.super_key,
            Self::Linux | Self::Windows => self.primary_held(modifiers),
        }
    }

    /// Whether a key pressed with `modifiers` held types its text in a field: without the
    /// modifiers of shortcuts. On Windows, Ctrl and Alt together are AltGr, which types; on macOS,
    /// Option types.
    pub(crate) fn types(self, modifiers: Modifiers) -> bool {
        match self {
            Self::Mac => !modifiers.control && !modifiers.super_key,
            Self::Linux | Self::Windows => {
                modifiers.control == modifiers.alt && !modifiers.super_key
            }
        }
    }
}

/// A shortcut read: in a scope, a chord, and the command it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    pub(crate) scope: String,
    pub(crate) chord: Chord,
    pub(crate) command: String,
}

/// Modifiers and a key, as a shortcut names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Chord {
    primary: bool,
    control: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
    key: ChordKey,
}

/// The keys that a chord is on a platform: its modifiers resolved, `Primary` become Ctrl or ⌘.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Held {
    control: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
}

/// The key of a chord: a character, in lower case, a named key, or a button of the mouse.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ChordKey {
    Character(String),
    Named(NamedKey),
    Button(MouseButton),
}

/// The buttons of the mouse that a chord names, by their names.
const BUTTONS: [(&str, MouseButton); 5] = [
    ("LeftButton", MouseButton::Left),
    ("RightButton", MouseButton::Right),
    ("MiddleButton", MouseButton::Middle),
    ("BackButton", MouseButton::Back),
    ("ForwardButton", MouseButton::Forward),
];

/// The named keys that are modifiers, which no chord ends with.
const MODIFIER_KEYS: [NamedKey; 7] = [
    NamedKey::Alt,
    NamedKey::AltGraph,
    NamedKey::Control,
    NamedKey::Shift,
    NamedKey::Super,
    NamedKey::Meta,
    NamedKey::Hyper,
];

/// The named keys that macOS writes with a symbol in a menu, as Apple lists them
/// (<https://support.apple.com/en-us/102650>); every other is written by its name.
const MAC_SYMBOLS: [(NamedKey, &str); 8] = [
    (NamedKey::Escape, "⎋"),
    (NamedKey::Tab, "⇥"),
    (NamedKey::Backspace, "⌫"),
    (NamedKey::Enter, "⏎"),
    (NamedKey::ArrowUp, "▲"),
    (NamedKey::ArrowDown, "▼"),
    (NamedKey::ArrowLeft, "◀"),
    (NamedKey::ArrowRight, "▶"),
];

impl Chord {
    /// The chord that `text` writes, `Primary+Shift+Z`, or why it writes none.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        // The key comes last, and may itself be `+`: `Primary++`.
        let (modifiers, key) = match text.strip_suffix("++") {
            Some(modifiers) => (modifiers, "+"),
            None if text == "+" => ("", "+"),
            None => text.rsplit_once('+').unwrap_or(("", text)),
        };
        // A `+` joins a modifier to what follows it: `+Z`, `Ctrl++Z` and `++` name none.
        if key != text && modifiers.split('+').any(str::is_empty) {
            return Err(format!(
                "`{text}` joins no modifier by a `+`: modifiers and a key joined by `+`, \
                 `Primary+Shift+Z`, or `Primary++` for the key `+`"
            ));
        }
        if NamedKey::from_name(key).is_some_and(|named| MODIFIER_KEYS.contains(&named)) {
            return Err(format!(
                "`{text}` ends with a modifier, `{key}`, which is held with a key, not pressed for \
                 a command"
            ));
        }
        let mut chord = Self {
            primary: false,
            control: false,
            alt: false,
            shift: false,
            super_key: false,
            key: match (
                NamedKey::from_name(key),
                BUTTONS.iter().find(|(name, _)| *name == key),
            ) {
                (Some(named), _) => ChordKey::Named(named),
                (None, Some(&(_, button))) => ChordKey::Button(button),
                (None, None) if key.chars().count() == 1 => ChordKey::Character(key.to_lowercase()),
                (None, None) => {
                    return Err(format!(
                        "`{text}` does not end with a key: a character, the name of a key, as \
                         `Delete` or `F2`, or of a button, as `LeftButton`; a chord whose key KDL \
                         does not take bare, as `;` or `=`, is written in quotes, \
                         `\"Primary+;\"`"
                    ));
                }
            },
        };
        for modifier in modifiers.split('+').filter(|modifier| !modifier.is_empty()) {
            let held = match modifier {
                "Primary" => &mut chord.primary,
                "Ctrl" => &mut chord.control,
                "Alt" => &mut chord.alt,
                "Shift" => &mut chord.shift,
                "Super" => &mut chord.super_key,
                _ => {
                    return Err(format!(
                        "`{modifier}` is not a modifier: `Primary`, `Ctrl`, `Alt`, `Shift` or \
                         `Super`"
                    ));
                }
            };
            if *held {
                return Err(format!("`{text}` names `{modifier}` twice"));
            }
            *held = true;
        }
        if chord.primary && (chord.control || chord.super_key) {
            return Err(format!(
                "`{text}` names `Primary` with `{}`, which it is on {}: name the keys apart, \
                 `Ctrl+Super`, or `Primary` alone",
                if chord.control { "Ctrl" } else { "Super" },
                if chord.control {
                    "Linux and Windows"
                } else {
                    "macOS"
                },
            ));
        }
        if chord.shift && chord.caseless() {
            return Err(format!(
                "`{key}` has no case: Shift is how a layout types it, if it needs it, and a chord \
                 of it does not name Shift"
            ));
        }
        Ok(chord)
    }

    /// The keys that it is on `platform`.
    fn held(&self, platform: Platform) -> Held {
        let mac = platform == Platform::Mac;
        Held {
            control: self.control || (self.primary && !mac),
            alt: self.alt,
            shift: self.shift,
            super_key: self.super_key || (self.primary && mac),
        }
    }

    /// Whether it is the same keys as `other` on `platform`: `Primary+Z` is `Ctrl+Z` on Linux and
    /// Windows, `Super+Z` on macOS.
    pub(crate) fn same(&self, other: &Self, platform: Platform) -> bool {
        self.key == other.key && self.held(platform) == other.held(platform)
    }

    /// The chord as `platform` writes it for a person to read, in a menu or a list of shortcuts:
    /// on macOS, its modifiers' symbols in Apple's order, Control, Option, Shift, Command, then its
    /// key, `⇧⌘Z`; on Windows, its modifiers in Microsoft's order, the Windows key, `Win`, then
    /// Ctrl, Alt, Shift, joined by `+`, `Ctrl+Shift+Z`; on Linux in the same order, the key with
    /// the system's logo last, `Super`, as GTK writes it. A letter is in upper case.
    pub(crate) fn shown(&self, platform: Platform) -> String {
        let held = self.held(platform);
        let modifiers: [(bool, &str); 4] = match platform {
            Platform::Mac => [
                (held.control, "⌃"),
                (held.alt, "⌥"),
                (held.shift, "⇧"),
                (held.super_key, "⌘"),
            ],
            Platform::Windows => [
                (held.super_key, "Win+"),
                (held.control, "Ctrl+"),
                (held.alt, "Alt+"),
                (held.shift, "Shift+"),
            ],
            Platform::Linux => [
                (held.control, "Ctrl+"),
                (held.alt, "Alt+"),
                (held.shift, "Shift+"),
                (held.super_key, "Super+"),
            ],
        };
        let mut text: String = modifiers
            .into_iter()
            .filter(|&(held, _)| held)
            .map(|(_, name)| name)
            .collect();
        text.push_str(&self.key_text(platform));
        text
    }

    /// Its key's text on `platform`: a letter in upper case, a named key's name, or on macOS its
    /// symbol where Apple gives one; a button's name.
    fn key_text(&self, platform: Platform) -> String {
        match &self.key {
            ChordKey::Character(character) => character.to_uppercase(),
            ChordKey::Named(named) => MAC_SYMBOLS
                .iter()
                .find(|(key, _)| platform == Platform::Mac && key == named)
                .map_or(named.name(), |(_, symbol)| symbol)
                .to_owned(),
            ChordKey::Button(button) => BUTTONS
                .iter()
                .find(|(_, each)| each == button)
                .map(|(name, _)| *name)
                .expect("a chord names one of the buttons that it reads")
                .to_owned(),
        }
    }

    /// Whether it names a button of the mouse, which runs no command as a key does.
    pub(crate) fn is_button(&self) -> bool {
        matches!(self.key, ChordKey::Button(_))
    }

    /// How many modifiers it names, if a press of `button` with `modifiers` held is it on
    /// `platform`: its button, the modifiers that it names held, whatever others are.
    pub(crate) fn holds(
        &self,
        button: MouseButton,
        modifiers: Modifiers,
        platform: Platform,
    ) -> Option<usize> {
        let held = self.held(platform);
        let named = [
            (held.shift, modifiers.shift),
            (held.control, modifiers.control),
            (held.alt, modifiers.alt),
            (held.super_key, modifiers.super_key),
        ];
        (self.key == ChordKey::Button(button) && named.iter().all(|&(named, held)| held || !named))
            .then(|| named.iter().filter(|(named, _)| *named).count())
    }

    /// Whether its key is a character without case, `+` or `1`, which some layouts type with
    /// Shift and others without: Shift is left out of what it matches.
    fn caseless(&self) -> bool {
        match &self.key {
            ChordKey::Character(character) => character.to_uppercase() == *character,
            ChordKey::Named(_) | ChordKey::Button(_) => false,
        }
    }

    /// Whether `event`, a press, `modifiers` held, is this chord on `platform`. A character of
    /// another script than the Latin one, which a layout of that script types, is the letter of
    /// its key's position on a QWERTY keyboard: Primary+Z is the key of Z's position on a Cyrillic
    /// layout.
    pub(crate) fn matches(
        &self,
        event: &KeyEvent,
        modifiers: Modifiers,
        platform: Platform,
    ) -> bool {
        let key = match (&event.logical, &self.key) {
            (Key::Character(character), ChordKey::Character(wanted)) => {
                character.to_lowercase() == *wanted
                    || (!character.is_ascii()
                        && position_letter(event).is_some_and(|letter| letter == *wanted))
            }
            (Key::Named(named), ChordKey::Named(wanted)) => named == wanted,
            _ => false,
        };
        let held = self.held(platform);
        key && (self.caseless() || modifiers.shift == held.shift)
            && modifiers.control == held.control
            && modifiers.alt == held.alt
            && modifiers.super_key == held.super_key
    }
}

/// The chord that `chord` writes, as a table writes it, `Primary+Shift+P`, as `platform` writes it
/// for a person to read, `Ctrl+Shift+P` or `⇧⌘P`, as a menu shows it
/// ([`Ui::command_chord`](crate::tree::Ui::command_chord)): for a text that names a chord outside
/// the tables, as a hint. None for a text that writes no chord.
#[must_use]
pub fn shown(chord: &str, platform: Platform) -> Option<String> {
    Chord::parse(chord).ok().map(|chord| chord.shown(platform))
}

/// The letter of the position of `event`'s key on a QWERTY keyboard, lowercase, which a chord
/// reads for a character of another script than the Latin one; none for a key that holds no
/// letter there.
pub(crate) fn position_letter(event: &KeyEvent) -> Option<String> {
    event
        .physical
        .name()
        .strip_prefix("Key")
        .map(str::to_lowercase)
}

/// Reads the shortcuts of `tables`, in their order, those of a later table after an earlier one's.
///
/// # Errors
///
/// Each error found, up to [`DocumentErrors::MOST`], at its line: a text that is not KDL; a node
/// that is not a scope, or a scope given twice in a table; a chord that names no key or modifier,
/// or two chords of a scope that are the same keys on some platform; a command that is not one
/// name; an annotation, a property or a child where none goes; a name that starts with `$`.
pub(crate) fn bindings(tables: &[Shortcuts]) -> Result<Vec<Binding>, DocumentErrors> {
    let mut errors = DocumentErrors::new();
    let mut bindings = Vec::new();
    for table in tables {
        let document = match uwui_kdl::read(&table.text) {
            Ok(document) => document,
            Err(error) => {
                errors.push(DocumentError {
                    document: table.name.clone(),
                    error: Fault::Kdl(error),
                });
                continue;
            }
        };
        let mut fail = |offset: usize, message: String| {
            errors.push_with(|| DocumentError::at(&table.name, &table.text, offset, message));
        };
        // The scopes of this table, and the keys that a scope's chords are on each platform, with
        // the line where each was first given, to refuse one given twice in time linear in the
        // table; lines counted as the nodes come, in the order of the text.
        let mut scopes: HashMap<String, u32> = HashMap::new();
        let mut lines = Lines::default();
        let mut given: HashMap<(Platform, ChordKey, Held), u32> = HashMap::new();
        for node in document.nodes() {
            let Some(scope) = scope_name(&node, &mut fail) else {
                continue;
            };
            let here = lines.at(&table.text, node.span().start);
            if let Some(&first) = scopes.get(&scope) {
                fail(
                    node.span().start,
                    format!(
                        "the scope `{scope}` is given twice in the table, first at line {first}"
                    ),
                );
                continue;
            }
            scopes.insert(scope.clone(), here);
            given.clear();
            for child in node.children() {
                let Some((chord, command)) = binding(&child, &mut fail) else {
                    continue;
                };
                let at = child.span().start;
                let here = lines.at(&table.text, at);
                let twice = Platform::ALL.into_iter().find_map(|platform| {
                    let keys = (platform, chord.key.clone(), chord.held(platform));
                    given.get(&keys).map(|&first| (platform, first))
                });
                if let Some((platform, first)) = twice {
                    fail(
                        at,
                        format!(
                            "`{}` is the same keys on {} as the chord at line {} of the scope \
                             `{scope}`",
                            child.name().text(),
                            match platform {
                                Platform::Linux => "Linux",
                                Platform::Windows => "Windows",
                                Platform::Mac => "macOS",
                            },
                            first
                        ),
                    );
                    continue;
                }
                for platform in Platform::ALL {
                    let keys = (platform, chord.key.clone(), chord.held(platform));
                    given.insert(keys, here);
                }
                bindings.push(Binding {
                    scope: scope.clone(),
                    chord,
                    command,
                });
            }
        }
    }
    if errors.is_empty() {
        Ok(bindings)
    } else {
        Err(errors)
    }
}

/// The name of the scope that `node` writes, `scope global`; none where it writes none, why told
/// to `fail`.
fn scope_name(node: &Node<'_, '_>, fail: &mut impl FnMut(usize, String)) -> Option<String> {
    let at = node.span().start;
    let name = node.name().text();
    if node.annotation().is_some() {
        fail(at, "a scope takes no type".to_owned());
        return None;
    }
    if name != "scope" {
        fail(
            at,
            format!("`{name}` is not a scope: a table holds scopes, `scope global {{ … }}`"),
        );
        return None;
    }
    let mut arguments = node.arguments();
    let first = arguments.next();
    let one = first.as_ref().and_then(|entry| match entry.value() {
        Value::String(string) if entry.annotation().is_none() => Some(string.text().into_owned()),
        _ => None,
    });
    let Some(scope) = one else {
        let found = first.as_ref().map_or("nothing".to_owned(), entry_text);
        fail(
            first.map_or(at, |entry| entry.span().start),
            format!("a scope is named by a name, `scope global`, not {found}"),
        );
        return None;
    };
    if let Some(extra) = arguments
        .next()
        .or_else(|| node.entries().find(|entry| entry.name().is_some()))
    {
        fail(
            extra.span().start,
            format!("a scope takes its name alone, not {}", entry_text(&extra)),
        );
        return None;
    }
    if scope.is_empty() {
        fail(at, "a scope's name is not empty".to_owned());
        return None;
    }
    if scope.starts_with('$') {
        fail(at, dollar(&scope));
        return None;
    }
    Some(scope)
}

/// The chord and the command that `node`, a child of a scope, writes, `Primary+Z edit.undo`; none
/// where it writes none, why told to `fail`.
fn binding(node: &Node<'_, '_>, fail: &mut impl FnMut(usize, String)) -> Option<(Chord, String)> {
    let at = node.span().start;
    let text = node.name().text();
    if node.annotation().is_some() {
        fail(at, format!("the chord `{text}` takes no type"));
        return None;
    }
    if text.starts_with('$') {
        fail(at, dollar(&text));
        return None;
    }
    let chord = match Chord::parse(&text) {
        Ok(chord) => chord,
        Err(reason) => {
            fail(at, reason);
            return None;
        }
    };
    if let Some(child) = node.children().next() {
        fail(
            child.span().start,
            format!("the chord `{text}` takes no child: it names one command"),
        );
        return None;
    }
    let mut entries = node.entries();
    let first = entries.next();
    let command = first.as_ref().and_then(|entry| match entry.value() {
        Value::String(string) if entry.name().is_none() && entry.annotation().is_none() => {
            Some(string.text().into_owned())
        }
        _ => None,
    });
    let Some(command) = command else {
        let found = first.as_ref().map_or("nothing".to_owned(), |entry| {
            if entry.name().is_some() {
                format!(
                    "a property, `{}`",
                    entry.name().map(|name| name.text()).unwrap_or_default()
                )
            } else {
                entry_text(entry)
            }
        });
        fail(
            first.map_or(at, |entry| entry.span().start),
            format!("the chord `{text}` names a command by its name, not {found}"),
        );
        return None;
    };
    if let Some(extra) = entries.next() {
        fail(
            extra.span().start,
            format!(
                "the chord `{text}` names one command, not {}",
                entry_text(&extra)
            ),
        );
        return None;
    }
    if command.is_empty() {
        fail(
            at,
            format!("the chord `{text}` names a command by its name, not by an empty text"),
        );
        return None;
    }
    if command.starts_with('$') {
        fail(at, dollar(&command));
        return None;
    }
    Some((chord, command))
}

/// Why the name `name`, which starts with `$`, is refused.
fn dollar(name: &str) -> String {
    format!(
        "`{name}`: a name never starts with `$`, which names a theme's value where a value goes"
    )
}

/// A count of a text's lines, moved forward through it: the line of each byte asked for, the bytes
/// asked for in their order, in time linear in the text.
#[derive(Default)]
struct Lines {
    /// The byte counted to.
    offset: usize,
    /// The newlines before it.
    newlines: u32,
}

impl Lines {
    /// The line of the byte `offset` of `text`, from 1, at or past the last asked for, its lines
    /// counted as the reader counts them. An item's start, which these are, never splits `\r\n`.
    fn at(&mut self, text: &str, offset: usize) -> u32 {
        let offset = offset.min(text.len()).max(self.offset);
        let between = &text[self.offset..offset];
        self.newlines += uwui_kdl::Error::at(between, between.len(), "").line - 1;
        self.offset = offset;
        self.newlines + 1
    }
}
