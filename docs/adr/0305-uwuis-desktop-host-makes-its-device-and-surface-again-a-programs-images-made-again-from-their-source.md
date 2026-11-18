# 0305. uwui's desktop host makes its device and its surface again; a program's images made again from their source

- Status: Accepted; applies [0264](0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md)'s
  budget of a lost surface and [0075](0075-recovering-from-a-lost-gpu-device.md)'s of a lost device
  to uwui's desktop host
- Date: 2026-11-18

## Context

M6d.7a2b2, the second part of [ADR-0302](0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md)'s
cut, makes uwui's desktop host, `uwui-desktop`, survive a lost GPU device and a lost surface, each
lost on purpose by a door of the tests. Its card also asks that an image drawn in part be no longer
presented, and that who makes a program's images again after a lost device be said.

The facts, read in the repository on 2026-11-18:

- Until this milestone the host configured a surface lost once again, then ended with
  `EXIT_HOST_FAILED`; a device lost ended it at its next frame. No test lost either.
- The engine makes both again: a device replaced at most three times within five minutes
  (ADR-0075), a surface given up at its fourth loss in a row, made again a second later or at an
  event that may change it, the fourth given up within five minutes ending the run (ADR-0264). Its
  windowed loop counts them with `Budget` and `Remake` (`crates/slime-app/src/windowed.rs`), under
  the engine's licence; uwui is under MIT OR Apache-2.0, and the owner is the author of both
  ([ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)).
- The engine's doors of the tests are public: `Gpu::lose`, `Surface::lose` and
  `Context::lose_surface`, which the game reaches by `--lose-device-at` and a button of its overlay.
- On the device, the host holds only what `uwui-render` makes: the atlas of glyphs and the canvas's
  buffers, which its first frame makes again, and uwui draws its whole display list at each frame.
  An application on the host gives no image yet: the host's draw gives `uwui-render` none.
- `uwui-render` takes each image as a texture of the device, which the program holds
  (`crates/uwui-render/src/lib.rs`); uwui holds only the images' keys. After a loss, those textures
  are the lost device's.
- wgpu 30 says why a device was lost only as `Unknown` or `Destroyed`: whether memory ran out is
  not told, which the engine knows from its own errors (ADR-0075).
- A draw that fails, the GPU's memory short, was presented in part, and the frame's changes were
  spent: the window stayed as drawn, black or in part, until something else changed.

## Decision

**A program's images are made again by the host, from their source**, the owner's choice of
2026-11-18. When the host takes images, a program gives each with its source, a file or its encoded
bytes; the host decodes it, gives it to the device, and makes it again after a lost device. The
program hears nothing of a loss. The host keeps the source, not the decoded pixels. Nothing is built
now: the host takes no image yet. The milestone that gives it images builds it.

Then the lead's, within ADR-0264 and ADR-0075:

- **The engine's budgets, in the host's core.** A module of the core, `recovery`, holds the
  engine's `Budget` and `Remake`, adapted to uwui's clock, the copy named in its documentation. The
  host replaces a lost device at once, between frames, three times within five minutes; the fourth
  loss ends the run with `EXIT_HOST_FAILED`, the application stopped. A surface lost is made again
  at once, and given up at its fourth loss in a row without an image between; the host makes a
  surface given up again a second later, or at once when the window is configured or its scale
  changes; after an attempt that failed, after a wait that doubles. The fourth given up or failed
  within five minutes ends the run. While its surface is given up, the window holds its frames.
- **Always on.** The host replaces a lost device without asking the application, unlike the
  engine's `AppConfig::restore_lost_device`: the application holds no object of the device.
- **The device replaced with every object of the old one gone first**, its renderer and its
  surface, as ADR-0075 asks for Direct3D 12; through the same wgpu instance, the adapter chosen as
  at the start, and never one on the processor after one that was not.
- **The doors of the tests**: `Window::lose_device` and `Window::lose_surface`, public, documented
  for the tests and the tools, as the engine's `Context::lose_surface`. The host makes them after
  the application's step, between frames. The example takes them as commands on its standard input,
  with `--commands`, which the `rest` test gives; the series of compositors runs it.
- **An image drawn in part is not presented.** The host drops it, the window keeping the image
  before, and draws the frame again after a wait that doubles from 100 ms to 1.6 s, reset by an
  image presented. A device lost during a draw drops its image too; the next turn replaces it.
- **After either loss, the next frame runs even while the window holds its frames**, as the
  engine's: the image that the lost device presented may never reach the compositor, whose signal
  would then never come.
- **wgpu's errors once the device is lost** are logged at the debug level: wgpu refuses what uses
  a lost device, which tells nothing more
  ([ADR-0103](0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)).

## Alternatives considered

On a program's images, put to the owner:

- **The host, from the decoded pixels**: the simplest; nothing for the program to forget. But it
  keeps a copy of every image in the memory, against the budget of memory.
- **The program**, told by a call of `App` and giving its images again: the host keeps nothing.
  But every program must do it, and one that forgets shows its images missing without an error.

On the rest:

- **A door through an environment variable** read by the host: no public API. But a behaviour
  hidden in every program built on the host, unlike the engine's doors.
- **A door behind a feature of Cargo**: no public API in a normal build. But a second build of the
  host for the tests, and the series runs the example as it is built.
- **The engine's choice, a device replaced only if the application asks**: one rule for both. But
  the engine's opt-in exists for a game's objects of the device, which an application on uwui does
  not hold.
- **A device lost by memory never replaced**, as ADR-0075 says: wgpu does not tell it; replacing it
  costs at most three new devices within five minutes.
- **An image drawn in part retried at the next signal**: drawn again at once if the memory came
  back. But a memory that never comes back draws at the display's rate for nothing.

## Consequences

- An application on uwui's desktop host survives three lost devices and three surfaces given up
  within five minutes; the fourth of either ends it with `EXIT_HOST_FAILED` and its reason, after
  `App::stop`.
- `Window` gains `lose_device` and `lose_surface`; the example `--commands`; the `rest` test a
  third test, and the series of compositors a row, *uwui's host's losses*
  ([`compositors.md`](../compositors.md)).
- A device lost while the window rests is found at the next frame, as in the engine: nothing polls
  the device at rest.
- A window whose GPU's memory stays short wakes about once every 1.6 s to try its frame again.
- The milestone that gives the host images builds them from their source, made again after a loss
  ([`debt.md`](../debt.md)).
