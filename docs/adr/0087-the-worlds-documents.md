# 0087. The world's documents

- Status: Accepted; amends [0079](0079-the-engines-own-world.md) (what a save differs from, and the
  identities of entities created outside scenes), [0083](0083-transforms-and-the-hierarchy.md) (keys
  and names that hold `/`), [0085](0085-reflection.md) (a transient field has a default) and
  [0086](0086-serialization.md) (documents of several values, and references to entities); amended
  by [0088](0088-saving-and-resuming-a-run.md) (a save begins with what the game says of it),
  [0089](0089-scenes-and-prefabs.md) (what a save holds of the scenes and prefabs spawned while a
  game runs), [0092](0092-a-world-written-back-as-a-scene.md) (a rotation changed is a difference
  whole), [0093](0093-saves-and-hot-reload-with-placements.md) (a save says its layout, holds what
  the world spawned while it ran, and is loaded through a scene library),
  [0101](0101-saves-and-identities-that-never-mislead.md) (the identities of what a game creates in
  code, saves of layout 3, references compared by the identity they name, and what loading warns
  of), [0108](0108-what-the-world-knows-of-its-changes.md) (driven components),
  [0112](0112-levels-saves-and-worlds-for-open-worlds.md) (saves made of differences per scene, and
  what persists) and [0124](0124-driven-components-keys-and-created-entities-in-levels.md) (layout
  4, which says where a scene file written back holds what the run created)
- Date: 2026-10-16

## Context

[ADR-0079](0079-the-engines-own-world.md) decides that a save holds what differs between a world and
the scenes it was made from: values changed, components added and removed, entities removed and
created; that data marked transient is not saved; that loading spawns the scenes from their current
content, then applies the differences, a difference whose target is gone being set aside and
reported; and that files, saves and the references between entities name entities by their
*persistent identities*, the path of local IDs through the scenes that placed them, `zone/door`. It
leaves the rest to the milestone that builds it, M5a.8.

Where the engine stood:

- Only a scene's entities had an identity. `World::spawn` gave an entity none, and a key or a node
  name could hold the `/` with which an identity's parts are joined
  ([ADR-0083](0083-transforms-and-the-hierarchy.md) left it to M5a.8).
- A game prepares its world after it spawns its scenes. *Hunna Wood* turns the extras of the zone's
  nodes into its own components (colliders, markers, the key, the door, the clock) and chooses the
  sun; the `world` example chooses which directional light shows in the sky. None of that is in a
  scene.
- `slime-serialize` wrote one value per document, refused references to entities, and wrote
  transient fields as any other ([ADR-0086](0086-serialization.md)).

## Decision

The owner took the first three decisions on 2026-10-16, and the one on references to no entity.

**A save differs from a start world.** `World::save(start)` writes what differs between the world
and `start`, a world that the game made as at the start of a run: from the same scenes, prepared the
same way. `World::load` applies a save to such a world, made from the scenes as they are now. What a
game makes at every start, such as the components it reads from extras, is never in a save unless it
changed; a scene corrected, or a change in how the game prepares its world, reaches the saves made
before. A save records, for each scene that the world spawned, its key, its name and a fingerprint of
its cooked bytes, FNV-1a over 64 bits: an asset ID stays when a scene changes, and could not say that
it did. Loading names each scene whose fingerprint changed, or that the world no longer spawns.

**A save is laid out by entity and component.** Its sections, each left out when empty:

```toml
next_created = 3

[scenes]
yard = { name = "yard", content = "6c1e0a4f9b3d2e71" }

[removed]
entities = ["yard/tree"]

[removed.components]
"yard/post" = ["Meshes"]

[parents]
"yard/post" = ["yard/well"]

[created."created/1"]
parent = ["yard/well"]

[created."created/1".components]
Name = "torch"

[created."created/1".components.Transform]
translation = { x = 0.5, y = 1.0, z = 0.0 }
rotation = { x = 0.0, y = 0.0, z = 0.0, w = 1.0 }
scale = 1.0
stretch = { x = 1.0, y = 1.0, z = 1.0 }

[added."yard/gate"]
Hidden = {}

[changed."yard/well".Transform]
translation = { x = 6.5 }
```

