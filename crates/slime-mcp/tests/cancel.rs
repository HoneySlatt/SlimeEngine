//! A call that its agent cancels while the program holds its edit, as an editor holds an agent's
//! edits while a person drags, becomes the core's cancel of that request, through the engine's MCP
//! server and uwui's MCP library, and is never applied
//! (`docs/adr/0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md`).
//!
//! The program is this test's own binary, started by the server's `start_game` with a token, as a
//! program on the development channel's listener alone (`common/program.rs`): it holds the edits
//! that come while a key is held, a drag, and applies them once it is let go, but those that their
//! tool cancelled, which it answers so. That a program of the engine's never applies an edit
//! cancelled while it waits for a drag is `slime-app`'s test
//! `a_read_passes_a_waiting_edit_and_a_cancelled_edit_is_never_applied`; this one proves the
//! server's half. It needs no window, and runs on Windows too.

#[expect(dead_code, reason = "this test starts no silent program")]
#[path = "common/program.rs"]
mod program;
#[path = "common/session.rs"]
#[expect(dead_code, reason = "this session reads no image")]
mod session;

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use session::Session;

/// The test that, run with a token in its environment, is the program.
const PROGRAM: &str = "the_program_of_the_test";

/// How long the test waits for the program to see what it was asked.
const SOON: Duration = Duration::from_secs(30);

/// The program, in the run that the server starts; nothing in a run of the tests.
#[test]
fn the_program_of_the_test() {
    program::run_if_started();
}

/// Calls the tool `name` with `arguments`, and returns the texts of its result, which must not be
/// an error; every message read before its answer is checked not to answer the call `held`, which
/// its agent cancelled: MCP answers no cancelled call.
fn tool(session: &mut Session, held: Option<u64>, name: &str, arguments: Value) -> Vec<String> {
    let asked = session.send(
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    );
    loop {
        let message = session.next_message(name);
        if let Some(held) = held {
            assert_ne!(
                message["id"],
                json!(held),
                "a cancelled call answered: {message}"
            );
        }
        if message["id"] == json!(asked) {
            assert_ne!(
                message["result"]["isError"],
                json!(true),
                "{name}: {message}"
            );
            return message["result"]["content"]
                .as_array()
                .expect("content")
                .iter()
                .filter_map(|block| block["text"].as_str().map(str::to_owned))
                .collect();
        }
    }
}

/// The program's report of what it saw.
fn seen(session: &mut Session, held: Option<u64>) -> String {
    tool(session, held, "read_report", json!({ "name": "seen" })).join("\n")
}

/// Reads the program's report until it holds `what`, within [`SOON`].
fn wait_for(session: &mut Session, held: Option<u64>, what: &str) -> String {
    let started = Instant::now();
    loop {
        let report = seen(session, held);
        if report.contains(what) {
            return report;
        }
        assert!(started.elapsed() < SOON, "{what:?} not seen: {report}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_call_cancelled_while_its_edit_is_held_is_cancelled_in_the_program_and_never_applied() {
    let program = std::env::current_exe().expect("the test's program");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (mut session, mut server, _) = Session::start(&workspace, "cancel");
    let (said, _) = session.tool(
        "start_game",
        json!({
            "command": [program, "--exact", PROGRAM, "--nocapture", "--test-threads=1"],
            "timeout_seconds": 60,
        }),
    );
    assert!(said[0].contains("the tests' program runs"), "{said:?}");
    let step = |label: &str| {
        json!({
            "name": "edit",
            "arguments": { "label": label, "commands": [] },
        })
    };
    // A step applied at once, while no drag goes on.
    let (said, _) = session.tool("edit", step("1")["arguments"].clone());
    assert_eq!(said, ["done"]);
    // A step held by a drag, then cancelled by its agent.
    session.tool("press_key", json!({ "key": "KeyW" }));
    let held = session.send("tools/call", step("2"));
    wait_for(&mut session, None, "held: [2]");
    session.notify(
        "notifications/cancelled",
        json!({ "requestId": held, "reason": "the agent gave it up" }),
    );
    // From here on, every message read is checked not to answer it.
    let held = Some(held);
    let report = wait_for(&mut session, held, "cancelled: [2]");
    assert!(report.contains("held: []"), "{report}");
    // The drag ends: what it held is applied, the step cancelled not.
    tool(&mut session, held, "release_key", json!({ "key": "KeyW" }));
    let report = seen(&mut session, held);
    assert!(
        report.starts_with("applied: [1];"),
        "never applied: {report}"
    );
    let said = tool(&mut session, held, "stop_game", json!({}));
    assert!(said[0].contains("the game ended"), "{said:?}");
    drop(session);
    let status = server.wait().expect("the server ends with its input");
    assert!(status.success(), "{status}");
}
