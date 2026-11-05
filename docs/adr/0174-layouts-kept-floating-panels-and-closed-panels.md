# 0174. Layouts kept, floating panels and closed panels

- Status: Accepted; amends [0150](0150-layers.md) (a floating panel that the pointer moved or sized
  tells the program as it lets go) and [0155](0155-the-editors-design.md) (a layout kept against
  any other default is set aside, not only against an older one)
- Date: 2026-11-05

## Context

[ADR-0155](0155-the-editors-design.md) decided docking's model and where layouts live (D9): a panel
closed keeps its place, to come back there; the default layout carries a version, so that a newer
default replaces a layout saved against an older one; layouts are kept per user and per project in
the user's state directory, a game's editor package giving the project's default; a layout is read
leniently, set aside when it cannot be read, the user told; "Reset layout" always works.
[ADR-0172](0172-regions-of-tabs.md) cut milestone M6c.5d in two and left these to M6c.5e, with
floating panels, which "Move to" reaches.

What the code held before this milestone:

- `slime_ui::dock`: a `Layout`, a tree of splits and stacks of tabs, reflected; a `Dock` that builds
  it under a slot of the program's tree, takes its splitters' actions, chooses a panel's tab
  (`Dock::show`), moves a panel into another stack or to an edge of the window, and says whether the
  layout changed (`Dock::take_changed`), at each step of a border's drag too.
- `slime-ui`'s floating panels: layers placed in the window (`Place::Floating`), which the pointer
  moves and sizes, the place kept by the interface alone, read by `Ui::layer_place`; the dev
  overlay opens its panels so, and keeps their places while the program runs.
- The editor's window: five panels, `hierarchy`, `components`, `viewport`, `history` and
  `console`, laid out at each start from a default in code; `view.move_panel`, which the tabs'
  context menus reach. It keeps the user's choice of keys and the levels opened last in the user's
  state directory, `slime-editor/keys` and `slime-editor/<program's name>.recent`, which agents keep
  off with a state directory of their own (`XDG_STATE_HOME`).
- M6c.8 still listed what ADR-0172 moved here: closed panels coming back to their places, a
  project's default layout, a layout saved against an older default, a layout that cannot be read.

## Decision

**A closed panel stays where it was, hidden.** The layout keeps the panels closed, by their keys,
beside its tree; each stays in its stack, or among the floating panels, where it was. The dock
builds no tab for a closed panel, no stack whose panels are all closed, no split whose children are
all hidden, and no splitter beside what it hides; what shows shares the room by the shares that it
holds. A splitter moves the border between the two children that it parts, which a hidden child
between them leaves as it was. Closing a stack's chosen panel chooses the next open one, or the one
before. Opening a panel takes it out of the closed ones and chooses its tab
([`Dock::show`](../../crates/uwui/src/dock.rs), which now opens too), so that it comes back to
its stack, in its place among the tabs, with its share, or floats where it floated. A panel moved
opens where it goes.

**A floating panel is a layer that the dock opens.** The layout keeps its floating panels, each its
key, where its top left corner stands in the window and its size, in logical pixels, the last above
the others. The dock opens a floating layer for each that is open, over its slot
([ADR-0150](0150-layers.md)), named `floating <panel>` in paths, and builds in it a floating panel
titled as the panel's tab, whose slot the program fills as a stack's. `Target::Float` moves a panel
there, out of its stack: it floats where its stack stood, or in the middle of the window when its
stack never showed, a third of the window's width and half its height. A floating panel holds one
panel. "Move to" brings it back into a stack or to an edge; a panel that floats already is not
floated again. Its layer opens no larger than the window, a layout kept on a larger screen keeping
its corner within reach. A floating panel that the pointer moved or sized gives the program
`ActionKind::Placed` as the pointer lets it go, with its layer's root and its place: the dock keeps
it in the layout. An agent places a layer by the same action, a place that is not finite or of no
size left out. Whether a floating panel is folded to its bar, and which one stands above the others
once the pointer raised one, stay the interface's.

**A layout fits the program's panels.** `Layout::fit` drops the panels that the program does not
have, and adds each that the layout lacks as the last tab of the stack of the panel nearest before
it in the program's list, or after it, or of the root, open and not chosen. The editor fits each
layout that it reads, so that a panel added to the editor shows in layouts kept before it, and a
panel removed leaves them.

**One file per program, written once the layout stands still.** The editor keeps the layout in
`slime-editor/<program's name>.layout` in the user's state directory (`slime_app::state_directory`),
beside its recent levels: per user and per project, a game's editor program being its project's. The
file, a *layout file*, is a document of `slime-serialize`, the layout and the default that it was
made from, written whole or not at all. The layout settles as a panel is moved, floated, closed or
opened, a tab chosen, a border or a floating panel let go, or the layout reset; not at each step of
a drag, which `Dock::take_changed` no longer reports, a border's `Committed` alone counting. The
window writes it once the layout has stood still for half a second after it settled, waking for it
then, and as it closes: a write that syncs the disk costs a large part of a frame, which a border
moved by a held arrow, or tabs chosen one after another, would pay at each step. A crash loses what
changed in that half second. A file that cannot be written is told once, the layout kept for the
run. Two editors of one program running at once, or two checkouts of a project whose programs share
a name, share the file: the last to write wins, and where their defaults differ, each start sets
aside the other's layout.

