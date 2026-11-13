# 0246. One core for agents cut in five: a session without input or output, in JSON-RPC, versioned by family

- Status: Accepted; amends [0110](0110-how-the-editor-runs-a-games-code.md) (the messages in
  JSON-RPC, not in the engine's binary encoding), [0122](0122-the-development-channel.md) (the
  channel's messages in JSON-RPC through `serde`, versioned by family, `slime-reflect` and
  `slime-serialize` gone from `slime-channel`) and [0218](0218-one-core-for-agents-in-uwui.md)
  (M6d.9a cut in five); amended by [0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) (one waker, meaning followed by each reader, an
  element's capture from the host's image)
- Date: 2026-11-12

## Context

M6d.9a's card ([`stage-6d.md`](../stage-6d.md#m6d9a--one-core-for-agents-and-its-mcp-server))
holds the whole of [ADR-0218](0218-one-core-for-agents-in-uwui.md)'s core: a protocol that no
transport binds, the agent named with the person's consent, numbered requests and events,
attaching to a program already running, rights, previews, tools found by families, uwui's MCP
server, and the engine's channel and MCP server on it. Stage 6d's rule lets a milestone be cut at
its start.

The facts, read in the repository and in the current documentation on 2026-11-12:

- The development channel ([`slime-channel`](../../crates/slime-channel/README.md)) has 25
  requests. Four read uwui's tree alone: the elements of a role and a name, the element at a
  pixel, an action on an element, its capture. `slime-app` answers them with uwui's types and no
  other of the engine's. The others belong to the engine: the host's time and window, its input
  and image, its world, its files and the editor.
- One request, then its reply: a connection waits for each answer, and a failed request breaks
  it. The protocol has one version, 13, raised at the change of any request. Its messages are
  reflected values in the engine's binary encoding.
- Each connection has a thread of its own, and an unbounded queue hands its requests to the game.
  A request of the channel answered at once took 0.14 ms, the median of 21 on the loopback
  ([`measurements.md`](../measurements.md)).
- `slime-mcp`'s 36 tools reach the channel through its client. Its session tests speak MCP as
  JSON-RPC over the server's standard input and output, without `rmcp`.
- The editor's Play asks how the played game's time stands every 200 ms: a reader of events in
  waiting.
- MCP's current version is `2026-07-28`. It has no handshake: each request carries its version,
  its client's capabilities, and the client's name. A transport over a stream of bytes should
  reuse the framing of its standard input: one JSON-RPC message a line. Lists of tools no longer
  vary by connection, and should come in a fixed order, so that clients keep their prompt in
  cache. `rmcp` 3.9.0 implements it.
- [ADR-0224](0224-a-files-change-reaches-the-application-through-the-development-tool.md):
  an application in development opens one connection, to `uwui dev`, on the desktop, the web and
  Android; a page of the web cannot listen. [ADR-0211](0211-the-executor-and-other-threads-through-channels.md):
  uwui's own threads stay few, and a host gives the core its threads.

The design was attacked by a fresh agent before it went to the owner: its first cut in three left
a first part as large as the whole milestone, a core holding its own sockets and threads
contradicted ADR-0224 and ADR-0211, a binary frame after a reply could be split by an event from
another thread, and one version for everything would stall uwui and the engine on each other.

## Decision

The owner's decisions of 2026-11-12.

**M6d.9a is cut in five**, each part in a session of its own, its own questions put to the owner at
its start:

- M6d.9a1a, the new wire under today's meaning: the core's session and its messages; the engine's
  channel on it, its requests and their behaviour unchanged; the MCP session tests unchanged.
- M6d.9a1b, requests in flight and events: several requests at once, bounded, ordered and
  cancellable; events, the editor's Play their first reader; the interface's family moved into
  uwui, with the place of the code that wrote an element's own style
  ([ADR-0231](0231-an-elements-own-style-written-in-code.md)).
- M6d.9a2, uwui's MCP library and families: `slime-mcp` on it with the engine's families.
- M6d.9a3a, attaching to a program already running, and the person's consent.
- M6d.9a3b, rights for each agent, and previews before applying.

**A session without input or output.** The core is a state machine: it is given the bytes that
came, and gives back the messages to handle and the bytes to send. It opens no socket, starts no
thread and holds no clock. Whoever dialled, the program answers: on the desktop, the tool dials,
on the machine's own address with a token, as now; the program's connection out to `uwui dev`
comes with `uwui dev`, at uwui's first stage, carrying the same session. The host gives the
threads and the wake: in the engine, `slime-channel`'s listener and client, on the standard
library's threads and sockets, as now. A page's socket of the web, Android's, and the paired
machine's encryption are carriers added beside it, the core unchanged.

