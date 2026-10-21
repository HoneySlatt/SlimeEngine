# 0119. The world's ticks, blocks and log

- Status: Accepted; amends [0082](0082-the-worlds-storage-and-handles.md) (the ticks of each value,
  a summary per block of values and a log of removals in each storage; `query_mut` gives guards)
  and [0108](0108-what-the-world-knows-of-its-changes.md) (a tick of 64 bits given at each
  operation, 16 bytes of ticks per value where it estimated 8)
- Date: 2026-10-21

## Context

[ADR-0108](0108-what-the-world-knows-of-its-changes.md) decided that the world stamps every
change: each value of a component holds the tick at which it was added and the one at which it
last changed, mutable access gives a guard that stamps what is written through it, each storage
keeps a summary of its changes, whole and per block of values, a log tells what left, a structure
counter tells when the hierarchy moved, and a flag says which entities an ancestor hides. It left
to measurement the width of a tick, how a long run never sees one wrap, and the size of a block,
and to the milestone that builds it ([`docs/PLAN.md`](../PLAN.md), M6a.3) the shape of the rest.

The figures below were measured on the owner's machine, 16 logical CPUs and an RX 6900 XT, in a
release build, with `world.changes.100k`: 100 000 entities that place a light each, of which
1 000 change between two reads, every hundredth, then 1 000 side by side
([`benches/README.md`](../../benches/README.md)). The sizes of blocks and the tick of 32 bits
were measured on variants of the code, two runs each; the code as built, before and after, is in
[`measurements.md`](../measurements.md), "M6a.3, before" and "M6a.3".

## Decision

**A tick per operation, 64 bits wide.** The world's tick grows by one at each operation that may
change it: `insert`, `remove`, `get_mut`, `query_mut`, a value set through reflection, spawning,
destroying, a new parent, a scene spawned. What the operation writes is stamped with that tick.
A reader keeps `World::tick`, the latest tick given, and asks next time for what is stamped after
it: every later operation stamps a later tick, so that each reader sees each write once, at its
own pace, and the world holds nothing of its readers. Reading needs no `&mut World` and no atomic.
A tick of 64 bits never wraps: a world that changes a billion times a second takes centuries to
run out of them. Each value holds two, 16 bytes.

**The guard.** `World::get_mut` and the `&mut T` of `World::query_mut` give `Mut<T>`, which reads
as the value; its first write, through `DerefMut`, stamps the value, its block and its storage
with the operation's tick. `Mut::set_if_differs` writes and stamps only a value that differs from
the one held. A `Transform` marks where its entity and their descendants are in the world to be
computed again at its first write, not when it is reached: the guard holds the hierarchy's marks,
given to the one part of a query that changes transforms.

**Blocks of 16 values.** Each storage keeps, for each block of 16 values side by side, a tick at
or after the latest change of every value it holds, one moved into it as another was removed
included; and the latest tick at which any value was written, added or removed. `World::changed` skips a storage
whose summary is not after a reader's tick, and the blocks whose summary is not; a block that a
value left keeps its summary, which a reader reads again to find nothing there. Measured:

| Block | 1 000 spread, found | 1 000 side by side, found |
|---|---|---|
| 16 | 17.6 to 18.2 µs | 4.1 to 4.2 µs |
| 32 | 24.5 to 25.4 µs | 2.4 to 2.5 µs |
| 64 | 35.7 to 36.8 µs | 1.7 to 1.8 µs |
| 128 | 46.3 to 47.6 µs | 1.3 to 1.4 µs |
| 256 | 39.3 µs | 1.2 µs |
| 1 024 | 47.6 to 51.4 µs | 1.3 µs |

Changes spread among many values, as the few entities that move among a level's still ones are,
cost most; blocks of 16 halve it, and leave the case of values side by side under 5 µs. A summary
costs half a byte per value.

**A log of what left.** Each storage logs the entities that a component of its type left, by
removal or because the entity was destroyed, and the world logs the entities destroyed, each with
its tick, oldest first (`World::removed`, `World::destroyed`). A log keeps 65 536 entries, then
forgets its older half and the tick of the newest it forgot: a reader whose tick is before it is
told so, `None`, and reads the whole world again. 16 bytes an entry, 1 MiB per log at most.

**The structure counter is a tick**, the latest at which an entity was spawned or destroyed or
changed parent, or the latest change of the storage of `Name`, a name given, changed or taken
away (`World::structure`): the names need nothing of their own, their storage's summary says it.
The dev overlay's world panel makes its list again only when the structure or the number of
entities differs from what it listed: shown another world, as after a hot reload, it lists the
new one once one of the two differs.

**The hidden flag** is kept in the hierarchy's node of each entity: adding or removing `Hidden`,
and a new parent, set it for the entity and down to its descendants whose flag changes;
`World::is_hidden` reads it in one step, where it walked the ancestors of every entity that a
frame draws (G32, in part).

## Alternatives considered

- **A tick of 32 bits, given at each frame, with ticks too old brought forward from time to
  time**, as Bevy does: 8 bytes per value, and a spread read of 13.8 µs where 64 bits took 17.5 to
  18.1 µs in the same runs, but a reader and a writer must agree on what a frame is, a pass over every value of
  the world every so many frames, and a reader that waited too long sees everything as changed. A
  tick of 32 bits given at each operation runs out in about 72 minutes in a world that changes a
  million times a second. `world.frame.100k` and `world.transforms.100k` took the same time, within
  a few percent, with both.
- **A tick that grows when a reader reads**, through an atomic, so that a world changed many
  times between two reads gives one tick: the same guarantee, but every read writes to the world,
  and nothing measured asks for it.
- **Blocks of 64 or more**: values side by side found faster, under 2 µs, but spread changes,
  the common ones, twice as slow or more.
- **A log that each reader empties, or one cleared at each frame**: the world would know its
  readers, or a reader that reads less often than once a frame, as a save, would miss what left.
  An unbounded log grows without end in a game that creates and destroys projectiles.
- **A structure counter of its own, moved by a guard of `Name`**: the guard would need a side for
  names beside the one for transforms, which the summary of names' storage already gives.
- **Marking a transform when it is reached**, as before: a reader that only reads through
  `get_mut` would make work for `update_transforms` and say that something moved.

## Consequences

- A reader of 100 000 entities of which 1 % changed costs 17.4 to 18.2 µs where comparing each
  with a copy costs 3.3 to 3.4 ms; 1 000 values written through guards take 8.6 to 8.7 µs at the
  median, where they took 10.0 to 10.7 µs through `&mut T`, the rounds of values side by side
  among them; `world.frame.100k` takes 10.0 to 10.4 ms where it took 11.4 ms, the hidden flag read
  in one step; `world.transforms.100k`, 3.44 to 3.45 ms where it took 3.57 to 3.58 ms.
- Each value of a component takes 16.5 bytes more: its ticks and its share of a summary.
- A closure of `query_mut` that writes through a guard declares it `mut`; a function that takes
  `&mut T` is given `&mut guard`, and a match that binds parts of the value to change them matches
  `&mut *guard`: each stamps the value even if nothing in it changes, and code that changes only
  some values reads first, as the examples do.
- Nothing reads what changed yet but the world panel, through the structure counter, and the
  tests: saves, frames and the dev overlay's panel of components read the whole world until each
  is made a reader.
- The figures were measured on one machine.
