# 0142. Input and focus

- Status: Accepted; amends [0131](0131-stage-6b-and-stage-6c.md) (M6b.5 cut in two: input and
  focus, then what agents see); amended by [0145](0145-text-edited.md) (`slime-app` gives the
  window's events to a presentation's interface, whose input calls give whether they took each;
  fields hold the focus too; a press on what the interface draws, and every press and key while a
  modal root is open, are the interface's), [0150](0150-layers.md) (roots that the framework
  places, and closes on a press outside them or Escape) and
  [0152](0152-the-dev-overlay-on-the-engines-interface.md) (a focus that the pointer gave takes no
  keys but a field's and an open list's), [0153](0153-tab-and-f6-in-a-games-interface.md) (Tab
  with no focus that takes the keys enters only an interface that allows it; F6 moves between
  roots), [0154](0154-floating-panels-and-held-keys.md) (a held key gives a shortcut's command
  once) and [0160](0160-two-viewports-and-blenders-way.md) (a chord may name a button of the mouse,
  which the program reads)
- Date: 2026-10-25

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided that a person's input becomes an *action* of
the element it reaches, a click, a value set, a node expanded, or a navigation intent, next,
previous, a direction, accept, cancel; that an agent and a screen reader send the same actions; that
what changes the world leaves the interface as a command; that input goes to the top layer of the
interface that takes it, popups and modals above panels; that focus has scopes, Tab order and
directions; and that shortcuts are data that name commands. `slime-ui` must not use a window or the
platform's events, so that it runs headless. [ADR-0140](0140-the-tree-of-elements.md) left popups,
which no element out of its parent's flow can be, to a root or a layer of their own.

M6b.5 was to bring all of this with the tree's meaning and the agents' tools. The owner cut it in
two on 2026-10-25, before it started: M6b.5a, input and focus, which this ADR decides, and M6b.5b,
what agents see. Building M6b.5a raised what ADR-0132 left open: how the program learns what a
person did, where the popups live, where the focus goes along a direction, where the shortcuts live
and which key they name, and where the vocabulary of input lives. The owner decided each that day,
among the options that "Alternatives considered" lists.

## Decision

**One vocabulary of input, `slime-input`.** The engine's input types, a key's position (`KeyCode`)
and meaning (`Key`, `NamedKey`), a key's press or release (`KeyEvent`), the modifiers, the mouse's
buttons and the wheel, leave `slime-platform` for a crate of their own, without the windowing
library. `slime-platform` translates winit's events into them and re-exports them, and `slime-ui`
reads them as they are. The names of the key codes and of the named keys are listed once, in two
macros of `slime-input` that declare the enums and that `slime-platform` translates winit's values
by.

**Input becomes actions, in a queue.** `Ui` takes the window's input as it comes: the pointer's
moves and buttons, the wheel, the modifiers and the keys. A button pressed and let go over it gives
an action, `ActionKind::Press`, of that button; the pointer stays with the pressed button until it
is let go. The wheel scrolls the box or the list under the pointer. A key gives first the command of
a shortcut, then an intent: Tab and Shift+Tab next and previous, the arrows a direction, Enter and
Space accept, Escape cancel. `Ui::act` takes an action from an agent or the program by the same
path, within the same limits: a button disabled, hidden or under a modal root is not pressed. The
interface does the intents that move the focus itself; the presses, the commands, cancel, and
accept where no button holds the focus, wait in a queue, which the program takes
(`Ui::take_actions`) and turns into what it changes, a command of its own.

**The states that input sets.** The button under the pointer is hovered, the one held pressed, the
element with the focus focused: states of the element, which style sheets select by
([ADR-0141](0141-style-sheets-and-themes.md)). The program sets whether an element is disabled;
the interface keeps the focused state true to its focus. A button disabled, or hidden with an
ancestor, takes nothing, and loses the focus at the next frame.

**Layers are roots, stacked.** A popup, a menu or a dialog is a root of its own over the window's,
at the place that the program gives it (`RootSettings::origin`); roots stack in the order they are
added, the last drawn last and reaching the pointer first. A modal root (`RootSettings::modal`)
keeps the pointer from the roots below and the focus among its own elements, takes the focus when it
opens, and gives it back to the element that held it when it is removed. Modal roots close in any
order: one closed below another leaves what it would give back to the one above.

**The focus: the tree's order for Tab, the screen for directions.** Buttons that are not disabled
hold the focus, the one clicked taking it. Tab and Shift+Tab go to the next and the previous in the
order of the tree, around; an arrow goes to the nearest on screen that way, of those whose middle
lies that way, a step aside counting twice. The focus moves within its scope: the top modal root, or
the root that holds it; without a focus, Tab and Shift+Tab start in the top root that can take it,
from its first and its last element.

**Shortcuts are documents.** A table of shortcuts, `slime_ui::shortcuts::Shortcuts`, holds a table
for each scope, `global` or the class of an element, which holds while the focus lies inside such
an element, innermost first; each entry names a command for a chord, modifiers and a key,
`"Ctrl+Shift+Z" = "redo"`. A later table wins over an earlier one for one chord in one scope. Its
types are checked when the interface reads it (`Ui::set_shortcuts`), as a style sheet's are; the
cooker cooks a `.keys` file into its binary encoding, of the kind `AssetKind::Shortcuts`, and a
program loads it again when it changes. Its errors and the style sheets' are one type,
`DocumentError`.

**A chord names the key's meaning.** Ctrl+Z is the key labelled Z, wherever the keyboard's layout
puts it: the logical key, a character, either case, or a named key. The keys that move a player in
a game stay apart, by their position. Two cases bend the rule. A character without case, `+` or
`1`, is typed with Shift on some layouts and without on others: its chord names no Shift and
matches either way. A layout of another script than the Latin one types no Latin letter: there, a
chord of a letter is the key at that letter's place on a QWERTY keyboard.

## Alternatives considered

- **Functions attached to elements**, called when a button is pressed: natural to write, but they
  would own the state of the program that they change, which Rust makes heavy, and they compare
  badly from one build to the next.
- **Questions asked of the tree** while a panel builds, as egui asks them: a panel not built that
  frame would see nothing, and agents would have no path of their own.
- **An element out of its parent's flow** for a popup, CSS's `position: absolute` and `z-index`:
  against ADR-0140, and CSS's stacking rules are a known source of bugs.
- **The tree's order for the arrows**: simpler, but wrong for a grid, the down arrow going right.
- **Shortcuts given in Rust first**, files later with the editor's settings: less work now, a format
  to add then.
- **A chord of the key's position**, the same gesture on every layout: the letter shown in a menu
  would not be the one on the user's key.
- **A vocabulary of input of `slime-ui`'s own**, which `slime-app` would translate into: two
  vocabularies of keys to keep alike, and `slime-app` depending on the interface.
- **The translation in each program**: the same match written in every program that has an
  interface.

## Consequences

- `slime-platform` re-exports the types it moved, so that its users keep their paths; `Modifiers`
  stays open to more modifiers, so that the crate that translates into it sets it field by field.
- A program translates the window's events into the interface's calls, one line each, since
  `slime-app` gives the presentation the platform's events as they are.
- Buttons alone hold the focus and take the pointer; the boxes that scroll and the lists take the
  wheel. Text fields, lists of rows that select and the other widgets come with the milestones that
  build them.
- A modal root is modal from when it is added; a root made modal later stays as it was added.
- The interface's actions are not yet the editor's or a game's commands: the program turns them
  into its own. What changes a world, and what an agent reads of the tree, come with M6b.5b.
