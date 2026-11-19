# 0269. uwui's MCP library reaches a program through its host's carrier; faults as tools' errors; changes followed by requests that wait; M6d.9a2 cut in three

- Status: Accepted; amends
  [0209](0209-platforms-the-editor-and-agents.md) (tools found on demand by the agent's client),
  [0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md) (uwui's MCP
  server as a program waits for a carrier of uwui's own),
  [0260](0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md)
  (`STALE` reaches an MCP client as a tool's error, not −32602),
  [0261](0261-each-reader-of-meaning-its-changes-each-target-its-tree-no-number-on-a-screen-readers-action-m6d5d-cut-in-two.md)
  (an agent follows the interface's changes by a request that waits, not an event) and
  [0265](0265-the-editors-journal-read-by-pages-its-number-drawn-by-the-journal-pages-of-whole-entries-on-the-editors-thread.md)
  (a page that reaches the journal's end gives a cursor there); amended by
  [0272](0272-the-carriers-families-and-its-connections-state-join-it-at-their-first-reader.md)
  (the carrier's families served and its connection's state join it at their first reader) and
  [0274](0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md) (M6d.9a2b cut in two);
  superseded in part by
  [0310](0310-the-journals-wait-answered-once-the-simulation-ran-through-a-read-of-its-own-its-tool-gives-end.md)
  (what the tools that wait give back) and
  [0314](0314-m6d9a2c4-cut-in-two-an-interfaces-wait-gives-the-elements-seen-as-after.md) (what an interface's wait sees between two calls)
- Date: 2026-11-15

## Context

M6d.9a2's card ([`stage-6d.md`](../stage-6d.md#m6d9a2--uwuis-mcp-library-and-families)) asks
for uwui's MCP library on `rmcp`, its tools by families, the interface's among them; `slime-mcp`
on it with the engine's families; one way of paging a list for every tool, `STALE` given to MCP's
clients as −32602; an event of the interface's changes that an agent subscribes to; and, the
owner's choice of 2026-11-14, an event of the editor's steps. It is done when the engine's MCP
session tests pass through the library and a session test of uwui's server drives an example.

The facts, read in the repository on 2026-11-14:

- uwui's MCP server needs a carrier on the tool's side to reach a program. The only one is
  `slime-channel`'s `Client`, the engine's: loopback, a token, `launch`, which
  [ADR-0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md) keeps in
  the engine. uwui never depends on the engine
  ([ADR-0181](0181-uwuis-scope.md)). uwui's own carrier is the program's connection out to
  `uwui dev`, at uwui's first stage.
- `slime-mcp` holds 37 tools in one file. It sends every request that is not the editor's, the
  interface's included, to the game that an editor plays, while the game's tools drive it.
- A call waits in `Pending::wait`, which takes the request's handle: nothing can cancel it from
  the MCP side.
- The carrier keeps one cell for each event's name, and a newer value replaces the one waiting
  ([ADR-0254](0254-events-declared-in-the-welcome-replaced-in-the-carrier-published-at-the-frames-end.md)).
- A page of the journal at its end gives no cursor: an agent cannot say where it stopped.
- `play/logs` pages by `after`, a line's number, and `partial`, not by `cursor` and `next`.

The field, read on 2026-11-14 (sources saved by the lead):

- MCP `2026-07-28` is the current revision. It has no session: state across calls goes in
  handles that the server gives and the agent passes back as a tool's arguments.
- Only MCP's own lists page, an invalid cursor faulting −32602. A tool's argument that fails its
  checks is a tool's error, `isError`, which clients give the model so that it corrects itself
  (SEP-1303, in the specification since `2025-11-25`; the reference TypeScript SDK does so).
- A tool list must not vary by connection, nor as a side effect of other requests; its order
  should not change.
- Claude Code and Codex give the model a tool's results alone, never a notification. Codex logs
  them, and stops a call after 60 s by default; Claude Code moves a call past 2 minutes to a
  background task, defers tools' definitions until the model looks for them, and cuts a result
  past 25,000 tokens.
- `rmcp` 3.10.0 is the latest, 3.9.0 in the workspace; 3.10.0 adds nothing that this needs. Its
  server answers both MCP's handshake of before and `2026-07-28`'s requests. A cancelled request
  cancels the handler's token, not the handler: a call given up runs on unless it watches that
  token. Routers of tools add up within one server's type; a tool of a name already taken
  replaces the first silently.

A fresh agent attacked the lead's first design before it went to the owner. It found the
carrier moved into uwui against ADR-0246; an event of the journal that showed steps twice, and one
of the interface that lost targets and went stale between an agent's calls; places kept per MCP
session, which `2026-07-28` no longer has; and the cancellation, the played game's routing, the
routers' types and a test of `2026-07-28` missing.

## Decision

The owner's decisions of 2026-11-15.

**The library reaches a program through its host's carrier.** It talks to a program through a
small interface of the core's level: send a method and its parameters, and await the answer and
the bytes after it; cancel a request; the families served; whether the connection is lost. Its
host chooses the connection of each request, so that `slime-mcp` sends what the game's tools ask
to the game that an editor plays. Two carriers serve it: the engine's, `slime-channel`'s `Client`,
wrapped in `slime-mcp`; and, in the library's tests, a session in memory answered by `uwui` from a
`Ui`, which runs on Windows too. `launch`, the token's variable and the announcement stay in
`slime-channel`, unrenamed, until M6d.9a3a designs how tools find and reach programs. uwui's MCP
server as a program of its own waits for a carrier of uwui's: the card's "a session test of uwui's
server drives an example" becomes "the library drives an interface in memory in its tests, and
the engine's examples through `slime-mcp`".

**`uwui-mcp`,** a crate of this workspace under MIT OR Apache-2.0, moved with uwui at M6d.11, its
name provisional until M6d.10 as uwui's others. It depends on `uwui-agent`, `rmcp` (3.9.0),
`tokio`, `serde`, `serde_json`, `base64` and `png`, and on `uwui` with its feature `agents` for its
tests alone. `rmcp` is under Apache-2.0 alone: no crate that an application links depends on it.

**A program's faults reach an agent as a tool's errors.** Every fault that the program answers,
`STALE` included, is a result marked `isError`, whose text the model reads; `STALE` says to list
again from the start, as `slime-mcp` says now. JSON-RPC's errors stay for a tool unknown or a call
malformed. `CANCELLED` is not passed on: MCP answers no cancelled call.

**An agent follows what changes through requests that wait.** No event of the interface's
changes, nor of the editor's steps:

- `interface/wait` names a target and a condition on its elements, a role, a name and a state,
  present or absent. It is answered at once when the condition holds, else checked at each frame's
  end on the elements that changed, through one reader of meaning for each target that the waits
  in flight share (ADR-0261's readers).
- `editing/journal` takes `wait`, answered once the journal grows past its cursor. A page that
  reaches the journal's end gives `end`, a cursor there, apart from `next`.
- Each is a request in flight as others are, bounded and cancelled as they are
  ([ADR-0250](0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md)).
  Nothing runs while none waits.
- The MCP tools that carry them wait at most a bound under 60 s, and give `after`, a handle that
  the next call passes back; the journal's is its cursor, passed through untouched. The server
  keeps nothing for an agent.

**M6d.9a2 is cut in three**, each part in a session of its own:

- M6d.9a2a, the library: `uwui-mcp`, its carrier's interface and its two carriers, the
  interface's tools, one loop of pages, faults as tools' errors, cancellation, the agent's name cut
  and tested, a session test of `2026-07-28`; `slime-mcp`'s tools of the interface through it. The
  answer through MCP is measured before, at its base commit.
- M6d.9a2b, the engine on the library: `slime-mcp`'s families as the library's, the played game's
  routing tested, the debts of `stop_game`, the error's chain and a tool's name taken twice, the
  instructions written by each family; the answer through MCP measured after.
- M6d.9a2c, waiting: `interface/wait`, the journal that waits, their tools; it settles the debt
  of an agent reading the journal again. Its parts in `uwui` and `slime-app` are timed against
  M6d.7.

**The lead's decisions within them:**

- `rmcp` stays at 3.9.0.
- Every tool that reads a list takes `cursor` and writes `next` first. `read_logs` gives `play`'s
  `after` and `partial` as a `cursor` of its own, so that `play` keeps its version.
- The tools are listed in `rmcp`'s order, by name: MCP asks only that it not change. A tool's name
  taken twice is refused when the server is built.
- The agent's name and version are cut in bytes of JSON, with `uwui_agent::page::cut`, so that a
  hello stays within its limit whatever their characters.
- A call cancelled by MCP sends the core's `cancel`, and its answer is dropped.
- Tools found on demand, as [ADR-0209](0209-platforms-the-editor-and-agents.md) asks, are the
  agent's client's work, Claude Code's search of tools and Codex's deferred servers: MCP forbids a
  list that changes with what the agent asks. Each family keeps few tools; an application's
  declared actions (M6d.9b) go through one tool that lists them and one that runs them.

## Alternatives considered

The carrier:

- **The tool's side of the desktop's carrier moved into uwui**, `launch`, the token's variable and
  the announcement renamed: uwui's server whole now, but against ADR-0246, a public rename paid
  again when M6d.9a3a reshapes listening and consent, and the program's side still the engine's.
- **Only the tool's generic end moved**, a program that takes an address and a secret: a server
  of uwui's now, but code moved before M6d.9a3a decides how tools reach programs.
- **A carrier of its own on tokio** in the library: two carriers on the tool's side.

The library:

- **A feature `mcp` of `uwui-agent`**: a crate fewer, but `rmcp` and `tokio` in the core's
  manifest, and a server in a crate that has no input or output.

Faults:

- **−32602 for `STALE`, as ADR-0260 said**: against MCP, and a client may never show the model
  the advice to list again.

Following changes:

- **Events by subscription, as the card had them**, each fixed: one value for every target, a log
  of changes kept by element and fed without making meaning, a handle bound to the process and the
  interface, the journal's `end`. They follow the card and ADR-0261, but need a new API of
  `uwui`, cost each frame while an agent subscribes, tell no element gone, and end in the same
  tools that wait.
- **A place kept by the server for each MCP session**: MCP `2026-07-28` has no session, two calls
  side by side would take each other's changes, and nothing would tell the server to forget one.

The cut:

- **In two**, the library with all of `slime-mcp` first: a first part as large as M6d.9a1b whole.
- **One milestone**: the stage's longest session.

## Consequences

- The card of M6d.9a2 gives way to three; uwui's MCP server as a program comes with a carrier of
  uwui's own, at uwui's first stage at the latest.
- `slime-channel`'s `Client` gains a way to cancel a request from another task.
- `interface` and `editing` rise at M6d.9a2c, a tool of before refused with each family.
- A request that waits holds a place in flight while it waits: an agent that waits on 16 holds
  its connection's places.
- Nothing reaches an agent between its calls: what changes while it does not wait is seen at its
  next call, from its handle.
- A program serves 8 connections: a ninth MCP server is refused.
- M6d.9a3a, which designs how tools find programs, weighs moving the tool's carrier into uwui.
