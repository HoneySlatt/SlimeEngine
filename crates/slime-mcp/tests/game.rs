//! The server's game off its lock
//! (`docs/adr/0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md`):
//! no tool waits on the server's lock for a game that starts, stops or is driven. A start answers
//! the other calls while it waits, and is given up by its call's cancel or by `stop_game`, leaving
//! no program behind; `stop_game` ends a game whose every place in flight a long step holds, the
//! game ending on its own, and a start after it runs; the last call of `drive_played_game` wins.
//!
//! The programs are this test's own binary, started by the server's `start_game` with a token
//! (`common/program.rs`): one on the development channel's listener alone, which holds each step
//! of game time, and an editor's played game while a key is held; one that never says where it
//! listens. It needs no window, and runs on Windows too.

#[path = "common/program.rs"]
mod program;
#[path = "common/session.rs"]
#[expect(dead_code, reason = "this session reads no image")]
mod session;

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use session::Session;

/// The test that, run with a token in its environment, is the program.
const PROGRAM: &str = "the_program_of_the_test";

/// The program, in the run that the server starts; nothing in a run of the tests.
#[test]
fn the_program_of_the_test() {
    program::run_if_started();
}

/// The test that, run with a token in its environment, is a program that never says where it
/// listens.
const SILENT: &str = "the_silent_program_of_the_test";

/// The silent program, in the run that the server starts; nothing in a run of the tests.
#[test]
fn the_silent_program_of_the_test() {
    program::stay_silent_if_started();
}

/// How long the test waits for what it asked to be seen.
const SOON: Duration = Duration::from_secs(30);

/// How many requests a connection has in flight at most, the development channel's places. The
/// test that holds them checks that one more is not held, so that a change of the figure fails it.
const PLACES: usize = 16;

/// How long the session waits for the server to end once its input closed.
const END: Duration = Duration::from_secs(30);

/// An agent's session and its server, whose calls go side by side.
struct Calls {
    session: Session,
    server: Child,
}

impl Calls {
    /// Starts the server, as an agent named `client`.
    fn start(client: &str) -> Self {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (session, server, _) = Session::start(&workspace, client);
        Self { session, server }
    }

    /// Calls the tool `name` with `arguments`, without waiting for its answer: its number.
    fn send(&mut self, name: &str, arguments: Value) -> u64 {
        self.session.send_tool(name, arguments)
    }

    /// The result of the call `id`, once it answered, which must not be an error of the protocol.
    fn answer(&mut self, id: u64) -> Value {
        self.session.answer(id)
    }

