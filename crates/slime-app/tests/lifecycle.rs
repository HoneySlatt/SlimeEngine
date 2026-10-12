//! Tests of the application lifecycle driven by `slime_app::run` in headless mode. The windowed
//! lifecycle is checked by running the `window` example in a headless compositor
//! (`scripts/check.sh`) and by `windowed_errors.rs`.

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
        if self.exit_in_update_of_frame == Some(frame.index()) {
            ctx.request_exit();
        }
    }

    fn shutdown(&mut self, _ctx: &mut Context) {
        self.calls.push(Call::Shutdown);
    }
}

fn config(frame_limit: Option<u64>) -> AppConfig {
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
