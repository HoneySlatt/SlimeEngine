# 0309. Input to shown measured beside input to present; in motion, an input reaches the next image

- Status: Accepted; supersedes in part [0175](0175-the-quality-bar.md) and
  [0189](0189-first-measures-of-time.md) (the editor's and applications' input to present), and
  [0271](0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md) (frames drawn just
  in time, and the first frame after a rest)
- Date: 2026-11-18

## Context

M6d.7a2c2 ([`stage-6d.md`](../stage-6d.md#m6d7a2c2--frames-in-step-with-the-display)) draws uwui's
desktop host's frames just in time
([ADR-0271](0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md)) and reads, on
Wayland, when each image showed (`wp_presentation`). Its "Done when" is M6d.7a2's: input to present
within 4.17 ms at the 99th percentile in a headless sway at 240 Hz, inputs in motion and at rest.
Three facts, found on 2026-11-18 before building:

- **Drawing just in time gains nothing at the end of the present.** An input in motion waits for the
  next frame's start, on the signal or just in time, about half a refresh on average either way,
  then the frame's work. What changes is the refresh that composes the image: drawn on the signal,
  the image waits at the compositor for most of a refresh; drawn just in time, it is composed at
  once. The gain, a refresh less the frame's work and its margin, about 3 ms at 240 Hz, shows only
  where the measure ends at the screen. ADR-0189's input to present ends at the present.
- **No pacing in step with the display meets 4.17 ms at the 99th percentile in motion.** While
  frames run, an input that comes just after a frame starts waits for the next: a whole refresh,
  4.17 ms at 240 Hz, then that frame's work. The floor is a refresh plus the frame's work, about
  4.4 ms on the owner's GPU. Only an image drawn at each input would pass under it, which ADR-0271
  rejected.
- **`scripts/host-cost.py` measured no input in motion on the GPU.** Its inputs in a row, 180 a
  second, come less often than the display's 240 refreshes: each found the window at rest since the
  last image's signal, and drew at once. M6d.7a2c1's figures for inputs in a row are inputs at rest.

And one on the first frame after a rest. ADR-0271 starts it at once. After a rest shorter than a
period, the next start just in time comes before the compositor composes its next refresh: a frame
started at once and one started just in time reach the same refresh, the second with the inputs that
came meanwhile.

## Decision

The owner's decisions of 2026-11-18.

**The editor's and applications' input to present**, the rule whole:

- *Input to present*: from the input's time to the end of the present of the first image after it,
  measured inside the program and written in its trace (`input to present`), as ADR-0189 defined it.
  The budget's figure.
- *Input to shown*, beside it: from the same input to when that image showed, as the platform tells
  it (Wayland's `wp_presentation`, later Present, DXGI's statistics, the display link), written in
  the trace as each image's time shown (`shown`). It shows what drawing just in time gains; a
  program whose platform tells nothing, as winit's, has none.
- *The budget at the 99th percentile*, in a headless sway at 240 Hz for uwui's host:
  - at rest, inputs that find no frame running: one frame of the tier, 4.17 ms on the high tier, as
    before;
  - in motion, inputs that come while frames run: the input reaches the next image, at most one
    refresh of the display, 4.17 ms at 240 Hz, plus the frame's work at its 99th percentile.
- The compositor and the display add time after the shown time that stays unmeasured, until the
  light sensor times input to photon ([ADR-0215](0215-the-engine-measured-against-the-market.md)).
  Games' input to present stays ADR-0215's.

**`scripts/host-cost.py` measures inputs in motion:** 400 notches of the wheel a second, more than
the display's refreshes, so that every refresh finds an input waiting; its inputs apart and in a
row stay. It gives input to present and input to shown for each, and counts the frames that missed
the refresh that their time was predicted for.

**Frames drawn just in time**, the rule whole, uwui's desktop host's:

- Where the platform tells when images show, a frame starts at the signal's time plus the display's
  period, less the frame's work and a margin, or at once if that time has passed. The work is the
  longest of the latest frames', from the start of the turn that draws them to the end of their
  present. The margin covers what the compositor spends between composing and signalling: a frame
  that waited for its start and whose image's signal comes half a period or more after the refresh
  that it aimed at, the signal before it plus the period, missed it; it widens the margin, and the
  host traces it (`missed`); frames on time in a row narrow it back; it never exceeds a period. A
  frame that did not wait for its start, a signal much later, as a hidden window's, and an image
  shown later than predicted, as where a compositor shows its images a refresh after composing
  them, say nothing of the margin. Its figures are in the rustdoc of `uwui-desktop`'s `pacing`.
- So the first frame after a rest longer than a period starts at once: an input at rest reaches the
  screen within one frame's work. After a shorter rest, it starts just in time, which reaches the
  same refresh.
- Where the platform tells no presentation, the frame starts on the signal.
- Built on Wayland at M6d.7a2c2, on each other platform at its part of M6d.7.

## Alternatives considered

On where the measure ends:

- **The budget's figure moved to the screen:** truer to what a person sees, but the bar redefined,
  its figures no longer comparable with the engine's, which cannot read presentations until
  M6c.Q2f.
- **The present alone:** nothing changes, but drawing just in time is built without a figure that
  shows its gain.

On the budget in motion:

- **4.17 ms kept in motion:** the bar unchanged, but missed by a fraction of a millisecond forever,
  a debt that no work closes.
- **The budget at rest alone, motion measured without a threshold:** simple, but the bar says
  nothing of motion, where a person feels latency most.

On the first frame after a rest:

- **At once after any rest, however short:** ADR-0271's words, a lower input to present for inputs
  that come a little apart, but the image waits at the compositor for the same refresh, and inputs
  that come meanwhile miss it; and what counts as a rest needs a rule of its own.

## Validation and reconsideration

- Measured: the gain and the budgets, in a headless sway at 240 Hz alone, which composes a late
  image when it comes and tells no period ([`measurements.md`](../measurements.md),
  [`compositors.md`](../compositors.md)). Reviewed, not measured: the margin's rule and the lags,
  on a model of compositors in `pacing`'s tests. Assumed: that a real compositor signals at
  each composition, as wlroots, Weston, KWin and Mutter do, and composes an image ready before
  the refresh at that refresh.
- A real display, sway's and niri's on the owner's machine, under a variable refresh too, is
  checked by hand, the frames that miss counted in a trace. Many there, or a margin that stays
  wide, reopen the margin's rule.
- The budget in motion is reconsidered if a platform's pacing passes under a refresh plus the
  frame's work, as one that composes at an input's commit would.

## Consequences

- ADR-0175's and ADR-0189's input to present, and ADR-0271's frames drawn just in time and first
  frame after a rest, are this ADR's for the editor and applications; the rest of each stands.
- The quality bar's row of input to present (`ROADMAP.md`) states the budget in motion; uwui's
  repository its own.
- M6d.7a2's "Done when" reads in motion as this budget.
- Inputs that come a little apart, closer than a period, wait for the frame's start just in time:
  their input to present grows by up to a refresh, under 4.17 ms; their input to shown does not.
- The engine follows at M6c.Q2f, where it reads presentations.
