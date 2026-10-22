# 0134. The editor's core

- Status: Accepted; amends [0089](0089-scenes-and-prefabs.md) and
  [0092](0092-a-world-written-back-as-a-scene.md) (the editor keeps the comments of a level that it
  writes), [0109](0109-stable-references-and-undo.md) (the world's commands are the editor's, and
  the journal holds the steps made, not what they deleted),
  [0110](0110-how-the-editor-runs-a-games-code.md) (the editor is a program of the engine's
  application, and a game's editor package comes with Stage 6b),
  [0121](0121-a-game-as-a-simulation-and-a-presentation.md) (applying a command gives back the
  command that undoes it), [0122](0122-the-development-channel.md) (version 7 of the protocol: the
  editor's requests) and [0131](0131-stage-6b-and-stage-6c.md) (Stage 6b's first milestone in two,
  and *Hunna Wood*'s editor package in it); amended by [0135](0135-the-editors-program.md) (what the
  program does with the journal that a crash leaves, the shape of the program, the level's file
  named in its journal, and a level not written over once changed apart from the editor)
- Date: 2026-10-22

## Context

[ADR-0131](0131-stage-6b-and-stage-6c.md) opens Stage 6b with the editor's core, without its
interface, as [ADR-0116](0116-stage-6-in-two-halves.md) decided: the editor's commands, which name
entities by their persistent identities, the journal, undo and redo, the selection, a level opened
and written back, all of which the MCP server gains at once.
[ADR-0109](0109-stable-references-and-undo.md) decided how: commands named by identities, a
deletion recorded and undone by creating its entities again under the same identities at the same
ranks, the references to them pointed at them again, a journal of commands as values that
`slime-serialize` writes, and what the editor creates named at once as the level's file names it.

Where the engine stood:

- **The world took one command**, `Command::SetProperty`, which the host applies at the start of a
  frame ([ADR-0121](0121-a-game-as-a-simulation-and-a-presentation.md)), and the dev overlay and
  the development channel send. Nothing created an entity under a given identity nested in a
  scene's, nor put a child at a given rank among its siblings; `World::despawn` forgot an identity,
  and nothing undid.
- **A level written back lost its comments.** [ADR-0089](0089-scenes-and-prefabs.md) left it open,
  "Stage 6 decides whether its editor keeps them", and
  [ADR-0092](0092-a-world-written-back-as-a-scene.md) wrote files without them. The examples' yard
  and *Hunna Wood*'s zone, written by hand, explain their lamps and lanterns in comments.
- **A value refers to an entity by its handle**, which holds within one run of one world: an
  entity deleted and created again takes another handle.
- **No program held a level to edit.** The development channel answers in a running game, with the
  host's world ([ADR-0122](0122-the-development-channel.md)); a game's editor package, which knows
  its types ([ADR-0110](0110-how-the-editor-runs-a-games-code.md)), was to come with the editor's
  window, at Stage 6c.

The owner took eight decisions on 2026-10-22, one at a time, each among the options that
"Alternatives considered" lists, then validated the design below and the split of the milestone in
two.

## Decision

**The world's commands are the editor's.** `slime_world::Command` gains the commands that the
editor makes, beside `SetProperty`: `Create`, an entity of a scene with its components; `Place`, a
prefab placed in a scene; `Delete`; `Reparent`; `Insert` and `Remove`, a component attached or
detached; and `Restore`, which creates again what a deletion took out. Each names its entities by
their persistent identities, and puts what it creates or moves under a parent at a rank, the last
if none is given. `World::apply` gives back the command that undoes the one it applied: the value
before, for `SetProperty` and `Insert`; `Delete`, for `Create` and `Place`; the parent and rank
before, for `Reparent`; `Restore`, for `Delete`. The dev overlay, the development channel, a game
and the editor send the same commands, and a game that runs accepts what the editor sends.

**A deletion is recorded, and undone by creating it again.** `Delete` records its subtree before it
deletes it (`slime_world::Deleted`): each entity's persistent identity, its components as the values
that reflection reads, its place among its siblings, the placement whose root it is, what the world
remembers of what a file wrote of it, and the components of the other entities that referred to one
of them, found among the types that may hold a reference. Its values refer to entities by their
identities, never by handles. `Restore` creates them again under the same identities, at the same
places, and points those references at them, or changes nothing if an identity is taken; the
components of the other entities that referred to them are set back to their values at the deletion,
which is exact when nothing changed them since, as in the editor's history. An entity that had a
base, which saves compare it with ([ADR-0125](0125-saves-per-scene.md)), as every entity that a
scene spawned has, comes back with its values as its base: what a run changed of it before it was
deleted is no longer told apart. The editor never saves its world; a game that deletes and restores
such an entity loses that difference in its saves.

**The world gives what this needs.** `World::spawn_as` creates an entity under an identity of two
parts, a key under which a scene is spawned and an ID that its file can hold, refused if an
entity has it; `World::place_as` places a prefab so, its root named by its ID unless the prefab
names it, as a file's placement is; `World::new_id` gives a free identity, a
name made fit for a file and four random letters or digits, `zone/torch-0rdx`, as a scene file
written back names what it creates. `World::set_parent_at` puts a child at a rank among its
siblings, and `World::rank` says an entity's.

**Commands are written as text.** `Command::write` writes a command as a document of the engine's
notation, a table named after its kind, its entities by their identities, the entities that its
values refer to too, a prefab by its asset ID; `Command::read` reads it in the world as it is when
the command is about to be applied, the part that `SetProperty` sets giving its value's type. A
restoration has no text: it holds what a deletion took out of one world.

**The editor's core is a library, `slime-editor`.** `Editor` opens a level, a scene file whose
asset ID its metadata file gives, spawned into a world under the key that the game spawns it under,
the prefabs that it places resolved through a library of the cooked scenes. It changes the level
through steps: a step's commands, given as values or as text, are applied in their order, each read
as the world is when it comes, so that one may name what a command before it created; when one is
refused, those before it are undone and the step is not made. A step holds one command at least.
The editor changes only what is under its level's root: it refuses a command that names an entity
of another key, that deletes or moves the root, or that leaves an entity without a parent, out of
what the level's file holds.

- **Undo and redo.** The history holds, for each step done, the commands that undo it, and for
  each step undone, those that make it again: as text, which names entities by identity, but a
  deletion's record, held in memory, of reflected values. Undoing a step applies them and keeps
  what applying gave back, to redo. The history is bounded in memory, 256 MiB unless the editor is
  given another bound; past it, the steps undone furthest from the present are forgotten first,
  then the oldest steps done, but the last one.
- **The selection.** What is selected is a list of persistent identities. Selecting is not a step,
  but each step records what was selected before and after it: undoing selects again what was
  selected before, redoing what was selected after, and a step may select what it creates.
- **The journal.** The editor writes a journal of its steps, as text, in entries named by lines that
  start with `#`. It starts before any step, with the level, its key and its file's text as the
  editor opened or last wrote it; then each step, its label, the selection that it chose, and the
  text of each of its commands; each step undone and each step done again; each selection made
  apart from a step; and each writing of the level. Only the commands as made are written: playing
  the journal again makes the undoing again. Each entry is handed to the system as it is made, so
  that a crash of the editor loses none; it is not forced to the disk. `Editor::recover` opens the
  level from the text that the journal starts with and plays every entry again, the writings
  without writing, so that a step made before the level was written may be undone after it; the
  level's file must then be the text that the journal last wrote, or the file changed apart from
  the editor, and the recovery is refused. It recovers an editor that crashed, with its history
  and its selection, and replays a session of tests or of an agent. A program keeps its journal in
  the editor's directory of the user's data directory, one per level, unless `--journal` names
  another file, as `--save` names a game's save.
- **The level written back.** `Editor::write` writes the level as `World::write_scene` writes it,
  whole or not at all, through a file beside it that is renamed, as the cooker writes its files.
- **Comments are kept.** `slime_serialize::keep_comments` puts the comments of the file as the
  editor opened or last wrote it back where they were in the text written: each before the header
  or the key that it preceded, found by its path of keys whichever way each text writes it, or
  before the line that holds that key within a value; those at the top and at the end of the file,
  at the top and at the end; one at the end of a line, at the end of its key's line. The comments of
  what the level no longer holds go with it.
- **Placing a prefab** is one of the editor's commands from the start: the levels are made of
  placements, and undoing a placement's deletion creates a placement again in any case.

**A program of the editor's own hosts the core**, the editor's process, which gains its window at
Stage 6c: it opens a level, holds its world without running the game's rules, and listens on the
development channel, through which an agent drives it as it drives a game, started by the MCP
server's `start_game`. It runs on `slime-app`, as a game does: its simulation is the editor's core,
without a window until Stage 6c, the frames of a run without a window paced, and its window,
presentation and recovery from a lost GPU device are `slime-app`'s then. The channel's requests on
the world are answered by `slime-app` as for a game, and the editor's requests reach its simulation
through methods of `Simulation` that a game leaves to their defaults, which answer that it is not an
editor, as `Simulation::save` answers that a game does not save. A command that the channel or the
dev overlay sends to an editor is a step of its history, which undoes.

- **Two programs.** `slime_editor` gives the program's entry point, and two programs call it: the
  example `editor`, which knows the engine's types and opens the examples' yard, and
  `games/hunnawood-editor`, *Hunna Wood*'s editor package
  ([ADR-0110](0110-how-the-editor-runs-a-games-code.md)), which knows the game's and opens its zone.
  CI's session of an agent runs on the first, whose assets the repository holds.
- **Version 7 of the protocol.** The channel gains the editor's requests: a step of commands as
  text, undo, redo, the history, the journal, the selection read and set, and the level written
  back. The MCP server gains a tool for each.

**When it is built.** Stage 6b's first milestone is split in two, as M6a.10 was: M6b.1a, the
commands and undo, the world's part and the library `slime-editor`, with the journal, the levels
and their comments, and a benchmark; M6b.1b, the editor driven by an agent, the program, the two
packages, the channel's requests and the MCP server's tools.

Measured in a release build on the owner's machine (`slime-benches`'s `world.undo.1k`,
`docs/measurements.md`): a subtree of 1 000 entities, each with a name, a transform and a light, in
a level of 100 000, is deleted in 1.4 ms and restored in 1.8 ms, 3.2 ms in all, the median of
twenty; the audit of 0.4 estimated 10 to 15 ms.

