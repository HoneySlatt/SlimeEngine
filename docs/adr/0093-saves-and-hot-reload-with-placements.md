# 0093. Saves and hot reload with placements

- Status: Accepted; amends [0087](0087-the-worlds-documents.md) (a save says its layout, holds what
  the world spawned while it ran, and is loaded through a scene library); amended by
  [0101](0101-saves-and-identities-that-never-mislead.md) (a change of level, and the numbers of what a run spawned)
- Date: 2026-10-18

## Context

[ADR-0089](0089-scenes-and-prefabs.md) decides how saves treat what a game spawns while it runs. A
prefab spawned from code, `created/<n>`, is saved as a placement of its prefab with what differs
from it. Loading spawns the prefab as it is then, under the same number, and applies the
differences, so that a fix of the prefab reaches the saves made before. A prefab that the game no
longer has is set aside and reported, with what was under it. A scene spawned under a key during a
run is saved the same way. A save records a fingerprint of what each of them resolves to, prefabs
included. The save's format takes a new version, and the saves of 0.3 still load. On hot reload, a
game carries its run through a save: it saves against its old start world, makes a new start world
from the new files, and loads the save into it. [ADR-0090](0090-stage-5b-scenes-and-prefabs.md)
gives all of it to M5b.3, in one call for the carrying.

Where the engine stood after [ADR-0092](0092-a-world-written-back-as-a-scene.md):

- `World::save` wrote an entity that the start world lacks as created, whole, under its identity.
  `World::load` creates only `created/<number>` entities, and refused such a save: a scene spawned
  during a run, or a placement made with `World::spawn_placement`, made the next save one that
  loading refuses, which the known debt recorded.
- The world kept the scenes that it spawned under keys, even once their roots were destroyed. A
  placement made while it runs was remembered on its root only.
- `World::load` took the text of a save alone: it had no way to spawn a scene.
- A save said nothing of its own layout. Every section was optional, and an unknown one was left
  unread.

## Decision

**A save says its layout.** A save begins with `about`, then `layout = 2`, the version of its
sections. A save without it is of layout 1, as 0.3 wrote it, and is read as it was. A save of a
later layout is refused, at its line. A scene file names its layout by the same key.

**What the world spawned while it ran is a section of its own, by its root.** `spawned` is keyed by
the persistent identity of each root: a key under which the start world spawned no scene, or another
scene, as when a run changes level, or `created/<number>` for a placement made while the world ran.
Each entry gives its scene's asset ID, which decides, its name, for messages, and its fingerprint,
which covers its prefabs:

```toml
layout = 2
next_created = 3

[scenes]
yard = { name = "yard", content = "e6c1ff1cb67bf1be" }

[spawned]
"created/1" = { scene = "00000000-0000-0000-0000-000000000002", name = "lamp", content = "f4206aba0c870e7c" }
village = { scene = "00000000-0000-0000-0000-000000000003", name = "village", content = "acf4dbb10f49ffef" }

[removed]
entities = ["created/1/glass"]

[parents]
"created/1" = ["yard/porch"]

[changed."created/1/light"]
"test::Glow" = { intensity = 5.0 }

[created."created/2"]
parent = ["created/1/post"]
```

`scenes` now holds the scenes that the start world spawned and that the world still has under their
keys, which loading compares, as before. A scene is the same when its asset ID is: what changed of
its content is what its fingerprint says. An
entry of `spawned` is written while an entity of it is saved: a placement whose entities all hold
`Unsaved`, or are under one that does, is not written.

**Its entities are compared with what its scene spawns.** Each entity of a scene spawned while the
world ran is compared with the record that spawned it, as an entity of the start world is compared
with the start world's entity of its identity. The comparison takes the record's values, with
their references made references to the entities spawned from the records, and its parent. The
root of a scene is named after its key, and the root of a placement is not named. The differences
go in the sections that hold the start world's: entities removed, parents, components removed,
added and changed. A placement's root is spawned at the world's root, and its parent is a
difference like any other.

**Loading reads all that can refuse the save first, then spawns, through a scene library.**
`World::load(text, &mut SceneLibrary)` reads first, before the world changes, all that can refuse
the save: its layout, the identities of what it spawns and of the entities that it creates, its
count, its scenes, what it removes and the parents. Then it spawns each entry of `spawned`, before
the entities that the save creates and before any value is read, so that a reference can name its
entities. What comes after can only be set aside: a refused save leaves the world as it was, with
nothing to undo.

