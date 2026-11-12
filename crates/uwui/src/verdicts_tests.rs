//! Fixtures of verdicts: random documents from a fixed seed, each kept with what the reflected
//! readers that uwui's readers of KDL replaced read from it, recorded before reflection left uwui
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`). The readers of KDL
//! must read each case's documents to the lines recorded.
//!
//! A fixture is a list of cases, each a line `=== case <number>`, then each document under a line
//! `--- <label>`, its lines as they are, then the verdict under a line `+++ read`: what was read, as
//! lines, or `refused`. No document holds a line that starts with one of these marks, a carriage
//! return, or a last line without its line feed: the recording refuses them.

use std::fmt::Write as _;
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

/// `cases` written as a fixture.
///
/// # Panics
///
/// If a document holds a line that a mark starts, a carriage return, or ends without a line feed:
/// the fixture could not give it back as it is.
pub(crate) fn write(cases: &[Case]) -> String {
    let mut out = String::new();
    for (number, case) in cases.iter().enumerate() {
        writeln!(out, "{CASE}{number}").expect("written");
        for (label, text) in &case.inputs {
            assert!(
                !text.contains('\r') && (text.is_empty() || text.ends_with('\n')),
                "case {number}: `{label}` holds a carriage return, or lacks its last line feed"
            );
            assert!(
                text.lines().all(|line| !is_mark(line)),
                "case {number}: `{label}` holds a line that a mark starts"
            );
            writeln!(out, "{INPUT}{label}").expect("written");
            out.push_str(text);
        }
        writeln!(out, "{VERDICT}").expect("written");
        out.push_str(&case.verdict);
        if !case.verdict.is_empty() && !case.verdict.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

fn is_mark(line: &str) -> bool {
    line.starts_with(CASE) || line.starts_with(INPUT) || line.starts_with(VERDICT)
}

/// The cases of the fixture at `path`.
///
/// # Panics
///
/// If it cannot be read, or is not a fixture.
pub(crate) fn read(path: &Path) -> Vec<Case> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is read: {error}", path.display()));
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
fn a_fixture_reads_back_the_cases_written() {
    let cases = vec![
        Case {
            inputs: vec![
                (
                    "a.theme".to_owned(),
                    "color {\n  a \"#fff\"\n}\n".to_owned(),
                ),
                ("empty".to_owned(), String::new()),
            ],
            verdict: "token a 1\n".to_owned(),
        },
        Case {
            inputs: vec![("b".to_owned(), "x\n".to_owned())],
            verdict: "refused\n".to_owned(),
        },
    ];
    let path = std::env::temp_dir().join(format!("uwui-verdicts-{}.txt", std::process::id()));
    std::fs::write(&path, write(&cases)).expect("written");
    let read = read(&path);
    std::fs::remove_file(&path).expect("removed");
    assert_eq!(read.len(), 2);
    for (read, case) in read.iter().zip(&cases) {
        assert_eq!(read.inputs, case.inputs);
        assert_eq!(read.verdict, case.verdict);
    }
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
