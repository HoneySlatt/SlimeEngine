# Architecture

This document maps the code: which crates exist, what each one is responsible for, how they
depend on each other, and where to start reading. It describes the repository as it is now;
[`ROADMAP.md`](ROADMAP.md) describes where it is going, and [`adr/`](adr/README.md) records why
it is built this way.

## Repository layout

```text
.
├── .github/workflows/   CI: runs scripts/check.sh
├── Cargo.toml           virtual workspace: shared metadata, lints and dependency versions
├── crates/              engine crates, one directory per crate, named after the package
│   ├── slime-app/       application lifecycle and main loop; headless only so far
│   └── slime-platform/  windows and input (skeleton)
├── deny.toml            dependency license, advisory and source rules
├── docs/                roadmap, plan, decisions (adr/), glossary, dependencies, this file
├── examples/            runnable examples, one per feature (package `slime-examples`)
├── flake.nix            Nix development shell
├── rust-toolchain.toml  pinned Rust toolchain
├── scripts/check.sh     every automated check, as CI runs them
└── AGENTS.md            rules for every contributor
```

[ADR-0004](adr/0004-workspace-layout-and-crate-naming.md) sets the layout and naming rules: the
root is a virtual workspace, engine crates are `crates/slime-<role>/`, and a directory or a crate
is created only when code needs it.

## Crates

| Crate | Role | Depends on | Status |
|---|---|---|---|
| [`slime-platform`](../crates/slime-platform/README.md) | Windows and input; the only crate that uses the windowing library | — | Skeleton |
| [`slime-app`](../crates/slime-app/README.md) | Application lifecycle, main loop, frame timing, diagnostics setup, headless mode | `tracing`, `tracing-subscriber` | Headless runs work; no window yet |
| [`slime-examples`](../examples/README.md) | Runnable examples; not part of the engine | `slime-app`, `tracing` | `hello`, `headless` |

A *skeleton* crate exists with its documentation and inherits the workspace policies, but holds
no code yet.

## Dependency rules

```text
slime-app ──may depend on──▶ slime-platform
```

- Dependencies between crates point one way. `slime-app` may depend on `slime-platform`;
  `slime-platform` never depends on `slime-app`. Neither depends on the other yet.
- Only `slime-platform` uses the windowing library, and its types never appear in a public API.
- A headless run never touches the windowing system.

## Main flows

### Diagnostics setup

An engine program calls `slime_app::init_diagnostics()` first. It installs a global `tracing`
subscriber writing to standard error (unless one already exists), filtered by the `SLIME_LOG`
environment variable, then a panic hook that logs panics before the standard hook prints them,
then logs one line describing the engine and the machine. From then on, every `tracing` event
from the engine, the game or a dependency goes through that subscriber. Source:
[`crates/slime-app/src/diagnostics.rs`](../crates/slime-app/src/diagnostics.rs); conventions:
[ADR-0008](adr/0008-errors-logging-and-assertions.md).

### Running an application

A game implements `slime_app::App` and calls `slime_app::run`, which owns the main loop
([ADR-0010](adr/0010-main-loop-ownership.md)). Today every run is headless:

```text
slime_app::run(&mut app, &config)
├── init_diagnostics()                   no effect if already done in this process
├── app.init(ctx)
├── loop, until ctx.request_exit() was called or config.frame_limit frames have run
│   └── frame                            span "frame", trace level
│       ├── FrameClock::advance(delta)   headless: delta = the fixed step (simulated time)
│       ├── FixedStep::advance(delta)    how many fixed steps are due: n
│       ├── app.fixed_update(ctx, step)  n times, span "fixed_update"
│       └── app.update(ctx, &frame)      once, span "update"
└── app.shutdown(ctx)
```

An exit request takes effect between frames: the frame in progress always finishes. Source:
[`crates/slime-app/src/app.rs`](../crates/slime-app/src/app.rs); the call order is checked by
[`crates/slime-app/tests/lifecycle.rs`](../crates/slime-app/tests/lifecycle.rs).

### Frame timing

`slime_app::time` has no clock of its own: the code that drives frames gives `FrameClock` the
duration of each frame (measured, or simulated in headless runs), gets a `FrameTime` back, and
gives its delta to `FixedStep`, which says how many fixed steps the frame must run. Source:
[`crates/slime-app/src/time.rs`](../crates/slime-app/src/time.rs); rules:
[ADR-0009](adr/0009-frame-timing.md).

## Where to start reading

1. [`README.md`](../README.md) for what the project is and its state.
2. This file for the crate map.
3. [`PLAN.md`](PLAN.md) for the milestone in progress.
4. The `README.md` of the crate you are about to change.
5. [`examples/`](../examples/README.md) for runnable programs that use the engine.

## `unsafe` code

No crate contains `unsafe` code. [ADR-0005](adr/0005-lints-and-unsafe-policy.md) says when it is
allowed and how it must be written.
