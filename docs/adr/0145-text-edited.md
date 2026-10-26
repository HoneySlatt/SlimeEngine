# 0145. Text edited

- Status: Accepted; amends [0131](0131-stage-6b-and-stage-6c.md) (M6b.6 cut in two: text edited,
  then screen readers), [0132](0132-the-engines-ui-framework.md) (the engine composes the text of
  input methods and keeps a field's text, parley giving the selection's moves) and
  [0142](0142-input-and-focus.md) (`slime-app`, not each program, gives the window's events to a
  presentation's interface, which takes a press on what it draws, and every press and key while a
  modal root is open; fields hold the focus too)
- Date: 2026-10-26

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided that parley shapes and edits text, input
methods included; that `slime-platform` gains input methods, the clipboard through arboard, with its
Wayland feature, and cursor shapes; and that a field of numbers shows the value it holds whole and
changes it only when the user changed it. [ADR-0142](0142-input-and-focus.md) left each program to
translate the window's events into the interface's calls, and
[ADR-0143](0143-what-agents-see-of-the-interface.md) gave `slime-app` the presentation's interface
(`Presentation::ui`).

M6b.6 was to bring text edited and screen readers. Before it started, the owner cut it in two on
2026-10-25: M6b.6a, text edited and what the platform lacks for it, which this ADR decides, and
M6b.6b, screen readers. Building M6b.6a raised what the earlier ADRs left open: who connects the
window to the interface, who keeps a field's text and how the program reads it, how the interface
reaches the clipboard, which of the interface and the game takes an event, and on what a field is
built. The owner decided each on 2026-10-25 and 2026-10-26, among the options that "Alternatives
considered" lists. The milestone's review then found that a click on a panel's box, or beside a
dialog, reached the game; that the first arrow pressed in a game gave a button of its interface the
focus, and the arrows to the interface from then on; and that a program could neither clear a field
after a message sent nor give back a text that it refused, since it gave the same text as before.
The owner decided these three on 2026-10-26.

## Decision

**`slime-app` connects the window to the interface.** It gives each event of the window to the
presentation's interface first: the pointer's moves and buttons, the wheel, the modifiers, the keys
and what an input method composes. It gives the interface the system's clipboard. After each frame
drawn, it shows the pointer's shape that the interface asks for, and allows the input method, placed
beside the caret, while a field takes text, forbidding it otherwise, since it takes keys that a game
reads. A program no longer translates the window's events itself.

**The interface takes an event before the game when the event reaches it.** A press of any of the
mouse's buttons reaches the interface when it falls on what the interface draws, a button or a
field, a panel's box, a label or an image, and its release with it; a key, when a field that holds
the focus takes it, a shortcut names it, or it is a key of navigation, Tab, an arrow, Enter, Space
or Escape, while an element holds the focus; a wheel, when a box that scrolls or a list lies under
the pointer, or when it turns over what the interface draws; a composition, when a field holds the
focus. While a modal root is open, every
press, key and wheel reaches the interface. Without a focus, Tab alone gives it, to the first
element, and Shift+Tab to the last: the arrows, which a game moves with, stay the game's. The
interface's calls say so (`Ui::pointer_button`, `Ui::key`, `Ui::wheel`, `Ui::ime` give whether
they took the event), and an event taken reaches neither the presentation's `on_event` nor the
game's input. The pointer's moves, the modifiers and the releases of keys reach both. A key that the
interface does not take still gives its intent to the interface's queue: Escape with nothing
focused cancels for the program and reaches the game. A game with no element focused, its menus
closed, receives every key but Tab and those that its shortcuts name; a field being typed in does
not move its hero, and a click on its inventory's panel does not make the hero strike. A press that
reaches no button and no field takes the focus away, the field that held it committing, so that the
keys are the game's again once the player clicks back into the world.

**A field keeps its text, and the program reads it as actions.** A field of text, one line or
several (`Element::text_field`, `Element::text_area`), keeps what the user types, its selection, the
text that an input method composes and its undo. The program reads `ActionKind::Changed` at each
edit, and `ActionKind::Committed` when Enter, in a field of one line, or the focus leaving commits
it, once the user changed it since it took the focus or since its last commit. The text that the
program gives when it builds the field replaces the field's only when it differs from the later of
the text that the program gave before and the text that the field last committed, and from what the
field holds as the program reads it: building a panel again keeps what the user typed, a program
that keeps what the field commits gives it back without changing it, and one that gives back each
change, as an inspector showing a value does, keeps the user's spelling of a number and the text
that an input method composes; a program clears a field after a message sent, or gives back the
text it had for one that it refuses, by giving the text it gave before. A field of numbers (`Element::number_field`)
shows its number whole, as Rust writes it, reads what is typed as a number, and is dragged left and
right, a step for each logical pixel, rounded to the step's decimals; it commits a number only when
the user changed it, and shows again the number that the program gave or that it last committed
when its text reads as none.

