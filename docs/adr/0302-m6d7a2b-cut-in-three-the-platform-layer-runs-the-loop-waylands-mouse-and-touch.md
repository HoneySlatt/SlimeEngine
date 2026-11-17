# 0302. M6d.7a2b cut in three; the platform layer runs the loop; Wayland's mouse and touch

- Status: Accepted
- Date: 2026-11-17

## Context

M6d.7a2b, "the host made robust" ([`stage-6d.md`](../stage-6d.md#m6d7a2b--the-host-made-robust)),
was cut out of M6d.7a2 by [ADR-0273](0273-m6d7a2-cut-in-three-waylands-keyboard-cursor-scale-and-activation.md)
with two items: a lost device and a lost surface made again, and the crash report with the
session's marker. Three more came since: the rule of panics
([ADR-0298](0298-a-panic-ends-a-run-without-stop-the-same-rule-in-both-hosts.md)), the seam inside
the host ([ADR-0279](0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md)),
and each pointer's identity and device in the host's events
([ADR-0285](0285-every-pointer-named-in-the-hosts-events-the-tree-following-each-with-touch.md)).
Four items of [`debt.md`](../debt.md) wait for it too: an image drawn in part, a termination asked
twice, what a panic leaves, and who makes a program's images again after a lost device.

The facts, read in the repository on 2026-11-17:

- A `cargo tree` check can keep only a crate, not a module: the seam needs the layer in a crate of
  its own. It takes about 3,300 lines of `uwui-desktop`: `wayland/protocol.rs`, `repeat.rs`,
  `wait.rs`, `xkb.rs`, `clock.rs` and part of `wayland/mod.rs`. `uwui::time`, 260 lines, which the
  layer reads, refers to the rest of `uwui` in its rustdoc alone.
- The host's three pointer events, `Input::PointerMoved`, `PointerLeft` and `PointerButton`, are
  written 52 times in the workspace, in `uwui`, its tests, `uwui-desktop` and `slime-app`'s bridge.
- Wayland gives touch through `wl_touch`, part of its core protocol. A stylus comes through
  `zwp_tablet_v2`, which `wayland-protocols`' feature `unstable` already builds. No compositor of
  the series injects fingers or a stylus.
- Win32 runs a modal loop of its own while a window is moved or resized, and calls the window's
  procedure from inside it. AppKit's run loop never returns to its caller. Wayland and X11 leave
  their loop to the program. winit, Zed's GPUI and SDL3 each run the platform's loop and call the
  program back.

## Decision

The owner's decisions of 2026-11-17.

**M6d.7a2b is cut in three,** each part in a session of its own, the seam first, so that the code
that makes the host robust is written once, in its final place:

- **M6d.7a2b1 — The seam and the pointers.** The platform layer in a crate of its own, which knows
  neither uwui's interface nor the GPU's device, kept so by a `cargo tree` check; `uwui::time` in a
  crate below `uwui` and the layer; each pointer's identity and device in the host's events,
  filled by Wayland's layer and the engine's bridge.
- **M6d.7a2b2 — A lost device and a lost surface made again,** within
  [ADR-0264](0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md)'s
  budget, each lost on purpose by a door of the tests; an image drawn in part not presented; who
  makes a program's images again after a lost device.
- **M6d.7a2b3 — Crashes and panics.** The crash report and the session's marker
  ([ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md));
  ADR-0298's rule of panics; a termination asked twice.

M6d.7a2b's "Done when" is met at M6d.7a2b3.

**The platform layer runs the loop and calls the host.** The layer waits for its platform, and
calls the host above it at each event, at each frame's signal, and before it waits again, when the
host gives the deadline of its next wake. Requests go down as calls on the layer during those
callbacks. This shape fits Win32's modal loops and AppKit's run loop, which never hand the loop to
their caller, as well as Wayland's and X11's.

**Wayland's layer translates the mouse and touch.** The shape of ADR-0285 is built whole in `uwui`:
the mouse, a finger, a stylus with its pressure and tilt. Wayland's layer fills the mouse, pointer
0, and each finger of `wl_touch`. A stylus's translation through `zwp_tablet_v2` comes with uwui's
first stage, whose milestone follows each pointer.

## Alternatives considered

On the cut:

- **Three parts, the robustness first and the seam last:** what protects a person comes earlier;
  but the seam then moves the code just written, and the largest change of the host comes just
  before M6d.7a2c measures it.
- **Two parts, the seam then all the robustness:** one session fewer, but a second part as large as
  the card that its own start already found heavy.
- **Whole, in one session:** no ADR of a cut, but too much code for one closing review, and a
  session that stops halfway.

On the loop:

- **The host runs the loop and pulls from the layer:** the shape of today's code, the least work for
  Wayland and X11; but Win32 and AppKit cannot return to the host, and the seam would be turned at
  M6d.7c, Wayland's and X11's layers with it.

On the pointers:

- **The mouse, touch and a stylus now:** all that the shape holds, translated at once; some 300
  lines more, a stylus's pressure read by nothing before uwui's first stage, tested by unit tests
  alone.
- **The mouse alone:** the least; but the tree's rule of the first finger down has no source in the
  desktop host before uwui's first stage.

## Consequences

- Stage 6d grows by two sessions.
- The workspace gains two crates in M6d.7a2b1: the platform layer, and `uwui::time` below `uwui`,
  which `uwui` gives again at the same path. Their names are provisional until M6d.10a names
  uwui's crates.
- X11's layer at M6d.7b, Win32's at M6d.7c and AppKit's at M6d.7d are written against the seam.
- A finger on a touch screen reaches an application on Wayland from M6d.7a2b1; a stylus reaches it
  only where the compositor turns it into a mouse, until uwui's first stage. Touch is checked by unit tests
  alone: no compositor of the series injects it.

## Validation and reconsideration

Counted: the layer's lines and the uses of the pointer events. Read: ADR-0279, ADR-0285, ADR-0298,
the Wayland layer's code. Assumed: that Win32's and AppKit's layers fit a handler called by the
layer, checked as M6d.7c and M6d.7d write them. Proposing another decision is justified if a
platform's layer cannot run its loop and call the host without the host's interface or device.
