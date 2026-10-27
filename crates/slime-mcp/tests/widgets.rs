//! An agent's session that uses each widget of a panel through the engine's MCP server
//! (`docs/adr/0148-widgets.md`): it starts the `widgets` example, finds its widgets by their roles
//! and their names, and uses each through the actions that a person's input becomes: it unchecks a
//! check box and gives a slider a value, which the example reads and tells in its bar, opens a
//! folded section, whose slider then shows, moves the handle between two panels, scrolls the list
//! of entities, and chooses a tab, whose panel then shows, where it unchecks a level of the log. It
//! uses the rich widgets (`docs/adr/0151-rich-widgets.md`): it chooses a tool among segments, turns
//! a button that toggles off, searches the list of entities and gives a component of the position a
//! value as an expression of lengths, resets the light by a button of its section's header and
//! reads a property's help as its description. It uses what opens over the panel too
//! (`docs/adr/0150-layers.md`): it chooses the quality of shadows by the text of an option, reads
//! the list's tooltip as its description, opens the bar's menus, shows the floating panel of
//! statistics through an item that *View* holds, and opens a recent level through *File*'s submenu,
//! whose menus then close; it gives a command that the example tells of in a notification, and
//! finds a command in the palette of commands, which it gives, and opens and closes the sheet of
//! shortcuts.
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

