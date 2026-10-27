# 0089. Scenes and prefabs

- Status: Accepted; amends [0087](0087-the-worlds-documents.md) (what a save holds of the scenes and
  prefabs spawned while a game runs, and of the prefabs that its scenes place); amended by
  [0101](0101-saves-and-identities-that-never-mislead.md) (`Unsaved` in a level written back, and
  the numbers of what a run spawned), [0112](0112-levels-saves-and-worlds-for-open-worlds.md)
  (references to entities of other scenes, former identities written by the editor, and hot reload
  without a start world), [0134](0134-the-editors-core.md) (the editor keeps the comments of a
  level that it writes) and [0155](0155-the-editors-design.md) (the editor opens a level that holds
  what the program does not know; the world updated in place at Stage 8)
- Date: 2026-10-17

## Context

Stage 5b, the second half of Stage 5 ([ADR-0078](0078-stage-5-in-two-halves.md)), writes worlds in
files: scenes, and prefabs placed many times, each placement with values of its own, which follow
the changes of their prefab. Its exit, in the roadmap: the game's scenes round-trip through save and
load as files; prefabs instantiate with overrides. Stage 6's editor then builds, saves and reloads a
level of the game, on the world's handles, components and persistent identities.

Where the engine stands at the end of Stage 5a:

- A world spawns one kind of scene, a cooked glTF scene: `World::spawn_scene(key, &CookedScene)`
  gives each node the persistent identity `key/<node name>`, or `key/#<index>`
  ([ADR-0079](0079-the-engines-own-world.md), [ADR-0083](0083-transforms-and-the-hierarchy.md)).
  Nothing places a scene inside another, nothing records what placed an entity, and nothing writes a
  world back as a scene.
- A game turns the untyped `extras` of the nodes into its own components after it spawns a scene:
  *Hunna Wood* reads its colliders, markers and times of day from them, and finds its campfire's
  light by its distance to a marker. The entity then holds the same fact twice, as `Extras` and as
  the game's component, and nothing in a file can set a value on what exists only after spawning.
- A save holds what differs between a world and a *start world* that the game makes again
  ([ADR-0087](0087-the-worlds-documents.md)): values changed as *partial values*, components added
  and removed, entities removed, entities created outside scenes as `created/<n>`. An entity of a
  scene spawned during a run, which the start world lacks, is written as created under its scene's
  identity, and loading refuses the save: no game can yet spawn a scene while it runs and save.
- On hot reload, *Hunna Wood* despawns its zone, spawns it again, and puts back the key, the door
  and the time of day: anything else that the run changed in the zone is lost.
- `slime-serialize` writes TOML only; ADR-0080 leaves the binary encoding of cooked scene files to
  Stage 5b, beside the text one ([ADR-0086](0086-serialization.md)).

Five design studies of the code preceded the plan, and surveyed other engines:

- Unity keeps a prefab instance's modifications as explicit records (target, property path, value),
  with components and objects added and removed; nested prefabs and variants came in 2018, thirteen
  years after its prefabs. A model's imported objects are known by a hash of their names, so that a
  rename in the modelling tool drops the overrides made on them; overrides whose target is gone are
  kept and listed since 2022. Builds flatten instances into plain objects, and saves never see the
  prefab again.
- Godot named nodes by their paths, and lost the changes made in inherited scenes when a node of the
  base was renamed or moved, for years; Godot 4.6 (January 2026) added a unique ID per node as a
  fallback, which a glTF file imported again changes.
- Unreal keeps the properties that differ from the class's defaults, found by comparison: a value set
  equal to the default follows later changes of the default. Per-instance overrides of nested levels
  came in 5.5, as an experiment.
- Bevy's scenes made of patches arrived in code in 0.19 (June 2026); its scene files had not shipped
  in 0.20 (October 2026). O3DE applies JSON patches to nested prefabs, skips those that fail, and
  flattens prefabs when it processes assets. Fyrox marks each inherited field modified or not, which
  wraps every field of a component.

## Decision

The owner took these decisions on 2026-10-17, one by one, when Stage 5b was planned.

**One concept: the scene.** A scene is a group of entities with one root, written in a *scene file*
or exported from Blender as a glTF scene. A world spawns it under a key, as a level, or a scene
places it inside itself, as a *prefab*: a prefab is a role that a scene plays, not a kind of file. A
prefab placed in a scene is a *placement*, the roadmap's word, since "instance" names what the
renderer draws. Placements nest: a prefab may place prefabs. A cycle is refused, with the chain of
scenes that makes it, and nesting is limited in depth.

**Overrides are explicit, and kept.** The file that places a prefab writes what its placement
changes, in the grammar of saves: values as partial values, components added and removed, entities
of the prefab removed with their subtrees, entities added. An *override* stays when the prefab later
takes the same value: what its author set does not follow the prefab any more. What no override
changes follows the prefab. Overrides apply from the innermost scene out, so that the outermost
wins. An override whose target the prefab no longer has is kept, not applied, and reported, as a
save's difference is. Lists are overridden whole, as partial values hold them, until reflection has
maps.

