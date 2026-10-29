# 0168. Rearranging the hierarchy

- Status: Accepted
- Date: 2026-10-29

## Context

[ADR-0155](0155-the-editors-design.md) decided that the hierarchy reparents by drag and drop, which
`slime-ui` gains between its elements, and by Alt and the arrows, and that the editor never undoes
what is hidden, locked or folded, the user's. [ADR-0167](0167-the-hierarchy.md) built the
hierarchy that finds, selects and renames. Milestone M6c.5b, which was to bring the drags, the keys,
the eye and the lock, was cut in two at its start, on 2026-10-29, as ADR-0155 allows: this ADR
records the drags and the keys that reparent, with G32, a debt of the world's walks of every
entity's ancestors; the eye and the lock come with M6c.5c.

What the code held before this milestone:

- `slime-ui` drags what a widget is: a slider's handle, two panels' handle, a floating panel, a
  field of numbers. A press captures an element, which a release over it presses; nothing carried
  one element onto another, and agents had no such action.
- `Command::Reparent` gives an entity a parent and a rank among the parent's children, counted
  without the entity, past the others for none; the world refuses a parent that is the entity or
  one of its descendants.
- G32: three walks of an entity's ancestors run once per entity, so that a hierarchy N deep costs
  N² for what its N entities ask: the search for a cycle as an entity is given a parent, which
  spawning and loading do for each; whether an entity or one of its ancestors holds `Unsaved`, which
  saving, unloading and writing a level back ask of each; and the ancestor that moved during the
  last step, which a frame's interpolation asks of each entity that did not move itself.

## Decision

**Drag and drop in `slime-ui`.** An element made draggable (`Element::draggable`), once pressed and
moved past a few pixels, is dragged rather than pressed; an element made a target of drops
(`Element::drop_target`) under the pointer during the drag is where it would drop. `Ui::dragging`
gives the element dragged, the target under the pointer, if any, and where in the target, from 0 at
its top to 1 at its bottom, which the program shows. Released over a target, the drag gives the
target the action `Dropped`, with the element dropped and where; released elsewhere, or given up by
Escape, nothing. The drag goes on once the element dragged is built away, as a list's row scrolled
out of sight is: the program keeps what it dragged. Agents drop through `Ui::drop`, the development channel and the MCP server's
`act_on_interface`, the element dragged by its path, the target by its own and where in it. Screen
readers have no drag: the keys do what the drags do.

**Reparenting by a drag.** The hierarchy's rows are draggable and targets of drops. What is dragged
is what is selected, its outermost entities, when the row pressed is selected; its entity alone
otherwise. Dropped on the top quarter of a row, it goes before the row's entity, among its siblings;
on its bottom quarter, after it; between, into it, as its last children, the row unfolded. Dropped
before or after one of the entities that it carries, it goes where that one stands. The row
under the pointer shows where, a line above or below it, or its whole box. A drop that would put an
entity under itself or one of its descendants, or out of the level, shows nothing and makes nothing.
A drop is one step of the history, a `Reparent` for each entity, in their order, each rank counted as
the world will count it when the commands before it applied, and selects the entity dragged if it
was not selected. Each entity that changes parent keeps where it is in the world, as grouping keeps
it ([ADR-0162](0162-creating-copying-and-arranging.md)), its transform given in its new parent's
space in the same step. The level's root does not move, nor does an entity that a placement holds
but for its root, which a scene file cannot move out of its place in the prefab; nothing goes
before or after one of a placement's entities, whose prefab's order a scene file keeps before its
own: into such an entity, last, it may.

**Reparenting by the keys.** While a row holds the focus, Alt and the up or the down arrow move what
is selected before its previous sibling or after its next one; Alt and the left arrow put it beside
its parent, after it; Alt and the right arrow into its previous sibling, as its last child: the
operations `hierarchy.move_up`, `hierarchy.move_down`, `hierarchy.outdent` and `hierarchy.indent`,
each one step, under the same rules as a drop. They change the level, not the window: agents and a
program without its window run them too. Entities selected one after the other among the same
siblings move together, in their order; none passes over another selected.

**G32: each walk made linear.** The search for a cycle is not run for an entity without children,
which no entity has as an ancestor: a hierarchy spawned or loaded from its root down gives each
entity its parent in constant time. Whether an entity or one of its ancestors holds `Unsaved` is a
flag of the world's, kept as the flag of `Hidden` is, as the component comes and goes and as the
entity changes parent. At the end of each step, the world marks each entity that did not move with the
nearest of its ancestors that did, walking the subtree of each that moved as far as one that moved
too, which marks its own; a frame's interpolation reads the mark at once. Until the next step ends,
a change outside a step that makes an entity that moved be drawn where it is, a teleport outside a
step, or a change of parent leaves the marks behind: the interpolation walks the ancestors again
then, as before.

## Alternatives considered

- **A drag of rows of the hierarchy alone, in `slime-editor`**: the asset browser of a later
  milestone drags assets onto the viewport and the hierarchy; `slime-ui` drags between any two of
  its elements once.
- **The drop's place given as before, after or into by `slime-ui`**: a list's rows may want halves,
  a grid's cells none; where in the target, from its top to its bottom, lets each program cut it.
- **The ranks given as each entity's final place**: the world applies a step's commands one after
  the other, each counting ranks without the entity that it moves; the ranks of a step that moves
  several entities among the same siblings are counted as the commands before them leave them.
- **A cached depth of each entity, for the search for a cycle**: a field kept up to date through
  every change of parent, for a search that an entity without children skips.
- **The ancestor that moved found by a pass over every entity at each step**: a cost per step in the
  world's size, where the subtrees of what moved are most often small.
- **The marks kept exact through each change outside a step**: each change of parent and each
  teleport between two steps would mark its subtree again, for changes that a game seldom makes
  between its steps; the walk serves them until the next step's end.
- **A drop that leaves each entity's transform as it is**: the entity would jump to where its new
  parent's space puts it; grouping keeps where each entity is, and a drag does as it does.

## Consequences

- A drag of the hierarchy shows where it drops; an agent drops as a person does.
- An entity's flag of `Unsaved`, as its flag of `Hidden`, costs a walk of its subtree as the
  component comes or goes, and as it changes parent.
- A step marks the subtrees of what moved during it, whose cost grows with them.
- Between two steps, after a change of parent or a teleport, a frame's interpolation walks the
  ancestors as it did before.
