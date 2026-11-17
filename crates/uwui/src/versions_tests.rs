//! Each version of each format of uwui's documents read from its fixture, with its node `version`
//! and without it, and a later version refused
//! (`docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md`).
//!
//! The fixtures, in `tests/versions`, are never changed once their version is out: a later reader
//! reads them, migrated.

use crate::DocumentErrors;
use crate::dock::{self, ReadError, Reading};
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

/// What `sheet` over `theme` give: each token's value, in the order declared, the tokens that the
/// theme lacks, then each rule's selector and declarations, its numbers rounded as the
/// repository's fixture of documents rounds them.
fn styled(sheet: &str, theme: &str) -> Result<String, DocumentErrors> {
    let declared: Vec<_> = theme::ROLES.each().chain(FIXTURE_TOKENS.each()).collect();
    let mut tokens = TokenValues::default();
    let styling = theme::read(
        &[StyleSheet::from_text("1.style.kdl", sheet)],
        &[Theme::from_text("1.theme.kdl", theme)],
        &declared,
        &mut tokens,
    )?;
    let mut out = String::new();
    for token in &declared {
        out.push_str(&format!("token {} {:?}\n", token.name, tokens.value(token)));
    }
    out.push_str(&format!("lacking {:?}\n", styling.lacking));
    for rule in &styling.rules {
        out.push_str(&format!("rule {:?}\n", rule.selector));
        for declaration in &rule.declarations {
            out.push_str(&format!("  {declaration:?}\n"));
        }
    }
    Ok(theme::documents_tests::rounded(&out))
}

/// The shortcuts that `table` binds, one a line.
fn bound(table: &str) -> Result<String, DocumentErrors> {
    shortcuts::bindings(&[Shortcuts::from_text("1.keys.kdl", table)]).map(|bindings| {
        bindings
            .iter()
            .map(|binding| format!("binding {binding:?}\n"))
            .collect()
    })
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
fn version_1_s_fixtures_read_as_recorded() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/versions/1.expected");
    let styled = styled(SHEET_1, THEME_1).unwrap_or_else(|errors| panic!("{errors}"));
    let bound = bound(KEYS_1).unwrap_or_else(|errors| panic!("{errors}"));
    let lines = format!("# 1.style.kdl over 1.theme.kdl\n{styled}# 1.keys.kdl\n{bound}");
    if std::env::var_os("SLIME_UPDATE_REFERENCE_IMAGES").is_some() {
        std::fs::write(&path, &lines).expect("the record is written");
        return;
    }
    let recorded = std::fs::read_to_string(&path).expect("the record is read");
    for (number, (line, want)) in lines.lines().zip(recorded.lines()).enumerate() {
        assert_eq!(line, want, "line {} of {}", number + 1, path.display());
    }
    assert_eq!(
        lines.lines().count(),
        recorded.lines().count(),
        "{}",
        path.display()
    );
    // Every token is given by the theme, and every rule and shortcut read.
    let lacking = styled
        .lines()
        .find(|line| line.starts_with("lacking"))
        .expect("its line");
    for token in ["\"gap\"", "\"slow\"", "\"blend\""] {
        assert!(
            !lacking.contains(token),
            "the theme gives {token}: {lacking}"
        );
    }
    assert_eq!(styled.matches("rule ").count(), 5, "{styled}");
    assert_eq!(bound.lines().count(), 6, "{bound}");
}

#[test]
fn a_theme_and_a_sheet_of_version_1_read_the_same_with_their_version_and_without() {
    let marked = styled(SHEET_1, THEME_1).unwrap_or_else(|errors| panic!("{errors}"));
    for (sheet, theme) in [
        (unmarked(SHEET_1), THEME_1.to_owned()),
        (SHEET_1.to_owned(), unmarked(THEME_1)),
        (unmarked(SHEET_1), unmarked(THEME_1)),
        (SHEET_1.replace('\n', "\r\n"), THEME_1.replace('\n', "\r\n")),
    ] {
        let read = styled(&sheet, &theme).unwrap_or_else(|errors| panic!("{errors}"));
        assert_eq!(read, marked);
    }
}

#[test]
fn a_table_of_version_1_reads_the_same_with_its_version_and_without() {
    let marked = bound(KEYS_1).unwrap_or_else(|errors| panic!("{errors}"));
    for table in [unmarked(KEYS_1), KEYS_1.replace('\n', "\r\n")] {
        let read = bound(&table).unwrap_or_else(|errors| panic!("{errors}"));
        assert_eq!(read, marked);
    }
}

