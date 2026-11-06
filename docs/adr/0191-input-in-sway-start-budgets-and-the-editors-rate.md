# 0191. Input given in sway, two budgets of a start, and the editor's rate

- Status: Accepted; amends [0175](0175-the-quality-bar.md) (the budgets of a level opened and of a
  game to control) and [0189](0189-first-measures-of-time.md) (input to present measured with a
  person's input in a headless sway; the editor's rate of frames)
- Date: 2026-11-06

## Context

[ADR-0189](0189-first-measures-of-time.md) decided how M6c.Q2d times a start and input to present.
It left three things to its measures:

- Input to present was to be measured by hand on the owner's desktop: nothing gives a headless
  Weston a person's input, and an agent never gives input to the owner's session.
- A level opened and a game to control had no budget, to be proposed once measured
  ([ADR-0175](0175-the-quality-bar.md)).
- The editor's frames run at most 60 a second, `AppConfig::max_frame_rate`'s default, against a
  frame of input to present of 4.17 ms.

Measured on 2026-11-06 on the RX 6900 XT, warm, in a headless Weston
([`measurements.md`](../measurements.md#m6cq2d-ryzen-7-5800x-and-rx-6900-xt)): the sample's zone
drawn whole in the editor in about 0.47 s, *Hunna Wood* giving control in about 0.34 s. In a
headless sway with input from `wlrctl`, the editor's input to present in motion passes one frame of
4.17 ms by about three, its frames waiting for the schedule of 60 a second.

The owner decided on 2026-11-06, one question at a time: how input reaches a program measured, the
two budgets, the editor's rate of frames and where it is built. The same day,
[ADR-0190](0190-uwuis-requirements.md) made uwui the owner's main project and paused Stage 6c after
M6c.Q2e.

## Decision

**Input given in a headless sway.** sway, a compositor of wlroots, runs without a screen, its own
runtime directory, no Xwayland, its output at 240 Hz, the high tier's display, and takes virtual
input. `wlrctl` gives it a person's pointer, buttons, wheel and keys, through wlroots' protocols of
virtual pointers and keyboards; they reach the window through the compositor as a person's do.
`scripts/input-to-present.sh` runs the editor and *Hunna Wood* twice each: inputs apart, every
100 ms; then inputs in a row, about 180 a second, as a mouse moves; each run's trace read apart by
`scripts/frame-pacing.py`. Nothing reaches the desktop's session. sway and `wlrctl` enter the
development shell.

**A level opened** is within 300 ms: the editor, warm, from its launch to the sample's zone drawn
whole, every mesh loaded, on the high tier. Met by no editor yet: debt, which pushes it ahead of the
best editors.

**A game to control** is within 1 s: a game, warm, from its launch to the first frame that gives
the player control, on its tier's machine.

**The editor's rate of frames.** The editor and applications draw at the display's rate, 240
frames a second on the high tier's display: on Wayland, by the compositor's signal, which already
follows the display ([ADR-0187](0187-a-hidden-window-at-rest.md)); on Windows and X11, which give
no such signal, by the refresh rate of the display that holds the window, read again when the window
moves to another. The rule's design, in an ADR that amends [ADR-0021](0021-frame-pacing.md), comes
with uwui's desktop host at Stage 6d, for applications. M6c.Q2f applies it to the editor once Stage
6c resumes, and measures input to present, variable refresh rates and rest again; M6c.Q2d's figures
stand as those before it.

## Alternatives considered

- **Input to present measured by hand alone**: the owner's compositor and gestures, but at every
  milestone that touches the pacing, by the owner.
- **Input given to the desktop's session**, through the kernel's input devices (`ydotool`): every
  session's input, which an agent never gives.
- **A level opened within 500 ms or 1 s**: met today, but the owner chose a bar that pushes the
  editor ahead of the best.
- **A game to control within 500 ms or 300 ms**: closer to the figure measured, but a richer zone
  would pass them before streaming comes, at Stage 10a; 300 ms is passed today.
- **The editor's rate raised to 240 everywhere, the display not read**: the simplest, but on
  Windows and X11 at 60 Hz, three images of four drawn for nothing.
- **The editor's rate built within M6c.Q2d, or before M6c.Q2e**: the editor within its budget
  before the pause, but a milestone on an editor that then rests, and the rule designed twice, once
  more in uwui's desktop host.

## Consequences

- Input to present is measured on the owner's machine by an agent, in a headless sway, at each
  milestone that touches the pacing; a person's path through the desktop's compositor and display
  is not.
- The editor's input to present in motion, its start and a level opened pass their budgets: debt
  ([`debt.md`](../debt.md)), the first through Stage 6c's pause.
- A game's budgets are judged on the owner's machine until its tier's is measured.
- Variable refresh rates stay to be judged at a rate that does not divide the display's refresh:
  at 60 frames a second, on displays at 180 and 240 Hz, a display that does not follow the frames
  paces them the same.
