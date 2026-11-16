# 0282. Errors, logging, assertions; uwui's public enumerations open to new cases

- Status: Accepted; supersedes [0008](0008-errors-logging-and-assertions.md) and
  [0019](0019-default-log-filter.md)
- Date: 2026-11-16

## Context

[ADR-0008](0008-errors-logging-and-assertions.md) set the engine's errors, logging and assertions;
[ADR-0019](0019-default-log-filter.md) changed its default log filter, and
[ADR-0085](0085-reflection.md) and [ADR-0241](0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)
let two crates write their errors without `thiserror`. ADR-0008 marked an error enumeration
`#[non_exhaustive]` "when new cases are expected", decided case by case.

On 2026-11-16, 7 of uwui's 67 public enumerations carried the attribute: `Kind`, the kinds of
elements, did not, nor 5 of its 8 enumerations of errors. uwui keeps semantic versioning from its
1.0: a case added to an enumeration without the attribute breaks every program that matches it,
a major release. Clippy's `exhaustive_enums` names every public enumeration without the attribute.

This ADR states the rule of errors, logging and assertions whole, as
[ADR-0281](0281-decisions-written-whole-consolidated-before-the-move-the-changelog-in-fragments.md)
asks of a changed decision.

## Decision

The rules of ADR-0008 and ADR-0019, the owner's decisions of 2026-10-12, and the rule of uwui's
enumerations, the owner's decision of 2026-11-16.

**Errors.**

- A failure that depends on the environment (no display server, a file that cannot be read, a
  device that refuses a request) is returned as an error. A violated invariant, which can only come
  from a bug, panics.
- Each library crate defines its own error types: enumerations, so that callers can match on what
  went wrong. Their `Display` and `Error` are derived with
  [`thiserror`](https://crates.io/crates/thiserror), but in `slime-reflect`, which depends on glam
  and its derive alone ([ADR-0085](0085-reflection.md)), and in `uwui-kdl`, which depends on
  nothing ([ADR-0241](0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)). uwui's own
  rule for writing its errors is set at M6d.10 ([`debt.md`](../debt.md)).
- A dependency's error type never appears in a public error type. It is kept as the error's
  `source()`, boxed as `Box<dyn std::error::Error + Send + Sync>`, so that the cause is still
  printed and the dependency can be replaced.
- Engine crates do not use dynamic error crates such as `anyhow`. Examples and tools may, or may
  return `Box<dyn Error>`.

**Enumerations.**

- In uwui's crates, and in `slime-color` and `slime-input`, which leave with it, every public
  enumeration is `#[non_exhaustive]`, errors included. Each crate turns on
  `#![warn(clippy::exhaustive_enums)]`, which `scripts/check.sh` makes an error.
- An enumeration closed by nature, whose cases cannot grow, is left exhaustive with
  `#[allow(clippy::exhaustive_enums, reason = "…")]`, its reason saying why: a button pressed or
  released, the four edges of a rectangle.
- A program that matches one of them, the engine and uwui's other crates included, gives the cases
  that it does not know a behaviour of their own, written beside them: nothing drawn, a plain
  container read, a refusal that says so.
- In the engine's crates, an error enumeration is `#[non_exhaustive]` when new cases are expected,
  until the engine keeps semantic versioning.

**Logging.**

- All crates log through the [`tracing`](https://crates.io/crates/tracing) macros (`error!`,
  `warn!`, `info!`, `debug!`, `trace!`) and record profiling spans with the same crate.
- Only `slime-app` installs the log subscriber, in `init_diagnostics`, using
  [`tracing-subscriber`](https://crates.io/crates/tracing-subscriber). It writes to standard error;
  the `SLIME_LOG` environment variable selects what is recorded. A game that installs its own
  subscriber first keeps it.
- When `SLIME_LOG` is unset or holds no directive, `slime_app::DEFAULT_LOG_FILTER` applies: `info`
  and above, but for the modules of dependencies whose messages the engine cannot act on, each
  named in its rustdoc with why. Any value of `SLIME_LOG` replaces it whole.
- Levels: `error` when something failed and the requested work did not happen; `warn` for
  something unexpected that was handled; `info` for a few lifecycle events (start, stop, a window
  created); `debug` for details useful when investigating; `trace` for per-frame or per-event
  detail.
- Values that matter are recorded as structured fields (`info!(width, height, "resized")`), not
  only formatted into the message.

**Assertions and panics.**

- `assert!` checks invariants that are cheap enough to keep in release builds; `debug_assert!`
  checks the ones on hot paths. There are no engine-specific assertion macros until a need appears
  that the standard ones cannot meet.
- `init_diagnostics` installs a panic hook that logs the panic (message, location, thread) as an
  `error` event, then runs the previously installed hook, so that panics appear in the log stream
  as well as in the standard output of a crash.

## Alternatives considered

- **uwui's enumerations marked case by case,** as before: nothing to do, but the same 7 of 67, and
  each one forgotten a major release after the 1.0.
- **No attribute, each case added a major release:** programs told of each new case by the
  compiler, but uwui bound to many majors or to adding nothing.
- **`uwui_render::Error` kept closed,** so that a new case breaks the host's match where it is
  compiled: the host told, but each new error a major release after the 1.0.
- **`log` and `env_logger`.** Simpler, but without spans: profiling would need a second
  instrumentation system.
- **A logging system written for the engine.** Reimplements what `tracing` provides.
- **Hand-written `Display` and `Error` everywhere.** Equivalent at run time; `thiserror` removes
  the boilerplate, and since it does not appear in the public API, dropping it later is mechanical.
- **`anyhow` in engine crates.** Callers could no longer match on the kind of failure.
- **The default filter `info` for every module:** dependencies' messages that the engine cannot act
  on would fill every log.

## Consequences

- uwui can gain a case after its 1.0 without breaking a program.
- A program, uwui's own renderer and screen readers' adapter among them, is no longer told by the
  compiler of a case that uwui gained: it falls to the behaviour written for unknown cases. A
  case added to uwui is followed through its crates by hand and by their tests.
- Logs from the engine and from dependencies that use `tracing` or `log` share one format and one
  filter; a profiler can be attached as a `tracing` layer without touching instrumented code.
- `tracing` macros are everywhere: switching to another logging framework would be expensive.

## Validation and reconsideration

Counted: uwui's public enumerations and those marked, by `clippy::exhaustive_enums`. Tried: the
workspace built, checked and tested with the 59 marked and the 6 closed by nature.

Proposing another decision is justified if a case added to uwui goes unhandled in one of its own
crates because no compiler said so, or once a stable lint that names the cases that a wildcard
hides, as nightly's `non_exhaustive_omitted_patterns` does, makes that check possible.
