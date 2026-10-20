# 0079. The engine's own world

- Status: Accepted; amends [0045](0045-one-linear-color-type.md) (where the color type lives) and
  [0048](0048-rendering-relative-to-the-camera.md) (positions in the API); amended by
  [0087](0087-the-worlds-documents.md) (what a save differs from, and the identities of entities
  created outside scenes), [0107](0107-the-execution-model.md) (the phases of a frame) and
  [0108](0108-what-the-world-knows-of-its-changes.md) (change detection for every component, and
  components that clone)
- Date: 2026-10-16

## Context

The roadmap leaves the entity model open until Stage 5, between hecs, bevy_ecs, an ECS of the
engine's own and a model without one, and [ADR-0048](0048-rendering-relative-to-the-camera.md) and
[ADR-0052](0052-amend-roadmap-before-0-1.md) leave it the precision of world positions.
[ADR-0057](0057-what-the-first-game-asks-of-the-next-stages.md) asks that the world keep what is
drawn, and the transforms of what moves at its last two fixed steps;
[ADR-0072](0072-stage-4b-live-assets.md) leaves it handles to assets and assets shared between
scenes. Stage 5a builds the world ([ADR-0078](0078-stage-5-in-two-halves.md)).

Where the engine stands:

- There is no world. A game keeps its own objects and gives the renderer, at every frame, a list
  of instances that borrow their GPU objects; the renderer keeps no scene between frames
  ([ADR-0027](0027-the-render-crate.md)).
- *Hunna Wood* keeps its hero's previous position to draw it between fixed steps, rebuilds about
  430 instances at every frame, and leaves out the key's and the door's parts by the indices of
  their nodes. After a lost GPU device it makes its whole view again; on hot reload it swaps whole
  scenes.
- A cooked scene's nodes have a name, a parent, an `f32` matrix, meshes, a light and `extras`. Their
  only identity is their index, which an export from Blender can change.

What later stages ask of the world: an editor that inspects any entity, shows a hierarchy, undoes
every edit and copies a world to play in it (Stage 6); a job system that runs work in parallel
(Stage 7); scripts and plugins that define components with no Rust type behind them and reach
entities through a C ABI (Stage 8); identities that hold across machines and across the cells of a
streamed world (Stage 10).

What the design studies found:

- bevy_ecs 0.20 makes a breaking release every four or five months (0.16 in April 2025, 0.20 in
  October 2026), raising the Rust version it needs each time, from 1.85 to 1.95; it adds
  thirty-five to forty crates to the build, requires glam 0.33 where the engine uses 0.34, and
  brings a reflection and a thread pool of its own.
- hecs 0.11 adds two crates, but keys components by their Rust type and has no change detection:
  the hierarchy, the transforms and the identities would still be the engine's to write.
- flecs is a C library, reached through Rust bindings that call themselves alpha; its world cannot
  be shared between threads, and building it needs a C compiler and bindgen.
- An `f32` position moves by steps of 1 mm at 10 km from the origin, 7.8 mm at 100 km and 6 cm at
  1 000 km. At 100 km, *Hunna Wood*'s hero, which moves 5 cm per fixed step, would move by six or
  seven steps of 7.8 mm, its speed off by 8 %. Drawing relative to the camera repairs the image,
  not the simulation.

## Decision

The owner took these decisions on 2026-10-16, when Stage 5a was planned.

**The entity model is the engine's own**, in a crate of its own, `slime-world`, which depends on no
GPU and no window.

- An entity is a handle made of an index and a generation: the handle of a destroyed entity is
  refused, and a slot whose generation would wrap is retired. Handles are never written to files.
- A component is a value of a type that threads can share; an entity holds at most one of each
  type. The world stores components by type, keyed by a component ID of its own rather than by the
  Rust type alone, so that Stage 8 can add components with no Rust type. The storage is private: it
  starts simple, in safe code, and changes only for a measured need.
- Queries visit the entities that hold the components they ask for.
- The world keeps the hierarchy: each entity's parent and ordered children. Destroying an entity
  destroys its subtree.
- Worlds are values: a game, a test, an editor or a server holds as many as it needs.
- No systems and no schedule at Stage 5: a game's logic stays plain code in its `App` methods,
  which read and change the world. How work runs in parallel is Stage 7's. Change detection covers
  transforms only, until another user needs it.

**Translations are `f64`**, in the world and in the renderer's API: the camera, the lights and the
instances. Rotations, scales, velocities, meshes and everything the GPU reads stay `f32`; the
renderer still subtracts the eye on the CPU (ADR-0048). This amends ADR-0048, whose API kept `f32`
positions until Stage 5 decided.

**Transforms.** An entity's local transform is a translation, a rotation and a uniform scale, which
its children inherit, and a *stretch* along its own axes, which applies to its own meshes and
shapes and which its children do not inherit. Its transform in the world is computed from the
hierarchy, after a change only for the subtrees that changed, and is current whenever read. An
imported node's scale becomes the uniform scale when it is uniform within a tolerance; otherwise
the stretch of a node without children; on a node with children, a warning, the node taking it as
its stretch and its children placed from their exact transforms in the world.