## Alternatives considered

The program that hosts the editor's core:

- **The `world` example**, the commands applied to its world as it runs: nothing new to start, and
  the agent sees each change in a capture; but the ring floats and the run creates while the level
  is edited, what the run creates keeps `created/<n>`, a level written back holds the run's state,
  and the work moves when the editor gets its process at Stage 6c.
- **Both**, the editor's program and a game that runs accepting the same steps, undone: ready for
  play in the editor, but more work now, for a need that comes with Stage 6c's play.

Where the commands live:

- **A type of commands of the editor's own**, built on the world's public API, `SetProperty` kept
  for the overlay and the channel: the world barely changes, but two types for one change, to be
  unified, the world exposing what finds the references to a subtree, and a running game accepting
  none of the editor's commands until then.

The programs:

- **The example alone**, *Hunna Wood*'s editor package at Stage 6c: the smallest, but the entry
  point with one user, the game's zone, which holds types of the game, not editable by an agent
  before 0.7, and ADR-0110's packages checked only then.
- ***Hunna Wood*'s alone**: the real level at once, but the examples would not show the editor, and
  CI's session would depend on the game's fetched files.

What the program runs on:

- **A loop of its own**, listening on the channel itself and waiting for requests without a
  frame: `slime-app` unchanged and nothing that runs when nothing happens, but the answers on the
  world shared or written twice, and at Stage 6c what `slime-app` does with a window, the GPU, its
  loss and the capture written again, or the program moved onto `slime-app` then.

