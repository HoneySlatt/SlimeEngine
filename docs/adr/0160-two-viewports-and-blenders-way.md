# 0160. Two viewports and Blender's way

- Status: Accepted; amends [0158](0158-the-viewport.md) and
  [0159](0159-picking-and-the-selection.md) (the mouse's buttons over the viewport bound by the
  table of shortcuts), [0142](0142-input-and-focus.md) (a chord may name a button of the mouse) and
  [0155](0155-the-editors-design.md) (the check that every entry of the editor's tables names an
  operation leaves out what the buttons do, which no operation is)
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided that the editor shows a second viewport, the one
being navigated alone drawn at the full rate, and that a `.keys` preset gives Blender's way beside
the editor's own, its mouse's buttons with it. [ADR-0158](0158-the-viewport.md) left Blender's way
for picking, which [ADR-0159](0159-picking-and-the-selection.md) brought, and moved both to
milestone M6c.2c.

What the code held before this milestone:

- The editor's window had one viewport: one surface, one camera, one view, one set of settings.
- Which button flies, orbits or pans was written in the editor's code: the right button, Alt and the
  left button, the middle button; the left button selected.
- A table of shortcuts (`slime_ui::shortcuts`, [ADR-0142](0142-input-and-focus.md)) bound chords of
  keys to commands, the editor's to its operations' names
  ([ADR-0156](0156-the-editors-window-and-its-operations.md)), and named no button of the mouse.

## Decision

**A chord may name a button.** A chord of a table of shortcuts may end with a button of the mouse,
`LeftButton`, `RightButton`, `MiddleButton`, `BackButton` or `ForwardButton`, in place of a key. Its
command is not run as a key's is: the program reads it, `Ui::button_command(scope, button,
modifiers)`, as the editor reads what a press over its viewport does. A button's chord holds while
the modifiers that it names are held, whatever others are, and of those that hold, the one that
names the most wins, a later table's before an earlier one's: `LeftButton` selects with Shift held
too, which adds, and `Alt+LeftButton` orbits. A key's chord keeps its rule, every modifier held
named.

**The editor's buttons are its table's.** The scope `viewport` of `editor.keys` binds each button
to a gesture, `fly`, `orbit` or `pan`, held while the button is, or to `select`, a click or a
marquee, or `through`, a click through what lies behind: by default, the right button flies, Alt
and the left button orbit, the middle button pans, the left button selects, and an orbit of the
left button let go before the mouse moved goes through, as ADR-0159 decided.

**Blender's way is a table laid over the editor's.** `editor-blender.keys` holds what Blender's way
changes: the middle button orbits, Shift and the middle button pan, Alt and the left button go
through what lies behind and orbit no more, Alt+A selects nothing, F3 finds an operation, and the
period frames what is selected; the rest stays the editor's, the right button flying. The check
that every entry of the editor's tables names an operation, which ADR-0155 asks, leaves out the
scope `viewport`, whose entries name what a button does. *View* > *Keys of most editors* and
*Keys of Blender* choose (`view.keys_default`, `view.keys_blender`, the one on ticked), and the
choice is kept in the user's state directory, `slime-editor/keys`, for every program of the editor:
a person's hands are the same whichever game they edit.

**Two viewports.** *View* > *Two viewports* (`view.split`) shows a second viewport beside the first,
seeing as it sees, and one again. Each is a pane of the window of its own: its surface, its camera,
what its buttons pick, its settings, its wireframe, its view and its image, the level's meshes,
frame builder, icons and picker shared by both;
the View menu and F change the one that the pointer last pressed in. Tools name them `viewport`
and `viewport 2`, capture them, change their settings, read their reports and pick in them. While
a gesture holds the cursor in one, or its keys fly it, the others draw at most every tenth of a
second, whatever changes in what they show: the one navigated alone draws at the full rate. A
picking asked of a resting viewport is drawn at once.

## Alternatives considered

- **The buttons in a table of the editor's own**, apart from the shortcuts: two documents that bind
  what a person presses, where ADR-0156 keeps one place for it.
- **A button's chord matched as a key's, every modifier named**: Shift and the left button, which
  add to the selection, and Shift and the right button, which fly faster, would each need a chord
  of their own, a table twice as long.
- **Blender's way as a whole table**, every key written again: two copies of what does not change,
  which drift.
- **Blender's flying, Shift and the backtick**, a mode rather than a held button: the right button
  flies in both ways, which Blender gives a context menu that the viewport does not have.
- **The choice of keys per program**: a person who edits two games would choose twice.
- **The second viewport from above or in orthographic**: a view chosen for the user; it sees as the
  first sees, and the user turns it.
- **The resting viewports not drawn at all while one is navigated**: what an agent changes, or
  meshes that load, would show only once the gesture ends.

## Consequences

- A table of shortcuts binds what the mouse's buttons do where a program reads them; a game's
  interface may do the same.
- The editor's window holds a list of panes, one or two, which a later milestone's docking can
  arrange; each viewport costs its own view's targets, the level's meshes loaded once for all of
  them, through one asset cache and one frame builder that they share.
- Blender's modal G, R and S keys and its pie menus are not part of its way here
  ([ADR-0155](0155-the-editors-design.md)).
