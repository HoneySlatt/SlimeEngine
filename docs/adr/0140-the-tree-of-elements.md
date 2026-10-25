# 0140. The tree of elements

- Status: Accepted; amends [0132](0132-the-engines-ui-framework.md) (each element carries the marks
  of its layout and its paint from this milestone, those of its style and its meaning from the
  milestones that match style sheets and give the tree its meaning) and
  [0131](0131-stage-6b-and-stage-6c.md) (M6b.4 cut in two: the tree, then style sheets and
  themes); amended by [0141](0141-style-sheets-and-themes.md) and
  [0143](0143-what-agents-see-of-the-interface.md)
- Date: 2026-10-24

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided that an interface is a retained tree of
elements in an arena of the framework's own: handles of an index and a generation, keys unique among
siblings, paths of keys; what changed in each element, its style, layout, paint or meaning, so that
a frame redoes only what changed; panels built by code again when a revision that they read moves,
the elements built again matched with those before by their keys; typed styles, start and end
rather than left and right, a direction per root; taffy's flexbox and grid on the framework's tree,
through taffy's traits, so that there is one tree; lists that build only the rows that show. Its
study measured taffy 0.15 laying out a tree of 12 001 elements in 9.8 ms, then in 0.88 ms after one
element changed.

M6b.4 was to build that tree, its elements, their styles and their layout, and the display list made
from them, with style sheets and themes, documents of reflected types. Building the tree raised what
ADR-0132 left to it: how a builder gives an element's children, how a change spreads, in which unit
taffy works, how a root is mirrored, how a list lays out its rows, how scrolling places elements
again, and what a frame of a large list may cost.

The owner validated the design below on 2026-10-25, with a tighter budget for a list's frame than
the one proposed, 0.25 ms rather than 0.5 ms, and cut M6b.4 in two, the tree having grown larger
than planned: M6b.4a, the tree, which this ADR decides, and M6b.4b, style sheets and themes, which
it does not.

## Decision

**The arena and its keys.** `slime_ui::tree::Ui` holds every element of its roots in one arena;
`ElementId` is an index and a generation, so that the id of an element removed names no other that
takes its place. A `Key` is a name or a number, unique among siblings; an element's path is the keys
from below its root down to it. A root is added with its `RootSettings`, its size in logical pixels,
its scale, the pixels of the screen per logical pixel, and its direction; the root's element is a
container of that size.

**Builders.** `Ui::build(parent, revision, builder)` runs the builder only when `revision` moved
since it last ran for `parent`, or when a list below it shows rows that it did not build. The
builder adds children through `Children::add(key, element, children)`, each with its own children
given the same way. A child is matched with the one of the same key built before: of the same kind,
it is kept, with its state and its scroll, and given the new description; of another kind, it is
removed and a new one made; one not given again is removed. Two siblings of one key are refused, a
panic, as a bug of the builder. `Children::add_slot` adds a child whose children another builder
gives, through `Ui::build` on its id, so that a panel is built by its own revision and kept as it
is when its parent is built again.

**What changed.** Each element carries two marks: its layout and its paint. A change of its
layout's style, its text's style, its label or its list's rows marks it and its ancestors laid out
again, up to its root, and clears taffy's cache along that path; up to its list, for a row, whose
list lays its rows out again alone. A change of its paint's style, its text's color or its state
marks it drawn again alone. Placing elements after a layout, or after a scroll, marks drawn again
those whose box or cut moved. A frame where nothing changed lays out and draws nothing; it gathers
the display list from what each element drew before. The marks of style and meaning come with the
milestones that match style sheets and that give the tree its meaning.

**Styles, typed, in logical pixels.** `slime_ui::style::Style` groups an element's properties by
what a change redoes: its layout, CSS's flexbox and grid properties, its paint, a box's fill, its
border, drawn inside its edges without taking space, its corners and its shadow, and its text, its
style and its color. Sides and corners are named by the start and the end of a line. taffy works in
pixels of the screen: each element's style is converted at its root's scale when it changes, so that
taffy rounds boxes to whole pixels of the screen, and texts are measured at the scale they are drawn
at.

