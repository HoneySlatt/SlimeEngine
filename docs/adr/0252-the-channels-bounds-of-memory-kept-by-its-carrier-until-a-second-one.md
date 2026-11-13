# 0252. The channel's bounds of memory kept by its carrier until a second carrier; a place freed as its answer leaves; the budget measured by a run's frames

- Status: Accepted; amends [0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
  (where the bounds are kept, when a place frees, how the budget is measured)
- Date: 2026-11-13

## Context

[ADR-0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
says that the bounds of the requests in flight are kept by the core's session, so that a carrier
without threads, of the web or Android, keeps them too. M6d.9a1b1 built them, and found that they
are of two kinds:

- The requests in flight on a connection, their count and the bytes of their lines: what the
  welcome states and what a tool must respect. One session sees them whole.
- The game's memory: the bytes that a connection's writer has not written, the same for every
  connection together, and the connections welcomed. They are counted across every session of
  the program, and by the carrier that writes; no session sees them.

`slime-channel`'s listener is the only carrier today. The next, `uwui dev`'s connection out of an
application, comes at uwui's first stage.

Two more facts came of the milestone's reviews and measures:

- ADR-0250 frees a request's place once its answer is written. The tool frees its own once it
  reads the answer, and may send again at once: on the loopback, the write wakes the tool before
  the program says that it wrote, and a tool within its bounds is then answered `BUSY`.
- ADR-0250 sets the budget of a frame's start by the time from input to present with an agent at
  the bounds. `scripts/input-to-present.sh` gives a person's input to the editor and the game,
  started without a channel; giving it an agent would change a file of track B's.

## Decision

The owner's decisions of 2026-11-13 on the bounds and the budget's measure; the place's rule is
the milestone's, which its closing review asked for.

- The core's session keeps the requests in flight, as ADR-0250 says.
- The bounds of the game's memory are kept by `slime-channel`'s listener while it is the only
  carrier: the bytes unwritten of a connection and of all, and the connections welcomed. Their
  figures are in its rustdoc.
- They move into the core with the second carrier, at uwui's first stage, when two carriers show
  what they share.
- A request's place frees when the carrier takes its answer to write it, before any byte of it
  can reach the tool.
- The budget is measured by a run's frames, headless, with as many agents as the channel welcomes
  each at its bounds, with the budget and without it; input to present with an agent is measured
  later, its script given an agent.

## Alternatives considered

- **A program's count in the core now**, a type of `uwui-agent` that counts connections and bytes
  across its sessions: ADR-0250 kept to the letter, but an abstraction of one user, shaped before
  the carrier that would test it.
- **A place freed once written, the tool sending again on `BUSY`**: the wording of ADR-0250 kept,
  but a refusal and a second send for what the bounds allow.
- **Input to present with an agent now**, `scripts/input-to-present.sh` given one: the bar's own
  measure, but a file of the other track changed, and a milestone larger.

## Consequences

- A carrier of the web or Android, built before uwui's first stage, would keep these bounds again;
  none is planned before it.
- [`debt.md`](../debt.md) follows the move, and the measure of input to present with an agent.