An entity created since the start is written whole, its parent and its components; a component
added, whole; a component that changed, as a *partial value* of the fields that differ, at any depth
(`Registry::difference`), set back with `Registry::overlay`. An upgrade of M5a.7 applies to a part as
to a whole value, so that a save outlives a change of a component's shape. `slime_reflect::diff` and
`apply` give and set differences path by path instead, the form of an edit of the world panel: they
stay for comparing values and for Stage 6's undo, and a save holds parts, which upgrades read.
Entities are written in the order of their identities and components in the order of their names:
the same worlds give the same bytes.

**Entities created outside scenes are named by a count.** `World::spawn` gives an entity the identity
`created/<number>`, the next number of a count that starts at 1 and that saves keep. A number is never
given twice: loading keeps the larger of the world's count and the save's, and stays above every
number that the save creates. A save that creates a number past `i64::MAX`, or written otherwise
than a world writes it (`created/01`), is refused, and a count past it is read as `i64::MAX`: the
count never runs out, and one number has one identity. A save made up to bring the count near that
end can lead, spawns later, to saves that loading refuses; no run counts that far. Every such
entity is saved, with its descendants, unless it or an ancestor holds the engine's `Unsaved`
component, which a game puts on what it makes again at each run, such as an effect or a projectile:
forgetting the marker costs a larger save, never a lost entity.

**What the start holds is never created again.** A save matches the world's entities with all of
the start's that have an identity, saved or not. An entity that the start leaves out and the world
saves is saved as a change of it, `Unsaved` removed; one that the start saves and the world leaves
out since is neither changed nor removed, and loading keeps it as the start has it, out of its parent
if the save removes that.

**Keys and names.** `created` names no scene, and a key may be neither empty nor hold `/`:
`spawn_scene` refuses them. A node whose name holds `/` takes its index, `#12`, as a node without a
name does. An identity is then written as text, its parts joined by `/`, and parsed back
(`PersistentId: FromStr`).

**References between entities are persistent identities.** `slime-serialize` gains documents of
several values, each at its place: `Composer` writes them and `Document` reads them, one place at a
time. Given `References`, which a world implements with its identities, they write a reference to an
entity as its identity, as text, and read it back as the entity alive under that identity; without,
a reference is refused, as [ADR-0086](0086-serialization.md) decided.

**A reference to no entity is `""`.** The owner decided, when the review found that a reference to
an entity destroyed, or left out of the save, made the whole save fail, that such a reference is
written `""` and read back as a handle that no world gives out: it refers to no entity, as the
handle it stands for did. A target that died, a common state in a game, never fails a save. A
reference to an entity that the world loading the save does not have, one that a scene no longer
holds, reads as no entity too, and is reported with its component.

**Transient fields are left out, and have a default.** A save leaves out the fields marked
`transient`, at any depth. A created entity's components and an added component are read back whole,
and so is what a change holds whole: a variant that replaced another, a list, an option. The
registry refuses a transient field without a default, the field's own, its struct's or its type's,
as an option's `None`, which what is read whole takes; and a transient field of a tuple, which is
written whole, by places.

**Loading parses once, reads in two steps, then applies.** It first spawns the entities that the save
creates, under their numbers, so that the references read next can name them; then it reads every
value, lenient as a save is read, references resolved. An error in either, a save that is not one,
of a later version, or creating an entity under an identity that a world does not give or that the
world has, leaves the world as it was: the entities created are destroyed again and the count
restored. Then the save is applied. Every entity given another parent is taken out of its own first,
then put where the save says, so that a child can take its parent's place; one whose parent the
world does not have is left out of its own. The entities removed, found before any is, are
despawned after, so that one moved out of an entity removed stays, and one removed with its parent
is not reported as gone; a saved entity under one removed, which the save does not remove, is taken
out of it first. Then the components are removed, added and changed. A difference whose entity or
component the world no longer has, or that the component's type refuses, is set aside with its
reason in `Loaded`, and the rest is applied. A scene stays recorded when its root is destroyed,
since its entities moved elsewhere live on.

