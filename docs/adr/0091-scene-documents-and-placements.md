# 0091. Scene documents and placements

- Status: Accepted; amends [0086](0086-serialization.md) (how documents name assets, and the errors
  of a document's layout); amended by [0102](0102-a-damaged-file-gives-an-error.md) (the bound of a
  resolved scene), [0112](0112-levels-saves-and-worlds-for-open-worlds.md) (references to
  entities of other scenes, written as absolute identities) and
  [0155](0155-the-editors-design.md) (a level names its parts in a table of their own)
- Date: 2026-10-17

## Context

[ADR-0089](0089-scenes-and-prefabs.md) decides what scenes and prefabs are: one concept, the scene,
spawned under a key or placed in another scene as a prefab; overrides written by the file that
places a prefab, in the grammar of saves, kept when the prefab later takes the same value, set aside
when their target is gone; placements nested, resolved when a scene is spawned; identities as paths
through the placements; a world that remembers its placements; scene files read strictly. M5b.1
builds it ([ADR-0090](0090-stage-5b-scenes-and-prefabs.md)): scene files read from their text,
their placements and overrides, and the cost of spawning them.

Where the engine stood:

- `World::spawn_scene(key, &CookedScene)` spawned one glTF scene, typed component by typed
  component; nothing placed a scene in another.
- `slime-serialize` read documents of several values (`Document`), entities named through
  `References`, assets always written as their IDs, in UUIDs
  ([ADR-0086](0086-serialization.md)). A program that read a document's layout itself could neither
  list the keys of its root nor place an error of that layout at its line.
- A save's differences are applied to a spawned world, one component at a time, through
  reflection ([ADR-0087](0087-the-worlds-documents.md)).

## Decision

**The layout of a scene file.** A scene file is TOML, read strictly through `slime-serialize`:

```toml
[assets]
lamp = { id = "6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c", path = "lamp.scene" }

[entities.yard.components]
Name = "yard"
Transform = { translation = { x = 0.0, y = 0.0, z = -12.0 } }

[entities.porch]
prefab = "lamp"
parent = "yard"

[entities.gate]
prefab = "lamp"

[entities.sign]
parent = "gate/post"

[entities.sign.components]
"demo::Aim" = { at = "gate/light" }

[changed.gate.Transform]
translation = { x = 4.0 }

[changed."gate/light".Light.Point]
intensity = 10.0

[added."porch/light"]
Hidden = {}

[removed]
entities = ["porch/glass"]
```

- Its root holds `layout`, the version of this layout, left out at 1 and refused past what the
  program reads; `assets`; `root`; `entities`; and the sections of overrides. Any other key is
  refused.
- `assets` gives each asset that the file uses a short name: its `id`, which decides, and its
  `path`, which says where its source was. An ID named twice is refused. Values name assets by these
  names only, `prefab = "lamp"`, so that the table is exactly what the scene needs.
- `root.components` holds the components of the scene's root. The root holds `Transform::IDENTITY`
  unless the file gives it another, and, once spawned, a `Name` of its key or of its placement's ID
  unless the scene gives it one.
- `entities` is one table, keyed by the entities' IDs in the file, the order of its keys the order
  of siblings, a placement's entities in its prefab's order at its place: an ID is not empty, holds
  no `/`, does not start with `#`, as the IDs of glTF nodes without names do, and is not `.`. A
  record names its `parent` by its path from the scene's root, the root when it names none, and
  holds its `components` by their types' names or aliases, whole. A record with a `prefab` is a
  placement, which holds no components: what it changes of its prefab is written in the sections of
  overrides.
- The overrides are the sections of saves: `changed`, parts of components; `added`, whole
  components; `removed.entities`, entities removed with their descendants; `removed.components`.
  Their keys are paths that begin with a placement of the file: `gate` for its root, `gate/light`
  for an entity of its prefab, `north/square/light` through a nested placement. An entity that the
  file makes holds its values in its record, and a placement is removed by removing its record: both
  are refused as overrides. A component given twice for one entity, by its name and an alias, is
  refused. An entity added under a placement is a record of the file whose parent is in it; there is
  no section of parents, so that a placement's entities keep their places.
- A reference to an entity is its path from the file's root: `gate/light`, `.` for the root, `""`
  for no entity. A path that leads into a placement whose prefab has no entity there refers to no
  entity, and is reported, as a save reads a reference to an entity that a scene no longer holds: a
  prefab changed since, or a typo, which an override's path cannot be told from either. A path that
  leads into nothing of the file is refused.

**A library resolves, a world spawns.** `SceneLibrary` holds a game's scenes by asset ID: glTF
scenes as the cooker makes them, and the text of scene files. `SceneLibrary::resolve` gives a
`ResolvedScene`, kept and shared until a scene is inserted again; `ResolvedScene::from_cooked`
resolves a glTF scene alone, without a library. A resolved scene is a flat list of records, each
with its path from the scene's root, its parent, its components as values checked against their
types, and what the world will remember of a placement. `World::spawn_scene(key, &ResolvedScene)`
replaces `World::spawn_scene(key, &CookedScene)`: one way to spawn a scene.