    /// The result of the tool `name` with `arguments`.
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        let id = self.send(name, arguments);
        self.answer(id)
    }

    /// The text of the tool `name` with `arguments`, which must not fail.
    fn ok(&mut self, name: &str, arguments: Value) -> String {
        let result = self.call(name, arguments);
        assert!(!failed(&result), "{name}: {result}");
        said(&result)
    }

    /// Cancels the call `id`, as its agent gives it up.
    fn cancel(&mut self, id: u64) {
        self.session.notify(
            "notifications/cancelled",
            json!({ "requestId": id, "reason": "the agent gave it up" }),
        );
    }

    /// Whether the call `id` answered, among the messages that the server sent so far.
    fn answered(&mut self, id: u64) -> bool {
        self.session.answered(id)
    }

    /// Ends the session, and checks that the server ends with it, within [`END`].
    fn end(self) {
        let Self {
            session,
            mut server,
            ..
        } = self;
        drop(session);
        let asked = Instant::now();
        let status = loop {
            if let Some(status) = server.try_wait().expect("the server's state") {
                break status;
            }
            if asked.elapsed() > END {
                let _ = server.kill();
                let _ = server.wait();
                panic!("the server did not end within {END:?} once its input closed");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(status.success(), "{status}");
    }
}

/// Whether a tool's `result` is an error.
fn failed(result: &Value) -> bool {
    result["isError"] == json!(true)
}

/// The texts of a tool's `result`, on lines.
fn said(result: &Value) -> String {
    result["content"]
        .as_array()
        .expect("content")
        .iter()
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The command that runs this test's `program`.
fn command(program: &str) -> Value {
    let binary = std::env::current_exe().expect("the test's program");
    json!([
        binary,
        "--exact",
        program,
        "--nocapture",
        "--test-threads=1"
    ])
}

/// Waits, within [`SOON`], until `what` holds.
fn until(what: &str, mut holds: impl FnMut() -> bool) {
    let started = Instant::now();
    while !holds() {
        assert!(started.elapsed() < SOON, "{what}: not within {SOON:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Where the silent program holds a socket while it runs, once it wrote it; its file is removed,
/// for the next start's.
fn silent_socket(file: &Path) -> SocketAddr {
    let mut port = None;
    until("the silent program runs", || {
        port = std::fs::read_to_string(file).ok();
        port.is_some()
    });
    std::fs::remove_file(file).expect("the port's file removed");
    let port: u16 = port.expect("read").trim().parse().expect("a port");
    SocketAddr::from(([127, 0, 0, 1], port))
}

/// Whether a program holds the socket at `address`.
fn holds(address: SocketAddr) -> bool {
    TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_ok()
}

/// The silent program's file of its port, in the data directory of the session of `client`.
fn port_file(client: &str) -> PathBuf {
    session::data(client).join(program::SILENT_PORT)
}

/// While `start_game` waits for a program that never says where it listens, the server answers its
/// other calls at once: another start is refused, and a tool that asks the game says that it
/// starts; the start's cancel, then `stop_game` during another start, give the start up, each
/// killing the program that it started, and leave no game; a start then runs. A start given up
/// after its program said where it listens is not tested: it needs a program that welcomes late,
/// which the channel's listener cannot be (`docs/debt.md`).
#[test]
fn a_start_waits_off_the_lock_and_given_up_leaves_no_game() {
    let client = "starting";
    let file = port_file(client);
    let mut calls = Calls::start(client);
    let start = json!({ "command": command(SILENT), "timeout_seconds": 600 });

    let cancelled = calls.send("start_game", start.clone());
    let silent = silent_socket(&file);
    let asked = Instant::now();
    let again = calls.call("start_game", start.clone());
    assert!(failed(&again), "{again}");
    assert!(said(&again).contains("a game starts already"), "{again}");
    let read = calls.call("read_logs", json!({}));
    assert!(failed(&read), "{read}");
    assert!(said(&read).contains("the game starts"), "{read}");
    let listing = calls.session.send("tools/list", json!({}));
    calls.answer(listing);
    let answered = asked.elapsed();
    assert!(
        answered < Duration::from_secs(2),
        "the second start, the read and the list of tools waited {answered:?}"
    );

    // Cancelled: the program goes, and MCP answers no cancelled call.
    calls.cancel(cancelled);
    until("the cancelled start's program ends", || !holds(silent));
    let state = calls.call("game_state", json!({}));
    assert!(said(&state).contains("no game runs"), "{state}");

    // Given up by `stop_game`, which says so; the start answers so.
    let starting = calls.send("start_game", start);
    let silent = silent_socket(&file);
    let stopped = calls.ok("stop_game", json!({}));
    assert!(stopped.contains("start is given up"), "{stopped}");
    let given_up = calls.answer(starting);
    assert!(failed(&given_up), "{given_up}");
    assert!(said(&given_up).contains("was given up"), "{given_up}");
    until("the stopped start's program ends", || !holds(silent));

    let started = calls.ok(
        "start_game",
        json!({ "command": command(PROGRAM), "timeout_seconds": 60 }),
    );
    assert!(started.contains("the tests' program runs"), "{started}");
    let stopped = calls.ok("stop_game", json!({}));
    assert!(stopped.starts_with("the game ended"), "{stopped}");
    assert!(!calls.answered(cancelled), "a cancelled call answered");
    calls.end();
}

/// `stop_game` ends a game whose every place in flight a request to run steps holds, which it never
/// runs, and one more waits for a place, well within the time that it gives a game before killing
/// it: the game ends on its own, as its standard input closes, and each request is told that the
/// game stops, not connected to again. A start after it runs.
#[test]
fn stop_game_ends_a_game_whose_places_are_all_held_and_a_start_after_it_runs() {
    let mut calls = Calls::start("stopping");
    let start = json!({ "command": command(PROGRAM), "timeout_seconds": 60 });
    let started = calls.ok("start_game", start.clone());
    let (_, log) = started.rsplit_once("its log in ").expect("its log");
    let log = PathBuf::from(log.trim());

    let steps: Vec<u64> = (0..=PLACES)
        .map(|_| calls.send("step", json!({ "steps": 1 })))
        .collect();
    let every = format!("{}{PLACES}\n", program::STEPS_HELD);
    until("every place held by a step", || {
        std::fs::read_to_string(&log).is_ok_and(|text| text.contains(&every))
    });
    // The last one waits for a place: had the connection more, the program would hold it.
    std::thread::sleep(Duration::from_millis(200));
    let more = format!("{}{}\n", program::STEPS_HELD, PLACES + 1);
    let log_text = std::fs::read_to_string(&log).expect("the log");
    assert!(!log_text.contains(&more), "more places than {PLACES}");

    let asked = Instant::now();
    let stopped = calls.ok("stop_game", json!({}));
    let took = asked.elapsed();
    assert!(
        stopped.starts_with("the game ended") && !stopped.contains("killed"),
        "{stopped}"
    );
    assert!(took < SOON, "`stop_game` took {took:?}");
    for step in steps {
        let held = calls.answer(step);
        assert!(failed(&held), "{held}");
        assert!(said(&held).contains("not connected to again"), "{held}");
    }

    let started = calls.ok("start_game", start);
    assert!(started.contains("the tests' program runs"), "{started}");
    let seen = calls.ok("read_report", json!({ "name": "seen" }));
    assert!(seen.starts_with("applied: []"), "{seen}");
    calls.ok("stop_game", json!({}));
    calls.end();
}

/// The last call of `drive_played_game` wins: one whose request the test's program holds while a
/// key is down does not take the place of a later one, which answers meanwhile, nor does one
/// answered before a later one for the same game; a call alone points the game's tools at the
/// played game, and back.
#[test]
fn the_last_call_of_drive_played_game_wins() {
    let mut calls = Calls::start("driving");
    let started = calls.ok(
        "start_game",
        json!({ "command": command(PROGRAM), "timeout_seconds": 60 }),
    );
    assert!(started.contains("the tests' program runs"), "{started}");
    let seen = |calls: &mut Calls| calls.ok("read_report", json!({ "name": "seen" }));

    calls.ok("press_key", json!({ "key": "KeyW" }));
    let first = calls.send("drive_played_game", json!({ "played": true }));
    until("the program holds the first call's request", || {
        seen(&mut calls).contains("held: [Edit { edit: Played }]")
    });
    let later = calls.ok("drive_played_game", json!({ "played": false }));
    assert!(later.contains("drive the editor again"), "{later}");
    calls.ok("release_key", json!({ "key": "KeyW" }));
    let lost = calls.answer(first);
    assert!(failed(&lost), "{lost}");
    assert!(said(&lost).contains("a later call"), "{lost}");
    assert!(seen(&mut calls).starts_with("applied: []"), "the editor");

    let driven = calls.ok("drive_played_game", json!({ "played": true }));
    assert!(
        driven.contains("drive the game that the editor plays"),
        "{driven}"
    );
    assert_eq!(seen(&mut calls), program::PLAYED);
    calls.ok("drive_played_game", json!({ "played": false }));
    assert!(seen(&mut calls).starts_with("applied: []"), "the editor");

    // Two calls for the played game, both held, answered in their order: the first, answered
    // first, still loses.
    calls.ok("press_key", json!({ "key": "KeyW" }));
    let first = calls.send("drive_played_game", json!({ "played": true }));
    let second = calls.send("drive_played_game", json!({ "played": true }));
    until("the program holds both calls' requests", || {
        seen(&mut calls).contains("held: [Edit { edit: Played }, Edit { edit: Played }]")
    });
    calls.ok("release_key", json!({ "key": "KeyW" }));
    let lost = calls.answer(first);
    assert!(said(&lost).contains("a later call"), "{lost}");
    let won = calls.answer(second);
    assert!(!failed(&won), "{won}");
    assert_eq!(seen(&mut calls), program::PLAYED);
    calls.ok("stop_game", json!({}));
    calls.end();
}
