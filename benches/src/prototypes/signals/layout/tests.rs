//! The layouts' contract on Slatty's library, frozen before the code: the library written as
//! layouts shows what the library written in code does, and a layout's change shows at the next
//! frame with the state kept, the elements matched by their names.

use super::*;
use crate::prototypes::host::{Find, found};
use crate::prototypes::reload::{kept, played, settled};
use crate::prototypes::signals::{LAID_LIBRARY, LIBRARY};

/// The library's file as the bench ships it.
const FILE: &str = include_str!("../../layouts/library.kdl");

/// The file with a label added before the grid, which shifts the grid's place among its siblings.
fn with_subtitle() -> String {
    let grid = "            grid covers class=covers";
    assert_eq!(FILE.matches(grid).count(), 1);
    FILE.replace(
        grid,
        &format!("            label subtitle text=\"Your games\" class=subtitle\n{grid}"),
    )
}

#[test]
fn the_file_reads() {
    let layouts = read(FILE).expect("the library's file reads");
    for name in ["library", "cover", "game-page"] {
        assert!(layouts.get(name).is_some(), "the layout `{name}`");
    }
}

#[test]
fn the_laid_library_shows_what_the_coded_one_does() {
    let coded = played(&LIBRARY);
    let laid = played(&LAID_LIBRARY);
    assert_eq!(kept(&laid), kept(&coded));
    assert!(kept(&laid).page, "played to a game's page");
}

#[test]
fn a_layout_change_shows_with_the_state_kept() {
    let mut host = played(&LAID_LIBRARY);
    settled(&mut host);
    let before = kept(&host);
    assert!(found(&host.ui, host.root, Find::Class("subtitle")).is_none());
    host.program
        .relayout(&with_subtitle())
        .expect("the changed file reads");
    settled(&mut host);
    let subtitle = found(&host.ui, host.root, Find::Class("subtitle")).expect("the label added");
    assert!(matches!(
        &host.ui.element(subtitle).expect("in the tree").kind,
        uwui::tree::Kind::Label(text) if text == "Your games"
    ));
    // The grid moved one place among its siblings, and keeps its scroll and its selection by its
    // name; the field its text, the page its game.
    assert_eq!(kept(&host), before);
}

#[test]
fn a_change_back_shows_the_first_file_again() {
    let mut host = played(&LAID_LIBRARY);
    settled(&mut host);
    let before = kept(&host);
    host.program.relayout(&with_subtitle()).expect("it reads");
    settled(&mut host);
    host.program.relayout(FILE).expect("it reads");
    settled(&mut host);
    assert!(found(&host.ui, host.root, Find::Class("subtitle")).is_none());
    assert_eq!(kept(&host), before);
}

#[test]
fn a_file_that_does_not_read_keeps_the_last_layout() {
    let mut host = played(&LAID_LIBRARY);
    settled(&mut host);
    let before = kept(&host);
    let error = host
        .program
        .relayout("layout library {\n    stack {\n")
        .expect_err("an unclosed node does not read");
    assert!(error.contains("line"), "the error names its line: {error}");
    settled(&mut host);
    assert_eq!(kept(&host), before);
    host.program.relayout(&with_subtitle()).expect("it reads");
    settled(&mut host);
    assert!(found(&host.ui, host.root, Find::Class("subtitle")).is_some());
}

#[test]
fn a_bound_name_that_the_file_lost_waits_with_a_note() {
    let mut host = played(&LAID_LIBRARY);
    settled(&mut host);
    let without_title = FILE.replace("        label title class=title\n", "");
    assert_ne!(without_title, FILE);
    host.program
        .relayout(&without_title)
        .expect("a file without a bound name reads");
    settled(&mut host);
    assert!(kept(&host).page, "the page still shows");
    assert!(found(&host.ui, host.root, Find::Class("title")).is_none());
    let notes = host.program.notes();
    assert!(
        notes.iter().any(|note| note.contains("title")),
        "a note names the binding left unused: {notes:?}"
    );
}

#[test]
fn two_elements_of_one_name_in_a_layout_do_not_read() {
    let twice = FILE.replace(
        "            grid covers class=covers",
        "            grid covers class=covers\n            label covers",
    );
    let error = read(&twice).err().expect("a name twice in one layout");
    assert!(error.contains("covers"), "{error}");
    assert!(error.contains("line"), "{error}");
}

#[test]
fn an_unknown_kind_does_not_read() {
    let error = read("layout a {\n    carousel b\n}\n")
        .err()
        .expect("no such kind");
    assert!(error.contains("carousel"), "{error}");
    assert!(error.contains("line 2"), "{error}");
}

#[test]
fn a_layout_nested_too_deep_does_not_read() {
    let depth = 300;
    let mut text = String::from("layout deep {\n");
    for _ in 0..depth {
        text.push_str("column {\n");
    }
    for _ in 0..depth {
        text.push_str("}\n");
    }
    text.push_str("}\n");
    let error = read(&text).err().expect("past the limit of depth");
    assert!(error.contains("deep"), "{error}");
}
