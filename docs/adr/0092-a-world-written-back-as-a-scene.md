# 0092. A world written back as a scene

- Status: Accepted; amends [0087](0087-the-worlds-documents.md) (a rotation changed is a difference
  whole); amended by [0101](0101-saves-and-identities-that-never-mislead.md) (`Unsaved` in a level
  written back) and [0108](0108-what-the-world-knows-of-its-changes.md) (driven components)
- Date: 2026-10-17

## Context

[ADR-0089](0089-scenes-and-prefabs.md) decides that the world remembers its placements and writes a
world, or a subtree of it, back as a scene file: placements stay placements with what differs from
their prefab, an entity created since gets an ID when it is first written and keeps it, children
keep their order, entities marked `Unsaved` and transient fields are left out, and the same world
gives the same bytes. Stage 6's editor saves a level so. M5b.2 builds it
([ADR-0090](0090-stage-5b-scenes-and-prefabs.md)).

Where the engine stood after [ADR-0091](0091-scene-documents-and-placements.md):

- The root of each placement remembers its prefab, resolved, and its overrides as the file wrote
  them, those set aside included. The world did not keep the scenes that it spawned, only their
  names and fingerprints, and a resolved scene was not shared but through an `Arc` that a library
  held.
- An entity created outside a scene has the identity `created/<number>`, and no ID that a file could
  give it.
- `slime-serialize` writes every field of a value, and has no way to make a table that holds
  nothing, which an entity of a scene file without components or parent needs.
- No code places a prefab while a world runs: [ADR-0089](0089-scenes-and-prefabs.md) gives it to
  M5b.3, with its saves.

## Decision

**`World::write_scene(root)`** writes the subtree of `root` as a scene file, and reports what the
file cannot hold (`Written::left_out`).

**What a file's root may be.** The root of a spawned scene, an entity that a scene's file makes, or
an entity created outside a scene, as an editor starts a level. A placement, and an entity of one,
are refused: the scene that places them writes them, and making a prefab of a selection is Stage
6's.

**What the file holds.**

- The root's components are the file's root's, but for the `Name` that spawning gives it, the last
  ID of its identity, and a `Transform` that is the identity. A root created outside a scene writes
  its `Name`: the file spawned under that name gives it back, and the world spawned so writes the
  file without it, then the same bytes.
- An entity of the root's scene, `zone/door` under `zone`, is a record under its ID, with its parent
  and its components, whole.
- An entity created outside a scene, and one of the scene whose ID a file cannot hold, such as a
  glTF node without a name of its own, `#3`, is a record under an ID that the world gives it when it
  first writes it: its `Name`, its prefab's name for a placement, or `entity`, and four random
  letters or digits, `torch-k3f9`, unique in the file and in the scene, as
  [ADR-0079](0079-the-engines-own-world.md) gives a tool's entities random IDs. The world keeps the
  ID, and gives it again while it is free: once the file is spawned again, the entity that the file
  makes has it, and an entity that kept it takes another. An entity created keeps its identity,
  `created/<number>`, which saves name it by, until the file is spawned again. A placement made
  while the world runs has no `Name` until one is given: the file says that its `Name` is removed.
- A placement is a record that names its prefab. Its overrides are what differs between its
  entities, found by their identities, and its prefab's records: an entity removed, the topmost of a
  subtree; a component removed or added, whole; the part of a value that changed. The overrides that
  the scene that placed it wrote are kept while what they say holds: one whose fields hold the
  prefab's values still is written with them, the world's values in its fields; one set aside, whose
  entity or component the prefab no longer has, or that no longer fits it, is written as the file
  wrote it while the world holds the prefab's value, and gives way to the world's value, reported,
  once that value changed; a component that the file added stays added. One whose entity or
  component the world removed since is not written again: the removal is.
- Entities that hold `Unsaved`, with their descendants, the `Unsaved` component, transient fields,
  and fields that hold their defaults are left out. What the scene wrote of a placement's entity
  that holds `Unsaved` is kept as it wrote it.
- A reference to an entity is its path from the root, `.` the root; one to an entity out of the
  subtree, or that holds `Unsaved`, is `""`, and reported while that entity is alive. Assets are
  named by the short names of the table of the file that the scene was spawned from, else a prefab's
  name, numbered when another asset has it, `lamp 2`, else `asset 1`, `asset 2`...

**One form for one world.** The assets in the order of their names, the root, the records in the
order of a walk of the hierarchy, children in their order, then the overrides in the order of their
paths and of the components' names, through the canonical writer of `slime-serialize`. A placement's
entities keep its prefab's order; the file's entities under one of them come after the prefab's
children. Writing a world again gives the same bytes, and the file spawned again gives a world that
writes the same file.

