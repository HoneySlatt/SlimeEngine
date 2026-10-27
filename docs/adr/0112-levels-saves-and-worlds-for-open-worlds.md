# 0112. Levels, saves and worlds for open worlds

- Status: Accepted; amends [0079](0079-the-engines-own-world.md) (identities across the cells of a
  streamed world), [0087](0087-the-worlds-documents.md), [0088](0088-saving-and-resuming-a-run.md)
  and [0093](0093-saves-and-hot-reload-with-placements.md) (saves made of differences per scene, and
  what persists), [0089](0089-scenes-and-prefabs.md) (references to entities of other scenes, former
  identities written by the editor, and hot reload without a start world),
  [0091](0091-scene-documents-and-placements.md) (references written as absolute identities),
  [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) (saves that cost what changed,
  brought to Stage 6) and [0101](0101-saves-and-identities-that-never-mislead.md) (an entity that
  outlives its scene); amended by [0125](0125-saves-per-scene.md) (how a scene's base is made, and
  what resets) and [0155](0155-the-editors-design.md) (how a level names its parts, and the layout
  of scene files changed once, with `EntityRef`'s)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) studies open worlds before Stage 6's editor writes levels: how a
level is cut into files, identities, saves per scene and references across scenes (C3-02, C4-03,
C4-05, C7-03). It builds no streaming, which is Stage 10a's; it decides what the editor writes in
files from Stage 6, the dearest to change once levels are built and players hold saves.
[ADR-0098](0098-the-games-the-engine-aims-at.md) adds several worlds, and a state that follows the
player from one to the other; a whole region loaded during play, then switched to; content of a
scene that depends on the state of the story, which belongs to one player or to a step of the story
that all share; cells that serve streaming and, one day, the split of a world between servers.
[ADR-0109](0109-stable-references-and-undo.md) left identities of 128 bits to weigh here. The audit
of 0.4 measured the figures below on worlds of up to a million entities made for it, on the owner's
RX 6900 XT at `v0.4.0`; its report is kept outside the repository, as the plan says.

- **A level is already a scene that places others**: *Hunna Wood*'s zone places its land and six
  lanterns. An entity's persistent identity is the path of IDs through the placements
  ([ADR-0089](0089-scenes-and-prefabs.md)): changing an entity's `Name`, or its parent in its file,
  keeps it; changing its ID, as renaming a glTF node in Blender does, or moving it out of its file,
  changes it. ADR-0089 left *former identities*, a file saying what an entity was called before, to
  Stages 9b and 11 at the latest, no game having players to protect; the editor moves entities
  between files whenever it makes a prefab of a selection or moves an object into another of the
  scene files that make a level. [ADR-0079](0079-the-engines-own-world.md) left identities
  across the cells of a streamed world to Stage 10.
- **A file names no entity outside itself** (ADR-0089,
  [ADR-0091](0091-scene-documents-and-placements.md)): a reference between scenes, a lever in one
  that opens a door in another, can only be made in a game's code. In a world, it works, and a save
  writes it by its target's identity ([ADR-0087](0087-the-worlds-documents.md)); loaded into a
  world that does not hold its target, it becomes no entity, with a report, and the next save writes
  it so: a streamed world, whose regions come and go, loses it for good.
- **A save is the difference from a start world that the game makes again whole.** `World::save`
  writes what differs from a start world, which *Hunna Wood* makes again at each save
  ([ADR-0088](0088-saving-and-resuming-a-run.md)); a scene spawned while the world ran is compared
  with its own resolved scene ([ADR-0093](0093-saves-and-hot-reload-with-placements.md)). Saving
  100 000 entities of which none changed takes 0.72 s, 63 % of it comparing what did not change,
  which [ADR-0108](0108-what-the-world-knows-of-its-changes.md)'s ticks take away; the start world
  must still be made whole, which a streamed world never holds.
- **Nothing outlives its scene.** A companion met in a level belongs to it; when a level replaces
  another under its key, `World::spawn_scene` refuses it while an entity of the earlier one lives,
  and a level that keeps something of the one before goes under a key of its own
  ([ADR-0101](0101-saves-and-identities-that-never-mislead.md)), a workaround that P1 left to this
  study. Worlds are values, and a game holds as many as it needs (ADR-0079), but nothing moves an
  entity from one to another.

Open-world games keep their state two ways. Some keep what changed of every object, so that an
apple put on a table stays there a hundred hours later; their saves grow, and they need rules to
reset places. Others keep only what their design names, flags of events, a door opened, a boss
defeated, and load each area from its data again, enemies coming back when the player rests; their
saves stay small, and a thing that must persist is planned for. Most mix them.

The owner took four decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**A level is a composition of files, cells are not identities, and the editor writes former
identities.**

- A level is a scene file that places others, its parts; the editor never treats a level as one
  file, and can open some of its parts only.
- Cells are not part of an entity's identity: they are groups that cooking computes from the level,
  at Stage 10a, and moving an entity in the world never changes its identity.
- When the editor moves an entity out of its file, as a prefab made of a selection or an object
  moved into another part, the file that holds it then writes its former identity, so that saves,
  overrides and references that named it find it again. ADR-0089's former identities come at Stage
  6, with the editor that needs them.
- Identities stay paths. Identities of 128 bits, which ADR-0109 left to weigh here, would follow an
  entity across files and cells; cells kept out of identities and former identities give the same,
  readable.

**References to entities of other scenes.** A new kind of reflected value, `EntityRef`, names its
target by its persistent identity and finds its handle when asked, when the target is loaded. Files
and saves write it as an absolute identity, its whole persistent identity, `village/door`, where a
file writes its references as paths from its own root: a new layout of scene files, amending
ADR-0091. It is never cleared when its target is missing, but waits for it, and a game asks whether
the target is there. `Entity`, a handle, stays for links within a scene and while a game runs; the
components that hold one change nothing. The editor's inspector picks an entity of any scene for it.
It is built at Stage 6, with the inspector. Loading together what refers to each other, as a
complement for streaming, comes with Stage 10a.

**Saves are differences per scene, and a game chooses what persists.**

- Each scene that a world spawned, a level, a part of one, a placed prefab, is compared with its
  own resolved scene: no start world is made again to save. When a scene is unloaded, its
  differences are kept, and applied again when it is loaded. A save holds the differences of every
  scene that changed, loaded or not, what the run created (`created/<n>`) and what has a key.
  A scene's base is what it spawns and what the game adds to it when it spawns it, as *Hunna
  Wood* adds its key, its door and its clock to the entities of its zone: the game's code that
  prepares a scene is part of making its base, so that a save holds what the run changed, not what
  every run adds, which [ADR-0087](0087-the-worlds-documents.md) chose a start world for. How the
  base is made, the preparation run again or kept as it was, is decided when it is built.
  Through ADR-0108's ticks, a save costs what changed. Carrying a run into scenes that changed on
  disk, `World::carry`, goes the same way.
- A game chooses, per scene and per entity, what becomes of its differences:
  - *persistent*: kept for good, a door broken open, a chest emptied;
  - *reset*: forgotten when the scene loads again, or when the game signals it, enemies coming back
    when the player rests;
  - *never saved*: what is `Unsaved` or driven (ADR-0108), as today.
- The state of a game's progress, its flags, lives in entities that have a key, which saves hold.
- Saves take a new layout: *Hunna Wood*'s fixture is made again, which
  [ADR-0100](0100-updates-of-shipped-games.md) allows before the game's first release.

**What outlives a scene.**

- *Adoption*, at Stage 6: a game takes an entity out of its scene and gives it a key of its own,
  `companion`; its former identity is written, as the editor writes it, so that saves and
  references follow it, and it no longer ends with its scene. `World::spawn_scene` no longer refuses
  a level because something of the earlier one lives: what must outlive it is adopted, and what of
  the earlier level is not adopted ends with it.
- *Transfer between worlds*, at Stage 10a: a region is prepared in the background in a second world,
  its work run as [ADR-0107](0107-the-execution-model.md)'s background tasks, its copies to the GPU
  within the budget per frame; the game transfers into it what follows the player, the adopted
  entities, which keep their identities, then switches to it. Within a region, streaming loads and
  unloads scenes in one world.
- *Layers that depend on the story*, at Stage 9b: in a scene file, a part of a level, a *layer*,
  holds a condition on the state of the story, "after the quest X"; that state lives in entities
  that have a key, of one player or of a step that all share; the editor shows or hides each layer.
  Its form is fixed now, so that the editor of Stage 6 writes levels that layers can be added to; it
  is built with Stage 9b's quests.

**When it is built.** At Stage 6: levels of several files, former identities, saves per scene with
what persists, adoption, and `EntityRef` with the inspector. At Stage 9b, layers. At Stage 10a,
cells, transfer between worlds, and loading together what refers to each other.

## Alternatives considered

Cutting a level and identities:

- **Everything left to Stage 10**: nothing now, but the editor would choose alone, and levels of
  one file or identities tied to files would have to be cut by a script, and saves migrated.
- **Identities of 128 bits per entity**, as some engines keep: an identity that nothing changes; but
  files and saves that a person cannot read, which ADR-0079 rejected, new identities for every
  entity duplicated or pasted, and the scene files and saves of today to migrate, for what cells
  kept out of identities and former identities give.

References across scenes:

- **Links made in a game's code**, as today: each game makes its own registry of identities, the
  editor cannot make the links, and saves lose those whose target is not loaded.
- **Loading together what refers to each other** alone: no reference dangles within a region loaded
  together, but links between levels, quests that name far entities and saves stay unsolved.

Saves:

- **A start world made again whole**: nothing changes, but every save makes the whole start, which
  a streamed world cannot do for a region it has unloaded.
- **The whole state of the world, without differences**: no start to make, but large saves, and an
  update of a game could no longer fix what the player did not touch, which ADR-0087 chose
  differences for; still impossible for an unloaded region.
- **Every difference kept for good**, without a choice of what persists: general, but saves that
  grow without end, and no enemies that come back.

What outlives a scene:

- **Everything left to Stages 9b and 10a**: the companion stays blocked by P1's workaround, and the
  editor writes levels without knowing how to say "this part exists after that quest".
- **One world for good**, every level loaded into it and the player never leaving it: no transfer
  between worlds, but no region prepared apart without mixing two places, and no world split between
  servers one day; adoption and layers are needed all the same.

## Consequences

- The editor writes levels as several files, and former identities when it moves an entity out of
  one; a move in the world changes no identity.
- A link between scenes is data that the editor makes and saves keep, its target loaded or not.
- A save is made without a start world, costs what changed, and holds what a game chose to persist;
  whether saves of layout 3 are still read is decided when it is built, no shipped game holding one
  (ADR-0100), and *Hunna Wood*'s fixture is made again.
- A companion, a hero, what follows the player is adopted; `World::spawn_scene` no longer refuses a
  level because something of the earlier one lives.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6 builds levels of several files, former
  identities, `EntityRef`, saves per scene and adoption; Stage 9b layers; Stage 10a cells and
  transfer between worlds.
