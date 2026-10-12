//! Tests of `init_diagnostics`.
//!
//! The log subscriber and the panic hook are global to a process, so each test re-runs this test
//! binary as a child process, restricted to that one test, and checks what the child wrote to
//! standard error. The child knows it is the child because `CHILD_ENV` is set.

use std::process::{Command, Output, Stdio};

const CHILD_ENV: &str = "SLIME_APP_TEST_CHILD";

fn is_child() -> bool {
    std::env::var_os(CHILD_ENV).is_some()
}

/// Runs the test named `test` in a child process, with `SLIME_LOG` set to `log_filter` (or unset).
fn run_child(test: &str, log_filter: Option<&str>) -> Output {
    let mut command = Command::new(std::env::current_exe().expect("test binary path"));
    command
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env_remove(slime_app::LOG_ENV_VAR)
        .env_remove("RUST_BACKTRACE");
    if let Some(filter) = log_filter {
        command.env(slime_app::LOG_ENV_VAR, filter);
    }
    command.output().expect("child test process runs")
}

#[test]
fn panic_is_logged_before_the_default_hook_runs() {
    if is_child() {
        slime_app::init_diagnostics();
        panic!("deliberate test panic");
    }

    let output = run_child("panic_is_logged_before_the_default_hook_runs", None);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "the child must fail\n{stderr}");
    let logged = stderr
        .find("ERROR slime_app::diagnostics: panic: deliberate test panic")
        .unwrap_or_else(|| panic!("panic not logged\n{stderr}"));
    let default_hook = stderr
        .find("panicked at")
        .unwrap_or_else(|| panic!("default hook output missing\n{stderr}"));
    assert!(
        logged < default_hook,
        "the log line must come first\n{stderr}"
    );
}

#[test]
fn startup_line_follows_the_log_filter() {
    if is_child() {
        slime_app::init_diagnostics();
        return;
    }

    let default = run_child("startup_line_follows_the_log_filter", None);
    let default = String::from_utf8_lossy(&default.stderr);
    assert!(
        default.contains("INFO slime_app::diagnostics: SlimeEngine starting"),
        "startup line missing at the default level\n{default}"
    );

    let quiet = run_child("startup_line_follows_the_log_filter", Some("warn"));
    let quiet = String::from_utf8_lossy(&quiet.stderr);
    assert!(
        !quiet.contains("SlimeEngine starting"),
        "SLIME_LOG=warn must hide the startup line\n{quiet}"
    );
}

#[test]
fn second_call_installs_nothing_more() {
    if is_child() {
        slime_app::init_diagnostics();
        slime_app::init_diagnostics();
        panic!("deliberate test panic");
    }

    let output = run_child("second_call_installs_nothing_more", None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("SlimeEngine starting").count(),
        1,
        "startup line must appear once\n{stderr}"
    );
    assert_eq!(
        stderr.matches("panic: deliberate test panic").count(),
        1,
        "the panic must be logged once, so the hook is installed once\n{stderr}"
    );
}

/// The log output can disappear while the engine runs: a pipe whose reader exited, a terminal
/// that closed. Writing to it then fails, which must not end the process.
#[test]
fn logging_to_a_broken_pipe_does_not_end_the_process() {
    if is_child() {
        slime_app::init_diagnostics();
        // Enough events to reach the pipe after the parent has closed its end.
        for index in 0..10_000 {
            tracing::info!(index, "line");
        }
        return;
    }

    let mut command = Command::new(std::env::current_exe().expect("test binary path"));
    command
        .args([
            "--exact",
            "logging_to_a_broken_pipe_does_not_end_the_process",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, "1")
        .env(slime_app::LOG_ENV_VAR, "info")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("child test process starts");
    // Closing the reading end makes every write of the child fail.
    drop(child.stderr.take());
    let status = child.wait().expect("the child ends");
    assert!(
        status.success(),
        "a broken log pipe must not end the process: {status}"
    );
}

#[test]
fn modules_without_a_level_of_their_own_record_info() {
    if is_child() {
        slime_app::init_diagnostics();
        tracing::info!("line from the game");
        return;
    }

    let engine_only = run_child(
        "modules_without_a_level_of_their_own_record_info",
        Some("slime_app=trace"),
    );
    let engine_only = String::from_utf8_lossy(&engine_only.stderr);
    assert!(
        engine_only.contains("line from the game"),
        "a value without a bare level must keep the other modules at info\n{engine_only}"
    );

    let quiet = run_child(
        "modules_without_a_level_of_their_own_record_info",
        Some("warn,slime_app=trace"),
    );
    let quiet = String::from_utf8_lossy(&quiet.stderr);
    assert!(
        !quiet.contains("line from the game"),
        "a bare level in the value applies to the other modules\n{quiet}"
    );
}

#[test]
fn a_panic_payload_that_is_not_a_string_is_logged() {
    if is_child() {
        slime_app::init_diagnostics();
        std::panic::panic_any(42_u32);
    }

    let output = run_child("a_panic_payload_that_is_not_a_string_is_logged", None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("panic: <panic payload is not a string>"),
        "the payload's absence must be logged\n{stderr}"
    );
}
