//! An agent's session that uses a game's interface through the engine's MCP server
//! (`docs/adr/0143-what-agents-see-of-the-interface.md`): it starts the `interface` example, finds
//! a button of its bar by its role and its name, presses it, reads in the world the count that the
//! press changed through a command, captures the button, and finds it again at a pixel. It moves
//! the focus as Tab does, sees an action refused, opens the dialog, a modal root, which keeps the
//! elements below from input, and closes it as Escape does.
//!
//! It needs a Wayland compositor, the example built (`cargo build --example interface`) and the
//! examples' assets cooked, in the directory that its first argument names, or
//! `target/cooked/examples`: `scripts/check.sh` runs it in a headless Weston with lavapipe, as it
//! runs the `views` test. It is not in the default run of `cargo test` (`test = false` in
//! Cargo.toml), and has no harness, so that its failures print once.

#[path = "common/session.rs"]
mod session;

use std::path::Path;
use std::thread;
use std::time::Duration;

use serde_json::json;

use session::{Session, image, server};

/// Asks `ask` again, a tenth of a second apart, up to a hundred times, until `done` holds for what
/// it gives: the frames that the game draws meanwhile apply what the session asked.
fn until<T>(what: &str, mut ask: impl FnMut() -> T, done: impl Fn(&T) -> bool) -> T {
    for _ in 0..100 {
        let given = ask();
        if done(&given) {
            return given;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("{what} did not come in a hundred asks");
}

/// The line of `lines`, as `find_elements` writes them, of the element at `path`.
fn line<'a>(lines: &'a [String], path: &str) -> &'a str {
    lines
        .iter()
        .flat_map(|text| text.lines())
        .find(|line| line.starts_with(&format!("{path} — ")))
        .unwrap_or_else(|| panic!("no element `{path}` in {lines:?}"))
}

/// Where the element of `line` shows: its left column, top row, width and height.
fn bounds(line: &str) -> [u32; 4] {
    let at = line
        .split("; at (")
        .nth(1)
        .unwrap_or_else(|| panic!("`{line}` does not show"));
    let numbers: Vec<u32> = at
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .take(4)
        .map(|part| part.parse().expect("a number"))
        .collect();
    numbers.try_into().expect("four numbers")
}

fn main() {
    let server = server();
    let target = server.parent().expect("the target directory");
    let example = target.join(format!(
        "examples/interface{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(
        example.is_file(),
        "{} is not built: `cargo build --example interface` first",
        example.display()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = std::env::args()
        .nth(1)
        .map_or_else(|| workspace.join("target/cooked/examples"), Into::into);

    let (mut session, mut child, _) = Session::start(&workspace, "interface");
    let command = json!([
        example.to_str().expect("a path of text"),
        "--assets",
        assets.to_str().expect("a path of text"),
    ]);
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("interface runs"), "{said:?}");

    // The bar, once a frame has laid it out, and its buttons, the first of which opens the dialog.
    let toolbar = until(
        "the bar laid out",
        || {
            session
                .try_tool("find_elements", json!({ "role": "toolbar" }))
                .0
        },
        |said| said.first().is_some_and(|text| text.contains("; at (")),
    );
    assert!(
        line(&toolbar, "#0/window/bar").contains("Toolbar `bar`"),
        "{toolbar:?}"
    );
    let (buttons, _) = session.tool("find_elements", json!({ "role": "Button" }));
    let second = line(&buttons, "#0/window/bar/#1");
    let name = second
        .split('`')
        .nth(1)
        .expect("a button named by its label");
    assert!(second.ends_with("takes press, focus"), "{second}");
    // Found again by its role and its name.
    let (found, _) = session.tool(
        "find_elements",
        json!({ "role": "button", "name": name.to_uppercase() }),
    );
    assert_eq!(line(&found, "#0/window/bar/#1"), second);

    // Pressed, it counts a press in the world, by a command that the next frame applies.
    let (presses, _) = session.tool("inspect_entity", json!({ "entity": "presses" }));
    assert!(presses[0].contains("count = 0"), "{presses:?}");
    let (said, _) = session.tool(
        "act_on_interface",
        json!({ "action": "press", "element": "#0/window/bar/#1" }),
    );
    assert_eq!(said, ["done"]);
    until(
        "the press counted",
        || {
            session
                .tool("inspect_entity", json!({ "entity": "presses" }))
                .0
        },
        |said| said[0].contains("count = 1"),
    );
    // The button's part of the image, as `find_elements` placed it, and the button found at its
    // middle.
    let [x, y, width, height] = bounds(second);
    let (said, captured) =
        session.tool("capture_element", json!({ "element": "#0/window/bar/#1" }));
    let (captured_width, captured_height, pixels) = image(&captured);
    assert_eq!(
        (captured_width, captured_height),
        (width, height),
        "{said:?}"
    );
    let colors: std::collections::HashSet<&[u8]> = pixels.chunks(4).collect();
    assert!(
        colors.len() > 2,
        "a label on a fill: {} colors",
        colors.len()
    );
    let (at, _) = session.tool(
        "element_at",
        json!({ "x": x + width / 2, "y": y + height / 2 }),
    );
    assert!(at[0].starts_with("#0/window/bar/#1 — Button"), "{at:?}");

    // Tab, as an agent sends it: a button takes the focus.
    let (said, _) = session.tool("act_on_interface", json!({ "action": "next" }));
    assert_eq!(said, ["done"]);
    until(
        "a button focused",
        || session.tool("find_elements", json!({ "role": "Button" })).0,
        |said| said[0].contains(", focused"),
    );
    // What an element does not take is refused, and says what it takes.
    let (refused, result) = session.try_tool(
        "act_on_interface",
        json!({ "action": "press", "element": "#0/window/bar" }),
    );
    assert_eq!(result["isError"], json!(true), "{refused:?}");
    assert!(refused[0].contains("takes no `press`"), "{refused:?}");
    let (refused, result) = session.try_tool("act_on_interface", json!({ "action": "dance" }));
    assert_eq!(result["isError"], json!(true), "{refused:?}");

    // The first button opens the dialog, a modal root: the bar's buttons take nothing below it, and
    // Escape, as an agent sends it, closes it.
    session.tool(
        "act_on_interface",
        json!({ "action": "press", "element": "#0/window/bar/#0" }),
    );
    let dialog = until(
        "the dialog open",
        || {
            session
                .try_tool("find_elements", json!({ "role": "Dialog" }))
                .0
        },
        |said| {
            said.first()
                .is_some_and(|text| text.contains("Dialog `dialog`"))
        },
    );
    assert!(dialog[0].starts_with("#1/"), "{dialog:?}");
    let (buttons, _) = session.tool("find_elements", json!({ "role": "Button" }));
    assert!(
        !line(&buttons, "#0/window/bar/#1").contains("takes"),
        "{buttons:?}"
    );
    session.tool("act_on_interface", json!({ "action": "cancel" }));
    until(
        "the dialog closed",
        || session.tool("find_elements", json!({ "role": "Dialog" })).0,
        |said| said == &["no element"],
    );

    let (stopped, _) = session.tool("stop_game", json!({}));
    assert!(stopped[0].contains("the game ended"), "{stopped:?}");
    drop(session);
    let status = child.wait().expect("the server ends with its input");
    assert!(status.success(), "{status}");
    println!("test interface ... ok");
}
