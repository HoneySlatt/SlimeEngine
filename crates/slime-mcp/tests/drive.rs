//! An agent's session, played by this test: it speaks the Model Context Protocol to the engine's MCP
//! server on its standard input and output, starts *Hunna Wood* through it, pauses and steps it,
//! reads the hero, changes a value, walks the hero with a scripted key, captures the game's image,
//! reads its log and stops it (`docs/adr/0122-the-development-channel.md`).
//!
//! It needs a Wayland compositor, the game built (`cargo build -p hunnawood`) beside the server and
//! its zone cooked in `target/cooked/hunnawood`: `scripts/check.sh` runs it in a headless Weston
//! with lavapipe, as it runs the windowed lifecycle test (ADR-0024). It is not in the default run of
//! `cargo test` (`test = false` in Cargo.toml), and has no harness, so that its failures print once.

#[path = "common/session.rs"]
mod session;

use std::path::Path;

use serde_json::json;

use session::{Session, image, server};

fn main() {
    let server = server();
    let target = server.parent().expect("the target directory");
    let game = target.join(format!("hunnawood{}", std::env::consts::EXE_SUFFIX));
    assert!(
        game.is_file(),
        "{} is not built: `cargo build -p hunnawood` first",
        game.display()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cooked = workspace.join("target/cooked/hunnawood");
    // In the build's own directory of temporary files, never the player's save; a run before this one
    // may have left it, with its lock.
    let save = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-drive.toml");
    let lock = save.with_extension("toml.lock");
    for file in [&save, &lock] {
        let _ = std::fs::remove_file(file);
    }

    let (mut session, mut child, initialized) = Session::start(&workspace, "drive");
    assert!(
        initialized["capabilities"]["tools"].is_object(),
        "{initialized}"
    );
    assert_eq!(initialized["serverInfo"]["name"], json!("slime-mcp"));
    let tools = session.call("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    for name in [
        "start_game",
        "pause",
        "step",
        "inspect_entity",
        "set_property",
        "capture",
        "save_game",
        "load_game",
    ] {
        assert!(names.contains(&name), "{name}: {names:?}");
    }

    let command = json!([
        game.to_str().expect("a path of text"),
        "--new",
        "--save",
        save.to_str().expect("a path of text"),
        "--assets",
        cooked.to_str().expect("a path of text"),
    ]);
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("hunnawood runs"), "{said:?}");
    session.tool("pause", json!({}));
    let (hero, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert!(
        hero[0].contains("[Transform]") && hero[0].contains("[hunnawood::Look]"),
        "{hero:?}"
    );
    session.tool(
        "set_property",
        json!({ "entity": "hero", "component": "hunnawood::Look", "path": "yaw", "value": "1.0" }),
    );
    let (hero, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert!(hero[0].contains("yaw = 1.0"), "{hero:?}");
    // The game holds its time while its zone loads; steps asked for while paused run all the same:
    // W held for 30 steps walks the hero.
    let place = |hero: &[String]| {
        let transform = hero[0].split("[Transform]").nth(1).expect("a transform");
        transform
            .lines()
            .find(|line| line.starts_with("translation"))
            .map(str::to_owned)
    };
    let before = place(&hero);
    session.tool("press_key", json!({ "key": "KeyW" }));
    session.tool("step", json!({ "steps": 30 }));
    session.tool("release_key", json!({ "key": "KeyW" }));
    let (walked, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert_ne!(place(&walked), before, "the hero walked: {walked:?}");
    // Saved there, to a file of the test's own, moved, then loaded: the hero is back where it was
    // saved. Moved by a command rather than walked: each request waits for a frame, which costs
    // seconds on a CPU driver.
    let saved = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-drive-saved.toml");
    let _ = std::fs::remove_file(&saved);
    let file = json!({ "file": saved.to_str().expect("a path of text") });
    session.tool("save_game", file.clone());
    assert!(saved.is_file(), "saved");
    session.tool(
        "set_property",
        json!({ "entity": "hero", "component": "Transform", "path": "translation.x", "value": "40.0" }),
    );
    session.tool("load_game", file);
    let (loaded, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert_eq!(place(&loaded), place(&walked), "{loaded:?}");
    let (state, _) = session.tool("game_state", json!({}));
    assert!(state[0].contains("paused"), "{state:?}");
    let (_, captured) = session.tool("capture", json!({}));
    let (width, height, pixels) = image(&captured);
    assert!(width > 0 && height > 0);
    // The zone drawn, not a cleared image: many colors.
    let colors: std::collections::HashSet<&[u8]> = pixels.chunks(4).collect();
    assert!(colors.len() > 1000, "{} colors", colors.len());
    let (lines, _) = session.tool("read_logs", json!({ "after": 0 }));
    assert!(lines[0].contains("development channel"), "{lines:?}");
    let (stopped, _) = session.tool("stop_game", json!({}));
    assert!(stopped[0].contains("the game ended"), "{stopped:?}");

    drop(session);
    let status = child.wait().expect("the server ends with its input");
    assert!(status.success(), "{status}");
    for file in [&save, &lock, &saved] {
        let _ = std::fs::remove_file(file);
    }
    println!("test drive ... ok");
}
