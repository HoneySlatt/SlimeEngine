# 0153. Tab and F6 in a game's interface

- Status: Accepted; amends [0142](0142-input-and-focus.md) (Tab with no focus that takes the keys
  gives the focus to the interface only where the program allows it; F6 moves the focus from one
  root to the next) and [0152](0152-the-dev-overlay-on-the-engines-interface.md) (in an interface
  that allows it, Tab goes on from a focus that a press gave)
- Date: 2026-10-27

## Context

[ADR-0152](0152-the-dev-overlay-on-the-engines-interface.md) builds the dev overlay as roots of the
presentation's one interface, and leaves the keys to the program while the focus is one that a
press gave. [ADR-0142](0142-input-and-focus.md) made Tab and Shift+Tab, with no focus, give it to
the top root that holds an element that can take it, and the interface take them. Building M6b.8b
showed what follows: once one of the overlay's panels shows, a game loses Tab. The `world` example
selects its next entity with it, and games often bind it, to an inventory, a map or the scores.
Only Tab is lost so: with no focus, the arrows, Enter, Space and Escape reach the program. egui's
overlay left Tab to the program unless one of its fields took the keyboard.

A game's interface still needs a way in from the keyboard, for whoever does not use a pointer and
for the screen reader that follows them. The owner decided on 2026-10-27, among the options that
"Alternatives considered" lists.

## Decision

**A setting of the interface: whether Tab enters it.** `Ui::set_tab_enters` tells whether Tab and
Shift+Tab, while no focus takes the keys, give the focus to the interface: none holds it, or one
that a press gave, which takes no keys (ADR-0152). By default they do, as a program whose window is
its interface wants, the editor and the examples of widgets: Tab then goes on from the element that
a press gave the focus, as browsers do. A game turns it off: its interface takes Tab only once it
holds a focus that takes the keys, given by F6, by an agent, or by the game itself, which gives
its menu the focus as it opens it. Godot and Unity work this way: the keys move the focus only from
a control that holds it. A modal root takes Tab, as it takes every key, whatever the setting.

**F6 moves between the interface's roots.** F6, in every interface, gives the focus to the first
element of the next root that holds one, from the top root down, then gives it back to nothing,
the keys to the program; Shift+F6 goes the other way. The layers that close as the user leaves
them close first, and a tooltip is no root to reach. While a modal root is open, F6 does nothing
more than any other key does. GTK, Windows and browsers move between a window's parts with it; it
is the way from the keyboard into a game's interface, the overlay's panels among them.

## Alternatives considered

- **Another key for the `world` example**: nothing to change in the interface, but every game that
  binds Tab loses it while one of the overlay's panels shows, and while its own interface shows a
  button.
- **The setting on each root**: a game's menu taking Tab and its heads-up display not, but no
  program needs two roots that differ so, and the game can give its menu the focus as it opens it.
- **The setting without F6**: no key reaches a game's interface, for a keyboard user or a screen
  reader, unless the game gives it the focus.

## Consequences

- `slime-ui` gains the setting and F6. The programs that the overlay is hosted in, *Hunna Wood* and
  the `world` and `scene` examples, turn the setting off.
- A game loses F6 while its interface shows a root that holds an element that can take the focus.
