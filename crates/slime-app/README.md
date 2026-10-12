# slime-app

The application lifecycle: starting an engine application, running its main loop, and shutting it
down.

## Status

Partial. What exists today is the diagnostics setup (logging and panic reporting) and the frame
timing types. The main loop and the headless mode arrive in milestone M1.3, windows in M1.4 (see
`docs/PLAN.md`).

## Role

- Starts an application, runs its frames, and shuts it down, with or without a window. Running
  without a window (headless) is a normal configuration, used by automated tests and later by
  dedicated servers.
- Sets up the engine's diagnostics and owns frame timing.

## Entry points

- `init_diagnostics()` installs the log subscriber (output on standard error, filtered by the
  `SLIME_LOG` environment variable, `info` by default) and a panic hook that logs panics, then
  logs the engine version, operating system, CPU architecture and number of logical CPUs. Call
  it once, first thing in `main`; later calls do nothing.
- `LOG_ENV_VAR` is the name of that environment variable.

```rust
slime_app::init_diagnostics();
tracing::info!(answer = 42, "recorded at the default level");
tracing::debug!("recorded only with SLIME_LOG=debug or more verbose");
```

Engine code and games log through the `tracing` macros. The conventions for errors, log levels and
assertions are in `docs/adr/0008-errors-logging-and-assertions.md`.

The `time` module holds the frame timing types: `FrameClock` turns the time between frames into
a `FrameTime` (index, delta, elapsed time) and limits how much time one frame can represent;
`FixedStep` splits frame time into steps of a fixed duration for code that must not depend on
the frame rate. Both are fed durations rather than reading the system clock, so a run can use
simulated, deterministic time. The rules are in `docs/adr/0009-frame-timing.md`.

## Dependencies

- `tracing`: the logging macros.
- `tracing-subscriber`: the subscriber that formats, filters and writes log events.

When windows arrive (milestone M1.4), it also depends on `slime-platform`.

## Must not

- Use the windowing library directly: windows and input go through `slime-platform`.
- Touch the windowing system when it runs headless.
