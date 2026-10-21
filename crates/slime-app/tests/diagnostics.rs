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

    // A stray comma or space, as a script or an environment file leaves, changes nothing.
    for value in ["slime_app=trace,", " debug"] {
        let stray = run_child(
            "modules_without_a_level_of_their_own_record_info",
            Some(value),
        );
        let stray = String::from_utf8_lossy(&stray.stderr);
        assert!(
            stray.contains("line from the game"),
            "SLIME_LOG={value:?} must keep the other modules recording\n{stray}"
        );
    }

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

/// wgpu logs through the `log` crate, not through `tracing`; its records reach the engine's log.
#[test]
fn records_of_the_log_crate_reach_the_log() {
    if is_child() {
        slime_app::init_diagnostics();
        // wgpu logs at `debug` which graphics APIs it could start, whether a GPU opens or not.
        let _ = slime_gfx::Gpu::new();
        return;
    }

    let output = run_child(
        "records_of_the_log_crate_reach_the_log",
        Some("wgpu_core=debug"),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(" wgpu_core::"),
        "wgpu's records must be logged\n{stderr}"
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

#[test]
fn a_failed_run_is_logged_with_every_cause() {
    if is_child() {
        /// A game whose `start` fails with an error that has a cause of its own.
        struct FailingStart;

        #[derive(Debug)]
        struct Missing;

        impl std::fmt::Display for Missing {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("the save file is missing")
            }
        }

        impl std::error::Error for Missing {}

        #[derive(Debug)]
        struct CannotLoad(Missing);

        impl std::fmt::Display for CannotLoad {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("cannot load the game")
            }
        }

        impl std::error::Error for CannotLoad {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        impl slime_app::Simulation for FailingStart {
            fn start(
                &mut self,
                _ctx: &mut slime_app::SimulationContext<'_>,
                _world: &mut slime_world::World,
            ) -> Result<(), slime_app::AppError> {
                Err(Box::new(CannotLoad(Missing)))
            }
        }

        let config = slime_app::AppConfig::default();
        let result = slime_app::run(&mut FailingStart, &mut (), &config);
        assert!(result.is_err());
        return;
    }

    let output = run_child("a_failed_run_is_logged_with_every_cause", None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "ERROR slime_app::app: the run failed: the application failed to start: cannot load \
             the game: the save file is missing"
        ),
        "the failure must be logged with its causes\n{stderr}"
    );
}

#[test]
fn a_trace_of_the_spans_is_written_when_asked_for() {
    const FRAMES: u64 = 3;
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("diagnostics-trace.json");
    if is_child() {
        struct Quiet;
        impl slime_app::Simulation for Quiet {}
        let config = slime_app::AppConfig {
            frame_limit: Some(FRAMES),
            ..slime_app::AppConfig::default()
        };
        slime_app::run(&mut Quiet, &mut (), &config).expect("a headless run succeeds");
        return;
    }

    let _ = std::fs::remove_file(&path);
    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args([
            "--exact",
            "a_trace_of_the_spans_is_written_when_asked_for",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, "1")
        .env_remove(slime_app::LOG_ENV_VAR)
        .env(slime_app::TRACE_ENV_VAR, &path)
        .output()
        .expect("child test process runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let trace = std::fs::read_to_string(&path).expect("the trace is written");
    assert!(trace.starts_with("[\n"), "a JSON array: {trace}");
    // Every frame's span is entered and left once, the log filter at `info` notwithstanding, on a
    // thread that the trace names.
    for phase in ["B", "E"] {
        let events = format!("{{\"name\":\"frame\",\"cat\":\"slime_app::app\",\"ph\":\"{phase}\"");
        assert_eq!(
            trace.matches(&events).count(),
            FRAMES as usize,
            "{phase}: {trace}"
        );
    }
    assert!(trace.contains("\"name\":\"thread_name\""), "{trace}");
}
