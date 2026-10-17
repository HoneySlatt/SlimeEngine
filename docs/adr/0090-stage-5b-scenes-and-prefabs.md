# 0090. Stage 5b: scenes and prefabs

- Status: Accepted; amends [0078](0078-stage-5-in-two-halves.md) (Stage 5b is released as 0.4)
- Date: 2026-10-17

## Context

Stage 5a is done, and released as 0.3 with Stage 4b on 2026-10-17: *Hunna Wood* runs on the
engine's world, whose components are listed and edited by name, and saves and resumes its runs.
Stage 5b, *Scenes and prefabs*, comes next ([ADR-0078](0078-stage-5-in-two-halves.md)). Its exit, in
the roadmap: the game's scenes round-trip through save and load as files; prefabs instantiate with
overrides; the sample game's zone is made of prefabs.

ADR-0078 releases Stage 5b with 0.4, the release of the editor that authors scenes, and says that
when 5b is released is reconsidered if Stage 6 turns out long. Stage 6 is the broadest stage of the
roadmap: the editor's interface, its viewport, hierarchy, inspector and asset browser, undo and redo
of every edit, and play in the editor. Stages 3, 4 and 5 were each split once planned.

Five design studies of the code preceded the plan: scene files and their cooking; prefabs and
overrides; scenes at run time, saves and hot reload; the art pipeline and the zone; the shape of the
stage. [ADR-0089](0089-scenes-and-prefabs.md) records the design that the owner chose from them. The
studies also found two faults that Stage 5a left:

- A scene spawned while a game runs makes the next save one that loading refuses: its entities,
  which the start world lacks, are written as created entities under their scene's identities, and
  loading creates only `created/<n>` ones. No game spawns a scene while it runs yet.
- *Hunna Wood*'s hot reload compares the layout of the zone reloaded with the layout of the running
  world, where [ADR-0088](0088-saving-and-resuming-a-run.md) says that it uses the fingerprint of a
  new game's zone: a collider changed through the world panel makes the next reload start the run
  again.

The zone today: one glTF scene exported by a Blender script, 543 nodes, of which six lanterns made
of a post, a lantern, a spot light and a collider, about 273 trees, rocks, ferns and logs scattered
from a fixed seed, a campfire, the chapel's door and the key. 196 of its nodes carry the number that
Blender adds to a name already taken, `.001`, which follows the order in which the script creates
them; the save fixture names one of them.

## Decision

The owner took these decisions on 2026-10-17, one by one, when the stage was planned.

**The exit, made precise.** Three round trips: the text of a scene file read and written again gives
the same bytes, and its binary encoding reads to the same values; a world written back as a scene
file, then spawned again, is the same world, its placements kept as placements; the saves of a run
load on a world made of prefabs. Prefabs instantiate with overrides and follow the changes of their
prefab, nested, where an override or the run changed nothing; an override whose target is gone is
reported. *Hunna Wood*'s zone is made of prefabs, and plays as before.

**The order: the risk first.** Seven milestones:

1. Scenes placed in a world: the model of ADR-0089, its overrides and nesting, its errors, and the
   cost of spawning.
2. A world written back as a scene, its placements kept.
3. Saves and hot reload with placements: what a game spawns while it runs, saved as placements, and a
   run carried through a save when its scenes change.
4. The binary encoding of documents, which does not depend on the first three.
5. Scene files cooked, typed components on Blender's nodes, and prefabs in the `world` example.
6. *Hunna Wood*'s zone made of prefabs.
7. A check on Windows, the documents reread, and the release of 0.4.

**The zone made of prefabs.** The lantern becomes a prefab, exported from Blender apart from the
rest, its glass apart from its body so that a placement can remove it. A scene file written by hand
places the land, the rest of today's zone exported as one glTF scene, under `land`, and the lantern
six times: the third lantern lights less, and the fifth is broken, its light and its glass removed.
The lanterns' places move from the script's placements to that file, which Stage 6's editor will
edit. The trees, rocks, ferns and logs stay in the land, where the script scatters them: they hold
no data of the game, each is still an entity with an identity, and an override on a tree named by
the scatter's numbering would land on another tree once the scatter changes. The colliders, markers
and times of day become typed components on the nodes, and the game's code that read them from
untyped extras goes. The export script names every node, and fails when Blender renames one. A
change of the lantern in Blender, exported again while the game runs with `--watch`, changes every
lantern but the third's light and the fifth's missing parts.

**The save fixture.** The zone's identities change, and so does its layout: the fixture is made again
by playing, in the commit that changes the zone, as ADR-0088 says. The fixture of 0.3 is kept, with a
test that the game sets it aside as a save of another layout, without a panic, and starts a new run:
what a player of 0.3 would see. After the change, `zone/lantern 1` names a lantern's placement where
it named its light, and only the fingerprint of the layout keeps a save of 0.3 from changing the
wrong entity; the test checks it.

