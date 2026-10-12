# 0012. Clean shutdown on termination signals

- Status: Accepted
- Date: 2026-10-12

## Context

A Rust program that receives SIGINT (Ctrl+C in a terminal), SIGTERM (a service manager or
`kill` stopping it) or SIGHUP (its terminal closing) is ended by the operating system at once:
no destructor runs and `App::shutdown` is never called, so a game cannot save or release anything,
and a future dedicated server cannot leave cleanly. The engine's lifecycle promises that
`shutdown` is called once `init` has been (ADR-0010).

## Decision

- `slime_app::run` installs a handler for SIGINT, SIGTERM and SIGHUP, once per process, with the
  [`ctrlc`](https://crates.io/crates/ctrlc) crate (feature `termination`).
- The first signal sets a process-wide flag. The drivers check it between frames, as they check
  an exit request: the frame in progress finishes, `App::shutdown` runs, and `run` returns
  normally. The application sees the signal through `Context::exit_requested`. The headless driver
  checks it before every frame; the windowed driver wakes up at least once per frame period while
  its loop runs, so a signal is then noticed within one frame. It is noticed later when the loop
  does not run: on Windows during the system's own loop (a move, a resize, a held title bar, an
  open system menu), and before the first frame, while the windowing library waits for the
  window's first configuration.
- A second signal ends the process at once with exit code 130, without `shutdown`. Without it, an
  application stuck in a frame could not be stopped with Ctrl+C.
- The flag is never cleared: a signal asks the whole process to stop, so later runs in the same
  process stop immediately too.
- If the process already has a `ctrlc` handler, for example one installed by the game, the
  engine keeps it and logs a warning; signals then follow the game's handler.

## Alternatives considered

- **No handling.** Ctrl+C would skip `shutdown`, breaking the lifecycle guarantee.
- **`signal-hook`.** More flexible (any signal, several handlers), but lower level: the engine
  would write the waiting thread and the platform differences that `ctrlc` already provides, for
  three signals.
- **Handling signals in `slime-platform`.** Signals concern the process, not the windowing system,
  and headless runs, which never use `slime-platform`, need them most.
- **Exiting with code 130 after a clean stop**, as shells expect from a program stopped by
  Ctrl+C. `run` returning normally lets the game choose its exit code; the examples exit with 0.

## Consequences

- Ctrl+C and service stops run `App::shutdown`, in both modes; tests check SIGINT, SIGTERM and the
  forced exit (`crates/slime-app/tests/signals.rs`).
- On Windows, `ctrlc` receives the console's events: Ctrl+C and Ctrl+Break stop the run cleanly, but
  Windows ends the process as soon as the handler returns for the console being closed, a logoff
  or a shutdown, so those get no clean shutdown.
- The handler replaces whatever the process had set for these signals: handlers installed by
  other libraries (signal-hook, tokio) stop receiving them, and an inherited "ignore" is lifted
  (under `nohup`, closing the terminal then stops the run cleanly instead of being ignored).
  `ctrlc`'s `try_set_handler` would keep them, but it installs the three signals together, so an
  inherited "ignore" of SIGINT, common for commands a script starts in the background, would
  leave SIGTERM unhandled too.
- A game cannot install its own `ctrlc` handler after `run` has started; if games need custom
  signal handling, the engine will need an API for it.
- Reversing the decision means removing one module of `slime-app` and the `ctrlc` dependency.
