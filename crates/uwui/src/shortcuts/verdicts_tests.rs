//! Random tables of shortcuts, written in KDL, read to the shortcuts that the reflected reader of
//! tables in TOML read, or refused as it refused them: its verdicts on a fixed seed, recorded
//! before reflection left uwui (`crate::verdicts_tests`, `tests/reference/verdicts/shortcuts.txt`).
//! Where the reflected reader read `Ctrl`, half the scopes of the KDL write `Primary`, as the
//! repository's tables were converted: a chord's verdict is the keys that it is on Linux and on
//! Windows, where `Primary` is Ctrl.

use super::{Binding, Held, Platform, Shortcuts, bindings};
use crate::verdicts_tests;

/// The fixture's name.
pub(super) const FIXTURE: &str = "shortcuts";

/// The shortcuts read as a verdict's lines, each its scope, its command, its key and the
/// modifiers it holds on Linux and on Windows, `held(Linux)/held(Windows)`; or `refused`.
pub(super) fn verdict<E>(read: Result<Vec<Binding>, E>) -> String {
    let Ok(read) = read else {
        return "refused\n".to_owned();
    };
    read.iter()
        .map(|binding| {
            format!(
                "{:?} {:?} {:?} {}/{}\n",
                binding.scope,
                binding.command,
                binding.chord.key,
                held(binding.chord.held(Platform::Linux)),
                held(binding.chord.held(Platform::Windows))
            )
        })
        .collect()
}

/// The modifiers of `held` as their letters, Control, Alt, Shift and Super, in that order:
/// `held(cs)`.
fn held(held: Held) -> String {
    let letters: String = [
        (held.control, 'c'),
        (held.alt, 'a'),
        (held.shift, 's'),
        (held.super_key, 'w'),
    ]
    .iter()
    .filter_map(|&(on, letter)| on.then_some(letter))
    .collect();
    format!("held({letters})")
}

#[test]
fn random_tables_read_as_the_fixture_records() {
    let path = verdicts_tests::path(FIXTURE);
    let cases = verdicts_tests::read(&path);
    assert!(cases.len() >= 150, "{} cases", cases.len());
    let mut refused = 0;
    for (number, case) in cases.iter().enumerate() {
        let tables: Vec<Shortcuts> = case
            .inputs
            .iter()
            .map(|(name, text)| Shortcuts::from_text(name.as_str(), text.as_str()))
            .collect();
        let verdict = verdict(bindings(&tables));
        refused += usize::from(verdict == "refused\n");
        verdicts_tests::check(&path, number, &verdict, &case.verdict);
    }
    // Both outcomes are recorded.
    assert!(
        refused > 10 && refused < cases.len() / 2,
        "{refused} refused"
    );
}
