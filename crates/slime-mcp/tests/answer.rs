//! The engine's answer through its MCP server, measured: an agent's call timed from the line that
//! it writes to the line that it reads, through the server, the development channel and the game's
//! frame that takes the request. It starts the `editor` example on a copy of the yard, in a window,
//! and times three calls, each a median and the slowest of 21 after three to warm up, taken in
//! turn: `game_state`, the floor of a call; `find_elements` without a filter, a list of the
//! editor's whole interface gathered from the game's pages; `editor_journal` from its start, a
//! page of its text. It prints each figure, with what the list and the page held. Then it times,
//! as many times, a step made by `edit` to the answer of `editor_journal` waiting at the journal's
//! end, held by the editor before the step comes, and a press that opens the File menu to the
//! answer of `wait_for_elements` waiting for its item *Open a level…*, held before the press comes,
//! the menu closed after each. It needs a
//! Wayland compositor, the example built (`cargo build --release --example editor`) and the
//! examples' assets cooked, in the directory that its first argument names, or
//! `target/cooked/examples`: run it in a headless Weston with lavapipe.
//!
//! With `--at-once` as its first argument, it times instead what the server and the development
//! channel add, with no frame to wait for: it starts a program of the tests' own, its own binary on
//! the channel's listener alone (`common/program.rs`), which answers at once, and times
//! `read_report`, a short reply, and `list_entities`, a page of 100 entities, each a median and the
//! slowest of 201 after 21; it needs nothing else.
//!
//! `SLIME_MCP_SERVER` names the server timed, another commit's build, as the measure's "before";
//! the programs are this build's either way. As a server built before `editing` 3 is refused that
//! family by this build's editor, neither `editor_journal` nor the wait is timed against another
//! build, nor the interface's wait, which a server built before `wait_for_elements` lacks. Without
//! the editor's check of its waits right after its frame's extraction, the interface's wait would
//! be answered only after the frame's drawing: this measure is what shows that point at work. A
//! measure, not a check (`docs/measurements.md`): run it in release, on a machine at rest. It is
//! not in the default run of `cargo test` (`test = false` in Cargo.toml), and has no harness, so
//! that it prints once.

#[expect(dead_code, reason = "this measure acts on no element by `act`")]
#[path = "common/interface.rs"]
mod interface;
#[path = "common/journal.rs"]
mod journal;
#[expect(dead_code, reason = "this measure starts no silent program")]
#[path = "common/program.rs"]
mod program;
#[expect(
    dead_code,
    reason = "this measure reads no image, and starts each server it names"
)]
#[path = "common/session.rs"]
mod session;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use interface::{path_of, until};
use session::{Session, server};

/// How many calls of each kind are timed on the editor's window, after [`WARM_UP`] untimed: each
/// waits for a frame.
const TIMED: usize = 21;
/// How many calls of each kind run first on the editor's window, untimed.
const WARM_UP: usize = 3;
/// How many calls of each kind are timed on the program that answers at once, after
/// [`WARM_UP_AT_ONCE`] untimed: each takes a fraction of a millisecond.
const TIMED_AT_ONCE: usize = 201;
/// How many calls of each kind run first on that program, untimed.
const WARM_UP_AT_ONCE: usize = 21;

/// The time that a call of `name` with `arguments` takes, and the texts that it gave.
fn timed(session: &mut Session, name: &str, arguments: &Value) -> (Duration, Vec<String>) {
    let started = Instant::now();
    let (said, _) = session.tool(name, arguments.clone());
    (started.elapsed(), said)
}

/// The server timed: the build that `SLIME_MCP_SERVER` names, another commit's, as the measure's
/// "before"; this build's otherwise.
fn timed_server() -> std::path::PathBuf {
    std::env::var_os("SLIME_MCP_SERVER").map_or_else(server, Into::into)
}

/// The median and the slowest of `times`, in milliseconds.
fn figures(times: &mut [Duration]) -> (f64, f64) {
    times.sort();
    let milliseconds = |time: Duration| time.as_secs_f64() * 1000.0;
    (
        milliseconds(times[times.len() / 2]),
        milliseconds(times[times.len() - 1]),
    )
}

fn main() {
    if program::run_if_started() {
        return;
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if std::env::args().nth(1).as_deref() == Some("--at-once") {
        let program = std::env::current_exe().expect("the measure's program");
        let (mut session, mut child, _) =
            Session::start_with(&timed_server(), &workspace, "answer-at-once");
        let (said, _) = session.tool("start_game", json!({ "command": [program] }));
        assert!(said[0].contains("the tests' program runs"), "{said:?}");
        let calls = [
            ("read_report", json!({ "name": "seen" })),
            ("list_entities", json!({ "filter": "" })),
        ];
        measure(&mut session, &calls, TIMED_AT_ONCE, WARM_UP_AT_ONCE);
        stop(session, &mut child);
        return;
    }
    // The examples beside this build's server, whichever server is timed.
    let server = server();
    let target = server.parent().expect("the target directory");
    let example = target.join(format!("examples/editor{}", std::env::consts::EXE_SUFFIX));
    assert!(
        example.is_file(),
        "{} is not built: `cargo build --release --example editor` first",
        example.display()
    );
    let assets = std::env::args()
        .nth(1)
        .map_or_else(|| workspace.join("target/cooked/examples"), Into::into);
    // A copy of the yard, and a journal, in the build's own directory of temporary files.
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-answer");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a directory of the measure's own");
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

    let (mut session, mut child, _) = Session::start_with(&timed_server(), &workspace, "answer");
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("editor runs"), "{said:?}");
    // The window laid out, and the first run's hint gone, so that each call finds the same
    // interface.
    until(
        "the window laid out",
        || {
            session
                .tool("find_elements", json!({ "role": "menubar" }))
                .0
        },
        |said| said.first().is_some_and(|text| text.contains("; at (")),
    );
    until(
        "the hint gone",
        || session.tool("find_elements", json!({})).0,
        |said| !said.iter().any(|text| text.contains("notifications/")),
    );

    // A server of another build may speak an `editing` of before, which this editor refuses, and
    // lack the tool that waits on the interface.
    let other_server = std::env::var_os("SLIME_MCP_SERVER").is_some();
    let mut calls = vec![("game_state", json!({})), ("find_elements", json!({}))];
    if !other_server {
        calls.push(("editor_journal", json!({})));
    }
    measure(&mut session, &calls, TIMED, WARM_UP);
    if !other_server {
        measure_wait(&mut session);
        measure_interface_wait(&mut session);
    }
    stop(session, &mut child);
}

