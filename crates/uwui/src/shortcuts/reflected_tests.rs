//! The reader of tables of shortcuts in KDL checked against the reflected reader of tables in TOML
//! that it replaced (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`):
//! random tables, written in both notations, read to the same shortcuts, or are both refused.
//!
//! The reflected reader is kept here as the test's oracle while `slime-reflect` is in uwui, until
//! M6d.3c3; the fixture of the editor's tables, `crates/slime-editor/tests/reference/shortcuts.txt`,
//! recorded through it, outlives it. Where the reflected reader read `Ctrl`, the reader of KDL
//! reads `Primary` in half the scopes, as the repository's tables were converted: the chords are
//! then compared as Linux and Windows resolve them, where `Primary` is Ctrl.

use slime_reflect::{Problem, Registry, Value};
use slime_serialize::{Document, Mode};

use super::{Binding, Chord, ChordKey, Platform, Shortcuts, bindings};

/// The shortcuts of `tables`, each `(name, TOML)`, as the reflected reader read them: in their
/// order, a chord as it parsed one, `Ctrl` for the Control key. Its errors as their text.
fn reflected(tables: &[(&str, &str)]) -> Result<Vec<Binding>, String> {
    let mut registry = Registry::new();
    registry.register::<String>();
    let mut bindings = Vec::new();
    for &(name, text) in tables {
        let fault = |error: slime_serialize::ReadError| format!("{name}: {error}");
        let mut document = Document::parse(&registry, text, Mode::Strict).map_err(fault)?;
        for scope in document.keys(&[]) {
            if !document.is_table(&[&scope]) {
                let problem = Problem::Invalid(format!(
                    "`{scope}` is not a scope: a scope is a table of chords and commands"
                ));
                return Err(fault(document.error_at(&[&scope], problem)));
            }
            for chord in document.keys(&[&scope]) {
                let parsed = old_chord(&chord).map_err(|reason| {
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

/// The chord that `text` wrote for the reflected reader, `Ctrl+Shift+Z`, or why it wrote none: its
/// modifiers `Ctrl`, `Shift`, `Alt` and `Super`, as it parsed them.
fn old_chord(text: &str) -> Result<Chord, String> {
    let (modifiers, key) = match text.strip_suffix("++") {
        Some(modifiers) => (modifiers, "+"),
        None if text == "+" => ("", "+"),
        None => text.rsplit_once('+').unwrap_or(("", text)),
    };
    let mut chord = Chord {
        primary: false,
        control: false,
        alt: false,
        shift: false,
        super_key: false,
        key: match (
            slime_input::NamedKey::from_name(key),
            super::BUTTONS.iter().find(|(name, _)| *name == key),
        ) {
            (Some(named), _) => ChordKey::Named(named),
            (None, Some(&(_, button))) => ChordKey::Button(button),
            (None, None) if key.chars().count() == 1 => ChordKey::Character(key.to_lowercase()),
            (None, None) => return Err(format!("`{text}` does not end with a key")),
        },
    };
    for modifier in modifiers.split('+').filter(|modifier| !modifier.is_empty()) {
        let held = match modifier {
            "Ctrl" => &mut chord.control,
            "Shift" => &mut chord.shift,
            "Alt" => &mut chord.alt,
            "Super" => &mut chord.super_key,
            _ => return Err(format!("`{modifier}` is not a modifier")),
        };
        *held = true;
    }
    if chord.shift && chord.caseless() {
        return Err(format!("`{key}` has no case"));
    }
    Ok(chord)
}

/// A generator of numbers that repeats from its seed: xorshift.
struct Numbers(u64);

impl Numbers {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }

    fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[self.below(from.len())]
    }
}

/// The keys of random chords: letters of either case, characters without case, named keys and
/// buttons.
const KEYS: [&str; 21] = [
    "Z",
    "z",
    "A",
    "q",
    "1",
    "+",
    ".",
    "-",
    "=",
    ";",
    "Delete",
    "F2",
    "ArrowUp",
    "Enter",
    "Tab",
    "Home",
    "LeftButton",
    "MiddleButton",
    "RightButton",
    "ж",
    "é",
];

/// The characters without case among [`KEYS`], whose chord names no Shift.
const CASELESS: [&str; 6] = ["1", "+", ".", "-", "=", ";"];

/// The modifiers of random chords, now and then one that is none.
const MODIFIERS: [&str; 5] = ["Ctrl", "Alt", "Shift", "Super", "Hyper"];

const SCOPES: [&str; 4] = ["global", "hierarchy", "viewport", "panel-1"];
const COMMANDS: [&str; 5] = [
    "undo",
    "edit.redo",
    "file.save",
    "orbit",
    "view.frame_selection",
];

/// Whether `text` may be written in KDL as a bare name: starting with a letter, holding none of the
/// characters that KDL keeps for itself.
fn bare(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_alphabetic)
        && !text
            .chars()
            .any(|c| "\\/(){};[]\"#=".contains(c) || c.is_whitespace())
}

/// `text` as a KDL string: bare where it may be and `quote` does not ask for quotes, quoted else.
fn kdl_string(text: &str, quote: bool) -> String {
    if bare(text) && !quote {
        text.to_owned()
    } else {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// A random table, as TOML and as KDL: one to three scopes, each of chords that no other chord of
/// the scope writes alike, mostly valid, now and then one that names no key or modifier.
fn random_table(numbers: &mut Numbers) -> (String, String) {
    let mut toml = String::new();
    let mut kdl = String::new();
    let mut scopes: Vec<&str> = Vec::new();
    for _ in 0..1 + numbers.below(3) {
        let scope = numbers.pick(&SCOPES);
        if scopes.contains(&scope) {
            continue;
        }
        scopes.push(scope);
        toml.push_str(&format!("[{scope}]\n"));
        kdl.push_str(&format!(
            "scope {} {{\n",
            kdl_string(scope, numbers.chance(4))
        ));
        let mut chords: Vec<Chord> = Vec::new();
        // A scope whose KDL writes `Primary` for `Ctrl` names no `Super`, which `Primary` is on
        // macOS: two chords that the reader of KDL refuses there, as the same keys.
        let primary = numbers.chance(2);
        for _ in 0..numbers.below(7) {
            let mut written: Vec<&str> = Vec::new();
            for modifier in MODIFIERS {
                if primary && modifier == "Super" {
                    continue;
                }
                // Hyper, which names no modifier, rarely.
                let one_in = if modifier == "Hyper" { 40 } else { 3 };
                if numbers.chance(one_in) {
                    written.push(modifier);
                }
            }
            // The modifiers in any order: a table may write them so.
            if written.len() > 1 && numbers.chance(2) {
                written.reverse();
            }
            // Now and then what names no key, or Shift with a character without case.
            let key = if numbers.chance(80) {
                "Foo"
            } else {
                numbers.pick(&KEYS)
            };
            if CASELESS.contains(&key) && !numbers.chance(20) {
                written.retain(|modifier| *modifier != "Shift");
            }
            written.push(key);
            let text = written.join("+");
            // A chord that another of the scope already is: TOML allows two texts of one chord,
            // which KDL refuses, so neither is written.
            if let Ok(chord) = old_chord(&text) {
                if chords.contains(&chord) {
                    continue;
                }
                chords.push(chord);
            }
            let command = numbers.pick(&COMMANDS);
            toml.push_str(&format!("\"{text}\" = \"{command}\"\n"));
            let kdl_text = if primary {
                written
                    .iter()
                    .map(|part| if *part == "Ctrl" { "Primary" } else { part })
                    .collect::<Vec<_>>()
                    .join("+")
            } else {
                text.clone()
            };
            kdl.push_str(&format!(
                "    {} {}\n",
                kdl_string(&kdl_text, numbers.chance(4)),
                kdl_string(command, numbers.chance(4))
            ));
        }
        kdl.push_str("}\n");
    }
    (toml, kdl)
}

#[test]
fn random_tables_read_as_the_reflected_reader_read_them() {
    let seed = std::env::var("UWUI_SHORTCUTS_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |time| time.as_nanos() as u64)
                | 1
        });
    let mut numbers = Numbers(seed);
    let (mut agreed, mut refused) = (0, 0);
    for case in 0..2_000 {
        let mut toml = Vec::new();
        let mut kdl = Vec::new();
        for table in 0..1 + numbers.below(2) {
            let (toml_text, kdl_text) = random_table(&mut numbers);
            toml.push((format!("table {table}"), toml_text));
            kdl.push(Shortcuts::from_text(format!("table {table}"), kdl_text));
        }
        let toml_tables: Vec<(&str, &str)> = toml
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str()))
            .collect();
        let want = reflected(&toml_tables);
        let got = bindings(&kdl);
        let context = || {
            let toml: Vec<&str> = toml.iter().map(|(_, text)| text.as_str()).collect();
            let kdl: Vec<&str> = kdl.iter().map(|table| table.text.as_str()).collect();
            format!(
                "case {case}, from the seed UWUI_SHORTCUTS_SEED={seed}:\nTOML:\n{}\nKDL:\n{}",
                toml.join("\n"),
                kdl.join("\n")
            )
        };
        match (want, got) {
            (Ok(want), Ok(got)) => {
                // The same shortcuts, a chord the same keys on Linux and Windows.
                let same = got.len() == want.len()
                    && got.iter().zip(&want).all(|(got, want)| {
                        got.scope == want.scope
                            && got.command == want.command
                            && [Platform::Linux, Platform::Windows]
                                .iter()
                                .all(|&platform| got.chord.same(&want.chord, platform))
                    });
                assert!(same, "{}\nwant {want:?}\ngot {got:?}", context());
                agreed += 1;
            }
            (Err(_), Err(_)) => refused += 1,
            (want, got) => panic!(
                "{}\nthe reflected reader: {want:?}\nthe reader of KDL: {got:?}",
                context()
            ),
        }
    }
    // Both outcomes are tried, most tables read.
    assert!(
        agreed > 1_000 && refused > 50,
        "{agreed} read, {refused} refused, from the seed {seed}"
    );
}
