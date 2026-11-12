//! Fixtures of verdicts: random documents from a fixed seed, each kept with what the reflected
//! readers that uwui's readers of KDL replaced read from it, recorded before reflection left uwui
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`). The readers of KDL
//! must read each case's documents to the lines recorded.
//!
//! A fixture is a list of cases, each a line `=== case <number>`, then each document under a line
//! `--- <label>`, its lines as they are, then the verdict under a line `+++ read`: what was read, as
//! lines, or `refused`. No document holds a line that starts with one of these marks, a carriage
//! return, or a last line without its line feed: the recording refused them.

use std::path::{Path, PathBuf};

/// A document's label and its text.
pub(crate) type Input = (String, String);

/// A case of a fixture: its documents, and the lines that reading them gave.
#[derive(Debug)]
pub(crate) struct Case {
    pub(crate) inputs: Vec<Input>,
    pub(crate) verdict: String,
}

/// The marks that start a case, a document and a verdict.
const CASE: &str = "=== case ";
const INPUT: &str = "--- ";
const VERDICT: &str = "+++ read";

/// The path of the fixture named `name`, `tests/reference/verdicts/<name>.txt`.
pub(crate) fn path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/reference/verdicts")
        .join(format!("{name}.txt"))
}

/// The cases of the fixture at `path`.
///
/// # Panics
///
/// If it cannot be read, or is not a fixture.
pub(crate) fn read(path: &Path) -> Vec<Case> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is read: {error}", path.display()));
    cases(path, &text)
}

/// The cases of `text`, the fixture at `path`.
fn cases(path: &Path, text: &str) -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    // Where the lines go: a document of the last case, or its verdict.
    let mut in_verdict = false;
    // `lines` drops a carriage return that a checkout on Windows may add before each line feed.
    for line in text.lines() {
        if line.starts_with(CASE) {
            cases.push(Case {
                inputs: Vec::new(),
                verdict: String::new(),
            });
            in_verdict = false;
            continue;
        }
        let case = cases
            .last_mut()
            .unwrap_or_else(|| panic!("{} starts with a case", path.display()));
        if let Some(label) = line.strip_prefix(INPUT) {
            case.inputs.push((label.to_owned(), String::new()));
        } else if line == VERDICT {
            in_verdict = true;
        } else if in_verdict {
            case.verdict.push_str(line);
            case.verdict.push('\n');
        } else {
            let (_, text) = case
                .inputs
                .last_mut()
                .unwrap_or_else(|| panic!("{}: a line outside a document", path.display()));
            text.push_str(line);
            text.push('\n');
        }
    }
    cases
}

/// Checks that `verdict`, read again from the documents of the `number`th case of the fixture at
/// `path`, gives the lines that it records, `want`: every line the same, but for the numbers of a
/// color, `LinearColor { r: …, g: …, b: …, a: … }`, which may differ by 1e-5 of their size, as
/// colors are computed through the platform's own mathematical functions, `powf`, `cbrt` and
/// `atan2`, whose last bits differ between Linux and Windows.
///
/// # Panics
///
/// If a line differs.
pub(crate) fn check(path: &Path, number: usize, verdict: &str, want: &str) {
    let context = || format!("case {number} of {}", path.display());
    for (line, (got, wanted)) in verdict.lines().zip(want.lines()).enumerate() {
        assert!(
            same_line(got, wanted),
            "{}, line {} of its verdict:\n  read     {got}\n  recorded {wanted}",
            context(),
            line + 1
        );
    }
    assert_eq!(
        verdict.lines().count(),
        want.lines().count(),
        "{}: the lines of its verdict, read\n{verdict}",
        context()
    );
}

/// Whether two lines of a verdict are the same, a color's numbers within 1e-5 of their size.
fn same_line(got: &str, want: &str) -> bool {
    if got == want {
        return true;
    }
    let (got, want) = (colors_apart(got), colors_apart(want));
    got.len() == want.len()
        && got.iter().zip(&want).all(|(got, want)| match (got, want) {
            (Piece::Text(got), Piece::Text(want)) => got == want,
            (Piece::Color(got), Piece::Color(want)) => got.iter().zip(want).all(|(&got, &want)| {
                got == want
                    || (got.is_nan() && want.is_nan())
                    || (got - want).abs() <= 1e-5 * want.abs().max(1.0)
            }),
            _ => false,
        })
}

/// A part of a line of a verdict: text, or a color's four numbers.
#[derive(Debug)]
enum Piece<'a> {
    Text(&'a str),
    Color([f64; 4]),
}

/// `line` cut into its colors, as `Debug` writes them, and the text between them. A color whose
/// numbers do not read is kept as text, to be compared as it is.
fn colors_apart(line: &str) -> Vec<Piece<'_>> {
    const START: &str = "LinearColor { r: ";
    let mut pieces = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find(START) {
        let Some(length) = rest[at..].find('}').map(|end| end + 1) else {
            break;
        };
        let color = &rest[at..at + length];
        let numbers: Vec<f64> = color[START.len() - 3..color.len() - 1]
            .split(',')
            .filter_map(|field| field.split(':').nth(1)?.trim().parse().ok())
            .collect();
        pieces.push(Piece::Text(&rest[..at]));
        match <[f64; 4]>::try_from(numbers) {
            Ok(numbers) => pieces.push(Piece::Color(numbers)),
            Err(_) => pieces.push(Piece::Text(color)),
        }
        rest = &rest[at + length..];
    }
    pieces.push(Piece::Text(rest));
    pieces
}

#[test]
fn a_fixture_gives_each_case_its_documents_and_its_verdict() {
    // As a checkout on Windows may write it, a carriage return before each line feed.
    let text = "=== case 0\r\n--- a.theme\r\ncolor {\r\n  a \"#fff\"\r\n}\r\n--- empty\r\n\
                +++ read\r\ntoken a 1\r\n=== case 1\r\n--- b\r\nx\r\n+++ read\r\nrefused\r\n";
    let cases = cases(Path::new("test.txt"), text);
    assert_eq!(cases.len(), 2);
    assert_eq!(
        cases[0].inputs,
        [
            (
                "a.theme".to_owned(),
                "color {\n  a \"#fff\"\n}\n".to_owned()
            ),
            ("empty".to_owned(), String::new()),
        ]
    );
    assert_eq!(cases[0].verdict, "token a 1\n");
    assert_eq!(cases[1].inputs, [("b".to_owned(), "x\n".to_owned())]);
    assert_eq!(cases[1].verdict, "refused\n");
}

#[test]
fn a_color_s_numbers_may_differ_by_a_little_and_nothing_else_may() {
    let want = "Paint(Background(LinearColor { r: 0.5, g: 0.25, b: 1.0, a: 1.0 })) 12.5";
    assert!(same_line(
        "Paint(Background(LinearColor { r: 0.50000006, g: 0.25, b: 1.0, a: 1.0 })) 12.5",
        want
    ));
    // A length outside a color is compared as it is written.
    assert!(!same_line(
        "Paint(Background(LinearColor { r: 0.5, g: 0.25, b: 1.0, a: 1.0 })) 12.500001",
        want
    ));
    assert!(!same_line(
        "Paint(Background(LinearColor { r: 0.51, g: 0.25, b: 1.0, a: 1.0 })) 12.5",
        want
    ));
    assert!(!same_line(
        "Paint(Border(LinearColor { r: 0.5, g: 0.25, b: 1.0, a: 1.0 })) 12.5",
        want
    ));
    // A value that is not a number is itself alone.
    assert!(same_line(
        "LinearColor { r: NaN, g: 0.25, b: 1.0, a: 1.0 } 1",
        "LinearColor { r: NaN, g: 0.25, b: 1.0, a: 1.00000001 } 1"
    ));
    assert!(!same_line(
        "LinearColor { r: NaN, g: 0.25, b: 1.0, a: 1.0 }",
        "LinearColor { r: 0.5, g: 0.25, b: 1.0, a: 1.0 }"
    ));
}
