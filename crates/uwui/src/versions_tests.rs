//! Each version of each format of uwui's documents read from its fixture, with its node `version`
//! and without it, and a later version refused
//! (`docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md`).
//!
//! The fixtures, in `tests/versions`, are never changed once their version is out: a later reader
//! reads them, migrated.

use crate::DocumentErrors;
use crate::shortcuts::{self, Shortcuts};
use crate::theme::{
    self, Duration, Fallback, Length, Number, StyleSheet, Theme, Token, TokenValues, Tokens,
};

/// Version 1 of each format, as written.
const THEME_1: &str = include_str!("../tests/versions/1.theme.kdl");
const SHEET_1: &str = include_str!("../tests/versions/1.style.kdl");
const KEYS_1: &str = include_str!("../tests/versions/1.keys.kdl");

/// The line of `text` that writes its version: `version 1`.
const VERSION_LINE: &str = "version 1\n";

/// `text` without its version: a document of before the versions.
fn unmarked(text: &str) -> String {
    assert!(text.contains(VERSION_LINE), "a fixture writes its version");
    text.replacen(VERSION_LINE, "", 1)
}

/// `text` of the version `version`.
fn of_version(text: &str, version: &str) -> String {
    text.replacen(VERSION_LINE, &format!("version {version}\n"), 1)
}

/// Tokens that name the theme's values of each kind but colors, which the roles name.
const GAP: Token<Length> = Token::new("gap", Fallback::Value(0.0));
const SLOW: Token<Duration> = Token::new("slow", Fallback::Value(std::time::Duration::ZERO));
const BLEND: Token<Number> = Token::new("blend", Fallback::Value(0.0));
const FIXTURE_TOKENS: Tokens = Tokens::new(&[&GAP, &SLOW, &BLEND]);

/// The rules and tokens that `sheet` over `theme` give: the rules written out, to be compared.
fn styled(sheet: &str, theme: &str) -> Result<(String, TokenValues), DocumentErrors> {
    let declared: Vec<_> = theme::ROLES.each().chain(FIXTURE_TOKENS.each()).collect();
    let mut tokens = TokenValues::default();
    let styling = theme::read(
        &[StyleSheet::from_text("1.style.kdl", sheet)],
        &[Theme::from_text("1.theme.kdl", theme)],
        &declared,
        &mut tokens,
    )?;
    let rules: Vec<_> = styling
        .rules
        .iter()
        .map(|rule| format!("{:?} {:?}", rule.selector, rule.declarations))
        .collect();
    Ok((format!("{rules:#?}\n{:?}", styling.lacking), tokens))
}

/// The shortcuts that `table` binds, written out to be compared.
fn bound(table: &str) -> Result<String, DocumentErrors> {
    shortcuts::bindings(&[Shortcuts::from_text("1.keys.kdl", table)])
        .map(|bindings| format!("{bindings:#?}"))
}

/// The one error of `result`, whose version it keeps, at `line`, naming both versions.
fn refused_as_later<T>(result: Result<T, DocumentErrors>, what: &str, line: u32) {
    let Err(errors) = result else {
        panic!("{what} of version 2 is refused");
    };
    assert_eq!(errors.errors().len(), 1, "{errors}");
    let error = &errors.errors()[0];
    assert_eq!(error.later, Some(2), "{error}");
    assert_eq!(error.error.line, line, "{error}");
    assert!(
        error.to_string().contains(&format!(
            "{what} is of the format's version 2, later than 1, the last that this program reads"
        )),
        "{error}"
    );
}

#[test]
fn a_theme_and_a_sheet_of_version_1_read_the_same_with_their_version_and_without() {
    let marked = styled(SHEET_1, THEME_1).unwrap_or_else(|errors| panic!("{errors}"));
    assert_eq!(marked.0.matches("Selector").count(), 5, "{}", marked.0);
    for token in ["\"gap\"", "\"slow\"", "\"blend\""] {
        assert!(
            !marked.0.contains(token),
            "the theme gives {token}: {}",
            marked.0
        );
    }
    for (sheet, theme) in [
        (unmarked(SHEET_1), THEME_1.to_owned()),
        (SHEET_1.to_owned(), unmarked(THEME_1)),
        (unmarked(SHEET_1), unmarked(THEME_1)),
    ] {
        let read = styled(&sheet, &theme).unwrap_or_else(|errors| panic!("{errors}"));
        assert_eq!(read, marked);
    }
}

