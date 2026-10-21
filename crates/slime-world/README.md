# slime-world

The world: the entities of a game, their components and their hierarchy, kept by the engine
(`docs/adr/0079-the-engines-own-world.md`).

## Status

Started in M5a.1 (`docs/PLAN.md`). A world gives out entity handles, keeps the components attached
to them, and finds the entities that hold the components a query asks for. It keeps each entity's
parent and children and where each one is in the world, and spawns scenes as entities with persistent
identities: glTF scenes as the cooker makes them, and scene files that place prefabs, each placement
with values of its own. Its components say what is drawn, by asset ID, and it keeps where what moved
during the last fixed step was, to draw it between its two transforms; `slime-world-render` draws
it. Every component is reflected, reached by name, and cloned. It knows what changed: each value
holds the ticks at which it was added and last changed, and readers ask what changed, what left and
whether the hierarchy moved since they last read. It saves what differs from the world as a
game makes it at the start of a run, as text, what it spawned while it ran as its scenes, and loads
it back; it writes a world, or a subtree of it, back as a scene file that keeps its placements. Its
library of scenes reads them from cooked data, the scene files in their binary encoding.

## Role

- Keeps a game's things as *entities*, handles made of an index and a generation, and their
  values as *components*, one of each type per entity, stored by type
  (`docs/adr/0082-the-worlds-storage-and-handles.md`).
- Stamps every change with the world's *tick*, so that each reader asks, at its own pace, what was
  added, changed or removed since it last read, rather than read the whole world
  (`docs/adr/0108-what-the-world-knows-of-its-changes.md`,
  `docs/adr/0119-the-worlds-ticks-blocks-and-log.md`).
- Keeps the *hierarchy*, each entity's parent and children, and where each entity with a
  `Transform` is in the world, positions in double precision
  (`docs/adr/0083-transforms-and-the-hierarchy.md`).
- Says what is drawn without holding GPU objects: meshes by asset ID, lights, what is hidden; and
  keeps the transforms before the last fixed step of what moved during it, so that it is drawn
  between its two transforms (`docs/adr/0084-what-the-world-draws.md`).
- Keeps the registry of its components' types, through which every component is listed, read and
  set by name, without its type (`docs/adr/0085-reflection.md`).
- Spawns scenes: glTF scenes, and scene files that place prefabs, other scenes placed many times,
  each placement with overrides of its own, resolved once by a library of scenes and then spawned
  as often as a game needs (`docs/adr/0089-scenes-and-prefabs.md`,
  `docs/adr/0091-scene-documents-and-placements.md`); reads them from cooked data, the typed
  components of glTF nodes among them (`docs/adr/0095-cooked-scene-files.md`).
- Writes a world, or a subtree of it, back as a scene file that keeps its placements
  (`docs/adr/0092-a-world-written-back-as-a-scene.md`).
- Saves a world as what differs from the world that the game makes at the start of a run, entities
  named by their persistent identities, what it spawned while it ran as its scenes, and loads it
  back, setting aside what no longer applies; carries a run into a world spawned anew from scenes
  that changed (`docs/adr/0087-the-worlds-documents.md`,
  `docs/adr/0093-saves-and-hot-reload-with-placements.md`).
- Runs headless: it holds no GPU object and needs no window, so that tests, servers and tools hold
  worlds too. Worlds are values, which threads can share.

## Entry points

```rust
use slime_reflect::{Reflect, Value};
use slime_world::{Component, Transform, World};
use slime_world::glam::DVec3;

#[derive(Clone, Reflect)]
#[reflect(name = "demo::Velocity")]
struct Velocity(f64);
impl Component for Velocity {}

let mut world = World::new();
let cart = world.spawn();
world.insert(cart, Transform::from_translation(DVec3::new(0.0, 0.0, 0.0)));
world.insert(cart, Velocity(3.0));
let lantern = world.spawn();
world.insert(lantern, Transform::from_translation(DVec3::new(0.0, 2.0, 0.0)));
world.set_parent(lantern, Some(cart)).expect("both are alive");

world.query_mut::<(&mut Transform, &Velocity)>(|_, (mut transform, velocity)| {
    transform.translation.x += velocity.0;
});
// The lantern rides on the cart.
let lantern_at = world.global_transform(lantern).expect("placed").translation;
assert_eq!(lantern_at, DVec3::new(3.0, 2.0, 0.0));

// Through reflection, by name, without the component's type.
let speed = "0".parse().expect("a path");
world.set_property(cart, "demo::Velocity", &speed, Value::F64(5.0)).expect("an f64");
assert_eq!(world.get::<Velocity>(cart).expect("held").0, 5.0);
```

