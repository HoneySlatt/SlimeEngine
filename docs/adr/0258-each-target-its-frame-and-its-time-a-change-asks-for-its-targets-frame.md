# 0258. Each target its frame and its time; a change asks for its target's frame

- Status: Accepted; amends [0251](0251-the-engines-host-clock-and-its-times.md) (one frame's time
  for every root) and
  [0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md)
  (one frame settling every target, `Ui::display_list`)
- Date: 2026-11-14

## Context

[ADR-0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md)
cut M6d.5c3 in two. M6d.5c3a gave each target its input and outputs. Its second half, M6d.5c3b
([`PLAN.md`](../PLAN.md#m6d5c3b--frames-and-times-per-target)), gives each target its own frame,
its frame's time and its next frame due. [ADR-0227](0227-the-time-that-motion-reads.md) asks for
one time for each window, read by every root drawn into it.
[ADR-0251](0251-the-engines-host-clock-and-its-times.md) left one frame's time serving every root
until then.

The facts, read in the repository on 2026-11-14:

- One `Ui::frame` settles the whole interface. It restyles every marked element, steps every
  motion, redraws every spinner, settles every target's rest and input, the notifications, the
  palette, the picker and the layers. It lays out every root that changed, every list and every
  scroll, then paints every root. It gives `TargetId::MAIN`'s display list, and
  `Ui::display_list` gathers another target's.
- The interface's time, `now`, is the latest time given, of a frame or an event. Host-clock motions
  start and step on it, as do the waiting bar's phase and a root without a clock of its own.
  Deadlines read it too: the pointer's rest, the notifications and `Ui::next_frame_due`.
- What waits to be drawn is a flag on each element. `Ui::needs_frame` counts only motions,
  spinners and builders asked. That is enough for the engine, which draws after every event, but
  not for a host of two windows: a change on one window, made by a build, a scroll, or the other
  window's frame closing one of its layers, asks for no frame there.
- `Ui::frame` has about 470 direct callers, `Ui::set_frame_shown` 38, `Ui::next_frame_due` 16,
  `Ui::needs_frame` 9, `Ui::animating` 14. Two of the frames are in track A's
  `slime-app/src/channel.rs`, whose mechanical change ADR-0256 allows.

A challenge of a first design found the faults that the alternatives below record.

## Decision

The owner's decision of 2026-11-14: input to present is measured on the GPU at the end of
M6d.5c3b. Then the lead's decisions, within ADR-0256's.

**A frame for one target.** `Ui::frame(target)` restyles, steps, lays out, places, paints and
gathers that target's roots alone. It gives that target's display list. `Ui::display_list` is
gone. A frame of a target that the interface does not know, or without roots, gives an empty list.

- It settles that target's rest, input, layers opened by the keyboard and layers that follow an
  anchor.
- The notifications settle in `TargetId::MAIN`'s frame, with or without roots there. The palette,
  the sheet of shortcuts and the picker settle in the frame of their own target.
- Two things stay global, run by any frame: the layers' settings, which close a layer whose anchor
  went and move one placed over another target's root, and the cleanup of builders asked.
- What another target holds waits for that target's frame: its marked styles, its motions, its
  spinners, its lists, its scrolls.

**Each element knows its target.** It takes its parent's target as it is made, a root its
settings' target. A root or a layer moved to another target moves its elements with it. Every
filter of a frame reads an element's target at once, rather than walking up to its root.

**Each target its frame's time.** `Ui::set_frame_shown(target, shown)` gives the time at which
that target's next frame shows.

- The interface keeps it for that target from the first time given, before the target's first root
  if the host gives it first. The interface keeps these times apart from the targets' input, which
  [ADR-0257](0257-targets-known-from-their-first-root-a-moved-root-below-its-new-targets-modal-root.md)
  makes with a target's first root.
- A target's time never goes back.
- `now` stays the latest time given, of any target or event.
- A target never given a time reads `now`.
- Its host-clock motions, its waiting bars and its roots without a clock of their own read its
  time. Each reader names the target it reads: no target is "loaded" during a frame.
- Deadlines read `now`: the pointer's rest, the notifications, the next frame due. They are times
  on the host's clock, which the host compares with its own.
- `FrameStats::shown` tells the time given for that frame, as before.

**A change asks for its target's frame.** Each target keeps whether it holds a change that a frame
would draw. Marking an element's style, layout or drawing sets it on the element's target. So do a
list's rows, a scroll, a root added, removed or moved, and a builder asked. That target's frame
clears it.

- `Ui::needs_frame(target)` is true while the target holds such a change, a motion, a spinner or a
  builder asked.
- `Ui::animating(target)` reads its motions and spinners.
- `Ui::next_frame_due(target)` reads its pointer's rest, and the notifications for
  `TargetId::MAIN`.

**A root or a layer moved to another target ends its transitions at once.** Its motions were
placed on the old target's time, which the new target's frames do not read. A host moves a root
when a panel is torn off into a window, so a transition cut short by one frame does not show.

**The frame's statistics are the target's.** `FrameStats` counts what that target's frame styled,
laid out, painted and visited, and whether it animates. `elements` is the whole tree. `built`
counts the builders that ran since the last frame of any target.

**What a host reads.** It reads every target's outputs after each step, since one target's frame
may change another's input method or ask for the clipboard. The meaning's changes after one
target's frame may list another target's new elements before that target's frame styles and lays
them out. Its own frame marks them again. M6d.5d gives each window its own tree.

**The engine** gives `TargetId::MAIN`'s time and reads its next frame due and need of a frame. Its
behaviour does not change.

## Alternatives considered

- **A target "loaded" for the time of a frame**, the times read through it. ADR-0256 refuses a
  loaded target. A reader that appears outside a frame would read the wrong time.
- **Times kept in each target's input record.** A time given before the target's first root would
  be lost, and the first frame of a new window would read another window's time.
- **A new target starting at `now`.** It would read another window's time, then hold its own
  earlier times back, its motions frozen for the gap.
- **Hosts drawing every target at each step**, without a need of a frame for each target. Every
  window would be drawn when one changes, against the quality bar: nothing draws while nothing
  changes.
- **An element's target found by walking to its root** at each filter. A change of look restyles
  about 12,500 elements of the editor; with two targets, each pass would walk up to about 60,000
  steps.
- **Transitions carried to the new target's time**, each keeping how far it had gone, as a root's
  clock that goes back does. That would mean more code to keep a frame of motion when a panel is
  torn off.
- **The notifications settled only where `TargetId::MAIN` has roots.** An ended notification would
  stay due at once, and the host would draw without end.

## Consequences

- `Ui::frame`, `Ui::set_frame_shown`, `Ui::needs_frame`, `Ui::animating` and `Ui::next_frame_due`
  take a target. Every caller in the workspace gives `TargetId::MAIN`, track A's `channel.rs`
  included.
- Each element stores its target.
- The interface keeps a small table of targets' frames: each one's time and whether it holds a
  change.
- A host of two windows draws each one only when it changes, at its own time.
- A frame standing still allocates as before. The filters cost nothing on one target: when every
  root is on the frame's target, nothing is filtered.
- `docs/debt.md`'s items on one frame for every target and one time for every root are settled,
  but not their other points.