**The `world` example** shows prefabs written by hand in scene files: a yard places the sample scene
and four lamps, a prefab that reuses the sample's meshes, one lamp red and one without its light; it
writes its world back when it quits and fails if the file reads back otherwise, and follows a change
of the lamp with `--watch`. The sample scene, the `scene` example and the image tests do not change.

**The world panel** shows each entity's persistent identity and, for a placement, its prefab.
Marking which values a placement overrides is the work of Stage 6's inspector.

**Measurements.** The first milestone measures the cost of spawning in a release build: the zone,
which should take a few milliseconds, a thousand placements, and a level of 100 000 entities. If the
largest is too slow, the remedies, in order, stay inside the world (ADR-0089).

**Release.** Stage 5b is released as 0.4 at its close, and the editor of Stage 6 becomes 0.5: scene
files serve without the editor, as *Hunna Wood*'s are written by its script and by hand, and a game
that follows releases gets them sooner than after Stage 6. Every later release moves by one: Stage 11
gives 0.11, then 1.0. This amends ADR-0078.

**The asset mirror.** Exporting the zone again needs a release of the mirror of the game's source
assets, about 6 MB, at the zone's milestone; the owner agreed to it when the stage was planned
([ADR-0070](0070-the-sample-games-source-assets.md)).

**CI.** Each milestone that adds a crate or a dependency records how long the two jobs take, and the
levers of [ADR-0081](0081-stage-5a-the-world.md) stay: the cooked zone cached once the Windows job
passes 19 minutes, the Windows job split past 20.

**Dependencies.** None.

**Known debt.** The two faults above are fixed by the third and the sixth milestones. The tests that
edit every leaf of every component, written twice, are written once if the stage needs a third copy.
The Blender names that change with the order of creation go with the zone's milestone.

**Out of scope**: variants and former identities (ADR-0089); updating a world in place on hot reload,
the editor's hierarchy, inspector, marks of overridden values, applying and reverting overrides,
making a prefab of a selection, undo and redo, and saving a level from the editor (Stage 6); systems
declared in scenes, and physics shapes built from the colliders (Stage 7); scripts that place
prefabs, and placements through the C ABI (Stage 8); values of materials set per placement (Stage
9b); flattening placements when cooking, streaming scenes as cells, and spawning on other threads
(Stage 10); tools that migrate scene files (Stage 11); binary saves; the cooked glTF scene moved onto
the binary encoding; the vegetation as placements; a scene file for the hero.

**The roadmap.** Stage 5b is released as 0.4, Stage 6 as 0.5, and each later stage one release
later; its items name what ADR-0089 decides.

## Alternatives considered

- **Stage 5b released with the editor, as ADR-0078 says**: one story, scenes and the editor that
  makes them, but 5b unreleased for the length of Stage 6, likely fifteen milestones or more.
  **Deciding at 5b's close, once Stage 6 is planned**: what ADR-0078 says literally; nothing gained
  by waiting, since publishing costs little.
- **Every object placed more than once made a prefab**, lanterns, oaks, conifers, rocks and ferns,
  about 250 placements in a scene file that the script writes from its scatter: the scale of the
  model tested on the game, but the file would change with every scatter and would have two owners,
  the script and Stage 6's editor. The scale is measured by the first milestone instead. **The
  campfire made a prefab too**: one use, whose place also shapes the path and the clearing in the
  script, so that it would be written twice.
- **The zone as a variant of the land**, keeping `zone/door`: only one exported file can be a base,
  and no other scene needs variants (ADR-0089).
- **The fixture of 0.3 kept resuming through former identities**: proof that saves outlive a change
  of the scenes, but the zone bent to match 0.3, its colliders in the order of 0.3 and its lanterns
  named apart from their former lights, for a feature that no player needs yet.
- **Six milestones, saves folded into the first**: the saves of placements spawned while running
  and the run carried through hot reload, which the owner chose, make a milestone of their own.
- **Updating the world in place on hot reload, now**: about one milestone more (ADR-0089).

## Consequences

- The plan shows Stage 5b, seven milestones; Stage 5a joins the completed stages.
- The owner tries the model in the `world` example (fifth milestone), plays the zone made of prefabs
  and changes its lantern while it runs (sixth), and plays the release's Windows programs (seventh).
- The zone's identities and its save fixture change once, in one commit, with a release of the asset
  mirror.
- 0.4 carries scenes and prefabs; the release map of the roadmap moves by one from 0.5 on.
- Each milestone that designs a part records its design in an ADR of its own.
