# 0162. Creating, copying and arranging

- Status: Accepted; amends [0155](0155-the-editors-design.md) (a duplicate and what is pasted go
  last among their parent's children, in place; nudging moves along the world's axes)
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided what the user does to create and arrange: Ctrl+D
duplicates, the references inside the copy pointed at the copy, nested overrides kept; copy and
paste carry commands as text on the system's clipboard; the last operation repeats; Ctrl+G
groups. Milestone M6c.3b, which was to bring them with placing prefabs and the history panel, was
cut in two at its start, on 2026-10-28, as ADR-0155 allows: M6c.3b, creating, deleting,
duplicating, copying, grouping and nudging, which this ADR records; M6c.3c, placing prefabs and
the history panel.

What the code held before this milestone:

- The world's commands ([ADR-0134](0134-the-editors-core.md)) create an entity of a scene with its
  components, place a prefab, delete, reparent, and insert, remove or set a component; each is
  written as text, references to entities by their persistent identities, and read back as the
  world is when it comes, so that a command may name what one before it in a step created.
- A placement remembers its prefab; a scene file written from the world holds, for each, what
  differs from its prefab, worked out as it is written
  ([ADR-0092](0092-a-world-written-back-as-a-scene.md)).
- The editor's journal writes a step as its label, the selection after it, and its commands' texts,
  each after a line `# command`.
- `slime-ui`'s interface reads and writes the system's clipboard for its fields alone
  ([ADR-0145](0145-text-edited.md)).

## Decision

**Copying writes the commands that create what is selected.**
`World::creating_commands(roots)` gives the commands that create again the subtrees of `roots`, as
they are: a placement as `Place`, then `Delete` for each entity of its prefab that it lost and
whose parent it kept, and `Remove` for each component that one lost; any other entity as
`Create`, with its components; the entities below each, in the order of the hierarchy. Then
`Insert` for each component of a placement's entities whose value differs from its prefab's, or
that the prefab does not give, and for each component of another entity that refers to an entity
among those created: inserted after every entity is created, so that each command reads what came
before it. Each command keeps the identities of what it copies. *Edit* > *Copy*
(`edit.copy`, Ctrl+C) writes them, the outermost of what is selected, as a step of the journal
writes them, and puts the text on the system's clipboard, and the editor keeps it too, which a
program without a window pastes from.

**Pasting reads them under new identities.** `Command::read_renamed` reads a command as
`Command::read` does, through a `Renaming`: each entity that a `Create` or a `Place` names gets a
new identity of the level, made as `World::new_id` makes one, from the old one's ID; each identity
that the commands name afterwards, the entity of a command, a parent, or a reference within a
value, is the new one of the entity that it named, or of the entity created whose identity it
extends, the longest, as a placement's entities extend its root's, and stays as it is otherwise. A
parent that the level does not hold, as when pasting into another level, is the level's root. A
command that is not one of those that copying gives, or that changes an entity that the commands
before it did not create, refuses the paste, so that a paste creates what was copied and changes
nothing else, whatever text the clipboard holds. *Edit* > *Paste* (`edit.paste`, Ctrl+V) reads the
clipboard's text, the system's where a window has one, as one step, which selects what it created:
the references inside the copy point at the copy, those to entities outside it at them in the same
level, and a placement's overrides, nested ones included, are what the level writes for the copy,
as they are worked out from the world. *Duplicate* (`edit.duplicate`, Ctrl+D) copies and pastes
what is selected in one step, without the clipboard. What is pasted and duplicated goes last among
its parent's children, where it was in the world.

**Creating, deleting, grouping and nudging.** *Create an entity* (`edit.create`, Ctrl+Shift+N)
creates an entity named `entity`, at the level's root, at the middle of what is selected or at the
origin, and selects it; *Delete* (`edit.delete`, the Delete key) deletes the outermost of what is
selected; *Group* (`edit.group`, Ctrl+G) creates an entity named `group` at the middle of what is
selected, under the parent that they share or at the level's root, and puts each of the outermost
of what is selected under it, where it was in the world, then selects the group. It refuses an
entity that a placement holds but for its root, which a scene file cannot move out of its place.
*Nudge* moves the outermost of what is selected by a quarter of a metre along the world's axes:
Ctrl and the left and right arrows along X, Ctrl and the up and down arrows along Z, away from and
towards the viewer who faces −Z, Ctrl and Page Up and Page Down along Y (`edit.nudge_left`,
`edit.nudge_right`, `edit.nudge_forward`, `edit.nudge_back`, `edit.nudge_up`, `edit.nudge_down`).
Each is one step.
*Repeat* (`edit.repeat`, Shift+R) runs again, with its arguments, the last operation that made a
step. Blender's way binds Shift+D to duplicating, X to deleting and Shift+A to creating.

**The interface's clipboard reaches the program.** `Ui::clipboard_text` and `Ui::set_clipboard_text`
read and write the clipboard that the interface was given, so that a program copies without a
field.

## Alternatives considered

- **The clipboard as a scene file of what is copied**, pasted as entities of that scene: a scene
  file writes placements and their overrides already, but pasting it would spawn what the journal
  cannot hold as commands; ADR-0155 chose commands, which an agent reads and sends as it sends a
  step.
- **New identities written into the copied text**: a text pasted twice would name the same
  identities twice; renamed as it is read, each paste makes its own.
- **Renaming the text's strings**: a value that holds an identity's text, a name, would change too;
  the renaming reads the identities where the commands and their values name entities, by their
  types.
- **Every component of a placement's entity inserted**: a placement of a thousand entities, as the
  zone's land, would make thousands of commands; only what differs from the prefab is.
- **A duplicate right after its original among its parent's children**: the commands would carry
  ranks that a paste into another parent would not mean; last is what both do.
- **Nudging along the viewer's axes**: the operation would need the viewport's camera, which an
  agent's editor without a window does not have; the world's axes are the same for both.
- **Nudging with the arrows alone**: a shortcut of the table comes before the arrows that move
  through a list, as the world panel's, which they would nudge what is selected from.
- **A duplicate moved aside, or held by the pointer as Blender's**: a copy in place, which the
  handles move, is what most editors do; Blender's modal keys come later.

## Consequences

- An agent copies, pastes and duplicates by the operations' names, and may send a copied text as a
  step of its own after renaming it, as a person pastes it.
- A copy pasted into another level needs the same prefabs and the same types, and the entities
  outside the copy that it refers to: a prefab that the program cannot resolve, a component that it
  does not know, or a reference to an entity that the level does not hold, refuses the paste,
  whole.
- Nudging along the world's axes surprises a user who looks along +Z: the arrows go the other way.
- M6c.3c brings placing prefabs from the manifest's scenes and the history panel.
