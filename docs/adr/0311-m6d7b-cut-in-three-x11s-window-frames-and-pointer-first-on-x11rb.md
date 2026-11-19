# 0311. M6d.7b cut in three, X11's window, frames and pointer first, on x11rb

- Status: Accepted
- Date: 2026-11-19

## Context

M6d.7b, "X11" ([`stage-6d.md`](../stage-6d.md#m6d7b--x11)), is the third part of M6d.7
([ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)).
Its card holds X11's whole layer, `x11rb`'s window, XKB, XInput2, Present's notices and RandR's
rate, and Linux's notice of a resume, logind's `PrepareForSleep`, carried to uwui.

The facts, read in the repository on 2026-11-19:

- Wayland's layer took four sessions: M6d.7a1, then M6d.7a2's three parts
  ([ADR-0273](0273-m6d7a2-cut-in-three-waylands-keyboard-cursor-scale-and-activation.md)). X11's
  shares the core, the keyboard's code of libxkbcommon (`crates/uwui-windowing/src/xkb.rs`) and the
  host's pacing, not its protocol: the window, its frames, the pointer, the keyboard, the cursor,
  the scale and touch are each written again.
- The host's tests at rest read the protocol that `WAYLAND_DEBUG` logs: on X11 they need another
  way to see the images presented and the signals awaited.
- A resume's notice is no part of X11: it is Linux's, for both layers, and reaches uwui only
  through its lifecycle ([ADR-0264](0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md)),
  which gives the focus alone today.
- `x11rb` 0.13.2 and `x11rb-protocol` 0.13.2 (MIT OR Apache-2.0) are in the lockfile, pulled by
  winit with libxcb loaded at run time, XInput2, XKB and RandR; `xcursor` 0.3.11 too, and
  `xkbcommon-dl` 0.4.2 with its `x11` feature. Present's protocol is a feature of the same crates.
- Vulkan's drivers present on X11 through a connection of libxcb, which the window's raw handle
  names.

## Decision

The owner's decisions of 2026-11-19.

**M6d.7b is cut in three,** each part in a session of its own:

- **M6d.7b1 — X11's window, frames and pointer.** The layer on `x11rb`, chosen at the start where
  no Wayland compositor answers; the window and its requests; frames on Present's notices, the
  display's period from RandR, a timer where Present is missing; the pointer through XInput2 with
  its times; the example at rest under Xvfb in the series' job.
- **M6d.7b2 — X11's keyboard, cursor, scale and touch.** XKB through libxkbcommon, its code shared
  with Wayland's layer; the cursor's shapes; the scale; touch through XInput2; typing in the
  series' job on X11.
- **M6d.7b3 — A resume's notice on Linux.** The system's notice that it resumed, on both layers,
  carried to uwui; its source weighed at its start.

M6d.7b's "Done when" is met at M6d.7b3.

**`x11rb` 0.13.2 speaks X11,** as ADR-0270 chose, at the version that winit pulls, with features
that add no crate to the lockfile: libxcb loaded at run time (`dl-libxcb`), whose connection the
window's raw handle gives Vulkan's driver; XInput2, RandR and Present. M6d.7b2's dependencies are
put to the owner at its start.

## Alternatives considered

- **Two parts, the keyboard, cursor, scale, touch and the resume together in the second:** one
  session less, but a second part as large as the first, at risk of stopping midway.
- **Whole, in one session:** no ADR of a cut, but four sessions' work on Wayland for one closing
  review.
- **Xlib through `x11-dl`, as winit's X11 backend:** a new direct crate, an older interface whose
  events are decoded in C, and a library that ends the process when the connection breaks
  ([`debt.md`](../debt.md)).

## Consequences

- M6d.7b1, M6d.7b2 and M6d.7b3 replace M6d.7b in [`stage-6d.md`](../stage-6d.md).
- `uwui-windowing` depends on `x11rb` on Linux; [`dependencies.md`](../dependencies.md) records it.
- Until M6d.7b2, a window on X11 takes no keys and draws at a scale of 1.
