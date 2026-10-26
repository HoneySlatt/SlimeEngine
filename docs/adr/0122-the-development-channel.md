# 0122. The development channel and the MCP server

- Status: Accepted; amends [0033](0033-gpu-layer-for-multi-pass-rendering.md) (a window's image is
  read back where the system allows it), [0096](0096-the-files-the-engine-reads.md) (its threat
  model extended to the channel and the server), [0099](0099-ai-agents-drive-the-engine.md) (the
  server's dependency, and where it runs), [0110](0110-how-the-editor-runs-a-games-code.md) (the
  token's randomness, where the port is made known, values as text in the messages, builds for
  development and for shipping) and [0121](0121-a-game-as-a-simulation-and-a-presentation.md) (a
  game holds game time apart from the pause); amended by [0125](0125-saves-per-scene.md) (version 2
  of the protocol: saving and loading a running game), [0126](0126-views-apart-from-the-renderer.md)
  (version 3 of the protocol: a view captured), [0127](0127-the-settings-of-a-view.md) (version 4 of
  the protocol: a view's settings read and changed), [0128](0128-asset-requests.md) (version 5 of
  the protocol: a game's reports), [0130](0130-picking.md) (version 6 of the protocol: a picking),
  [0134](0134-the-editors-core.md) (version 7 of the protocol: the editor's requests; a reference to
  an entity written `""` read as no entity), [0135](0135-the-editors-program.md) (version 7 of the
  protocol, as built), [0143](0143-what-agents-see-of-the-interface.md) (version 8 of the
  protocol: an interface read, acted on and captured) and [0146](0146-screen-readers.md) (version 9
  of the protocol: a field's value read, and set)
- Date: 2026-10-21

## Context

[ADR-0110](0110-how-the-editor-runs-a-games-code.md) decided the shape of the *development
channel*, through which the editor, tests and tools drive a running game: a game started with a
secret token in its environment listens on the machine's own address, accepts only connections
that give the token, and exchanges messages in the engine's binary encoding behind a version of the
protocol. [ADR-0099](0099-ai-agents-drive-the-engine.md) decided that the engine has its own MCP
server, through which AI agents drive a game as a person would, and
[ADR-0116](0116-stage-6-in-two-halves.md) placed both in milestone M6a.6, before play in the
editor. They left open: the server's dependency; how the token is drawn; how a build is made for
development or for shipping, the channel and the server left out of the latter; and how a tool sees
the game's image, which [ADR-0033](0033-gpu-layer-for-multi-pass-rendering.md) made impossible: a
window's image could only be drawn into.

Building the channel raised two more questions. A tool does not know a game's types, so it cannot
write a value of them in the binary encoding, which a reader decodes with the type in hand. And
*Hunna Wood* kept game time paused while its zone loaded
([ADR-0121](0121-a-game-as-a-simulation-and-a-presentation.md)), then let it run once the zone was
drawn: a tool that paused the game in the meantime saw its pause undone.

The owner decided the dependency, the token, the builds, the capture and the encoding of images
for the server on 2026-10-21.

## Decision

**Two crates.** `slime-channel` holds the protocol and both of its ends: the game's, a `Listener`,
and the tool's, a `Client` and `launch`, which starts a game with a new token and finds where it
listens. It depends on the standard library, `getrandom`, `slime-reflect` and `slime-serialize`,
and on no GPU, window or world. `slime-mcp` is the MCP server, a program that no game links.
`slime-app` holds the game's side, which answers the requests with its host's world, time and
input, behind its `development` feature.

**The token** is 128 bits drawn from the system's randomness through `getrandom`, written as 32
hexadecimal digits in the environment variable `SLIME_DEV_TOKEN` of the game that `launch` starts.
It is never on a command line, which other users of the machine can read, nor in a log: its debug
form hides it. A game compares it in a time that does not depend on where it differs. A game whose
variable does not hold a token logs an error and does not listen.

**Listening.** A game started with a token listens on `127.0.0.1`, on a port that the system
chooses, then writes one line on its standard output, `slime-dev-channel 127.0.0.1:<port>`, which
whoever started it reads: its log goes to the standard error. A thread accepts the connections, and
one more for each connection reads its requests and writes the answers: threads that wait, as
[ADR-0107](0107-the-execution-model.md) allows. The first message of a connection is a hello, a
message of its own whose debug form hides the token, which gives the version of the protocol and
the token; it comes whole within a few seconds and holds little, or the connection is refused with
the reason, and closed, as it is for another version or token. A few connections at most wait for
their hello at once: more are closed unread. The figures are in `Listener::open`'s documentation.

**The protocol.** Each message is a document of the engine's notation
([ADR-0086](0086-serialization.md)), a value of the reflected types `Hello`, `Request` or
`Reply`, in the binary encoding ([ADR-0094](0094-the-binary-encoding-of-documents.md)) and read
strictly, preceded by its length; then the length of the bytes that follow it, and those bytes:
the pixels of an image, the only message that carries them. A change of the messages raises the
version of the protocol, which both ends check at the hello. The values that a tool reads and changes, a
component or a part of it, cross as text in the engine's notation inside the messages, not as
binary values: the game reads them with the types it knows, through its registry and with the
ranges of their fields, and writes them as its documents do, entities named by their persistent
identities. A tool needs no type of the game, and an agent reads and writes what a scene file
holds.

**What the channel does.** The game's requests wait in a queue until the start of the next frame,
where the host answers them before the commands of phase 1 (ADR-0107): a request changes the host
between frames, as a command does. A connection's requests are answered one at a time, in their
order. A tool asks for:

- the game's state: its frame, whether game time is paused or held, its speed, the fixed steps run
  and the game time they covered, real time, the average and longest times between the starts of
  the last 120 frames, and how many entities the world holds;
- the host's controls: pause, run again, a speed, steps run while paused, answered once every step
  asked for has run, over as many frames as they take, or with a failure when game time ran again
  and dropped them, and stop, as a closed window stops the game;
- the world: its entities, by persistent identity and name, a bounded number and how many more;
  an entity's parent and its components as documents; a part of a component set by the same
  `Command::SetProperty` as the dev overlay's, by persistent identity
  ([ADR-0121](0121-a-game-as-a-simulation-and-a-presentation.md));
- input: a key put down and let go in the host's input, as the player's would be, named by its
  position on the keyboard, `KeyW`;
- the image that the game shows, its window's, copied after the presentation drew it and before it
  is shown, the dev overlay included; a run without a window has none;
- the log's last lines: a run started with a token keeps the last lines that its log writes,
  numbered.

**Builds for development and for shipping.** The game's side of the channel, and the crates it
needs, are in `slime-app` behind its cargo feature `development`. A game turns it on among its
default features, so that `cargo run` builds a game that listens when started with a token, and
`--no-default-features` builds it for shipping, without the channel's code or crates; the checks
build the game so, and fail if its build holds the channel. The MCP server is a program apart,
which never ships with a game. The programs that CI builds for checks by hand keep their default
features, the channel with them.

**The capture.** A window's image gets the copy-source usage too where the window's system offers
it, and `Gpu::read_render_target` reads it back as it reads an offscreen target; where the usage is
not offered, the read is refused with an error, which the tool receives.

**A game holds game time apart from the pause.** `Context::hold` holds game time, as a game does
while it is not ready to be played, without touching the pause that the player and the tools set:
held and not paused, game time stands still; steps asked for while paused run, held or not; the
state that the channel gives says both. *Hunna Wood* holds its time while its zone loads, rather
than pausing it.

**The MCP server.** `slime-mcp` speaks the Model Context Protocol on its standard input and output,
through `rmcp`, the protocol's official Rust implementation: an agent starts it as a program of its
own, and the server opens no port. Its tools start a game with a token, one at a time, and give the
channel's requests; they cook a directory of source assets, which a game started with `--watch`
loads again, its shaders as well, and run `cargo test`, from the repository's root. A capture is
given to the agent as a PNG image, encoded in base64, as the protocol carries images. A game's
standard error goes to a file of the temporary directory, which the server names. A game that
ended is told at the next tool that asks it; a game still running when the agent goes is stopped.
After a request that failed, the tools' end of the channel gives its connection up, since the
game's late answer would be read as the next one's, and the server opens another.

**The threat model of [ADR-0096](0096-the-files-the-engine-reads.md), extended.** The channel
reads bytes from other processes, which that decision did not cover:

- Other machines cannot reach the channel, which listens on the machine's own address only.
- Other users of the machine can connect to that address: the token keeps them out. They cannot
  read it from the game's command line or log; the system lets only the same user and the
  administrators read a process's environment.
- What runs as the same user is out of scope: it has the user's rights already, and can read the
  game's environment.
- A tool with the token can do what the user can: the agent acts with the rights of the person who
  runs it, as ADR-0099 says. The MCP server runs the command that the agent gives it, as the
  agent's own shell would.
- Bytes from a connection never make the game panic, hang or exhaust its memory: a hello is bounded
  in size and time, and the connections that wait for one in number; every message is bounded by
  the reader of the binary encoding, which refuses what it would not read from a file; a message
  refused by its length closes the connection, since the next one cannot be found; a request that
  the game refuses is answered with the reason; a refusal is logged escaped, so that what a
  connection sent makes no line of the log. The ranking of defects of ADR-0096 applies.
- A game built for shipping holds no channel.

## Alternatives considered

The server's dependency:

- **The protocol written by the engine**, JSON-RPC on the standard input and output with
  `serde_json`: fewer crates, but the protocol changes every few months, and each version would be
  followed by hand.
- **Another Rust implementation of MCP** from the community: smaller, but behind the official one in
  the protocol's versions, with fewer users.

The token:

- **A value made from the time and the process's identifier**, without a dependency: another user
  of the machine could guess it.
- **The system's randomness reached by the engine's own code**, `/dev/urandom` on Linux and a call
  to Windows: `unsafe` code, or a crate of Windows bindings, for what `getrandom` does already;
  `uuid`, already a dependency, draws its random identifiers through it.

Builds for development and for shipping:

- **A flag read at run time only**: a shipped game would hold the channel, which ADR-0099 forbids.
- **The channel in debug builds only** (`debug_assertions`): an optimized build, which agents and
  profiling use, would lose it, and a debug build would ship it.
- **The feature off by default, turned on by the tools**: every command that runs a game for an
  agent would name it, and `cargo run` would build a game that cannot be driven.

The capture:

- **The frame drawn a second time, into an offscreen target**: a second frame's cost, and not the
  image that the player sees.
- **The presentation's own image before tone mapping**: not what the player sees, without the
  overlay, and a convention that each presentation would follow.
- **A copy of each frame into an offscreen target**: a copy every frame, for a capture now and
  then.

The rest:

- **Values in the binary encoding**: a tool would need the game's types, which only the game's
  program holds.
- **The port in a file**, or chosen by whoever starts the game: a file to clean and a race between
  two games, or a port taken between its choice and the game's listening.
- **Requests answered by the channel's threads, under a lock on the world**: the world belongs to
  the main thread's frame (ADR-0107), and a request would land in the middle of a phase.
- **The game's pause used while it loads**, as before: a tool's pause would be undone.

## Consequences

- An agent drives a running game, and checks on screen what it changes, through the MCP server:
  each later milestone exposes its system to it (ADR-0116).
- A game built with its default features listens when started with a token; without one, reading
  an environment variable is all that the channel costs. A workspace's build turns the feature on
  for every crate that links `slime-app`, as cargo unifies features: a build for shipping names its
  game alone, with `--no-default-features`.
- The engine gains `getrandom` in the development feature, and the server gains `rmcp`, `tokio`,
  `serde`, `schemars` through `rmcp`, and `base64` ([`dependencies.md`](../dependencies.md)).
- A window's image holds the copy-source usage on every frame, where it is offered, whether or not
  a tool captures it; what that costs a driver is not measured.
- Limits, stated where they live: keys only, no mouse, in scripted input; one game at a time per
  server; captures of windowed runs only; the measurements are the frames' times, not the
  renderer's statistics, which the presentation holds; a capture travels through the CPU, about 8 MB
  at 1080p, enough for checks, not for video; a component that refers to an entity without a
  persistent identity is read, the reference written as no entity, and cannot be written back.
- The Windows virtual machine checks that the system's firewall asks nothing when a game listens
  ([ADR-0015](0015-platforms-and-ci.md)).
