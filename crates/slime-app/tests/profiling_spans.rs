//! The profiling spans of a headless run, which `slime_app::run` documents: each frame is a `frame`
//! span, containing a `fixed_update` span around each fixed step and an `update` span around the
//! update. The spans of a windowed run (`acquire`, `render` with `submit`, `present`) are checked
//! by `windowed_lifecycle.rs`.
//!
//! The spans are recorded by a subscriber set for the test's thread, on which a headless run calls
//! the application. It is a test program of its own, so that no other test's run shares the
//! process's subscribers with it.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, Context, run};

/// Adds a line to the log at each call, among those of the spans.
struct Recorder(Arc<Mutex<Vec<String>>>);

impl Recorder {
    fn log(&self, line: &str) {
        self.0.lock().expect("the log").push(line.to_owned());
    }
}

impl App for Recorder {
    fn fixed_update(&mut self, _ctx: &mut Context, _step: Duration) {
        self.log("call fixed_update");
    }

    fn update(&mut self, _ctx: &mut Context, _frame: &FrameTime) {
        self.log("call update");
    }
}

/// A span whose guard is dropped too early is entered and exited before the call it should
/// contain, and a span moved out of `frame` loses `frame/` from its path: both change the log.
#[test]
fn each_frame_is_a_span_containing_its_fixed_steps_and_its_update() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let subscriber = common::span_log(move |line| sink.lock().expect("the log").push(line));
    let config = AppConfig {
        frame_limit: Some(2),
        ..AppConfig::default()
    };
    tracing::subscriber::with_default(subscriber, || {
        run(&mut Recorder(Arc::clone(&log)), &config).expect("this application starts");
    });

    // A headless frame represents exactly one fixed step, so it runs exactly one.
    let frame = [
        "enter frame",
        "enter frame/fixed_update",
        "call fixed_update",
        "exit frame/fixed_update",
        "enter frame/update",
        "call update",
        "exit frame/update",
        "exit frame",
    ];
    assert_eq!(*log.lock().expect("the log"), [frame, frame].concat());
}