**The world keeps where things were.** `slime-app` owns the game's world, in its `Context`, and
calls it around each fixed step. What moved during a step is drawn between its last two
transforms, at the frame's `fixed_step_alpha`. A change made outside a step, such as a restart, is
drawn at once; a game marks a teleport made inside a step.

**The world keeps what is drawn, and no GPU object.** Components name meshes, materials and textures
by their asset IDs. The GPU objects live in a cache outside the world, which loads them in the
background, shares those that several scenes use, makes them again after a lost device, and loads
them again when their cooked data changes. At each frame, a bridge builds the renderer's instances
and lights from the world; `slime-render` does not depend on the world. The color type moves below
`slime-gfx`, as [ADR-0045](0045-one-linear-color-type.md) foresaw, so that the world holds colors
without the GPU layer; this amends ADR-0045.

**Identity.** In a file, each entity has an ID local to that file: for an imported glTF node, its
name, or its index with a warning when the name is missing or repeated; a readable ID chosen by
whoever writes a file by hand; a random one for an entity that a tool creates. Its *persistent
identity* is the path of local IDs through the scenes that placed it, from the key that the world
spawned the outermost one under: `zone/door`. Files, saves and the references between entities in
them use it.

**A save is a difference.** It holds what differs between a world and the scenes it was made from:
values changed, components added or removed, entities removed, entities created while it ran. Data
marked transient, such as the previous transform kept to draw between steps, is not saved. Loading
spawns the scenes from their current content, then applies the differences; a difference whose
target is gone is kept aside and reported.

## Alternatives considered

- **bevy_ecs used alone**: the most complete entity model in Rust, but its releases, its crates
  and its glam version above; its types would become the API of every game, and its
  reflection and its thread pool would duplicate or decide Stage 5's and Stage 7's.
- **hecs behind a facade of the engine's own**: fast storage for two crates, but components keyed
  by their Rust type, so a script's component would need a workaround; the rest is to write
  anyway.
- **flecs**: the best support for components with no Rust type, but alpha bindings, a C compiler in
  the build and a world that threads cannot share.
- **A tree of nodes, as Godot's**: familiar, but finding everything that holds a light visits every
  node.
- **`f32` positions**: worlds of about 5 to 10 km around the origin. Changing later touches every
  system written by then, as Unreal Engine 5's large world coordinates did.
- **`f32` within cells, or an origin that follows the player**, now: every system must follow when
  the origin moves, or positions become pairs everywhere; premature before the streaming of
  Stage 10, which can add cells on top of `f64` translations.
- **A non-uniform scale that children inherit**, as in Bevy, Godot and Unity: a child that turns
  under a stretched parent is sheared, and transforms in the world no longer split into
  translation, rotation and scale, which interpolation and the shapes of physics need. **A uniform
  scale only**: walls and box colliders could not stretch.
- **A game that keeps its world and calls it around each step itself**: every game, example and the
  editor could get the order wrong.
- **Interpolation that each entity asks for**: one more thing to forget.
- **GPU objects in components**: the world would hold GPU state; it could not run headless or be
  saved, and every lost device would purge it. **`slime-render` depending on the world**: one
  crate fewer, but the renderer would no longer draw any list it is given (ADR-0027).
- **Handles that count references**: counters in components complicate the C ABI and saves;
  counting can be added on top of asset IDs later.
- **Identity by node index**, which an export reorders; **by the path of names in the hierarchy**,
  which a change of parent breaks; **by random IDs only**, which imported nodes cannot carry; **by a
  128-bit ID derived and stored for each entity**: it works, but makes saves unreadable; it can be
  how the world keeps identities in memory, if a measurement asks for it.
- **A save that copies the whole world**: a game update that fixes its scenes would never reach the
  saves made before. **Saves that each game writes itself**: the same code in every game, and any
  state forgotten is lost.

## Consequences

- The engine writes and maintains its world, about two milestones; its cost at 100 000 entities is
  measured, and improved inside its private storage.
- The renderer's API takes `f64` positions: every game and example that places a camera, a light or
  an instance changes. Physics (Stage 7) takes `f64` positions, or simulates around an origin of its
  own.
- `slime-app` depends on `slime-world`, and headless runs hold a world too.
- Recovering from a lost device, and hot reload, become the cache's work: the world is untouched,
  and a game no longer makes its view again itself.
- Renaming an object in Blender changes its identity: a save forgets what it knew of it, and loading
  says so. The zone's script names explicitly the objects the game refers to.
- Stage 6 builds the editor's hierarchy, inspector and undo on the world's handles, components and
  persistent identities; Stage 8 adds components with no Rust type behind component IDs; Stage 10
  uses identities and differences for cells and replication.
- Each milestone that builds a part of the world records its design in an ADR of its own.