/// Times, as many times as [`measure`] times each call, a step's call to the answer of a call
/// that waits for the journal's next entry, held by the editor before the step comes, and prints
/// the figures.
fn measure_wait(session: &mut Session) {
    let mut end = journal::end(session);
    let mut times = Vec::with_capacity(TIMED);
    for round in 0..WARM_UP + TIMED {
        let waiting = session.send_tool("editor_journal", json!({ "cursor": end, "wait": 50 }));
        // So that the editor holds the wait before the step comes.
        std::thread::sleep(Duration::from_millis(300));
        let step = json!({
            "label": "Move the lamp",
            "commands": [format!(
                "[SetProperty]\nentity = \"yard/lamp 1\"\ncomponent = \"Transform\"\n\
                 path = \"translation.x\"\nvalue = {}.0\n",
                round % 2 + 5
            )],
        });
        let started = Instant::now();
        let stepping = session.send_tool("edit", step);
        let (said, result) = session.tool_answer(waiting);
        let time = started.elapsed();
        assert_ne!(result["isError"], json!(true), "{said:?}");
        assert!(
            said.last()
                .is_some_and(|page| page.contains("label = \"Move the lamp\"")),
            "the step's entry: {said:?}"
        );
        assert_eq!(session.tool_answer(stepping).0, ["done"]);
        end = journal::cursor(&said[0], "end")
            .unwrap_or_else(|| panic!("the journal's new end: {said:?}"));
        if round >= WARM_UP {
            times.push(time);
        }
    }
    let (median, slowest) = figures(&mut times);
    println!(
        "a step to the answer of editor_journal waiting: median {median:.3} ms, slowest \
         {slowest:.3} ms, of {TIMED}"
    );
}

/// Times, as many times as [`measure`] times each call, a press that opens the File menu to the
/// answer of a call that waits for its item *Open a level…*, held by the editor before the press
/// comes, and prints the figures; the menu is closed after each.
fn measure_interface_wait(session: &mut Session) {
    let file = path_of(session, "MenuItem", "File");
    let item = json!({ "role": "MenuItem", "name": "Open a level" });
    let mut times = Vec::with_capacity(TIMED);
    for round in 0..WARM_UP + TIMED {
        let waiting = session.send_tool("wait_for_elements", item.clone());
        // So that the editor holds the wait before the press comes.
        std::thread::sleep(Duration::from_millis(300));
        assert!(!session.answered(waiting), "the menu is closed");
        let started = Instant::now();
        let press = json!({ "action": "press", "element": file });
        let pressing = session.send_tool("act_on_interface", press);
        let (said, result) = session.tool_answer(waiting);
        let time = started.elapsed();
        assert_ne!(result["isError"], json!(true), "{said:?}");
        assert!(
            said[0]
                .lines()
                .any(|line| line.starts_with("menu/") && line.contains("— MenuItem")),
            "the menu's item: {said:?}"
        );
        assert_eq!(session.tool_answer(pressing).0, ["done"]);
        session.tool("act_on_interface", json!({ "action": "cancel" }));
        let gone = json!({ "role": "MenuItem", "name": "Open a level", "absent": true });
        let (said, _) = session.tool("wait_for_elements", gone);
        assert!(said[0].ends_with("no element matches"), "{said:?}");
        if round >= WARM_UP {
            times.push(time);
        }
    }
    let (median, slowest) = figures(&mut times);
    println!(
        "a press to the answer of wait_for_elements waiting: median {median:.3} ms, slowest \
         {slowest:.3} ms, of {TIMED}"
    );
}

/// Times `calls`, taken in turn, `timed_calls` times each after `warm_up` untimed, and
/// prints each figure.
fn measure(session: &mut Session, calls: &[(&str, Value)], timed_calls: usize, warm_up: usize) {
    let mut times = vec![Vec::with_capacity(timed_calls); calls.len()];
    let mut sizes = vec![0; calls.len()];
    for round in 0..warm_up + timed_calls {
        for (at, (name, arguments)) in calls.iter().enumerate() {
            let (time, said) = timed(session, name, arguments);
            if round >= warm_up {
                times[at].push(time);
                sizes[at] = said.iter().map(String::len).sum();
            }
        }
    }
    for (at, (name, _)) in calls.iter().enumerate() {
        let (median, slowest) = figures(&mut times[at]);
        println!(
            "{name}: median {median:.3} ms, slowest {slowest:.3} ms, of {timed_calls}; {} bytes \
             of text",
            sizes[at]
        );
    }
}

/// Stops the program, then the server.
fn stop(mut session: Session, server: &mut std::process::Child) {
    let (said, _) = session.tool("stop_game", json!({}));
    assert!(said[0].contains("ended"), "{said:?}");
    drop(session);
    server.wait().expect("the server ends");
}
