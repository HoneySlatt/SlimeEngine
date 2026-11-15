//! The engine's answer through its MCP server, measured: an agent's call timed from the line that
//! it writes to the line that it reads, through the server, the development channel and the game's
//! frame that takes the request. It starts the `editor` example on a copy of the yard, in a window,
//! and times three calls, each a median and the slowest of 21 after three to warm up, taken in
//! turn: `game_state`, the floor of a call; `find_elements` without a filter, a list of the
//! editor's whole interface gathered from the game's pages; `editor_journal` from its start, a
//! page of its text. It prints each figure, with what the list and the page held. It needs a
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
//! the programs are this build's either way. A measure, not a check (`docs/measurements.md`): run
//! it in release, on a machine at rest. It is not in the default run of
//! `cargo test` (`test = false` in Cargo.toml), and has no harness, so that it prints once.

#[expect(dead_code, reason = "this measure only waits for the window")]
#[path = "common/interface.rs"]
mod interface;
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

use interface::until;
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

    let calls = [
        ("game_state", json!({})),
        ("find_elements", json!({})),
        ("editor_journal", json!({})),
    ];
    measure(&mut session, &calls, TIMED, WARM_UP);
    stop(session, &mut child);
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
