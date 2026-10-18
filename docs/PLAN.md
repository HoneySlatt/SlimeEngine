# Plan

This file says what is being worked on now. It lists the milestones of the current stage, their
status, how each one is checked, and the known debt. Direction beyond this stage is in
[`ROADMAP.md`](ROADMAP.md); decisions are in [`adr/`](adr/README.md). A *stage* is a step of the
roadmap; a *milestone* is a small, verifiable step of a stage.

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread. From Stage 2 on, every milestone is also checked on Windows
([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage 5a — The world: done and released as 0.3 on 2026-10-17, with Stage 4b
  ([ADR-0078](adr/0078-stage-5-in-two-halves.md) to [ADR-0088](adr/0088-saving-and-resuming-a-run.md)).
  *Hunna Wood* runs on the engine's world, whose components are listed and edited by name, and
  saves and resumes its runs, on Linux and on Windows.
- Stage 5b — Scenes and prefabs: planned on 2026-10-17 and validated by the owner
  ([ADR-0089](adr/0089-scenes-and-prefabs.md), [ADR-0090](adr/0090-stage-5b-scenes-and-prefabs.md)),
  to be released as 0.4 at its close. M5b.1 to M5b.6 are done; M5b.7 is next.

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M5b.1](#m5b1--scenes-placed-in-a-world) | A scene written as text places entities and other scenes in a world, each placement with values of its own | Done |
| [M5b.2](#m5b2--a-world-written-back-as-a-scene) | A world, or a subtree of it, is written as a scene file that keeps its placements, and reads back to the same world | Done |
| [M5b.3](#m5b3--saves-and-hot-reload-with-placements) | What a game spawns while it runs is saved as placements, and a run is carried through a save when its scenes change | Done |
| [M5b.4](#m5b4--the-binary-encoding-of-documents) | A document is written in a binary encoding of the engine's own, and read back without parsing text | Done |
| [M5b.5](#m5b5--scene-files-cooked-typed-components-prefabs-in-the-world-example) | Scene files are cooked and spawned, Blender's nodes carry typed components, and the `world` example places prefabs | Done |
| [M5b.6](#m5b6--hunna-woods-zone-made-of-prefabs) | *Hunna Wood*'s zone is a scene file that places its land and its lanterns | Done |
| [M5b.7](#m5b7--windows-check-documentation-release-04) | Stage 5b is done and released as 0.4 | Planned |

Stages 0, 1, 2, 3a, 3b, 3c, 3d, 4a, 4b and 5a are done; see [Completed stages](#completed-stages).

## Stage 5b — Scenes and prefabs

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-5b--scenes-and-prefabs--04)): the game's scenes
round-trip through save and load as files; prefabs instantiate with overrides; the sample game's
zone is made of prefabs. [ADR-0090](adr/0090-stage-5b-scenes-and-prefabs.md) makes it precise: the
text of a scene file read and written again gives the same bytes, and its binary encoding reads to
the same values; a world written back as a scene file, then spawned again, is the same world, its
placements kept; the saves of a run load on a world made of prefabs; prefabs follow the changes of
their prefab, nested, where an override or the run changed nothing; *Hunna Wood*'s zone is made of
prefabs, and plays as before.

This is the second half of Stage 5 ([ADR-0078](adr/0078-stage-5-in-two-halves.md)): worlds written
in files. Where the engine stands at the start:

- **One kind of scene.** A world spawns cooked glTF scenes, each node known by its name under the
  key it was spawned with ([ADR-0083](adr/0083-transforms-and-the-hierarchy.md)). Nothing places a
  scene inside another, records what placed an entity, or writes a world back as a scene.
- **The game's data read in code.** *Hunna Wood* turns the untyped extras of its zone's nodes into
  its colliders, markers and times of day after it spawns the zone, and finds the campfire's light by
  its distance to a marker.
- **Saves of what the start world spawns.** A save holds what differs from a start world that the
  game makes again ([ADR-0087](adr/0087-the-worlds-documents.md)); a scene spawned during a run
  made the next save one that loading refused, until M5b.3.
- **Hot reload spawns the zone again**, and *Hunna Wood* puts back the key, the door and the time
  of day; the rest that the run changed in the zone is lost.
- **No binary encoding.** `slime-serialize` writes TOML only
  ([ADR-0086](adr/0086-serialization.md)).

Out of scope: variants and former identities; updating a world in place on hot reload, and the
editor's panels, undo and saving (Stage 6); systems declared in scenes (Stage 7); scripts and the C
ABI (Stage 8); material values per placement (Stage 9b); flattening placements when cooking, and
streaming (Stage 10); migration tooling (Stage 11); binary saves; the cooked glTF scene moved onto
the binary encoding. [ADR-0090](adr/0090-stage-5b-scenes-and-prefabs.md) lists the rest.

Decisions taken when the stage was planned, by the owner on 2026-10-17, with the alternatives they
rejected, are in [ADR-0089](adr/0089-scenes-and-prefabs.md) (scenes and prefabs) and ADR-0090 (the
rest):

- **One concept, the scene**, written in a scene file or exported from Blender as a glTF scene,
  spawned under a key or placed in another scene as a *prefab*; a prefab placed is a *placement*.
  Placements nest; a cycle is refused.
- **Overrides** are explicit, written by the file that places a prefab in the grammar of saves, and
  kept when the prefab later takes the same value; what no override changes follows the prefab; an
  override whose target is gone is kept, set aside and reported. Lists are overridden whole.
- **Assembled at load**, by the world with its registry: the cooker needs no type of a game.
- **Identities**: an ID in a scene file apart from the entity's name; persistent identities that
  are paths through the placements, `zone/lantern 3/light`; references relative to their file. glTF
  nodes keep their names as IDs, and export scripts fail when Blender renames one.
- **Written back**: the world remembers its placements, and writes a world or a subtree back as a
  scene file that keeps them, the same world giving the same bytes.
- **Typed data from Blender**: a node's `slime:<registered type name>` property holds a component in
  the engine's notation, read through the registry when the scene spawns.
- **Strict scene files**: a file that says something wrong refuses its scene, naming the file, the
  entity, the component and the field.
- **What a game spawns while it runs** is saved as a placement of its prefab, with what differs from
  it, so that a fix of the prefab reaches older saves.
- **Hot reload** carries a run through a save, from its old start world into a new one.
- **Files and encoding**: TOML, `.scene`, a metadata file, records keyed by their IDs, assets named
  through a table at the file's top; cooked into the engine's own binary encoding of documents, in
  `slime-serialize`; saves stay text. The code lives in `slime-world`; the cooker depends on it; no
  new dependency.
- **The rest**: the risk first, seven milestones; the zone's lantern made a prefab, placed six times
  by a scene file written by hand, the third lighting less and the fifth broken, the rest of the zone
  one glTF scene placed as `land`, its vegetation as the script scatters it; the save fixture made
  again, the fixture of 0.3 kept with a test that it is set aside; the `world` example places prefabs
  written by hand; the world panel shows identities and the prefabs of placements; Stage 5b
  released as 0.4, the editor as 0.5; one release of the asset mirror, which the owner agreed to.

Risks:

- **The model.** Overrides, nesting and writing back are where other engines took years: M5b.1 and
  M5b.2 come first, and M5b.2 tests what an editor does.
- **The cost of spawning.** Values pass through the registry: M5b.1 measures the zone, a thousand
  placements and 100 000 entities, and ADR-0089 orders the remedies, all inside the world.
- **Saves across the change.** The save's format takes a new version: M5b.3 reads the saves of 0.3,
  and M5b.6 sets aside the fixture of 0.3 as a save of another layout.
- **Errors found at load.** The cooker cannot check a game's values: a game's tests spawn each of its
  scenes, and errors name the file, the entity, the component and the field.
- **Corrupt cooked files.** M5b.4 changes and cuts its encoding at every byte.
- **Identities from Blender's names.** M5b.6's export names every node and fails when Blender
  renames one; the zone's identities change once, in one commit.
- **Scope.** The editor's features are Stage 6's: each milestone delivers an API and its tests, and
  its review checks the list above.
- **The cost of the checks.** The Windows CI job took 12 to 19 minutes during Stage 5a, of the 20
  that the owner allows; each milestone that adds a crate or a dependency measures it (ADR-0081).

### M5b.1 — Scenes placed in a world

- **Goal.** A scene written as text places entities and other scenes in a world, each placement with
  values of its own.
- **Crates.** `slime-world`; `slime-serialize` and `slime-reflect` for what they lack.
- **Delivers.** Scene documents read from their text: entities under IDs of the file, apart from
  their names, each naming its parent; components by their registered names; assets through the
  table at the file's top; placements of scene documents and of cooked glTF scenes under an ID, with
  their overrides: values as partial values, components added and removed, entities removed with
  their subtrees, entities added; nesting, a cycle refused with its chain, a limit of depth;
  references relative to the file; spawning under a key, strict, all or nothing, its errors naming
  the file, the entity, the component and the field; overrides applied from the innermost scene
  out, an override whose target is gone kept, set aside and reported; what the world remembers of
  each placement; the cost of spawning the zone, a thousand placements and 100 000 entities,
  measured in a release build by ignored tests; the ADR on scene documents and placements;
  `docs/ARCHITECTURE.md`'s new flow, the glossary, `CHANGELOG.md`.
- **Done when.** Tests without a GPU: a prefab placed three times with different overrides gives
  three subtrees, equal to the prefab but where overridden; the prefab changed, a value, an entity
  added, one removed, one renamed: what no override changes follows, overrides stay even when equal
  to the new value, the new entity appears in every placement, an override of the entity removed is
  kept and reported; two levels of nesting, the outermost winning; a cycle refused, its chain named;
  a cooked glTF scene placed as a prefab gives the entities that spawning it under a key gives,
  under the placement's path; references inside a prefab point to each placement's own entities; an
  unknown component, an unknown field or a value out of its range refuses the scene, naming them,
  and leaves the world as it was; documents cut or changed at every character refused without a
  panic; the measurements recorded; CI is green.
- **How to test.** `cargo test -p slime-world -p slime-serialize -p slime-reflect`.
- **By hand.** The owner reads the sample scene files of the ADR and the tests, and says whether a
  person can read and write them.
- **Outcome.** Done on 2026-10-17 ([ADR-0091](adr/0091-scene-documents-and-placements.md)). A
  scene file is read strictly through `slime-serialize`: `[assets]`, `[root]`, one `[entities]`
  table of records and placements whose keys give the order of siblings, and the sections of saves
  for overrides. `SceneLibrary` resolves a scene once into a `ResolvedScene`, a flat list of records
  whose overrides are applied to their values before anything is spawned; `World::spawn_scene`
  spawns it, all or nothing, and replaces the spawning of a glTF scene, which a game now resolves
  first (`ResolvedScene::from_cooked`). The root of each placement keeps its prefab and its overrides
  (`World::placement`). `slime-serialize` gained the naming of assets through `References`, the keys
  of a document's root, and the errors of a document's layout placed at their line; `Transform`
  declares the identity as its default. The ADR holds what spawning costs, measured on the zone and
  on scenes of a thousand placements; a scene of 100 000 entities waits about a second, which the
  known debt records. The world panel's identities and prefabs, which ADR-0090 decides, come with
  M5b.5, which places prefabs in the `world` example. The review found that a prefab entity gone
  made the scenes that referred to it, or added under it, refuse to spawn, where ADR-0089 sets
  such overrides aside: references and parents into a placement that misses now refer to no entity
  or give the root, and are reported; the file's entities under an entity that it removes stay,
  reported; a glTF scene counts in the depth of placements; a scene holds at most `MAX_ENTITIES`
  entities, counted before any is made; a component given twice for one entity is refused; and a
  spawning error names its scene. The owner read the scene files of the ADR and of the tests, and
  found them readable. CI took 13 minutes 42 seconds on Linux and 13 minutes 32 seconds on Windows.

### M5b.2 — A world written back as a scene

- **Goal.** A world, or a subtree of it, is written as a scene file that keeps its placements, and
  reads back to the same world.
- **Crates.** `slime-world`, `slime-reflect`; `games/hunnawood` for its tests.
- **Delivers.** A world or a subtree written as a scene file: placements as placements, with what
  differs from their prefab and the overrides set aside; entities created since given an ID when
  first written, which they keep; children in their order; entities marked `Unsaved` and transient
  fields left out; references relative to the subtree; the same world giving the same bytes. For an
  entity, the placement it belongs to and whether each of its components differs from its prefab,
  for Stage 6's inspector. The tests that edit every leaf of every component written once, if the
  milestone needs a third copy. Its ADR, or an amendment of M5b.1's; `docs/ARCHITECTURE.md`, the
  glossary, `CHANGELOG.md`.
- **Done when.** Tests: a world spawned from scene files and changed as an editor would, a placed
  value changed, a placement added and one removed, an entity of a placement removed, a component
  added, an entity created and given a parent, children reordered, is written, spawned again and
  equals it (`World::first_difference`, floats by their bits), and written again gives the same
  bytes; the file places the prefab rather than copying it; an override equal to its prefab's value
  and an override set aside survive; a created entity keeps its ID through a second write; a prefab
  spawned alone and written back gives itself; every leaf of every registered component, the
  engine's and the game's, overridden through a file, reads back; CI is green.
- **How to test.** `cargo test -p slime-world -p hunnawood`.
- **By hand.** The owner reads a file written back.
- **Outcome.** Done on 2026-10-17
  ([ADR-0092](adr/0092-a-world-written-back-as-a-scene.md)). `World::write_scene` writes a subtree
  from what the world remembers: the scenes that it spawned, now kept as shared handles, and each
  placement's prefab and overrides; a placement's overrides are what differs from its prefab's
  records, found by identity, and the overrides that its scene wrote, kept when they hold the
  prefab's value or were set aside. Entities created since get IDs of their name and four random
  characters, which the world keeps for the next write. `slime-serialize` gained
  `Composer::without_defaults` and `Composer::table`. `World::spawn_placement`, which M5b.3 was to
  bring, came here, so that an edit can add a placement; its saves stay M5b.3's. The tests that edit
  every leaf through a file reuse each crate's copy, the engine's and the game's, so that no third
  copy was written. `World::write_scene`'s rustdoc holds what writing back costs.

  The review found, and a later commit fixed: changes that a file wrote, whose component, entity or
  ancestor the world removed since, were written again, then set aside at every spawn; an override
  set aside that no longer fitted its prefab was replaced by a whole value; a glTF node without a
  name of its own gave a file that could not be read; an ID kept from a first write could make two
  entities one; a changed rotation was written as the coordinates that changed, which a prefab
  turned later made a quaternion that `Transform` refuses: a difference now holds a rotation whole,
  which amends ADR-0087, while the parts that 0.3 wrote still load. What a file cannot say of a
  root, of a placement's entity moved out, or of the order of its children, is reported; `Unsaved`
  is left out of placements too; writing is linear in the file's own entities. Tests cover each, and
  nested placements, references to entities created, placements made while the world runs,
  transient fields, and a level made from nothing. The owner read a file written back, and found it
  readable. CI took 13 minutes 37 seconds on Linux and 19 minutes 21 seconds on Windows.

### M5b.3 — Saves and hot reload with placements

- **Goal.** What a game spawns while it runs is saved as placements of its prefabs, and a run is
  carried through a save when its scenes change.
- **Crates.** `slime-world`.
- **Delivers.** Prefabs spawned from code (`World::spawn_placement`, which M5b.2 brought) under
  `created/<n>`, their entities under `created/<n>/<ID>`, and scenes spawned under a key during a
  run, saved as placements with their
  differences and spawned again from their prefabs as they are when a save is loaded; a prefab that
  the game no longer has set aside, with what was under it; a fingerprint for each scene spawned and
  each placement spawned while running, over what it resolves to, prefabs included; the save's
  format version raised, the saves of 0.3 still read; a run carried through a save, saved against an
  old start world and loaded into a new one, in one call; the ADR amending the world's documents;
  the rustdoc, `docs/ARCHITECTURE.md`'s flow of saves, `CHANGELOG.md`; the known debt on scenes
  spawned during a run retired.
- **Done when.** Tests: a run that spawns a prefab and a scene, changes them, saves and loads gives
  the same world, and saves again the same bytes; a prefab fixed after a save: the placements loaded
  take the fix where the run changed nothing; a prefab gone is set aside without a panic; the
  fingerprint names a prefab changed inside a scene; a save of 0.3 loads; a run carried through a
  save equals the run saved and loaded into the new start world, a prefab changed in every placement
  but where an override or the run changed it; saves cut at every character refused or loaded,
  without a panic; CI is green.
- **How to test.** `cargo test -p slime-world`.
- **Outcome.** Done on 2026-10-18
  ([ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md)). A save says its layout,
  `layout = 2`, and holds in `spawned` what the world spawned while it ran: each scene spawned under
  a key that the start world did not spawn, and each placement made while the world ran, by its
  root, with its scene's asset ID, name and fingerprint. Their entities are compared with the records
  of their scene, in the sections that hold the start world's differences. `World::load` takes a
  `SceneLibrary`, and spawns them first, as the library resolves them then, so that a fix of a prefab
  reaches the saves made before; what it cannot spawn is set aside. The world keeps the prefab of a
  placement made while it runs while an entity of it lives, and forgets the others as more are
  made. `World::carry` saves a run against its old start world and loads it into the new one. The
  saves of 0.3 load as they did; the fixture of *Hunna Wood* resumes. `World::save`'s rustdoc holds
  what saving and loading placements cost. *Hunna Wood* and the `world` example pass an empty
  library: they spawn nothing while they run, until M5b.5 and M5b.6.

  The review found, and a later commit fixed: a save refused while what it spawns was read left
  what it had spawned in the world, which could be left inconsistent; all that can refuse a save is
  now read before the world changes, with nothing to undo. Another scene spawned under a key of the
  start world, as when a run changes level, still made a save that loading refused: it is saved as
  spawned while the world ran, and loading destroys the start world's scene under that key first.
  A reference to a destroyed entity of a placement gave other bytes when saved again; loading many
  placements was quadratic; a save spawns at most `MAX_ENTITIES` entities while it is loaded. CI
  took 11 minutes 34 seconds on Linux and 18 minutes 26 seconds on Windows.

### M5b.4 — The binary encoding of documents

- **Goal.** A document is written in a binary encoding of the engine's own, and read back without
  parsing text.
- **Crates.** `slime-serialize`.
- **Delivers.** A self-describing encoding of the document that a text holds, without its types,
  with a version of its own; written from a text without the types, as the cooker will, and by a
  composer; read through the same reader as text, strict and lenient alike, upgrades and partial
  values included; the same bytes on every platform; limits of size and depth; errors that name the
  byte and the place, never a panic; the ADR; the crate's README, `docs/ARCHITECTURE.md`'s flow of
  serialization, the glossary, `CHANGELOG.md`.
- **Done when.** Tests: every registered type's value read from the binary equals the value read
  from the text; a document's text and its binary read to equal values; the encoding of a fixed
  document gives the bytes that the test holds, in both CI jobs; encodings cut and changed at every
  byte refused without a panic, in a bounded number of tries; too deep and too large refused; reading
  a document the size of the zone measured against its text; CI is green.
- **How to test.** `cargo test -p slime-serialize`.
- **Outcome.** Done on 2026-10-18
  ([ADR-0094](adr/0094-the-binary-encoding-of-documents.md)). A document's tree, without its
  types, is encoded with its keys and texts once each in a table of strings, then its values, each a
  tag and what it holds; floats in four bytes when an `f32` holds them exactly. `encode` encodes a
  text, a composer's too, and `Document::parse_binary` gives the `Document` that a text gives, read
  through the same reader; errors name the byte, found by walking the bytes again along their path.
  The module `format` lays out the bytes, and a test holds the bytes of a fixed document. The
  encoding is smaller than the text and decoded several times faster than the text is parsed;
  reading the values then costs the same, which `Document::parse_binary`'s rustdoc measures.

  The review found, and a later commit fixed: a short string named many times was copied each time,
  so that a small file could make the reader hold more memory than any; the keys and texts of a
  document are now counted where they are used, and refused past `MAX_SIZE`. Counts nested within a
  few bytes reserved memory at each level; they now reserve little ahead. A string given twice let
  a table hold a key twice; it is refused. `Composer::finish_binary` encoded the floats of 32 bits
  otherwise than their text, so that one document had two encodings: it is gone, and a composer's
  document is encoded from its text.

  While checking it, the text writer was found to write a table's small values before its headers,
  so that a scene file written back (M5b.2) lost the order of siblings when a placement came after
  an entity whose record holds components: once a value of a table goes under a header, the tables
  after it now do too, and a test holds the order. CI took 9 minutes 33 seconds on Linux and 19
  minutes 24 seconds on Windows.

### M5b.5 — Scene files cooked, typed components, prefabs in the `world` example

- **Goal.** Scene files are sources that the cooker cooks and a game spawns; Blender's nodes carry
  typed components; the `world` example places prefabs.
- **Crates.** `slime-cook`, `slime-assets`, `slime-world`, `slime-examples`.
- **Delivers.** `.scene` sources with their metadata files. The cooker reads their structure, checks
  their asset table against the IDs it knows and the kind that each use allows, refuses a parent
  missing, a cycle of parents and a cycle of prefabs across files, warns of a stale path, and writes
  the binary encoding, under a manifest kind of its own whose dependencies are the asset table;
  `slime-cook watch` cooks them. The `slime:<type name>` properties of glTF nodes cooked as typed
  components, read through the registry when the scene spawns; the other properties stay untyped.
  The scenes that place a prefab reloaded when it changes. The `world` example: a yard that places
  the sample scene and four lamps, one red and one without its light, written back when it quits and
  failing if it reads back otherwise, following a change of the lamp with `--watch`. The world panel
  shows each entity's persistent identity and, for a placement, its prefab. The ADR on
  cooking scene files; `examples/README.md`, `CONTRIBUTING.md` (writing and cooking a scene file),
  the crates' READMEs, `CHANGELOG.md`; the known debt on the cooked glTF scene, a format apart from
  the binary encoding.
- **Done when.** Cooking the examples twice gives the same bytes, scene files included
  (`scripts/check.sh`); the example's cooked yard spawns its placements with their overrides
  (`examples/tests/world.rs`); the cooker refuses, naming the file and the line, an unknown asset, a
  prefab missing, a cycle across files and a malformed file; a typed property on a node of a glTF
  scene built in a test becomes its component, and a wrong one refuses the scene, naming the node,
  the type and the field; a prefab cooked again: the scenes that place it are reloaded, and a test
  without a GPU shows the world following it, on both platforms; the example runs without a window
  and in Weston on Linux, and without a window on Windows; CI is green.
- **How to test.** `cargo test -p slime-cook -p slime-world -p slime-examples`;
  `cargo run --example world`, and with `--watch` beside
  `cargo run --release -p slime-cook -- watch examples/assets target/cooked/examples`.
- **By hand (Linux).** The owner changes a value in the lamp's file and sees every lamp change but
  the one that overrides it, then changes an override and sees its lamp change.
- **Outcome.** Done on 2026-10-18 ([ADR-0095](adr/0095-cooked-scene-files.md)). The cooker reads a
  scene file's layout through `slime_world::outline`, without the game's types, and checks its table
  of assets against the IDs and kinds of the sources, its parents and its prefabs, across files; it
  writes the file's binary encoding under the manifest's kind `SceneFile`, whose needs are the
  table's assets. A glTF node's `slime:<type name>` properties are checked as values when imported,
  and kept in the cooked scene's node as a binary document, which `ResolvedScene::from_cooked` reads
  strictly through the world's registry; the cooked scene's format and the import of glTF files took
  a new version. `SceneLibrary::of` reads the scenes of cooked data when each is first resolved;
  `SceneLibrary::reload` forgets the scenes whose cooked files changed and resolves every scene
  again, so that a yard follows its lamp, as a test that cooks the lamp again shows without a GPU.
  The `world` example spawns `yard.scene`, four lamps of `lamp.scene` around the sample scene, and,
  when it quits, writes the yard back and checks that the file spawns the world written; with
  `--watch`, it carries its run into the yard spawned anew. The world panel shows the identity of
  the entity selected and the prefab of a placement.

  The review found, and a later commit fixed: after a change that the world refused, the example's
  `--watch` built the next start world from the refused scenes, and carried nothing more; the
  library now goes back to the cooked data that the run was spawned from. *Hunna Wood* panicked
  when its hero, loaded again, held a typed property that its registry refused: the hero now stays
  as it was. A typed property's text could hold several values through a line break, and an array
  or an object was left out with a warning: both refuse the import. `outline` placed each entry by
  counting the text's lines from its start, and walked each entity's ancestors to the root, so that
  a file of 40 000 entities took 6.5 s: documents now keep the offsets of their line breaks when
  they place errors (`slime-serialize`), and each entity's ancestors are walked once, 0.3 s for
  such a file. The cooker let a parent's path go into an entity that is not a placement; a scene
  file renamed kept its old name after a reload; the example's ring drifted at each carry. A scene
  file's fingerprint now covers its binary encoding, whether cooked or made from its text, which
  ADR-0095 records with the glTF scenes' fingerprints that the new format changes.
  The review also found that a typed property could not say which version of its type it is
  written in, so that a value of a type past version 1 was brought through upgrades meant for
  version 1: the owner chose that a node gives the versions in its property `slime:$versions`, as
  a document's `$versions` does.
  The owner changed the lamp's file in the running example, and saw every lamp change but the one
  that overrides it, then an override, and saw its lamp change. CI took 13 minutes 51 seconds on
  Linux and 11 minutes 39 seconds on Windows.

### M5b.6 — *Hunna Wood*'s zone made of prefabs

- **Goal.** *Hunna Wood*'s zone is a scene file that places its land and its lanterns, its data
  typed in its files.
- **Crates.** `games/hunnawood`: its code, its zone's scripts, its list of source assets, its
  fixtures; the engine's crates for what the game finds missing.
- **Delivers.** The zone's script exports the land and the lantern apart, as it does the hero, the
  lantern's glass apart from its body; it names every node and fails when Blender renames one, and
  writes colliders, markers and times of day as typed components. A scene file written by hand
  places the land under `land` and the lantern six times, the third lighting less and the fifth
  without its light and its glass; the lanterns' places move there from the script's placements.
  The game's reading of untyped extras goes. The release of the asset mirror. Saves on the zone made
  of prefabs; the fixture made again, and the fixture of 0.3 kept with its test. Hot reload carries
  the run through a save, and decides that the layout changed from the old and the new start worlds.
  The cost of spawning the zone and of building its frame, measured against 0.3. The game's README
  (how it is made, the zone, changing a prefab, making the fixture again), `CHANGELOG.md`; the known
  debt on the hot reload's comparison retired, and the one on renamed nodes updated.
- **Done when.** The game's tests pass on the zone made of prefabs: the walk past the campfire, the
  key, the door and onto the terrace; the door without the key; the hero kept out of the colliders;
  the restart; a run saved, resumed and saved again to the same bytes, and the same 300 steps
  later; the new fixture resumed with nothing set aside; the fixture of 0.3 set aside as a save of
  another layout, a new run started, without a panic. The zone's tests: one marker of each kind; six
  lanterns from one prefab, the third at its lower intensity and the fifth without light or glass;
  the colliders where 0.3 had them, within 10⁻⁴, whatever their order; no node named with Blender's
  numbers; the budgets counted over the whole zone. The lantern changed in a test: every lantern
  follows but the parts overridden. A collider changed through the world panel does not make a
  reload start the run again. `scripts/check.sh` runs the game by day, at night, around a lost
  device and resumed from the new fixture. The owner plays it on Linux and finds it unchanged. CI is
  green.
- **How to test.** `cargo test -p hunnawood -- --ignored`; `cargo run -p hunnawood`, and with
  `--watch` beside `slime-cook watch`.
- **By hand (Linux).** Play by day and at night; F5, F9, quit and resume; change the lantern in
  Blender and export it while the game runs; move a lantern in the zone's file, a change of layout
  that starts the run again.
- **Outcome.** Done on 2026-10-18. The zone's script exports `land.gltf`, which keeps the ID of the
  zone's former export, and `lantern.gltf`, a lantern on its post at the origin, its glass a node of
  its own; every node is named for what it is, `oak 12 collider`, and the script fails when Blender
  numbers a name. Colliders, markers, the lights' times of day and the campfire's flicker are typed
  properties, and the game's reading of untyped extras is gone, with the search for the campfire's
  light near its marker. `assets/zone/zone.scene`, written by hand, places the land and the six
  lanterns where `placements.toml` placed them, the third at 50 cd instead of 140, the fifth without
  its light and its glass, and shows the moon in the sky. The game spawns it through
  `SceneLibrary::of`; hot reload makes a new game of the scenes before and after the change,
  restarts when their layouts differ and carries the run otherwise (`saves::reload`). The mirror's
  release `hunnawood-7` holds the four new files; the textures and the hero did not change.

  The script used to take a model's meshes in the order of a Python set, which changed from one run
  to the next, and with it the rock placed at each scattered place: it takes them by name now, and
  the land and the lantern come out the same from one run to the next, the hero not yet. The
  scattered rocks thus have other meshes than in 0.3, at the same places: their colliders keep
  their centers, with the sizes of their new rocks, and the test of the colliders of 0.3 compares
  only their centers. Other variants changed with the order too, the ferns' and seven of the ten
  rocks of the gully; and the lanterns' bodies, which 0.3 hung unturned whatever their posts, now
  turn with them. The layout's fingerprint hashes the colliders in the order of their numbers, so
  that the order of the scene file's records does not change it. The fixture was written from the
  tests' walk to the chapel's portal, its hero where the fixture of 0.3 stood; the owner chose to
  keep it, rather than make it again by playing as ADR-0090 planned. That of 0.3 is kept, and a test
  sets it aside as a save of another layout.

  Measured in a release build on the owner's machine, side by side with 0.3: spawning the zone's 555
  entities takes 0.59 to 0.86 ms, as 0.3's 543 took 0.72 to 0.90 ms; reading the scenes from cooked
  data and resolving them takes 1.24 to 1.75 ms, and resolving them again, their files read, 1.14
  to 1.21 ms, where 0.3 resolved its one scene, already in memory, in 0.52 to 0.65 ms: the scene
  file and its seven placements cost twice as much. Building a frame of the zone alone, 422
  instances and 8 lights, takes 89 to 92 µs at best, against 82 to 86 µs for 0.3's 429 instances and
  9 lights.

  The review found, and a later commit fixed: the scene file's comments said east for west; no test
  checked which way a lantern faces, which the places of the lanterns' lights in 0.3 now do; the
  layout's fingerprint followed the order of the scene file's records, so that moving one would have
  set aside every save; and this outcome said less than what looks different from 0.3. It also
  found that any change of the cooked data, a texture's, makes a new game of the scenes before the
  game knows whether a scene changed: about 2 ms, left so in a tool of development.
  The owner played the zone by day and at night, saved, loaded and resumed a run, changed the
  lantern in Blender while the game ran and moved a lantern in the scene file, and found the zone as
  it should be. CI took 14 minutes 17 seconds on Linux and 19 minutes 48 seconds on Windows.

### M5b.7 — Windows check, documentation, release 0.4

- **Goal.** Stage 5b is done and released as 0.4.
- **Crates.** All, for their documents.
- **Delivers.** The documents reread as a newcomer would; the owner plays the game and the `world`
  example from CI's Windows programs, and saves and resumes there; `CHANGELOG.md`'s 0.4 section, the
  tag and the GitHub release, as [Releasing](#releasing) says; Stage 5b marked done; Stage 6 planned
  with the owner.
- **Done when.** The exit criteria hold on Linux and on Windows; `v0.4.0` is on a commit whose CI is
  green, and its GitHub release carries the Windows programs.

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)):

1. In one commit, set the version in the root `Cargo.toml` and the tag that the README's "Using
   SlimeEngine in a game" names, rename the `[Unreleased]` section of `CHANGELOG.md` to
   `[<version>] - <date>`, and update this plan.
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.
4. From 0.1 on, create the GitHub release of the tag, its `CHANGELOG.md` section as its notes,
   with the `slime-examples-windows` artifact of that commit's CI run attached as
   `slime-examples-windows-<version>.zip`, holding beside the programs, and DXC's library and
   licence that CI puts with them, `LICENSE.txt`, a `NOTICE.txt` and the licenses of the egui fonts
   that `scene.exe`, `world.exe` and `hunnawood.exe` embed in their dev overlay, which the Ubuntu
   Font Licence asks to travel with them: CI keeps its artifacts 14 days, a release keeps them.

0.0.1 was released on 2026-10-12, before Windows joined CI: the tag `v0.0.1` is on the commit
"Release 0.0.1". 0.0.2 was released the same way on 2026-10-12: the tag `v0.0.2` is on the
commit "Release 0.0.2", and 0.0.3 on 2026-10-13: the tag `v0.0.3` is on the commit
"Release 0.0.3". 0.1 was released on 2026-10-14: the tag `v0.1.0` is on the commit "Release 0.1.0",
and its GitHub release carries the Windows examples that CI built from that commit.
0.2 was released on 2026-10-15: the tag `v0.2.0` is on the commit "Release 0.2.0", and its GitHub
release carries the Windows programs that CI built from that commit, *Hunna Wood* with its zone
cooked and the notice that credits its assets among them. 0.3 was released on 2026-10-17, with
Stages 4b and 5a: the tag `v0.3.0` is on the commit "Release 0.3.0", and its GitHub release carries
the Windows programs that CI built from that commit, `world.exe` among them.

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3a — First renderer → 0.0.3 | M3.1 GPU resources for a renderer; M3.2 the render crate; M3.3 directional light and material; M3.4 textures and samplers; M3.5 a glTF scene and camera control; M3.6 statistics, GPU timings and the dev overlay; M3.7 Windows check, documentation, release | Done; released as 0.0.3 (tag `v0.0.3`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3b — Physically based renderer → 0.1 | M3b.1 GPU layer for multi-pass rendering; M3b.2 HDR rendering and tone mapping; M3b.3 physically based materials; M3b.4 light types; M3b.5 shadows; M3b.6 image-based lighting; M3b.7 bloom; M3b.8 render graph; a pass over the known debt; M3b.9 Windows check, documentation, release | Done; released as 0.1 (tag `v0.1.0`), with the Windows examples on its GitHub release. Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3c — A first game | M3c.1 skeleton; M3c.2 guards, detection and the objective; M3c.3 night, feel and the Windows check; M3c.4 findings for the next stages | Done; no release of its own: its game, *Nightwatch*, is never released, as *Hunna Wood* replaces it in Stage 4a ([ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)). The stage each of its findings goes to is in [ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md), and the two that became Stage 3d in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md). |
| 3d — Anti-aliasing and the shadows of spot lights | M3d.1 multisampling in the GPU layer; M3d.2 anti-aliasing in the renderer; M3d.3 spot lights' shadows: what each tile sees; M3d.4 spot lights' shadows: drawn and read; M3d.5 *Nightwatch* and the Windows check; M3d.6 a smoother shadow filter; M3d.7 what each cascade sees; M3d.8 the cascades read; M3d.9 *Nightwatch* at 3840 × 2160 | Done; no release of its own: what it delivers is released with 0.2. Its decisions are in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) and ADR-0058 to [ADR-0065](adr/0065-reading-the-cascades.md); the cost of its scene pass on Direct3D 12 with each shader compiler, which made DXC ship beside the Windows programs, is in [ADR-0066](adr/0066-shipping-dxc-beside-the-programs.md). |
| 4a — Cooked assets → 0.2 | M4a.1 *Hunna Wood*'s skeleton; M4a.2 asset identity; M4a.3 images; M4a.4 glTF; M4a.5 the sample scene through the pipeline; M4a.6 the zone's sources; M4a.7 *Hunna Wood* by day; M4a.8 night; M4a.9 Windows check, documentation, release 0.2 | Done, released as 0.2 on 2026-10-15. Its decisions are in [ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md) to [ADR-0071](adr/0071-cooked-textures.md); the zone's budgets are set by the test that checks them, in `games/hunnawood/src/zone.rs`, and what was left to refine went to Stage 4b's M4b.8. |
| 4b — Live assets | M4b.1 a shared GPU and a task pool; M4b.2 the zone loads while frames go on; M4b.3 recovering from a lost GPU device; M4b.4 *Hunna Wood* goes on after a lost device; M4b.5 materials that change in place; M4b.6 hot reload of cooked data; M4b.7 hot reload of shaders; M4b.8 refining *Hunna Wood*; M4b.9 Windows check, documentation, close | Done on 2026-10-16; no release of its own: what it delivers is released with 0.3, at the end of Stage 5a. Its decisions are in [ADR-0072](adr/0072-stage-4b-live-assets.md) to [ADR-0077](adr/0077-hot-reload-of-shaders.md). The owner checked the recovery from a lost device by hand at its close: in the Windows virtual machine, from the dev overlay and by a forced TDR, and in a KDE Plasma session after a real reset of the GPU (see [Known debt](#known-debt)). |
| 5a — The world → 0.3 | M5a.1 the world: entities and components; M5a.2 the hierarchy and the transforms; M5a.3 what the world draws; M5a.4 *Hunna Wood* on the world; M5a.5 reflection; M5a.6 any component listed and edited; M5a.7 serialization with format versioning; M5a.8 the world round-trips; M5a.9 saving and resuming a run; M5a.10 Windows check, documentation, release 0.3 | Done, released as 0.3 on 2026-10-17 with Stage 4b. Its decisions are in [ADR-0078](adr/0078-stage-5-in-two-halves.md) to [ADR-0088](adr/0088-saving-and-resuming-a-run.md), with the measurements of the world's storage, transforms and frame in ADR-0082 to ADR-0084; the save that later versions of *Hunna Wood* must read is `games/hunnawood/fixtures/save.toml`, and how long CI took with each crate it added is in the [Notes](#notes). |

The details of each completed milestone (what it delivered, how it was checked, what was measured)
are in the history of this file and in the commits that implemented it; the measurements that
justify a decision are also in its ADR.

## Known debt

Limitations known and accepted for now. The ones that a contributor could take on alone are also
GitHub issues, linked from their entry; this list stays the reference.

- **The first frame that draws shadows costs more, and more still while a scene loads**
  ([ADR-0074](adr/0074-uploading-textures-without-holding-up-frames.md)). It makes the renderer
  create its shadow atlas and its frame again; during a load, the GPU, busy with the load's copies,
  presents that frame late, past 33 ms in some runs. *Hunna Wood* draws its hero, and so its first
  shadows, from its first frame. Creating the atlases with the renderer would remove it, at a cost in
  memory for every program, shadows or not.
- **The zone costs up to 8 s per frame on lavapipe**, the CPU driver of the checks, where the RX
  6900 XT takes under 2 ms: most of it goes to the four shadow cascades of the sun, each 2048 ×
  2048 texels, through which thousands of overlapping leaf cards are drawn with a shader that
  discards. The checks run the game for one frame by day, one at night, two around a lost device
  and one resumed from the save fixture. The oaks' cards made smaller and four times as many at
  M4b.8 changed it little: the day's shadow pass took 6.0 s
  instead of 6.2 s, in a release build. Leaves that cast no shadow in the farther cascades would
  lower it, to measure with views of the zone.
- **`cargo test` leaves out the tests of the cooked zone**: they need the zone fetched and cooked,
  so they are ignored (`#[ignore]`); `scripts/check.sh` and CI's Windows job fetch and cook the
  zone, then run them (`cargo test -p hunnawood -- --ignored`).
- **Frames are not synchronized with the display's refresh** (ADR-0021; issue
  [#1](https://github.com/HoneySlatt/SlimeEngine/issues/1)). With Mailbox, the default, the
  schedule alone paces frames: on a 60 Hz display, an image is now and then shown twice while the
  next one is dropped. On a display of 144 or 165 Hz, the default 60 frames a second last 2 or 3
  refreshes in turn, which shows as judder in motion: a game can raise `AppConfig::max_frame_rate`
  to the display's rate, which the engine does not report, or choose `PresentMode::Fifo`, which
  shows one image per refresh but can stall as the next entry says. Aligning frames on the
  compositor's frame callbacks is to reconsider when a renderer draws real scenes.
- **Without Mailbox, presenting can still stall the loop.** Windows that do not offer it, and
  applications that choose `PresentMode::Fifo`, wait for the display: one frame per second for a
  hidden window on niri. In a headless Weston with lavapipe, `acquire` waits for Weston's frame
  callbacks for a time that depends on the machine's load: 120 frames of the `window` example took
  15 s on 2026-10-12 (release build, waits of 130 ms on average), 9 s and 25 s in other
  measurements the same day, against 2 s with Mailbox.
- **A hidden window keeps drawing** at the scheduled rate (2 to 3 % of one CPU core on the
  development machine): the engine cannot tell on Wayland whether its window is visible. winit
  reports occlusion on X11, which the engine ignores: under a compositing manager, Xwayland
  included, the X server reports windows as visible anyway.
  On Windows, a window covered by another keeps drawing too; a minimized one, of size zero, draws
  nothing (M3b.9).
- **wgpu's errors panic, in release builds too** (ADR-0016): a use of wgpu that it rejects is
  treated as a bug. Before games ship, logging the error and dropping the frame is to be
  considered. Running out of GPU memory is not such a use: creations return
  `GfxError::OutOfMemory`, and memory that runs out where no caller hears of it gives the device up,
  which stops the run (ADR-0044). The engine avoids the rejections that the environment can
  cause: a window larger than the device's largest image is drawn at that size (`Surface::resize`),
  and a shader or pipeline that the driver rejects is returned as an error (`GfxError::Shader` or
  `GfxError::Pipeline` on Direct3D 12; on Vulkan, wgpu treats the driver's failure as a lost
  device, `GfxError::DeviceLost`); a window's surface that the graphics API refuses to configure
  counts as lost (ADR-0025). The refusal of a configuration is not tested: nothing the tests
  control makes a conformant driver refuse one.
- **On Linux, a game goes on after a real reset of the GPU only if the compositor does**
  (ADR-0075). A reset reaches every program that uses the GPU. Mesa ends the ones that did not ask
  to be told of it, as niri, Hyprland and COSMIC do not: on 2026-10-15, programs that hung the
  owner's RX 6900 XT made the amdgpu driver reset it twice, and niri ended each time, taking the
  session with it. KDE Plasma's KWin recovers from resets: on 2026-10-16, after a reset forced on
  the owner's machine in a Plasma session, *Hunna Wood* opened a new device 70 ms after it found the
  loss and drew its zone again 2 s later, its run kept, while the terminal it was started from and
  Plasma's panel ended. On Windows, the desktop's compositor survives resets, and the game went on
  after a forced timeout detection and recovery (TDR) in the owner's virtual machine.
- **A lost GPU device is replaced only for the applications that ask for it** (ADR-0075; issue
  [#2](https://github.com/HoneySlatt/SlimeEngine/issues/2)): the others stop with an error, and
  since winit allows one event loop per process, carrying on means starting the process again. An
  application that asks drops and creates its GPU objects again itself, as *Hunna Wood* does. A
  real driver reset can only be checked by hand, as it was at Stage 4b's end (above). A restore
  that fails in a windowed run is not tested: nothing makes one fail on purpose there.
- **In capture tools, the sky pass shows as a second `scene`.** A pass is named after the target it
  draws into, and the sky pass draws into the scene's high-dynamic-range target, after the scene
  pass (`declare_frame` in `crates/slime-render/src/renderer.rs`); only its pipeline is named `sky`.
  Seen in RenderDoc on Direct3D 12 (M3b.9).
- **Frames pause on Windows while the user moves the window, holds its title bar or a border, or
  opens the system menu**: the system then runs a loop of its own, in which no redraw arrives and
  the engine's loop does not wake, so frames and the check for termination signals pause until it
  ends (ADR-0020; the move was measured in M2.4). Keeping frames going would mean asking for
  redraws from inside each frame and waiting there for the schedule.
- **wgpu-hal logs an error when an adapter cannot present to the window**, even though the engine
  then uses another one: seen with RADV in a headless Weston, where only lavapipe can present.
- **No title text in client-side decorations.** On compositors that do not draw decorations
  (GNOME), the title bar has no text, because winit's text renderer depends on an unmaintained
  crate (ADR-0011).
- **Input not reported yet** (issue [#3](https://github.com/HoneySlatt/SlimeEngine/issues/3)):
  input-method text, touch, touchpad gestures, gamepads, dropped files, the mouse's raw motion.
  The window cannot grab or hide the cursor, go fullscreen, or change its title once created.
- **A compositor that stops answering blocks the engine.** Before the first frame, winit waits for
  the window's first configuration without a timeout, so a termination signal is noticed only when
  a second one ends the process; after `shutdown`, releasing the window or the surface can block
  too (seen with a Weston stopped by SIGSTOP).
- **Logging off or shutting down Windows ends a game without `App::shutdown`.** Windows tells a
  window of it with end-session messages, which winit does not report, and sends no console event
  to a process that has loaded `user32.dll`, as every engine process has
  (`crates/slime-app/src/signal.rs`): what a game saves in `shutdown` is lost then. *Hunna Wood*
  saves its run there since M5a.9: the run since the last F5 is lost when Windows ends the session
  (ADR-0088). A fix needs a new dependency, which ADR-0081 leaves to a later decision.
- **WARP's shader compiler crashes the process, now and then, when several threads compile
  shaders at once**, even on separate devices: an access violation (`STATUS_ACCESS_VIOLATION`)
  that other projects trace to `d3d10warp.dll`, and that no test can catch. Seen once in the
  Windows CI job, on 2026-10-13, in the texture tests of
  `slime-gfx`, after about ten green runs; other projects that test on WARP report the same crash,
  and that running the tests one at a time avoids it, which the Windows job does since, and which
  `CONTRIBUTING.md` asks of contributors on WARP. The engine's own code is not involved.
- **Client-side decorations run `dbus-send` twice when the window is created** (winit's
  sctk-adwaita, on compositors that do not draw decorations), each waiting up to 100 ms when the
  settings portal does not answer; the error they log when no session bus or portal answers is
  hidden by the default log filter (ADR-0019).
- **Upgrading wgpu waits for egui** while the engine's tooling draws with it: the dev overlay, and
  from Stage 6 the editor (ADR-0031, ADR-0052). `egui-wgpu` is built against one wgpu version
  (egui 0.36 against wgpu 30 today), so wgpu moves only once egui follows.
- **The dev overlay blends in an sRGB framebuffer.** egui prefers a linear one for its blending,
  warns once at startup, and its panels look slightly lighter than egui's own demos (ADR-0031).
  Harmless for tooling.
- **Mipmaps of textures made in code are computed on the CPU when they are created**
  (`Gpu::create_texture`, whose documentation gives the cost), on the calling thread, in work
  proportional to the texture's pixels (ADR-0032): a level of tens of large textures waits seconds.
  A caller with levels of its own gives them to `Gpu::create_texture_with_levels` (ADR-0039), as
  cooked textures do, whose levels the cooker computed (ADR-0071).
- **Cooked bytes are the same from one cook to the next on one platform, not across platforms**
  (ADR-0071): the cooker computes the sRGB curve with the platform's mathematical functions, which
  may round differently on Linux and Windows. Tests compare decoded values across platforms; a
  table of the curve's values would make the bytes the same everywhere.
- **A cook reads and hashes every source file at every run**, and cooked files that no source makes
  any more stay in the output directory (ADR-0071): nothing records a source's size and time to
  skip unchanged ones, which is to measure on *Hunna Wood*'s zone, nor cleans the output, where
  each change that `slime-cook watch` cooks leaves the files it replaced (ADR-0076).
- **Two watches repeat the same polling** (ADR-0077): `slime_assets::ManifestWatch` and
  `slime_render::ShaderWatch` each read their files every half second and tell a failure once,
  with the same dozen lines, over what each reads. No crate that both reach suits a type for both
  yet.
- **Every offscreen render target and depth target can be sampled** (ADR-0033): they all carry the
  usage that allows it, which may cost some compression on some GPUs. Measured at M3b.5 on an RX
  6900 XT (RADV), with two builds that differ only by it: no cost (the scene pass took 43.9 µs
  with it and 44.0 µs without at 1920 × 1080, ADR-0033's correction). Other GPUs are not measured.
- **A pipeline that does not fit its pass, or a target sampled by the pass that draws into it,
  panics only when the commands of its submission are finished** inside `Gpu::submit` (or
  `Gpu::render`, `Gpu::render_depth`), with wgpu's message naming the pass, rather than at the
  faulty call (ADR-0033, ADR-0041). For the renderer's frame, the render graph refuses a pass that
  samples what it draws into before wgpu sees it.
- **On X11, losing the connection to the X server ends the process** with exit status 1, inside
  the X11 library (Xlib). `EventHandler::exiting`, `App::shutdown` and the return of `run` do not
  happen, although ADR-0010 promises that `shutdown` is called once `init` has been. winit
  installs no IO error handler. Installing one (`XSetIOErrorExitHandler`, libX11 1.7 or later)
  would need a new direct dependency such as `x11-dl`. On Wayland, a compositor that dies ends the
  loop cleanly.
- **Without libxkbcommon, the windowing library panics** rather than returning
  `PlatformError::Connect`. On X11 it panics at once, also without libxkbcommon-x11 or without the
  X server's XKB extension. On Wayland it panics when the compositor first reports a keyboard,
  which can be in the middle of a run. Turning it into an error would mean probing the libraries
  before winit does.
- **A termination signal received after `run` has returned no longer ends the process by itself**
  (ADR-0012). `ctrlc` cannot remove the handler that `run` installs. A first signal received then
  only stops later runs, and logs that the run stops after the current frame; only a second one
  ends the process, with exit code 130. This matters for a program that keeps working after
  `run`, for example under a service manager, which waits out its stop timeout. Ending the process
  at once when no run is active would amend ADR-0012.
- **A window frame dropped without being presented keeps one of the window's images on Vulkan.**
  wgpu 30 discards such a frame with wgpu-hal's `discard_texture`, which does nothing on Vulkan.
  The image stays acquired until the surface is configured again, and once none is left,
  `Surface::acquire` waits one second and returns `None`. `SurfaceFrame` documents this as a
  misuse, and nothing reports it; the engine always presents. A `Drop` that presents was rejected:
  it would show a black or half-drawn frame instead of skipping it, and present during unwinding.
- **An oversize window's surface is configured again at every frame on X11.** A window wider or
  taller than the device's largest image (8192 pixels with the default limits) gets images of that
  size, smaller than the window. On X11, Mesa's Vulkan drivers mark such a swapchain suboptimal, and
  every image of it too. `Surface::acquire` treats each suboptimal image as a reason to configure
  the surface again, at the same clamped size, so every frame waits for the GPU to finish and
  rebuilds the window's images. The frames are correct, and only `debug` logs show it; Wayland and
  Direct3D 12 are not affected. Not setting `stale` in the `Suboptimal` arm of `Surface::next_image`
  while `oversize` is set is not enough: measured, the `Outdated` branch then configures the surface
  again. Asking the device for the adapter's own largest image (`max_texture_dimension_2d`) instead
  of the default limit would make such windows rare. To measure, before and after: start Xvfb with a
  screen wider than 9000 pixels; with `WAYLAND_DISPLAY` unset, `DISPLAY` set to it and lavapipe, run
  the child of the `oversize` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` with
  `SLIME_LOG=wgpu_core=debug`; count wgpu-core's `configuring surface with` records (about one per
  frame without the fix, one with it) and note the frame times.
- **Termination signals are ignored for the rest of the process when `ctrlc` cannot start its
  thread.** `ctrlc` installs its handlers, then starts the thread that waits for what they catch.
  When that thread cannot start (a limit on threads reached when `run` installs the handler), the
  handlers stay installed and SIGINT, SIGTERM and SIGHUP (Ctrl+C and Ctrl+Break on Windows) are
  ignored: only SIGKILL, or closing the console on Windows, ends the process. The engine logs a
  warning that says so and gives the system's error. Restoring the default dispositions would need
  a direct dependency on libc or nix.
- **Any `trace` directive in `SLIME_LOG` makes wgpu build its per-command trace records.**
  tracing-subscriber sets the `log` crate's maximum level from the most verbose directive. With
  `slime_app=trace`, wgpu-core then formats and filters out about five records per draw while it
  encodes a pass, which counts in the caller's span (`render` in a windowed run): measured with
  `RenderStats::cpu_time` for a scene of 1000 instances, 0.38 ms more per frame, 35 %, with
  `slime_app=trace` than with `SLIME_LOG` unset. Alternatively, profile with
  `perf record -g` and look for `tracing_log::LogTracer::log`, `EnvFilter::enabled` and
  `wgpu_core::resource::Labeled::error_ident`. Capping the `log` level needs `log` or
  `tracing-log` as a direct dependency (ADR-0006). Span close lines are also written while the
  enclosing span is entered, so `frame`, `render` and, with `slime_gfx=trace`,
  `RenderStats::cpu_time` include them: take timing baselines with `SLIME_LOG` unset.
- **On Windows with Vulkan (`WGPU_BACKEND=vulkan`), an acquire that waits more than one second
  loses the device.** wgpu-hal 30.0.1 waits on a fence after `vkAcquireNextImageKHR` hands out an
  image, on Windows only, and maps a timeout of that wait to `DeviceError::Unexpected`, which
  wgpu-core treats as the loss of the device. A GPU frame longer than about one second, or a
  compositor that holds the window's images that long (a covered window, a locked session), then
  stops the run with `GfxError::DeviceLost`, with a reason that blames the driver; on Linux the
  frame is skipped instead. Not reproduced: in the owner's Windows 11 virtual machine with an AMD
  driver, the `scene` example kept running with its window covered for six seconds and with the
  session locked (2026-10-13). To report to wgpu: that wait's timeout should give
  `SurfaceError::Timeout`, as its other waits do.
- **Seen from below, the sample scene's walls and crates flicker where they touch the floor.** Their
  bottom faces lie in the floor's plane, and since M3b.3 the floor is double-sided, so from below
  both are drawn at the same depth (z-fighting). The `scene` example keeps its camera above the
  floor, but another viewer of the asset sees it. Lifting them by a few millimetres in
  `examples/assets/make_scene.py` fixes it, at the cost of rendering the glTF references again.
- **A mouse button can stay down on Wayland when the compositor takes the pointer away during a
  press without taking the keyboard focus.** winit 0.30 reports only `CursorLeft` then, and
  `Translator` (`crates/slime-platform/src/event.rs`) releases held buttons only when the focus is
  lost. Releasing them on `CursorLeft` would be right on Wayland only: on X11 and Windows the
  cursor leaves the window during an ordinary drag.
- **On Windows, the first of two held Shift keys let go of is released late.** Windows sends a
  single key-up for both Shift keys, so the engine releases the first one only when the second
  goes up. Windows 11 marks the second Shift's press as a repeat, so neither its press nor its
  release is reported (seen in the owner's virtual machine on 2026-10-13). Polling `GetKeyState`,
  as GLFW and SDL do, would remove the delay and report both keys, but needs `windows-sys` as a
  direct dependency.
- **After a dead key, a press and its release report different logical keys.** On Wayland and
  X11, a character key that does not combine with a pending dead key is reported as
  `Key::Unidentified` with no text; when the keys combine, the press is the composed character and
  the release the plain one. winit exposes no better value; rebuilding one would need libxkbcommon
  called directly. Documented in `KeyEvent::logical`.
- **A key pressed just after the window gains the focus can be swallowed on X11 and Windows.**
  winit reads which keys are down when it handles the focus event, not when the focus changed. A
  key pressed in between counts as held before the focus and is not reported until it is pressed
  again; on X11 the gap covers `App::init` when the window manager focuses the window while it
  runs. The translator cannot tell such a press from X11's unmarked first auto-repeat. On Windows,
  trusting `KeyEvent::repeat` would close the gap, at the cost of a rule that differs between
  platforms; left as documented.
- **No test covers the empty submission of frames without a window image.** When
  `Surface::acquire` gives no image, it submits an empty batch, so that wgpu releases the writes
  an application makes outside `App::render`. Without it, 2000 frames of a surface resized to
  zero, each writing 64 KiB to a uniform buffer, grew the process by about 120 MiB (lavapipe in a
  headless Weston, 2026-10-13), measured by a program kept out of the repository. The
  `draws-nothing` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` checks the same for
  frames whose image nothing draws, not for frames without an image.
- **A GPU time can read 0 once when the timestamp counter wraps.** On adapters whose timestamps
  have 36 valid bits (Intel's Vulkan driver, about once every one to one and a half hours), a
  timed pass that runs across the wrap ends before it starts. `GpuTimer::latest` then gives 0 for
  that pass until the next time replaces it. wgpu does not tell how many bits are valid, so the
  wrapped difference cannot be recovered. Dropping such a sample (`end.checked_sub(start)` in
  `crates/slime-gfx/src/timer.rs`) is a one-line change if it ever matters.
- **The instance buffer is uploaded whole every frame.** `Renderer::render` uploads
  `capacity × stride` bytes (256-byte slots with the default limits), where the capacity is a
  power of two from 64 that fits the frame's instances, up to four times as many: it shrinks back
  once a frame uses a quarter of it or less, so that a 10-instance frame after a 100 000-instance
  one costs what it costs on a new renderer (0.054 ms against 21.4 ms before, on lavapipe).
  Uploading only the live slots would amend ADR-0026's rule that a uniform buffer is always
  written at its creation size.
- **The dev overlay can still panic on a lost device** (ADR-0022). `Overlay::render` skips the
  frame when the device was lost earlier in it, but two cases panic inside egui-wgpu or wgpu: a
  loss that egui-wgpu's own buffer upload is the first to find, and `Overlay::new` on a device
  already lost (egui-wgpu creates its buffers with `create_buffer_init`). Both are documented
  under `# Panics`; closing them needs a change in egui-wgpu or wgpu.
- **Exposed light beyond 64 × 65 504 is clamped** (ADR-0035, ADR-0037, ADR-0039, ADR-0040). The
  scene and sky shaders write exposed light divided by 64 into the `Rgba16Float` target, and light
  above 64 × 65 504 as 65 504, the largest half, rather than leave it to the driver, which may store
  an infinity that the tone curve turns into NaN; lavapipe and RADV clamp it themselves, so that no
  test can show either clamp. The sun's disc, 1.47 × 10⁹ nits for the sample's sun, is clamped from
  an exposure of 2.85 × 10⁻³, six and a half stops above the sample's; emission above
  64 × 65 504 / exposure, and any light once the exposure is raised about 13.5 stops above the
  sample's, are clamped too, and the bloom spreads them with the energy they keep. A sky's radiance
  is at most 65 504 nits before the exposure: brighter zenith, horizon or ground is refused, and a
  brighter glow kept within it. Divided by 64, exposed light below 64 × 2⁻¹⁴, about 0.004, is
  stored in half precision's subnormals, in steps of 64 × 2⁻²⁴: black on an 8-bit display, whose
  darkest step is about 0.007 exposed; an output of higher precision would show its steps.
- **Every fragment evaluates every light of the scene** (ADR-0037). The scene pass has no light
  culling: its cost grows with the number of lights, about 20 µs per light for a screen of 1920 ×
  1080 fragments on an RX 6900 XT (2026-10-13), 338 µs for 16, the most a scene holds, and 10 to 20
  times more on an integrated GPU; a light's range saves no work. More lights would need clustered
  or tiled culling and a storage buffer, a measured decision of Stage 9a (ADR-0052).
- **Point lights cast no shadows, and light what objects hide** (ADR-0037; the first directional
  light that casts shadows casts them, ADR-0038, and so do spot lights, ADR-0058). The sample's ring
  light, a point light, reaches 4 m, and gives little beyond a metre. The shadows of point lights
  come with Stage 9a (ADR-0052).
- **Every instance is drawn, with a draw call of its own** (`Renderer`). Nothing leaves out the
  instances that the camera does not see, and instances of the same mesh and material are not
  drawn together: a frame's CPU and GPU time grow with every instance of the scene, wherever it is.
  Frustum culling and instancing are left to Stage 9a (ADR-0052).
- **Cascades are fitted to spheres around slices of the view** (ADR-0064). Each covers its slice
  whichever way the camera looks, which keeps its texels steady, but a wide view makes the spheres
  large: in the first game, *Nightwatch* (deleted at M4a.7), with the single map's 30 m, the texels
  near the player would be about half as wide as its texels, not a quarter, by ADR-0064's study;
  that game's moon cast its shadows up to 16 m to make them finer. Fitting the cascades to what the camera sees at each frame,
  or virtual shadow maps, is left to Stage 9a.
- **Shadows shift, and thin objects close to a surface lose theirs (peter-panning)** (ADR-0050).
  The normal offset that keeps surfaces from shadowing themselves looks a point up 4.5 texels × the
  sine of the light's incidence off its triangle (ADR-0063, ADR-0065): at the default distance,
  under a light 45° from the normal, 1.2 cm in the first cascade and 18 cm in the last, and 26 cm
  there at grazing. A shadow shifts by up to that much, and a sheet lying closer than that above a
  floor casts nothing on it.
- **Every draw of the scene pass runs a shader that can discard fragments** (ADR-0035). The `MASK`
  alpha mode discards in the one fragment shader that every material uses, opaque materials with a
  cutoff of 0. Some GPUs then test depth after the shader rather than before, for opaque draws
  too, which costs overdraw. Not measured: compare the scene pass's GPU time on a scene with much
  overdraw against a build whose opaque materials use a fragment entry point without `discard`
  (six pipelines instead of three).
- **Edges that a masked material cuts inside a triangle stay aliased** (ADR-0059). Multisampling
  smooths the edges of triangles, but the scene shader runs once per pixel, and the `MASK` alpha
  mode keeps or discards all of a pixel's samples at once. Smoothing those edges, alpha to
  coverage, behaves differently from one driver to the next, which image tests would see. The leaf
  cards of *Hunna Wood*'s trees and its ferns are masked.
- **The sky's light is neither shadowed nor occluded** (ADR-0039). A surface under an object, or
  inside a box, receives the whole sky, but for its material's occlusion texture: no ambient
  occlusion (screen-space effects are outside the stage), no reflection probes, and mirrors reflect
  the sky, never the objects around them. An interior is lit by the whole sky through its walls:
  until then, draw it without an environment, or with a sky of lower radiances, and light it with
  point and spot lights. Khronos's Sponza shows it (M3b.9): its arcades are lit by the sky as its
  open courtyard is, in the same blue.
- **Glass is drawn opaque.** The importer leaves out glTF's `KHR_materials_transmission` and
  `KHR_materials_volume`, and warns that it does: the renderer neither lets light through a surface
  nor blends one (Khronos's FlightHelmet and ABeautifulGame, M3b.9). Transparency is left to
  Stage 9a (ADR-0052).
- **One GPU, an AMD, and two CPU drivers have run the engine.** The image tests' tolerances
  (ADR-0030), the measurements of this plan and the samples' looks come from an RX 6900 XT, with
  RADV on Linux and AMD's driver on Windows, from lavapipe and from WARP: NVIDIA's and Intel's GPUs
  are untried.
- **The environment is computed on the CPU, in every process that draws a sky** (ADR-0039): 23 ms
  in a release build on the owner's 16 threads, 162 ms on one core, and the BRDF table once (15
  ms); about eight times longer in a debug build, which every test that draws a sky with a gradient
  or a sun pays: the workspace's tests took 23.9 s instead of 9.7 s on lavapipe on the owner's
  machine, most in `slime-render`'s `tests/environment.rs` (6.2 s) and the examples'
  `tests/gltf.rs` (5.7 s), whose every load of a file computes the sample's sky. A sky that changes
  needs a new environment: no time of day. To measure: `cargo test --release -p slime-render --lib
  -- --ignored --nocapture`, against the budget of M3b.6. Not yet scheduled: to move to the
  cooker, or to compute passes if skies change at run time; if the tests' time matters first,
  building `slime-render` optimized in debug builds would shorten it, at the cost of stepping through it in
  a debugger.
- **The sky's sun is described twice in the renderer's own API** (ADR-0039). A game that draws
  with it gives the same `DirectionalLight` to `Sky::sun`, for the disc and the glow, and to
  `Scene::lights`, for its light and its shadows; two different lights show a disc where no light
  comes from. The `scene` example takes both from the file's first directional light. A world gives
  it once ([ADR-0084](adr/0084-what-the-world-draws.md)), as *Hunna Wood*'s does.
- **A node renamed in Blender loses what saves held of it** (ADR-0079, ADR-0087). A scene's entity
  is known by its node's name: loading a save made before the rename sets aside what it held of
  the node, and reports it. *Hunna Wood*'s zone script names every node and fails when Blender
  numbers one, so that only a change of the script renames a node; the saves made before it set
  aside what they held of it, or are saves of another layout. Identities that a scene file keeps for
  a former name wait for a game that has players (ADR-0089).
- **Some meshes of *Hunna Wood*'s zone are named with Blender's numbers** (`wooden_lantern_01.001`
  in the lantern, `capital.004` in the land): the script checks the names of its objects, not of
  their meshes. The cooker derives a mesh's ID from its name, so that a later change of the script
  that numbers a mesh otherwise changes the mesh's ID. Naming the meshes changes the export, and
  waits for the next release of the mirror.
- **The hero's export differs from one run of the zone's script to the next**, where the land's
  and the lantern's no longer do: the hero kept on the mirror is the one the game reads.
- **The camera's yaw is not saved** (ADR-0088). It is the game's, outside the world: a run resumed
  or loaded puts the camera behind the hero.
- **A save cut short may load in part** (ADR-0087). A save is a document of sections: one cut at the
  boundary of a section is still a document, and loads what it holds without an error. *Hunna Wood*
  writes its saves whole or not at all (ADR-0088), as every game that writes saves must.
- **A save is text, of limited size** (ADR-0087). Saving refuses a save larger than what
  `slime-serialize` reads, and loading costs more than saving, much of it parsing the text: the
  rustdoc of `World::save` and `World::load` has the figures. *Hunna Wood*'s saves are far below
  the limit. The binary encoding exists since M5b.4, but saves stay text until a measurement asks
  (ADR-0089): a larger world needs saves in the binary encoding, or read in parts.
- **Children and queries in another order after loading** (ADR-0087). A world loaded from a save
  holds its entities in other slots, and an entity's children in the order the save set them: what
  must go the same way after loading cannot depend on either order.
- **Spawning a large scene makes each component from its value** (ADR-0091, which holds the
  figures). A scene of 100 000 entities resolves and spawns in about a second in a release build,
  where spawning as many entities with `World::spawn` takes milliseconds (ADR-0083): each
  component is made from its value, and each entity's identity is a list of texts, held twice.
  *Hunna Wood*'s zone, its seven placements resolved, takes about 2 ms. ADR-0089 orders the
  remedies, inside the world: typed components copied rather than made from values, identities that
  share their parts, scenes prepared on another thread.
- **A world written back writes what it holds of what spawning set aside** (ADR-0092). A
  reference, or a parent, that a scene file wrote into a placement whose prefab no longer had its
  entity is written as the world holds it, no entity or the root: what it named is lost, as a save
  loses it. An entity of a placement moved out of the place that its prefab gives it is written in
  that place, and the children of a placement's entity in its prefab's order, then the file's; both
  are reported: a scene file has no parents for a placement's entities, nor an order among them.
  Stage 6 decides whether a file says them.
- **A scene file written back loses its comments** (ADR-0089): `World::write_scene` writes the
  canonical form. Stage 6 decides whether its editor keeps the comments of files written by hand.
- **The cooked glTF scene is a format apart from the binary encoding** (ADR-0089, ADR-0095). A
  glTF file's scenes are cooked into a format of their own, `slime_assets::CookedScene`, and only
  the typed properties of their nodes into the binary encoding: cooked scenes have two formats, and
  two readers. The renderer's scene loader, the `scene` example and the image tests read the format;
  moving it onto the binary encoding would make `slime-render` depend on serialization, and waits
  for a reason.
- **After a lost device, what is hidden loads only once shown** (ADR-0084): the cache loads what is
  drawn. In *Hunna Wood*, a key taken or a door opened before the loss shows a few frames after R
  shows them again.
- **A zone loaded again with `--watch` changes in parts.** Its entities change at once, the meshes,
  materials and textures that the cache holds when its reload ends, and new meshes once their batch
  is loaded; *Hunna Wood* swapped whole scenes before it ran on the world.
- **The tests that edit every leaf of every component are written twice** (ADR-0085): once in
  `slime-world` for its components, once in *Hunna Wood* for the game's, about fifty lines that
  choose another value for a leaf. One copy would need a test-support crate, or a feature of
  `slime-world` that only tests enable; neither is worth it yet.
- **An edit through reflection reads the whole component again** (ADR-0085): a field that its type
  skips takes its `Default` value, and a rotation is edited whole, as `Transform`'s check refuses a
  coordinate edited alone. The panel of M5a.6 edits a rotation whole.
- **Rough metals reflect the sky with multiple scattering compensated, the lights without**
  (ADR-0039, as the Khronos glTF Sample Viewer): a white metal of roughness 1 reflects all of a
  uniform sky, but its lights keep glTF's single-scattering BRDF, under which it would reflect
  31 % of that sky; rough metals are darker in the sun than in the shade, relatively.
- **A roughness between two prefiltered levels reads a blend of their lobes** (ADR-0039): the four
  levels are 0.25 apart, and a lobe in between is approximated by mixing the nearest two, the split
  sum's usual approximation; each level is within 2.2 % of its own lobe's average.
- **The image tests use up to 75 of the 131 pixels that RADV may have off by more than 8**
  (ADR-0030, ADR-0054): the glTF scene's front view 75, on the wall's mortar joints seen at a
  grazing angle (76 before its textures were cooked); `shapes` 28, on edges and the metal spheres'
  reflections of the sky (49 at M3b.6, before the bloom; 33 before the shadow map followed the
  camera and the output was dithered). A change that adds fine or reflective detail may exceed it;
  measure it again then.
- **Four passes per submission is a number measured on one machine** (ADR-0041). On the owner's
  RX 6900 XT (RADV), whose memory that the CPU can map is a 256 MiB window, one submission of a
  whole bloomed frame stalled in the kernel once the desktop's programs and a test program filled
  that window to 241 MiB (a mean 2.4 ms, 8 ms in one frame of ten, at 1920 × 1080); submissions of
  2 to 6 passes did not, 8 did. The cause in the kernel is not established; Direct3D 12 and other
  GPUs are not measured, and a GPU with resizable BAR has no such window. To measure: the CPU time
  of `render` at 1920 × 1080 with the sample's bloom, for several values of
  `PASSES_PER_SUBMISSION` in `crates/slime-render/src/graph.rs`, with the window filled to
  several levels, by the desktop's programs or by a program that allocates device-local,
  host-visible memory (on Linux, `mem_info_vis_vram_used` in the GPU's sysfs directory tells how
  full it is).
- **The render graph trusts what a pass says it samples from imported targets** (ADR-0041). The
  groups that sample transient targets are built from the declarations, but the scene pass's group
  holding the shadow atlases is built by the renderer: a pass that binds an imported target must
  declare it, or the graph orders it without knowing.
- **A renderer whose frame changes shape at every call reports no tone-mapping or bloom time**
  (ADR-0041). Each change of shape (the output's size, the shadow pass or the bloom starting or
  stopping) builds the frame again with new timers, and the last submission's times come back
  during the next call, by then read from newer timers; such a renderer also creates its targets
  again at every call, as it did before. Use one renderer per output size.
- **Tiny, very bright sources blink in the bloom** (ADR-0040). Without a firefly filter (Karis's
  average dims the sun's halo and makes it flicker, measured), a highlight smaller than a pixel
  blooms as its coverage of the pixel changes from frame to frame. Temporal anti-aliasing, or a cap
  on the light a pixel brings to the chain, belong to Stage 9a (ADR-0052).
- **A pixel whose light is not a number spreads through the bloom** (ADR-0040). The renderer
  refuses values that are not finite in meshes, materials, lights (ADR-0042) and skies, but not
  everything that reaches the shaders is checked: a normal of zero length, which `MeshData` asks
  callers not to give, can make its pixels NaN in the high-dynamic-range target, and the bloom
  averages them into its levels, and from there into every pixel within reach of their halo, a blot
  where one surface was wrong. Dropping such light before the bloom would keep it to its pixels.
- **The bloom has one shape** (ADR-0040): every level the same weight, down to a side of 8 texels;
  no radius, threshold or tint for a game to set. A window resized across a power of two adds or
  removes a level: the glow's reach changes a little while the edge is dragged.
- **Light near the right and bottom edges blooms a little more or less** (ADR-0040). Where an
  output's side is not a multiple of 2^levels, the coarse levels' last texels straddle the edge: a
  pixel's halo there holds 0.83 to 1.37 of its light at ten sizes measured, up to 1.9 at the far
  corner (1023 × 1023, 2047 × 1151), within 1.011 at 1280 × 720 and about 1.1 at 1920 × 1080; inside
  the image and at the top and left edges, its light exactly. To measure: the CPU model of
  `tests/bloom.rs` at the size in question. A fix would pad the chain beyond those edges.
- **glTF minification filters are approximated.** `slime-gfx` ties mip blending to the
  minification filter and gives every texture mipmaps, so only `NEAREST_MIPMAP_NEAREST` and
  `LINEAR_MIPMAP_LINEAR` are exact (documented in the importer, `crates/slime-cook`). Revisit when
  an engine-side user needs a separate mipmap filter or a base-level-only read.
## Notes

- CI build cache: not added yet. With wgpu, the Linux job takes about 2.5 minutes and the Windows
  job about 5 (measured on the runs of 2026-10-12; it compiles the dependencies three times: for
  clippy, for the tests, and in release for the artifact), which is acceptable; a cache is to add if
  it grows much further. The dependencies of Stage 3a (glam, gltf, egui) were expected to add to
  it, and each milestone that added one recorded the new duration here. With glam and the render
  crate (M3.2, 2026-10-13): Linux 2 minutes 49 seconds, Windows 4 minutes 59 seconds. With gltf and
  egui (M3.6, 2026-10-13): Linux 3 minutes 22 seconds, Windows 5 minutes 42 seconds, and 6 minutes
  2 seconds once its tests ran one at a time. At the release (M3.7, 2026-10-13), over the ten
  green runs before it: Linux from 2 minutes 35 seconds to 3 minutes 38 seconds, Windows from
  3 minutes 12 seconds to 7 minutes 10 seconds; the Windows runner's speed varies from one run to
  the next more than the code changes between them. At 0.1 (M3b.9, 2026-10-14): Linux 5 minutes 40
  seconds, Windows 11 minutes 32 seconds, of which the tests take 4 minutes 31 seconds and the
  release build of the examples 3 minutes 24 seconds. The owner decided then to keep CI as it is,
  the repository being public and its minutes free, and to reconsider past 20 minutes. With
  `slime-world` (M5a.1, 2026-10-16): Linux 10 minutes 25 seconds, Windows 17 minutes 20 seconds.
  With `slime-color` and `slime-world-render` (M5a.3, 2026-10-16): Linux 12 minutes 33 seconds,
  Windows 17 minutes 26 seconds. With `slime-reflect` and `slime-reflect-derive` (M5a.5,
  2026-10-16): Linux 9 minutes 6 seconds, Windows 18 minutes 27 seconds. With `slime-serialize`
  (M5a.7, 2026-10-16): Linux 13 minutes 26 seconds, Windows 15 minutes 31 seconds.