Resolving a scene file:

1. The text is parsed strictly, and its layout read.
2. The prefabs that it places are resolved, through the library: a cycle is refused with the scenes
   that make it, and placements nest at most `MAX_NESTING` scenes deep, counted for a prefab
   resolved before too, a glTF scene included. A scene of more than `MAX_ENTITIES` entities, its
   placements' counted, is refused before any record is made, so that a few small files whose
   placements nest cannot fill the memory.
3. Each placement's records are its prefab's, under its ID, their references to entities following
   them.
4. Parents are given, and an entity that would be its own ancestor is refused. An entity of the
   file whose parent is in a placement whose prefab no longer has it is the root's child, and is
   reported, as an entity of a save whose parent the world does not have is left out of its own.
5. The components of the records that the file makes are read, references resolved.
6. The overrides are read, then applied to the values, in the order in which a save's are applied:
   entities removed, all found before any is removed, components removed, added, changed. An
   override whose entity or component the prefab no longer has, or that no longer fits it, such as
   a part of a variant that the prefab no longer holds, is set aside and reported, and kept. An
   entity of the file, or a placement, under an entity removed is the root's child, and is
   reported, as a save keeps an entity that it does not remove.
7. The records removed are left out, and the references to them refer to no entity.

Spawning is all or nothing: identities in use, and types that the world stores no component of,
are refused before anything is spawned; a value that breaks a rule of its type's own check, which
reflection leaves to the type, is found as its component is made, and what was spawned is
despawned; the error names the scene spawned, the entity by the identity it would have, whose path
names the placements it is in, and the component. Overrides are applied to values before anything
is spawned, so that each component is made once.

**What the world remembers.** The root of each placement keeps what the scene that placed it
remembers: its prefab, resolved; its overrides as the file writes them, those set aside included;
and those set aside, with why (`World::placement`). It goes with its entity.

**Fingerprints.** A scene file's fingerprint is the FNV-1a hash of its text and of its prefabs'
fingerprints, in their order; a glTF scene's is its cooked bytes', as before, so that saves made
with 0.3 see their scenes unchanged.

**What `slime-serialize` gains.** `References` names assets too, `asset_name` and `asset`, by their
IDs in UUIDs unless a document names them otherwise, as a scene file does through its table; this
amends the convention of [ADR-0086](0086-serialization.md), under which a document always wrote an
asset as its ID. `Document::keys(&[])` lists the keys of the root; `Document::is_table` says where
tables are; `Document::error_at` places an error that a program finds in a document's layout at its
line and column, as a reading places its own.

**A transform left out is the identity's.** `Transform` declares its default,
`Transform::IDENTITY`, so that a file gives a translation alone.

**What it costs.** Measured in a release build on the owner's machine: the zone of *Hunna Wood*,
543 nodes, resolves in 0.46 ms and spawns in 0.61 ms; a thousand placements of a prefab of 10
entities, 11 001 entities, resolve in 114 ms and spawn in 37 ms; a thousand placements of a prefab
of 100 entities, 101 001 entities, resolve in 0.60 s and spawn in 0.40 s. Spawning makes each
component from its value, where spawning a glTF scene made typed components; a game that spawns a
level of 100 000 entities waits about a second. [ADR-0089](0089-scenes-and-prefabs.md) orders the
remedies, inside the world, for when a game needs them.

## Alternatives considered

- **`World::spawn_scene(key, &CookedScene)` kept beside the new one**: no change to the games and
  examples that spawn a glTF scene alone, but two ways of spawning a scene.
- **Placements in a table of their own**: they would read apart, but the order of siblings, which
  the file's keys give, could not mix placements and entities.
- **Overrides applied to the spawned world, as a save is loaded**: one mechanism, but each override
  would turn its whole component into a value and back; overlaid on the values before spawning, a
  component is made once.
- **Overrides nested in their placement's record**: they would read beside it, but in a second
  grammar beside the saves'.
- **A scene resolved at each spawn, by the world**: no library to keep, but a prefab placed a
  thousand times would be read a thousand times.
- **Every rule of a type checked when resolving**: errors found earlier, but a type's own check
  needs its Rust type, which only a world's storages have.
- **A reference, or a parent, into a placement whose prefab no longer has it refused**, as a
  file that says something wrong is: a typo found at once, but a node renamed in Blender would make
  every file that refers to it, through a placement, refuse its scene.
- **The entities of the file under an entity that it removes removed with it**: nothing to report,
  but a placement and its overrides would vanish without a word, and a world written back would
  lose them.
- **A library that loads scenes from cooked data by itself**: scene files are not cooked before
  M5b.5; a game gives the library its scenes until then.

## Consequences

- Each game and example that spawns a glTF scene resolves it first, `ResolvedScene::from_cooked`.
- M5b.2 writes a world back from what it remembers of its placements; M5b.3 saves the placements
  spawned while a game runs; M5b.5 cooks scene files and lets the library read them from cooked
  data.
- Spawning a large scene costs more than spawning a glTF scene did, as the figures above say: the
  known debt in `docs/PLAN.md` records it until a game needs the remedies.
