//! How the editor's window adapts to a compositor of the series
//! (`docs/adr/0195-a-series-of-headless-compositors.md`): it starts the `editor` example on a copy
//! of the yard through the engine's MCP server and, where the compositor takes input, moves the
//! pointer into the window, which then shows a cursor, and makes a round trip through the system's
//! clipboard: a text given to the palette's field, selected and copied with Ctrl+A and Ctrl+C, the
//! field emptied, then the text pasted with Ctrl+V, as a person types them, through the
//! compositor.
//!
//! `scripts/compositors.sh` runs it in each compositor of the series, with `WAYLAND_DEBUG` set, so
//! that the editor's log holds the protocols that it used: the script reads them there, and this
//! test tells where the log is. It gives input through `scripts/compositor-input.py`, which the
//! script serves on the socket that `SLIME_COMPOSITOR_INPUT` names; without it, in Weston, which
//! takes no input, it makes no round trip, and says so. It prints what it found, one line each, `clipboard: …`, for the script's report.
//!
//! It needs the example built (`cargo build --example editor`) and the examples' assets cooked, in
//! the directory that its first argument names, or `target/cooked/examples`. It is not in the
//! default run of `cargo test` (`test = false` in Cargo.toml), and has no harness, so that its
//! failures print once.

#[path = "common/interface.rs"]
mod interface;
#[path = "common/session.rs"]
#[expect(dead_code, reason = "this session reads no image")]
mod session;

use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use serde_json::json;

use interface::{act, path_of};
use session::{Session, server};

/// The text that goes through the clipboard: a person's, with a space.
const COPIED: &str = "slime clipboard";

/// Gives the compositor a person's input, through the tool of its family.
fn input(workspace: &Path, arguments: &[&str]) {
    let status = Command::new(workspace.join("scripts/compositor-input.py"))
        .args(arguments)
        .status()
        .expect("scripts/compositor-input.py runs");
    assert!(status.success(), "input {arguments:?}: {status}");
}

/// What the field at `path` holds, as `find_elements` tells it.
fn holding(session: &mut Session, path: &str) -> String {
    let (said, _) = session.tool("find_elements", json!({ "role": "SearchInput" }));
    let field = said
        .iter()
        .flat_map(|text| text.lines())
        .find(|line| line.starts_with(path))
        .unwrap_or_else(|| panic!("no field {path} in {said:?}"));
    field
        .split_once("holding \"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map_or_else(String::new, |(text, _)| text.to_owned())
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
    let takes_input = std::env::var_os("SLIME_COMPOSITOR_INPUT").is_some();
    // A copy of the yard, and a journal, in the build's own directory of temporary files.
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-compositor");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a directory of the test's own");
    let original = workspace.join("examples/assets/yard.scene");
    let level = directory.join("yard.scene");
    fs::copy(&original, &level).expect("the yard copied");
    fs::copy(
        original.with_extension("scene.meta"),
        level.with_extension("scene.meta"),
    )
    .expect("its metadata copied");
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

    let (mut session, mut child, _) = Session::start(&workspace, "compositor");
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    let log = said[0]
        .split_once("its log in ")
        .map(|(_, log)| log.trim())
        .unwrap_or_else(|| panic!("no log in {said:?}"));
    println!("log: {log}");
    path_of(&mut session, "Label", "all written");

    if !takes_input {
        println!("clipboard: not run, no input");
    } else {
        // Into the window, which a compositor places in the middle of its output or from its
        // corner, wide enough to hold the middle: the window then shows a cursor.
        input(&workspace, &["move-to", "960", "540"]);
        let (said, _) = session.tool("run_operation", json!({ "name": "view.palette" }));
        assert_eq!(said, ["done"]);
        let field = path_of(&mut session, "SearchInput", "command");
        act(
            &mut session,
            "set_value",
            &field,
            json!({ "value": COPIED }),
        );
        act(&mut session, "focus", &field, json!({}));
        input(&workspace, &["key", "ctrl+a"]);
        input(&workspace, &["key", "ctrl+c"]);
        act(&mut session, "set_value", &field, json!({ "value": "" }));
        act(&mut session, "focus", &field, json!({}));
        input(&workspace, &["key", "ctrl+v"]);
        // The paste reaches the field at the frame after the keys; none comes without a clipboard,
        // which a compositor that gives no protocol of the clipboard's managers leaves the
        // editor (docs/debt.md).
        let pasted = (0..30).find_map(|_| {
            let text = holding(&mut session, &field);
            thread::sleep(Duration::from_millis(100));
            (!text.is_empty()).then_some(text)
        });
        match pasted {
            Some(text) => {
                assert_eq!(text, COPIED);
                println!("clipboard: round trip");
            }
            None => println!("clipboard: nothing pasted"),
        }
        session.tool("act_on_interface", json!({ "action": "cancel" }));
    }

    let (stopped, _) = session.tool("stop_game", json!({}));
    assert!(stopped[0].contains("the game ended"), "{stopped:?}");
    drop(session);
    let status = child.wait().expect("the server ends with its input");
    assert!(status.success(), "{status}");
    println!("test compositor ... ok");
}
