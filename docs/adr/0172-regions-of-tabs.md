# 0172. Regions of tabs

- Status: Accepted; amends [0153](0153-tab-and-f6-in-a-games-interface.md) (F6 moves between the
  panes of a root too) and [0155](0155-the-editors-design.md) (docking a model of `slime-ui` that
  builds its elements, not a kind of element)
- Date: 2026-11-04

## Context

[ADR-0155](0155-the-editors-design.md) decided docking's model (D9): splits of any number of
children, each with its share of the space, and stacks of tabs, normalized after each edit so that
no split is empty or holds one child; a border resized changes the shares of the two panels beside
it and nothing else; a panel closed keeps its place; the default layout carries a version; layouts
are kept per user and per project in the user's state directory, read leniently, set aside when
they cannot be read, and "Reset layout" always works. Regions with tabs, splits that the user
resizes, "Move to" and floating panels come with the hierarchy, in milestone M6c.5d; panels dragged
to dock come last. It said that docking is a kind of element of `slime-ui`, its layout reflected
data that `slime-serialize` writes, the editor owning its panels and when their layout is saved.

What the code held before this milestone:

- The editor's window builds fixed regions: a column 340 pixels wide on the left, the hierarchy and
  the components sharing what the history, 200 pixels high, leaves; the toolbar of Play, the
  viewports and, once Play has run, its console in the center; nothing that the user resizes.
- `slime-ui` has two panels and the handle between them (`Element::split`,
  [ADR-0148](0148-widgets.md)): two children, a third folded, its share the interface's alone,
  which tells the program nothing; a bar of tabs whose choice the program reads as it builds
  (`Children::chosen`); floating panels, layers placed in the window
  ([ADR-0150](0150-layers.md), [ADR-0154](0154-floating-panels-and-held-keys.md)); drags of an
  element onto another ([ADR-0168](0168-rearranging-the-hierarchy.md)); context menus asked for by
  the second button and Shift+F10, which the editor answers nowhere yet.
- F6 moves the focus from one root of the interface to the next
  ([ADR-0153](0153-tab-and-f6-in-a-games-interface.md)); the editor's window is one root, so F6
  reaches nothing within it.
- The editor's window operations, the palette's or the split of the viewports, are queued by name
  alone: none takes arguments.

## Decision

**The milestone in two.** M6c.5d holds docking's model, the regions of tabs, the borders that the
user drags, "Move to" and F6 between the regions. M6c.5e holds what outlives a session or leaves the
regions: the layout kept per user and per project, read leniently, set aside when it cannot be read
and the user told, "Reset layout", the default's version, floating panels, and a panel closed that
keeps its place. The inspector becomes M6c.5f.

**A model that builds elements: `slime_ui::dock`.** A `Layout` is a tree whose leaves are stacks
of tabs, each a list of panels, named by keys that the program gives, and the one chosen, and whose
other nodes are splits, side by side or one above the other, of any number of children, each with
its share. It derives `Reflect`, so that M6c.5e writes it through `slime-serialize`. Each edit, a
panel moved or a border dragged, is followed by its normalization: a split left with one child is
replaced by it, a split within a split of the same direction is merged into it, its children's
shares scaled to the space that it held, and an empty stack is removed. A `Dock` holds a layout and
builds it under a slot of the program's tree, as the hierarchy builds its rows
([ADR-0167](0167-the-hierarchy.md)): each stack as a pane, its bar of tabs, one tab for each panel,
even alone, and the chosen panel's slot, which the program fills; each split as a box whose children
grow by their shares, a splitter between each two. It takes the actions of its elements, reads the
tab chosen as it builds, and tells the program whether the layout changed, for M6c.5e to keep it.
Docking is so a model and the elements that it builds, as ADR-0155 decided its model, not a kind of
element: a kind would lay out and draw tab bars and handles of its own, which `slime-ui` has as
elements already.

**A splitter: one border, one element.** `Element::splitter` is a new kind, a bar between two
children of a box, which the pointer drags and the arrows move, a resize cursor over it. It moves
the border between the child before it and the child after it, and nothing else: it gives the
program, as `ActionKind::Changed` while it moves and `ActionKind::Committed` as it ends, the share
of the two that the first takes, which the dock makes the two children's, their sum unchanged,
neither below the least size of a pane. Agents and screen readers read it as a splitter, its value
that share, which agents set (`Ui::set_value`), as they set the handle of two panels'.

**"Move to".** `view.move_panel`, an operation of the window with two arguments, moves a panel, by
its key, into the stack of another panel, as its last tab, or to an edge of the window, `left`,
`right`, `top` or `bottom`, as a new stack the whole height or width of the window, a quarter of
it. People reach it from a tab's context menu, the second button or Shift+F10 on a tab: *Move to*,
a submenu of the other stacks, each by its panels' titles, and of the edges; agents run it by name
with its arguments, as they run `file.open`. The window's queue of operations carries their
arguments so.

**F6 between the panes.** An element marked as a pane (`Element::pane`) is one of the places that
F6 moves the focus between. F6 gives the focus to the first element of the next pane that holds
one, within the top root, in the order of the tree, then to the first of the next root, a root
without panes counting as one place, then to nothing; Shift+F6 goes the other way. Each of the
dock's stacks is a pane, so F6 goes from the hierarchy to the components, the viewport, the history
and back. This amends [ADR-0153](0153-tab-and-f6-in-a-games-interface.md), whose roots were the
only places.

**The editor's panels and its default layout.** The window's panels are the hierarchy, the
components, the viewport, which holds the toolbar of Play and one viewport or two, the history and
the console, named `hierarchy`, `components`, `viewport`, `history` and `console`. The default
layout puts the hierarchy above the components in a column on the left, a quarter of the window,
and the viewport above a stack of the history and the console, a quarter of its height, the console
showing that Play has not run until it does. Each panel's title is its tab. The layout is the
window's while it runs, from the default at each start, until M6c.5e keeps it.

## Alternatives considered

- **Docking as a kind of element**, as ADR-0155 said: the interface would own the layout, but it
  would lay out, draw and hit tabs and handles of its own beside the bar of tabs and the splitter,
  and tell their meaning to agents and screen readers a second time.
- **`Element::split` made of any number of children, its handles inside it**: one element whose
  handles neither the focus, an agent nor a screen reader can name apart; a splitter for each
  border is how ARIA's window splitter is made.
- **Splits of two children nested**: what ADR-0155 rejected, a border moving several panels.
- **Fixed regions with tabs, left, right and bottom, as Godot's docks**: less to build, but what
  ADR-0155 rejected, the user arranging only what the slots allow.
- **The history left in the left column**: three stacks in one column leave each little height;
  the history and the console are both read along a session, below what the viewport shows.
- **F6 remembering the element that last held the focus in each pane**: what GTK does; the first
  element serves until someone misses it.
- **The splitter's share kept by the interface, as the two panels' handle keeps its own**: the
  layout to keep is the program's; a share that the interface alone holds is lost when a builder
  removes its element, the debt of two panels.

## Consequences

- `slime-ui` gains the dock, the splitter and panes; the editor's window builds its regions from a
  layout, and its default images change, each region under a bar of tabs.
- There are two ways to split a box in `slime-ui`: two panels and their handle, the share the
  interface's, and a box with splitters, the shares the program's; the first is to be built on the
  second, which settles the debt of two panels.
- A layout changed by the user is lost when the editor stops, until M6c.5e.
- The paths that agents find the panels' elements by change: each panel lies under the dock's
  stacks, not under the fixed regions.
