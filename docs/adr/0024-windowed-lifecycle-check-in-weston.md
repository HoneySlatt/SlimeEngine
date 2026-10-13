# 0024. Windowed lifecycle check in the headless compositor

- Status: Accepted
- Date: 2026-10-12

## Context

[ADR-0013](0013-windowed-smoke-test-in-weston.md) runs the `window` example for 120 frames in a
headless Weston compositor and reads its exit code. That proves that the engine connects, creates
its window, draws into it and ends, but nothing about how the windowed driver calls the
application: the order of `init`, `on_event`, `fixed_update`, `update`, `render` and `shutdown`,
what `init` sees, the error of a failed `init`, the one-event-loop-per-process rule, or the frame
schedule. [ADR-0010](0010-main-loop-ownership.md) says the order of calls is a guarantee that tests
check, which the headless tests do for the headless driver only. A regression of the schedule
after unscheduled redraws, which pushed frames seconds away after a resize, went through the check:
nothing resizes a window in the headless compositor.

A plain `#[test]` cannot do this: the windowing library creates its event loop on the main thread
only, and the test harness runs tests on other threads; and a test that skipped itself without a
compositor would hide its own absence from `cargo test`.

## Decision

- `crates/slime-app/tests/windowed_lifecycle.rs` is a test target without a harness
  (`harness = false`) and outside the default test run (`test = false`): `scripts/check.sh` runs it
  explicitly in the headless Weston compositor, after the `window` example, with lavapipe. It uses
  the public API only.
- A process can run one windowed application, so the program runs itself again once per scenario
  and checks what the child prints: a normal run (`init` sees the window, the GPU and the surface
  format; each frame runs its fixed steps, then `update`, then `render`; `shutdown` comes last; a
  second windowed run returns `PlatformError::SecondEventLoop`; the schedule paces the frames, a
  burst of redraws requested by the application runs frames back to back, and the first scheduled
  frame after the burst comes within 100 ms), a failed `init` (`RunError::Init`, no frame, no
  `shutdown`) and an exit requested in `init` (no frame, `shutdown`).
- The Windows job does not run it, having no Weston: the windowed lifecycle on Windows stays a
  check by hand ([ADR-0015](0015-platforms-and-ci.md)).

This amends ADR-0013: the Weston step also checks the windowed lifecycle, not only that the sample
runs and draws.

## Alternatives considered

- **A plain `#[test]` that skips without a display.** It would run on a thread the windowing
  library refuses, and its skipping would hide its absence.
- **An example in `examples/`.** Examples demonstrate features; this program is a test.
- **Assertions inside the `window` example.** The sample would become a test, in one process, which
  cannot cover both a failed `init` and a normal run.

## Consequences

- The Weston step takes a few seconds more: one child process per scenario, each opening
  lavapipe.
- A change to the order of calls, to what `Context` offers in `init`, to the stop conditions or to
  the schedule fails the Linux job.
- The program's scenarios are where windowed checks that need no input go; input and what appears
  on screen remain manual checks, as ADR-0013 says.

## Corrections

- 2026-10-13: "It uses the public API only" no longer holds for one scenario: `device-lost`
  destroys the GPU device through `slime-gfx`'s `wgpu` feature, which ADR-0031 now allows the
  engine's own tests to enable. Every other scenario uses the public API only.
