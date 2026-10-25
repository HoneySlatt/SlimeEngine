//! Shortcuts: documents that name a command for a key and its modifiers, in a scope
//! (`docs/adr/0142-input-and-focus.md`).
//!
//! A table of shortcuts holds a table for each scope: `global`, or the class of an element, whose
//! shortcuts hold while the focus lies inside it. Each key is a *chord*, modifiers and a key joined
//! by `+`, and its value the name of the command that it gives the program
//! ([`ActionKind::Command`](crate::action::ActionKind::Command)):
//!
//! ```toml
//! [global]
//! "Ctrl+Z" = "undo"
//! "Ctrl+Shift+Z" = "redo"
//! F2 = "rename"
//!
//! [hierarchy]
//! Delete = "delete"
//! ```
//!
//! A chord names its key by what the key means with the keyboard's layout, its *logical* key: a
//! character, `Z`, which a letter of either case matches, or the name of a named key, `Delete`,
//! `F2`, `ArrowUp`; so Ctrl+Z is the key labelled Z, wherever the layout puts it. Its modifiers are
//! `Ctrl`, `Shift`, `Alt` and `Super`, all held and no other, and `+` is written last, `Ctrl++`.
//!
//! Two cases bend the rule. A character without case, `+` or `1`, is typed with Shift on some
//! layouts and without on others: its chord does not name Shift, and matches either way. A layout
//! of another script than the Latin one types no Latin letter: there, a chord of a letter is the
//! key at that letter's position on a QWERTY keyboard.

use slime_input::{Key, KeyEvent, Modifiers, NamedKey};
use slime_reflect::{Problem, Registry, Value};

use crate::DocumentError;
use crate::document::Source;

/// A table of shortcuts, read when an interface takes it
/// ([`Ui::set_shortcuts`](crate::tree::Ui::set_shortcuts)).
#[derive(Clone, Debug)]
pub struct Shortcuts {
    name: String,
    source: Source,
}

impl Shortcuts {
    /// The table that `text` holds, named `name` in its errors: its file, usually.
    pub fn from_text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: Source::Text(text.into()),
        }
    }

    /// The table that `bytes`, the binary encoding of its document, hold, as the cooker writes
    /// them.
    pub fn from_binary(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            source: Source::Binary(bytes),
        }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
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
    shift: bool,
    control: bool,
    alt: bool,
    super_key: bool,
    key: ChordKey,
}

/// The key of a chord: a character, in lower case, or a named key.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ChordKey {
    Character(String),
    Named(NamedKey),
}

impl Chord {
    /// The chord that `text` writes, `Ctrl+Shift+Z`, or why it writes none.
    fn parse(text: &str) -> Result<Self, String> {
        // The key comes last, and may itself be `+`: `Ctrl++`.
        let (modifiers, key) = match text.strip_suffix("++") {
            Some(modifiers) => (modifiers, "+"),
            None if text == "+" => ("", "+"),
            None => text.rsplit_once('+').unwrap_or(("", text)),
        };
        let mut chord = Self {
            shift: false,
            control: false,
            alt: false,
            super_key: false,
            key: match NamedKey::from_name(key) {
                Some(named) => ChordKey::Named(named),
                None if key.chars().count() == 1 => ChordKey::Character(key.to_lowercase()),
                None => {
                    return Err(format!(
                        "`{text}` does not end with a key: a character, or the name of a key, as \
                         `Delete` or `F2`"
                    ));
                }
            },
        };
        for modifier in modifiers.split('+').filter(|modifier| !modifier.is_empty()) {
            let held = match modifier {
                "Ctrl" => &mut chord.control,
                "Shift" => &mut chord.shift,
                "Alt" => &mut chord.alt,
                "Super" => &mut chord.super_key,
                _ => {
                    return Err(format!(
                        "`{modifier}` is not a modifier: `Ctrl`, `Shift`, `Alt` or `Super`"
                    ));
                }
            };
            *held = true;
        }
        if chord.shift && chord.caseless() {
            return Err(format!(
                "`{key}` has no case: Shift is how a layout types it, if it needs it, and a chord \
                 of it does not name Shift"
            ));
        }
        Ok(chord)
    }

    /// Whether its key is a character without case, `+` or `1`, which some layouts type with
    /// Shift and others without: Shift is left out of what it matches.
    fn caseless(&self) -> bool {
        match &self.key {
            ChordKey::Character(character) => character.to_uppercase() == *character,
            ChordKey::Named(_) => false,
        }
    }

    /// Whether `event`, a press, `modifiers` held, is this chord. A character of another script
    /// than the Latin one, which a layout of that script types, is the letter of its key's
    /// position on a QWERTY keyboard: Ctrl+Z is the key of Z's position on a Cyrillic layout.
    pub(crate) fn matches(&self, event: &KeyEvent, modifiers: Modifiers) -> bool {
        let key = match (&event.logical, &self.key) {
            (Key::Character(character), ChordKey::Character(wanted)) => {
                character.to_lowercase() == *wanted
                    || (!character.is_ascii()
                        && event
                            .physical
                            .name()
                            .strip_prefix("Key")
                            .is_some_and(|letter| letter.to_lowercase() == *wanted))
            }
            (Key::Named(named), ChordKey::Named(wanted)) => named == wanted,
            _ => false,
        };
        key && (self.caseless() || modifiers.shift == self.shift)
            && modifiers.control == self.control
            && modifiers.alt == self.alt
            && modifiers.super_key == self.super_key
    }
}

/// Reads the shortcuts of `tables`, in their order, those of a later table after an earlier one's.
pub(crate) fn bindings(tables: &[Shortcuts]) -> Result<Vec<Binding>, DocumentError> {
    let mut registry = Registry::new();
    registry.register::<String>();
    let mut bindings = Vec::new();
    for table in tables {
        let fault = |error| DocumentError {
            document: table.name.clone(),
            error,
        };
        let mut document = table.source.parse(&registry).map_err(fault)?;
        for scope in document.keys(&[]) {
            if !document.is_table(&[&scope]) {
                let problem = Problem::Invalid(format!(
                    "`{scope}` is not a scope: a scope is a table of chords and commands"
                ));
                return Err(fault(document.error_at(&[&scope], problem)));
            }
            for chord in document.keys(&[&scope]) {
                let parsed = Chord::parse(&chord).map_err(|reason| {
                    fault(document.error_at(&[&scope, &chord], Problem::Invalid(reason)))
                })?;
                let command = document
                    .read(&[&scope, &chord], "String", None)
                    .map_err(fault)?;
                let Some(Value::Text(command)) = command else {
                    let problem = Problem::Invalid("a command is named by a text".to_owned());
                    return Err(fault(document.error_at(&[&scope, &chord], problem)));
                };
                bindings.push(Binding {
                    scope: scope.clone(),
                    chord: parsed,
                    command,
                });
            }
        }
    }
    Ok(bindings)
}
