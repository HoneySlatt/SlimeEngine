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

- Stage 4a — Cooked assets: done and released as 0.2 on 2026-10-15. *Hunna Wood* loads its zone
  from cooked assets and is played by day and by night, on Linux and on Windows.
- Stage 4b — Live assets: done on 2026-10-16, and released with 0.3 at the end of Stage 5a
  ([ADR-0078](adr/0078-stage-5-in-two-halves.md)). Scenes load in the background while a game
  draws, a game goes on after a lost GPU device, a material's values change while it runs, and a
  texture or a shader changed on disk changes in the running game. The owner checked the recovery
  by hand, in the Windows virtual machine and after a real GPU reset in KDE Plasma.
- Stage 5a — The world: planned on 2026-10-16 and validated by the owner
  ([ADR-0078](adr/0078-stage-5-in-two-halves.md) to [ADR-0081](adr/0081-stage-5a-the-world.md));
  the stage being worked on. M5a.1 done on 2026-10-16: a world of entities and components, in
  `slime-world` ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)). M5a.2 done on
  2026-10-16: the hierarchy, transforms in double precision, and cooked scenes spawned as entities
  ([ADR-0083](adr/0083-transforms-and-the-hierarchy.md)). M5a.3 done on 2026-10-16: the world drawn
  from an asset cache through `slime-world-render`, what moves drawn between fixed steps
  ([ADR-0084](adr/0084-what-the-world-draws.md)). M5a.4 done on 2026-10-16: *Hunna Wood* runs on the
  world, which holds its run. M5a.5 built: reflection, through which the world reaches every
  component by name ([ADR-0085](adr/0085-reflection.md)); CI checks it.

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M5a.1](#m5a1--the-world-entities-and-components) | Entities and their components live in a world, without a GPU or a window | Done |
| [M5a.2](#m5a2--the-hierarchy-and-the-transforms) | Entities are placed by a hierarchy of transforms in double precision; a cooked scene becomes entities | Done |
| [M5a.3](#m5a3--what-the-world-draws) | The renderer draws a world, and what moves between its last two fixed steps | Done |
| [M5a.4](#m5a4--hunna-wood-on-the-world) | *Hunna Wood* runs on the world, and changes only what moves | Done |
| [M5a.5](#m5a5--reflection) | The engine and the game describe, read and change their types' values through one registry | Built; CI checks it |
| [M5a.6](#m5a6--any-component-listed-and-edited) | Any component's properties are listed and edited in the running game | Planned |
| [M5a.7](#m5a7--serialization-with-format-versioning) | Any reflected value is written to a file and read back, even from an older version of its type | Planned |
| [M5a.8](#m5a8--the-world-round-trips) | A world saves what differs from the scenes it was made from, and loads it back | Planned |
| [M5a.9](#m5a9--saving-and-resuming-a-run) | A player saves a run of *Hunna Wood* and resumes it | Planned |
| [M5a.10](#m5a10--windows-check-documentation-release-03) | Stage 5a is done and released as 0.3, with Stage 4b | Planned |

Stages 0, 1, 2, 3a, 3b, 3c, 3d, 4a and 4b are done; see [Completed stages](#completed-stages).

## Stage 5a — The world

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-5a--the-world--03)): *Hunna Wood* runs on the
world, which keeps what is drawn and where what moves was; any component's properties can be
listed and edited generically; a run of the game round-trips through save and load.

This is the first half of Stage 5 ([ADR-0078](adr/0078-stage-5-in-two-halves.md)): the world at run
time. Scenes written as files, and prefabs, are Stage 5b's, planned with the owner at the end of
this one. Where the engine stands at the start:

- **No world.** A game keeps its own objects and gives the renderer, at every frame, a list of
  instances that borrow their GPU objects. *Hunna Wood* keeps its hero's previous position to draw
  it between fixed steps ([ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md)),
  rebuilds about 430 instances at every frame, and leaves out the key's and the door's parts by the
  indices of their cooked nodes.
- **No reflection and no serialization.** No crate describes its types at run time. The cooker reads
  and writes metadata files with code of its own
  ([ADR-0069](adr/0069-the-asset-pipeline.md)), and the cooked formats are binary formats written
  by hand.
- **Positions are `f32`** in the renderer's API, which draws them relative to the camera
  ([ADR-0048](adr/0048-rendering-relative-to-the-camera.md)).

Out of scope: scene files and prefabs (Stage 5b); the editor (Stage 6); systems and a schedule, the
job system, physics, audio, input action mapping and the UI of games (Stage 7); scripts, plugins
and the C ABI (Stage 8); streaming and networking (Stage 10); migration tooling (Stage 11); handles
that count references, and progressive loading; import settings in metadata files; moving the
cooked formats onto the new serialization. [ADR-0081](adr/0081-stage-5a-the-world.md) lists the
rest.

Decisions taken when the stage was planned, by the owner on 2026-10-16, with the alternatives they
rejected, are in [ADR-0079](adr/0079-the-engines-own-world.md) (the world),
[ADR-0080](adr/0080-reflection-and-serialization.md) (reflection and serialization) and ADR-0081
(the rest):

- **The world**: an entity model of the engine's own, in `slime-world`, which depends on no GPU and
  no window; components stored by type under component IDs of the world's own; the hierarchy in the
  world; no systems or schedule; `slime-app` owning the game's world.
- **Positions and transforms**: `f64` translations, `f32` for the rest; a uniform scale that
  children inherit, and a stretch that they do not.
- **Drawing**: what moved during a fixed step drawn between its last two transforms; components that
  name assets by ID; GPU objects in a cache outside the world; a bridge that builds each frame from
  the world; a renderer that does not depend on the world.
- **Identities and saves**: an ID local to each file, and a persistent identity that is the path of
  those IDs through the scenes that placed an entity; a save holds what differs from the scenes.
- **Reflection**: the engine's own, with a derive written with `syn`, `quote` and `proc-macro2`;
  stable names that the derive requires, and aliases; one value type; explicit registration.
- **Serialization**: driven by reflection, without serde; TOML, read with `toml_edit` and written by
  the engine's own canonical writer; versions per type, and upgrades on plain values; a binary
  encoding at Stage 5b.
- **The rest**: the risk first; a panel of the dev overlay on F2; F5 and F9, a save on quit and a
  resume on start; a `world` example; Stage 4b open until the owner's checks, which closed it on 2026-10-16; no
  dependency but
  the two the owner approved, with their helper crates.

Risks:

- **How the world fits.** How a world of the engine's own fits the renderer, the fixed step,
  recovery from a lost device and hot reload is the largest unknown: M5a.1 to M5a.4 come first, and
  the game's tests are the reference.
- **The cost of the world's storage.** Simple storages join components by lookups: M5a.1 measures
  100 000 entities, and M5a.3 the frame built from the zone's world, against the game's frame of
  0.2.
- **The renderer's API in `f64`.** Every game, example and test that places a camera, a light or an
  instance changes; the precision test checks the result bit for bit far from the origin.
- **The derive.** Generic types, enums and error messages can make it grow: its scope is bounded to
  what Stage 5a's components use, and its misuses are tested to fail to compile.
- **Saves across versions.** A test type upgraded from one version to the next (M5a.7), and a save
  committed as a fixture (M5a.9), keep later changes honest.
- **Identities from Blender's names.** A name changed in Blender changes an identity: the zone's
  script names the objects the game refers to, and loading reports what it could not apply.
- **The cost of the checks.** The Windows CI job takes 15 to 18 of the 20 minutes that the owner
  allows; each milestone that adds a crate or a dependency measures it (ADR-0081).

### M5a.1 — The world: entities and components

- **Goal.** Entities and their components live in a world, without a GPU or a window.
- **Crates.** `slime-world` (new).
- **Delivers.** The crate and its README; entities as handles of an index and a generation, the
  handle of a destroyed entity refused and its slot reused with another generation, a slot whose
  generation would wrap retired; components of any type that threads can share, added, read,
  changed and removed, stored by type under component IDs of the world's own; queries over several
  component types, that read or change them; several worlds at once, which threads can share; the
  cost of spawning, querying and changing 100 000 entities, measured in a release build by an
  ignored test; the ADR on the world's storage and handles; `docs/ARCHITECTURE.md` (crates table,
  dependency graph), the glossary; the CI jobs' durations in the [Notes](#notes).
- **Done when.** Tests without a GPU: a destroyed entity's handle refused and its slot reused;
  components added, read, changed and removed; a query visits exactly the entities that hold what
  it asks for, and changing through it changes the world; a world sent to another thread; the
  measurement recorded; CI is green.
- **How to test.** `cargo test -p slime-world`.
- **Outcome.** Done on 2026-10-16 ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)). Components
  are stored by type in sparse sets, in safe code: `World::query` reads through an iterator, and
  `World::query_mut` changes through a closure, each storage borrowed apart. With 100 000 entities
  in a release build, spawning them takes 5.3 ms, reading every position 68 µs, moving the 50 000
  that hold a velocity 190 µs, and destroying them 0.85 ms; the ADR holds the figures. The review
  made two changes beyond the plan: a type becomes a component by saying so, `impl Component for
  Position {}`, so that a tuple given to `insert` by mistake does not compile; and destroying an
  entity drops its components before it frees its slot, so that a component that panics as it is
  dropped leaves nothing to the next entity of the slot. CI took 10 minutes 25 seconds on Linux and
  17 minutes 20 seconds on Windows ([Notes](#notes)).

### M5a.2 — The hierarchy and the transforms

- **Goal.** Entities are placed by a hierarchy of transforms in double precision, and a cooked
  scene becomes entities.
- **Crates.** `slime-world`; `games/hunnawood` for its ignored tests.
- **Delivers.** Parents and ordered children, reparenting that refuses a cycle, an entity destroyed
  with its subtree; the local transform, an `f64` translation, a rotation, a uniform scale that
  children inherit and a stretch that they do not, and the transform in the world, computed again
  only for the subtrees that changed and current whenever it is read; a cooked scene spawned as
  entities under a key, one per node, with its name, transform, parent, meshes, light and extras,
  its scales split as ADR-0079 says; the persistent identity of these entities, from the key and
  the node's name, or its index with a warning; the ADR on transforms, the hierarchy and precision;
  `docs/ARCHITECTURE.md`'s new flow, the glossary.
- **Done when.** Tests: reparenting, and a cycle refused; a subtree destroyed with its root; the
  transforms in the world of the examples' cooked scene, and of the zone's 543 nodes in the ignored
  tests, equal to `CookedScene::world_transforms` within `f32`'s rounding; a child 1 mm from a
  parent 10 000 km from the origin stays 1 mm from it; only the subtrees that changed computed
  again; scales split as the rule says; the persistent identities of a cooked scene's entities kept
  when its nodes are reordered; CI is green.
- **How to test.** `cargo test -p slime-world`; `cargo test -p hunnawood -- --ignored`.
- **Outcome.** Done on 2026-10-16 ([ADR-0083](adr/0083-transforms-and-the-hierarchy.md)). Every
  way of changing a `Transform` marks the entity's subtree stale; `World::update_transforms`
  computes each stale subtree once, and `World::global_transform` is current between updates,
  computed from the nearest current ancestor. With 1 000 roots of 99 children each in a release
  build, computing the 100 000 transforms takes 3.4 ms, and computing again the 1 000 whose root
  moved 32 µs. `World::spawn_scene` places the zone's 543 nodes where its file puts them, each known
  by its name. The world splits a node's matrix itself, so that one that flattens an axis or shears
  keeps a unit rotation: `scripts/check.sh` runs the world's tests with glam's assertions on, as the
  renderer's. Each slot's place in the hierarchy makes spawning 100 000 entities take 10.5 ms rather
  than 5.3 ms; the ADR holds the figures. The review found three faults, fixed and tested before the
  milestone was pushed: an entity already stale that changed parents was never computed again; a
  node named like an index, `#2`, could share another node's identity; and a component that panicked
  as it was dropped could leave a destroyed child in its parent's list. CI took 12 minutes on Linux
  and 17 minutes 27 seconds on Windows.

### M5a.3 — What the world draws

- **Goal.** The renderer draws a world, and draws what moves between its last two fixed steps.
- **Crates.** `slime-world`, `slime-app`, `slime-render`, `slime-gfx` (the color type), a crate
  between the world and the renderer, `slime-examples`.
- **Delivers.**
  - Components for what is drawn, without GPU objects: meshes and materials named by asset ID,
    whether they cast shadows, hidden with their descendants; and for lights. The color type moved
    below `slime-gfx`.
  - The cache of the GPU objects of the assets that the world names, outside the world: loaded in
    the background on the renderer's pool, shared by the scenes that use them, made again after a
    lost device, loaded again on hot reload, where ADR-0076's rule moves.
  - At each frame, the instances and lights built from the world, the renderer keeping no world;
    `f64` positions in the renderer's API: the camera, the lights, the instances.
  - `slime-app` owning the game's world in its `Context` and calling it around each fixed step:
    what moved during a step is drawn between its last two transforms at `fixed_step_alpha`, a
    change made outside a step at once, and a teleport inside a step as the game marks it.
  - The sky's sun given once, to the sky and to the lights (see [Known debt](#known-debt)).
  - The `world` example: the examples' cooked scene in a world, and something that moves at the
    fixed step, with `--headless`, `--frames` and `--watch`, which `scripts/check.sh` runs and CI
    publishes with the Windows programs.
  - The CPU cost of a frame built from the zone's world, in a release build.
  - The ADR on what the world draws; `docs/ARCHITECTURE.md`'s flows (a frame of the world, losing
    the GPU), `examples/README.md`, the glossary.
- **Done when.** A world made from the examples' cooked scene draws the instances that
  `LoadedScene::instances` gave, and the glTF image test's pixels within its tolerance
  ([ADR-0054](adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md)); something moved at
  fixed steps is drawn where `fixed_step_alpha` puts it, at 0, 0.25 and 1, frames without a step
  and with two included; a teleport does not slide; a hidden subtree and its lights are not drawn;
  after `Gpu::lose`, the cache made again draws the same world, which the loss left unchanged; the
  precision test, with `f64` positions, draws a scene 10 000 km from the origin bit for bit as at
  it; `slime-app`'s lifecycle test checks the order of the world's calls around `fixed_update` and
  `update`; the example runs headless and in the headless Weston; CI is green.
- **How to test.** `cargo test -p slime-world -p slime-render -p slime-app`;
  `cargo run --example world`.
- **Outcome.** Done on 2026-10-16 ([ADR-0084](adr/0084-what-the-world-draws.md)). The color type
  moved into `slime-color`, with the sRGB transfer function that the GPU layer and the cooker each
  copied. The renderer's positions are `f64`s: its precision test draws a scene 10 000 km from the
  origin bit for bit as at it. `slime_render::AssetCache` holds the meshes a game draws by asset ID,
  with their materials and textures, loading what it is asked for in batches that arrive whole,
  reloading what changed and dropping everything after a lost device; scene loads and the cache
  share one loader. The world's components say what is drawn, `Meshes`, `Hidden` and its own
  `Light`, a directional one saying whether it shows in the sky; the world keeps the transforms
  before the last fixed step of what moved during it, and `slime-app` owns it and calls it around
  each step. `slime-world-render` builds each frame from the world and the cache, the sun given
  once. The materials are those that each cooked mesh names: no component names another yet. Built
  from *Hunna Wood*'s zone in a release build, a frame of 429 instances takes 74 µs on the CPU.
  The review found that an entity placed for the first time during a step slid in from its
  parent, that a sun whose shadow distance changed made the sky again, and two behaviours that the
  owner decided on: what a moving entity holds stays on it, though changed outside a step or placed
  during one, and the sun comes first among the lights, so that it casts the shadows. The
  lifecycle test sees the world's calls around `fixed_update`; the one after `update`, which only
  makes reading the transforms a lookup, shows in no behaviour that a test can see. CI's Linux job,
  which has no GPU, failed on that measurement until `scripts/check.sh` ran the game's ignored
  tests on lavapipe, as the others. CI then took 12 minutes 33 seconds on Linux and 17 minutes 26
  seconds on Windows ([Notes](#notes)).

### M5a.4 — Hunna Wood on the world

- **Goal.** *Hunna Wood* runs on the world, and changes only what moves.
- **Crates.** `games/hunnawood`; engine crates for what the game finds missing.
- **Delivers.** The zone and the hero as entities: meshes, the lights of day and of night, the
  colliders and markers as the game's components, read from the nodes' extras as before; the key's
  and the door's parts hidden once taken or open; the rules as functions over the world at the
  fixed step; the fire's flicker and the key's pulse on the world's light and materials; ADR-0057's
  workarounds removed but the keys held: the hero's previous position, the instances rebuilt at
  every frame, the nodes left out by index; after a lost device, the world kept and only the cache
  made again; with `--watch`, a change of layout starting the run again and a change of looks not;
  the frame's CPU time compared with 0.2's on the RX 6900 XT; the game's README; `CHANGELOG.md`.
- **Done when.** The game's tests, moved onto the world, pass: the walk past the campfire, the key,
  the door and onto the terrace; the door that stops the hero without the key; the hero kept out of
  colliders; the restart; the hero waiting for the zone; a change of layout against one of looks.
  `scripts/check.sh` runs the game by day, at night and losing its device; the owner plays it on
  Linux and finds it unchanged; CI is green.
- **How to test.** `cargo test -p hunnawood`, and with `-- --ignored`; `cargo run -p hunnawood`,
  with `--night`, `--lose-device-at 300` and `--watch`.
- **Outcome.** Done on 2026-10-16. The zone and the hero are spawned in the game's world before
  the window opens, and the game gives the zone's entities components of its own, read from their
  extras: `Collider`, `Marker`, `TimeOfDay`, and `Flickers` for the campfire's light. The run is in
  the world: the hero is where the root of its scene stands, which the world draws between fixed
  steps; the key taken and the door open are hidden, which takes the door's colliders away too
  (`World::is_hidden`, moved into the world from `slime-world-render`); the night is a component of
  the zone's root, the lights of the other time of day hidden. The rules are functions over the
  world, without the GPU; their tests build cooked scenes and play them in worlds of their own. The
  view draws through an `AssetCache` and a `FrameBuilder` for each time of day. As ADR-0074 decided,
  the hero's meshes load before the first frame, while the zone starts loading and both skies are
  computed, the other time's from a world of its lights alone: the game asks the cache for the
  hero's meshes itself, the first frame's, which ADR-0084's frame builder would ask for only then.
  The materials that glow are found by name in the cooked data. After a lost device, the view is made again and the world kept. With `--watch`,
  the cache loads again what changed, and the zone or the hero is spawned again when its scene
  changed: a new layout starts the run again, new looks do not, and a zone that cannot be played
  changes nothing. The review found that a mesh removed from the zone made every later reload
  fail, since the cache kept loading it again: `AssetCache::reload` drops the meshes that the new
  data no longer lists. The previous position, the instances rebuilt by hand and the nodes left out by
  index are gone, `LoadedScene::instance_nodes` with them; the keys held and the game's own
  collisions stay (ADR-0057). In the headless Weston, the game draws as many instances, shadows and
  triangles as before, by day and at night; `--watch` was tried on the RX 6900 XT with a change of
  looks, a change of layout, a changed hero and a zone without a start. On that GPU, in a release
  build, the zone is drawn 3 or 4 frames after the first, the longest interval between two frames
  17 to 27 ms in nine runs, below ADR-0072's 33 ms (ADR-0074 measured 8 to 22 ms); by day at the
  start, the game's work takes 1.66 ms per frame at the median of four runs, against 1.55 ms for
  0.2 on the same zone: about 0.1 ms more, most of it the frame built from the world (M5a.3). The
  owner played it on Linux and found it unchanged. CI took 12 minutes 42 seconds on Linux and 18
  minutes 1 second on Windows.

### M5a.5 — Reflection

- **Goal.** The engine and the game describe, read and change their types' values through one
  registry.
- **Crates.** `slime-reflect` and `slime-reflect-derive` (new); `slime-world`, `slime-assets`, the
  color type's crate, `games/hunnawood`.
- **Delivers.** The `Reflect` trait and its derive, for structs and enums; the stable name that the
  derive requires, aliases, and ADR-0080's attributes (documentation, version, default, skip,
  transient, range, unit); one value type, in which references to assets and to entities are kinds
  of their own; property paths, read and set; values compared, and their differences applied, path
  by path; the registry, explicit and recursive, which can hold a type with no Rust type behind it;
  reflection for the standard types the components use, glam's types, the color type, `AssetId`
  and references to entities; every component of the world and of the game registered, and
  reached through the world without knowing its type; the derive's misuses as `compile_fail` doc
  tests; the ADR on reflection; two READMEs, `docs/ARCHITECTURE.md`, the glossary,
  `docs/dependencies.md` (`syn`, `quote`, `proc-macro2`); the build's cost, and the game's rebuild
  after a one-line change, before and after.
- **Done when.** Every registered type lists its properties, with their names, kinds and
  attributes; every leaf of every component set through its path reads back the new value, through
  reflection and typed; a path that names nothing, a value out of range or of the wrong kind
  refused, the path in the message; a component with only the derive passes the same tests; values
  round-trip through the value type; applying a value's differences from another gives it back;
  aliases read former names; misuses fail to compile; CI is green.
- **How to test.** `cargo test -p slime-reflect -p slime-world -p hunnawood`.
- **Outcome.** Built on 2026-10-16 ([ADR-0085](adr/0085-reflection.md)). `slime-reflect` and its
  derive describe a type's values under a stable name, turn them into plain values and back, check
  them, set their parts by property path within their fields' ranges, list their leaves, compare
  them and apply their differences; a registry holds types with no Rust type behind them too. The
  owner decided that every component is reflected, `Component` requiring `Reflect`, and that each
  world keeps its registry, a component's type registered when the world first stores one. A world
  lists its entities and the components each holds, and reads and sets any of them by name and
  path; an edit takes the component's value, sets the part, and reads the component back. The
  engine's six components, *Hunna Wood*'s five, the color type, asset IDs, the custom values of
  nodes and entity handles are reflected, the lights' ranges and units those their rustdoc gave.
  Tests set every leaf of every component through its path and read it back, through reflection
  and typed, a component that only derives among them; the derive's misuses are doc tests that
  fail to compile, each for the reason it names. The review found that edits valid field by field
  could give the renderer a light it refuses, which panics: the owner decided that a type declares
  a check of the rules between its fields, `#[reflect(check)]`, which `Light` and `Transform` do,
  and that ranges hold finite numbers only. It also made paths go through options, refused fields
  given twice and descriptions that are not ones, and removed what nothing called. The two new
  crates take about 7 s of compile time
  in a cold build of the workspace, beside the rest; a cold build took 60 to 88 s from one run to
  the next on the owner's machine, before and after alike. The game's rebuild after a one-line
  change takes 2.12 s instead of 2.04 at the median of ten debug builds, and after a one-line
  change in `slime-world` 2.58 s instead of 2.45.

### M5a.6 — Any component listed and edited

- **Goal.** Any component's properties are listed and edited generically, in the running game.
- **Crates.** `slime-devui`, `games/hunnawood`, `slime-examples`.
- **Delivers.** A panel of the dev overlay, on F2: the world's entities by name, with a filter; the
  components of the one selected and their properties, through the registry; numbers, booleans,
  vectors, colors, enum variants and text edited, values out of range refused; references to
  entities and assets shown; each edit returned as an entity, a path and a value, which the caller
  applies; in the game and in the `world` example; its limits in `slime-devui`'s README: it does
  not undo, add, remove or save, and is removed once Stage 6's inspector does at least as well
  (ADR-0081).
- **Done when.** The panel is built, in an egui context without a window, for every registered
  component type; by hand, the owner moves the key, dims a lantern and opens the door from the
  panel; CI is green.
- **How to test.** `cargo run -p hunnawood`, then F2; `cargo run --example world`.

### M5a.7 — Serialization with format versioning

- **Goal.** Any reflected value is written to a file and read back, even from an older version of
  its type.
- **Crates.** `slime-reflect`, or a crate of its own if the milestone's ADR decides so;
  `slime-cook`.
- **Delivers.** TOML read with `toml_edit` and written by the engine's own canonical writer: the
  same value, the same bytes, with LF line endings, on Linux and on Windows, `.gitattributes`
  keeping `*.toml` files LF; the conventions for enum variants, absent values and large integers;
  the version of each type written once per file, aliases, upgrades from one version to the next
  on plain values that accept partial ones, sources read strictly and saves leniently, a newer
  version refused; errors with the file, the line, the column and the path, never a panic; limits
  on the size and depth of what is read; the metadata files read through the same reader, every
  existing one unchanged, the cooker's own reader removed; the ADR on serialization and versioning;
  READMEs, `docs/ARCHITECTURE.md`, the glossary, `docs/dependencies.md` (`toml_edit` and its
  crates).
- **Done when.** Every registered type round-trips: value to text to an equal value, and text to
  value to the same bytes; floats come back to the same bits, special values included; a test type
  written at version 1 is read at version 2 through its upgrade, from a whole value and from a
  partial one; aliases read; a newer version refused; truncated, altered, too deep or too large
  documents give errors, never panics; every committed metadata file gives the same ID through both
  readers before the old one goes; CI is green on both platforms.
- **How to test.** `cargo test -p slime-reflect -p slime-cook`.

### M5a.8 — The world round-trips

- **Goal.** A world saves what differs from the scenes it was made from, and loads it back.
- **Crates.** `slime-world`, `slime-examples`.
- **Delivers.** Persistent identities: entities spawned from a scene under a key, and those created
  while the world runs, under keys from a counter that saves keep; a save as a document of the
  differences from the spawned scenes: values changed, by entity, component and path; components
  added and removed; entities removed; entities created while it ran, whole; transient data left
  out; loading that spawns the scenes from their current content, then applies the differences,
  keeps aside and reports those whose target is gone, and reports a scene changed since the save;
  references between entities written as persistent identities; the format's version and each
  scene's identity in the document; the `world` example saving and loading its world, `--headless`
  comparing the two; the ADR on the world's documents and stable references;
  `docs/ARCHITECTURE.md`'s flow, the glossary.
- **Done when.** The cooked zone and hero, changed (an entity removed, values changed, a component
  added), saved and loaded into a new world, equal the changed world through reflection, entity by
  entity, component by component and property by property, floats by their bits; saving again
  gives the same bytes; the same input over 300 fixed steps gives both worlds the same state; a save
  loaded on a scene that changed since (an entity moved, one removed, one renamed, one added) moves
  and adds what it should, and reports the removed and the renamed ones; two entities with one
  identity refused; malformed and newer documents refused without a panic; the example's round trip
  runs in `scripts/check.sh`; CI is green.
- **How to test.** `cargo test -p slime-world`; `cargo test -p hunnawood -- --ignored`;
  `cargo run --example world -- --headless`.

### M5a.9 — Saving and resuming a run

- **Goal.** A player saves a run of *Hunna Wood* and resumes it.
- **Crates.** `games/hunnawood`; `slime-world` for what M5a.8 lacks.
- **Delivers.** F5 saves and F9 loads, the hero drawn at once where the save puts it; a save on quit
  and a resume on start; `--new` and `--save <file>`; the save file in the user's data directory
  (ADR-0081); an unreadable save logged and set aside, and a new run started; a save made on
  another layout of the zone starting a new run; the checks never touching the user's save; a save
  committed as a fixture, which every later version must load; what was saved or loaded in the log;
  the game's README; the known debt on Windows ending a session updated.
- **Done when.** After the scripted walk at night, a save loaded into a new game gives the same run,
  and the walk goes on to the terrace; the fixture loads; corrupt and newer saves refused without a
  panic; the save directory computed from the environments a test gives; `scripts/check.sh` resumes
  the game in the headless Weston from a copy of the fixture; the owner saves, quits, resumes and
  loads on Linux; CI is green.
- **How to test.** `cargo run -p hunnawood`: F5, F9, Escape, then start it again;
  `cargo test -p hunnawood -- --ignored`.

### M5a.10 — Windows check, documentation, release 0.3

- **Goal.** Stage 5a is done and released as 0.3, with Stage 4b.
- **Crates.** All, for their documents.
- **Delivers.** The documents reread as a newcomer would; `CHANGELOG.md`'s 0.3 section; the owner
  plays the game and the `world` example from CI's Windows programs, and saves and resumes there;
  Stage 5a marked done; Stage 5b planned with the owner; the release, as [Releasing](#releasing) says,
  `world.exe` among the programs.
- **Done when.** The exit criterion holds, on Linux and on Windows; the tag `v0.3.0` is on a commit
  whose CI is green, and its GitHub release carries the Windows programs.
- **How to test.** Reading; CI; playing it.

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
   that `scene.exe` embeds, which the Ubuntu Font Licence asks to travel with them: CI keeps its
   artifacts 14 days, a release keeps them.

0.0.1 was released on 2026-10-12, before Windows joined CI: the tag `v0.0.1` is on the commit
"Release 0.0.1". 0.0.2 was released the same way on 2026-10-12: the tag `v0.0.2` is on the
commit "Release 0.0.2", and 0.0.3 on 2026-10-13: the tag `v0.0.3` is on the commit
"Release 0.0.3". 0.1 was released on 2026-10-14: the tag `v0.1.0` is on the commit "Release 0.1.0",
and its GitHub release carries the Windows examples that CI built from that commit.
0.2 was released on 2026-10-15: the tag `v0.2.0` is on the commit "Release 0.2.0", and its GitHub
release carries the Windows programs that CI built from that commit, *Hunna Wood* with its zone
cooked and the notice that credits its assets among them.

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3a — First renderer → 0.0.3 | M3.1 GPU resources for a renderer; M3.2 the render crate; M3.3 directional light and material; M3.4 textures and samplers; M3.5 a glTF scene and camera control; M3.6 statistics, GPU timings and the dev overlay; M3.7 Windows check, documentation, release | Done; released as 0.0.3 (tag `v0.0.3`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3b — Physically based renderer → 0.1 | M3b.1 GPU layer for multi-pass rendering; M3b.2 HDR rendering and tone mapping; M3b.3 physically based materials; M3b.4 light types; M3b.5 shadows; M3b.6 image-based lighting; M3b.7 bloom; M3b.8 render graph; a pass over the known debt; M3b.9 Windows check, documentation, release | Done; released as 0.1 (tag `v0.1.0`), with the Windows examples on its GitHub release. Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3c — A first game | M3c.1 skeleton; M3c.2 guards, detection and the objective; M3c.3 night, feel and the Windows check; M3c.4 findings for the next stages | Done; no release of its own: its game, *Nightwatch*, is never released, as *Hunna Wood* replaces it in Stage 4a ([ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)). The stage each of its findings goes to is in [ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md), and the two that became Stage 3d in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md). |
| 3d — Anti-aliasing and the shadows of spot lights | M3d.1 multisampling in the GPU layer; M3d.2 anti-aliasing in the renderer; M3d.3 spot lights' shadows: what each tile sees; M3d.4 spot lights' shadows: drawn and read; M3d.5 *Nightwatch* and the Windows check; M3d.6 a smoother shadow filter; M3d.7 what each cascade sees; M3d.8 the cascades read; M3d.9 *Nightwatch* at 3840 × 2160 | Done; no release of its own: what it delivers is released with 0.2. Its decisions are in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) and ADR-0058 to [ADR-0065](adr/0065-reading-the-cascades.md); the cost of its scene pass on Direct3D 12, with each shader compiler, is in the [Known debt](#known-debt). |
| 4a — Cooked assets → 0.2 | M4a.1 *Hunna Wood*'s skeleton; M4a.2 asset identity; M4a.3 images; M4a.4 glTF; M4a.5 the sample scene through the pipeline; M4a.6 the zone's sources; M4a.7 *Hunna Wood* by day; M4a.8 night; M4a.9 Windows check, documentation, release 0.2 | Done, released as 0.2 on 2026-10-15. Its decisions are in [ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md) to [ADR-0071](adr/0071-cooked-textures.md); the zone's budgets are set by the test that checks them, in `games/hunnawood/src/zone.rs`, and what was left to refine went to Stage 4b's M4b.8. |
| 4b — Live assets | M4b.1 a shared GPU and a task pool; M4b.2 the zone loads while frames go on; M4b.3 recovering from a lost GPU device; M4b.4 *Hunna Wood* goes on after a lost device; M4b.5 materials that change in place; M4b.6 hot reload of cooked data; M4b.7 hot reload of shaders; M4b.8 refining *Hunna Wood*; M4b.9 Windows check, documentation, close | Done on 2026-10-16; no release of its own: what it delivers is released with 0.3, at the end of Stage 5a. Its decisions are in [ADR-0072](adr/0072-stage-4b-live-assets.md) to [ADR-0077](adr/0077-hot-reload-of-shaders.md). The owner checked the recovery from a lost device by hand at its close: in the Windows virtual machine, from the dev overlay and by a forced TDR, and in a KDE Plasma session after a real reset of the GPU (see [Known debt](#known-debt)). |

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
  discards. The checks run the game for one frame by day and one at night. The oaks' cards made
  smaller and four times as many at M4b.8 changed it little: the day's shadow pass took 6.0 s
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
  (`crates/slime-app/src/signal.rs`): what a game saves in `shutdown` is lost then.
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
  -- --ignored --nocapture`, against the budget of M3b.6. To move to the asset pipeline of Stage 4,
  or to compute passes if skies change at run time; if the tests' time matters first, building
  `slime-render` optimized in debug builds would shorten it, at the cost of stepping through it in
  a debugger.
- **The sky's sun is described twice in the renderer's own API** (ADR-0039). A game that draws
  with it gives the same `DirectionalLight` to `Sky::sun`, for the disc and the glow, and to
  `Scene::lights`, for its light and its shadows; two different lights show a disc where no light
  comes from. The `scene` example takes both from the file's first directional light. A world gives
  it once ([ADR-0084](adr/0084-what-the-world-draws.md)), as *Hunna Wood*'s does.
- **A save of *Hunna Wood*'s world would hold more than its run** (ADR-0079). Besides the hero's
  transform, what is hidden and the night, the world holds what the game derives from the zone when
  it spawns it: its own components, the moon showing in the sky, the lights of the other time of day
  hidden; and the campfire's intensity, which the game writes at each frame. The camera's yaw is
  outside the world. M5a.8 and M5a.9 compare a world with the one the game prepared, or mark these
  values transient.
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
  Windows 17 minutes 26 seconds.
