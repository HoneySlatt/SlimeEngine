# 0167. The hierarchy

- Status: Accepted; amends [0108](0108-what-the-world-knows-of-its-changes.md) (a rename moves a
  tick of the names, not the structure counter)
- Date: 2026-10-29

## Context

[ADR-0155](0155-the-editors-design.md) decided that the hierarchy is a list of the rows that show,
which the editor flattens from the world's children, with the roles of a tree and rows that are
selected; that what is folded is the user's, never the level's; that a search names a component
with `c:` and a prefab with `p:`, and runs off the frame or after a pause; and that the world gains
a tick of its entities' names apart from its structure, which the hierarchy follows and which spares
the dev overlay's world panel its list made again at each rename. Milestone M6c.5, which was to
bring the hierarchy, docking's regions and the inspector, was cut in four at its start, on
2026-10-29, as ADR-0155 allows; this ADR records the first part, M6c.5a: the hierarchy that finds,
selects and renames. Rearranging it, by drags and keys, and its eye and lock come with M6c.5b.

What the code held before this milestone:

- `slime-ui`'s list lays its rows out itself and builds only those that show, by their numbers
  ([ADR-0140](0140-the-tree-of-elements.md)); a program marks a row selected through its state.
  Neither a tree's roles nor a row's level existed, nothing scrolled an element into view, and the
  focus that Tab moves stayed where a scroll left it.
- `World::structure` moved at each spawn, destruction and change of parent, and at each change of a
  name: the dev overlay's world panel, a flat list of the world's entities with a filter by name,
  made its list again at each rename, some 1.5 ms on a world of 100 000 entities
  (`docs/measurements.md`, the study of ADR-0155).
- The editor showed that world panel, its list and the components of the entity selected, in the
  left column of its window, above the history.

## Decision

**A tick of the names.** `World::names` gives the latest tick at which an entity was given, changed
or lost its `Name`; `World::structure` no longer counts names. The world panel makes its list again
when the structure moves, and, when the names move, reads again the names of the entities whose
`Name` changed since (`World::changed`).

**A tree's roles.** `slime-ui` gains the roles `Tree` and `TreeItem`, and a level for an element
(`Element::level`), from 1 for the tree's top rows, which agents read and AccessKit gives screen
readers; an item that holds others is expanded or not, through the state that sections already
have. The list stays the element that a tree is: the program flattens its tree into rows, which no
element of `slime-ui` knows how to walk.

**Into view.** `Ui::show_row` scrolls a list so that a row shows, by its number, whether built or
not; `Ui::scroll_into_view` scrolls the scrolling elements around an element built so that it shows
whole, as far as each can; the focus that keys move is scrolled into view so.

**The hierarchy.** A panel of the editor, `slime-editor`'s own, takes the place of the world panel's
list, above the components of what is selected, which the world panel keeps showing alone until the
inspector replaces it. Its rows are the world's entities, depth first from its roots, each parent's
children in their order, each indented by its depth, with a mark that folds or unfolds those that
have children, its name, or its handle without one, and a mark for a placement's root. The editor
flattens them again only when the structure or the folds change, and builds only the rows that
show, their names read as they are built, so that a rename builds again the rows in sight alone.
What is folded is the user's, kept in memory while the level is open: the level opens with its
root's children shown.

A click selects the row's entity alone; with Ctrl, it adds it to what is selected or takes it out;
with Shift, it selects the rows from the last one clicked to this one. The arrows move the
selection to the row above or below, Left folds or goes to the parent, Right unfolds or goes to the
first child, while the list holds the focus. A selection made elsewhere, in the viewport, by an
undo or an agent, unfolds the ancestors of what it selects and shows its first row.

**The search.** A field above the rows finds entities by their names, any case, `c:` followed by a
text keeping those that hold a component whose type's name holds it, and `p:` those that place a
prefab whose name holds it; terms separated by spaces must all hold. While it holds text, the rows
are the entities found, a flat list, each with the path of its parent. It runs once the typing
paused, an eighth of a second, over the whole world at once, which the benchmark measures.

**Renaming.** F2 (`edit.rename`) turns the row of the entity selected into a field that holds its
name, which takes the focus: Enter, or the focus leaving it, as a field commits, makes the rename
one step of the history, Escape gives it up. Agents rename through the same field, found by its
name, `rename`.

## Alternatives considered

- **A tree element in `slime-ui`, which walks the program's tree**: an interface that knows a
  program's model, and a second walk of the world each frame; the flattened rows are the editor's,
  and the list already builds only those that show.
- **The search on a thread of its own**: the world cannot be read from another thread while the
  editor changes it, and a copy of 100 000 names costs what the search would; after a pause, on the
  editor's thread, measured, suffices until a world says otherwise.
- **The folds kept in the user's state directory, or in the level**: ADR-0155 makes them the
  user's, never the level's; keeping them across starts waits for a user who asks, the level's
  root's children shown at each opening meanwhile.
- **The tree's search keeping the ancestors of what it finds, dimmed**: rows that the user did not
  ask for, among which what was found is lost in a large world; the flat list with each parent's
  path names where each is.
- **The hierarchy in `slime-devui`, for every game's dev overlay**: a game's overlay lists the
  entities to find one; the editor's hierarchy selects many, renames and rearranges, as the editor
  alone does.

## Consequences

- A rename costs the world panel and the hierarchy the rows in sight, not a list of the world.
- Screen readers read the hierarchy as a tree, each item with its level.
- What is folded is lost when the level closes.
- The search runs at once over the world: a world in which it takes more than a frame waits on it.
