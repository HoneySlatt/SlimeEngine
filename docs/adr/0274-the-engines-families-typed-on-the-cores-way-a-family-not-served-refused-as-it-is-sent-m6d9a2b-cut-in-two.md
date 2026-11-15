# 0274. The engine's families typed on the core's way; a family not served refused as it is sent; a tool's name taken twice an error; M6d.9a2b cut in two

- Status: Accepted; amends
  [0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)
  (M6d.9a2b cut in two) and
  [0272](0272-the-carriers-families-and-its-connections-state-join-it-at-their-first-reader.md)
  (the families served never join the carrier: the carrier refuses as it sends)
- Date: 2026-11-15

## Context

M6d.9a2b's card ([`stage-6d.md`](../stage-6d.md#m6d9a2b--the-engine-on-the-library)) moves
`slime-mcp`'s every tool onto uwui's MCP library, `uwui-mcp`. It asks for the families `play`,
`world` and `editing` as the library's; a tool of a family not served refused in the server's
words; the played game reached by the interface's tools; a call cancelled while an edit waits for a
drag never applied, tested through the server; the server's instructions written by each family;
a tool's name taken twice refused; and the debts of `stop_game`, `drive_played_game` and the
error's chain.

The facts, read in the repository on 2026-11-15:

- `slime-channel`'s `Request`, `Edit` and `Reply` are public and tagged by method, as each family
  of the core writes its own; `uwui_agent::call` and `uwui_agent::read` write and read any such
  type. Only `Request::call`, which unwraps `Request::Edit` and `Request::Interface`, is private.
  The interface's tools already go from their family's type to the core and back that way.
- `uwui_mcp::List` and `Page` cannot be implemented in `slime-mcp` for `slime-channel`'s types:
  both are foreign there.
- After the move, the tool's side of `slime-channel` carries the interface's requests for its
  tests only: `Pending`'s `of_interface` and `Reply::read`'s branch of the interface.
- Every program of the workspace serves the four families. One is refused only when the tool and
  the program speak two versions of it, as the welcome says. The core's tool session refuses a
  request of a family not served, on the connection it goes to, as it is sent.
- `slime-mcp` chooses that connection under its lock, at each request: the editor's, or the played
  game's while the game's tools drive it. A choice made before the send may no longer hold at it.
- `rmcp`'s routers, added up, replace a tool of a name already taken without a word. Within one
  server's type a second tool of the same name does not compile; across the library and its
  server, nothing refuses it.
- No tool can hold a person's gesture in the editor: gestures come from its window's pointer.
  `slime-app`'s test proves that an edit cancelled while it waits for a drag is never applied,
  through the channel's own cancel.
- `start_game` holds the server's lock while `cargo run` builds the game, up to 600 s; `stop_game`
  and `drive_played_game` hold it while their requests wait. Taking them off it needs one state of
  the server's game, `Starting`, `Running` or `Stopping`, numbered by each start: a stop that waits
  must never kill the game started after it.

A fresh agent attacked the lead's first design. It found a check of the families served before
the send that raced with the routing; local types needed for the engine's lists; a public way to
write a request beside the core's; a cut that kept the card's two tests apart from the code they
prove; and the start's lock, which the debt did not name.

## Decision

The owner's decisions of 2026-11-15.

**The engine's families go through the core's typed way.** `uwui-mcp` gains a typed form of
`ask`: it takes a request of any family's type, sends it through `uwui_agent::call`, and reads the
answer as that family's reply through `uwui_agent::read`. The interface's tools and the engine's
take it alike. `slime-mcp`'s tools of `editing` send an `Edit`, those of `play` and `world` a
`Request`. `slime-mcp` names local types for its lists, which implement `List` and `Page`. Its own
loop of pages goes. `slime-channel` makes nothing new public, and its tool's side loses its
second way of asking the interface.

