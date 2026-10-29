# 0169. The eye and the lock

- Status: Accepted
- Date: 2026-10-29

## Context

[ADR-0155](0155-the-editors-design.md) decided that what is hidden, locked or folded is the user's,
never the level's, and never undone: the editor hides nothing through `World::is_hidden`, a flag of
the level that the frame builder reads. [ADR-0168](0168-rearranging-the-hierarchy.md) cut milestone
M6c.5b in two, the eye and the lock of each row of the hierarchy becoming M6c.5c, which this ADR
records.

What the code held before this milestone:

- `slime-world-render`'s `FrameBuilder` draws every entity that places meshes or lights but those
  that `World::is_hidden` hides, by the `Hidden` component of the entity or of an ancestor; its
  `Picker` casts rays through the same entities, and gives their boxes.
- The editor picks in a viewport by the GPU, a click and a marquee reading the numbers that each
  instance is drawn with, its entity's; by the CPU's picker, Alt and a click, and the drops of what
  is selected on what lies below it (`edit.drop`); and an agent picks through the development
  channel, the GPU answering. The icons of the lights are picked as their entities.
- The hierarchy keeps what is folded in memory while the level is open
  ([ADR-0167](0167-the-hierarchy.md)).

## Decision

**The user's, in memory.** Each row of the hierarchy shows an eye and a lock, buttons at its end,
which a press turns: the row's entity hidden or shown, locked or unlocked. What is hidden and what is
locked are two sets of persistent identities that the editor's core and its window share, kept in
memory while the level is open, as the folds are: never written to the level nor journaled, never
undone, forgotten when another level opens. An entity is hidden, or locked, when it or one of its
ancestors is, as `Hidden` hides a subtree; the window turns the sets into the entities that they
cover again only when they or the world's structure change.

**What is hidden is not there.** It is not drawn, nor its light, nor its light's icon; it is not
picked, by a click, a marquee, Alt and a click, or an agent; a drop's ray passes through it; framing
leaves it out. `FrameBuilder` and `Picker` are told the entities to leave out besides those that
`World::is_hidden` hides (`set_hidden`), the editor's alone.

**What is locked is seen, not picked.** It is drawn and lights the level; a click on it selects
nothing, a marquee leaves it out, Alt and a click go through it, and an agent's pick names nothing.
What is dropped lands on it: a level's ground is what a user locks, to place things on it without
picking it.

**Agents** press the buttons, found by their names, `hide` or `show`, `lock` or `unlock`, in the
entity's row, as a person does.

## Alternatives considered

- **The sets kept in the user's state directory, per level**: what is hidden would outlive the
  editor, as a user may expect of a large level; kept in memory as the folds are
  ([ADR-0167](0167-the-hierarchy.md)), until a user asks.
- **A click on what is locked going through to what lies behind**: the GPU's picking draws every
  instance; leaving the locked ones out of the picking but drawing them asks `slime-render` for
  instances that its picking leaves out, which waits for a user who misses it. Alt and a click go
  through already.
- **A drop's ray passing through what is locked**: the ground, locked so that a click does not pick
  it, would let what is dropped fall through it.
- **A light hidden that still lights**: two meanings of hidden in the engine, where `Hidden` leaves
  the light out; what is hidden is not there.
- **Operations that hide and lock what is selected, H and Alt+H**: the rows' buttons give persons
  and agents the eye and the lock that ADR-0155 decided; shortcuts wait for a user who asks.

## Consequences

- Hiding or locking a subtree costs a walk of it, once, as the structure changes.
- A frame, a pick and a drop read a set of the entities hidden, besides the flag of `Hidden`.
- What is hidden or locked is lost when the editor stops, or another level opens.
