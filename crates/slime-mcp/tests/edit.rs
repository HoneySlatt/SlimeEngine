//! An agent's session that edits a level through the engine's MCP server
//! (`docs/adr/0134-the-editors-core.md`, `docs/adr/0135-the-editors-program.md`): it starts the
//! `editor` example on a copy of the examples' yard, writes it back unchanged, which gives the file
//! written by hand byte for byte, then makes a step that creates, places, changes and deletes,
//! changes a value as it would in a game, which the editor makes a step of its own, undoes and
//! redoes, reads the history and the journal, and writes the level; then stops the editor and
//! finds again, as it recovers its journal, the step done after the writing and its history; makes
//! a step that it does not write, and finds it again the same way; and, the level written, starts
//! the editor once more, which opens the level from its file with nothing to recover; there, it
//! raises three lamps in one step, which one undo puts back, and drops one on the ground by the
//! operation's name (`docs/adr/0161-the-open-step-and-the-handles.md`), then duplicates it, copies
//! it and pastes it the same way (`docs/adr/0162-creating-copying-and-arranging.md`), places a
//! lamp by its prefab's file and goes back and forth in the history
//! (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`); and plays the yard in the `world`
//! example, which it pauses, steps and stops, reading the game's log in the editor's
//! (`docs/adr/0164-play.md`), and drives with the game's tools while a step made in the editor,
//! which places a lamp, reaches it, until it stops (`docs/adr/0165-editing-while-playing.md`); and
//! moves a lamp into the one above it and out again by the operations of Alt and the arrows
//! (`docs/adr/0168-rearranging-the-hierarchy.md`).
//!
//! It needs the examples built (`cargo build --example editor --example world`) beside the server
//! and the examples' assets cooked, in the directory that its first argument names, or
//! `target/cooked/examples`. It
//! needs no compositor: the editor has no window. `scripts/check.sh` runs it; it is not in the
//! default run of `cargo test` (`test = false` in Cargo.toml), and has no harness, so that its
//! failures print once.

#[path = "common/session.rs"]
#[expect(dead_code, reason = "this session reads no image")]
mod session;

use std::fs;
use std::path::Path;

use serde_json::json;

use session::{Session, server};

/// The lamp prefab of the examples, by its asset ID, which the yard places.
const LAMP: &str = "6647a9c8-d378-4f9a-89d7-3ec9fc2b172a";

