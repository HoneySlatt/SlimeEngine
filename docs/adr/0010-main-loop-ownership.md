# 0010. Main loop ownership and the application API

- Status: Accepted; amended by [0020](0020-rendering-in-the-main-loop.md) (rendering and errors)
  and [0107](0107-the-execution-model.md) (a game as a simulation and a presentation, run by a host)
- Date: 2026-10-12

## Context

The engine runs frames in a loop. With a window, the windowing library chosen for Stage 1
(winit, see the windowing ADR) owns the event loop: it calls the program back when events arrive
or when it is about to wait (`run_app` and its `ApplicationHandler` trait), and on some platforms
it never gives control back. Without a window there is no event loop at all. Whatever shape the
loop takes decides the shape of the API that every game implements, which is expensive to change
once games exist.

Two shapes are possible:

- the engine calls the game back (inversion of control): the game implements a trait and hands
  it to the engine;
- the game owns the loop and calls the engine once per frame (`while engine.step() {}`), which
  with winit requires "pumping" its events from the game's loop (`pump_app_events`).

## Decision

- A game implements the `slime_app::App` trait and calls `slime_app::run(&mut app, &config)`.
  The engine owns the main loop in every mode.
- `App` has four methods, all with empty default implementations: `init` (once), `fixed_update`
  (once per fixed step), `update` (once per frame, after its fixed steps) and `shutdown` (once,
  after the last frame). Windows will add a method for platform events.
- Each mode has a *driver*, the loop that decides when frames run and what time they represent.
  All drivers call the same frame function, so frames behave the same in every mode:
  - the headless driver is a plain loop in `slime-app` that runs frames back to back with
    simulated time (ADR-0009);
  - the windowed driver will be called back by winit's event loop through `slime-platform`.
- A run stops when the application calls `Context::request_exit` or when
  `AppConfig::frame_limit` is reached. The current frame always finishes, and `shutdown` is
  always called once `init` has been.
- `run` borrows the application (`&mut`), so the caller can inspect it after the run.
- Each frame is a `trace`-level profiling span (`frame`), containing one span per
  `fixed_update` and one for `update`.
- `App` methods return nothing. An application that hits an error it cannot handle logs it and
  requests an exit.

## Alternatives considered

- **Engine-owned loop pumping winit's events (`pump_app_events`).** Headless and windowed loops
  would look identical, but winit discourages this mode: it is unavailable on the web and on
  mobile platforms, and on Windows the loop freezes while the user resizes or moves the window.
- **Game-owned loop calling `engine.step()`.** Gives games the most control, but cannot be built
  on top of winit's callback model without pumping, and lets every game get the frame order wrong
  in its own way.
- **Closures instead of a trait.** Several closures cannot all borrow the game's state mutably;
  a trait with default methods expresses the same thing without that problem.
- **Methods returning `Result`.** No caller handles an application's error better than the
  application itself yet; it can be added when a real case appears.

## Consequences

- Games are written the same way whether they run with a window or headless, and tests drive
  them headless.
- Changing how a driver works (for example moving the windowed driver to pumped events) is an
  internal change that games do not see.
- Applications cannot return errors to `run`; if that proves limiting, changing the method
  signatures will break every game, so it should be decided early.
- The order of calls is a guarantee that tests check (`crates/slime-app/tests/lifecycle.rs`).
