# 0166. Building the game again

- Status: Accepted
- Date: 2026-10-28

## Context

[ADR-0110](0110-how-the-editor-runs-a-games-code.md) decided that when the game's code changes
while it runs, the editor builds it again, asks the running game to save its run into a file of
its own, and starts the new build on that save. [ADR-0155](0155-the-editors-design.md) decided
(its decision D5) that the build is read from cargo's messages in JSON, through `serde_json`, that
the executable built is started itself, not `cargo run`, without a console window on Windows, with
a mark on Play saying that the game was built again, and that a running executable that Windows
keeps locked may block the link, which is to check; and (its decision D10) that the tools learn of
a source's change through notify from milestone M6c.7, once polling is measured on Windows, and
poll until then. Milestone M6c.4c, which was to bring this and to measure Play, was cut in two at
its start, on 2026-10-28, as ADR-0155 allows: the game built again, which this ADR records; Play
measured, M6c.4d.

What the code held before this milestone:

- Play starts the game's program built already, found beside the editor's own
  (`Played::program`), on a level of Play's own directory and a save there
  ([ADR-0164](0164-play.md)), and sends it the steps made while it runs
  ([ADR-0165](0165-editing-while-playing.md)).
- A game that saves its runs saves one into a file that a tool names (`Request::Save`), as
  differences from the level as it started, the run's own changes and the steps applied included
  ([ADR-0125](0125-saves-per-scene.md)); *Hunna Wood* resumes the run of the save that `--save`
  names unless `--new` is given. The `world` example does not save its runs.
- cargo writes, beside each program that it builds, a file of its dependencies (`.d`) that lists the
  source files of the program and of the workspace's crates that it uses: about 200 for *Hunna
  Wood* or the `world` example. A `cargo build` with nothing to build takes 0.13 s on the owner's
  machine; neither `cargo run` nor `cargo test` holds the build directory's lock while the program
  or the test runs, so that a program that they started may run cargo itself.

## Decision

**Cargo builds the game as Play starts it.** A program gives how cargo builds its game
(`Program::build`): a directory of the game's workspace and the arguments that name the game's
program to cargo, `--package hunnawood --bin hunnawood`. Play's thread runs
`cargo build --message-format=json-render-diagnostics` with them, in the profile that the editor's
own program was built in, before it starts the game: the toolbar says that it builds, and Stop
gives it up, cargo stopped. cargo's messages, read through `serde_json`, give the executable built;
the errors that cargo writes on its standard error go to the console. A build that fails ends Play,
and the user is told. A program without `build`, as a game
shipped built, plays the program beside the editor's, as before.

**The code is watched by polling the files that cargo lists.** After each build, Play reads the
file of the dependencies beside the executable, and adds the manifests and locks of cargo in the
directories above the files that it lists, which it does not list; while the game runs, its thread
looks at the times at which those files were last changed, once a second, which D10 keeps until
M6c.7. When one changed since the build started, cargo builds the game again, while it plays on,
the controls and the steps still reaching it.

**A new build starts again on a save of the run.** When the program that cargo gives is not the one
that runs, another file, or one written since it started, by this build or another, Play asks the
game to save its run into Play's save, stops it, and starts the new build resuming that save
(`Played::resume`), on the level as Play wrote it at the start: the save holds what changed since,
the steps sent included. A game that does not save its run starts a new run, on the level as the
editor holds it, which the editor writes again into Play's directory first, the steps sent
meanwhile in it; the user is told why it did not save. The steps sent while the game starts again
reach the new game. Game time paused, or at another speed, is set again. The toolbar marks the game
as built again. The new game listens at an address of its own, with a token of its own: an agent
that drove the game is pointed back at the editor, as when a game no longer answers.

**A build that fails leaves the game playing.** Its errors go to the console, the toolbar marks the
game as not built, and the game plays on its last build; the next change builds it again.

**The executable is started itself.** Play starts the program where cargo built it, but on Windows,
where it starts a copy, of the same name, with the libraries beside it, the shader compiler's among
them, in a directory of Play's own for each start, removed once that game stopped, as Windows keeps
the file of a running program from being replaced, which would fail the next build's link. On
Windows, cargo and the game start without a console window.

## Alternatives considered

- **`cargo run`**: one command, but its messages are not read, Play cannot tell whether it built
  anything, and the game runs as cargo's child; ADR-0155 rejected it.
- **A `cargo build` every second, which cargo finds has nothing to do**: 0.13 s of a processor each
  second, and the build directory's lock taken as often, which the user's own builds would wait on.
- **notify now**: ADR-0155 adopts it at M6c.7 for every tool, once polling is measured on Windows.
- **A copy of the executable on every system**: a debug build of *Hunna Wood* is 280 MB, copied at
  each start into a directory of temporary files that may be held in memory; only Windows needs it.
- **The level's text written again before the new build of a game that saves its run starts**: the
  save holds the steps sent as changes of the run already; a level that held them too would create
  twice what they created.
- **The code loaded again in the running process**: ADR-0110 rejected it.

## Consequences

- Play takes 0.13 s more when nothing changed, and a first Play after a change waits for its build.
- What a save leaves out starts again with the new build, as ADR-0110 says.
- A game that plays on its last build after a failed build differs from its code until it builds.
- A new build that builds but does not start, or crashes as it starts, ends Play, as a game that
  crashes does, and its run's save goes with Play's directory.
- cargo stopped, as Play stops during a build, leaves the compilers that it started to finish the
  crate under way.
- The game is built with its package's features alone: a build of the whole workspace, which
  unifies others, makes cargo build it again once.
- That the copy leaves the build free to replace the program on Windows, and that cargo and the game
  open no console window there, are checked by hand in the Windows virtual machine.