**Worlds are compared once.** `World::first_difference` says the first difference between two worlds
as saves see them, or none: an entity that one saves and the other does not have, by identity;
and of those that both save, their parents, their components and their values without their
transient fields, floats by their bits, references by the identities they name. The tests, the
`world` example and *Hunna Wood*'s test of a saved run use it.

**The count's index costs next to nothing.** A world finds an entity created outside a scene by its
number through a vector of numbers and entities in the order of the numbers, which spawning keeps,
since a new number is the largest; destroyed entities stay in it until they are more than half, and
a lookup is a binary search. Each slot holds either its scene's identity or its number, never both.
A hash map from number to entity made spawning 100 000 entities about twice as slow and destroying
them about five times. With the vector, measured in a release build on the owner's machine, best of
five runs interleaved with the same test before M5a.8, spawning 100 000 entities takes 18.3 ms
against 16.7 ms, and destroying them 4.6 ms against 2.9 ms. The machine was busy: the same test at
the end of M5a.3 took 15.6 ms there, against the 10.5 ms of
[ADR-0083](0083-transforms-and-the-hierarchy.md).

## Alternatives rejected

- **A difference from the scenes alone, as cooked**: everything a game adds after spawning would be
  in every save, and loading would add it a second time over what the game prepared; saves would grow
  and keep the preparation of the day they were made.
- **A snapshot of each entity when its preparation ends**: no world to make again when saving, but
  twice the memory, an engine that must know when a game's preparation is done, and saves that a game
  update reaches less cleanly.
- **A flat list of changes, one per property path**: simple to write, but longer to read, and an
  upgrade does not apply to a value alone at the end of a path: a save could become unreadable when a
  component's shape changes.
- **Entities created outside scenes unsaved unless spawned for it** (`spawn_saved`): smaller saves,
  but an entity spawned the ordinary way would vanish on loading without a sound, a bug that only
  playing shows.
- **The scene's asset ID in the save**: it stays when the scene changes, so loading could not say
  that it did.
- **Transient fields written for whole values**: no default needed, but the same field would be
  saved for some entities and not for others.
- **A reference to no entity refused**, the save failing: a game would clear every reference to an
  entity destroyed before it saves, or F5 would fail at any moment of a run.
- **A hash map from number to entity**, with the standard hasher or a faster one: the measurement
  above; the faster hasher still added about half to the time of spawning.

## Consequences

- A game saves by making its start world again, which needs no GPU, and loads into one: M5a.9
  builds the game's saves on it.
- Renaming a node in Blender changes its identity: loading sets aside what the save held of it, and
  reports it; a node moved, removed or added is moved, set aside or spawned as the scene now says.
- The previous transform, kept to draw between fixed steps, lives in the world's slots and is never a
  component: saves never see it.
- `slime-world` depends on `slime-serialize`, which depends on no world: references reach it as
  names.
- A save cut short at the boundary of a section is still a document, and loads what it holds: M5a.9
  writes saves so that a file is either the old save or the new one.
- The order of an entity's children, and the order in which queries visit entities, may differ in a
  world loaded from a save, its entities in other slots: what must go the same way after loading
  cannot depend on them.
- A save is text of at most `slime_serialize::MAX_SIZE`, which saving refuses past, and its cost
  grows in proportion with what it holds, loading costing more than saving: `World::save` and
  `World::load` give the figures. A larger world needs the binary encoding of Stage 5b, or saves
  read in parts.
