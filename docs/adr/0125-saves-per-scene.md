# 0125. Saves per scene, scenes unloaded and adoption

- Status: Accepted; amends [0087](0087-the-worlds-documents.md) (layout 5 of saves, the earlier
  layouts no longer read), [0088](0088-saving-and-resuming-a-run.md) (*Hunna Wood*'s saves made
  without a start world), [0093](0093-saves-and-hot-reload-with-placements.md) (a run carried
  without the start of the scenes before they changed),
  [0101](0101-saves-and-identities-that-never-mislead.md) (`World::spawn_scene` unloads the scene
  before it under its key), [0112](0112-levels-saves-and-worlds-for-open-worlds.md) (how a scene's
  base is made and what resets), [0122](0122-the-development-channel.md) (version 2 of the
  protocol: saving and loading a running game) and
  [0124](0124-driven-components-keys-and-created-entities-in-levels.md) (the driven mark read at
  the base)
- Date: 2026-10-21

## Context

[ADR-0112](0112-levels-saves-and-worlds-for-open-worlds.md) decided that saves are made of the
differences of each scene from its *base*, what it spawns and what the game adds when it spawns it,
without a start world made again; that the differences of a scene unloaded are kept, and applied
again when it loads; that a game chooses per scene and per entity what persists and what resets;
and that a game *adopts* an entity, giving it a key of its own, so that `World::spawn_scene` no
longer refuses a level because something of the one before it lives. It left to this milestone how
the base is made, and whether the saves of earlier layouts are still read.

Until now `World::save` compared the world with a start world that the game made again at each
save, entity by entity: a save of a level of 100 000 entities of which 1 % changed spent nearly
all of its time on what did not change (`world.save.100k`, `docs/measurements.md`). *Hunna Wood* made its start again at each save, and at each hot reload made
the start of the scenes before they changed. A level spawned under a key could not replace another
while an entity of that one lived, and P1 worked around it with a key per level.

The owner took four decisions on 2026-10-21, one at a time:

- **Q1, the base**: kept as it was, a copy of a value taken at its first write after its entity was
  settled, rather than the game's preparation run again, or a copy of the whole scene.
- **Q2, what persists**: a component, `Persistence`, authored in scene files: on a scene's root for
  its scene, on an entity for itself, persistent by default; `Unsaved` and driven components stay
  as they are.
- **Q3, the saves of earlier layouts**: refused clearly, and *Hunna Wood* sets such a save aside, says
  so, and starts a new run; its fixture is made again, which
  [ADR-0100](0100-updates-of-shipped-games.md) allows before the game's first release.
- **Q4, saving a running game from the MCP server**: through the game's simulation, which saves and
  loads as it does for its player, the development channel asking it at the start of a frame.

## Decision

**A world is settled, and its saves are what changed since.** `World::settle` makes what the world
holds its base: the first time every entity that has an identity, later what was spawned since, the
scenes spawned under keys and the placements, as the game prepared them. A scene or a placement is
settled as it spawns already, its values as the scene gives them; settling it again makes what the
game did to it since part of it. From then on:

- the first write of a component, its first insertion or its removal, copies the value that it had,
  or its absence, which the storage of its type keeps by slot, and a parent's first change keeps the
  parent before: these copies are what changed, and what a save reads;
- the world keeps the identity of each entity settled that the run destroys, with the
  `Persistence` that it held then, but those that saves leave out: what a save removes;
- what the run creates after the start, outside scenes or under keys, and what it adopts, is not
  settled: a save holds it whole.

`World::save` takes no start any more. It refuses a world never settled, `DocumentError::NotSettled`,
and still a start that created, without keys, entities that saves hold. It writes layout 5: the
scenes spawned under keys in `[scenes]`, by key, with their asset IDs, names and fingerprints, so
that loading spawns those that the world lacks or holds others in place of; the placements in
`[spawned]`, by their roots' identities; the entities removed, the components removed, added and
changed and the parents changed, each compared with the base kept; what the run created, whole; the
former identities of what it adopted in `[adopted]`; and what the world keeps of the scenes that it
unloaded, in `[unloaded.<key>.<asset>]`, laid out as the save's own differences. It costs what
changed, about a hundred times less than before for a level of 100 000 entities of which 1 %
changed, its size the same (`docs/measurements.md`).

`World::load` applies a save to a world that the game made as at the start of a run, and settled,
settling one that it never settled. A scene that the save names and the world holds is kept; one
that it lacks or holds another in place of is spawned in place of the world's, and the game prepares
it through a callback, `prepare`, given its key, before the world is settled and the save applied.
A scene that the world holds and the save does not name stays, as one that a later version of the
game spawns at its start; one that the save says the run unloaded is unloaded. `World::carry`
carries a run into a world made from the scenes as they are now, through a save and a load, without
the start of the scenes before they changed.

The saves of layouts 1 to 4, those of 0.3, of 0.4, of P1 and of M6a.7a, are refused, before the
world changes, with `DocumentError::Layout`, which says that an earlier version of the engine wrote
them; `World::about` refuses them too, so that a game tells such a save from its own without loading
it. A document that holds nothing is not a save.

