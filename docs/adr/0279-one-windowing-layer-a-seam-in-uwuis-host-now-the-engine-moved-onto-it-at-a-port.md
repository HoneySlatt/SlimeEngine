# 0279. One windowing layer: a seam in uwui's host now, the engine moved onto it at a port

- Status: Accepted; amends [0181](0181-uwuis-scope.md) (the engine's own platform layer over winit,
  beside uwui's) and
  [0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)
  (the engine on winit until M6d.11, when the two part)
- Date: 2026-11-16

## Context

uwui's desktop host, `uwui-desktop`, speaks each platform itself
([ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)).
The engine keeps `slime-platform` on winit ([ADR-0011](0011-windowing-with-winit.md)), and ADR-0270
leaves the two apart after the move. The facts read on 2026-11-16:

- [ADR-0181](0181-uwuis-scope.md) kept two layers while both used winit; uwui's no longer does.
- `slime-platform` holds 2,411 lines, 696 of them tests: one window, keys, the mouse, input
  methods, raw motion, a held cursor, the clipboard, a file's dialog, and AccessKit through
  `accesskit_winit`. winit appears in no other crate's code.
- winit gives no event's time, no frame's time and no resume. The engine's frames are paced at the
  monitor's rate outside Wayland ([ADR-0271](0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md)),
  and its budgets of pacing and of input to present ask for these times
  ([`ROADMAP.md`](../ROADMAP.md#budgets)).
- M6d.8 builds the clipboard, input methods and AccessKit on each of uwui's platforms: what
  `slime-platform` already has on winit, built twice.
- uwui's layer is Wayland's alone, on track B's branch; X11, Win32 and AppKit are not written. Its
  only public entry is `run`, which hosts a whole interface and never returns. Of the Wayland
  layer's 3,066 lines, about 2,252 touch only uwui's input, its time, `slime-input` and the core's
  clock; the ties to the session, the pacing and the device sit in one file of 814 lines.
- The engine owns its main loop ([ADR-0010](0010-main-loop-ownership.md)) and lends its device
  ([ADR-0244](0244-the-engine-lends-its-device-through-a-guarded-door.md)): it cannot run as an
  application of uwui's host.
- The editor holds the cursor and reads raw motion, as games do.

## Decision

The owner's decision of 2026-11-16.

**One windowing layer is the target:** uwui's, under the engine's editor and games as under uwui's
applications. Reached in three steps.

**A seam inside `uwui-desktop` now, public nowhere.** The part of M6d.7a2 after M6d.7a2a cuts the
host in two: a platform layer, which gives events and takes requests and knows neither uwui's
interface nor the GPU's device, and the host above it. A `cargo tree` check keeps `uwui` and wgpu
out of the layer. X11's layer, at M6d.7b, is written against the seam, its second user.

**A blank window on the layer alone,** at the end of M6d.7a2, measured in one run beside winit's:
the start, memory, the binary's size, rest, and input to present.

**The engine moved onto it at a port** ([ADR-0277](0277-the-engine-ported-to-uwui-at-the-end-of-each-of-uwuis-stages.md)),
before uwui's 1.0, so that the engine tries the layer before semantic versioning freezes it.

- The layer then becomes a crate of its own, versioned apart from uwui after uwui's 1.0.
- `slime-platform` keeps the engine's API and becomes a thin adapter on the layer; consoles come
  through private layers under the same API.
- Each platform moves when it passes the engine's windowed lifecycle tests and its budget of
  pacing, Linux first; winit leaves the engine with the last.
- The held cursor and raw motion go into the layer, since the editor needs them. What only games
  need, such as display modes, stays in `slime-platform`, built when a game asks.
- Code that moves from `slime-platform` into uwui passes under MIT OR Apache-2.0 at that port,
  with the owner's go.

## Alternatives considered

- **Two layers for good:** nothing to change, but each platform's behaviour built twice, and the
  editor, uwui's most demanding client, paced worse than uwui's applications.
- **The layer made public now, for the engine:** one layer sooner, but an API drawn from Wayland
  alone, which Win32's modal loops and AppKit's run loop would break, for a user that comes at a
  port only.
- **The engine as an application of uwui's host:** the host owning the loop and opening its device,
  against ADR-0010 and ADR-0244.

## Consequences

- M6d.7a2's rest takes the seam and the blank window; M6d.7b's X11 follows the seam.
- `uwui::time` and what else the layer needs of `uwui` move below it.
- The engine keeps winit's debt until its port; its items in [`debt.md`](../debt.md) wait for it.
- The port that moves the engine takes M6d.8's clipboard, input methods and AccessKit, and drops
  `slime-platform`'s.

## Validation and reconsideration

Counted: `slime-platform`'s lines and functions, the Wayland layer's lines by what they touch, the
uses of winit outside `slime-platform`. Read: ADR-0010, ADR-0244, ADR-0270, ADR-0271. Assumed: that
Win32 and AppKit fit a layer that gives events and takes requests, and that the seam costs one
file now against four after M6d.7d.

Proposing another decision is justified if the blank window on the layer misses winit's figures,
or if a platform's layer cannot keep the seam without the host's interface or device.