#[test]
fn a_table_of_version_1_reads_the_same_with_its_version_and_without() {
    let marked = bound(KEYS_1).unwrap_or_else(|errors| panic!("{errors}"));
    assert!(
        marked.contains("edit.undo") && marked.contains("orbit"),
        "{marked}"
    );
    let read = bound(&unmarked(KEYS_1)).unwrap_or_else(|errors| panic!("{errors}"));
    assert_eq!(read, marked);
}

#[test]
fn a_later_version_is_refused_whole_naming_both_versions() {
    // The fixtures' version is on their third line, after two lines of comment.
    refused_as_later(styled(SHEET_1, &of_version(THEME_1, "2")), "the theme", 3);
    refused_as_later(
        styled(&of_version(SHEET_1, "2"), THEME_1),
        "the style sheet",
        3,
    );
    refused_as_later(bound(&of_version(KEYS_1, "2")), "the table of shortcuts", 3);
    // Whatever the rest holds: a later version may mean anything by it.
    refused_as_later(
        bound("version 2\nscope global { Hyper+Z undo; }\nnot-a-scope\n"),
        "the table of shortcuts",
        1,
    );
}

#[test]
fn the_values_of_a_theme_refused_as_later_are_not_told_missing() {
    // A theme of a later version gives `accent`; the theme stacked after it names it.
    let errors = theme::values::read(&[
        (
            "later.theme.kdl",
            "version 2\ncolor { accent \"#4C6EF5\"; }\n",
        ),
        (
            "after.theme.kdl",
            "color { soft \"alpha($accent, 20%)\"; }\n",
        ),
    ])
    .expect_err("a later theme is refused");
    assert_eq!(errors.errors().len(), 1, "{errors}");
    assert_eq!(errors.errors()[0].later, Some(2));
}

#[test]
fn a_version_written_wrongly_or_out_of_place_is_refused_at_its_line() {
    for (version, problem) in [
        ("version 0", "one whole number from 1 alone"),
        ("version -1", "one whole number from 1 alone"),
        ("version 1.0", "one whole number from 1 alone"),
        ("version \"1\"", "one whole number from 1 alone"),
        ("version", "one whole number from 1 alone"),
        ("version 1 2", "one whole number from 1 alone"),
        ("version 1 at=2", "one whole number from 1 alone"),
        ("version (v)1", "one whole number from 1 alone"),
        ("(v)version 1", "one whole number from 1 alone"),
        ("version 1 { minor 2; }", "one whole number from 1 alone"),
        (
            "version 99999999999999999999",
            "one whole number from 1 alone",
        ),
    ] {
        let table = format!("{version}\nscope global {{ F2 rename; }}\n");
        let errors = bound(&table).expect_err(&table);
        assert_eq!(errors.errors().len(), 1, "{table}: {errors}");
        let error = &errors.errors()[0];
        assert_eq!(
            (error.error.line, error.later),
            (1, None),
            "{table}: {error}"
        );
        assert!(error.to_string().contains(problem), "{table}: {error}");
    }
    // The rest of a document whose version is written wrongly is read as version 1, its errors
    // told too.
    let errors = bound("version \"1\"\nnot-a-scope\n").expect_err("two errors");
    assert_eq!(errors.errors().len(), 2, "{errors}");
    // Given again, or anywhere but first: a style sheet names no rule `version`.
    for (document, read) in [
        (
            "version 1\nscope global { F2 rename; }\nversion 1\n",
            bound as fn(&str) -> Result<_, DocumentErrors>,
        ),
        ("scope global { F2 rename; }\nversion 1\n", bound),
        (
            "panel { select { classes panel; }; }\nversion { select kind=Label; }\n",
            |sheet| styled(sheet, THEME_1).map(|(rules, _)| rules),
        ),
        ("color { a \"#000000\"; }\nversion 1\n", |theme| {
            styled("", theme).map(|(rules, _)| rules)
        }),
    ] {
        let errors = read(document).expect_err(document);
        assert_eq!(errors.errors().len(), 1, "{document}: {errors}");
        let error = &errors.errors()[0];
        assert_eq!(error.error.line, document.lines().count() as u32, "{error}");
        assert!(
            error
                .to_string()
                .contains("`version` is given once, the first node of"),
            "{document}: {error}"
        );
    }
}
