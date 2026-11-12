//! Random layouts, written in KDL, read to the layouts that the reflected reader of layouts in TOML
//! read, bit for bit: its verdicts on a fixed seed, recorded before reflection left uwui
//! (`crate::verdicts_tests`, `tests/reference/verdicts/layouts.txt`). The layouts were held as a
//! program may hold them before it normalizes them, and written by [`write`], which must write
//! each read back to its text; `tests/dock.rs` writes and reads back random layouts at each run,
//! deeper ones too.

use super::{Layout, Reading, read, write};
use crate::verdicts_tests;

/// The fixture's name.
pub(super) const FIXTURE: &str = "layouts";

/// `layout` as a verdict's lines, which tell apart each float's bits, but a value that is not a
/// number's: `Debug` writes the shortest text that reads back to each finite float, `-0.0` apart
/// from `0.0`, and every value that is not a number as `NaN`.
pub(super) fn verdict(layout: &Layout) -> String {
    format!("{layout:?}\n")
}

#[test]
fn random_layouts_read_as_the_fixture_records() {
    let path = verdicts_tests::path(FIXTURE);
    let cases = verdicts_tests::read(&path);
    assert!(cases.len() >= 60, "{} cases", cases.len());
    for (number, case) in cases.iter().enumerate() {
        let [(_, text)] = case.inputs.as_slice() else {
            panic!("case {number}: one layout");
        };
        let read = read("random.layout.kdl", text, Reading::Strict)
            .unwrap_or_else(|error| panic!("case {number}: {error}"));
        assert!(read.warnings.is_none(), "case {number}");
        verdicts_tests::check(&path, number, &verdict(&read.layout), &case.verdict);
        assert_eq!(
            write(&read.layout, None).expect("written"),
            *text,
            "case {number}: written back"
        );
    }
}
