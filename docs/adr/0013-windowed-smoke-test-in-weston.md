# 0013. Windowed smoke test in a headless Weston compositor

- Status: Accepted; amended by [0020](0020-rendering-in-the-main-loop.md) (GPU images, duration) and [0024](0024-windowed-lifecycle-check-in-weston.md) (lifecycle check)
- Date: 2026-10-12

## Context

The windowed path of the engine (connecting to the compositor, creating the window, running
frames from winit's callbacks, limiting the frame rate, shutting down) cannot be unit-tested,
and CI machines have no display server. Checking it by hand on a developer's desktop finds
regressions late, if at all.

## Decision

`scripts/check.sh`, which CI runs, starts [Weston](https://gitlab.freedesktop.org/wayland/weston)
with its headless backend (a Wayland compositor without screen, input devices or GPU) in a private
runtime directory, and runs the `window` example inside it for 120 frames. The check passes if the
example exits with code 0. It exits with an error code when it cannot connect, create its window
or draw into it; nothing stops it before its frame limit, since the compositor has no input and
never asks to close the window. Weston is provided by the Nix development shell.

This checks that the engine connects to a Wayland compositor, creates a window, that the example
can present a CPU buffer in it, that frames run and that the run ends cleanly. It does not check
keyboard or mouse input (the headless compositor has no input devices), what appears on screen
(its renderer draws nothing), X11, or behaviour specific to other compositors. Those remain manual
checks on a real desktop (niri, and X11 through Xwayland), described in `docs/PLAN.md`.

## Alternatives considered

- **No windowed check in CI.** Regressions in the windowed path would only be found by hand.
- **Xvfb, a virtual X server.** It would check the X11 path, but Wayland is the project's first
  target; it can be added later if X11 regressions appear.
- **Running the developer's own compositor nested.** Needs a display, which CI does not have.

## Consequences

- Every change is checked against a real Wayland compositor, in about two seconds.
- The development shell is larger, since it includes Weston and its libraries.
- The check depends on Weston's headless backend and its command-line options; a Weston upgrade
  through `flake.lock` that changes them shows up as a failing check.
