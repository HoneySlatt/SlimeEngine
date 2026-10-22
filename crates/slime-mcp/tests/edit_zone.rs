//! An agent's session that edits *Hunna Wood*'s zone through the engine's MCP server, with the
//! game's types (`docs/adr/0135-the-editors-program.md`): it starts `hunnawood-editor` on a copy of
//! the zone, changes a value of one of the game's components, the campfire's flicker, creates an
//! entity that holds another of them, a collider, undoes and redoes, writes the zone back and
//! checks what the file holds.
//!
//! It needs `hunnawood-editor` built (`cargo build -p hunnawood-editor`) beside the server and the
//! zone cooked in `target/cooked/hunnawood`. It needs no compositor: the editor has no window.
//! `scripts/check.sh` runs it with the tests that read the zone; it is not in the default run of
//! `cargo test` (`test = false` in Cargo.toml), and has no harness, so that its failures print
//! once.

#[path = "common/session.rs"]
#[expect(dead_code, reason = "this session reads no image")]
mod session;

use std::fs;
use std::path::Path;

use serde_json::json;

use session::{Session, server};

fn main() {
    let server = server();
    let target = server.parent().expect("the target directory");
    let editor = target.join(format!("hunnawood-editor{}", std::env::consts::EXE_SUFFIX));
    assert!(
        editor.is_file(),
        "{} is not built: `cargo build -p hunnawood-editor` first",
        editor.display()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let zone = workspace.join("games/hunnawood/assets/zone");
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-edit-zone");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a directory of the test's own");
    for name in ["zone.scene", "zone.scene.meta"] {
        fs::copy(zone.join(name), directory.join(name)).expect("the zone copied");
    }
    let level = directory.join("zone.scene");
    let command = json!([
        editor.to_str().expect("a path of text"),
        "--level",
        level.to_str().expect("a path of text"),
        "--assets",
        workspace
            .join("target/cooked/hunnawood")
            .to_str()
            .expect("a path of text"),
        "--journal",
        directory
            .join("zone.journal")
            .to_str()
            .expect("a path of text"),
    ]);

    let (mut session, mut child, _) = Session::start(&workspace, "edit-zone");
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("hunnawood-editor runs"), "{said:?}");
    let campfire = "zone/land/campfire light";
    let (said, _) = session.tool("inspect_entity", json!({ "entity": campfire }));
    assert!(
        said[0].contains("[hunnawood::Flickers]\nintensity = 120.0"),
        "{said:?}"
    );
    session.tool(
        "edit",
        json!({
            "label": "Dim the campfire, and put a stone by it",
            "commands": [
                format!(
                    "[SetProperty]\nentity = \"{campfire}\"\ncomponent = \"hunnawood::Flickers\"\n\
                     path = \"intensity\"\nvalue = 90.0\n"
                ),
                "[Create]\nentity = \"zone/stone-0rdx\"\nparent = \"zone\"\n\n[Create.components]\n\
                 \"hunnawood::Collider\" = { kind = \"Box\", height = 1.0 }\n",
            ],
        }),
    );
    session.tool("undo", json!({}));
    let (said, _) = session.tool("inspect_entity", json!({ "entity": campfire }));
    assert!(said[0].contains("intensity = 120.0"), "undone: {said:?}");
    session.tool("redo", json!({}));
    let (said, _) = session.tool("inspect_entity", json!({ "entity": "zone/stone-0rdx" }));
    assert!(said[0].contains("[hunnawood::Collider]"), "{said:?}");
    let (said, _) = session.tool("write_level", json!({}));
    assert_eq!(said, ["the level is written, whole"]);
    let written = fs::read_to_string(&level).expect("written");
    assert!(
        written.contains("\"hunnawood::Flickers\" = { intensity = 90.0 }")
            && written.contains("stone-0rdx")
            && written.contains("\"hunnawood::Collider\""),
        "{written}"
    );
    session.tool("stop_game", json!({}));

    drop(session);
    let status = child.wait().expect("the server ends");
    assert!(status.success(), "the server ended with {status}");
    let _ = fs::remove_dir_all(&directory);
}
