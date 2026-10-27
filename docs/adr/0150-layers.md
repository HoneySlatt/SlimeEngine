# 0150. Layers

- Status: Accepted; amends [0142](0142-input-and-focus.md) (roots that the framework places, and
  closes on a press outside them or Escape)
- Date: 2026-10-27

## Context

[ADR-0142](0142-input-and-focus.md) made popups, menus and dialogs roots of their own, stacked over
the window's, at the place that the program gives them (`RootSettings::origin`), a modal root
keeping the input and the focus while it is open. M6b.7c brings what opens over a panel
([ADR-0149](0149-the-looks-foundations.md)): popups placed beside what opens them and kept within
the window, tooltips, context menus and their submenus, lists to choose from, panels that float
inside a window and are moved, folded and sized, and modal layers, with the icons of the editor's
look. A root placed by hand at a fixed point does none of it: it does not follow what opened it,
leaves the window where it lacks room, and stays open when the user clicks elsewhere.

Which part of each the framework does and which the program does is what decides how much every
program writes again. The owner decided on 2026-10-27, among the options that "Alternatives
considered" lists, that the framework does the part that is the same in every program, and that the
editor's icons come whole.

## Decision

**Who opens a layer: the widget that owns one, or the program.** A widget whose layer is part of
it opens and closes it alone: a list to choose from opens its list, a tooltip shows over its element
after the pointer rests there. The program builds the layers whose content is its own, menus,
context menus, floating panels and dialogs, and the framework places them, closes them on a press
outside or Escape, and moves the keyboard through menus and their submenus.

**A layer is a root that the framework places** (`Ui::open_layer`, `Layer`): beside an element, on
one of its sides and aligned to one of its edges, as a popup, a menu or a list is; at a point, as a
context menu is at the pointer; or at a place and of a size that the program gives first, as a
floating panel is. A layer beside an element or at a point takes the size of its content, laid out
within the window; each frame, once the root of what it stands beside is laid out, the framework
places it on its side, turns it to the other side where there is no room, and moves it along the
edge to keep it within the window. It takes its scale and its direction from the root that holds
what it stands beside, and closes with that element.

**A press outside closes it.** A layer may close as the user leaves it: a press that falls outside
it and outside what opened it, or Escape while the focus is in it or nowhere, removes it and the
layers that it opened above it, and the program reads that it closed
(`ActionKind::Dismissed`). A press in a menu closes its submenus, not the menu. A modal layer keeps
the input as a modal root does, and closes only as the program or Escape closes it.

**Menus, their items and their submenus.** A menu (`Kind::Menu`) holds items (`Kind::MenuItem`):
a label, the shortcut of its command, a mark if it is checked, a chevron if it opens a submenu.
The arrows move the focus among the items of a menu, around; Right on an item that opens a submenu
presses it, the program opening the submenu beside it, Left closes a submenu and gives the focus
back to its item; Enter and Space press. A press on an item that opens no submenu closes every menu
open, the program reading the press. A layer opened by the keyboard gives its first item the focus.

**Context menus.** A press of the pointer's second button on what the interface draws, or the
key that opens a context menu, Shift+F10, while an element holds the focus, gives the program an
action (`ActionKind::ContextMenu`) with the element and the point, at which the program opens its
menu.

**Lists to choose from** (`Kind::Choice`): an element that shows its chosen option and opens the
list of its options below it, the chosen one marked, as a press, Enter, Space or Alt+Down asks;
the arrows move through the list, Enter chooses, Escape or a press outside closes it unchanged.
Closed, Up and Down choose the option before or after at once. The program reads the choice changed
then committed (`FieldValue::Chosen`), the program's choice replacing the user's only when it
changes, as fields' values do ([ADR-0148](0148-widgets.md)); agents and screen readers set it by an
option's text or its place, and read the list as a combo box, open or not.

**Tooltips** (`Element::tooltip`): an element's tooltip shows below it once the pointer has rested
on it for half a second of the interface's clock, or once the keyboard gave it the focus, and goes
when the pointer leaves it, presses, or a key is pressed; screen readers read it as the element's
description.

**Floating panels** (`Kind::FloatingPanel`): a layer whose element draws a bar of its title, which
the pointer drags to move the panel, with a mark that folds it to its bar, and a corner that the
pointer drags to size it; the place and the size that the user leaves are kept against the
program's until the program's change, as fields' values are, and a press raises the panel above
the others.

**The editor's icons: Lucide, whole.** The look holds Lucide 1.61.0's font of icons whole, its
2191 icons, with its license, ISC for Lucide and MIT for the icons that it took from Feather
(`look::icon`, by an icon's name). The parts of controls that the look draws as icons, a check box's
tick, a section's and a menu's chevrons, a list's arrow, name them in a control's style
(`ControlStyle::icons`); a style that names none draws the shapes that M6b.7a drew.

## Alternatives considered

Who opens layers:

- **The program alone**, the framework placing them only: less in the framework, but every program
  writing again how a list opens, how a tooltip waits, how a menu closes and how its keys move.
- **The framework alone**, menus and panels described as data that the framework builds: one way for
  every layer, but menus whose items are a program's own built from a description rather than by
  the program's builder, as everything else is.

Where a layer is placed:

- **By the program, from the element's box**: nothing new in the framework, but every program
  placing again, turning and moving its popups to keep them in the window, a frame late.
- **Elements out of their parent's flow**, CSS's `position: absolute` and `z-index`: one tree, but
  what ADR-0140 and ADR-0142 left out, clips and input that cross the tree.

Icons:

- **The icons that the editor uses, cut from the font**: a few tens of kilobytes, but a script run
  again for each icon added, and none left for a project's panels.

## Consequences

- `slime-ui` gains `Ui::open_layer` and `Layer`, `Kind::Menu`, `Kind::MenuItem`, `Kind::Choice`,
  `Kind::FloatingPanel`, `Element::tooltip`, `ActionKind::Dismissed` and `ActionKind::ContextMenu`,
  `FieldValue::Chosen`, the roles of menus, their items and combo boxes, and `ControlStyle::icons`.
  A program that matches on `ActionKind` or `FieldValue` reads the new values.
- A layer's place is worked out after its anchor's root is laid out, each frame that either moves.
- The look holds Lucide's font, about 932 KiB, with its license, which a program that builds the
  look in ships beside it, as it ships Geist's.
- `slime-app`'s translation and the development channel give screen readers and agents the new
  roles, states and descriptions; agents use the layers through the actions that exist and a
  choice set by value.
