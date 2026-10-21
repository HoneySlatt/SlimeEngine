# 0110. How the editor runs a game's code

- Status: Accepted; amends [0055](0055-games-in-the-workspace.md) (a game is a library, its program
  and its editor); amended by [0116](0116-stage-6-in-two-halves.md) (the development channel built
  before play in the editor, with the MCP server) and
  [0121](0121-a-game-as-a-simulation-and-a-presentation.md) (a game's library holds its
  presentation too)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) decides, before Stage 6 builds the editor, how the editor runs a
game's code (C4-02, C6-01, C7-02). ADR-0097 records the owner's choice of its direction: a game
becomes a library and its binaries, and playing in the editor starts the game in a child process
first. [ADR-0099](0099-ai-agents-drive-the-engine.md) asks that the channel between the editor and
the game's process serve tools outside the editor too, the engine's MCP server among them.
[ADR-0107](0107-the-execution-model.md) makes the editor's child process a *host* that pauses,
slows and steps a game's *simulation*. The audit of 0.4 measured the figures below on the owner's
machine, with its RX 6900 XT, at `v0.4.0`; its report is kept outside the repository, as the plan
says.

- **The editor cannot open a game's level without the game's types.** A game is a binary
  ([ADR-0055](0055-games-in-the-workspace.md)); its components, such as *Hunna Wood*'s
  `Collider` and `Marker`, exist only in it, and a scene that holds a component of an unknown type
  is refused whole ([ADR-0089](0089-scenes-and-prefabs.md)). A type checks its values and upgrades
  those of its earlier versions by functions of Rust, which run where the type is. ADR-0089 leaves
  a registry that a game exports to "an editor that runs apart from the game", if a need appears.
- **A game that crashes must not take the editor with it.** P3 made a panic in `App::render`, once
  the frame drew, an ordinary panic
  ([ADR-0103](0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)), which `catch_unwind`
  could catch in the editor's process; an abort, an overflowed stack or memory exhausted cannot be
  caught, and would end an editor that ran the game in its own process, with the work not saved; a
  GPU device lost by the game would be the editor's too.
- **Nothing links the editor and a running game.** The editor will pause, slow down and step the
  game, read its world, change a value while it plays, capture its image; the game will tell its
  state, its logs and its measurements; agents will play it with scripted input.
- **A window cannot be put inside another program's under Wayland**, where the engine is
  developed. A client sees only its own surfaces, and the protocol has no request that places
  another client's window inside its own: `xdg-foreign` names another client's window only to put
  a dialog above it. X11 (`XReparentWindow`) and Windows (`SetParent`) have such a request. Under
  Wayland, an editor that shows a game inside its own window must act as a compositor for it,
  receiving the game's images.
- **Changing a game's code costs a rebuild.** A line of *Hunna Wood* compiles again in 1.3 s in a
  debug build; the game, started again, draws its zone 0.25 s later, and resumes a save in 1.5 ms.
  ADR-0097 left reloading a game's code to Stage 8.

The owner took four decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**A game is a library, its program and its editor.** A game's package, `games/<game>/`, is a
library, which holds its components, its registry and its rules, and its program, a thin binary
that runs it. Its editor is a package of its own beside it, `games/<game>-editor/`, named as the
game's with `-editor`, without the engine's `slime-` prefix, not published: a thin binary that
links the engine's editor and the game's library, through which the editor knows the game's types,
their checks and their upgrades. The game's package never depends on the editor, so that a
shipped game holds nothing of it. Both packages use the engine's public API only, and the checks
and CI build, test and lint the editor's as they do the game's; how they run it is decided with
the editor. The editor's code reaches a game's components through reflection only, the registry
and reflected values, never through a game's Rust types: the editor of the engine serves every
game. A game's own editing tools, an inspector or a manipulator drawn in the view, come with Stage
8's plugins. A registry that a game exports as data, which ADR-0089 leaves to a need, comes with
its first user, an editor built once for every game or an add-on of Blender.

**Play starts the game in a child process, in its own window.** The editor writes the level it
edits as a scene file ([ADR-0092](0092-a-world-written-back-as-a-scene.md)) and starts the game's
own program on it, with a save of its own, never the player's: the editor's child process of
ADR-0107 is the game's program, which the editor drives through the channel below. The game draws
in its own window, which receives the keyboard and the mouse. Showing the game inside the editor's
window, from images that the game shares, is weighed again with the native GPU backends of Stage
9c, which can share an image between processes without a copy, or sooner if two windows prove a
real hindrance.

**A development channel links the game to the editor and to tools.** A game listens only when it
is started with a secret token in its environment: 128 random bits that whoever starts it makes,
the editor for each game it plays, a test or a tool for the game it starts; a game started without
one, from a terminal or as shipped, does not listen. It listens on the machine's own address,
`127.0.0.1`, on a port that the system chooses, which it makes known to whoever started it, and
accepts a connection only with the token. The token is never on its command line, which other
users of the machine can read, nor in a log. The editor gives it to the MCP server, which then
reaches the game. Through the channel go:

- from the editor or a tool to the game: the host's controls of
  [ADR-0107](0107-the-execution-model.md), pause, speed, one step, stop; the editor's commands,
  which name their entities by persistent identities
  ([ADR-0109](0109-stable-references-and-undo.md)); questions on the world, read through
  reflection; captures of the game's image; scripted input;
