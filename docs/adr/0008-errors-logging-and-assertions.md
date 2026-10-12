# 0008. Errors, logging and assertions

- Status: Accepted
- Date: 2026-10-12

## Context

Every crate needs to report failures, record what it does, and check its own invariants. If each
crate picks its own way, callers cannot handle errors uniformly, logs from different crates do
not look alike or cannot be filtered together, and panics go unnoticed in log files. The
roadmap also asks for profiling spans from the first system onwards (principle 6), which is a
form of instrumentation close to logging.

## Decision

**Errors.**

- A failure that depends on the environment (no display server, a file that cannot be read, a
  device that refuses a request) is returned as an error. A violated invariant, which can only
  come from a bug, panics.
- Each library crate defines its own error types: enums, so that callers can match on what went
  wrong, marked `#[non_exhaustive]` when new cases are expected. Their `Display` and `Error`
  implementations are derived with [`thiserror`](https://crates.io/crates/thiserror), added with
  the first error type that needs it.
- A dependency's error type never appears in a public error type. It is kept as the error's
  `source()`, boxed as `Box<dyn std::error::Error + Send + Sync>`, so that the cause is still
  printed and the dependency can be replaced.
- Engine crates do not use dynamic error crates such as `anyhow`. Examples and tools may, or may
  return `Box<dyn Error>`.

**Logging.**

- All crates log through the [`tracing`](https://crates.io/crates/tracing) macros (`error!`,
  `warn!`, `info!`, `debug!`, `trace!`) and record profiling spans with the same crate.
- Only `slime-app` installs the log subscriber, in `init_diagnostics`, using
  [`tracing-subscriber`](https://crates.io/crates/tracing-subscriber). It writes to standard
  error; the `SLIME_LOG` environment variable selects what is recorded (default `info`). A game
  that installs its own subscriber first keeps it.
- Levels: `error` when something failed and the requested work did not happen; `warn` for
  something unexpected that was handled; `info` for a few lifecycle events (start, stop, a window
  created); `debug` for details useful when investigating; `trace` for per-frame or per-event
  detail.
- Values that matter are recorded as structured fields (`info!(width, height, "resized")`), not
  only formatted into the message.

**Assertions and panics.**

- `assert!` checks invariants that are cheap enough to keep in release builds; `debug_assert!`
  checks the ones on hot paths. There are no engine-specific assertion macros until a need
  appears that the standard ones cannot meet.
- `init_diagnostics` installs a panic hook that logs the panic (message, location, thread) as an
  `error` event, then runs the previously installed hook, so that panics appear in the log stream
  as well as in the standard output of a crash.

## Alternatives considered

- **`log` and `env_logger`.** Simpler, but without spans: profiling would need a second
  instrumentation system.
- **A logging system written for the engine.** Reimplements what `tracing` provides, against
  the rule of not rebuilding what exists.
- **Hand-written `Display` and `Error` implementations.** Equivalent at run time; `thiserror`
  removes the boilerplate, and since it does not appear in the public API, dropping it later is
  mechanical.
- **`anyhow` in engine crates.** Callers could no longer match on the kind of failure.
- **Engine assertion macros from the start.** No requirement yet that `assert!` does not meet.

## Consequences

- Logs from the engine and from dependencies that use `tracing` or `log` (winit, for example)
  share one format and one filter.
- A profiler can later be attached as an extra `tracing` layer (for example for Tracy) without
  touching the instrumented code.
- `tracing` macros end up everywhere, so switching to another logging framework later would be
  expensive. This is why the choice is made now.
- `tracing-subscriber` with its filter adds a few seconds to a clean build.
- Games that want to log use `tracing` too, as a direct dependency.
