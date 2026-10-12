# slime-app

The application lifecycle: starting an engine application, running its main loop, and shutting it
down.

## Status

Applications run headless or with one window that receives keyboard and mouse input; the main
loop, frame timing, exit requests, clean shutdown on Ctrl+C and other termination signals,
profiling spans and diagnostics work. What is missing is listed in `docs/PLAN.md` (known debt).

## Role

- Starts an application, runs its frames, and shuts it down, with or without a window. Running
  without a window (headless) is a normal configuration, used by automated tests and later by
  dedicated servers.
- Sets up the engine's diagnostics and owns frame timing.

## Entry points

A game implements the `App` trait and hands it to `run`:

```rust
use std::time::Duration;
use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, Context};

#[derive(Default)]
struct Game {
    fixed_steps: u32,
}

impl App for Game {
    fn fixed_update(&mut self, _ctx: &mut Context, _step: Duration) {
        self.fixed_steps += 1;
    }

    fn update(&mut self, ctx: &mut Context, frame: &FrameTime) {
        if frame.elapsed() >= Duration::from_secs(1) {
            ctx.request_exit();
        }
    }
}

let mut game = Game::default();
// The default configuration runs headless; set `window` to open one.
slime_app::run(&mut game, &AppConfig::default()).expect("headless runs do not fail");
assert_eq!(game.fixed_steps, 60);
```

- `App`: the methods the engine calls, in this order: `init` once; then for each frame,
  `fixed_update` once per fixed step and `update` once; then `shutdown` once. With a window,
  `on_event` receives each window event between frames. All have empty defaults.
- `run(&mut app, &config)`: sets up diagnostics and a handler for termination signals, then runs
  the application until it calls `Context::request_exit`, the window is closed, the process
  receives SIGINT (Ctrl+C), SIGTERM or SIGHUP, or `AppConfig::frame_limit` is reached. In every
  case the frame in progress finishes and `shutdown` runs; a second signal ends the process at
  once (exit code 130) for an application that no longer finishes its frames
  (`docs/adr/0012-clean-shutdown-on-termination-signals.md`).
  - Headless: frames run back to back with simulated time: each frame represents one fixed step,
    so runs are deterministic. The windowing system is never touched.
  - With a window: the window exists before `init`; frames represent the real time between them,
    on average at most `AppConfig::max_frame_rate` per second (60 by default), and the loop sleeps
    between them. This limiter is temporary: presentation will pace frames once the GPU layer
    exists. `run` must then be called from the main thread.
  - It returns a `slime_platform::PlatformError` when the window cannot be opened, for example
    without a display server. Only one windowed run, or attempt, is possible per process.
  - The signal handler replaces any other handler the process had for these signals, except one
    installed earlier through the `ctrlc` crate, which is kept.
- `AppConfig`: the window (`None`, the default, runs headless), frame limit, fixed step
  (1/60 s), maximum fixed steps per frame (8), maximum frame delta (250 ms) and maximum frame rate
  (60). Write it as a struct literal ending with `..AppConfig::default()`.
- `Context`: what an application can ask from the engine: the window (`Context::window`, `None`
  when headless) and stopping the run.
- `time`: `FrameClock` turns the time between frames into a `FrameTime` (index, delta, elapsed
  time) and limits how much time one frame can represent; `FixedStep` splits frame time into
  steps of a fixed duration. Both take durations rather than reading a clock
  (`docs/adr/0009-frame-timing.md`).
- `init_diagnostics()`: installs the log subscriber (output on standard error, filtered by the
  `SLIME_LOG` environment variable, `info` by default) and a panic hook that logs panics, then
  logs the engine version, operating system, CPU architecture and number of logical CPUs. `run`
  calls it; call it yourself only in a program that does not use `run`. Later calls do nothing.
  `LOG_ENV_VAR` is the name of that environment variable.

Engine code and games log through the `tracing` macros. The conventions for errors, log levels and
assertions are in `docs/adr/0008-errors-logging-and-assertions.md`; why the engine owns the main
loop is in `docs/adr/0010-main-loop-ownership.md`. With `SLIME_LOG=trace`, each frame's profiling
spans (`frame`, `fixed_update`, `update`) are logged with the time spent in them.

## Dependencies

- `ctrlc`: handles termination signals.
- `slime-platform`: the window and its events, for windowed runs.
- `tracing`: the logging macros and profiling spans.
- `tracing-subscriber`: the subscriber that formats, filters and writes log events.

## Must not

- Use the windowing library directly: windows and input go through `slime-platform`.
- Touch the windowing system when it runs headless.
