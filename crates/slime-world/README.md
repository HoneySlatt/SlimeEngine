# slime-world

The world: the entities of a game, their components and their hierarchy, kept by the engine
(`docs/adr/0079-the-engines-own-world.md`).

## Status

Started in M5a.1 (`docs/PLAN.md`). A world gives out entity handles, keeps the components attached
to them, and finds the entities that hold the components a query asks for. It keeps each entity's
parent and children and where each one is in the world, and spawns a cooked scene as entities with
persistent identities. Its components say what is drawn, by asset ID, and it keeps where what moved
during the last fixed step was, to draw it between its two transforms; `slime-world-render` draws
it. Every component is reflected, and reached by name. It saves what differs from the world as a
game makes it at the start of a run, as text, and loads it back.

## Role

- Keeps a game's things as *entities*, handles made of an index and a generation, and their
  values as *components*, one of each type per entity, stored by type
  (`docs/adr/0082-the-worlds-storage-and-handles.md`).
- Keeps the *hierarchy*, each entity's parent and children, and where each entity with a
  `Transform` is in the world, positions in double precision
  (`docs/adr/0083-transforms-and-the-hierarchy.md`).
- Says what is drawn without holding GPU objects: meshes by asset ID, lights, what is hidden; and
  keeps the transforms before the last fixed step of what moved during it, so that it is drawn
  between its two transforms (`docs/adr/0084-what-the-world-draws.md`).
- Keeps the registry of its components' types, through which every component is listed, read and
  set by name, without its type (`docs/adr/0085-reflection.md`).
- Saves a world as what differs from the world that the game makes at the start of a run, entities
  named by their persistent identities, and loads it back, setting aside what no longer applies
  (`docs/adr/0087-the-worlds-documents.md`).
- Runs headless: it holds no GPU object and needs no window, so that tests, servers and tools hold
  worlds too. Worlds are values, which threads can share.

## Entry points

```rust
use slime_reflect::{Reflect, Value};
use slime_world::{Component, Transform, World};
use slime_world::glam::DVec3;

#[derive(Reflect)]
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

world.query_mut::<(&mut Transform, &Velocity)>(|_, (transform, velocity)| {
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
  (`#[derive(Reflect)]`, `slime-reflect`), and that says it is one: `impl Component for Velocity {}`.
- `World::query` reads the entities that hold every component of a `Query`, `&T` or a tuple of up
  to four, through an iterator; `World::query_mut` changes them through a closure, with `&mut T`
  in its `QueryMut`.
- `World::set_parent`, `parent` and `children` keep the hierarchy. `Transform` places an entity
  relative to its parent: an `f64` translation, a rotation, a uniform scale that children inherit
  and a stretch that they do not. `World::global_transform` gives where it is in the world, a
  `GlobalTransform`, always current; `World::update_transforms` computes again what changed, so that
  reading costs a lookup.
- `World::spawn_scene` spawns a cooked scene of `slime-assets` under a key, an entity per node with
  its `Transform`, `Name`, `Meshes`, `Light` and `Extras`; `World::find` gives the entity of a
  `PersistentId`, such as `zone/door`.
- `Meshes`, `Light` and `Hidden` say what is drawn: meshes by asset ID, each drawn with the material
  of its cooked mesh, lights at their entity's origin along its −Z axis, a directional one showing
  in the sky or not, and the subtrees left out; `World::is_hidden` says whether an entity is, by
  itself or through an ancestor.
- `World::begin_fixed_step` and `World::end_fixed_step`, which `slime-app` calls around each fixed
  step, keep the transforms before the step of what moves during it;
  `World::interpolated_transform(entity, fraction)` gives where it is drawn between the two, and
  `World::teleport` makes an entity be drawn at once where it goes.
- `World::registry` holds the types of the components, the engine's and each type the world stored a
  component of, or `World::register` registered ahead. `World::entities`, `component_names`,
  `component_value`, `set_component_value` and `set_property` list the entities and reach their
  components by name and property path, without their types, within the ranges of their fields.
- `World::save(start)` writes what differs between the world and `start`, the world that the game
  makes at the start of a run from the same scenes: entities removed and created, components added,
  removed and changed, parents changed, references to entities as persistent identities, or `""`
  for no entity. Entities that hold `Unsaved`, with their descendants, and fields marked transient
  are left out. `World::load` applies a save to such a world, and says what it set aside and which
  scenes changed since; `World::first_difference` compares two worlds as saves see them.
  `World::insert_component_value` and `remove_component_value` attach and detach a component by
  its type's name, as loading a save does.
- `slime_world::glam` re-exports the math crate the API uses, so that callers use the same version.

The rustdoc holds the contracts: what panics, what each query visits, what each operation costs.

## Dependencies

- `slime-assets`: the cooked scenes it spawns.
- `glam`: vectors, rotations and matrices; `DVec3` for positions.
- `slime-color`: the colors of its lights.
- `slime-reflect`: every component is reflected, and each world keeps the registry of their types.
- `slime-serialize`: the text of saves.
- `thiserror`: derives `WorldError`'s `Display` and `Error` implementations.
- `tracing`: the warnings of a scene spawned with nodes known by their index, or scaled unevenly or
  mirrored.

## Must not

- Depend on a GPU, a window or `slime-app`: the world runs headless, and holds asset IDs rather
  than GPU objects (ADR-0079).
- Write entity handles to files: they mean something only in the world that gave them out; files
  name entities by their persistent identity.
