# 0307. M6d.7a2c cut in two, the host measured first; the blank window on winit in a package of its own

- Status: Accepted
- Date: 2026-11-18

## Context

M6d.7a2c, "frames and the host measured"
([`stage-6d.md`](../stage-6d.md#m6d7a2c--frames-and-the-host-measured)), is the last part of
M6d.7a2 ([ADR-0273](0273-m6d7a2-cut-in-three-waylands-keyboard-cursor-scale-and-activation.md)).
Its card holds three pieces of work, each about a session's:

- the presentations' times read through `wp_presentation`, the display's period read rather than
  measured, and the time that the interface is given predicted from them
  ([ADR-0227](0227-the-time-that-motion-reads.md));
- frames drawn just in time
  ([ADR-0271](0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md)): a frame's
  work measured, its start set from the signal, the period and that work, a margin widened by a
  frame that misses;
- a *blank window*, a program that only clears its window and presents it, on the host's platform
  layer alone and one on winit
  ([ADR-0279](0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md)),
  and a tool that runs them and the host's example in turn in one compositor and measures the
  start, memory, the binary's size, rest and input to present.

The facts, read in the repository on 2026-11-18:

- The host gives the interface the time *on the grid*, on a period measured from the frame
  callbacks' gaps ([`debt.md`](../debt.md)); it reads no presentation's time.
- The host writes no trace: nothing measures its input to present today. The engine's
  `SLIME_TRACE` is `slime-app`'s, which uwui cannot depend on.
- winit 0.30 is in the workspace for `slime-platform` alone. No crate of uwui's depends on it.
  `uwui-windowing` may not depend on wgpu, even for its tests, which `scripts/check.sh` checks.
- AGENTS.md keeps wgpu's types in `slime-gfx`, `uwui-render` and `uwui-desktop`.
- Just in time is an optimisation of input to present: the method asks for its figure before and
  after.

## Decision

The owner's decisions of 2026-11-18.

**M6d.7a2c is cut in two,** each part in a session of its own, the measure first:

- **M6d.7a2c1 — The host measured.** The blank window on the platform layer, an example of
  `uwui-desktop`; the blank window on winit; the host's trace of its frames and of each input to
  the present of the image after it; the tool; the first figures, the starting point of M6d.7a2c2.
- **M6d.7a2c2 — Frames in step with the display.** `wp_presentation` read, the display's period
  read, the time given predicted from the presentations; frames drawn just in time; measured with
  M6d.7a2c1's tool before and after. M6d.7a2's "Done when" is met at M6d.7a2c2.

**The blank window on winit is a package of its own,** `uwui-blank-winit`, unpublished, under MIT
OR Apache-2.0, which leaves with uwui at M6d.11. The workspace builds it; only the measuring tool
runs it. It is the one crate of uwui's that depends on winit, and the fourth whose code holds wgpu's types: a leaf
program, whose types reach no other crate.

**The three programs write one trace,** in the Chrome Trace Event format that the engine's traces
use, into the file that `UWUI_TRACE` names: each frame, from its start to the end of its present,
with the notches of the wheel it took; and each interval from an input's time to the end of the
present of the next image. Their times count from the Unix epoch, read once with each program's
own clock and followed on it, so that the tool, which notes the time it starts a program, reads its
start from the first frame's end. The host and the blank window on its layer write it with one
file of code, the host's; the program on winit, which depends on nothing of uwui's, with its own.

An input's time in the trace is the platform's own where it gives one, placed on the host's
clock, on Wayland the compositor's to the millisecond; else its reception, as on winit, which
gives none. As the engine's trace ([ADR-0189](0189-first-measures-of-time.md)), it counts no input
before the first image, nor one across a device or a surface made again; unlike it, an input that
the interface answers with no image is not counted, so that the next image's interval is not taken
for the input's.

**The tool measures input to present from outside,** the same way for the three programs: from the
moment it sends a notch of the wheel through the compositor's virtual pointer to the end of the
present of the first image that took it, each image's notches counted in the trace. A program's own
time of an input would hide the time that the input waits behind a long frame before the program
reads it, which a blank window's short frames do not show.

## Alternatives considered

On the cut:

- **Two parts in the card's order, the frames then the measure:** follows ADR-0273's words, but
  just in time is built without a figure before it, and its gain cannot be shown.
- **Three parts, the tool, then the presentations' times, then just in time:** each part small,
  but one session more, and a middle part that changes nothing a person sees.
- **Whole, in one session:** no ADR of a cut, but three sessions' work for one closing review.

On the blank window on winit:

- **An example of `uwui-desktop`, winit among its development dependencies:** one file, but every
  build of the host's tests compiles winit, here and in uwui's repository.
- **An example of `slime-platform`, which already depends on winit:** no new edge, but uwui's
  tool would depend on the engine, and could not measure once uwui leaves at M6d.11.

On the trace's times:

- **The host's clock, `CLOCK_BOOTTIME`, in every program:** the precise clock, but winit's program
  would need `rustix` and code for each system, as Rust's `Instant` gives no absolute reading.
- **The start timed apart, each program ending after its first image,** as the engine's
  `scripts/start-times.sh` does: no absolute time, but a door to end the host's runs, and a run per
  figure where one gives all of them.

## Consequences

- M6d.7a2c1 and M6d.7a2c2 replace M6d.7a2c in [`stage-6d.md`](../stage-6d.md); M6d.7a2's "Done
  when" waits for M6d.7a2c2.
- AGENTS.md names `uwui-blank-winit` among the crates that hold wgpu's types. It is a binary
  alone, which no crate can depend on.
- The host's costs are measured twice, before and after its frames change; the tool makes the
  second run cheap.
- The trace's writer is written twice, the host's and winit's program's, a few dozen lines: the
  blank window on winit stays a program that depends on nothing of uwui's.
