//! The errors of a windowed run that need no windowing system to be checked: without one, `run`
//! returns `RunError::Platform` with `PlatformError::Connect`, and a second windowed run in the
//! same process, even after a failed one, returns `PlatformError::SecondEventLoop`.
//!
//! The windowing library creates its event loop on the main thread only, and the test harness
//! runs tests on other threads, so this test has no harness: it is a program that runs itself
//! again as a child process without the variables that name a windowing system (environment
//! variables are only changed safely before a process starts), and checks what the child prints.
//! Linux only: on Windows, a windowed run always finds a windowing system.

use std::process::Command;

use slime_app::{App, AppConfig, RunError};
use slime_platform::{PlatformError, WindowConfig};

const CHILD_ENV: &str = "SLIME_APP_TEST_CHILD";

struct Nothing;

impl App for Nothing {}

fn main() {
    if !cfg!(target_os = "linux") {
        println!("skipped: a windowed run always finds a windowing system on this platform");
        return;
    }
    if std::env::var_os(CHILD_ENV).is_some() {
        return child();
    }

    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .env(CHILD_ENV, "1")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("DISPLAY")
        .env_remove(slime_app::LOG_ENV_VAR)
        .output()
        .expect("child test process runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the child failed: {}\n{stdout}\n{stderr}",
        output.status
    );
    assert_eq!(
        stdout.trim(),
        "first: Connect\nsecond: SecondEventLoop",
        "unexpected errors\n{stderr}"
    );
    println!("test windowed_errors ... ok");
}

/// Attempts two windowed runs without a windowing system and prints the kind of each error.
fn child() {
    let config = AppConfig {
        window: Some(WindowConfig::default()),
        ..AppConfig::default()
    };
    for attempt in ["first", "second"] {
        let error = slime_app::run(&mut Nothing, &config).expect_err("no windowing system");
        let kind = match &error {
            RunError::Platform(PlatformError::Connect(_)) => "Connect",
            RunError::Platform(PlatformError::SecondEventLoop) => "SecondEventLoop",
            other => panic!("unexpected error: {other:?}"),
        };
        println!("{attempt}: {kind}");
    }
}
