# 0154. Floating panels and held keys

- Status: Accepted; amends [0150](0150-layers.md) (a floating panel takes no focus as it opens,
  and gives it to nothing as it closes) and [0142](0142-input-and-focus.md) (a held key gives a
  shortcut's command once)
- Date: 2026-10-27

## Context

[ADR-0150](0150-layers.md) gave a layer opened while the last input was not a press of the pointer
its first element's focus, as a menu opened by the keys needs, and gave the focus back to what held
it as the layer closes. [ADR-0142](0142-input-and-focus.md) made shortcuts chords that name
commands, and let a key's repeats reach the interface as its presses do, as a field's typing needs.
Building M6b.8b, the dev overlay over the games' views
([ADR-0152](0152-the-dev-overlay-on-the-engines-interface.md)), on the same interface as a game,
showed three faults that these rules made:

- F1, a shortcut of the overlay, opened its panel of statistics as a key, so that the panel's bar
  took the focus, and with it the arrows and Space that the game plays with.
- A panel closed by its chord gave the focus back to what held it as it opened, as the keys gave
  it, which took the game's keys again: a field typed into what the player meant to walk with.
- A held F1 gave its command at each repeat, the panel opening and closing at the rate of the
  repeats, its last state left to chance.

They were fixed while building the milestone, and the owner took them as decisions on 2026-10-27,
after the release of 0.6.

## Decision

**A floating panel stands beside the work, not over it.** A floating layer (`Place::Floating`)
takes no focus as it opens, whatever the last input was, and gives the focus to nothing as it
closes, if it held it, since it took it from no one. The keyboard
reaches a floating panel through F6, which moves the focus from one root to the next
([ADR-0153](0153-tab-and-f6-in-a-games-interface.md)). Menus, lists, popups and dialogs keep
ADR-0150's rule: they take the focus as the keys open them, and give it back as they close.

**A held key gives a shortcut's command once.** The press of a shortcut's chord gives its command;
its repeats are taken and give none, so that a chord that shows and hides something does not make
it flicker. A field still types and edits with a key's repeats, and the arrows of a list or a
slider still move at each.

## Alternatives considered

- **The overlay working around both**, closing its panels' focus itself and ignoring repeated
  commands: no change to the framework, but every program that opens a floating panel by a key, the
  editor's among them, meets the same faults.
- **A setting of each layer**, whether it takes the focus: finer, but no program needs a floating
  panel that takes it, and a setting would be one more for each layer to say.
- **Repeats kept for a chord with a modifier**, as Ctrl+Z held undoes again in many editors: closer
  to them, but a rule that the chord's modifiers change surprises, and the editor of Stage 6c may
  ask for a command that repeats, a property of its shortcut then.

## Consequences

- `slime-ui`'s floating layers and its shortcuts behave as above in every interface, the `widgets`
  example's panel of statistics among them, which no longer takes the focus as *View* opens it.
- A command held to repeat, Ctrl+Z held to undo again, needs a shortcut that says so, if the editor
  asks for one.
