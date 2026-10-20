# 0108. What the world knows of its changes

- Status: Accepted; amends [0079](0079-the-engines-own-world.md) (change detection for every
  component), [0083](0083-transforms-and-the-hierarchy.md) (the marks of transforms on the world's
  ticks, and the joints of a skin, which are not entities), [0085](0085-reflection.md) (components
  that clone), [0087](0087-the-worlds-documents.md), [0088](0088-saving-and-resuming-a-run.md) and
  [0092](0092-a-world-written-back-as-a-scene.md) (driven components, which saves and levels
  written back leave as their author wrote them)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) decides, before Stage 6 builds the editor on it, what the world
knows of what changed in it: change detection beyond transforms, changes made during a traversal,
lifecycle hooks, and the state that systems compute apart from the state an author writes (C1-02,
C1-04, C1-07, C5-02, C5-04). [ADR-0098](0098-the-games-the-engine-aims-at.md) adds a world cheap to
snapshot and simulate again, as the prediction of a shooter needs, and change detection precise
enough to send only what changed. The audit of 0.4 measured the figures below on the owner's RX
6900 XT at `v0.4.0`, on worlds of 100 000 entities made for it, unless a document of the
repository is named; its report is kept outside the repository, as the plan says.

- **Only transforms tell that they changed** ([ADR-0079](0079-the-engines-own-world.md),
  [ADR-0083](0083-transforms-and-the-hierarchy.md)). Reaching a `Transform` mutably marks its
  entity's subtree stale, whether the value changes or not, in one list per world, which
  `World::update_transforms` empties; no other type keeps a mark. Every other reader reads the
  whole world each time: building a frame, 11.7 ms for 100 000 entities on the RX 6900 XT after P4
  ([`measurements.md`](../measurements.md)); a save, 0.72 s when nothing changed, 63 % of it
  comparing components that did not change, measured before P2 bounded the size of a text save
  (the binary encoding, which a larger world needs, has no such bound); carrying a world into
  scenes that changed on disk, 0.79 s; the dev overlay's list of entities, made again at each
  frame, 125 to 145 ms. *Hunna Wood* stays within its budget. The editor's hierarchy and
  inspector, Stage 7's physics, which must learn that a collider was removed, audio, streaming and
  replication all ask the same question: what changed since I last looked.
- **Nothing tells authored state from driven state.** A save writes what differs from the start
  world ([ADR-0087](0087-the-worlds-documents.md)); the editor will show as overridden what differs
  from a prefab ([ADR-0092](0092-a-world-written-back-as-a-scene.md)). What a game computes again
  at each frame, a light that flickers, a door that a clip plays, a skeleton's pose, can be left out
  only by marking a field `transient`, which leaves it out of every component of that type, the
  lights that do not flicker included, or by `Unsaved`, which leaves out an entity and all its
  descendants and writes every reference to them as no entity. *Hunna Wood* puts its fire's
  intensity back before each save, and before it carries its world into scenes that changed on
  disk: `unflicker` ([ADR-0088](0088-saving-and-resuming-a-run.md)). A glTF node is an entity
  ([ADR-0083](0083-transforms-and-the-hierarchy.md)): a rigged model would give an entity per bone,
  which the audit estimated, from its figures per entity, at about 216 KB and 0.43 ms more per save
  for a character of 60 bones, plus its share of the transforms; 216 MB and 0.43 s per save for a
  thousand.
- **No hooks, and no deferred changes** (C1-04, C1-07). The only hook is the marking of
  transforms, written into the world. Creating, destroying, adding or removing a component needs
  the world mutably at once: on one thread, a game collects its changes and applies them after.
  The commands of the editor and the dev overlay are decided
  ([ADR-0107](0107-the-execution-model.md)): queued, applied at the start of a frame, their form
  decided with stable references. Stage 7's parallel systems cannot create or destroy during a
  traversal without a queue that several threads fill.
- **A world cannot be copied but through a save.** `World` is not `Clone`: a copy is a save then a
  load, 3.84 ms for *Hunna Wood*'s zone, about 0.6 s for 10 000 entities created in code, which a
  save holds whole, and 0.8 s for a level of 100 000 entities little changed. A component is any
  type that reflection describes (`Component: Reflect`,
  [ADR-0085](0085-reflection.md)), which lets a component hold what cannot be copied.


The owner took four decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**The world stamps every change.** The world counts *ticks*, a number that grows as it changes.
Each value of a component holds two: the tick at which it was added, and the tick at which it last
changed. A reader keeps the tick at which it last read, and asks what was added, changed or removed
since: building a frame, a save, the editor, physics and the network each read at their own pace,
sharing nothing.

- Mutable access gives a guard that stamps the value when it is written through; a variant writes
  and stamps only a value that differs from the one held, which the network needs. Most code that
  wrote through `&mut T` still compiles, the guard dereferencing to it; a match on the value, or a
  call that takes `&mut T`, writes `&mut *` before the guard.
- Each component's storage keeps a summary, the tick of its last change, whole and per block of
  values, so that a reader skips what did not change without reading it.
- A log of removals tells, with their ticks, which components left which entities, and which
  entities were destroyed.
- A *structure counter* tells when entities were spawned or destroyed, changed parent or name: the
  editor's lists are made again only then.