**Assembled at load.** A cooked scene keeps its placements and their overrides; the world resolves
them, with its registry, when it spawns the scene. The cooker needs no type of a game, upgrades run
where the types are, and a prefab that changes is cooked again alone. Flattening placements when
cooking stays an optimization for the streamed worlds of Stage 10, for a measured need.

**Identities.** An entity's ID in a scene file is separate from its displayed `Name`, so that
renaming it changes nothing that names it. Its persistent identity is the path of IDs through the
placements: `zone/lantern 3/light` is the light of the placement `lantern 3` of the scene spawned
under `zone`. An entity that a file adds under a placement is that file's. References between
entities in a file are paths relative to that file, made absolute when it is placed; a file names no
entity outside itself, and a game links scenes together in code. The nodes of a glTF scene keep
their names as IDs, as today: the scripts that export them give every node a name of its own, and
fail when Blender renames one by adding a number, `.001`. An ID kept apart from the name in Blender
(`slime:id`) comes when artists edit the Blender files by hand, and changes no file that does not
use it.

**The world remembers its placements, and writes them back.** For each placement, the world keeps
its prefab and its overrides, those set aside included. A world, or a subtree of it, is written back
as a scene file: placements stay placements with what differs from their prefab, an entity created
since gets an ID when it is first written and keeps it, children keep their order, entities marked
`Unsaved` and transient fields are left out, and the same world gives the same bytes. The comments of
a file written by hand are lost when the world writes it again; Stage 6 decides whether its editor
keeps them.

**The data that Blender holds is typed.** A glTF node carries a game's component as a custom property
named `slime:<registered type name>`, whose value is the component's value in the engine's notation:
`slime:hunnawood::Collider` = `{ kind = "Cylinder", height = 2.6 }`. What Blender shapes or places,
or what an export script makes with the geometry, is written there: colliders, markers, times of
day. Scene files hold what assembles the scenes, the values of each placement, and what Blender
cannot say. The cooker checks how the values are written; the world reads them through its registry
when it spawns the scene. Other custom properties stay untyped, in `Extras`. A registry that a game
exports, so that the cooker checks the values themselves, comes if a need appears: an add-on of
Blender, or an editor that runs apart from the game.

**A scene file that says something wrong is refused.** An unknown component, an unknown field, a
value out of its range or a file written wrong refuses the whole scene, and the world is left as it
was; the error names the file, the entity, the component and the field. Sources are read strictly,
as [ADR-0080](0080-reflection-and-serialization.md) decided. An override whose target is gone is not
an error: it is set aside. A game's tests spawn each of its scenes, so that its errors show in CI;
on hot reload, a game keeps the scene it has and logs the error, as it does for a shader. Stage 6
decides whether its editor opens a damaged scene leniently, to repair it, as saves are read.

**What a game spawns while it runs is saved as a placement.** A prefab spawned from code gets
`created/<n>` as its root's identity, from the count that saves keep, and its entities
`created/<n>/<ID>`. A save records it as a placement of its prefab, with what differs from it; loading
spawns the prefab as it is now under the same number, then applies the differences, so that a fix of
the prefab reaches the saves made before. A prefab that the game no longer has is set aside and
reported, with what was under it. A scene spawned under a key during a run is saved the same way.
A save records, for each scene spawned and each placement spawned while running, a fingerprint of
what it resolves to, prefabs included, so that loading names those that changed. The save's format
takes a new version, and the saves of 0.3 still load.

**Hot reload carries a run through a save.** When a scene or a prefab changes on disk, a game saves
its run against its old start world, makes a new start world from the new files, and loads the save
into it: after a reload, the world is what saving and loading would give. A prefab that changes
changes in every placement, but where an override or the run changed it; entities added to it
appear; what the run changed stays. The scenes that place a prefab are reloaded when it changes.
Handles to entities change, as when a save is loaded. Updating the world in place, which keeps the
handles, comes with Stage 6's editor, which needs it to change a prefab while a game runs, with
undo, and with its scripts; it must give the world that a save and a load give.

**Scene files.** A scene file is TOML, with the extension `.scene`, which generic TOML tools leave
alone, and a metadata file like every source. Its entities are records keyed by their IDs, each
naming its parent; values equal to their defaults are left out. A table at its top gives each asset
that it uses a short name, its asset ID, which decides, and its path, which a moved file makes stale
without breaking anything; the rest of the file names assets by their short names, so that a scene's
dependencies are exactly that table. The layout of a scene file has a version of its own.

**A binary encoding of documents.** A cooked scene file is the engine's own binary encoding of the
document that its text holds, without its types: self-describing, versioned, the same bytes on every
platform, read within limits without a panic, and read through the same reader as text, strict and
lenient alike. It lives in `slime-serialize`, beside the text. Saves stay text, until a measurement
asks. The cooked glTF scene keeps its format, which learns to carry typed components; moving it onto
the binary encoding waits for a reason, since the renderer's scene loader, the `scene` example and
the image tests read it.