**A default names itself and its version.** A default layout is a layout file too, its `default`
key naming it and its version, `editor 1`. The editor's own default is
`crates/slime-editor/editor.layout`; a game's editor package gives its project's in
`Program::layout`, the text of such a file, which the editor shows in place of its own. A
project's author arranges the editor, copies the file of their state directory into the package
and names it anew. A kept layout made from another default than the program's, by name or
version, newer or older, is set aside as `<file>.replaced`, the default shown, and the user told:
the default changed. Its author changes the version when the arrangement should replace those
kept; a panel added or removed needs no new version, since layouts fit the panels.

**Read leniently, set aside when it cannot be read.** A missing file gives the default, silently. A
file that cannot be read, its bytes or its text as a layout file, a text of another encoding
included, is set aside as `<file>.unreadable` (`slime_app::set_aside`), the default shown, and the
user told where it went; one that can be neither read nor set aside, a directory that takes no new
name, is not written over for the run, the user told. A file read is normalized and fitted: its
unknown fields and panels are dropped, a share or a place that is no finite number is mended. A
project's default that cannot be read, a fault of the game's package, gives the editor's own, the
user told, and the layout kept against the project's is neither read nor written for the run, so
that it waits for the package to be mended rather than being set aside.

**"Reset layout" and the operations.** `view.reset_layout`, *View* > *Reset layout*, shows the
program's default and writes it, whatever the file holds. `view.close_panel` and `view.open_panel`
close a panel, and open it where it was or choose its tab; `view.move_panel`'s target `float` floats
it. People reach them through the tabs' context menus, *Move to* and *Close*; the floating panels'
bars, the second button or Shift+F10, with the same menu; and *View* > *Panels*, an item for each
panel that shows it, opened where it was or its tab chosen, as JetBrains' tool windows and Unity's
windows are shown from their menus. A panel's own button to close its tab waits for M6c.8.

**Agents keep off the user's layout.** A tool drives the editor through a server whose state
directory is its own, as the session tests do: a tab chosen by an operation changes the layout that
the user's state directory keeps. `AGENTS.md` and `slime-mcp`'s README say so, for the keys, the
layout, the flying speeds and the recent levels together.

**M6c.8 keeps the drags.** Panels and tabs dragged to dock, the zones that show where they would
go, tabs reordered by drags, a tab's own button that closes it, floating panels docked by a drag,
and a least size for each panel stay M6c.8's. What it listed and this milestone delivers leaves it:
closed panels coming back to their places, a project's default, a layout kept against another
default, a layout that cannot be read, and tests of the layout's rules after random edits, which
`slime-ui` runs from here.

## Alternatives considered

- **A closed panel taken out of the tree, its neighbour remembered**, as AvalonDock's
  `PreviousContainer` or Dear ImGui's dock IDs: a simpler tree, but a panel alone in its stack has
  no neighbour left, and comes back elsewhere. Unreal's tab manager keeps closed tabs in their
  stacks, and Qt keeps placeholders for hidden docks, as this does.
- **A closed panel opened again where the default puts it, or floating**, as Unity's windows: what
  ADR-0155 rejected.
- **Floating panels kept as nodes of the tree, where they were docked**, as Qt's placeholders let a
  floating dock return there: more to build and to keep normalized, for a return that "Move to"
  gives.
- **Floating stacks of tabs**, as Unity's floating windows: a floating panel of several tabs, which
  drags would fill; M6c.8 may add it.
- **A version that is a number, a newer one replacing**: a program that gives its own default
  would compare its numbers with the editor's; a name and a version compared for equality cannot be
  mistaken, and a layout from a later program, read by an earlier, is set aside rather than read
  against panels that it may not know.
- **The default compared whole**, without a version: no version to forget, but every panel added
  would replace every layout kept, which fitting avoids.
- **A project's default given in code**, a function that builds a `Layout`: a game's editor package
  would depend on `slime-ui`, and its author would write by hand what the editor arranges; the
  layout file is what the editor writes.
- **The layout written as the editor quits**, alone: a crash loses the whole session's
  arrangement. **At each change**, as it settles: a held arrow on a border writes at each repeat.
  **On a thread of its own**: no frame pays for the write, but a thread, a queue and the errors
  sent back to the window, for a write that, half a second after the last change, lands in a frame
  where nothing moves.
- **View > Panels as check items that close and open**: what VS Code does for its areas, but a
  panel behind another's tab would close as the user meant to show it.
- **A `--layout <file>` option for agents**, as `--journal`: one more option, while a state
  directory of their own already keeps their keys and recent levels apart.
- **The layout not written in a program that a tool started**: the session tests could not check
  that a layout comes back.

## Consequences

- `slime-ui`'s dock gains closed and floating panels, `Layout::fit`, `Target::Float` and
  `ActionKind::Placed`; `Layout`'s new fields default to none, so that layouts written before read.
- The editor's window reads its layout as it starts and writes it once it stands still, waking
  once for it; its image tests read none, and start from the default.
- `Program` gains `layout`; *Hunna Wood*'s editor and the `editor` example give none, and show the
  editor's own.
- A program that a tool drives through a server without a state directory of its own writes the
  user's layout.
- M6c.8 is smaller: drags, their zones, tabs' buttons and panels' least sizes.
