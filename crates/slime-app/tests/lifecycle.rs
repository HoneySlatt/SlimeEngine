//! Tests of the application lifecycle driven by `slime_app::run` in headless mode. The windowed
//! lifecycle is checked by `windowed_lifecycle.rs`, which `scripts/check.sh` runs in a headless
//! compositor (ADR-0024), and the errors of a windowed run by `windowed_errors.rs`.

use std::time::Duration;

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context, RunError, run};

const STEP: Duration = Duration::from_millis(10);

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Init,
    FixedUpdate(Duration),
    Update {
        index: u64,
        delta: Duration,
        elapsed: Duration,
    },
    Shutdown,
}

/// Records every call it receives, and requests an exit or fails where it is told to.
#[derive(Default)]
struct Recorder {
    calls: Vec<Call>,
    exit_in_init: bool,
    fail_in_init: bool,
    exit_in_update_of_frame: Option<u64>,
    fail_in_update_of_frame: Option<u64>,
    fail_in_shutdown: bool,
    /// `Context::fixed_step_alpha` as each `update` reads it.
    alphas: Vec<f64>,
}

impl App for Recorder {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        self.calls.push(Call::Init);
        if self.fail_in_init {
            return Err("no save file".into());
        }
        if self.exit_in_init {
            ctx.request_exit();
        }
        Ok(())
    }

    fn fixed_update(&mut self, _ctx: &mut Context, step: Duration) {
        self.calls.push(Call::FixedUpdate(step));
    }

    fn update(&mut self, ctx: &mut Context, frame: &FrameTime) {
        self.calls.push(Call::Update {
            index: frame.index(),
            delta: frame.delta(),
            elapsed: frame.elapsed(),
        });
        self.alphas.push(ctx.fixed_step_alpha());
        if self.exit_in_update_of_frame == Some(frame.index()) {
            ctx.request_exit();
        }
        if self.fail_in_update_of_frame == Some(frame.index()) {
            ctx.exit_with_error("the save file is corrupt");
            // Only the first error is returned.
            ctx.exit_with_error("the second error");
        }
    }

    fn shutdown(&mut self, ctx: &mut Context) {
        self.calls.push(Call::Shutdown);
        if self.fail_in_shutdown {
            ctx.exit_with_error("cannot write the save file");
        }
    }
}

/// Sends the engine's log to the test harness, which shows it only for a failing test, instead of
/// the real standard error, which a `SLIME_LOG=trace` left in the shell would flood. `run` keeps
/// the subscriber that exists.
fn quiet_logs() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
}

fn config(frame_limit: Option<u64>) -> AppConfig {
    quiet_logs();
    AppConfig {
        frame_limit,
        fixed_step: STEP,
        ..AppConfig::default()
    }
}

fn update(index: u64) -> Call {
    Call::Update {
        index,
        delta: STEP,
        elapsed: STEP * (index as u32 + 1),
    }
}

#[test]
fn runs_init_then_one_fixed_step_and_one_update_per_frame_then_shutdown() {
    let mut app = Recorder::default();
    run(&mut app, &config(Some(3))).expect("this application starts");
    assert_eq!(
        app.calls,
        [
            Call::Init,
            Call::FixedUpdate(STEP),
            update(0),
            Call::FixedUpdate(STEP),
            update(1),
            Call::FixedUpdate(STEP),
            update(2),
            Call::Shutdown,
        ]
    );
}

#[test]
fn an_exit_request_lets_the_current_frame_finish_and_runs_no_other() {
    let mut app = Recorder {
        exit_in_update_of_frame: Some(1),
        ..Recorder::default()
    };
    run(&mut app, &config(None)).expect("this application starts");
    assert_eq!(
        app.calls,
        [
            Call::Init,
            Call::FixedUpdate(STEP),
            update(0),
            Call::FixedUpdate(STEP),
            update(1),
            Call::Shutdown,
        ]
    );
}

#[test]
fn an_exit_requested_during_init_runs_no_frame() {
    let mut app = Recorder {
        exit_in_init: true,
        ..Recorder::default()
    };
    run(&mut app, &config(None)).expect("this application starts");
    assert_eq!(app.calls, [Call::Init, Call::Shutdown]);
}

#[test]
fn an_init_error_stops_the_run_without_frames_or_shutdown() {
    let mut app = Recorder {
        fail_in_init: true,
        ..Recorder::default()
    };
    let error = run(&mut app, &config(None)).expect_err("the init error is returned");
    assert!(
        matches!(error, RunError::Init(_)),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        std::error::Error::source(&error).map(ToString::to_string),
        Some("no save file".to_owned())
    );
    assert_eq!(app.calls, [Call::Init]);
}

#[test]
fn a_frame_limit_of_zero_runs_no_frame() {
    let mut app = Recorder::default();
    run(&mut app, &config(Some(0))).expect("this application starts");
    assert_eq!(app.calls, [Call::Init, Call::Shutdown]);
}

#[test]
fn headless_time_is_simulated_and_identical_between_runs() {
    let mut first = Recorder::default();
    let mut second = Recorder::default();
    run(&mut first, &config(Some(600))).expect("this application starts");
    run(&mut second, &config(Some(600))).expect("this application starts");

    assert_eq!(first.calls, second.calls);
    assert_eq!(first.calls.len(), 1 + 600 * 2 + 1);
    assert_eq!(first.calls[first.calls.len() - 2], update(599));
}

#[test]
fn update_reads_the_fraction_of_a_fixed_step_left_to_run() {
    let mut app = Recorder::default();
    // Frames limited to 4 ms of a 10 ms step run a step every two or three frames.
    let limited = AppConfig {
        max_frame_delta: Duration::from_millis(4),
        ..config(Some(5))
    };
    run(&mut app, &limited).expect("this application starts");
    assert_eq!(app.alphas, [0.4, 0.8, 0.2, 0.6, 0.0]);

    let mut app = Recorder::default();
    run(&mut app, &config(Some(3))).expect("this application starts");
    assert_eq!(app.alphas, [0.0, 0.0, 0.0]);
}

/// The run's error, and the message of its source.
fn app_error(result: Result<(), RunError>) -> String {
    let error = result.expect_err("the application's error is returned");
    assert!(
        matches!(error, RunError::App(_)),
        "unexpected error: {error:?}"
    );
    std::error::Error::source(&error)
        .expect("the application's error is the source")
        .to_string()
}

#[test]
fn an_error_in_a_frame_lets_it_finish_runs_shutdown_and_is_returned() {
    let mut app = Recorder {
        fail_in_update_of_frame: Some(1),
        ..Recorder::default()
    };
    let result = run(&mut app, &config(None));
    assert_eq!(app_error(result), "the save file is corrupt");
    assert_eq!(
        app.calls,
        [
            Call::Init,
            Call::FixedUpdate(STEP),
            update(0),
            Call::FixedUpdate(STEP),
            update(1),
            Call::Shutdown,
        ]
    );
}

#[test]
fn an_error_in_shutdown_fails_a_run_that_ended_well() {
    let mut app = Recorder {
        fail_in_shutdown: true,
        ..Recorder::default()
    };
    let result = run(&mut app, &config(Some(1)));
    assert_eq!(app_error(result), "cannot write the save file");
}
