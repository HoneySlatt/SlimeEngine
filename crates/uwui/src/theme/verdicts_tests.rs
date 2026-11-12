//! Random stacks of themes and random style sheets, written in KDL, read to the tokens and rules
//! that the reflected reader of themes and sheets in TOML read, or refused as it refused them: its
//! verdicts on fixed seeds, recorded before reflection left uwui (`crate::verdicts_tests`,
//! `tests/reference/verdicts/themes.txt` and `sheets.txt`).
//!
//! The random documents left out what the reader of KDL refuses on purpose where the reflected one
//! took it, a number past an `f32`, a share that makes a color no finite number, a value given
//! wrongly then given again, a theme's value of another kind than its field's, a negative duration,
//! a name that starts with `$`, a key written `"#3"`, and what it takes that the reflected one
//! refused, a text named by a theme: `tests/themes.rs` and `tests/sheets.rs` test each refusal.
//! The random sheets set every leaf of a style.

use std::fmt::Write as _;

use slime_color::LinearColor;

use super::{Erased, StyleSheet, Styling, Theme, TokenValues};
use crate::verdicts_tests;

/// The fixtures' names.
pub(super) const THEMES: &str = "themes";
pub(super) const SHEETS: &str = "sheets";

/// The names that random themes give, by kind, their first letter: a name keeps its kind in every
/// theme of a stack.
pub(super) const COLORS: [&str; 6] = ["c0", "c1", "c2", "c3", "c4", "c5"];
pub(super) const LENGTHS: [&str; 3] = ["l0", "l1", "l2"];
pub(super) const NUMBERS: [&str; 3] = ["n0", "n1", "n2"];
pub(super) const DURATIONS: [&str; 2] = ["d0", "d1"];
pub(super) const TEXTS: [&str; 2] = ["t0", "t1"];

/// The theme that random sheets name: colors, lengths, a number, a whole number and a duration,
/// each of its group.
pub(super) const SHEETS_THEME: &str = "color {\n    sc0 \"#204060\"\n    sc1 \"mix($sc0, #FFFFFF, \
    30%)\"\n}\nlength {\n    sl0 6\n    sl1 -2.5\n}\nnumber {\n    sn0 0.75\n    sn1 3\n}\n\
    duration {\n    sd0 (s)0.15\n}\n";

/// The label of the input that lists the names of the tokens declared, apart from the documents.
pub(super) const DECLARED: &str = "declared";

/// A token of each name in `names`, of its kind, falling back on nothing.
pub(super) fn tokens_of(names: &[&'static str]) -> Vec<Erased> {
    use super::{Color, Duration, Fallback, Length, Number, Token};
    names
        .iter()
        .filter_map(|&name| match &name[..1] {
            "c" => Some(
                Token::<Color>::new(name, Fallback::Value(LinearColor::rgba(0.0, 0.0, 0.0, 0.0)))
                    .erased(),
            ),
            "l" => Some(Token::<Length>::new(name, Fallback::Value(0.0)).erased()),
            "n" => Some(Token::<Number>::new(name, Fallback::Value(0.0)).erased()),
            "d" => Some(
                Token::<Duration>::new(name, Fallback::Value(std::time::Duration::ZERO)).erased(),
            ),
            _ => None,
        })
        .collect()
}

/// What `read` gave, the tokens of `declared` taking their values from `tokens`, as a verdict's
/// lines: the tokens that the themes lack, each token's value, then each rule's selector and its
/// declarations in the order of their leaves, since a rule sets each leaf once and TOML and KDL
/// order a rule's fields apart; or `refused`.
pub(super) fn verdict<E>(
    read: Result<Styling, E>,
    declared: &[Erased],
    tokens: &TokenValues,
) -> String {
    let Ok(styling) = read else {
        return "refused\n".to_owned();
    };
    let mut out = String::new();
    writeln!(out, "lacking {:?}", styling.lacking).expect("written");
    for token in declared {
        writeln!(out, "token {} {:?}", token.name, tokens.value(token)).expect("written");
    }
    for (number, rule) in styling.rules.iter().enumerate() {
        writeln!(out, "rule {} {:?}", number + 1, rule.selector).expect("written");
        let mut declarations: Vec<_> = rule.declarations.iter().collect();
        declarations.sort_by_key(|declaration| declaration.leaf());
        for declaration in declarations {
            writeln!(out, "  {declaration:?}").expect("written");
        }
    }
    out
}

/// The name among the random themes' that `name` writes.
fn known(name: &str) -> &'static str {
    [
        &COLORS[..],
        &LENGTHS[..],
        &NUMBERS[..],
        &DURATIONS[..],
        &TEXTS[..],
    ]
    .concat()
    .into_iter()
    .find(|known| *known == name)
    .unwrap_or_else(|| panic!("`{name}` is a random theme's name"))
}

#[test]
fn random_stacks_of_themes_read_as_the_reflected_reader_read_them() {
    let path = verdicts_tests::path(THEMES);
    let cases = verdicts_tests::read(&path);
    assert!(cases.len() >= 100, "{} cases", cases.len());
    let mut refused = 0;
    for (number, case) in cases.iter().enumerate() {
        let (mut sheets, mut themes, mut declared) = (Vec::new(), Vec::new(), Vec::new());
        for (label, text) in &case.inputs {
            match label.as_str() {
                DECLARED => {
                    declared = tokens_of(&text.split_whitespace().map(known).collect::<Vec<_>>())
                }
                "sheet" => sheets.push(StyleSheet::from_text("random.style.kdl", text.as_str())),
                _ => themes.push(Theme::from_text("random.theme.kdl", text.as_str())),
            }
        }
        let mut tokens = TokenValues::default();
        let read = super::read(&sheets, &themes, &declared, &mut tokens);
        let verdict = verdict(read, &declared, &tokens);
        refused += usize::from(verdict == "refused\n");
        verdicts_tests::check(&path, number, &verdict, &case.verdict);
    }
    // Both outcomes are recorded.
    assert!(
        refused > 20 && refused < cases.len() * 3 / 4,
        "{refused} refused"
    );
}

#[test]
fn random_style_sheets_read_as_the_reflected_reader_read_them() {
    let path = verdicts_tests::path(SHEETS);
    let cases = verdicts_tests::read(&path);
    assert!(cases.len() >= 120, "{} cases", cases.len());
    let theme = [Theme::from_text("sheets.theme.kdl", SHEETS_THEME)];
    let (mut refused, mut declarations) = (0, 0);
    for (number, case) in cases.iter().enumerate() {
        let [(_, text)] = case.inputs.as_slice() else {
            panic!("case {number}: one sheet");
        };
        let mut tokens = TokenValues::default();
        let read = super::read(
            &[StyleSheet::from_text("random.style.kdl", text.as_str())],
            &theme,
            &[],
            &mut tokens,
        );
        if let Ok(styling) = &read {
            declarations += styling
                .rules
                .iter()
                .map(|rule| rule.declarations.len())
                .sum::<usize>();
        }
        let verdict = verdict(read, &[], &tokens);
        refused += usize::from(verdict == "refused\n");
        verdicts_tests::check(&path, number, &verdict, &case.verdict);
    }
    // Both outcomes are recorded, and declarations of many leaves.
    assert!(
        refused > 10 && refused < cases.len() / 2 && declarations > 500,
        "{refused} refused, {declarations} declarations"
    );
}
