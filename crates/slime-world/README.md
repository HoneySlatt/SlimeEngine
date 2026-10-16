# slime-world

The world: the entities of a game, their components and their hierarchy, kept by the engine
(`docs/adr/0079-the-engines-own-world.md`).

## Status

Started in M5a.1 (`docs/PLAN.md`). A world gives out entity handles, keeps the components attached
to them, and finds the entities that hold the components a query asks for. It keeps each entity's
parent and children and where each one is in the world, and spawns a cooked scene as entities with
persistent identities. It draws nothing yet (M5a.3), and cannot be saved (M5a.8).

## Role

- Keeps a game's things as *entities*, handles made of an index and a generation, and their
  values as *components*, one of each type per entity, stored by type
  (`docs/adr/0082-the-worlds-storage-and-handles.md`).
- Keeps the *hierarchy*, each entity's parent and children, and where each entity with a
  `Transform` is in the world, positions in double precision
  (`docs/adr/0083-transforms-and-the-hierarchy.md`).
- Runs headless: it holds no GPU object and needs no window, so that tests, servers and tools hold
  worlds too. Worlds are values, which threads can share.

## Entry points

```rust
use slime_world::{Component, Transform, World};
use slime_world::glam::DVec3;

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
```

- `World::spawn` gives a new `Entity`, `World::despawn` destroys one with its descendants and drops
  their components; the handle of a destroyed entity finds nothing afterwards.
- `World::insert`, `get`, `get_mut` and `remove` attach, read, change and detach one component of
  an entity. A component is a value of a type that threads can share and that says it is one:
  `impl Component for Velocity {}`.
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
- `slime_world::glam` re-exports the math crate the API uses, so that callers use the same version.

The rustdoc holds the contracts: what panics, what each query visits, what each operation costs.

## Dependencies

- `slime-assets`: the cooked scenes it spawns.
- `glam`: vectors, rotations and matrices; `DVec3` for positions.
- `thiserror`: derives `WorldError`'s `Display` and `Error` implementations.
- `tracing`: the warnings of a scene spawned with nodes known by their index, or scaled unevenly or
  mirrored.

## Must not

- Depend on a GPU, a window or `slime-app`: the world runs headless, and holds asset IDs rather
  than GPU objects (ADR-0079).
- Write entity handles to files: they mean something only in the world that gave them out; files
  name entities by their persistent identity.
