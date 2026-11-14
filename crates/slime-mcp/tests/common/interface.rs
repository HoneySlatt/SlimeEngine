//! What the tests that use a game's interface through the engine's MCP server share: waiting for
//! the frames to apply what a session asked, finding an element, acting on it.

use std::thread;
use std::time::Duration;

use serde_json::json;

use super::session::Session;

/// Asks `ask` again, a tenth of a second apart, up to a hundred times, until `done` holds for what
/// it gives: the frames that the game draws meanwhile apply what the session asked.
pub fn until<T>(what: &str, mut ask: impl FnMut() -> T, done: impl Fn(&T) -> bool) -> T {
    for _ in 0..100 {
        let given = ask();
        if done(&given) {
            return given;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("{what} did not come in a hundred asks");
}

/// Acts `action` on the element at `path`, with what `more` adds, which must be done.
pub fn act(session: &mut Session, action: &str, path: &str, more: serde_json::Value) {
    let mut arguments = json!({ "action": action, "element": path });
    if let (Some(arguments), Some(more)) = (arguments.as_object_mut(), more.as_object()) {
        arguments.extend(more.clone());
    }
    let (said, _) = session.tool("act_on_interface", arguments);
    assert_eq!(said, ["done"], "{action} on {path}");
}

/// The path of the first element of `role` whose name holds `name`, once one shows.
pub fn path_of(session: &mut Session, role: &str, name: &str) -> String {
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

/// The lines of every element that `find_elements` lists with `arguments`, page after page, each
/// page's cursor given back for the next: the whole list, without the lines that say where it goes
/// on or that a page holds none.
pub fn every_element(session: &mut Session, arguments: serde_json::Value) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..1000 {
        let mut asked = arguments.clone();
        if let Some(cursor) = cursor.take() {
            asked["cursor"] = json!(cursor);
        }
        let (said, _) = session.tool("find_elements", asked);
        for line in said.iter().flat_map(|text| text.lines()) {
            match line.strip_prefix("next: ") {
                Some(rest) => cursor = rest.split(' ').next().map(str::to_owned),
                None if line.contains(" — ") => lines.push(line.to_owned()),
                None => {}
            }
        }
        if cursor.is_none() {
            return lines;
        }
    }
    panic!("a list of elements that does not end");
}