- `World::spawn` gives a new `Entity`, whose persistent identity is `created/<number>`, from a count
  that saves keep; `World::despawn` destroys one with its descendants and drops their components;
  the handle of a destroyed entity finds nothing afterwards.
- `World::insert`, `get`, `get_mut` and `remove` attach, read, change and detach one component of
  an entity. A component is a value of a type that threads can share, that reflection describes
  (`#[derive(Reflect)]`, `slime-reflect`), that clones, and that says it is one:
  `impl Component for Velocity {}`. `get_mut` gives a guard, `Mut`, whose first write stamps the
  value as changed; `Mut::set_if_differs` writes and stamps only a value that differs.
- `World::query` reads the entities that hold every component of a `Query`, `&T` or a tuple of up
  to four, through an iterator; `World::query_mut` changes them through a closure, with `&mut T`
  in its `QueryMut`, which gives guards: a closure that writes through one declares it `mut`.
- `World::tick` is the world's tick now, which a reader keeps; `World::changed` gives the components
  of a type written or added after a tick, skipping the blocks of values that did not change;
  `World::removed` and `World::destroyed`, what left after it; `World::structure`, the tick at
  which entities last came, went, changed parent or name; `World::ticks`, a component's ticks.
- `World::set_parent`, `parent` and `children` keep the hierarchy, `children` an iterator over an
  entity's children in their order, each linked to its siblings, so that one leaves its parent in
  constant time (`docs/adr/0106-the-measured-quick-wins.md`). `Transform` places an entity
  relative to its parent: an `f64` translation, a rotation, a uniform scale that children inherit
  and a stretch that they do not. `World::global_transform` gives where it is in the world, a
  `GlobalTransform`, always current; `World::update_transforms` computes again what changed, so that
  reading costs a lookup.
- `SceneLibrary` holds a game's scenes by asset ID: those of its cooked data, glTF scenes and
  cooked scene files, read when first resolved (`SceneLibrary::of`), and read again when the cooked
  data changes (`SceneLibrary::reload`); and those given by hand, glTF scenes (`insert_cooked`) and
  the text of scene files (`insert_file`). It resolves one (`SceneLibrary::resolve`) into a
  `ResolvedScene`: its entities, the prefabs that it places resolved with it, and their overrides
  applied, those whose target the prefab no longer has set aside (`ResolvedScene::set_aside`).
  `ResolvedScene::from_cooked` resolves a glTF scene alone, an entity per node with its
  `Transform`, `Name`, `Meshes`, `Light`, `Extras` and its typed components, read through a
  registry. `outline` reads a scene file's layout without the types of its values, as the cooker
  checks it (`docs/adr/0095-cooked-scene-files.md`).
- `World::spawn_scene` spawns a resolved scene under a key, its entities named by their paths
  through the placements, such as `zone/lamp 3/light`, and refuses a key under which it spawned
  another scene of which an entity lives (`WorldError::KeyInUse`); `World::spawn_keyed` creates an
  entity named by a key that the game chooses, as what it creates at its start, which saves find
  whatever else it creates; `World::find` gives the entity of a `PersistentId`; `World::placement` says what the world remembers of a placement: its prefab, and
  the overrides of the scene that placed it that were set aside. `World::spawn_placement` places a
  prefab while the world runs, under `created/<number>`.
