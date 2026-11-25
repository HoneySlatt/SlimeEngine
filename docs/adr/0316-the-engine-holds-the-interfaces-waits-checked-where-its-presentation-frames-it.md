# 0316. The engine holds the interface's waits, checked where its presentation frames it

- Status: Accepted; supersedes in part
  [0314](0314-m6d9a2c4-cut-in-two-an-interfaces-wait-gives-the-elements-seen-as-after.md)
  (when a host counts its waits)
- Date: 2026-11-23

## Context

M6d.9a2c4b, "The engine that waits on the interface"
([`stage-6d.md`](../stage-6d.md#m6d9a2c4b--the-engine-that-waits-on-the-interface)), makes the
engine hold `interface/wait`, which uwui answers for every host
([ADR-0314](0314-m6d9a2c4-cut-in-two-an-interfaces-wait-gives-the-elements-seen-as-after.md)).
Until now, `slime-app` refused it.

The facts, read in the repository on 2026-11-23, at `31b17929`:

- `slime-app`'s channel holds the journal's waits apart from what makes frames run, and asks them
  again once each frame's simulation ran; a connection holds at most `MAX_WAITS`, 8, and a ninth
  that would wait is `BUSY`; a wait answered at once is never `BUSY`
  ([ADR-0310](0310-the-journals-wait-answered-once-the-simulation-ran-through-a-read-of-its-own-its-tool-gives-end.md)).
- `uwui::agents::Waits` checks a target's waits after its frame, and reads nothing while the
  target holds a change not framed yet. A wait held keeps a reader of meaning in the interface
  until it is met, cancelled or cleared.
- Presentations frame their interface in one of two places: the editor in `extract`, slime-devui's
  overlay, which Hunna Wood and the examples draw, in `render`. A frame that the window cannot show
  runs `extract` but not `render`.
- A run on demand, the editor's or a paused game's, runs no frame after a drawn one unless
  something asks for one: a change framed in `render` is seen only by a check after `render`.
- Answering after the frame's drawing cost 78 to 305 ms on lavapipe under a heavy load, against
  0.6 ms right after the simulation ([`measurements.md`](../measurements.md#m6d9a2c2-the-journal-that-waits)):
  the drawing's time, which grows with the load.
- The engine's window draws one target, the main one: a capture of an element on another target is
  refused.
- A request that the run drops is answered "the game has stopped" by the listener; the run answers
  so every request that it holds as it ends, a capture, a picking, a step.

A fresh agent attacked the lead's first design before it was built. It found a test seam that
needed a new dependency, no test that could fail without the check after `render`, a count that
would have made a wait met at once `BUSY` against ADR-0310, a message of the run's end of its own
beside the listener's, a wait cancelled during its frame answered with what it found, the
contract of presentations written nowhere, and documents that still said the engine refuses these
waits.

## Decision

The lead's decisions within ADR-0310 and ADR-0314.

- **`slime-app`'s channel holds the interface's waits in a `Waits`, beside the journal's,** each
  with its request. Neither is counted among what makes frames run: a run at rest with a wait in
  flight runs no frame for it.
- **The waits are checked after `extract`, and again after `render` when the frame draws.** A
  presentation frames its interface in one of the two, which `Presentation::ui` says. Each check
  costs nothing while no wait is held, and reads nothing in a frame where no element changed. The
  editor's waits are answered after `extract`, before the drawing; an overlay's after `render`, in
  the frame that draws the change. A frame that the window cannot show leaves what `render` would
  have framed, and its waits, for the next frame that draws.
- **One count of a connection's waits, both kinds together, `MAX_WAITS`,** cancelled ones not yet
  answered left out. **A host answers `BUSY` only a wait that it would hold:** the interface's is
  asked of `Waits` first, and one held past the count is cancelled there at once, its reader with
  it, then answered `BUSY`. This supersedes ADR-0314's "the host counts its waits before it asks a
  `Waits`", whose aim, no wait left held, still holds; both kinds now follow ADR-0310's rule.
- **Cancels:** a wait cancelled counts no more at once, so that a wait that comes beside another's
  cancel takes its place. The frame's start, which the cancel wakes, answers the waits cancelled,
  both kinds, and lets uwui's go. A wait cancelled later in the frame is answered `CANCELLED` when
  it is asked again or found met, as ADR-0250 says of a request that changed nothing.
- **A wait on a target other than the main one is refused,** as a capture is: no target of the
  engine closes before its run ends, so that no wait is left on a target that runs no frame.
- **The run's end lets the waits go:** before the presentation's `stop`, the channel clears its
  `Waits` with the interface, which lets their readers go, and drops the requests that wait, of
  both kinds, which the listener answers "the game has stopped", as every request that the run
  holds. After a panic, no `stop` runs: the waits go with the interface that the presentation drops.
- **The checks' time:** the replies of each of the two points take half the requests' budget,
  parted among the waits met; what uwui reads to find them met is bounded by none: a wait's first
  reading, which walks its target, and a check, which reads what changed, or the target whole once
  its marks passed their bound.

## Alternatives considered

- **One check, at the frame's end:** one point, but the editor's waits wait for the drawing.
- **One check, after `extract`:** the editor's answered before the drawing, but an overlay's change
  in a run on demand is never seen, its wait held until its tool's bound.
- **A frame asked for each wait held:** every point covered, but a run at rest woken for nothing.
- **Count first, as ADR-0314 said:** no cancel at once, but a wait met at once answered `BUSY`, a
  rule apart from the journal's for an agent that shares one connection among its tools.
- **A message of the run's end for the waits:** a test could tell the call from the drop, but one
  fact told two ways.
- **Waits on any target:** a host of several windows would need them, but the engine has one, and
  a target that closes leaves its waits held: uwui tells no target closed.

## Consequences

- An agent waits for a change of the editor's or of a game's interface through
  `wait_for_elements`; the engine's refusal of `interface/wait` goes.
- A presentation that frames its interface elsewhere than in `extract` or `render` leaves its waits
  held until their tools' bounds.
- A host that keeps its interface after a panicked run keeps the readers of its waits, and uwui
  still tells no target closed ([`debt.md`](../debt.md)).
- Two calls are untested: the windowed driver's call that lets the waits go, whose effect cannot
  be seen from outside the process, and the check after `extract`. In a frame that draws, that
  check answers the editor's waits before the drawing, about 6 ms sooner at rest, which only
  `slime-mcp`'s measure `answer` shows; in a frame that draws no image, a window minimized on
  Windows, covered, or whose surface is given up, it is the only check, and answers the editor's
  waits alone. The channel's own tests call both functions ([`debt.md`](../debt.md)).

## Validation and reconsideration

Read: `slime-app`'s channel and frame, `Presentation::ui`, `uwui::agents::Waits`, the editor's and
slime-devui's framing, `slime-mcp`'s tests `edit_window`, `drive` and `answer`. Proposing another
decision is justified if a presentation needs to frame its interface elsewhere, if the engine
draws more than one target, or if the two checks cost a measured share of a frame.
