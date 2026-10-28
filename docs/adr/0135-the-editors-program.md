# 0135. The editor's program

- Status: Accepted; amends [0121](0121-a-game-as-a-simulation-and-a-presentation.md) (the commands
  sent to a host go through its simulation), [0122](0122-the-development-channel.md) (version 7 of
  the protocol, as built) and [0134](0134-the-editors-core.md) (what the program does with the
  journal that a crash leaves, the shape of the program, the level's file named in its journal, and
  a level not written over once changed apart from the editor); amended by
  [0156](0156-the-editors-window-and-its-operations.md) (the program opens a window, and its run
  without a window runs its frames on demand)
- Date: 2026-10-23

## Context

[ADR-0134](0134-the-editors-core.md) decided that a program of the editor's own hosts the editor's
core, the editor's process, which gains its window at Stage 6c; that it runs on `slime-app` as a
game does, its simulation the editor's core; that the editor's requests reach it through methods
of `Simulation`, a command that the channel or the dev overlay sends to it being a step of its
history; that two programs call the editor's entry point, the example `editor` and
`games/hunnawood-editor`; that the development channel gains the editor's requests, version 7 of
its protocol, and the MCP server a tool for each; and that a program keeps its journal in the
editor's directory of the user's data directory unless `--journal` names another file. It left to
Stage 6b's milestone M6b.1b what a program does with the journal that a crash leaves.

Building it raised what ADR-0134 did not say: how a program without a window waits for its tools,
where in the data directory a journal goes, what keeps two editors from writing one journal, or
one level, what a program does with a journal that it cannot read, and the shape of the requests.

The owner decided on 2026-10-23 what a program does with a crash's journal, among the options that
"Alternatives considered" lists, and validated the rest of the design below the same day.

## Decision

**A journal that holds what the file does not is recovered.** As it starts, the editor's program
recovers its level's journal when the journal holds a step made, undone or done again after the
level was last written, or opened: what a crash leaves, and a stop without writing too, since
stopping does not write the level. It plays the journal again (`Editor::recover`), with its
history, and only once every entry is played goes on writing the same journal: a recovery that
fails leaves the journal as it was. Its log, and the history that a tool reads, say how many entries
it recovered. A journal that holds no such step is started anew from the level's file. `--discard`
starts from the file whatever the journal holds, which is set aside beside it, as
`<journal>.discarded`, or `<journal>.discarded.2` and on, never over a journal set aside before
(`slime_app::set_aside`, with which *Hunna Wood* sets aside a save it cannot read). A journal that
cannot be played again, its level's file changed apart from the editor, an entry refused or the
last one torn by a crash of the system, stops the program with a message that names `--discard`;
so does a journal that cannot be read, or is not text, rather than being taken for none.

**The program.** `slime_editor::run` takes a `Program`: its name, its level, the key that the game
spawns the level under, its cooked data, and what registers the game's types. Its options name
another level, `--level`, other cooked data, `--assets`, another journal, `--journal`, and
`--discard`. It runs `slime-app` without a window: `AppConfig::pace_headless` schedules its frames
at the engine's rate, 60 a second by default, the loop sleeping between them, rather than back to
back as a test runs, so that a program that waits for its tools takes no processor meanwhile.

- The example `editor` opens the examples' yard, `examples/assets/yard.scene`, under the key `yard`,
  and knows the engine's types.
- `games/hunnawood-editor`, *Hunna Wood*'s editor package, opens the game's zone under the key
  `zone`, and knows the game's types through `hunnawood::rules::register`, which the game makes
  public. The game does not depend on it.

**The journal's place.** A journal is `slime-editor/<the level's asset ID>.journal` in the user's
data directory, or `<asset ID>.2.journal` and on: the first that does not exist, or whose first
entry names the level's file, by its whole path, without a link or a `..`. The asset ID tells one
level from another, whatever project holds it, and the file tells a copy of a level from the level
copied, since a copy keeps its asset ID. A journal given with `--journal` whose first entry names
another level's file, or another key, is refused. `slime_app::data_directory` gives the user's data
directory, which *Hunna Wood* gave itself until now.

**One editor of a journal, and of a level's text.** One program at a time holds a journal:
`slime_app::HeldFile`, the lock of a file beside it that the system lets go when the program ends,
a crash included, which *Hunna Wood* held its save with until now, keeps a second editor from
writing the same journal, and that editor stops. Two editors of one level with journals of their
own both open it; the editor writes its level only while the file is the text that it read or last
wrote (`EditError::Changed`), so that the second to write does not write over the first, and says
so. A journal that cannot be written once a step is made, the disk full, stops the program: the
step stands, and nothing more is done that the journal would not hold.

**Commands go through the simulation.** `Simulation::apply` applies a command that the host
received, from the presentation, the dev overlay or a tool's `set_property`: by default to the
world, as before; the editor's makes it a step of its history, labelled with the command's kind and
entity. The commands that a simulation refuses are kept with why, as an `AppError`.
`Simulation::edit`, which the `development` feature brings, answers an editor's requests; a game
leaves it to its default, which answers that the program is not an editor.

**Version 7 of the protocol.** `Request::Edit` carries an `Edit`: a step, its label, its commands as
text and the selection after it; undo and redo, which fail when no step is there; the history, the
level's file, the steps done and undone, the selection, what was recovered and the journal's file;
the journal's text; a selection; the level written back, which answers with what the file could
not hold. The MCP server gains `edit`, `undo`, `redo`, `select`, `editor_history`,
`editor_journal` and `write_level`, and starts an editor with `start_game`, as it starts a game.

## Alternatives considered

What a program does with a crash's journal:

- **Refuse to start** until `--recover` or `--discard` says what to do: nothing happens that was not
  chosen, but agents and scripts stop until they pass the option, and a stop without writing asks
  the question at each start.
- **Start from the file**, the journal set aside and named in the log, recovered on demand with
  `--recover <journal>`: never a surprise, but the work not written is forgotten unless someone
  thinks of it.

How a program without a window waits for its tools:

- **A loop of its own** that waits for the channel's requests: nothing runs while nothing happens,
  but `slime-app`'s answers to the world's requests shared or written again, which ADR-0134
  rejected with the program's own loop.
- **Frames back to back**, as a headless run had them: no change to `slime-app`, but a processor
  kept busy while the editor waits.

Where the journal goes:

- **By the level's key**, `zone.journal`: readable, but two projects of levels under one key would
  share a journal.
- **By the asset ID alone**: one name per level, but copies of a level, which keep its asset ID,
  would share a journal, and an editor of one would play the other's steps on it.
- **By a hash of the level's whole path**: one journal per file without reading any, but the name
  says nothing of the level, and a level moved or renamed loses its journal without a word, where
  the asset ID and the file named inside find it, or say why not.

## Consequences

- A person or an agent who stops the editor without writing finds the work at the next start, and
  passes `--discard` to drop it.
- `slime-editor` depends on `slime-app`, and so on what a window and a GPU need, which the program's
  window uses at Stage 6c.
- *Hunna Wood* takes its data directory and the lock of its save from `slime-app`, unchanged for the
  player.
- A journal grows across recoveries, each going on with the one before, until a start finds
  nothing unwritten; the history survives a stop only with something unwritten.
- `scripts/check.sh` runs agents' sessions of the editor, without a compositor, on a copy of the
  yard and on a copy of *Hunna Wood*'s zone, with the game's types, and the zone written back by
  *Hunna Wood*'s editor with the tests that read the zone.

