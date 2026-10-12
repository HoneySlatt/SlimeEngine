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
│   ├── slime-app/       application lifecycle and main loop
│   └── slime-platform/  window, its events and input
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
| [`slime-platform`](../crates/slime-platform/README.md) | The window, its events and keyboard and mouse input; the only crate that uses the windowing library (winit) | `winit`, `raw-window-handle`, `thiserror`, `tracing` | One window; window, keyboard and mouse events |
| [`slime-app`](../crates/slime-app/README.md) | Application lifecycle, main loop, frame timing, diagnostics setup, headless mode | `slime-platform`, `ctrlc`, `tracing`, `tracing-subscriber` | Headless and windowed runs work |
| [`slime-examples`](../examples/README.md) | Runnable examples; not part of the engine | `slime-app`, `slime-platform`, `softbuffer`, `tracing` | `hello`, `headless`, `window` |

## Dependency rules

```text
slime-examples ──▶ slime-app ──▶ slime-platform ──▶ winit
       │                               ▲
       └───────────────────────────────┘
```

- Dependencies between crates point one way: `slime-app` depends on `slime-platform`, never the
  reverse.
- Only `slime-platform` uses winit, and winit's types never appear in a public API
  ([ADR-0011](adr/0011-windowing-with-winit.md)). `slime_platform::Window` exposes the
  `raw-window-handle` traits so that graphics libraries can draw into it.
- A headless run never touches the windowing system: it does not create an event loop, so the
  windowing libraries, which are loaded at run time, are not even needed. The checks run the
  headless sample with no display server to prove it.

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
([ADR-0010](adr/0010-main-loop-ownership.md)). A *driver* decides when frames run and what time
they represent; every driver runs a frame the same way (`Frames::run_one` in
[`crates/slime-app/src/app.rs`](../crates/slime-app/src/app.rs)):

```text
frame                                span "frame", trace level
├── FrameClock::advance(delta)       the time the frame represents, limited
├── FixedStep::advance(delta)        how many fixed steps are due: n
├── app.fixed_update(ctx, step)      n times, span "fixed_update"
└── app.update(ctx, &frame)          once, span "update"
```

The run stops before the next frame once the application has called `ctx.request_exit()`, the
window was closed, the process received a termination signal, or `config.frame_limit` frames have
run. The frame in progress always
finishes, and `app.shutdown` is always called after `app.init`. The call order is checked by
[`crates/slime-app/tests/lifecycle.rs`](../crates/slime-app/tests/lifecycle.rs).

**Headless driver** (`AppConfig::window` is `None`), in `app.rs`:

```text
slime_app::run
├── init_diagnostics()
├── app.init(ctx)
├── loop: frame with delta = the fixed step (simulated time), back to back
└── app.shutdown(ctx)
```

**Windowed driver** (`AppConfig::window` is `Some`), in
[`crates/slime-app/src/windowed.rs`](../crates/slime-app/src/windowed.rs). winit owns the event
loop; `slime-platform` adapts its callbacks to the `EventHandler` trait, which the windowed driver
implements:

```text
slime_app::run
├── init_diagnostics()
└── slime_platform::run_event_loop(window config, driver)
    ├── winit "resumed"         create the window → driver.window_created
    │                           → ctx gets the window, app.init(ctx)
    ├── winit "window_event"    translate (event.rs) → driver.event
    │                           → app.on_event(ctx, &event); CloseRequested → stop
    ├── winit "about_to_wait"   → driver.about_to_wait
    │                           → if the next frame is due: frame with the measured delta;
    │                             return WaitUntil(next frame) or Exit
    └── winit "exiting"         → driver.exiting → app.shutdown(ctx), the window is released
```

Frames are scheduled every `1 / AppConfig::max_frame_rate` seconds: between them the event loop
sleeps until the next frame is due or an event arrives, so an idle window uses almost no CPU. A
frame that starts a little late does not shift the schedule, which keeps the average rate; after
falling a whole period behind, the schedule restarts instead of catching up with frames run back
to back (`next_due` in `windowed.rs`). This frame limiter is temporary; presenting frames on
screen will pace them from Stage 2.

### Termination signals

`run` installs, once per process, a handler for SIGINT (Ctrl+C), SIGTERM and SIGHUP
([`crates/slime-app/src/signal.rs`](../crates/slime-app/src/signal.rs)). The handler runs on a
thread of its own and only sets a flag; before each frame, the driver records the flag in the
`Context` as an exit request, so the run stops like any other: the frame in progress finishes and
`app.shutdown` runs. A second signal ends the process at once with exit code 130
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).

### Frame timing

`slime_app::time` has no clock of its own: the driver gives `FrameClock` the duration of each
frame (measured, or simulated in headless runs), gets a `FrameTime` back, and gives its delta to
`FixedStep`, which says how many fixed steps the frame must run. Source:
[`crates/slime-app/src/time.rs`](../crates/slime-app/src/time.rs); rules:
[ADR-0009](adr/0009-frame-timing.md).

### Showing the window (examples only)

A Wayland compositor shows a window only once the application has given it pixels. The engine has
no renderer yet, so the `window` example fills its window from the CPU with `softbuffer`, through
the window's `raw-window-handle` traits, whenever the window is resized. This is temporary and
lives only in the examples ([ADR-0002](adr/0002-amend-roadmap-stages-0-to-3.md)).

## Where to start reading

1. [`README.md`](../README.md) for what the project is and its state.
2. This file for the crate map.
3. [`PLAN.md`](PLAN.md) for the milestone in progress.
4. The `README.md` of the crate you are about to change.
5. [`examples/`](../examples/README.md) for runnable programs that use the engine.

## `unsafe` code

No crate contains `unsafe` code. [ADR-0005](adr/0005-lints-and-unsafe-policy.md) says when it is
allowed and how it must be written.
