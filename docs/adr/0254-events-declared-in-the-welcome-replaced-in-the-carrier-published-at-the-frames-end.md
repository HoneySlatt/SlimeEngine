# 0254. Events declared in the welcome, replaced in the carrier, published at the frame's end; Play waits on one queue

- Status: Accepted; amends [0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
  (`play/time` holds `held` too; how a family declares its events) and
  [0164](0164-play.md) (Play follows game time by events, not by a question after each control)
- Date: 2026-11-13

## Context

[ADR-0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)
decided the events' rules:
- the core's requests `subscribe` and `unsubscribe` name events, `family/name`;
- an event tells a state: its current value comes after the answer to `subscribe`, then each
  change;
- a newer value replaces one not yet written, and nothing comes after the answer to
  `unsubscribe`;
- the program builds no event that no connection asked for, and the first is `play/time`.

It left M6d.9a1b2 the following:
- how a family declares its events;
- what `play/time` holds;
- where the replacement is kept;
- when the game publishes;
- how the editor's Play waits for events, controls, builds and the game's end with the standard
  library alone.

The facts, read in the repository on 2026-11-13:

- Play's thread asks the game how its time stands every 200 ms (`POLL`), and every 5 ms while it
  measures the time to control (`MEASURE`). The answer also tells it that a build ended or the
  game ended. Play measures the time to control by the first state whose `held` is false.
- *Hunna Wood* releases `held` in its presentation's `render`, after `host.frame`. It changes
  pause and speed from its input and its overlay too. A game that draws on demand may rest after
  the frame that changed them.
- The writer of a connection holds the core's session while it encodes an answer, up to 16 MiB.
  The game's thread never waits for that lock (ADR-0250, "The threads").
- Rust's standard library creates its sockets non-inheritable on Linux (`SOCK_CLOEXEC`,
  `accept4`) and on Windows (`WSA_FLAG_NO_HANDLE_INHERIT`). Read in its sources, version 1.99, on
  2026-11-13; Windows' `accept` was not checked.
- Nothing has been released since v0.6.0. Core 2 came after it.

A fresh agent attacked the design before it went to the owner. It found three defects:
- a value dropped as equal to the last written, when compared as published, can leave the tool on
  an older one;
- `held` changes after the frame's start, so a publication there misses it at rest;
- a connection that closes does not prove that the game ended.

## Decision

The owner's decisions of 2026-11-13.

**A family's events are declared by the program, and listed in the welcome.** The program gives
its session the events of each family it serves. The session checks this declaration as it
starts. The welcome lists the events of the families served on the connection, in a field that
an older welcome lacks, read as none. The session refuses:
- a name that a served family does not declare, with `BAD_PARAMS`;
- a name of a family not served, with `NOT_SERVED`.

The tool's end refuses a name that the welcome does not list.

**`play/time` holds `paused`, `speed` and `held`.** Play measures the time to control by the first
`play/time` whose `held` is false, to within the game's frame.

**The replacement lives in the carrier**, as the bounds of memory do
([ADR-0252](0252-the-channels-bounds-of-memory-kept-by-its-carrier-until-a-second-one.md)), until
a second carrier shows what they share. The core's session checks `subscribe` and `unsubscribe`
and answers them on the reader's thread. Each stays in flight like any request. The carrier holds
the subscriptions, one home, each with a cell: the value not yet written, and the last written.

- A subscription is *armed* when the writer takes the answer to `subscribe`. Values reach its
  cell only from then on, so its first event follows the answer. Arming forgets the last value
  written, so that a second `subscribe` sends the current value again.
- It is *disarmed* when the writer takes the answer to `unsubscribe`, which clears its cell under
  the same lock. A `subscribe` or `unsubscribe` cancelled before its answer is taken is answered
  `CANCELLED`, and changes nothing.
- A value published always replaces the one waiting. The writer compares it with the last
  written when it takes it, and writes nothing when they are equal.
- Between two items of the outbox, the writer writes each cell that waits, once. An event waits
  behind at most the one message being written, and no answer starves. Events and answers keep no
  order between them otherwise.
- A connection that closes, or whose reader finished, drops its subscriptions with it.
- After the game stopped, a `subscribe` is answered as any request is then.
- Every carrier keeps the same rule, as the core's documentation states it: a newer value replaces
  one not yet handed to the connection.

**The game publishes once at the end of each frame**, after its presentation, with a window or
without. It builds `play/time` only while a subscription is armed, and only when the value changed
or a subscription was armed since the last publication. No frame runs for an event.

**Play waits on one queue**, which receives:
- its controls;
- the game's events, from the client's reader thread;
- the connection's end;
- a build's end, from the thread that reads cargo;
- the editor's end, which `Player`'s drop posts.

Each input that comes from a start of the game carries that start's number, and Play drops those
of a start before. The time to control is stamped where the event is read, not where Play takes
it. `POLL` and `MEASURE` go. Play still wakes each second for the game's sources.

**A closed connection is the fast way to see the game's end, and never proof of it.** Play checks
the game's process at its wake of each second, and kills no game on a closed connection alone.

**A refused control, or one that got no answer, sends no event.** Play first takes the events that
wait, then tells the editor's toolbar the last value it received, rather than asking again.

**The versions stay**: core 2, which ADR-0250 counted `subscribe` in, released nowhere, and `play`
1. No end misreads the other:
- a program of before answers `subscribe` with `NO_METHOD`;
- a tool of after reads a welcome of before as declaring no events.

M6d.9a1b3 raises the families' versions with the cursor.

## Alternatives considered

- **Events known by a fault**, the welcome unchanged: smaller, but a tool learns what a program
  sends only by a failed request, and no version tells it either.
- **`play/time` as pause and speed alone**, ADR-0250's words: Play would ask every 5 ms while it
  measures the time to control.
- **The cells in the core's session**, shared by every carrier: the game's thread would need a
  second lock or type beside the session's, an abstraction of one user.
- **A value compared with the last written as it is published**: the defect above.
- **A publication at the frame's start**, with `waiting` asking a frame more while the value
  differs: a whole frame run, at 4K, for three fields, and four call sites changed.
- **Waiting on the game's process**, pidfd and `WaitForSingleObject`: exact and immediate, but
  `rustix` and `windows-sys` given to `slime-editor`, with code for each platform. The end shows
  within a second at worst instead.
- **The state read once after a refused control**, M6d.9a1b2's first card: a round trip and a
  second source of truth beside the events, which tell every change already.
- **Core 3 and `play` 2**: a refusal at the hello, which protects nobody while nothing is
  released.

## Consequences

- An event waits behind at most one message on its connection: a 4K capture, some 33 MB, and at
  worst an image of 1 GiB.
- A tool that never reads holds at most one value per armed event, uncounted in the bytes
  unwritten.
- A played game that does not declare `play/time`, as one of an older engine, is told by its
  welcome. Play then ends it with a message that says so.
- The toolbar follows another tool's pause at once.
- A paused game that Play plays no longer runs frames for Play's questions.
- uwui's MCP library (M6d.9a2) may show the welcome's events to an agent.