**A family not served is refused as it is sent, in the carrier's words.** The core already
refuses it, on the connection that the request goes to. `slime-mcp`'s carrier turns that refusal
into a tool's error of its own: the family, both versions, and which side to build again, the
server or the program, as the older one is. `slime-channel`'s `Client` keeps its welcome's
refusals for it. The families served never join `uwui_mcp::Carrier`: no tool reads them.

**A tool's name taken twice is an error when the server is built.** A family of the library is a
named group of the core's requests' tools with its paragraph of the server's instructions. The
server's own tools, which send no request of the core, come with the instructions' first
paragraph, and are no family. Adding a family whose tool's name another took, disabled or not, is
an error that names the tool and both owners. The server does not start, and says why.

**The cancel through the server is tested on a program of the test's own.** `slime-mcp`'s test
binary runs, in a mode of its own, as a program on `slime_channel::Listener`, with no dependency
added. It holds an edit until its tool cancels it, and says what it saw. The test proves that an
agent's cancel becomes the core's cancel of that request, on the connection that the routing chose.
`slime-app`'s test proves the rest: an edit cancelled while it waits for a drag is never applied.
It runs on Windows too.

**M6d.9a2b is cut in two**, each in a session of its own:

- M6d.9a2b1, the engine's families on the library: the typed way, the families and the
  instructions, a name taken twice, a family not served, the cancel and the played game reached by
  the interface's tools, tested; `read_logs` by a cursor; the error's chain; the answer measured.
- M6d.9a2b2, the server's game off its lock: one state of the game, numbered; `start_game` off the
  lock and cancellable; `stop_game` by closing the game's standard input; `drive_played_game` off
  the lock; the server's end. Tested by sixteen long steps holding every place, then `stop_game`; a
  start cancelled; a start after a stop.

**The lead's decisions within them:**

- `read_logs` takes `cursor` and gives `next` first. Its cursor names the program whose log it
  reads, so that a cursor of another program, the editor's or a game that Play started again, is
  refused as `STALE` is; `play` keeps its version 1.
- `slime_channel::why`, made public, writes an error's chain for `slime-mcp`, whose own helper goes.
  `slime_app::error_chain` stays: the shipping build links no `slime-channel`.
- The answer through MCP is measured where frames do not hide it: on a program that answers at
  once, beside the editor's window.

## Alternatives considered

The engine's families:

- **`Request::call` and a reading of `Reply` made public in `slime-channel`**: no new item in
  `uwui-mcp`, but two ways to write a request, and a second public way to the interface.
- **Methods and parameters written as JSON in `slime-mcp`**: the wire's names written twice.
- **`slime-channel`'s `Request` split into one type per family**: the right shape, but a rewrite of
  the listener, `slime-app` and the editor.

A family not served:

- **The families served given by the carrier, checked before the send, as ADR-0272 wrote**: one
  rule for every carrier, but a race with the routing, which then gives the core's words for the
  same condition.

A name taken twice:

- **A panic when the server is built**: shorter, but a crash at the agent's start, which a test
  catches only through `catch_unwind`.
- **A test of `slime-mcp` alone**: a server of another program counts nothing.

The cancel's test:

- **A game of `slime-app` in the test, whose edits wait while a key is held**: one test from end to
  end, but 178 crates more in `slime-mcp`'s tests, and what `slime-app`'s test proves already.

The cut:

- **One milestone**: a long session, and a review that mixes a port with concurrent code.
- **The card's two tests with the lock's work**: apart from the port that they prove.

## Consequences

- `uwui-mcp`'s public API grows by the typed `ask`, the family, and the tools that the families make
  up: an API between two repositories once uwui moves (M6d.11).
- ADR-0272's consequence changes: the carrier gives no families served; it refuses as it sends.
- `slime-mcp`'s first session test runs on Windows.
- The debt of the server's lock names `start_game` too; it waits for M6d.9a2b2.
- `read_logs`' argument `after` becomes `cursor`: an agent's call of before is refused, and told.
