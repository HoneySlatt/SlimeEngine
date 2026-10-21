# 0124. Driven components, start keys and created entities written into levels

- Status: Accepted; amends [0087](0087-the-worlds-documents.md) (layout 4 of saves),
  [0101](0101-saves-and-identities-that-never-mislead.md) (the fingerprint of a start's creations
  no longer written), [0108](0108-what-the-world-knows-of-its-changes.md) (the driven mark is a
  component, and its first user), [0109](0109-stable-references-and-undo.md) (how a created entity
  written into a level is carried once) and [0116](0116-stage-6-in-two-halves.md) (M6a.7 in two
  milestones); amended by [0125](0125-saves-per-scene.md) (the driven mark read at the base)
- Date: 2026-10-21

## Context

Three decisions wait to be built before the editor writes levels
([ADR-0116](0116-stage-6-in-two-halves.md)):

- [ADR-0108](0108-what-the-world-knows-of-its-changes.md): an entity marks some of its components
  *driven*, computed again by a system, so that saves, the overrides of prefabs and levels written
  back leave them as their author wrote them. Its first user was to be *Hunna Wood*'s fire, which
  flickers in the frame since M6a.5 and no longer writes the world
  ([ADR-0121](0121-a-game-as-a-simulation-and-a-presentation.md)). The `world` example's ring
  floats at each fixed step: its save and its yard written back held its place of the moment.
- [ADR-0109](0109-stable-references-and-undo.md): what a start creates in code holds a key or is
  `Unsaved`, else saving refuses, naming the entities; and a created entity that a scene written
  back holds is carried once, known by the ID that the world gave it. The audit of 0.4 showed a
  torch, created under a zone and written into it, twice after the run was carried into the file
  cooked again.

M6a.7 held these and the saves per scene of
[ADR-0112](0112-levels-saves-and-worlds-for-open-worlds.md). The owner split it in two at its
start, on 2026-10-21, as the plan allowed: M6a.7a builds these three, M6a.7b the saves per scene
and adoption. The owner chose the same day to build the driven mark with the `world` example's
ring as its first user.

## Decision

**The driven mark is a component, `Driven`, that lists the types of its entity's driven
components by their names**: `Driven = ["Transform"]` in a scene file, as any component. It is
authored, as ADR-0108 asks: a scene file holds it, a prefab's override adds it, the editor will
set it as any value, and a save holds it as any component. It is not inherited.

- A save writes no change of a component that its entity marks driven in the start, as its author
  marked it, and still in the run: loading leaves it as the start has it, its author's, and the
  system computes it again. A mark that the run adds or takes off hides nothing: a crate that
  physics takes over during the run is saved where it fell, and one that it lets go where it
  settled. A driven component that the run added, or of an entity that the run created, is
  saved whole, as it is: no author wrote it.
- A scene file written back writes a driven component as its author wrote it: as the scene's
  record of the entity holds it, or for a placement as the prefab gives it, with the override that
  the scene wrote for it, if it wrote one. One that no author wrote is written as it is.
- `World::first_difference`, which tells whether a world loaded from a save is the world saved,
  leaves out the values of the components that the first world marks driven, which a system
  computes again.
- The `world` example's yard marks its ring's transform driven: its save and its yard written back
  keep the ring where the file puts it.

**Saving refuses a start that created, without a key, entities that saves hold.** `World::save`
returns `DocumentError::UnkeyedAtStart`, which names each entity by its identity and its name, and
says to create it with `World::spawn_keyed`, a prefab with `World::spawn_scene` under a key, or to
mark it `Unsaved`. The fingerprint of what a start created, which ADR-0101 added, is no longer
written, as ADR-0109 said: nothing that a start created is named by a save any more. It is still
read in the saves of layout 3, which name numbers of such entities. The tests make such saves
through the writing behind `World::save`, without its check.

**A created entity written into a level is known by the identity that the file gives it.**
`World::write_scene` gives an entity created outside a scene, or the root of a placement made
while the world ran, an ID of the file, `torch-0rdx`, as before, and now remembers the identity that
it takes once the file is spawned under the scene's key, `zone/torch-0rdx`. A save writes it with
the entity, or with the placement, `written = "zone/torch-0rdx"`. Loading, and so carrying a run
through hot reload, looks for that identity once the scenes that the save spawns are spawned, a
level that the run changed to among them; when the file was cooked again and spawned, the world has
it:

- the save's created entity is that one, given what the save holds of it whole, its parent, its
  components, those that the save does not give it removed;
- the save's placement is the file's placement of the same prefab, not placed again, and the
  differences that the save holds of it and of its entities reach the file's;
- every name that the save gives them, references and parents included, reaches them.

Else the entity is created, or the prefab placed, as before, and keeps the identity that the file
will give it, which its next save says, and the next file written gives again. Each entity of the
world is taken once. The save's layout becomes 4; the saves of layouts 1 to 3 are read as they
were.

## Alternatives considered

The driven mark:

- **A flag on each component's value, outside the components**, kept by the world beside their
  ticks: no component to write, but a second kind of data for the editor, scene files and saves to
  read and write, where a component is read and written already.
- **Driven fields rather than components**: finer, but ADR-0108 drives a component whole, and a
  game keeps what a player changes in another component.
- **Waiting for Stage 7's animation and physics**: the editor of Stage 6b would write levels
  without it, against ADR-0108, and the ring's place would stay in saves.

The created entity written into a level:

- **The entity renamed by its ID when the file is written**: ADR-0092 rejected it, since a save
  made before would no longer find it.
- **Matching the file's entity with the run's by name and place when carrying**: guesses, which
  two torches of one name defeat.
- **The identity kept only by the world, not in saves**: carrying is a save and a load, so the save
  must say it.

## Consequences

- A game that drives a component marks it, in its scene files or when it spawns the entity, rather
  than putting it back before it saves: what `unflicker` did.
- A game whose start creates entities without keys fails at its first save, with the names to fix.
  The engine's tests that saved such starts give their entities keys, or make their saves through
  the writing without the check.
- A level written back, cooked and carried gives the file's entity, or placement, once; the run's
  values of it win over the file's, as for any entity that the run created, a placement set back
  to its prefab before the run's differences apply.
- An entity that the run created, wrote into the level and destroyed since comes back from the file
  when the run is carried into it: the save no longer names it. The plan's known debt lists it.
- The written identity is a field of the save's created entities and of its placements made while
  the world ran; M6a.7b's saves per scene keep it or replace it.
