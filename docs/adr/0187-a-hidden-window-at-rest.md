# 0187. A hidden window at rest

- Status: Accepted; amends [0021](0021-frame-pacing.md) (on Wayland, frames wait for the
  compositor's signal, and the loop sets no deadline of its own while it asks for a frame); amended by
  [0227](0227-the-time-that-motion-reads.md) (the loop wakes for its tasks, and draws nothing)
- Date: 2026-11-05

## Context

[ADR-0185](0185-rest-checked-in-ci.md) decided how a hidden window rests, and left it to M6c.Q2c
to build: on Wayland, a window draws when the compositor signals that it can show a new image, and
the loop does not wake on its own schedule while it waits for that signal; on Windows, a minimized
window draws nothing and does not wake. It left open how a test hides a game.

Measured on 2026-11-05 in a headless Weston 16 with lavapipe:

- winit 0.30 gives the compositor's signal, a *frame callback*, through `pre_present_notify`:
  called before an image is presented, it makes the next `request_redraw` wait for the callback.
  Weston's desktop shell sends none to a minimized window. Mesa presents with Mailbox without
  waiting for them.
- Hunna Wood minimized, its game time running, kept its 60 frames a second: in its last second
  before the check gave up, each of its 16 threads of lavapipe switched about 600 times.
- The loop asked for a redraw with a deadline of one frame, so that termination signals were
  noticed even if the redraw never came. Termination signals, the end of the tool that started the
  program and the development channel's requests wake the loop since
  [ADR-0156](0156-the-editors-window-and-its-operations.md).
- The development channel answers its requests within frames: a game that runs no frame while it
  is hidden answers none, the request to stop included.
- On Windows, a minimized window's size is zero: its frames ran at their schedule, drawing nothing.

The owner chose on 2026-11-05 how the test hides the game: a request of the development channel.

## Decision

**Frames on the compositor's signal.** Before each image is presented, the run calls
`slime_platform::Window::pre_present_notify`. On Wayland, the window's next redraw then waits for
the compositor's frame callback; elsewhere the call does nothing.

**A window holds its frames.** The loop asks for a redraw without a deadline. The window *holds its
frames* while a redraw asked for has not come, as on Wayland until the compositor's signal, or while
its size is zero, as a minimized window's on Windows. Meanwhile, no frame of the schedule runs, nor
one that the program asks for itself: `Context::request_frame`, its interface's motion, its steps.
The loop wakes on no schedule of its own.

**What asks from outside still runs a frame.** An event of the window, a wake of the run's
`Waker`, which a thread, a termination signal, the tool's end and the development channel give, or
a GPU device restored, runs a frame anyway, once the redraw has waited 100 ms for the compositor's
signal (`SIGNAL_WAIT`). 100 ms is longer than a refresh of any display, so a window that shows keeps
its frames in step with the display; it is short enough that an agent's request to a hidden game is
answered at once. A window without area runs it 100 ms after its last frame. A request of the
channel that waits for several frames, steps or a capture, gets one every 100 ms. Such a frame
draws and presents as any other.

**One frame checks each hold.** The first time a redraw has waited 100 ms, one frame runs too,
whatever asks. An image presented to a device lost before the run knew it never reaches the
compositor, whose signal would then never come: that frame finds the loss, and the device restored
draws. A device known to be lost presents nothing, and its frame does not call
`pre_present_notify`. The window then holds its frames until the signal comes, or something asks.

**The time hidden.** A frame run while the window holds its frames, and the one that ends a hold,
represent at most one period of the schedule: game time barely moves while the window is hidden,
and each request to a hidden game moves it by one frame at most.

**A way to hide a game.** `Window::minimize` asks the compositor to minimize the window, as the
user would. The development channel's `Request::Minimize` and the MCP server's `minimize_game`
call it; the protocol's version becomes 13. Only the user shows the window again: on Wayland, a
program cannot.

**Checked in CI.** The `drive` test, in `check (game)`, minimizes Hunna Wood once its paused check
passed, lets its game time run again, and checks that it is at rest
([ADR-0185](0185-rest-checked-in-ci.md)'s helper). With the old pacing, the same test fails.

## Alternatives considered

- **A hidden window that runs no frame at all until it shows**: the simplest rule, but the
  development channel answers nothing, and `stop_game` waits 120 s before it kills the game.
- **A deadline kept to find a lost device**: a hidden window would wake at each one. One frame per
  hold finds it, before the check of rest starts to count.
- **Frames run while hidden representing the real time since the last**: each request to a hidden
  game would move its game time by up to `AppConfig::max_frame_delta`, 250 ms by default.
- **What asks from outside, run at once without waiting for the signal**: a visible window's frames
  would leave the display's step at each event, while the pointer moves.
- **A longer deadline kept, one second**: a hidden window would wake every second, own wake-ups
  that the budget of rest forbids.
- **An option of the game's command line that minimizes it**: no change to the protocol, but a
  test's setting in every program, and the test would not choose the moment.
- **Another client of Weston hiding the game**: Weston gives no protocol to hide another client's
  window, but its test plugin, which nixpkgs's Weston does not ship.
- **Frames aligned on every frame callback, at the display's rate**: ADR-0021 rejected it, and the
  debt "Frames are not synchronized with the display's refresh" follows it.

## Consequences

- Measured in headless Westons with lavapipe, the `window` example in a release build: at 60, 144
  and 240 Hz, its frames keep their pacing; at 50 Hz, they follow the display, 50 a second, every
  image shown, where the old pacing drew 60, ten of which were never shown
  ([`measurements.md`](../measurements.md#m6cq2c-ryzen-7-5800x)).
- On Wayland, a hidden window under a compositor that sends no callback to hidden windows, as
  Weston, draws nothing and wakes not at all; under one that sends a callback each second, as niri
  did (ADR-0021), it draws once a second. The owner's compositor and Windows are checked by hand.
- A game hidden, or minimized on Windows, runs no fixed step of its own: a game that must go on
  while hidden, a server's or a multiplayer game's, cannot yet, until one needs it
  ([`debt.md`](../debt.md)). A run with a frame limit does not reach it while hidden.
- A capture asked of a window without area is never answered, its frames drawing nothing, and runs
  a frame every 100 ms until the window shows.
- A hidden window wakes once at the start of each hold, for the frame that checks it.
- On Wayland, the redraws that a program asks for itself, `Window::request_redraw`, run no faster
  than the compositor's signal comes, the display's rate: the windowed lifecycle test, whose burst
  of redraws must run frames ahead of their schedule, runs in a headless Weston at 240 Hz.
- On X11, a hidden window keeps drawing: the X server reports windows visible under a compositing
  manager.
- `check (game)`'s `drive` test watches one more rest, about 15 seconds.
