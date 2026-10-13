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

/// Prints its lifecycle on standard output; optionally blocks during the first frame, by sleeping
/// or by logging without end, or sleeps in every frame so that a run lasts long enough to receive a
/// signal.
#[derive(Default)]
struct Announcer {
    block_first_frame: bool,
    log_in_first_frame: bool,
    sleep_per_frame: Duration,
    frames: u64,
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
        if self.log_in_first_frame && frame.index() == 0 {
            // Blocks once standard error is full, when nobody reads it.
            loop {
                tracing::info!("a line of the first frame");
            }
        }
        thread::sleep(self.sleep_per_frame);
        self.frames += 1;
    }

    fn shutdown(&mut self, _ctx: &mut Context) {
        println!("shutdown frames={}", self.frames);
    }
}

fn is_child() -> bool {
    std::env::var_os(CHILD_ENV).is_some()
}

fn run_child_app(block_first_frame: bool) {
    let mut app = Announcer {
        block_first_frame,
        ..Announcer::default()
    };
    run(&mut app, &AppConfig::default()).expect("this application starts");
}

/// Starts the test named `test` in a child process and waits until its application is running.
/// Returns the child and the lines it prints on standard output from then on. With
/// `sigint_ignored`, the child inherits SIGINT set to "ignore", as a process started in the
/// background by a shell script does. The child's standard error goes to `stderr`; a pipe is kept
/// open, unread, until the child is waited for.
fn spawn_child(test: &str, sigint_ignored: bool, stderr: Stdio) -> (Child, Receiver<String>) {
    let exe = std::env::current_exe().expect("test binary path");
    let arguments = ["--exact", test, "--nocapture", "--test-threads=1"];
    let mut command = if sigint_ignored {
        let mut command = Command::new("sh");
        command
            .args(["-c", r#"trap '' INT; exec "$0" "$@""#])
            .arg(exe)
            .args(arguments);
        command
    } else {
        let mut command = Command::new(exe);
        command.args(arguments);
        command
    };
    let mut child = command
        .env(CHILD_ENV, "1")
        .env_remove(slime_app::LOG_ENV_VAR)
        .stdout(Stdio::piped())
        .stderr(stderr)
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

fn assert_clean_stop_on(signal: &str, test: &str, sigint_ignored: bool) {
    let (child, lines) = spawn_child(test, sigint_ignored, Stdio::null());
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
    assert_clean_stop_on("INT", "sigint_stops_the_run_cleanly", false);
}

#[test]
fn sigterm_stops_the_run_cleanly() {
    if is_child() {
        return run_child_app(false);
    }
    assert_clean_stop_on("TERM", "sigterm_stops_the_run_cleanly", false);
}

/// A process started in the background by a shell script inherits SIGINT set to "ignore"; the
/// engine's handler replaces it (ADR-0012).
#[test]
fn an_inherited_ignore_of_sigint_is_lifted() {
    if is_child() {
        return run_child_app(false);
    }
    assert_clean_stop_on("INT", "an_inherited_ignore_of_sigint_is_lifted", true);
}

#[test]
fn a_second_signal_ends_a_stuck_process_at_once() {
    if is_child() {
        return run_child_app(true);
    }
    let (child, lines) = spawn_child(
        "a_second_signal_ends_a_stuck_process_at_once",
        false,
        Stdio::null(),
    );
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

/// The log can be what blocks a stuck process: its standard error is a full pipe that nobody reads,
/// and a frame waits to write a line. The first signal's own log line then waits as well, which
/// must not keep the second signal from ending the process.
#[test]
fn a_second_signal_ends_a_process_whose_log_is_blocked() {
    if is_child() {
        let mut app = Announcer {
            log_in_first_frame: true,
            ..Announcer::default()
        };
        run(&mut app, &AppConfig::default()).expect("this application starts");
        return;
    }
    let (child, lines) = spawn_child(
        "a_second_signal_ends_a_process_whose_log_is_blocked",
        false,
        Stdio::piped(),
    );
    // Leaves the first frame time to fill the pipe.
    thread::sleep(Duration::from_millis(200));
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
        "the frames were blocked, so App::shutdown cannot have run; output: {output}"
    );
}

/// A signal asks the whole process to stop: a later run stops before its first frame, after its
/// `init` and with its `shutdown`.
#[test]
fn a_signal_stops_every_later_run_in_the_process() {
    if is_child() {
        run_child_app(false);
        let mut second = Announcer::default();
        let config = AppConfig {
            frame_limit: Some(100),
            ..AppConfig::default()
        };
        run(&mut second, &config).expect("the second run starts");
        println!("second run frames={}", second.frames);
        return;
    }
    let (child, lines) = spawn_child(
        "a_signal_stops_every_later_run_in_the_process",
        false,
        Stdio::null(),
    );
    send_signal(&child, "INT");
    let (status, output) = wait_for_end(child, &lines);
    assert!(status.success(), "{status}");
    assert!(
        output.contains("ready\nshutdown frames=0\nsecond run frames=0"),
        "a later run must run init and shutdown but no frame; output: {output}"
    );
}

/// A handler installed through the `ctrlc` crate before `run`, by the game, is kept: the signal
/// goes to it and does not stop the run.
#[test]
fn a_handler_installed_through_ctrlc_before_run_is_kept() {
    if is_child() {
        ctrlc::set_handler(|| println!("game handler")).expect("the game installs its handler");
        let mut app = Announcer {
            sleep_per_frame: Duration::from_millis(50),
            ..Announcer::default()
        };
        let config = AppConfig {
            frame_limit: Some(40),
            ..AppConfig::default()
        };
        run(&mut app, &config).expect("this application starts");
        return;
    }
    let (child, lines) = spawn_child(
        "a_handler_installed_through_ctrlc_before_run_is_kept",
        false,
        Stdio::null(),
    );
    send_signal(&child, "INT");
    let (status, output) = wait_for_end(child, &lines);
    assert!(status.success(), "{status}");
    assert!(
        output.contains("game handler"),
        "the game's handler receives the signal; output: {output}"
    );
    assert!(
        output.contains("shutdown frames=40"),
        "the run goes on to its frame limit; output: {output}"
    );
}
