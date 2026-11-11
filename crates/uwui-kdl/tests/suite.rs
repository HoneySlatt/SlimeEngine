//! KDL's official tests, kept in `tests/kdl-suite` (its `README.md` says from where, and under which
//! licence):
//! each valid input reads and is written in its expected normal form, each input meant to fail is
//! refused, and a valid input's normal form reads again to the same tree.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

/// The suite's folder.
fn suite() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/kdl-suite")
}

/// Every input of the suite, by name, sorted, with its text, and its expected normal form, none
/// for an input meant to fail.
fn cases() -> Vec<(String, Vec<u8>, Option<String>)> {
    let mut cases: Vec<_> = fs::read_dir(suite().join("input"))
        .expect("the suite's inputs")
        .map(|entry| {
            let path = entry.expect("an input").path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a name of UTF-8")
                .to_owned();
            let input = fs::read(&path).expect("an input read");
            let expected = (!name.ends_with("_fail.kdl")).then(|| {
                fs::read_to_string(suite().join("expected_kdl").join(&name))
                    .unwrap_or_else(|error| panic!("{name}'s expected form: {error}"))
            });
            (name, input, expected)
        })
        .collect();
    cases.sort();
    cases
}

#[test]
fn the_suite_is_whole() {
    let cases = cases();
    let failing = cases.iter().filter(|(_, _, expected)| expected.is_none());
    assert_eq!(cases.len(), 338, "the suite's inputs");
    assert_eq!(failing.count(), 95, "the inputs meant to fail");
}

#[test]
fn every_valid_input_reads_to_its_normal_form_and_every_other_is_refused() {
    let mut wrong = Vec::new();
    for (name, input, expected) in cases() {
        // An input that is not UTF-8 is not KDL, and is meant to fail.
        let read = std::str::from_utf8(&input)
            .map_err(|error| error.to_string())
            .and_then(|text| {
                uwui_kdl::read(text)
                    .map(|document| document.normal_form())
                    .map_err(|error| error.to_string())
            });
        match (read, expected) {
            (Ok(written), Some(expected)) if written == expected => {}
            (Ok(written), Some(expected)) => {
                wrong.push(format!("{name}: wrote\n{written}\nexpected\n{expected}"));
            }
            (Err(error), Some(_)) => wrong.push(format!("{name}: refused: {error}")),
            (Ok(written), None) => wrong.push(format!("{name}: read, meant to fail:\n{written}")),
            (Err(_), None) => {}
        }
    }
    assert!(
        wrong.is_empty(),
        "{} cases of the suite are wrong:\n\n{}",
        wrong.len(),
        wrong.join("\n\n")
    );
}

#[test]
fn a_valid_input_s_normal_form_reads_again_to_its_tree() {
    for (name, input, expected) in cases() {
        if expected.is_none() {
            continue;
        }
        let text = std::str::from_utf8(&input).expect("a valid input is UTF-8");
        let Ok(document) = uwui_kdl::read(text) else {
            continue;
        };
        let written = document.normal_form();
        let again = uwui_kdl::read(&written)
            .unwrap_or_else(|error| panic!("{name}'s normal form does not read: {error}"));
        assert_eq!(
            common::tree(&again),
            common::tree(&document),
            "{name}'s tree read again"
        );
        assert_eq!(
            again.normal_form(),
            written,
            "{name}'s normal form read again"
        );
    }
}
