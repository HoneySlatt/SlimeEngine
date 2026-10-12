# slime-app

The application lifecycle: starting an engine application, running its main loop, and shutting it
down.

## Status

Partial. Applications run headless (without a window): the main loop, frame timing, exit
requests, profiling spans and diagnostics work. Windows arrive in milestone M1.4 (see
`docs/PLAN.md`).

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
slime_app::run(&mut game, &AppConfig::default());
assert_eq!(game.fixed_steps, 60);
```

- `App`: the methods the engine calls, in this order: `init` once; then for each frame,
  `fixed_update` once per fixed step and `update` once; then `shutdown` once. All have empty
  defaults.
- `run(&mut app, &config)`: sets up diagnostics, then runs the application until it calls
  `Context::request_exit` or `AppConfig::frame_limit` is reached. Headless, frames run back to
  back with simulated time: each frame represents one fixed step, so runs are deterministic.
- `AppConfig`: frame limit, fixed step (1/60 s), maximum fixed steps per frame (8) and maximum
  frame delta (250 ms). Write it as a struct literal ending with `..AppConfig::default()`.
- `Context`: what an application can ask from the engine; today, to stop.
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

- `tracing`: the logging macros and profiling spans.
- `tracing-subscriber`: the subscriber that formats, filters and writes log events.

When windows arrive (milestone M1.4), it also depends on `slime-platform`.

## Must not

- Use the windowing library directly: windows and input go through `slime-platform`.
- Touch the windowing system when it runs headless.