**What a file cannot hold is reported, and written as near to the world as it can.**

- An entity of the subtree that belongs to another scene is left out, with its descendants.
- An entity of a placement moved out of the place that its prefab gives it is written in that place,
  with the entities of the file under it: a scene file has no parents for a placement's entities.
  One whose place the world removed since is lost.
- The children of a placement's entity come in its prefab's order, then in the file's: another order
  is lost.
- A root without its `Name` or its `Transform` gets them back when the file is spawned.
- An entity whose ID a file cannot hold is written under another, as above.
- A reference, or a parent, that the file of a scene wrote into a placement whose prefab no longer
  had its entity, and that spawning set aside, is written as the world holds it, no entity or the
  root: what it named is lost, as a save loses it ([ADR-0087](0087-the-worlds-documents.md)).

The comments of the file that the scene was read from are lost, as ADR-0089 says, and are not
reported.

**A rotation changed is a difference whole.** `Registry::difference` holds a rotation, `Quat`,
whole, as it holds a tuple: its four coordinates make a rotation only together. A file that held the
two coordinates that an edit changed, overlaid later on a prefab turned since, would give a
quaternion that is not a rotation, which `Transform` refuses, and the scene would not spawn. Saves
write rotations whole too, which amends [ADR-0087](0087-the-worlds-documents.md). A part written
otherwise, by hand or by 0.3, whose saves hold the coordinates that changed, is still read and
overlaid coordinate by coordinate, so that those saves load; `Transform`'s check refuses what is not
a rotation.

**For an inspector.** `World::placement_root` gives the outermost placement that an entity belongs
to, whose prefab a file compares it with; `World::differs_from_prefab` says whether a component of
it differs from the prefab's: what Stage 6 marks as overridden.

**A placement made while the world runs.** `World::spawn_placement(prefab, parent)`, which ADR-0089
gives to M5b.3, comes with M5b.2, so that an edit can add a placement: its root is
`created/<number>`, its entities `created/<number>/<ID>`, and the world remembers its prefab. Saving
a world that holds one waits for M5b.3.

**What the world and `slime-serialize` gain.** A resolved scene is a shared handle, whose clone
costs a count; the world keeps each scene that it spawned, its table of assets among it, as it kept
their names and fingerprints: even once the root is destroyed, since entities moved out of it live
on, until another scene is spawned under its key. `Composer::without_defaults` leaves out the fields
of whole values that hold their defaults, which a reader gives again; `Composer::table` makes a
table that holds nothing yet; `Value::same` compares values with their floats by their bits.

**What it costs.** Writing is linear in the entities of the subtree, the prefabs' entities placed,
not copied; the rustdoc of `World::write_scene` holds the measurements
([ADR-0053](0053-one-home-per-fact.md)).

## Alternatives considered

- **IDs from the numbers of the entities created, `created-5`**: no random letters and no write to
  the world, but a number given again in another session would name another entity, and two branches
  of a level would give the same IDs.
- **Entities created outside a scene given the identity that their ID makes when first written**:
  the world and the file would agree at once, but a save made before would no longer find them.
- **Overrides found by comparing the placement with its prefab alone**: no memory to read, but an
  override that holds the prefab's value would be lost, then follow the prefab, and one set aside
  would be lost at the first save, as Godot lost them.
- **A placement, or an entity of one, written as a root**: it would need variants, or the prefab's
  entities copied into the file; Stage 6 decides how a prefab is made of a selection.
- **Every field written**: no option to add, but files written by a tool as long as the values are
  wide, and different from those written by hand.
- **Assets named after their IDs**: unique at once, but unreadable.
- **The overrides that the scene wrote kept when the world removed their target**: nothing lost, but
  they would be set aside at every spawn after, and reported forever.
- **A part of a rotation refused when read**: a file written by hand that holds half a rotation
  would be refused where it says it, but the saves of 0.3, which M5b.3 reads, hold such parts.
- **A rotation overlaid then made a unit again**: no rotation refused, but the rotation read would
  be neither the file's nor the prefab's.

## Consequences

- Stage 6's editor saves a level with `World::write_scene`, and marks overridden values with
  `World::differs_from_prefab`.
- A world written back writes what it holds of a reference or a parent into a placement that
  spawning set aside, the place of a placement's entity moved out of its place, and the order of its
  children in its prefab's: the known debt in `docs/PLAN.md` records them.
- M5b.3 saves the placements made while a world runs, and carries a run through a save on hot
  reload.