The comments:

- **Lost**, as ADR-0092 wrote files: nothing to build, but a file kept by hand and by the editor
  loses its comments at each save, and *Hunna Wood*'s zone loses its own the first time an agent
  writes it.
- **A component of notes**, as the actors of comments of Unreal: text that the inspector shows and
  that survives as data; but the comments of sections, of the ends of lines and of overrides are
  lost, and every game carries one more component.

The selection:

- **A step at each change**, as Unity, Unreal and Blender do: what was selected comes back exactly,
  but the history fills with clicks, an agent's too, and a change is undone in several steps.
- **Out of the history**: the simplest, but a deletion undone leaves nothing selected, and a step
  undone may leave selected what no longer exists.

The journal:

- **In the engine's binary encoding**: compact and fast, but unreadable without a tool, converted
  for agents, and compared as bytes in tests.
- **Played from its last entry of the level**, the level's text not kept, a fingerprint of the file
  written at each writing: a smaller journal, but an undo of a step made before the level was last
  written could not be played again, and the recovery would rebuild another level than the one
  that crashed.
- **Beside the level's file**, `zone.scene.journal`: found with the level, but in the directories
  of assets, which git, the cooker and `slime-cook check` would have to learn to leave out.

Placing a prefab:

- **At Stage 6c, with placing in the viewport**: Stage 6b's first milestone kept to the plan's
  list, but an agent creates only entities without a prefab at 0.6, and cannot add a lantern as the
  zone places them.

How the history holds commands:

- **As values, their references by handles**, the history walked to point them at the new handles
  each time an entity is created again: no text read when undoing, but each restoration costs a
  walk of the whole history.
- **A deletion's record as text too**: one form for everything held, but a thousand entities
  written and read again at each undo, where values in memory cost a copy.

The milestone:

- **One milestone**, about 5 500 lines with their tests: one review and one run of CI, but larger
  than any milestone so far, which ADR-0116 found hard to review at 5 000.

## Consequences

- `World::apply` returns a command: a caller that only applies, as a host does, drops it.
- The examples' yard written back by the editor, unchanged, is the file that people wrote, byte
  for byte, its comments included.
- `slime_cook::write`, which writes a file whole or not at all, is public: the editor writes its
  levels through it.
- The development channel's `Identities` moves into the world, `slime_world::Identities`, which the
  commands' text and the channel share: a reference written `""` reads as no entity.
- *Hunna Wood*'s editor package, `games/hunnawood-editor`, comes with M6b.1b rather than with
  Stage 6c; the game makes public what registers its types.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6b's editor's core is a program of its own, on
  which a game's editor package starts.
