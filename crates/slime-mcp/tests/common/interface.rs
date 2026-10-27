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
