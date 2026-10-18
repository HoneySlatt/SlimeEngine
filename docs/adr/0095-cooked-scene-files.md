# 0095. Cooked scene files

- Status: Accepted
- Date: 2026-10-18

## Context

[ADR-0089](0089-scenes-and-prefabs.md) decides how scenes reach a game:

- A cooked scene file is the engine's own binary encoding of the document that its text holds,
  without its types ([ADR-0094](0094-the-binary-encoding-of-documents.md)).
- The cooker checks how a scene file is written; a world reads its values through its registry
  when it spawns the scene.
- The data that Blender holds is typed: a glTF node carries a game's component as a custom property
  named `slime:<registered type name>`, whose value is the component's in the engine's notation.
  Other custom properties stay untyped.
- The cooked glTF scene keeps its format, which learns to carry typed components.
- The scenes that place a prefab are reloaded when it changes, and a game carries its run through a
  save then ([ADR-0093](0093-saves-and-hot-reload-with-placements.md)).
- The cooker depends on `slime-world`, which needs no GPU, to check the scene files that it cooks.

[ADR-0090](0090-stage-5b-scenes-and-prefabs.md) gives M5b.5 all of it. It adds the `world` example,
a yard that places the sample scene and four lamps, and a world panel that shows each entity's
persistent identity and, for a placement, its prefab.

Where the engine stood:

- The cooker cooked images and glTF files. A glTF node's custom properties were untyped `Extras`,
  which a game read in code.
- A `SceneLibrary` held the scenes that a game gave it, one by one; nothing read scene files from
  cooked data.
- The `world` example spawned the sample's glTF scene directly, and spawned it again when its
  cooked data changed.

## Decision

**A scene file is a source.** A `.scene` file, with its metadata file, is cooked under its own ID:

- **Its layout is read without types.** `slime_world::outline` reads the file's version, its table
  of assets and its entities, and refuses what resolving it would refuse of those: a text that is
  not TOML or not a scene file's, a key or an ID that a scene file cannot have, a prefab that the
  table does not name, a parent that names no entity of the file, entities that would be each
  other's ancestors. The versions of the game's types, which the cooker does not know, are left
  alone.
- **The cooker refuses** an asset of the table that the sources do not make, an asset placed as a
  prefab that is not a scene, a glTF scene's or a scene file's, and scene files that place each
  other. It names the file and the line.
- **A stale path is a warning.** The path that the table gives an asset, relative to the scene
  file's directory, is only a hint: when it is not where the asset's source is, the cooker warns.
- **What is written.** The file's document in the binary encoding, named by the hash of the cooker's
  version, the file's ID and the encoding, under the manifest's kind `SceneFile`. Its needs are the
  assets of its table.
- **What waits for the game.** The values of components and overrides, and parents inside
  placements, need the game's types and the prefabs: they are checked when a world resolves the
  scene, and a game's tests resolve each of its scenes.

**Typed properties on glTF nodes.** A node's `slime:<type name>` custom properties are its typed
components:

- **How they are written.** Each value is text in the engine's notation, `{ height = 2.6 }`, or a
  boolean or a number.
- **What the cooker does.** It checks each as a value written in that notation, and keeps them in
  the cooked scene's node as the binary encoding of a document keyed by their types' names. A
  property without a type, or not written as a value, refuses the import, naming the node and the
  property.
- **What the world does.** It reads them through its registry when it resolves the scene:
  `ResolvedScene::from_cooked` takes a registry. It reads them strictly, and refuses the scene for a
  type that it does not hold, a value that the type refuses, or a component that the node gives
  already, naming the scene, the node, the type and the field.
- **The cooked scene's format takes a new version**, and so does the cooker's import of glTF files:
  cooked data is cooked again.

**A library of cooked data.** `SceneLibrary::of(assets)` reads the glTF scenes and the scene files
of a game's cooked data, each when it is first resolved. `SceneLibrary::reload(assets)` takes new
cooked data:

- it gives the IDs of the scenes whose cooked files changed, or that the new data no longer has;
- it forgets what it read of them;
- it resolves every scene again when next asked, so that a scene that places a prefab follows it.

A game then carries its run into the world spawned anew from the scenes that changed
(`World::carry`).

**The `world` example spawns a yard.** `examples/assets/yard.scene` places the sample scene and four
lamps of `lamp.scene`, a prefab made of the sample's meshes. The second lamp is red and the fourth
has no light.

- **When it quits**, the example writes the yard back, spawns the file, and fails if the world
  spawned is not the one written.
- **With `--watch`**, it carries its run when a scene changes, with a window or without.

**The world panel** shows the persistent identity of the entity selected and, for a placement's
root, the prefab that it places.

## Alternatives considered

- **The cooker resolving scene files with their prefabs**, to check parents inside placements and
  the values of overrides: it would need the game's types, which ADR-0089 leaves to a registry that
  a game exports, when a need appears.
- **Typed components kept as text in the cooked scene**: no encoding to read, but a text parsed at
  every resolution, where the binary encoding is read without parsing.
- **A component that a node gives already overridden by its typed property**: a way to set what the
  importer reads, a light's intensity, but two values for one component in one node, and an import
  that would silently lose one.
- **The scenes given to the library by the game**, as before: no library that knows cooked data, but
  every game would read its manifest, the kinds of its scenes and what changed in them itself.
- **Every scene of the cooked data read when the library is made**: no reading later, but the memory
  and the time of scenes that the game never spawns.
- **Only the scenes that place a changed one resolved again**, through the needs of the manifest:
  fewer resolutions, but a graph to keep, for what costs less than spawning.
- **The yard spawned anew when a scene changes**, as the example spawned the sample: no carrying,
  but the run's changes lost, against ADR-0089.

## Consequences

- Cooked data of 0.3 is cooked again: the cooked scene's format changed.
- *Hunna Wood*'s zone gives its colliders, markers and times of day as typed properties, and is
  placed by a scene file, in M5b.6.
- The cooked glTF scene stays a format apart from the binary encoding, as ADR-0089 decides: the
  known debt in `docs/PLAN.md` records it.
- Stage 6's editor writes the scene files that the cooker cooks, and shows what the panel shows now,
  with the overrides of each placement.