**Where the code lives.** Scenes, placements and their resolution live in `slime-world`, beside the
saves whose differences, identities and references they share: no new crate. The cooker depends on
`slime-world`, which needs no GPU, to check the scene files that it cooks. No new dependency.

## Alternatives considered

- **A kind of file for prefabs apart from scenes**, as Unity's: an intent easier to read, but two
  formats, two ways of spawning, and a level placed inside a level as a third feature, which Unreal
  added late.
- **Overrides found by comparing a placement with its prefab**, as Unreal and Flax do: nothing to
  record, but a value set equal to the prefab's is lost, then follows the prefab against its author.
  **A flag on every field**, as Fyrox's: every field wrapped, and a change through reflection cannot
  set it. **A flat list of paths and values**, as Unity's records or O3DE's JSON patches: rejected for
  saves by ADR-0087, since an upgrade cannot apply to a value at the end of a path; a list's element
  addressed by its index moves when the list does.
- **Placements flattened when cooking**: the fastest loading, but the cooker would need every game's
  types and could not run their upgrades, a prefab that changes would cook again every scene that
  places it, and a prefab spawned while a game runs would need a second way.
- **Writing worlds back left to Stage 6**: one milestone fewer, but the model unproven on the case
  that the editor needs, after scene files exist. **Placements forgotten once spawned**, as in
  Unity's builds: the editor could not save a level with its prefabs, and the overrides set aside
  would be lost at the first save.
- **Untyped extras that each game reads in code**: nothing to build, but every game writes its own
  reading, facts held twice, and no override can reach them. **All the game's data in scene files**:
  one place, but the colliders that the export script makes with the geometry would be listed by a
  file that the script generates and that the editor will own.
- **Variants**, a scene that extends another so that the other's entities are its own (`zone/door`
  rather than `zone/land/door`): a zone could extend only one exported file, and no scene of Stage
  5b needs one. A scene that places another as its child does what a variant does, with one more part
  in the identities; variants can be added when a game or the editor needs them, without changing a
  file.
- **Former identities**, a scene file saying what an entity was called, so that saves and overrides
  made before a rename find it: they close a known debt, but no game has players whose saves must
  outlive a change of its scenes; they come when one does, at the latest with the packaging and
  migration of Stages 9b and 11, without changing a file.
- **Values that the types refuse set aside**, as overrides whose target is gone: a game would never
  refuse a scene for one value, but an error could go unseen in a log, against ADR-0080. **Strict
  while developing, lenient in a released game**: two behaviours, and faults that only players see.
- **What a game spawns while it runs saved whole**, as entities created outside scenes are: simpler,
  but a fix of the prefab never reaches the saves made before, and every entity of every placement
  is in every save. **No prefab spawned while a game runs**: the known fault stays, and prefabs serve
  only to build levels.
- **Hot reload that spawns the scene again**, as today: each game puts back what it remembers, and
  the rest of the run is lost. **Updating the world in place now**: handles kept, but about one
  milestone more, for code that no game needs yet, whose details depend on the editor and the scripts.
- **The extension `.scene.toml`**: highlighting in every editor, but generic TOML tools reformat and
  reorder it. **Asset IDs written as UUIDs everywhere**: unreadable files, and the cooker could not
  tell assets from other text without the types. **Paths alone**: a moved file breaks the scenes that
  name it, against [ADR-0069](0069-the-asset-pipeline.md).
- **CBOR or MessagePack from a crate**: a dependency for a few hundred lines; the crates that the
  studies found are abandoned (`bincode`), positional (`postcard`), under a license that the project
  does not allow (`minicbor`), or tied to the layout of types (`rkyv`). **A binary encoding of typed
  values**: smaller, but the cooker would need the game's types. **Cooked scene files kept as text**:
  a level of 100 000 entities would pass the size that `slime-serialize` reads.
- **A crate `slime-scene`**: scenes would need the world's identities, spawning and saves as public
  API of their own.
- **The cooked glTF scene moved onto the binary encoding now**: one kind of cooked scene, but the
  renderer's loader and its tests would change, and `slime-render` would depend on serialization.

## Consequences

- A world spawns scene files and glTF scenes the same way; the identities of a scene's entities gain
  the parts of their placements.
- *Hunna Wood*'s identities change when its zone places prefabs, and its untyped extras become typed
  components: its save fixture is made again, as
  [ADR-0088](0088-saving-and-resuming-a-run.md) says for a change of layout.
- Spawning resolves values through the registry: its cost is measured on the zone, on a thousand
  placements and on a level of 100 000 entities. If the largest is too slow, the remedies stay inside
  the world: copying typed components rather than values, identities that share their parts, worlds
  prepared on another thread.
- A save's format changes; loading reads the saves of 0.3, and later versions read those of 0.4.
- Stage 6 builds on what the world remembers of its placements: which values are overridden,
  reverting one, applying one to its prefab, making a prefab of a selection, saving a level. Stage 8's
  components with no Rust type are overridden as any other. Stage 10 sends a placement spawned on a
  server as the save writes it, a prefab and its differences.
- Each milestone that builds a part records its design in an ADR of its own.
