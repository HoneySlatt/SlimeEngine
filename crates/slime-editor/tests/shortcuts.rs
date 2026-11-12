//! The editor's tables of shortcuts, read, kept as a fixture: each scope's chords and commands, as
//! the sheet of shortcuts lists them (`uwui::tree::Ui::shortcuts`), for the editor's own table and
//! for it with Blender's way laid over it
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
//!
//! The fixture, `tests/reference/shortcuts.txt`, was recorded through the reflected reader of the
//! tables in TOML, before they became KDL, and outlives it: the reader of KDL must read the
//! converted tables to the same lines on Linux and Windows. `SLIME_UPDATE_REFERENCE_IMAGES` writes
//! it again: a change of a table changes it on purpose; a change of the reader must not.

use std::path::Path;

use slime_editor::{BLENDER_SHORTCUTS, SHORTCUTS};
use uwui::shortcuts::Shortcuts;
use uwui::text::Fonts;
use uwui::tree::Ui;

/// The fixture's lines: for each stack of tables, a heading, then each scope and its chords.
fn lines() -> String {
    let mut out = String::new();
    let stacks: [(&str, Vec<Shortcuts>); 2] = [
        (
            "editor.keys",
            vec![Shortcuts::from_text("editor.keys", SHORTCUTS)],
        ),
        (
            "editor.keys, editor-blender.keys",
            vec![
                Shortcuts::from_text("editor.keys", SHORTCUTS),
                Shortcuts::from_text("editor-blender.keys", BLENDER_SHORTCUTS),
            ],
        ),
    ];
    for (heading, tables) in stacks {
        let mut ui = Ui::new(Fonts::new());
        ui.set_shortcuts(&tables)
            .unwrap_or_else(|error| panic!("{heading}: {error}"));
        out.push_str(&format!("# {heading}\n"));
        for (scope, chords) in ui.shortcuts() {
            out.push_str(&format!("[{scope}]\n"));
            for (chord, command) in chords {
                out.push_str(&format!("  {chord} = {command}\n"));
            }
        }
    }
    out
}

#[test]
fn the_editor_s_tables_of_shortcuts_read_as_the_fixture_records() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference/shortcuts.txt");
    let lines = lines();
    if std::env::var_os("SLIME_UPDATE_REFERENCE_IMAGES").is_some() {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("its folder is made");
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