fn main() {
    let server = server();
    let target = server.parent().expect("the target directory");
    let example = target.join(format!("examples/editor{}", std::env::consts::EXE_SUFFIX));
    assert!(
        example.is_file(),
        "{} is not built: `cargo build --example editor --example world` first",
        example.display()
    );
    // The game that the editor's Play runs, beside it.
    let game = target.join(format!("examples/world{}", std::env::consts::EXE_SUFFIX));
    assert!(
        game.is_file(),
        "{} is not built: `cargo build --example editor --example world` first",
        game.display()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = std::env::args()
        .nth(1)
        .map_or_else(|| workspace.join("target/cooked/examples"), Into::into);
    // A copy of the yard, and a journal, in the build's own directory of temporary files.
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-edit");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a directory of the test's own");
    let original = workspace.join("examples/assets/yard.scene");
    let level = directory.join("yard.scene");
    fs::copy(&original, &level).expect("the yard copied");
    fs::copy(
        original.with_extension("scene.meta"),
        directory.join("yard.scene.meta"),
    )
    .expect("its metadata copied");
    let journal = directory.join("yard.journal");
    let command = json!([
        example.to_str().expect("a path of text"),
        "--headless",
        "--level",
        level.to_str().expect("a path of text"),
        "--assets",
        assets.to_str().expect("a path of text"),
        "--journal",
        journal.to_str().expect("a path of text"),
    ]);

    let (mut session, mut child, _) = Session::start(&workspace, "edit");
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("editor runs"), "{said:?}");
    // Written back unchanged, the yard is the file written by hand.
    let (said, _) = session.tool("write_level", json!({}));
    assert_eq!(said, ["the level is written, whole"]);
    assert_eq!(
        fs::read(&level).expect("written"),
        fs::read(&original).expect("the original")
    );

    let step = json!({
        "label": "Light the yard",
        "commands": [
            "[Create]\nentity = \"yard/torch-0rdx\"\nparent = \"yard\"\n\n[Create.components]\nName = \"torch\"\n",
            format!("[Place]\nentity = \"yard/lamp-k3f9\"\nparent = \"yard\"\nprefab = \"{LAMP}\"\n"),
            "[SetProperty]\nentity = \"yard/lamp 1\"\ncomponent = \"Transform\"\npath = \"translation.x\"\nvalue = 5.0\n",
            "[Delete]\nentity = \"yard/lamp 4\"\n",
        ],
        "selection": ["yard/torch-0rdx"],
    });
    session.tool("edit", step);
    // A value changed as in a game, which the editor makes a step of its own.
    session.tool(
        "set_property",
        json!({
            "entity": "yard/lamp 2",
            "component": "Transform",
            "path": "translation.x",
            "value": "-5.0",
        }),
    );
    let lamp = |session: &mut Session, name: &str| {
        let (said, _) = session.tool(
            "inspect_entity",
            json!({ "entity": format!("yard/{name}") }),
        );
        said[0].clone()
    };
    assert!(lamp(&mut session, "lamp 2").contains("x = -5.0"));
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0]
            .contains("steps done, the oldest first: `Light the yard`, `SetProperty yard/lamp 2`")
            && said[0].contains("selected: `yard/torch-0rdx`"),
        "{said:?}"
    );
    session.tool("undo", json!({}));
    let said = lamp(&mut session, "lamp 2");
    assert!(
        said.contains("x = -3.5") && !said.contains("-5.0"),
        "undone: {said}"
    );
    // Undone by the operation's name, as a person undoes it from the menu or with Ctrl+Z.
    let (said, _) = session.tool("list_operations", json!({}));
    assert!(
        said[0].contains("edit.undo: Undo; changes the history")
            && said[0].contains("file.open (path): Open a level; changes the file"),
        "{said:?}"
    );
    let (said, _) = session.tool("run_operation", json!({ "name": "edit.undo" }));
    assert_eq!(said, ["done"]);
    let (said, _) = session.tool("list_entities", json!({ "filter": "torch" }));
    assert!(!said[0].contains("yard/torch-0rdx"), "undone: {said:?}");
    // Without a window, the window's operations are refused, and no step enters the history.
    let (said, result) = session.try_tool("run_operation", json!({ "name": "view.palette" }));
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(said[0].contains("needs the editor's window"), "{said:?}");
    let (said, result) = session.try_tool(
        "run_operation",
        json!({ "name": "file.open", "arguments": [] }),
    );
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(said[0].contains("takes 1 arguments"), "{said:?}");
    session.tool("redo", json!({}));
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0].contains("steps done, the oldest first: `Light the yard`;")
            && said[0].contains("steps undone, the next to redo first: `SetProperty yard/lamp 2`"),
        "{said:?}"
    );
    let (said, _) = session.tool("inspect_entity", json!({ "entity": "yard/lamp-k3f9" }));
    assert!(said[0].contains("named lamp-k3f9"), "{said:?}");
    let (said, _) = session.tool("editor_journal", json!({}));
    for entry in [
        "# level\n",
        "# do\n",
        "# command\n",
        "# undo\n",
        "# redo\n",
        "# written\n",
    ] {
        assert!(said[0].contains(entry), "{entry}: {said:?}");
    }
    let (said, _) = session.tool("run_operation", json!({ "name": "file.save" }));
    assert_eq!(said, ["the level is written, whole"]);
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0].contains("steps that the level's file does not hold: 0;"),
        "{said:?}"
    );
    let written = fs::read_to_string(&level).expect("written");
    assert!(
        written.contains("torch-0rdx")
            && written.contains("lamp-k3f9")
            && !written.contains("\"lamp 4\"")
            && written
                .contains("[changed.\"lamp 1\".Transform]\ntranslation = { x = 5.0, z = 3.5 }"),
        "{written}"
    );
    // The value changed, undone before the writing, done again after it; nothing more to redo.
    session.tool("redo", json!({}));
    let (said, result) = session.try_tool("redo", json!({}));
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(said[0].contains("no step is undone"), "{said:?}");
    session.tool("stop_game", json!({}));

    // Started again, the editor recovers its journal: it opens the yard as the journal's first entry
    // holds it, and plays every entry again, the step done again after the writing with them.
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("editor runs"), "{said:?}");
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0].contains("`Light the yard`, `SetProperty yard/lamp 2`")
            && !said[0].contains("recovered at the start: 0;"),
        "{said:?}"
    );
    let (said, _) = session.tool("inspect_entity", json!({ "entity": "yard/torch-0rdx" }));
    assert!(said[0].contains("named torch"), "{said:?}");
    // A step not written, then the editor stopped: started again, it recovers that one too.
    session.tool(
        "edit",
        json!({
            "label": "Put out the torch",
            "commands": ["[Delete]\nentity = \"yard/torch-0rdx\"\n"],
        }),
    );
    session.tool("stop_game", json!({}));
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("editor runs"), "{said:?}");
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(said[0].contains("`Put out the torch`"), "{said:?}");
    let (said, _) = session.tool("list_entities", json!({ "filter": "torch" }));
    assert!(!said[0].contains("yard/torch-0rdx"), "{said:?}");
    // Written, then started again: the editor opens the yard from its file, with nothing to recover.
    let (said, _) = session.tool("write_level", json!({}));
    assert_eq!(said, ["the level is written, whole"]);
    session.tool("stop_game", json!({}));
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("editor runs"), "{said:?}");
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0].contains("steps done, the oldest first: none;")
            && said[0].contains("recovered at the start: 0;"),
        "{said:?}"
    );
    assert!(lamp(&mut session, "lamp 1").contains("x = 5.0"));
    let (said, _) = session.tool("list_entities", json!({ "filter": "torch" }));
    assert!(!said[0].contains("yard/torch-0rdx"), "{said:?}");
    // Three lamps raised in one step, which one undo puts back
    // (`docs/adr/0161-the-open-step-and-the-handles.md`).
    let raised: Vec<String> = ["lamp 1", "lamp 2", "lamp 3"]
        .iter()
        .map(|name| {
            format!(
                "[SetProperty]\nentity = \"yard/{name}\"\ncomponent = \"Transform\"\npath = \"translation.y\"\nvalue = 2.0\n"
            )
        })
        .collect();
    session.tool(
        "edit",
        json!({ "label": "Raise three lamps", "commands": raised, "selection": ["yard/lamp 2"] }),
    );
    for name in ["lamp 1", "lamp 2", "lamp 3"] {
        assert!(lamp(&mut session, name).contains("y = 2.0"), "{name}");
    }
    session.tool("undo", json!({}));
    for name in ["lamp 1", "lamp 2", "lamp 3"] {
        assert!(!lamp(&mut session, name).contains("y = 2.0"), "{name}");
    }
    // Lamp 2 put up over the sample's floor, then dropped on what lies below by the operation's
    // name, as End drops it.
    let over_the_floor: Vec<String> = [("x", 0.5), ("y", 3.0), ("z", 1.5)]
        .iter()
        .map(|(axis, value)| {
            format!(
                "[SetProperty]\nentity = \"yard/lamp 2\"\ncomponent = \"Transform\"\npath = \"translation.{axis}\"\nvalue = {value:?}\n"
            )
        })
        .collect();
    session.tool(
        "edit",
        json!({ "label": "Put a lamp up", "commands": over_the_floor, "selection": ["yard/lamp 2"] }),
    );
    assert!(lamp(&mut session, "lamp 2").contains("y = 3.0"));
    let (said, _) = session.tool("run_operation", json!({ "name": "edit.drop" }));
    assert_eq!(said, ["done"]);
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(said[0].contains("`Drop on what lies below`"), "{said:?}");
    // On the sample's floor, which lies at the ground's height: no more than a few centimetres up.
    let dropped = lamp(&mut session, "lamp 2");
    let height: f64 = dropped
        .split("translation = { x = 0.5, y = ")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .and_then(|y| y.parse().ok())
        .unwrap_or_else(|| panic!("a height: {dropped}"));
    assert!((0.0..0.1).contains(&height), "{dropped}");
    // Lamp 2 duplicated, then copied and pasted, by the operations' names, each a step that
    // selects what it created, a placement of the lamp beside the others.
    for (name, label) in [
        ("edit.duplicate", "Duplicate"),
        ("edit.copy", ""),
        ("edit.paste", "Paste"),
    ] {
        let (said, _) = session.tool("run_operation", json!({ "name": name }));
        assert_eq!(said, ["done"], "{name}");
        if label.is_empty() {
            continue;
        }
        let (said, _) = session.tool("editor_history", json!({}));
        assert!(said[0].contains(&format!("`{label}`")), "{said:?}");
        assert!(said[0].contains("selected: `yard/lamp 2-"), "{said:?}");
    }
    let (said, _) = session.tool("list_entities", json!({ "filter": "lamp 2-" }));
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(
        said[0].matches(" (lamp 2-").count(),
        2,
        "two placements: {said:?}"
    );
    // A lamp placed by its prefab's source file, as the palette of prefabs places it; the history
    // gone one step back, then forth (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`).
    let (said, _) = session.tool(
        "run_operation",
        json!({ "name": "edit.place", "arguments": ["lamp.scene"] }),
    );
    assert_eq!(said, ["done"]);
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0].contains("`Place a prefab`") && said[0].contains("selected: `yard/lamp-"),
        "{said:?}"
    );
    // Placed again by its asset ID, as the palette places it, where the lamp just placed stands;
    // the level itself, and what the cooked data does not hold, refused.
    let (said, _) = session.tool(
        "run_operation",
        json!({ "name": "edit.place", "arguments": [LAMP] }),
    );
    assert_eq!(said, ["done"]);
    for refused in ["yard.scene", "nothing.scene"] {
        let (said, result) = session.try_tool(
            "run_operation",
            json!({ "name": "edit.place", "arguments": [refused] }),
        );
        assert_eq!(result["isError"], json!(true), "{refused}: {said:?}");
        assert!(
            said[0].contains("is not a prefab that the level can place"),
            "{said:?}"
        );
    }
    session.tool("undo", json!({}));
    let (said, _) = session.tool("editor_history", json!({}));
    let done = said[0]
        .split("steps done, the oldest first: ")
        .nth(1)
        .and_then(|rest| rest.split(';').next())
        .map_or(0, |steps| steps.matches('`').count() / 2);
    for (steps, placed) in [(done - 1, false), (done, true)] {
        let (said, _) = session.tool(
            "run_operation",
            json!({ "name": "edit.go_to_step", "arguments": [steps.to_string()] }),
        );
        assert_eq!(said, ["done"]);
        let (said, _) = session.tool("editor_history", json!({}));
        let last_done = said[0].contains("`Place a prefab`; steps undone");
        assert_eq!(last_done, placed, "{said:?}");
    }
    // Play: the `world` example plays the yard as the editor holds it, steps not written included,
    // without a window, its log in the editor's; paused, stepped, at another speed, then stopped,
    // by the operations' names (`docs/adr/0164-play.md`).
    let (said, result) = session.try_tool("drive_played_game", json!({ "played": true }));
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(said[0].contains("no game plays"), "{said:?}");
    let (said, _) = session.tool("run_operation", json!({ "name": "play.start" }));
    assert_eq!(said, ["done"]);
    let played = |session: &mut Session, what: &str| {
        let started = std::time::Instant::now();
        loop {
            let (said, _) = session.tool("read_logs", json!({ "after": 0 }));
            if said.iter().any(|line| line.contains(what)) {
                return;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(60),
                "{what}: {said:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    };
    played(&mut session, "Play: the game runs");
    played(&mut session, "INFO game:");
    played(&mut session, "Play: the game gave control");
    // The game's tools drive the game that the editor plays, the editor's the editor still: a step
    // made meanwhile reaches the game, a value set reaches the game alone; then the editor again
    // (`docs/adr/0165-editing-while-playing.md`).
    let (said, _) = session.tool("drive_played_game", json!({ "played": true }));
    assert!(
        said[0].contains("drive the game that the editor plays"),
        "{said:?}"
    );
    // The game's token, 32 hexadecimal digits, is the server's alone.
    let token = |said: &str| {
        said.split(|c: char| !c.is_ascii_hexdigit())
            .any(|word| word.len() >= 32)
    };
    assert!(!token(&said[0]), "{said:?}");
    // A step that raises a lamp and places another, which the game places through its own
    // library of scenes.
    session.tool(
        "edit",
        json!({
            "label": "Raise a lamp while it plays",
            "commands": [
                "[SetProperty]\nentity = \"yard/lamp 3\"\ncomponent = \"Transform\"\npath = \"translation.y\"\nvalue = 4.0\n",
                format!("[Place]\nentity = \"yard/lamp-p1ay\"\nparent = \"yard\"\nprefab = \"{LAMP}\"\n"),
            ],
        }),
    );
    let started = std::time::Instant::now();
    while !lamp(&mut session, "lamp 3").contains("y = 4.0") {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(30),
            "the step reaches the game"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let (said, _) = session.tool("list_entities", json!({ "filter": "lamp-p1ay" }));
    assert!(
        said[0].contains("yard/lamp-p1ay"),
        "placed in the game: {said:?}"
    );
    session.tool(
        "set_property",
        json!({
            "entity": "yard/lamp 3",
            "component": "Transform",
            "path": "translation.z",
            "value": "9.0",
        }),
    );
    assert!(lamp(&mut session, "lamp 3").contains("z = 9.0"));
    let (said, _) = session.tool("drive_played_game", json!({ "played": false }));
    assert_eq!(said, ["the game's tools drive the editor again"]);
    let in_editor = lamp(&mut session, "lamp 3");
    assert!(
        in_editor.contains("y = 4.0") && !in_editor.contains("z = 9.0"),
        "{in_editor}"
    );
    for (name, arguments) in [
        ("play.pause", vec![]),
        ("play.step", vec![]),
        ("play.speed", vec!["0.5"]),
    ] {
        let (said, _) = session.tool(
            "run_operation",
            json!({ "name": name, "arguments": arguments }),
        );
        assert_eq!(said, ["done"], "{name}");
    }
    // Stopped while the game's tools drive it: the first that finds it gone points them back at
    // the editor, once the game, which says that it stopped meanwhile, closed its channel.
    session.tool("drive_played_game", json!({ "played": true }));
    let (said, _) = session.tool("run_operation", json!({ "name": "play.stop" }));
    assert_eq!(said, ["done"]);
    let started = std::time::Instant::now();
    loop {
        let (said, result) = session.try_tool("game_state", json!({}));
        if result["isError"] == json!(true) && said[0] != "the game has stopped" {
            assert!(said[0].contains("drive the editor again"), "{said:?}");
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(30),
            "the game stops"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    played(&mut session, "Play: the game stopped");
    // What a placement holds but its root cannot be grouped out of it.
    session.tool(
        "select",
        json!({ "selection": ["yard/lamp 1/post", "yard/lamp 3"] }),
    );
    let (said, result) = session.try_tool("run_operation", json!({ "name": "edit.group" }));
    assert_eq!(result["isError"], json!(true), "{said:?}");
    assert!(said[0].contains("belongs to a placement"), "{said:?}");
    // A lamp moved into the one above it and out again by the keys' operations, each a step
    // (`docs/adr/0168-rearranging-the-hierarchy.md`).
    session.tool("select", json!({ "selection": ["yard/lamp 3"] }));
    for name in ["hierarchy.indent", "hierarchy.outdent"] {
        let (said, _) = session.tool("run_operation", json!({ "name": name }));
        assert_eq!(said, ["done"], "{name}");
    }
    let (said, _) = session.tool("editor_history", json!({}));
    assert!(
        said[0].contains("Move into the sibling above")
            && said[0].contains("Move out of the parent"),
        "{said:?}"
    );
    session.tool("stop_game", json!({}));

    drop(session);
    let status = child.wait().expect("the server ends");
    assert!(status.success(), "the server ended with {status}");
    let _ = fs::remove_dir_all(&directory);
}
