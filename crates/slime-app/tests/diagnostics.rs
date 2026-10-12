//! Tests of `init_diagnostics`.
//!
//! The log subscriber and the panic hook are global to a process, so each test re-runs this test
//! binary as a child process, restricted to that one test, and checks what the child wrote to
//! standard error. The child knows it is the child because `CHILD_ENV` is set.

use std::process::{Command, Output};

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