- **How it spawns.** Each scene is resolved as the library gives it now, under the identity that it
  had. A placement goes under its number, and the world's count stays above it.
- **What refuses the save.** A number that the world gives already, or that the save gives twice,
  refuses the whole save, and so does an identity that no world gives, as they do for the entities
  that a save creates.
- **What is set aside.** A scene that the library cannot give or resolve is set aside and reported
  with its reason. So is one whose values its types refuse. Its entities' differences are set aside
  with it, as their entities are missing. An entity that the save creates under one of its entities
  is left out of that parent, as ADR-0087 does.
- **A key under which the world spawned the same scene already**, because the start world now
  spawns it itself, is set aside too. Its entities take the save's differences.
- **A key under which the world spawned another scene**, as when a run changed level, has that
  scene destroyed first, with what is under it. The save removes its entities, and nothing is
  reported of them.
- **How much it spawns.** A save spawns at most `MAX_ENTITIES` entities so, as a scene holds at
  most as many. The scenes past it are set aside: a short save cannot make loading spawn without
  end.
- **What is named.** The scenes whose fingerprint changed since are named, these roots included.

**The world keeps a placement's prefab while an entity of it lives.** The world keeps the prefabs
of the placements made while it runs, by their numbers. A save then still writes a placement whose
root was destroyed while entities of it live on, moved elsewhere. Placements of which nothing lives
are forgotten, swept when the count of those kept has doubled since the last sweep, so that a game
that spawns and destroys prefabs all the time holds no growing list. The scenes spawned under keys
stay, as ADR-0087 has them.

**A run carried through a save, in one call.** `World::carry(old_start, start, scenes)` saves the
run against `old_start` and loads the save into `start`, the world that the game made from its new
files. A scene or a prefab that changed reaches every entity that it spawned, but what an override
or the run changed of it. Handles to entities change, as when a save is loaded. The world that it
gives is what Stage 6's update in place must give.

**What it costs.** Saving a placement made while the world runs costs about what saving the
entities of a scene costs: each value is compared with its record's. `World::save`'s rustdoc holds
the measurements ([ADR-0053](0053-one-home-per-fact.md)).

## Alternatives considered

- **What the world spawned written whole as created entities**, which loading would accept under
  any identity: no library needed to load, but a fix of the prefab never reaches the saves made
  before, which ADR-0089 rejected.
- **The parent of what was spawned in its entry of `spawned`**: shorter, but a second way of saying
  a parent, beside `parents`.
- **One section for the start world's scenes and those spawned while the world ran**, with a
  marker: loading could not tell a scene that the start no longer spawns from one that the run
  spawned.
- **The prefabs of placements kept forever**, as the scenes under keys are: no sweep, but a game
  that spawns a prefab for every projectile would keep one entry for each projectile ever spawned.
- **A placement forgotten when its root is destroyed**: an entity moved out of it would be saved as
  created under its placement's identity, which loading refuses.
- **A count of the entities alive in each placement**, forgotten at zero: exact, but a step in every
  destruction of an entity.
- **A function that gives a scene by its asset ID**, rather than a library: other sources possible,
  but a second way to resolve scenes beside the engine's library.
- **The save parsed twice**, once to spawn the scenes and once for the rest: spawning simpler, but
  parsing is half of what loading costs.
- **What was spawned destroyed again when the save is refused later**: the reading as it was, but a
  scene destroyed to make room for another under its key could not be brought back as the game
  made it.
- **A save refused when it spawns under a key that the start world spawns now**: a game update that
  moves a scene into the start world would end every run saved before.

## Consequences

- The known debt of the saves that loading refused once a scene was spawned during a run is
  retired.
- Every caller of `World::load` gives a scene library. *Hunna Wood* and the `world` example give an
  empty one, as they spawn nothing while they run: M5b.5 and M5b.6 give them the library that they
  spawn from, and *Hunna Wood*'s hot reload carries its run through a save.
- 0.3 reads no layout: it would load a save of 0.4 without what the run spawned, and set aside the
  differences of its entities.
- Stage 10 can send a placement spawned on a server as a save writes it: a scene and what differs
  from it.
