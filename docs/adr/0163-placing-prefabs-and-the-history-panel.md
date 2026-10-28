# 0163. Placing prefabs and the history panel

- Status: Accepted
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided that the user places prefabs, "Place prefab…" from
the manifest's scenes, and that one history per level is shown in a panel. Milestone M6c.3b, which
was to bring them with creating and arranging, was cut in two at its start, on 2026-10-28, as
ADR-0155 allows; this ADR records the second part, M6c.3c.

What the code held before this milestone:

- The world places a prefab by a command, `Command::Place`, which names the prefab by its asset ID
  and the placement's root by a new identity of the level ([ADR-0134](0134-the-editors-core.md)).
- The cooked data's manifest lists every cooked asset by its ID, with its kind and the source file
  that it was cooked from, for messages ([ADR-0069](0069-the-asset-pipeline.md)); glTF scenes and
  scene files are both scenes that a level can place.
- The editor's core keeps a history of the steps done and undone, which its window shared but did
  not show, and which agents read through the development channel
  ([ADR-0135](0135-the-editors-program.md)).
- The palette of the interface finds commands by what the user types, and gives the one chosen to
  the program ([ADR-0151](0151-rich-widgets.md)).

## Decision

**Placing a prefab.** *Edit* > *Place a prefab…* (`edit.choose_prefab`) opens the palette on the
prefabs that the level can place, listed as it opens: the cooked data's scenes and scene files,
the level itself left out, each named by its source file, in their order, and the scenes of one
file, as a glTF file of several, by the file and the scene's name, `props.glb: tree`; without one,
it tells the user so. The one chosen runs *Place a prefab* (`edit.place`), whose one argument names
the prefab by its asset ID or by its name, `lamp.scene`, which an agent gives: one step of the
world's commands that places it at the level's root, under a new identity named after its file,
its root at the middle of what is selected, or at the world's origin, not turned nor scaled, and
selects it. A prefab that places the level, at any depth (`ResolvedScene::places`), is refused: the
level would hold itself, and could not be opened again.

**The history panel.** Below the world panel, a panel of a fixed height shows the history: a row
*Before the first step kept*, then a row for each step done, the oldest first, then for each step
undone, the next to do again first, those dimmed; the row of the step that the level stands at is
marked. Its rows are built again only as the steps done or undone change, not at each frame of a
drag. A press on a row brings the level there, undoing or doing again each step between, as
*Go to a step of the history* (`edit.go_to_step`) does, whose argument is the count of steps done
after the first kept, from 0; each is journaled as an undo or a redo.

## Alternatives considered

- **A browser of assets, with thumbnails**: ADR-0155 puts the asset browser in a later milestone,
  *Build from a kit*; the palette finds a prefab by its name meanwhile.
- **The prefab placed where the pointer points in the viewport**: a ray into the level from a
  person's pointer, which an agent does not have, and a placement off the screen when the pointer
  is over a panel; the middle of what is selected, which the user selects, places it where they
  look, as creating an entity does ([ADR-0162](0162-creating-copying-and-arranging.md)).
- **The prefab's root turned and scaled as its prefab turns it**: the root's transform read from
  the prefab before it is placed, which the editor does not resolve itself.
- **Going to a step as one step of the history**: a step that undoes others would make the history
  a tree, which ADR-0134 keeps a line.
- **The history as a dock of its own, or a floating panel**: docking comes later; the column of
  the world panel holds it at a known place.

## Consequences

- An agent places a prefab and goes through the history by the operations' names, as a person does
  from the palette and the panel.
- A prefab whose root its prefab turns or scales loses that turn and scale where it is placed, its
  root given a transform of its own.
- The history panel builds a row for each step that the history keeps, which a long session makes
  many; a list that builds the rows that show, as the world panel's, waits for a history that needs
  it.
