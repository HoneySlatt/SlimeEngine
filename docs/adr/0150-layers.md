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
considered" lists, that the framework does the part that is the same in every program, that it
holds the bar of menus that the editor needs, and that the editor's icons come whole.

## Decision

**Who opens a layer: the widget that owns one, or the program.** A widget whose layer is part of
it opens and closes it alone: a list to choose from opens its list, a tooltip shows over its element
after the pointer rests there. The program builds the layers whose content is its own, menus,
context menus, floating panels and dialogs, and the framework places them, closes them on a press
outside or Escape, and moves the keyboard through menus and their submenus.

**A layer is a root that the framework places** (`Ui::open_layer`, `Layer`, `Place`): beside an
element, on one of its sides and aligned to one of its edges, as a popup, a menu or a list is; at a
point, as a context menu is at the pointer; or at a place and of a size that the program gives, as
a floating panel is, which the program reads and sets again (`Ui::layer_place`,
`Ui::set_layer_place`). A layer beside an element or at a point takes the size of its content; each
frame that its anchor moves, once its content is laid out, the framework places it on its side,
turns it to the other side where there is no room, and moves it along the edge to keep it within
the window, a submenu's layer within the window too, not within the menu's. It takes its scale and
its direction from the root that holds what it stands beside, and closes with that element. A
button or an item of a menu beside which the program opened a layer shows that it is open, by the
state `Expanded`.

**A press outside closes it.** A layer may close as the user leaves it: a press that falls outside
it and outside what opened it, or Escape while the focus is in it, on what opened it or nowhere,
removes it and the layers that stand over what it holds, the focus going back to what held it, and
the program reads that it closed (`ActionKind::Dismissed`). A press in a menu closes its submenus,
not the menu. A modal layer keeps the input as a modal root does: a press outside it closes neither
it nor what lies below it, and only Escape or the program closes it.

**Menus, their items and their submenus.** A menu (`Kind::Menu`) holds items (`Kind::MenuItem`,
`ItemMark`): a mark, a tick if it is checked, its label, the shortcut of its command, and a chevron
if it opens a submenu, the columns of every item kept so that a menu's labels line up. The arrows up
and down move the focus among the items of a menu, around, past those disabled, Home and End to the
first and the last; the arrow towards a line's end presses an item that opens a submenu, the
program opening the submenu beside it, and the arrow towards its start closes a submenu and gives
the focus back to its item; Enter and Space press. The pointer over an item gives it the focus;
resting on an item for a quarter of a second opens its submenu, or closes those of the other items.
A press on an item that opens no submenu reaches the program and closes the menus that led to it,
from the first, telling the program that they closed. A layer opened by the keyboard, an agent or
the program gives its first item the focus.

**The bar of menus** (`Kind::MenuBar`): a row of items that open its menus below them, their labels
alone. A press on an item opens its menu, and closes it once open; while one of its menus is open,
the pointer over another item opens that one's at once. The arrows towards a line's start and end
move along the bar, and, from a menu of the bar, to the bar's other menus, but on an item that
opens a submenu; the arrows up and down open the menu of the item that holds the focus. F10, or Alt
pressed and let go alone, gives the bar's first item the focus and takes it back; Escape on the bar
gives it back to what held it before, and so does a command of one of its menus. Its items take the
focus from the keys alone, so that a press of the pointer on the bar leaves a field its focus.

**Context menus.** A press of the pointer's second button on what the interface draws, or the
key that opens a context menu, Shift+F10, while an element holds the focus, gives the program an
action (`ActionKind::ContextMenu`) with the element and the point, at which the program opens its
menu.

**Lists to choose from** (`Kind::Choice`): an element that shows its chosen option, as wide as its
widest, and opens the list of its options below it, at least as wide as itself, as a press, Enter,
Space, Alt+Down or F4 asks. The list is a menu whose items are its options, the chosen one ticked
and given the focus by the keyboard, so that the keys and the pointer move through it as through a
menu; a press or Enter chooses, Escape, a press outside or a press on the element closes it
unchanged. Closed, Up and Down choose the option before or after at once, Home and End the first
and the last. The program reads the choice changed then committed (`FieldValue::Chosen`), the
program's choice replacing the user's only when it changes, as a check box's state does
([ADR-0148](0148-widgets.md)); agents and screen readers set it by an option's text or its place,
and read the element as a combo box, open or not, and its list as a list of options.