**The field on the engine's text, parley moving its selection.** A field's text is laid out as a
label's, by the same path, its fonts named the same way. parley moves its selection on that layout:
by clusters and words in the order they show, in text of both directions, by lines and to a line's
ends, to a point that the pointer gives. The engine keeps the text, the composition of an input
method, which shows under a line in place of the selection until it is committed, and an undo of
the field's own, Ctrl+Z and Ctrl+Shift+Z or Ctrl+Y, typing joined into one step, as the steps of
one drag are. A composition that starts over a selection deletes it, as typing does; while it
composes, the keys that reach the field are the input method's, and edit nothing; the focus
leaving the field takes the composition away, and `slime-app` turns the input method off and on
again when the focus goes from one field to another, so that it begins anew.

**The clipboard is given to the interface.** `slime-ui` declares what it needs of a clipboard, to
read and write text (`slime_ui::tree::Clipboard`); `slime-app` gives the interface the platform's,
`slime_platform::Clipboard`, on arboard; a test gives its own. A paste reads it at once.

**What the platform gains.** `slime-input` names what an input method does (`Ime`) and the pointer's
shapes (`CursorShape`). `slime-platform` reports what an input method composes and commits
(`Event::Ime`), and a window allows its input method, places it and sets the pointer's shape;
`slime_platform::Clipboard` reads and writes the system's clipboard of text, through arboard 3.7,
without its images, with the Wayland protocol that the compositors that give it give
(`wayland-data-control`), and X11's otherwise.

## Alternatives considered

- **M6b.6 as one milestone**: one pass of checks by hand, but a long one before the first push, and
  screen readers checked with fields that had not been used yet.
- **Each program connects the window**, as ADR-0142 left it: free to do otherwise, but the same
  lines in every program, the clipboard and the input method each program's to serve.
- **The program keeps a field's text**, given back at each build, as React does: every program
  handling typing, selection and composition itself.
- **A queue of the clipboard's requests**, served at the next frame: no trait, but a paste a frame
  late, and more code.
- **Every event to both** the interface and the game: simple, but a letter typed in a field would
  move the hero. **Any key while an element holds the focus to the interface**: a button clicked in
  a game's interface would keep its keys from the game until the focus left it.
- **Presses taken by the elements that act alone**, buttons and fields: a click on a panel's box,
  on a label, or beside a dialog, would reach the game, and make its hero strike.
- **Tab and the arrows give the focus** where none is held: the first arrow pressed in a game would
  give a button of its interface the focus, and its arrows to the interface from then on.
- **A call that sets a field's text**, `Ui::set_field_text`: explicit, but a second way to give a
  field its text, beside the builder's.
- **parley's `PlainEditor`**: less code, its buffer, moves and composition given; but its text laid
  out by a second path, with styles of its own, a text in a field taking other fonts than the same
  in a label, and two ways to lay out text in the engine.
- **window_clipboard or smithay-clipboard**, as ADR-0132 recorded.

## Consequences

- `Ui::pointer_button`, `Ui::key`, `Ui::wheel` and `Ui::ime` give whether they took the event; a
  program that has `slime-app` run it gives them nothing itself.
- A game's interface that lets presses through to the world draws nothing where they fall: a fill,
  a border or a shadow, a text or an image takes them.
- `ActionKind` gains `Changed` and `Committed`, of a `FieldValue`, a text or a number: an `Action`
  is no longer `Eq`.
- A field takes the keys it types and edits with before the shortcuts: a shortcut of a letter does
  not fire while a field holds the focus, and Ctrl+Z undoes the field's typing rather than giving
  the program's command. Tab and Escape, and the up and down arrows in a field of one line, go on to
  the focus.
- The `interface` example falls back on the system's fonts for what its built-in fonts do not hold,
  as the editor will, so that text typed in its fields shows in any writing system.
- Agents do not type in a field yet: its value, and the action that sets it, come with M6b.6b.
