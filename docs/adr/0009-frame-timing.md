# 0009. Frame timing

- Status: Accepted; amended by [0107](0107-the-execution-model.md) (game time apart from real time)
- Date: 2026-10-12

## Context

Each frame of the main loop represents some amount of time. Two kinds of code consume it:

- code that should follow the frame rate (animation of the user interface, camera smoothing)
  wants the real time between frames;
- code that must behave the same at any frame rate (physics, gameplay that has to be
  reproducible or replicated over a network) wants to advance in steps of a fixed duration.

Headless runs (automated tests now, dedicated servers later) need time to be deterministic: a
test that runs 600 frames must see exactly the same time values on every machine, without
sleeping. And when a process is paused (by a debugger, or a suspended laptop) the next frame must
not represent minutes of time.

## Decision

- `slime_app::time::FrameClock` turns the time between frames into a `FrameTime` (frame index,
  delta, elapsed application time). It limits the delta of a frame to a maximum; the time cut
  off is not counted, so application time falls behind wall-clock time after a pause.
- `slime_app::time::FixedStep` accumulates frame deltas and says how many steps of a fixed
  duration to run in each frame. It runs at most a maximum number of steps per frame and drops the
  rest of the backlog, so a slow machine sees the simulation slow down instead of entering the
  "spiral of death" (catching up takes longer than the time caught up on). It exposes the fraction
  of a step left in the accumulator, so a renderer can interpolate between simulation states.
- Neither type reads the system clock. Whoever drives them supplies the durations:
  - with a window, the real time measured between frames;
  - headless, a simulated delta equal to the fixed step, so each frame runs exactly one fixed
    step, as fast as the machine allows, with identical results on every run.
- Time is kept as `std::time::Duration` (whole nanoseconds), never as floating-point seconds,
  so accumulating it never drifts. Only the interpolation fraction is a floating-point number.
- Default values, set by the application configuration: fixed step 1/60 s (rounded to
  16 666 667 ns), at most 8 fixed steps per frame, maximum frame delta 250 ms.
- Headless runs paced in real time (a dedicated server ticking at a fixed rate) are not built
  yet; they come with the first dedicated server.

## Alternatives considered

- **Floating-point seconds.** Sums of `f32` or `f64` durations drift and depend on the order of
  additions, which breaks determinism.
- **A clock that reads the system time, behind a trait with a fake for tests.** An abstraction
  with two implementations where passing a duration does the same job.
- **Variable step only.** Physics and gameplay would behave differently at different frame
  rates.
- **Fixed step only, with rendering tied to it.** The display rate would be locked to the
  simulation rate.
- **No limit on the frame delta or on the steps per frame.** One debugger pause would produce a
  frame of several minutes, and a slow machine would never recover.

## Consequences

- Headless tests are deterministic and do not sleep.
- 1/60 s is not a whole number of nanoseconds; the default step runs at 59.999 998 Hz, a
  difference that no one can observe.
- Dropping the backlog means the simulation can fall behind wall-clock time. Games that must stay
  in sync with real time (networked games) will need their own policy on top, decided when
  networking is designed.
- Changing a default value is a one-line change; changing the model (for example to a
  "semi-fixed" step) would touch the drivers and every user of `FixedStep`.
