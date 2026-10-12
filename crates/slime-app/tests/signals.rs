//! Tests of the clean shutdown on termination signals.
//!
//! Signal handlers are global to a process, so each test re-runs this test binary as a child
//! process, restricted to that one test. The child runs a headless application that prints
//! `ready` when it starts and `shutdown` when it stops; the parent sends it signals with the
//! `kill` command and checks how it ended.
//!
//! Unix only: Windows has no signals to send, and a console Ctrl+C cannot be sent to a single
//! child process from a test. On Windows, Ctrl+C is checked by hand.
#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context, run};

const CHILD_ENV: &str = "SLIME_APP_TEST_CHILD";
const TIMEOUT: Duration = Duration::from_secs(10);

/// Prints its lifecycle on standard output; optionally blocks during the first frame.
struct Announcer {
    block_first_frame: bool,
}

impl App for Announcer {
    fn init(&mut self, _ctx: &mut Context) -> Result<(), AppError> {
        println!("ready");
        Ok(())
    }

    fn update(&mut self, _ctx: &mut Context, frame: &FrameTime) {
        if self.block_first_frame && frame.index() == 0 {
            thread::sleep(Duration::from_secs(30));
        }
    }

    fn shutdown(&mut self, _ctx: &mut Context) {
        println!("shutdown");
    }
}

fn is_child() -> bool {
    std::env::var_os(CHILD_ENV).is_some()
}

fn run_child_app(block_first_frame: bool) {
    let mut app = Announcer { block_first_frame };
    run(&mut app, &AppConfig::default()).expect("this application starts");
}

/// Starts the test named `test` in a child process and waits until its application is running.
/// Returns the child and the lines it prints on standard output from then on.
fn spawn_child(test: &str) -> (Child, Receiver<String>) {
    let mut child = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env_remove(slime_app::LOG_ENV_VAR)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("child test process starts");
    let stdout = child.stdout.take().expect("piped stdout");
    let (sender, lines) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });

    // The test harness prints "test <name> ... " without a line break, so "ready" ends a line.
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) if line.ends_with("ready") => return (child, lines),
            Ok(_) => {}
            Err(_) => {
                let _ = child.kill();
                panic!("the child's application did not start within {TIMEOUT:?}");
            }
        }
    }
}

fn send_signal(child: &Child, signal: &str) {
    let status = Command::new("kill")
        .args([&format!("-{signal}"), &child.id().to_string()])
        .status()
        .expect("the kill command runs");
    assert!(status.success(), "kill -{signal} failed");
}

/// Waits for the child to end, then returns its exit status and what it printed after `ready`.
/// Kills it and fails the test if it takes too long.
fn wait_for_end(mut child: Child, lines: &Receiver<String>) -> (ExitStatus, String) {
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll the child") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("the child did not stop within {TIMEOUT:?}");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = lines.iter().collect::<Vec<_>>().join("\n");
    (status, output)
}

fn assert_clean_stop_on(signal: &str, test: &str) {
    let (child, lines) = spawn_child(test);
    send_signal(&child, signal);
    let (status, output) = wait_for_end(child, &lines);
    assert!(
        status.success(),
        "SIG{signal} must end the run cleanly: {status}"
    );
    assert!(
        output.contains("shutdown"),
        "App::shutdown must run after SIG{signal}; output: {output}"
    );
}

#[test]
fn sigint_stops_the_run_cleanly() {
    if is_child() {
        return run_child_app(false);
    }
    assert_clean_stop_on("INT", "sigint_stops_the_run_cleanly");
}

#[test]
fn sigterm_stops_the_run_cleanly() {
    if is_child() {
        return run_child_app(false);
    }
    assert_clean_stop_on("TERM", "sigterm_stops_the_run_cleanly");
}

#[test]
fn a_second_signal_ends_a_stuck_process_at_once() {
    if is_child() {
        return run_child_app(true);
    }
    let (child, lines) = spawn_child("a_second_signal_ends_a_stuck_process_at_once");
    send_signal(&child, "INT");
    thread::sleep(Duration::from_millis(200));
    send_signal(&child, "INT");
    let (status, output) = wait_for_end(child, &lines);
    assert_eq!(
        status.code(),
        Some(130),
        "a second signal must exit with code 130"
    );
    assert!(
        !output.contains("shutdown"),
        "a forced exit does not run App::shutdown; output: {output}"
    );
}