- from the game: its state, paused or not, its frame and its game time; its logs and measurements;
  the answers.

Messages are commands and reflected values in the engine's binary encoding
([ADR-0094](0094-the-binary-encoding-of-documents.md)), behind a version of the protocol that both
ends check, so that an editor and a game of other versions say so rather than misread each other.
The channel is left out of the builds that a game ships, as the MCP server is (ADR-0099); how a
build is made for development or for shipping is decided with the MCP server, at Stage 6. The same
channel reaches a game on another device, a handheld or a console, once it listens on the network
with the same token, when a stage asks for it: that stage extends ADR-0099's rule, which keeps
other machines out, and [ADR-0096](0096-the-files-the-engine-reads.md)'s threat model to it. How
the token is drawn from the system's randomness, and whether it needs a dependency, is decided when
the channel is built.

**Code changed while the game runs: the game starts again where it was.** When the game's code
changes, the editor builds it again, asks the running game through the channel to save its run into
a file of its own, and starts the new build on that save. For *Hunna Wood*, the parts that the audit
measured add up to about 1.6 s; saving through the channel and starting the process, not measured,
come on top. A change of rules reaches the game so; what saves leave out, effects and projectiles
left `Unsaved`, starts again. A change of a component's shape builds the game's editor again too,
and the run's save, written in the earlier shape, is read as a save of an earlier version is: what
that reading cannot carry over is reported and starts again. Reloading the game's code in the
running process, as a dynamic library or by patching its functions, is weighed again if starting
again passes its budget on a real level; Stage 8's scripts serve the fastest changes of rules.

**When the game would run in the editor's process.** The child process stays while both hold:
Play gives the player control within 3 seconds of the click, the game already compiled, on the
game's largest level, measured on the owner's machine until the hardware tiers of P6 name another;
and what the editor inspects of a running game, through the channel, is enough for its needs. When
one fails, running the game in the editor's process is decided again, with what this ADR's
alternatives say of it.

**When it is built.** At Stage 6: *Hunna Wood* split into its library, its program and its editor;
the development channel, with Play, the host's controls and the editor's commands; starting again
after a change of code. The MCP server of ADR-0099 uses the channel from Stage 6.

## Alternatives considered

How the editor knows a game's types:

- **One editor built once for every game, reading a registry that a game exports**: never built
  again, open to who writes no Rust; but a type's checks and upgrades are functions of Rust that it
  could only ask the game's process to run, the world would need components without a Rust type,
  which is Stage 8's work, and a game could have no editing tools of its own before Stage 8's
  plugins.
- **Both now**: twice the work, for a need that nothing has before Stage 8.

The channel:

- **The game's standard input and output**: the simplest, the same everywhere, reachable by the
  editor alone, so nothing to guard; but the MCP server and tests could not reach a game but
  through the editor, and messages would share the stream with the logs.
- **A socket of the system's own**, a Unix domain socket on Linux, a named pipe on Windows: the
  system's permissions guard it; but two implementations, Rust's standard library having no named
  pipes, so a dependency or Windows code of the engine's own, and no way to another device.

The game's window:

- **The game's window put inside the editor's**, as Godot does since 4.4 on Windows and X11: not
  possible under Wayland but by writing a compositor into the editor, and code for each system.
- **The game's images copied through the CPU to the editor's view**, its input sent back through
  the channel: integrated on every system, but about 500 MB/s at 1080p and 60 frames per second, a
  frame of delay at least, and a cost on the GPU, which distort what playing is there to judge.

Code changed while the game runs:

- **The game's code as a dynamic library, reloaded in the running process**, as Fyrox can: under a
  second, its memory kept; but Rust promises no binary compatibility between two builds, a type
  whose shape changed leaves the memory incoherent, and the same compiler and options are needed.
- **Patching the running program's functions**, as the `subsecond` tool of Dioxus does and Bevy
  offers as an experiment: fast, its state kept; young, a dependency, and a type whose shape
  changes is not covered.

The game in the editor's process, as Unreal and Unity run it: the simplest to debug, a game started
without loading again what the editor loaded; but an abort, an overflowed stack or memory
exhausted end the editor with the game, a lost device is the editor's too, every change of the
game's code builds and starts the editor again, and the editor needs an API to host a game. It is
decided again on the conditions above.

## Consequences

- *Hunna Wood* becomes a library, its program and a package of its editor:
  [ADR-0055](0055-games-in-the-workspace.md)'s "a game is a binary" becomes "a game is a library,
  its program and its editor", and a game has two packages in `games/`, its own and its editor's.
- A change of a component's shape builds and starts the game's editor again; a change of rules
  reaches the running game in a couple of seconds for *Hunna Wood*.
- The editor's code holds no type of a game: what it does for *Hunna Wood* it does for any game.
- A game started by the editor, a test or a tool listens on a local port while it runs, guarded by
  a token; the [Windows virtual machine](0015-platforms-and-ci.md) checks
  that the system's firewall does not ask about it.
- Two windows while playing, the editor's and the game's.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6's play in the editor runs the game in a
  child process, in its own window, linked by the development channel.
