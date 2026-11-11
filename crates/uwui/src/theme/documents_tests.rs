//! The repository's style sheets and themes, read, kept as a fixture: every rule's selector and
//! declarations, and every token's value, for each stack of themes that a sheet is read with
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
//!
//! The fixture, `tests/reference/documents.txt`, was recorded through the reflected reader of the
//! documents in TOML, before they became KDL, and outlives it: the reader of KDL must read the
//! converted documents to the same lines. `SLIME_UPDATE_REFERENCE_IMAGES` writes it again: a change
//! of a document changes it on purpose; a change of the reader must not.

use std::path::Path;

use super::{Erased, Fallback, Length, StyleSheet, Theme, Token, TokenValues, Tokens};

/// The look's sheet, and the `interface` example's.
const LOOK_SHEET: Named = ("editor.style", include_str!("../../look/editor.style.kdl"));
const EXAMPLE_SHEET: Named = (
    "interface.style",
    include_str!("../../../../examples/assets/ui/interface.style.kdl"),
);

/// The look's themes, and the `interface` example's.
const BASE: Named = ("base.theme", include_str!("../../look/base.theme.kdl"));
const DARK: Named = ("dark.theme", include_str!("../../look/dark.theme.kdl"));
const LIGHT: Named = ("light.theme", include_str!("../../look/light.theme.kdl"));
const CONTRAST: Named = (
    "contrast.theme",
    include_str!("../../look/contrast.theme.kdl"),
);
const COMPACT: Named = (
    "compact.theme",
    include_str!("../../look/compact.theme.kdl"),
);
const EXAMPLE_DARK: Named = (
    "example dark.theme",
    include_str!("../../../../examples/assets/ui/dark.theme.kdl"),
);
const EXAMPLE_LIGHT: Named = (
    "example light.theme",
    include_str!("../../../../examples/assets/ui/light.theme.kdl"),
);

/// The `interface` example's own tokens, as `examples/interface.rs` declares them, their fallbacks
/// aside: the fixture records what the themes give them.
const INTERFACE_SPACE: Token<Length> = Token::new("interface_space", Fallback::Value(0.0));
const INTERFACE_GAP: Token<Length> = Token::new("interface_gap", Fallback::Value(0.0));
const EXAMPLE_TOKENS: Tokens = Tokens::new(&[&INTERFACE_SPACE, &INTERFACE_GAP]);

/// A document of the repository: the name it is read under, and its text.
type Named = (&'static str, &'static str);

/// Each sheet and the stacks of themes it is read with: the look's in its six stacks, as
/// `look::themes` stacks them, and the example's in its two.
fn cases() -> Vec<(Named, Vec<Named>)> {
    let mut cases = Vec::new();
    for colors in [DARK, LIGHT, CONTRAST] {
        cases.push((LOOK_SHEET, vec![BASE, colors]));
        cases.push((LOOK_SHEET, vec![BASE, colors, COMPACT]));
    }
    cases.push((EXAMPLE_SHEET, vec![EXAMPLE_DARK]));
    cases.push((EXAMPLE_SHEET, vec![EXAMPLE_LIGHT]));
    cases
}

/// The fixture's lines for every case: a heading naming the sheet and its themes, each token's
/// value, then each rule's selector and its declarations, in the order of their leaves.
fn lines() -> String {
    let declared: Vec<Erased> = super::ROLES.each().chain(EXAMPLE_TOKENS.each()).collect();
    let mut out = String::new();
    for ((sheet_name, sheet), themes) in cases() {
        let names: Vec<&str> = themes.iter().map(|(name, _)| *name).collect();
        out.push_str(&format!("# {sheet_name} over {}\n", names.join(", ")));
        let sheets = [StyleSheet::from_text(sheet_name, sheet)];
        let themes: Vec<Theme> = themes
            .iter()
            .map(|(name, text)| Theme::from_text(*name, *text))
            .collect();
        let mut tokens = TokenValues::default();
        let styling = super::read(&sheets, &themes, &declared, &mut tokens)
            .unwrap_or_else(|error| panic!("{sheet_name} over {names:?}:\n{error}"));
        for token in &declared {
            let value = tokens.value(token);
            out.push_str(&format!("token {} {value:?}\n", token.name));
        }
        out.push_str(&format!("lacking {:?}\n", styling.lacking));
        for (number, rule) in styling.rules.iter().enumerate() {
            out.push_str(&format!("rule {} {:?}\n", number + 1, rule.selector));
            // By leaf: a rule sets each leaf once, and the order of a rule's own declarations,
            // which TOML and KDL write apart, changes nothing that it styles.
            let mut declarations: Vec<_> = rule.declarations.iter().collect();
            declarations.sort_by_key(|declaration| declaration.leaf());
            for declaration in declarations {
                out.push_str(&format!("  {declaration:?}\n"));
            }
        }
    }
    rounded(&out)
}

#[test]
fn the_repositorys_sheets_and_themes_read_as_the_fixture_records() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference/documents.txt");
    let lines = lines();
    if std::env::var_os("SLIME_UPDATE_REFERENCE_IMAGES").is_some() {
        std::fs::write(&path, &lines).expect("the fixture is written");
        return;
    }
    let recorded = std::fs::read_to_string(&path).expect("the fixture is read");
    for (number, (line, want)) in lines.lines().zip(recorded.lines()).enumerate() {
        assert_eq!(line, want, "line {} of {}", number + 1, path.display());
    }
    assert_eq!(
        lines.lines().count(),
        recorded.lines().count(),
        "the lines of {}",
        path.display()
    );
}

/// `text` with each number that holds a point or an exponent written to four places: the look's
/// colours are computed through OKLCH's cube roots and arctangents, whose last bits differ between
/// the platforms' mathematical libraries, as `examples/common/style_fixture.rs` rounds them.
fn rounded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(|c: char| c.is_ascii_digit() || c == '-') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest
            .char_indices()
            .skip(1)
            .find(|&(_, c)| !(c.is_ascii_digit() || matches!(c, '.' | 'e' | '-')))
            .map_or(rest.len(), |(at, _)| at);
        let number = &rest[..end];
        match number.parse::<f64>() {
            Ok(value) if number.contains(['.', 'e']) => out.push_str(&format!("{value:.4}")),
            _ => out.push_str(number),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}
