//! An agent's session that uses each widget of a panel through the engine's MCP server
//! (`docs/adr/0148-widgets.md`): it starts the `widgets` example, finds its widgets by their roles
//! and their names, and uses each through the actions that a person's input becomes: it unchecks a
//! check box and gives a slider a value, which the example reads and tells in its bar, opens a
//! folded section, whose slider then shows, moves the handle between two panels, scrolls the list
//! of entities, and chooses a tab, whose panel then shows, where it unchecks a level of the log.
//!
//! It needs a Wayland compositor and the example built (`cargo build --example widgets`):
//! `scripts/check.sh` runs it in a headless Weston with lavapipe, as it runs the `interface` test.
//! It is not in the default run of `cargo test` (`test = false` in Cargo.toml), and has no harness,
//! so that its failures print once.

#[path = "common/session.rs"]
#[expect(dead_code, reason = "this session reads no image")]
mod session;

use std::path::Path;
use std::thread;
use std::time::Duration;

use serde_json::json;

use session::{Session, server};

/// Where the inspector's properties are.
const PROPERTIES: &str = "#0/window/Inspector/split/properties";

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

/// The line, as `find_elements` writes it, of the element at `path`; none where no element is
/// there.
fn line(session: &mut Session, path: &str) -> Option<String> {
    let (lines, _) = session.tool("find_elements", json!({}));
    lines
        .iter()
        .flat_map(|text| text.lines())
        .find(|line| line.starts_with(&format!("{path} — ")))
        .map(str::to_owned)
}

/// Acts `action` on the element at `path`, with what `more` adds, which must be done.
fn act(session: &mut Session, action: &str, path: &str, more: serde_json::Value) {
    let mut arguments = json!({ "action": action, "element": path });
    if let (Some(arguments), Some(more)) = (arguments.as_object_mut(), more.as_object()) {
        arguments.extend(more.clone());
    }
    let (said, _) = session.tool("act_on_interface", arguments);
    assert_eq!(said, ["done"], "{action} on {path}");
}

/// Waits until the bar at the bottom tells `told`, what the example read of the last change.
fn told(session: &mut Session, told: &str) {
    until(
        told,
        || line(session, "#0/window/status/last"),
        |line| {
            line.as_ref()
                .is_some_and(|line| line.contains(&format!("Label `{told}`")))
        },
    );
}

fn main() {
    let server = server();
    let target = server.parent().expect("the target directory");
    let example = target.join(format!("examples/widgets{}", std::env::consts::EXE_SUFFIX));
    assert!(
        example.is_file(),
        "{} is not built: `cargo build --example widgets` first",
        example.display()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    let (mut session, mut child, _) = Session::start(&workspace, "widgets");
    let command = json!([example.to_str().expect("a path of text")]);
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("widgets runs"), "{said:?}");

    // The bar of tabs, once a frame has laid it out, the inspector's tab chosen.
    let tabs = until(
        "the tabs laid out",
        || {
            session
                .try_tool("find_elements", json!({ "role": "tab" }))
                .0
        },
        |said| said.first().is_some_and(|text| text.contains("; at (")),
    );
    assert!(
        tabs[0].starts_with("#0/window/tabs/Inspector — Tab `Inspector`, selected"),
        "{tabs:?}"
    );

    // A check box, found by its role and its name, pressed: unchecked, which the example tells.
    let (found, _) = session.tool(
        "find_elements",
        json!({ "role": "CheckBox", "name": "casts shadows" }),
    );
    let shadows = format!("{PROPERTIES}/Rendering/casts shadows");
    assert!(found[0].starts_with(&format!("{shadows} — CheckBox `casts shadows`, checked")));
    assert!(found[0].ends_with("takes press, focus"), "{found:?}");
    act(&mut session, "press", &shadows, json!({}));
    told(&mut session, "casts shadows off");
    let unchecked = line(&mut session, &shadows).expect("the check box");
    assert!(!unchecked.contains("checked"), "{unchecked}");

    // A slider given a value, within its bounds and on its steps, as a person who moved it.
    let intensity = format!("{PROPERTIES}/Light/intensity/slider");
    let slider = line(&mut session, &intensity).expect("the slider");
    assert!(
        slider.contains("Slider `intensity`, holding \"2.5\""),
        "{slider}"
    );
    assert!(slider.ends_with("takes focus, set_value"), "{slider}");
    act(
        &mut session,
        "set_value",
        &intensity,
        json!({ "value": "7.5" }),
    );
    told(&mut session, "intensity 7.5");
    let (refused, result) = session.try_tool(
        "act_on_interface",
        json!({ "action": "set_value", "element": intensity, "value": "bright" }),
    );
    assert_eq!(result["isError"], json!(true), "{refused:?}");

    // A folded section: what it holds means nothing until a press opens it.
    let physics = format!("{PROPERTIES}/Physics");
    let section = line(&mut session, &physics).expect("the section");
    assert!(section.contains("Details `Physics`"), "{section}");
    assert!(!section.contains("expanded"), "{section}");
    let mass = format!("{PROPERTIES}/Physics/mass/slider");
    assert_eq!(line(&mut session, &mass), None);
    act(&mut session, "press", &physics, json!({}));
    until(
        "the section open",
        || line(&mut session, &mass),
        |line| line.is_some(),
    );
    let section = line(&mut session, &physics).expect("the section");
    assert!(section.contains(", expanded"), "{section}");

    // The handle between the two panels, moved: the first takes half.
    let split = "#0/window/Inspector/split";
    let panels = line(&mut session, split).expect("the two panels");
    assert!(
        panels.contains("Splitter `inspector`, holding \"0.32\""),
        "{panels}"
    );
    act(&mut session, "set_value", split, json!({ "value": "0.5" }));
    until(
        "the handle moved",
        || line(&mut session, split),
        |line| {
            line.as_ref()
                .is_some_and(|line| line.contains("holding \"0.5\""))
        },
    );

    // The list of entities, scrolled: its first rows leave, as the list builds those that show.
    let first = "#0/window/Inspector/split/entities/list/#0";
    assert!(line(&mut session, first).is_some());
    let list = "#0/window/Inspector/split/entities/list";
    act(
        &mut session,
        "scroll",
        list,
        json!({ "x": 0.0, "y": 1100.0 }),
    );
    until(
        "the list scrolled",
        || line(&mut session, first),
        |line| line.is_none(),
    );

    // Another tab chosen: its panel shows, the inspector's leaves.
    act(&mut session, "press", "#0/window/tabs/Console", json!({}));
    let panel = until(
        "the console shown",
        || {
            session
                .try_tool("find_elements", json!({ "role": "TabPanel" }))
                .0
        },
        |said| said.first().is_some_and(|text| text.contains("`Console`")),
    );
    // One panel, the console's: the inspector's left with its tab.
    assert_eq!(panel[0].lines().count(), 1, "{panel:?}");
    let tab = line(&mut session, "#0/window/tabs/Console").expect("the tab");
    assert!(tab.contains(", selected"), "{tab}");
    let console = line(&mut session, "#0/window/Console/split").expect("the console's panels");
    assert!(
        console.contains("Splitter `console`, holding \"0.75\""),
        "{console}"
    );
    act(
        &mut session,
        "press",
        "#0/window/Console/split/levels/info",
        json!({}),
    );
    told(&mut session, "info off");

    let (stopped, _) = session.tool("stop_game", json!({}));
    assert!(stopped[0].contains("the game ended"), "{stopped:?}");
    drop(session);
    let status = child.wait().expect("the server ends with its input");
    assert!(status.success(), "{status}");
    println!("test widgets ... ok");
}