#[test]
fn a_document_of_its_version_alone_reads_as_empty() {
    assert_eq!(bound("version 1\n").expect("a table"), "");
    let empty = styled("version 1\n", "version 1\n").expect("a sheet and a theme");
    assert!(!empty.contains("rule "), "{empty}");
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
            bound as fn(&str) -> Result<String, DocumentErrors>,
        ),
        ("scope global { F2 rename; }\nversion 1\n", bound),
        (
            "panel { select { classes panel; }; }\nversion { select kind=Label; }\n",
            |sheet| styled(sheet, THEME_1),
        ),
        ("color { a \"#000000\"; }\nversion 1\n", |theme| {
            styled("", theme)
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

#[test]
fn a_later_version_is_told_whatever_else_its_node_holds() {
    for node in [
        "version 2 3",
        "version 2 minor=1",
        "version 2 { minor 1; }",
        "(v)version 2",
        "version (v)2",
    ] {
        refused_as_later(
            bound(&format!("{node}\nscope global {{ F2 rename; }}\n")),
            "the table of shortcuts",
            1,
        );
        refused_as_later(styled(&format!("{node}\n"), THEME_1), "the style sheet", 1);
        refused_as_later(styled("", &format!("{node}\n")), "the theme", 1);
        assert!(
            matches!(
                dock::read(
                    "layout",
                    &format!("{node}\nTabs chosen=0 {{ panels a }}\n"),
                    Reading::Strict
                ),
                Err(ReadError::Later { version: 2 })
            ),
            "{node}"
        );
    }
}

#[test]
fn a_later_document_in_a_stack_is_refused_the_others_read() {
    let later = Shortcuts::from_text("later.keys.kdl", "version 2\nscope global { F2 later; }\n");
    let known = Shortcuts::from_text("known.keys.kdl", "scope global { Hyper+Z undo; }\n");
    let errors = shortcuts::bindings(&[later, known]).expect_err("two tables refused");
    let found: Vec<_> = errors
        .errors()
        .iter()
        .map(|error| (error.document.as_str(), error.later))
        .collect();
    assert_eq!(
        found,
        [("later.keys.kdl", Some(2)), ("known.keys.kdl", None)]
    );
    let mut tokens = TokenValues::default();
    let Err(errors) = theme::read(
        &[
            StyleSheet::from_text("later.style.kdl", "version 2\n"),
            StyleSheet::from_text("known.style.kdl", "panel { paint background=$none; }\n"),
        ],
        &[],
        &[],
        &mut tokens,
    ) else {
        panic!("two sheets refused");
    };
    let found: Vec<_> = errors
        .errors()
        .iter()
        .map(|error| (error.document.as_str(), error.later))
        .collect();
    assert_eq!(
        found,
        [("later.style.kdl", Some(2)), ("known.style.kdl", None)]
    );
}

#[test]
fn a_later_version_is_kept_past_the_most_errors() {
    let wrong = (0..DocumentErrors::MOST)
        .map(|number| format!("not-a-scope-{number}\n"))
        .collect::<String>();
    let errors = shortcuts::bindings(&[
        Shortcuts::from_text("wrong.keys.kdl", wrong),
        Shortcuts::from_text("later.keys.kdl", "version 2\n"),
        Shortcuts::from_text("also-later.keys.kdl", "version 3\n"),
    ])
    .expect_err("refused");
    let later: Vec<_> = errors
        .errors()
        .iter()
        .filter_map(|error| error.later)
        .collect();
    assert_eq!(later, [2, 3]);
    assert_eq!(errors.errors().len(), DocumentErrors::MOST + 2);
}

#[test]
fn no_rule_of_a_style_sheet_is_named_version() {
    for (sheet, line) in [
        ("version { select kind=Label; }\n", 1),
        (
            "panel { select { classes panel; }; }\nversion { select kind=Label; }\n",
            2,
        ),
    ] {
        let errors = styled(sheet, THEME_1).expect_err(sheet);
        assert_eq!(errors.errors().len(), 1, "{sheet}: {errors}");
        let error = &errors.errors()[0];
        assert_eq!(error.error.line, line, "{error}");
        assert!(
            error.to_string().ends_with("; no rule is named `version`"),
            "{sheet}: {error}"
        );
    }
}
