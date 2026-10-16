# slime-world

The world: the entities of a game and their components, kept by the engine
(`docs/adr/0079-the-engines-own-world.md`).

## Status

Started in M5a.1 (`docs/PLAN.md`). A world gives out entity handles, keeps the components attached
to them, and finds the entities that hold the components a query asks for. It has no hierarchy or
transforms yet (M5a.2), draws nothing (M5a.3), and cannot be saved (M5a.8).

## Role

- Keeps a game's things as *entities*, handles made of an index and a generation, and their
  values as *components*, one of each type per entity, stored by type
  (`docs/adr/0082-the-worlds-storage-and-handles.md`).
- Runs headless: it holds no GPU object and needs no window, so that tests, servers and tools hold
  worlds too. Worlds are values, which threads can share.

## Entry points

```rust
use slime_world::{Component, World};

struct Position(f64);
impl Component for Position {}
struct Velocity(f64);
impl Component for Velocity {}

let mut world = World::new();
let hero = world.spawn();
world.insert(hero, Position(0.0));
world.insert(hero, Velocity(3.0));

world.query_mut::<(&mut Position, &Velocity)>(|_, (position, velocity)| {
    position.0 += velocity.0 / 60.0;
});
for (entity, position) in world.query::<&Position>() {
    assert_eq!(entity, hero);
    assert_eq!(position.0, 0.05);
}
```

- `World::spawn` gives a new `Entity`, `World::despawn` destroys one and drops its components;
  the handle of a destroyed entity finds nothing afterwards.
- `World::insert`, `get`, `get_mut` and `remove` attach, read, change and detach one component of
  an entity. A component is a value of a type that threads can share and that says it is one:
  `impl Component for Position {}`.
- `World::query` reads the entities that hold every component of a `Query`, `&T` or a tuple of up
  to four, through an iterator; `World::query_mut` changes them through a closure, with `&mut T`
  in its `QueryMut`.

The rustdoc holds the contracts: what panics, what each query visits, what each operation costs.

## Dependencies

None: the standard library only.

## Must not

- Depend on a GPU, a window or `slime-app`: the world runs headless, and holds asset IDs rather
  than GPU objects (ADR-0079).
- Write entity handles to files: they mean something only in the world that gave them out.
