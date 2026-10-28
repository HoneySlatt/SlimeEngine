//! An agent's session with the editor's window through the engine's MCP server
//! (`docs/adr/0156-the-editors-window-and-its-operations.md`): it starts the `editor` example on a
//! copy of the yard, in a window, lists the operations, makes a step and undoes it by the
//! operation's name, opens the palette by its name and finds it in the window, opens the File menu
//! as a person does, runs `file.save` by name, captures the viewport by its name, which the menu's
//! frames do not draw again and framing a lamp does (`docs/adr/0158-the-viewport.md`), opens
//! another copy of the yard through *Open a
//! level…*, which opens no dialog of the system in a program that a tool started, its path typed
//! (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`), and checks that the editor,
//! left alone, wakes for the end of the first run's hint and runs no frame for ten seconds after
//! it, then quits it by name.
//!
//! It needs a Wayland compositor, the example built (`cargo build --example editor`) and the
//! examples' assets cooked, in the directory that its first argument names, or
//! `target/cooked/examples`: `scripts/check.sh` runs it in a headless Weston with lavapipe, as it
//! runs the `interface` test. It is not in the default run of `cargo test` (`test = false` in
//! Cargo.toml), and has no harness, so that its failures print once.

#[path = "common/interface.rs"]
mod interface;
#[path = "common/session.rs"]
mod session;

use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::json;

use interface::{act, path_of, until};
use session::{Session, image, server};

/// How many times the viewport drew its image, as its report says.
fn draws(session: &mut Session) -> u64 {
    let (said, _) = session.tool("read_report", json!({ "name": "viewport" }));
    said[0]
        .split("draws = ")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("no draws in {said:?}"))
}

/// The frame that `game_state` says the editor is at.
fn frame(session: &mut Session) -> u64 {
    let (said, _) = session.tool("game_state", json!({}));
    said[0]
        .strip_prefix("frame ")
        .and_then(|rest| rest.split(',').next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("no frame in {said:?}"))
}