**Tooltips** (`Element::tooltip`): an element's tooltip shows below it, in its middle, once the
pointer has rested on it, or on what it holds, for half a second of the interface's clock, or once
the keys gave it the focus, and goes when the pointer leaves it, presses, or a key is pressed,
Escape doing nothing more then. The framework builds it in a layer that the pointer passes
through, a container of the class `tooltip` holding a label, which style sheets select; screen
readers and agents read it as the element's description, not as a text of its own.

**Floating panels** (`Kind::FloatingPanel`): the only child of a floating layer, which it fills,
drawing a bar of its title, as a section draws its header, which the pointer drags to move the
layer within the window, with a mark that folds it to its bar, the layer as tall as the bar then,
and a corner at the end of its bottom that the pointer drags to size it, no smaller than a few of
its control's sizes. The place and the size that the user leaves are the layer's, which the
program reads and sets; whether it is open is kept as a section's is. A press anywhere on it raises
its layer above the others, which changes the numbers of the roots in paths.

**The editor's icons: Lucide, whole.** The look holds Lucide 1.61.0's font of icons whole, its
2191 icons, with its file of codepoints and its license, ISC for Lucide and MIT for the icons that
it took from Feather (`look::icon`, by an icon's name). The glyphs that controls draw in place of
their shapes, a tick, a chevron down, and chevrons towards the right and the left, which a root's
direction picks for what points to a line's end, are named in an element's style (`Style::icons`),
with the family of their font; the look's sheet names Lucide's by their codepoints, which a sheet
after it changes. A style that names none draws the shapes that M6b.7a drew.

## Alternatives considered

Who opens layers:

- **The program alone**, the framework placing them only: less in the framework, but every program
  writing again how a list opens, how a tooltip waits, how a menu closes and how its keys move.
- **The framework alone**, menus and panels described as data that the framework builds: one way for
  every layer, but menus whose items are a program's own built from a description rather than by
  the program's builder, as everything else is.

The bar of menus:

- **Buttons that open menus, the bar left to the program**: nothing more in the framework, but no
  menu that the pointer passes to along the bar, no arrows between its menus, and each program
  writing them again.

Where a layer is placed:

- **By the program, from the element's box**: nothing new in the framework, but every program
  placing again, turning and moving its popups to keep them in the window, a frame late.
- **Elements out of their parent's flow**, CSS's `position: absolute` and `z-index`: one tree, but
  what ADR-0140 and ADR-0142 left out, clips and input that cross the tree.

Icons:

- **The icons that the editor uses, cut from the font**: a few tens of kilobytes, but a script run
  again for each icon added, and none left for a project's panels.
- **The icons named in the themes**, `"$icon_tick"`: a theme that changes the icons, but a style
  sheet reads a theme's value in place of every value but a text, which an icon is; the sheet
  names them by their codepoints instead.

## Consequences

- `slime-ui` gains `Ui::open_layer`, `Ui::layer_place`, `Ui::set_layer_place`, `Layer`, `Place`,
  `Side` and `Along`, `Kind::Menu`, `Kind::MenuBar`, `Kind::MenuItem` and `ItemMark`,
  `Kind::Choice`, `Kind::FloatingPanel`, `Element::tooltip`, `ActionKind::Dismissed` and
  `ActionKind::ContextMenu`, `FieldValue::Chosen`, the roles of menus, their bar and items, combo
  boxes, their lists and options, and panes, `Meaning::popup` and `Meaning::description`, and
  `Style::icons`; `Ui::chosen_tab` becomes `Ui::chosen`, for tabs and lists to choose from alike. A
  program that matches on `Kind`, `ActionKind`, `FieldValue` or `Role` reads the new values.
- A layer's place is worked out after its content is laid out, each frame that it or its anchor
  moves; a floating layer takes its own size, not the window's.
- The look holds Lucide's font, about 932 KiB, with its license, which a program that builds the
  look in ships beside it, as it ships Geist's.
- `slime-app`'s translation and the development channel give screen readers and agents the new
  roles, states, popups and descriptions (`has_popup`, `description`); agents use the layers
  through the actions that exist and a choice set by value.