**Layout by taffy's traits on the arena.** The tree implements taffy's low-level traits, its
traversal, its cache, its flexbox and grid containers and its rounding, over the arena's own nodes,
each holding its taffy style, cache and layouts. Labels and buttons are measured by the text cache.
taffy is built with its flexbox, grid and content size alone, without its own tree, its block
layout or its `calc`; no element is placed out of its parent's flow, so that an overlay shares a
grid's cell with what it covers.

**A root from right to left is mirrored.** A root is laid out from left to right, its start sides
on the left; a root of the other direction then has every box mirrored across its width, its
corners and its shadows' offsets with them. A text keeps its own direction within its box.

**A list lays out its rows itself.** taffy does not see a list's rows: the list's own box comes from
its style, and each row is laid out alone, at the list's width and the height of a row, and placed
by its number, its key. Its builder adds the rows that show (`Children::rows`), from the list's box
when last laid out and its scroll; when another range shows, the builder that gives its rows runs
again: the one that made the list, or, for a list in a slot, its own, which builds its rows alone.
A list in a slot therefore costs per frame what it shows, and a change in its rows lays nothing
else out again; a list made with its panel builds the panel again with its new rows. A new list
shows its rows from the frame after its first layout. A list in a row of another is laid out after
it, its rows with it.

**Scrolling places again, without laying out.** A box that scrolls and a list cut their content
inside their border, as CSS cuts it at the edge of the padding: the clip that each element below
them carries. They move it by their scroll, in whole pixels of the screen, kept within their
content. `Ui::scroll_to` places again only what lies below the element that scrolled.

**What is drawn.** Each element keeps the primitives it draws: its shadow, its box, then its text
or its image; a button's paint as its state says, its label in the middle of its box. A frame
gathers them in the tree's order, each element before its children, with its clip; an element
whose display is none, and what it holds, takes no space and draws nothing.

**The list's budget.** A frame of a list of 100 000 rows, scrolled or standing still, takes at most
0.25 ms of the CPU, at its 90th percentile, which the frames that build new rows reach, on the
development machine, an AMD Ryzen 7 5800X: half the interface's share of the GPU on each tier
([ADR-0137](0137-the-canvas.md)), as the owner chose. `ui.tree.list.100k` measures it
([`docs/measurements.md`](../measurements.md), M6b.4a).

## Alternatives considered

- **taffy's own tree, `TaffyTree`, beside the framework's.** Two trees to keep in step, every
  element added, removed and moved twice; ADR-0132 decided one tree.
- **Layout in logical pixels, scaled after.** Boxes would be rounded to logical pixels, half a
  pixel of the screen off at scale 1.5, and texts measured at another scale than they are drawn at.
- **taffy's direction on each element.** taffy 0.15 lays out from right to left an element whose
  style says so, its margins and paddings still left and right. Each element would carry the
  direction and its sides converted by it; the mirror does the same for a direction per root,
  which ADR-0132 decided, in one subtraction per box.
- **A list's rows as taffy's children.** Every row of a list of 100 000 would be built, held and
  laid out; the rows that show alone, laid out by the list, cost what shows.
- **Builders kept to run again within the frame**, so that a list shows the rows of a new range in
  the frame that its layout finds them. Builders borrow the program's state; kept, they would have
  to own it. The rows come one frame later, only when a list first shows, or when it scrolls after
  its builder ran.
- **A list's budget of 0.5 ms**, as long as the interface's share of the GPU, which was proposed.
  The owner chose half, the stricter one, which the list's frames hold too.
- **The display list patched where elements changed**, rather than gathered each frame. A frame of
  an editor's tree in which nothing changed, which gathers its 9 000 items, takes 0.28 ms; a list
  patched in place is more code for a cost not yet measured as a problem.

## Consequences

- A tree of an editor's size, some 12 500 elements, takes 2.4 ms after one value changed, its
  panel built again, and 0.28 ms with nothing changed; its first frame, which lays out its texts,
  134 ms ([`docs/measurements.md`](../measurements.md), M6b.4a).
- A frame walks every element of its roots, to find what to draw again and to gather the display
  list, even when nothing changed.
- A Latin text in a root laid out from right to left starts at its box's left, as a text's own
  direction says, where CSS would start it at the right.
- No element is placed out of its parent's flow: a popup, a tooltip or a menu needs a root or a
  layer of its own, which later milestones give.
- Scrolling is set by the program: input reaches the tree with the next milestone.
