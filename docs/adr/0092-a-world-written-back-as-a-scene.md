# 0092. A world written back as a scene

- Status: Accepted
- Date: 2026-10-17

## Context

[ADR-0089](0089-scenes-and-prefabs.md) decides that the world remembers its placements and writes a
world, or a subtree of it, back as a scene file: placements stay placements with what differs from
their prefab, an entity created since gets an ID when it is first written and keeps it, children keep
their order, entities marked `Unsaved` and transient fields are left out, and the same world gives
the same bytes. Stage 6's editor saves a level so. M5b.2 builds it
([ADR-0090](0090-stage-5b-scenes-and-prefabs.md)).

Where the engine stood after [ADR-0091](0091-scene-documents-and-placements.md):

- The root of each placement remembers its prefab, resolved, and its overrides as the file wrote
  them, those set aside included. The world did not keep the scenes that it spawned, only their
  names and fingerprints, and a resolved scene was not shared but through an `Arc` that a library
  held.
- An entity created outside a scene has the identity `created/<number>`, and no ID that a file could
  give it.
- `slime-serialize` writes every field of a value, and has no way to make a table that holds nothing,
  which an entity of a scene file without components or parent needs.
- No code places a prefab while a world runs: [ADR-0089](0089-scenes-and-prefabs.md) gives it to
  M5b.3, with its saves.

## Decision

**`World::write_scene(root)`** writes the subtree of `root` as a scene file, and reports what the file
cannot hold (`Written::left_out`).

**What a file's root may be.** The root of a spawned scene, an entity that a scene's file makes, or
an entity created outside a scene, as an editor starts a level. A placement, and an entity of one, are
refused: the scene that places them writes them, and making a prefab of a selection is Stage 6's.

**What the file holds.**

- The root's components are the file's root's, but for the `Name` that spawning gives it, the last
  ID of its identity, and a `Transform` that is the identity.
- An entity of the root's scene, `zone/door` under `zone`, is a record under its ID, with its parent
  and its components, whole.
- An entity created outside a scene is a record under an ID that the world gives it when it first
  writes it: its `Name`, or `entity`, and four random letters or digits, `torch-k3f9`, unique in the
  file and in the scene, as [ADR-0079](0079-the-engines-own-world.md) gives a tool's entities random
  IDs. The world keeps the ID, and writes it again; the entity keeps its identity, `created/<number>`,
  which saves name it by, until the file is spawned again.
- A placement is a record that names its prefab. Its overrides are what differs between its entities,
  found by their identities, and its prefab's records: an entity removed, the topmost of a subtree; a
  component removed or added, whole; the part of a value that changed. The overrides that the scene
  that placed it wrote are kept: one whose fields hold the prefab's values still is written with
  them, the world's values in its fields; one set aside, whose entity or component the prefab no
  longer has, is written as the file wrote it; a component that the file added stays added.
- Entities that hold `Unsaved`, with their descendants, transient fields, and fields that hold their
  defaults are left out.
- A reference to an entity is its path from the root, `.` the root; one to an entity out of the
  subtree is `""`, and reported. Assets are named by the short names of the table of the file that
  the scene was spawned from, else a prefab's name, else `asset 1`, `asset 2`...

**One form for one world.** The assets in the order of their names, the root, the records in the
order of a walk of the hierarchy, children in their order, then the overrides in the order of their
paths and of the components' names, through the canonical writer of `slime-serialize`. A placement's
entities keep its prefab's order; the file's entities under one of them come after the prefab's
children. Writing a world again gives the same bytes, and the file spawned again gives a world that
writes the same file.

**What a file cannot hold is reported, and left as the world has it.** An entity of the subtree that
belongs to another scene is left out, with its descendants. An entity of a placement moved out of the
place that its prefab gives it is written in that place: a scene file has no parents for a placement's
entities. A reference, or a parent, that the file of a scene wrote into a placement whose prefab no
longer had its entity, and that spawning set aside, is written as the world holds it, no entity or the
root: what it named is lost, as a save loses it ([ADR-0087](0087-the-worlds-documents.md)). The
comments of the file that the scene was read from are lost, as ADR-0089 says.

**For an inspector.** `World::placement_root` gives the outermost placement that an entity belongs to,
whose prefab a file compares it with; `World::differs_from_prefab` says whether a component of it
differs from the prefab's: what Stage 6 marks as overridden.

**A placement made while the world runs.** `World::spawn_placement(prefab, parent)`, which ADR-0089
gives to M5b.3, comes with M5b.2, so that an edit can add a placement: its root is
`created/<number>`, its entities `created/<number>/<ID>`, and the world remembers its prefab. Saving a
world that holds one waits for M5b.3.

**What the world and `slime-serialize` gain.** A resolved scene is a shared handle, whose clone costs
a count; the world keeps each scene that it spawned, its table of assets among it.
`Composer::without_defaults` leaves out the fields of whole values that hold their defaults, which a
reader gives again; `Composer::table` makes a table that holds nothing yet; `Value::same` compares
values with their floats by their bits.

**What it costs.** Measured in a release build on the owner's machine: a world of a thousand
placements of a prefab of 10 entities, 11 001 entities, is written in 0.11 s; of a prefab of 100
entities, 101 001 entities, in 0.87 s; both files hold 136 kB, the prefab's entities placed, not
copied.

## Alternatives considered

- **IDs from the numbers of the entities created, `created-5`**: no random letters and no write to the
  world, but a number given again in another session would name another entity, and two branches of a
  level would give the same IDs.
- **Entities created outside a scene given the identity that their ID makes when first written**: the
  world and the file would agree at once, but a save made before would no longer find them.
- **Overrides found by comparing the placement with its prefab alone**: no memory to read, but an
  override that holds the prefab's value would be lost, then follow the prefab, and one set aside
  would be lost at the first save, as Godot lost them.
- **A placement, or an entity of one, written as a root**: it would need variants, or the prefab's
  entities copied into the file; Stage 6 decides how a prefab is made of a selection.
- **Every field written**: no option to add, but files written by a tool as long as the values are
  wide, and different from those written by hand.
- **Assets named after their IDs**: unique at once, but unreadable.

## Consequences

- Stage 6's editor saves a level with `World::write_scene`, and marks overridden values with
  `World::differs_from_prefab`.
- A world written back writes what it holds of a reference or a parent into a placement that spawning
  set aside, and the place of a placement's entity moved out of its place: the known debt in
  `docs/PLAN.md` records them.
- M5b.3 saves the placements made while a world runs, and carries a run through a save on hot reload.
