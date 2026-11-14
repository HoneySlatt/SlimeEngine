# 0250. Requests in flight answered in any order, cancelled with an answer, bounded in the core; events by subscription; M6d.9a1b cut in three

- Status: Accepted; settles the questions that
  [0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md) and
  [0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) left to M6d.9a1b; amends
  [0122](0122-the-development-channel.md) (a connection's requests in flight, answered in any
  order) and [0161](0161-the-open-step-and-the-handles.md) (an agent's reads answered while its
  changes wait); amended by
  [0252](0252-the-channels-bounds-of-memory-kept-by-its-carrier-until-a-second-one.md) (where the
  bounds are kept),
  [0254](0254-events-declared-in-the-welcome-replaced-in-the-carrier-published-at-the-frames-end.md)
  (`play/time` holds `held`; events declared in the welcome),
  [0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md) (M6d.9a1b3 follows M6d.5c3a) and
  [0259](0259-the-interfaces-family-in-uwui-agent-answered-by-uwui-an-elements-style-by-its-own-request.md)
  (M6d.9a1b3 cut in two; why the families' versions rise)
- Date: 2026-11-13

## Context

M6d.9a1b's card ([`stage-6d.md`](../stage-6d.md#m6d9a1b--requests-in-flight-and-events)) asks for
several requests at once, bounded, answered in an order the protocol states and cancellable; a
cursor to the rest of a long list; events to a connection that asked for them, the editor's Play
their first reader; and the interface's family moved into `uwui` behind its feature `agents`.
[ADR-0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md) left it the
order of answers and cancellation; [ADR-0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md)
gave it the design of the threads and the wake, and asked M6d.5a's ADR to fix three seams before it
starts. [ADR-0249](0249-the-hosts-api-its-items-and-their-table.md), accepted on 2026-11-13, fixed
them: one waker, the host's, which the agents' sessions ask a step through; each reader of meaning
following its own changes; an element's capture from the host's image, cropped to the element's
place, which a question gives. It runs M6d.5c, which changes `Ui`'s methods of input, beside
M6d.9a1b.

The facts, read in the repository and in the current documentation on 2026-11-12:

- A connection of the development channel has one request in flight: its thread reads a request,
  hands it to the game through an unbounded queue, blocks on the answer, writes it, then reads the
  next (`slime-channel`'s `listener.rs`). It reads nothing while it waits, so it could read no
  cancellation.
- The game drains the whole queue at each frame's start (`slime-app`'s `channel.rs`). Four kinds of
  request wait: a step until the frames ran it, a capture until the frame's image, a picking until
  the presentation answers, and the editor's changes while a person drags, kept in their order.
  Steps asked go into one count, which no request can take back.
- The welcomed connections, the game's queue and the size of what the program writes are unbounded
  ([`debt.md`](../debt.md)). Each capture copies the frame's image.
- `slime-mcp` holds one lock for the whole of each request, so an agent's calls run one at a time.
- The editor's Play asks how the played game's time stands every 200 ms.
- A list stops at 1,000 items and counts the rest; the editor's journal goes whole in one answer,
  and fails past the tool's 16 MiB line.
- JSON-RPC 2.0 matches answers to requests by number, and lets a batch's answers come in any
  order. LSP 3.18 lets a server answer out of order where it does not change the answers, and
  answers every cancelled request, advising `RequestCancelled`, −32800; DAP and gRPC answer every
  cancelled call too. MCP `2026-07-28` answers no cancelled request, lets a client interleave
  unrelated requests, replaces `resources/subscribe` with `subscriptions/listen`, a request that
  stays in flight, and pages with an opaque `cursor` and `nextCursor`, an invalid cursor faulting
  −32602. HTTP/2 bounds the streams in flight by `SETTINGS_MAX_CONCURRENT_STREAMS`, recommended no
  lower than 100. Sources: the specifications' pages, saved on 2026-11-12.
- The standard library cannot wait on a socket and a channel at once, nor on two sockets.

The design was attacked by a fresh agent before it went to the owner. It turned its order of
answers from changes kept in order to no order at all, as the editor's changes already keep theirs;
moved the bounds from the carrier into the core's session, freeing a request's place once its
answer is written; added the threads, which a cancellation needs; and cut the milestone in three
rather than two, `slime-mcp`'s lock in the first.

## Decision

The owner's decisions of 2026-11-13.

**M6d.9a1b is cut in three**, each part in a session of its own:

- M6d.9a1b1, requests in flight: the core's session counts them, bounds them and cancels them;
  `slime-channel`'s threads and client; `slime-mcp`'s calls side by side; the game's requests taken
  in turn within a budget.
- M6d.9a1b2, events: subscriptions in the core, `play/time`, the editor's Play its reader.
- M6d.9a1b3, long lists and the interface's family: the cursor, the journal paged, the family moved
  into `uwui` behind `agents` with the places of an element's own style, the build for shipping
  checked free of it. It starts once M6d.5c changed `Ui`'s methods of input, which an agent's
  action calls, so that input has one way in; where the family's messages live and how the places
  are given are put to the owner at its start.

**The order.** A program begins a connection's requests in the order they came, and answers each
when it is done, in any order, by its number. The editor's changes keep their order among
themselves; nothing else waits for another request. A tool that needs a request's effect before the
next waits for its answer.

**Cancellation.** The core's notification `cancel` names a request's number. Every request is
answered exactly once. A request that has changed nothing yet is answered with the fault
`CANCELLED`, −32800, and never applied: an edit waiting for a drag's end, a capture waiting for its
image. One that has changed something answers with its result: steps asked go into a count that no
request takes back. A cancel wakes the program, which answers it at its next frame. A connection
that closes cancels every request it had in flight. A number already in flight is refused.

**The bounds**, kept by the core's session, so that a carrier without threads, of the web or
Android, and a tool that carries several agents, keep them too. The figures are estimated, then set
by measure at M6d.9a1b1:

- At most 16 requests in flight on a connection, and 1 MiB of their lines. The welcome states both;
  the tool's session holds back what would pass them, and a request past them is answered `BUSY` at
  once, its connection kept. A request's place frees once its answer is written.
- At most 64 MiB of answers waiting to be written on a connection, 256 MiB in the program; one
  answer larger passes alone when nothing else waits. Past the bound, the program reads no more
  requests from that connection until it writes.
- The hello gives the tool's limits of reading; an answer past them is a fault that says so.
- At most 8 connections welcomed; the next hello is refused, with the reason.
- The captures of one frame share its image.

**The requests of a frame.** The host begins one request of each connection in turn, until a budget
of time, at least one round each frame; the rest wait for the next frame, which it asks for. The
budget starts at about 0.5 ms, estimated, set by the time from input to present with an agent at
the bounds. The clock is the host's; tests give their own.

**Events, by subscription.** The core's requests `subscribe` and `unsubscribe` name events,
`family/name`; the session refuses those of a family not served, or that the family does not
declare. An event tells a state: its current value comes as an event after the answer to
`subscribe`, then each change. An event not yet written is replaced by a newer one of its name, so
a slow tool never grows the program's memory; none comes after the answer to `unsubscribe`; the
program builds none that no connection asked for. Events that must all arrive, as the log's lines,
come with their first reader. The first event is `play/time`, the played game's pause and speed.

**The cursor of a long list.** An opaque text: a request gives it as `cursor`, an answer gives
`next` while more remain. A page is bounded by a count and by bytes; each list is ordered so that
resuming costs nothing: entities by their slots, elements by resuming the walk of the tree, the
journal by each entry's place in its file with a generation, the log by its lines' numbers. A
cursor that no longer holds faults −32602, and the tool starts again. The families' versions are
raised with it, so that a program of before does not serve the first page without end.

**The threads.** On the program's side, each connection has a thread that reads its requests and
cancellations, and one that writes its answers and events, both blocked without a timeout while
nothing comes. The program's thread never writes to a socket nor waits: it leaves its answers in
the connection's outbox. Each request and each cancel wakes the program through the function its
host gives, ADR-0249's waker. On the tool's side, each client has a thread that
reads, hands each answer to its request by number and each event to its owner; each request has its
own timeout, in place of the socket's.

## Alternatives considered

The cut:

- **In two**, requests in flight with events, then lists with the interface's family: a first part
  larger than M6d.9a1a, and `slime-mcp` still one request at a time.
- **One milestone**: the largest, all of it waiting for M6d.5c.

The order:

- **Answers in the order of the requests**, as HTTP/1.1's pipelining: the simplest for a tool, but
  a request that waits holds every later answer, and a read waits behind a person's drag.
- **A connection's changes kept in order**, reads passing them: a pause would wait for a drag
  because a parallel edit does; waiting changes would fill the places in flight, and every read
  would be answered `BUSY`; it gives no read of one's own writes; each host would build it again.

Cancellation:

- **No answer to a cancelled request**, as MCP: an answer already sent still arrives, and the tool
  must keep the numbers it cancelled to drop it. uwui's MCP library drops the fault instead.
- **None, timeouts only**: an edit given up would still apply.

The bounds:

- **Counts alone, in `slime-channel`**: simpler, but some 4 GB of captures waiting on 8 connections,
  and each carrier counting again.
- **None more than today**: the debt stays.

The requests of a frame:

- **All of them**, as today: the bounds cap the queue, but a busy agent drops frames while a person
  plays.
- **A count for each connection**, as ADR-0211's executor: no clock and the same each run, but blind
  to what a request costs.

Events:

- **A subscription that stays in flight**, as MCP `2026-07-28`'s `subscriptions/listen`: the same as
  MCP, but a place in flight for each, and the core bound to a revision that has just replaced its
  last way of subscribing.
- **A request of each family**, as the DevTools' `Domain.enable`: nothing in the core, but each
  family subscribing in its own way, and rights checked in each.

The cursor:

- **An offset and a count**: the simplest, but items skipped or doubled when the list changes
  between pages, and the journal read whole for each.
- **A snapshot kept for each cursor**: exact pages, but memory held and a lifetime to manage.

The threads:

- **`mio`**, one thread for every connection: fewer threads, but a dependency that the web and
  Android would not use.
- **One thread a connection waking on a timeout to write**: it runs at rest, and a blocking read that
  timed out leaves a socket unusable on Windows.
- **tokio**: against ADR-0211 and ADR-0246.

## Consequences

- The core's version is raised to 2 for `cancel`, `subscribe`, the limits in the hello and the
  welcome, and the faults `BUSY` and of an answer too large; the families' versions rise with what
  each changes. A tool of before cannot speak to a program of after, nor the reverse.
- A waiting edit of a connection that closed is no longer applied when the drag ends.
- Up to 17 threads of the channel at 8 connections, each blocked while nothing comes; each reserves
  its stack and costs some 10 to 20 µs to start, estimated.
- The figure of a request's line costing about 17 times its size in memory, which `wire.rs`, the
  debt and the card of M6d.9a1a quote, has no measure in [`measurements.md`](../measurements.md):
  M6d.9a1b1 measures it, as its bounds rest on it.
- An event waits behind an answer's bytes on the same connection: a 4K capture, some 33 MB, delays
  it.
- Play still wakes for the game's sources each second; the end of a build posts into the same queue
  as the events, so that a change of code runs again within its budget.
- uwui's MCP library (M6d.9a2) carries MCP's `notifications/cancelled` as `cancel` and drops the
  fault `CANCELLED`; MCP's list of tools fixed at the server's start remains its question.