fn main() {
    let server = server();
    let target = server.parent().expect("the target directory");
    let example = target.join(format!("examples/editor{}", std::env::consts::EXE_SUFFIX));
    assert!(
        example.is_file(),
        "{} is not built: `cargo build --example editor` first",
        example.display()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = std::env::args()
        .nth(1)
        .map_or_else(|| workspace.join("target/cooked/examples"), Into::into);
    // A copy of the yard, and a journal, in the build's own directory of temporary files.
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-edit-window");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a directory of the test's own");
    // Two copies of the yard: the first opened, the second through Open a level….
    let original = workspace.join("examples/assets/yard.scene");
    let level = directory.join("yard.scene");
    let other = directory.join("other yard.scene");
    for copy in [&level, &other] {
        fs::copy(&original, copy).expect("the yard copied");
        fs::copy(
            original.with_extension("scene.meta"),
            copy.with_extension("scene.meta"),
        )
        .expect("its metadata copied");
    }
    let command = json!([
        example.to_str().expect("a path of text"),
        "--level",
        level.to_str().expect("a path of text"),
        "--assets",
        assets.to_str().expect("a path of text"),
        "--journal",
        directory
            .join("yard.journal")
            .to_str()
            .expect("a path of text"),
    ]);

    let (mut session, mut child, _) = Session::start(&workspace, "edit-window");
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("editor runs"), "{said:?}");
    // The first run, which finds no list of recent levels, tells one hint.
    let hint = path_of(&mut session, "", "finds every operation");
    assert!(hint.starts_with("notifications/"), "{hint}");
    // It goes after its time: left alone meanwhile, the editor wakes for its end, at its deadline,
    // and runs a frame then, between the frames of these two requests, which a run that missed its
    // deadline would not.
    let before = frame(&mut session);
    std::thread::sleep(Duration::from_secs(6));
    let after = frame(&mut session);
    assert!(
        after - before >= 2,
        "the editor ran {} frame as the hint ended",
        after - before
    );
    let (said, _) = session.tool("find_elements", json!({}));
    assert!(
        !said.iter().any(|text| text.contains("notifications/")),
        "the hint went: {said:?}"
    );
    let (said, _) = session.tool("list_operations", json!({}));
    assert!(
        said[0].contains("view.palette: Find an operation; changes the window"),
        "{said:?}"
    );
    // A step, then undone by the operation's name, as Ctrl+Z or the Edit menu undoes it.
    session.tool(
        "edit",
        json!({
            "label": "Light the yard",
            "commands": [
                "[Create]\nentity = \"yard/torch-0rdx\"\nparent = \"yard\"\n\n[Create.components]\nName = \"torch\"\n",
            ],
        }),
    );
    let status = path_of(&mut session, "Label", "1 step not written");
    assert!(status.starts_with("editor/window/status/"), "{status}");
    let (said, _) = session.tool("run_operation", json!({ "name": "edit.undo" }));
    assert_eq!(said, ["done"]);
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0].contains("steps undone, the next to redo first: `Light the yard`"),
        "{said:?}"
    );
    // Undone, the level is its file again.
    path_of(&mut session, "Label", "all written");
    // The window's operations run in the window: the palette, found by its field.
    let (said, _) = session.tool("run_operation", json!({ "name": "view.palette" }));
    assert_eq!(said, ["done"]);
    let field = path_of(&mut session, "SearchInput", "command");
    assert!(field.starts_with("palette/"), "{field}");
    session.tool("act_on_interface", json!({ "action": "cancel" }));
    // The File menu, opened as a person opens it, lists its operations with their shortcuts.
    let file = path_of(&mut session, "MenuItem", "File");
    act(&mut session, "press", &file, json!({}));
    let save = path_of(&mut session, "MenuItem", "Save");
    assert!(save.starts_with("menu/"), "{save}");
    act(&mut session, "press", &save, json!({}));
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0].contains("steps that the level's file does not hold: 0;"),
        "the menu's Save wrote the level: {said:?}"
    );
    path_of(&mut session, "Label", "all written");
    // The viewport, captured by its name once the yard's meshes have loaded, which the editor
    // draws again only when what it shows changes: the frames of a menu opened and closed leave
    // it as it is; framing a lamp draws it again, closer.
    let captured = until(
        "the yard loaded in the viewport",
        || {
            let first = image(&session.tool("capture", json!({ "view": "viewport" })).1);
            let second = image(&session.tool("capture", json!({ "view": "viewport" })).1);
            (first == second).then_some(first)
        },
        Option::is_some,
    )
    .expect("found above");
    let (width, height, yard) = captured;
    assert!(width > 400 && height > 300, "{width} × {height}");
    let drawn = draws(&mut session);
    let file = path_of(&mut session, "MenuItem", "File");
    act(&mut session, "press", &file, json!({}));
    path_of(&mut session, "MenuItem", "Save");
    session.tool("act_on_interface", json!({ "action": "cancel" }));
    assert_eq!(
        draws(&mut session),
        drawn,
        "the menu drew the viewport again"
    );
    let (said, _) = session.tool("select", json!({ "selection": ["yard/lamp 1"] }));
    assert_eq!(said, ["done"]);
    let (said, _) = session.tool("run_operation", json!({ "name": "view.frame_selection" }));
    assert_eq!(said, ["done"]);
    let (_, captured) = session.tool("capture", json!({ "view": "viewport" }));
    assert_ne!(
        image(&captured).2,
        yard,
        "framing the lamp changed the view"
    );
    assert!(draws(&mut session) > drawn);
    // Open a level…, from the File menu: a program that a tool started opens no dialog of the
    // system, which the agent could not see, and its layer waits for the path, typed.
    let file = path_of(&mut session, "MenuItem", "File");
    act(&mut session, "press", &file, json!({}));
    let choose = path_of(&mut session, "MenuItem", "Open a level");
    act(&mut session, "press", &choose, json!({}));
    path_of(&mut session, "Label", "Type the level's path");
    let field = path_of(&mut session, "TextInput", "Level's path");
    assert!(field.starts_with("choose/"), "{field}");
    let other = other.to_str().expect("a path of text");
    act(&mut session, "set_value", &field, json!({ "value": other }));
    path_of(&mut session, "Label", other);
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(said[0].contains("other yard.scene"), "{said:?}");
    // Left alone, once the menu's closing transition has ended, the editor runs no frame: each of
    // these two requests wakes one.
    std::thread::sleep(Duration::from_secs(1));
    let before = frame(&mut session);
    std::thread::sleep(Duration::from_secs(10));
    let after = frame(&mut session);
    assert!(
        after - before <= 2,
        "the editor ran {} frames in ten seconds alone",
        after - before
    );
    // Quit by its name: the window closes at once.
    let (said, _) = session.tool("run_operation", json!({ "name": "file.quit" }));
    assert_eq!(said, ["done"]);
    let ended = until(
        "the editor's end",
        || session.try_tool("game_state", json!({})).0,
        |said| {
            said.first()
                .is_some_and(|text| text.contains("the game ended"))
        },
    );
    assert!(ended[0].contains("exit status: 0"), "{ended:?}");

    drop(session);
    let status = child.wait().expect("the server ends");
    assert!(status.success(), "the server ended with {status}");
    let _ = fs::remove_dir_all(&directory);
    println!("test edit_window ... ok");
}