/// The path of the first element of `role` whose name holds `name`, once one shows.
fn path_of(session: &mut Session, role: &str, name: &str) -> String {
    let found = until(
        name,
        || {
            session
                .try_tool("find_elements", json!({ "role": role, "name": name }))
                .0
        },
        |said| said.first().is_some_and(|text| text.contains(" — ")),
    );
    found[0]
        .split(" — ")
        .next()
        .expect("a path before the role")
        .to_owned()
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
    // Its tooltip read as its description.
    assert!(
        tabs[0].starts_with(
            "#0/window/tabs/Inspector — Tab `Inspector`, described \"The entities and their \
             properties\", selected"
        ),
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

    // The rich widgets: a tool chosen among segments, the snapping turned off, the list of entities
    // searched, a component of the position given a value.
    act(
        &mut session,
        "press",
        "#0/window/tools/tool/Turn",
        json!({}),
    );
    told(&mut session, "tool turn");
    let snap = line(&mut session, "#0/window/tools/snap").expect("the button that toggles");
    assert!(snap.contains("ToggleButton `Snap`"), "{snap}");
    assert!(snap.contains(", checked"), "{snap}");
    act(&mut session, "press", "#0/window/tools/snap", json!({}));
    told(&mut session, "snap off");
    let search = "#0/window/Inspector/split/entities/search";
    let field = line(&mut session, search).expect("the field of search");
    assert!(
        field.contains(
            "SearchInput `search entities`, holding \"\", showing \"Search entities\" while empty"
        ),
        "{field}"
    );
    act(
        &mut session,
        "set_value",
        search,
        json!({ "value": "torch" }),
    );
    told(&mut session, "25 entities found");
    // In meters, given as an expression of lengths.
    let x = format!("{PROPERTIES}/Transform/position/X");
    let field = line(&mut session, &x).expect("the position's first field");
    assert!(field.contains("holding \"1.5 m\""), "{field}");
    act(
        &mut session,
        "set_value",
        &x,
        json!({ "value": "1m + 150cm" }),
    );
    told(&mut session, "position x 2.5");
    // The light's header holds a button that resets it; a property's help is its description.
    act(
        &mut session,
        "press",
        &format!("{PROPERTIES}/Light/header/reset"),
        json!({}),
    );
    told(&mut session, "light reset");
    until(
        "the light reset",
        || line(&mut session, &intensity),
        |line| {
            line.as_ref()
                .is_some_and(|line| line.contains("holding \"2.5\""))
        },
    );
    // The light's color well, given a hexadecimal code, as its picker's field takes one
    // (docs/adr/0152-the-dev-overlay-on-the-engines-interface.md).
    let well = format!("{PROPERTIES}/Light/color/well");
    let color = line(&mut session, &well).expect("the light's color well");
    assert!(
        color.contains("ColorWell `color`, holding \"#ffddad\""),
        "{color}"
    );
    assert!(color.contains("has_popup"), "{color}");
    act(
        &mut session,
        "set_value",
        &well,
        json!({ "value": "#3a9bdc" }),
    );
    told(&mut session, "light color #3a9bdc");
    let roughness = line(
        &mut session,
        &format!("{PROPERTIES}/Material/roughness/slider"),
    )
    .expect("the roughness");
    assert!(
        roughness.contains("described \"roughness: How rough the surface is"),
        "{roughness}"
    );

    // A list to choose from, read with its tooltip, given an option by its text.
    let quality = format!("{PROPERTIES}/Rendering/shadow quality/choice");
    let choice = line(&mut session, &quality).expect("the list to choose from");
    assert!(
        choice.contains(
            "ComboBox `shadow quality`, holding \"High\", described \"How sharp the shadows are, \
             and what they cost\", has_popup"
        ),
        "{choice}"
    );
    assert!(
        choice.ends_with("takes press, focus, set_value"),
        "{choice}"
    );
    act(
        &mut session,
        "set_value",
        &quality,
        json!({ "value": "Low" }),
    );
    told(&mut session, "shadows low");

    // The bar's menu View, opened: its item shows the floating panel of statistics.
    act(&mut session, "press", "#0/window/menus/View", json!({}));
    let statistics = path_of(&mut session, "MenuItemCheckBox", "Statistics");
    act(&mut session, "press", &statistics, json!({}));
    told(&mut session, "statistics shown");
    let panel = path_of(&mut session, "Pane", "Statistics");
    let pane = line(&mut session, &panel).expect("the floating panel");
    assert!(pane.contains(", expanded"), "{pane}");

    // File's submenu of recent levels, through its item: a level opened, the menus closed.
    act(&mut session, "press", "#0/window/menus/File", json!({}));
    let recent = path_of(&mut session, "MenuItem", "Recent");
    let item = line(&mut session, &recent).expect("the item");
    assert!(item.contains("has_popup"), "{item}");
    act(&mut session, "press", &recent, json!({}));
    let zone = path_of(&mut session, "MenuItem", "zone.scene");
    act(&mut session, "press", &zone, json!({}));
    told(&mut session, "open zone.scene");
    let (menus, _) = session.tool("find_elements", json!({ "role": "Menu" }));
    assert_eq!(menus, ["no element"], "the menus closed");

    // A command that the example tells of in a notification, as a shortcut gives it.
    let (said, _) = session.tool(
        "act_on_interface",
        json!({ "action": "command", "command": "save" }),
    );
    assert_eq!(said, ["done"]);
    path_of(&mut session, "Status", "yard.scene is saved");
    // The palette of commands: what is given its field finds the commands, a press gives one.
    let (said, _) = session.tool(
        "act_on_interface",
        json!({ "action": "command", "command": "palette" }),
    );
    assert_eq!(said, ["done"]);
    let field = path_of(&mut session, "SearchInput", "command");
    act(
        &mut session,
        "set_value",
        &field,
        json!({ "value": "statistics" }),
    );
    let found = path_of(&mut session, "ListBoxOption", "Show or hide the statistics");
    act(&mut session, "press", &found, json!({}));
    told(&mut session, "statistics hidden");
    // The sheet of shortcuts, opened by its command, read, and closed as Escape closes it.
    let (said, _) = session.tool(
        "act_on_interface",
        json!({ "action": "command", "command": "shortcuts" }),
    );
    assert_eq!(said, ["done"]);
    path_of(&mut session, "Dialog", "Keyboard shortcuts");
    let (said, _) = session.tool("act_on_interface", json!({ "action": "cancel" }));
    assert_eq!(said, ["done"]);
    until(
        "the sheet closed",
        || {
            session
                .try_tool("find_elements", json!({ "role": "Dialog" }))
                .0
        },
        |said| said == &["no element"],
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