**A crate of uwui's, `uwui-agent`,** under MIT OR Apache-2.0, depending on no other crate of
uwui's, so that a tool speaks the protocol without building uwui's text and layout: the session,
its messages, its framing, and the families' versions. The code that answers the interface's family
from uwui's tree goes into `uwui` behind a feature `agents`, at M6d.9a1b; the feature compiles it
and serves nothing: a program or its host starts serving. `slime-channel` stays the engine's: the
families of play, the world and editing, `launch`, the token, the desktop's carrier. uwui's MCP
library is named at M6d.9a2's start.

**JSON-RPC 2.0, one message a line.** A request has its number, its method, `family/name`, and its
parameters; an answer has the number and a result or an error, with a code; an event is a
notification. Encoded by `serde` and `serde_json`, which `uwui-agent` depends on; `slime-channel`
derives `serde`'s traits for its messages and depends no longer on `slime-reflect` nor
`slime-serialize`. A message that carries bytes, the pixels of a capture, says how many in its
envelope, and they follow its line at once, written by the connection's one writer, so that
nothing comes between them; on a socket of the web they will be a binary frame of their own. A
number that may pass 2^53 is written as text, as JavaScript reads no larger integer exactly; a
float that is not finite is refused. Every message is bounded in size, and a list that may be long
gives a cursor to the rest, as the channel's lists already count what they leave out. The values of
a component stay text in the engine's notation inside the messages, read by the game with its
types ([ADR-0122](0122-the-development-channel.md)).

**A version for the core, one for each family.** The core's version covers the session and the
envelope; each family has its own. The first message names the agent, as it says it is, its name
and version, and the program that carries it, the client; it gives the secret that the carrier
asks for, and the families that the tool speaks, each with its version. The program answers with
its name and the families that it serves; a family of a version it does not know is refused alone,
and the rest is served. A program writes in its log which agent connected. The consent that a name
may be given comes with M6d.9a3a.

## Alternatives considered

The cut:

- **In four**, the new wire, the requests in flight, the events and the interface's family in one
  part: a session fewer, but the wire and its meaning changed at once, a regression harder to
  isolate.
- **In three**, the protocol whole, then MCP, then attaching, consent, rights and previews: fewer
  sessions, but a first and a last part each as large as a milestone whole.

The core's shape:

- **Sockets and threads in the core**, `slime-channel` moved as it is: the shortest to write now,
  but a second wake beside uwui's executor, nothing that runs on the web, and a rewrite for the web
  and Android.
- **A core on tokio**, as `rmcp` is: natural on the MCP side, but tokio in every application of
  uwui's, its threads against ADR-0211, and the same trouble on the web.

The crates:

- **`uwui-agent` depending on `uwui`**: one crate, no feature, but every tool builds uwui's text
  and layout for a protocol.
- **`uwui-agents`**: the same, in a plural that uwui's other crates do not use.

The encoding:

- **The pixels in base64 inside the JSON**, as Chrome's DevTools carry their captures: JSON alone,
  but a third more bytes, about 11 MB for an image of 1080p, and its encoding at each capture.
- **`postcard`**, binary: the most compact, but it does not describe itself, so that a field added
  breaks an older reader; unreadable in a log; the web and Android would need a decoder of their
  own.
- **The channel's length before each message**, as now: no change, but a third framing beside
  MCP's lines and the web's frames.
- **The engine's reflection kept for its messages**, translated to JSON by hand: no `serde` in
  `slime-channel`, but two ways to encode a message, and a translator to keep.

The versions:

- **One version for everything**, as now: the simplest, but uwui and the engine raising one number
  without knowing of each other, and each application that adds a family making it wrong.

## Consequences

- `slime-channel` gains `serde` and `uwui-agent`, and loses `slime-reflect` and `slime-serialize`;
  `uwui-agent` gains `serde` and `serde_json` ([`dependencies.md`](../dependencies.md)). The
  messages' encoding changes: a tool of before cannot speak to a program of after, nor the reverse,
  as with each version of the protocol before.
- The questions left to later parts, raised by the attack on the design: requests answered out of
  their order and their cancellation (M6d.9a1b); a list of tools fixed at the server's start, as
  MCP `2026-07-28` asks, against families opened on demand (M6d.9a2); listening only when started
  with a token or turned on by the person, rather than always; consent kept by a credential that
  the program gives rather than by a name, a sign while an agent is attached, a prompt that an
  agent cannot answer, a program without a window that refuses attachment; the author of each
  change kept; secrets never shown to an agent (M6d.9a3a); who applies a preview, the agent or the
  person; previews on the editor's gestures, staled entity by entity; rights of the server's own
  tools kept by the system's sandbox or only declared (M6d.9a3b).
- Until M6d.7's desktop host, a program on uwui alone does not exist: M6d.9a's parts are checked
  on the engine's examples, which are programs on uwui through the engine's host.
- The MCP session tests run on Linux in CI's windowed and headless jobs, not on Windows: each part
  adds tests of the session over the loopback that `cargo test --workspace` runs on Windows too.