- The marks of stale transforms ([ADR-0083](0083-transforms-and-the-hierarchy.md)) are kept by the
  same guard. The world keeps its own derived state up to date in the same way: whether an entity
  is hidden by an ancestor becomes a flag that adding or removing `Hidden`, and changing parent,
  update, where `World::is_hidden` walks the ancestors of every entity that places meshes or a
  light, at each frame.

The width of a tick, how a long run never sees one wrap, and the size of a block are set by
measurement when it is built, on the benchmarks of 100 000 entities; the audit estimates 8 bytes
per value, and a save of 100 000 entities of which 1 % changed in about 6 ms where it takes 0.72 s.

**Driven components, and poses that are not entities.**

- An entity marks some of its components *driven*: computed again by the game, an animation or
  physics, so that their value at a given moment does not matter. A save, the overrides of a prefab
  and a level written back leave a driven component as its author wrote it, never its value of the
  moment; the system that drives it computes it again after a load. A driven component that no
  author wrote, on an entity that a run created or added to one, is saved as it is: nothing else
  gives it, and its system computes it again all the same. The mark is not inherited, and it is
  itself authored: a scene file holds it, and the editor sets it. A component is driven whole: a
  game keeps what a player changes, a lamp switched off, in another component than what it drives,
  the lamp's flicker. The mark replaces `unflicker`.
- A skeleton's pose is not made of entities: it is one component of flat arrays, which saves never
  hold. A skin's joints are not entities: the importer keeps the skeleton with the entity of its
  mesh. What hangs on a bone holds an *attachment*, a component that names a bone of an entity,
  which saves and scene files hold as any reference.

**Hooks, messages and deferred changes at Stage 7.** Nothing public before Stage 7: the ticks above
answer what the editor and saves ask. Stage 7 designs them with their users, physics, audio and the
events of games (contacts, entering and leaving zones). They are hooks per component type, called
when a component is added or removed and when an entity is destroyed; messages buffered and read
at fixed phases of a frame; and a queue of deferred changes that several threads fill during a
traversal, applied at a fixed point, through which a hook that changes the world goes.

**Components that clone, and a copy of the world with its first user.** Every component can be
cloned: `Component` requires `Clone` beside `Reflect`. What cannot be copied, a physics world,
objects of the GPU, open files, lives outside the components, in state kept beside the world, as
ADR-0107's presentation keeps the drawing's, and as Stage 7's physics will keep its own; a
component holds a handle to it. A copy of the world and its restore, type by type and without
serialization, keeping identities and ticks, are built with their first user, replays, tests or
Stage 10b's prediction, and measured then.

**When it is built.** At the start of Stage 6, before the editor's hierarchy and inspector: the
ticks, the guard, the summaries, the log of removals, the structure counter, the flag of hidden
entities and `Component: Clone`, which touch the same code, in about one milestone. The driven mark
before the editor writes levels, at Stage 6. Hooks, messages and the deferred queue at Stage 7, and
the rule of poses there too, with the importer of skins and the core of skeletal animation.

## Alternatives considered

Change detection:

- **Nothing now, the structure counter alone for the editor**: saves, frames, physics and the
  network would keep reading the whole world, each system with its own detection; the shape of
  mutable access would change later, against more code of games.
- **Lists of the slots changed, per storage**, as transforms have them: a cost that follows the
  changes, close to the code of today, no signature changed; but readers at their own pace need
  lists kept and a cursor each, threads that write contend for one list, which Stage 7's parallel
  systems would meet, and reaching a value still counts as a change.

Driven state:

- **`transient` and `Unsaved` as they are**: every game writes its own `unflicker`; a skeleton
  marked `Unsaved` loses what hangs on it, the sword in a hand, and every reference to a bone; the
  editor shows as overridden what an animation moves while it plays.
- **Bones as entities with the same mark**, as Bevy has them: one model, which generic tools
  inspect, but each bone pays an entity's memory, identity, save and share of the transforms, and
  deep hierarchies cost more per entity: hundreds of MB and milliseconds per frame for the crowds
  of the games that ADR-0098 aims at.

Hooks and deferred changes:

- **Designing them now**: nothing uses them before Stage 7, neither physics, audio nor parallel
  systems, and an API designed without its users goes against the rule of a second concrete use.
- **Never hooks, only questions of what was added or removed since**: everything stays ordered,
  but a system sees an event only when it next runs, and an invariant that must hold at once, an
  index by name, cannot wait.

A world that copies:

- **Nothing now**, copies through a save or reflection: about 0.8 s at 100 000 entities, which
  prediction cannot build on, and requiring `Clone` later breaks every component that games wrote
  meanwhile.
- **Rollback designed now**, copies of what is predicted only, copies by difference, a history of
  copies: no network nor physics exists yet to choose among them.

## Consequences

- Every value of a component costs a few bytes more, and every write through a guard a little
  more; the figures are measured when it is built. Reading what changed costs what changed and the
  summaries, no longer the world.
- `get_mut` and `query_mut` give guards: most code that wrote through `&mut T` compiles still; code
  that matched on the value, passed it as `&mut T` or named that type changes.
- A component that held what cannot be cloned moves it into a system's state; every component
  derives or implements `Clone`, the examples of the documentation included.
- A game no longer resets what it drives before it saves: it marks it driven, and keeps what a
  player changes apart from it.
- A rigged model's bones are not entities once the importer reads skins; until then, the importer
  warns that it ignores a file's skins, and keeps each node as an entity.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6 builds change detection, the driven mark and
  components that clone; Stage 7 builds hooks, messages and the deferred queue with physics.
