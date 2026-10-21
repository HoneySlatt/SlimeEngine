# 0082. The world's storage and handles

- Status: Accepted; amended by [0119](0119-the-worlds-ticks-blocks-and-log.md) (the ticks of each
  value, a summary per block of values and a log of removals in each storage; `query_mut` gives
  guards)
- Date: 2026-10-16

## Context

[ADR-0079](0079-the-engines-own-world.md) decides that the entity model is the engine's own, in
`slime-world`: entities are handles of an index and a generation, components are stored by type
under component IDs of the world's own, so that Stage 8 can add types with no Rust type, and the
storage starts simple, in safe code, changing only for a measured need. It leaves how to the
milestone that builds it, M5a.1, and asks it to measure 100 000 entities.

What uses the world first: the renderer's bridge reads, at every frame, the entities that hold
something to draw (M5a.3); the game reads its colliders and changes its hero, a key and a door
(M5a.4); the dev overlay's panel and the saves walk every entity (M5a.6, M5a.8). The crates have no
`unsafe` code, and introducing some is the owner's decision
([ADR-0005](0005-lints-and-unsafe-policy.md)).

## Decision

**Handles.** An `Entity` is the index of a slot in its world (`u32`) and the slot's generation
(`NonZeroU32`), 8 bytes, and 8 bytes as an `Option<Entity>`. Destroying an entity frees its slot
for a later one under the next generation; a slot whose generation would wrap is retired for good,
so that no two entities of a world ever share a handle. The free slots are reused last freed first.
A world cannot tell a handle from another world that happens to have the same index and
generation: a handle means something in the world that gave it out.

**Components.** A type becomes a component by saying so, `impl Component for Position {}`; it must
be one that threads can share. A tuple or a function given to `World::insert` by mistake then does
not compile, where a component trait implemented for every type would store it as one component.

**Storage.** Each component type has a *sparse set*: its values side by side in a dense array, the
slot of each value beside it, and, for each slot, the index of its value. Reading, attaching and
detaching a component cost a lookup; detaching moves the last value into the hole. The world keeps
the sets in an array behind a trait, indexed by component IDs that it gives in the order types are
first attached; a map from Rust type to ID finds them. Destroying an entity removes its slot from
every set. Component IDs stay inside the process: files and tools name types through reflection
(ADR-0080).

**Queries.** A query names component types, `&T` to read and `&mut T` to change, alone or in tuples
of up to four:

- `World::query` reads, through an iterator of each entity and its components.
- `World::query_mut` may change, calling a closure for each entity. Each storage is borrowed apart,
  safely, so that the query's parts hold theirs together; a query that names a type twice panics.
- Both walk the set of the component that the fewest entities hold, and look the others up for each
  of its slots, in no particular order, which attaching and detaching change.

**Misuse.** Attaching a component to an entity that is not alive panics: the component would be
lost, which only a bug does ([ADR-0008](0008-errors-logging-and-assertions.md)). Reading, changing
or detaching one, and destroying an entity, give nothing for such a handle.

**Measured** by the crate's ignored test, in a release build on the owner's machine (Ryzen 7
5800X), with 100 000 entities, all holding a position, half a velocity and a third a name: spawning
them took 5.3 ms; reading every position, 68 µs; reading the 16 667 that hold all three, 34 µs;
moving the 50 000 that hold a velocity, 190 µs; reading the 100 000 positions one by one, by handle,
980 µs; destroying them, 0.85 ms.

## Alternatives considered

- **Archetype tables**, which store together the entities that hold the same set of types: faster
  to iterate many types at once, but attaching or detaching a component moves the entity between
  tables, and columns whose type is known only at run time need unsafe code or byte buffers. To
  reconsider if a measurement of the game asks for it; the storage is private, so games would not
  change.
- **A `Vec<Option<T>>` per type, indexed by slot**: simpler, but memory for every slot of every
  type, and iteration through the holes.
- **A map from entity to value per type**: no holes, but iteration and lookup slower.
- **Mutable queries as iterators**: idiomatic, but handing out several `&mut` into storages at once
  from an iterator needs unsafe code.
- **Handles with a 64-bit generation**: no retired slots, but 16 bytes per handle.
- **Attaching to a destroyed entity returning an error**: every caller would handle a case that only
  a bug produces.
- **Every type that threads can share a component**, without saying so: one line less per type,
  but `insert(entity, (position, velocity))` would store one tuple rather than two components.

## Consequences

- Games read with iterators and change with closures; a component can also be read or changed by
  handle, at the cost of a lookup of its type and of its slot.
- A component type that no entity has held costs nothing; one that entities have held keeps its
  storage, whose index from slots to values grows with the highest slot that held one.
- Destroying an entity costs one step per component type the world has held.
- A query of several types costs a lookup per type for each entity of the smallest set; the figures
  above are the baseline that later milestones and Stage 10 measure against.
- No `unsafe` code: the borrow checker proves the queries' borrows.