**A scene unloaded keeps its differences.** `World::unload_scene` destroys a scene's entities,
taking out of them first what is under them without being of the scene, and forgets the scene,
keeping by its key and its asset what the run changed of it since it was settled: the identities
that its values refer to are kept, as a save names them, rather than handles. When the game spawns
the same scene under the key again and settles it, the world applies them, as a save applies, after
the game's preparation: a door broken open stays open. `World::spawn_scene` under a key that holds a
scene unloads that one first, once it checked that the new scene spawns, so that a refused scene
leaves it: a run changes level, or spawns a level again, under one key.

**What resets.** `Persistence::Reset`, held by a scene's root for its scene, or by an entity for
itself, whatever its scene's, makes unloading keep nothing of that entity: spawned again, the scene
gives it as its file does, an enemy that comes back. While its scene is loaded, what the run changed
of it is saved as anything else: a game resumed in the level finds the enemy as the run left it. A
game that brings enemies back when the hero rests spawns the level again under its key.

**Adoption.** `World::adopt` takes an entity of a scene, below its root, or of a placement, and
the descendants whose identities are under its own, out of the scene under a key of its own:
`level/lamp 3` becomes `companion`, and `level/lamp 3/light` `companion/light`. The world keeps
its former identity with the scene's asset, and saves keep them: references written before, as
those of a scene unloaded, find it through it. Its scene, saved or unloaded, says that its records
are gone, whatever the scene's mark; spawning the scene again gives what of the scene refers to
such a record, or is under it, to the entity adopted, and loading a save into a world that holds
the former entity, of the same scene, adopts it again, so that what refers to it in its scene
follows it. A save holds what was adopted whole, as what the run created. A child whose identity
is not under the entity's own, as a scene file's entity whose parent it names, stays of its scene,
and the game adopts it too to keep it. The key of an entity adopted stays its own, alive or not: no
entity, scene or other adoption takes it, so that its former identity names one entity.

**Saving and loading a running game.** `Simulation` gains `save` and `load`, which a game that
saves its runs implements as it saves for its player, and which refuse by default. The development
channel asks them at the start of a frame, `Request::Save` and `Request::Load`, with a file of the
tool's: version 2 of the protocol. The MCP server gives them as `save_game` and `load_game`.
*Hunna Wood* saves the run to the file, and loads it as F9 does, the run left as it is when the save
is not resumed.

*Hunna Wood* settles its new game by day, keeps the layout of its zone as the run's start placed it,
which its saves say, and makes no start to save or to carry a run: a save of an earlier version of
the engine is set aside as `save.toml.earlier-version`. The fingerprint of a zone's layout that the
saves of 0.4 said is gone with them.

## Alternatives considered

The base:

- **The game's preparation run again** at each save, on the scene as it spawns: no copy kept, but a
  game's preparation would have to be pure and cheap, and a save would cost every scene again.
- **A copy of each scene at its settling**: simple to compare, but the memory of every value of every
  scene loaded, where copies taken at the first write cost what changed.
- **A start world made again**, as before: it cost what did not change, and a streamed world never
  holds a whole start.

What persists:

- **A list of what persists kept by the game in code**: nothing in scene files, but the editor could
  not show or set it, and two places would say it.
- **A reset that waits for a signal of the game while the scene is unloaded**, kept until then:
  ADR-0112 named both a reset when the scene loads again and one on a signal; spawning the scene
  again is that signal, and keeping what is to be forgotten would cost memory and a second rule.

The saves of earlier layouts:

- **Read and converted**: a start world made again once, to compare them with, for saves that only
  the engine's own tests and a game without players hold.

Saving a running game:

- **The host saves the world alone**: no game code, but a game's save holds more than its world,
  its version and its layout, and is checked when resumed; the tool's save would not be the
  player's.

Adoption:

- **Moving the entity to a world of its own**, the transfer of Stage 10a: more than a level needs to
  keep a companion.
- **Keeping its identity under the old key**: what `KeyInUse` refused, since the new level's would
  share it.

## Consequences

- A game settles its world once it made its start, and again once it prepared a scene that it spawns
  while the run goes on; it gives `World::load` and `World::carry` the preparation of the scenes that
  a save spawns.
- Saves cost what changed: the world keeps a copy of each value changed since its base, and the
  identity of each entity settled that the run destroyed, until a scene goes.
- Every save written before is refused, the engine's and *Hunna Wood*'s; its fixture is made again.
- `WorldError::KeyInUse` is gone: a level replaces another under one key, and a companion is adopted.
- A game's preparation of a scene changes the scene's entities only: what it creates, under a key
  or as a placement, is the run's, which a save holds, and which would meet what loading the save
  creates before it prepares the scenes that it spawns (the plan's known debt).
- What the run created under a scene's entities is taken out of them when the scene is unloaded,
  and lives on at the world's root; a scene loaded is reset by being spawned again.
- A tool saves and loads a running game through the MCP server, with files of its own.
