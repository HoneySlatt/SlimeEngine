# 0078. Stage 5 in two halves

- Status: Accepted; amends [0068](0068-stage-4-in-two-halves.md) and
  [0072](0072-stage-4b-live-assets.md) (where 0.3 is released); amended by
  [0090](0090-stage-5b-scenes-and-prefabs.md) (Stage 5b is released as 0.4)
- Date: 2026-10-16

## Context

The roadmap's Stage 5, released as 0.3, gives the engine a world model: reflection and a type
registry, serialization with format versioning, an entity model with components and a transform
hierarchy, a world that keeps what is drawn and where what moves was, the precision of world
positions, worlds, scenes and prefabs, entity lifetime and stable references. Its exit: the game's
scenes round-trip through save and load; prefabs instantiate with overrides; any component's
properties can be listed and edited generically.

Five design studies of the code preceded the plan. Planned with the game that the stage grows,
*Hunna Wood*, the stage came to twelve or thirteen milestones, more than any stage so far: Stage 4
was split at about fourteen ([ADR-0068](0068-stage-4-in-two-halves.md)). It holds two kinds of
work:

- the world at run time: entities and components, the hierarchy, drawing what the world holds,
  reflection, serialization, and saving what changed in a run;
- authoring: scenes written as files, and prefabs placed many times, each placement overriding
  some of the prefab's values.

The second builds on the first: an override is a value set through a reflected property path, on
an entity whose identity is stable. The engines the studies surveyed took several releases to
settle their overrides: Unity added nested prefabs in 2018, thirteen years after its prefabs, and
Bevy its scenes made of patches in 2026, six years after its first scene files.

Stage 4b's work waits for 0.3 (ADR-0068, ADR-0072), which the roadmap's release map describes as
"reflection, serialization, world".

## Decision

The roadmap's Stage 5 is split in two halves, as Stage 4 was: a reference to Stage 5 means both.

- **Stage 5a, The world, released as 0.3**, with Stage 4b's work: the engine's own entity model,
  with components and a transform hierarchy; the precision of world positions; a world that keeps
  what is drawn and where what moves was; reflection and the type registry, with a derive macro;
  serialization with format versioning; entity lifetime and stable references; saving what changed
  in a run, and loading it back. Exit: *Hunna Wood* runs on the world, which keeps what is drawn
  and where what moves was; any component's properties can be listed and edited generically; a run
  of the game round-trips through save and load.
- **Stage 5b, Scenes and prefabs, released with 0.4**: scenes as source files, cooked like other
  assets; prefabs with overrides, which follow the changes of their prefab; the sample game's zone
  made of prefabs. Exit: the game's scenes round-trip through save and load as files; prefabs
  instantiate with overrides. Stage 5b is planned with the owner at the end of Stage 5a, and has no
  release of its own, as Stage 4b.

The owner chose this on 2026-10-16.

## Alternatives considered

- **One stage of twelve or thirteen milestones, released as 0.3 at its end.** No change to the
  roadmap, but the longest stage so far, prefabs designed before a world exists to try them on, and
  Stage 4b's work unreleased for a dozen more milestones.
- **Another cut: reflection, serialization and the world in 5a, scenes, prefabs and saves in 5b.**
  Serialization would have no user in 5a but tests, and 5a would end with nothing new to play.
- **5a without serialization**: 0.3 would contradict its line of the release map.

## Consequences

- 0.3 comes at the end of Stage 5a, with Stage 4b's work. ADR-0068's "its release waits for Stage
  5's" and ADR-0072's "released with 0.3, at the end of Stage 5" now mean the end of Stage 5a.
- Stage 5b's scenes and prefabs ship with 0.4, the editor's release, which authors them; if Stage 6
  turns out long, when 5b is released is reconsidered then.
- The roadmap's Stage 5 becomes 5a and 5b, each with its exit, and its release map gives 0.3 to
  Stage 5a and 5b to 0.4.
- Saves in 5a and overrides in 5b share their mechanism: a value set at a property path of an
  entity. 5a builds it for saves; 5b adds files and prefabs on it.