- `World::write_scene` writes a subtree back as a scene file (`Written`): placements as placements
  with what differs from their prefab, the overrides that the scene wrote kept, driven components
  as their author wrote them, entities created since given IDs that they keep, and by which a run
  carried into the file cooked again finds them, once, the same world in the same bytes;
  `World::placement_root` and `World::differs_from_prefab` say which placement an entity belongs
  to, and whether a component of it differs from the prefab's
  (`docs/adr/0092-a-world-written-back-as-a-scene.md`).
- `Meshes`, `Light` and `Hidden` say what is drawn: meshes by asset ID, each drawn with the material
  of its cooked mesh, lights at their entity's origin along its −Z axis, a directional one showing
  in the sky or not, and the subtrees left out; `World::is_hidden` says whether an entity is, by
  itself or through an ancestor.
- `World::begin_fixed_step` and `World::end_fixed_step`, which `slime-app`'s host calls around each
  fixed step, keep the transforms before the step of what moves during it;
  `World::interpolated_transform(entity, fraction)` gives where it is drawn between the two, and
  `World::teleport` makes an entity be drawn at once where it goes.
- `World::registry` holds the types of the components, the engine's and each type the world stored a
  component of, or `World::register` registered ahead. `World::entities`, `component_names`,
  `component_value`, `set_component_value` and `set_property` list the entities and reach their
  components by name and property path, without their types, within the ranges of their fields.
- `Command` is a change of the world asked for outside the phases of a frame that may make it,
  which names its entity by its persistent identity: `Command::SetProperty`, a value set at a
  property path of a component. `World::apply` applies one, as `slime-app`'s host does at the start
  of each frame with those queued since the last one
  (`docs/adr/0121-a-game-as-a-simulation-and-a-presentation.md`).
- `World::save(start, about)` writes what differs between the world and `start`, the world that the
  game makes at the start of a run from the same scenes: entities removed and created, components
  added, removed and changed, parents changed, references to entities as persistent identities, or
  `""` for no entity. Entities that hold `Unsaved`, with their descendants, and fields marked
  transient are left out, and no change of the components that their entity marks driven
  (`Driven`) is saved, their author's values kept, which their system computes again; a start
  that created, without a key, entities that saves hold is refused
  (`DocumentError::UnkeyedAtStart`,
  `docs/adr/0124-driven-components-keys-and-created-entities-in-levels.md`). The save begins with
  `about`, what the game says of it, keys and texts that `World::about` reads without loading the
  save, and says how many entities `start` created outside scenes.
  What the world spawned while it ran, a scene under a key or a placement, is saved as its scene and
  what differs from it. `World::load` applies a save to such a world, spawning those scenes again
  through a `SceneLibrary`, what the run created numbered after the world's count, and says what it
  set aside, what it did not know (`Loaded::warnings`) and which scenes changed since
  (`docs/adr/0101-saves-and-identities-that-never-mislead.md`); `World::carry` carries a run into a start world made from
  changed files, as hot reload does; `World::first_difference` compares two worlds as saves see
  them (`docs/adr/0093-saves-and-hot-reload-with-placements.md`). `World::insert_component_value`
  and `remove_component_value` attach and detach a component by its type's name, as loading a save
  does.
- `slime_world::glam` re-exports the math crate the API uses, so that callers use the same version.

The rustdoc holds the contracts: what panics, what each query visits, what each operation costs.

## Dependencies

- `slime-assets`: the cooked scenes it spawns, and the asset IDs that name scenes.
- `glam`: vectors, rotations and matrices; `DVec3` for positions.
- `slime-color`: the colors of its lights.
- `slime-reflect`: every component is reflected, and each world keeps the registry of their types.
- `slime-serialize`: the text of saves and of scene files.
- `thiserror`: derives `WorldError`'s `Display` and `Error` implementations.
- `tracing`: the warnings of a scene spawned with nodes known by their index, or scaled unevenly or
  mirrored.

## Must not

- Depend on a GPU, a window or `slime-app`: the world runs headless, and holds asset IDs rather
  than GPU objects (ADR-0079).
- Write entity handles to files: they mean something only in the world that gave them out; files
  name entities by their persistent identity.
