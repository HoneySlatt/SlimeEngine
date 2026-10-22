# slime-editor

SlimeEngine's editor. Today its core, without an interface: a level changed through commands,
undone and redone, journaled, and written back (`docs/adr/0109-stable-references-and-undo.md`,
`docs/adr/0134-the-editors-core.md`). Its interface comes with Stage 6c, on the engine's UI
framework (`docs/adr/0132-the-engines-ui-framework.md`).

## Role

- `Editor::open` opens a level: a scene file, its asset ID read from its metadata file, spawned
  into a world under the key that the game spawns it under, the prefabs that it places resolved
  through a library of the cooked scenes.
- `Editor::apply` and `Editor::apply_text` change the level through the world's commands
  (`slime_world::Command`), given as values or as text, grouped into a step that is made whole or
  not at all; a command that would change what is not under the level's root is refused.
  `Editor::undo` and `Editor::redo` undo the last step and make it again, within a bound of memory
  (`HISTORY_BUDGET`). Commands name entities by their persistent identities, which hold where
  handles do not: undoing a deletion creates the entities again, under the same identities, at the
  same places, and the references to them point at them again.
- `Editor::select` and `Editor::selection`: what is selected, by persistent identity. Selecting is
  not a step, but undoing or redoing a step selects again what was selected before or after it.
- `Editor::new_id` gives what the editor creates the identity that the level's file gives it,
  `zone/torch-0rdx`, at once.
- `Editor::write` writes the level back to its file, whole or not at all, with the comments of the
  file kept (`slime_serialize::keep_comments`).
- `Editor::keep_journal` keeps a journal of the steps, as text, written as they are made, from the
  level as it is before any step; `Editor::recover` opens the level from the journal and plays it
  again: what recovers an editor that crashed, with its history and its selection, and what
  replays a session of tests or of an agent. `Editor::keep_journal`'s documentation describes a
  journal's entries.

## Depends on

`slime-world`, whose commands, levels and identities the editor uses through their public API;
`slime-serialize` and `slime-reflect`, for its journal and the comments of levels; `slime-cook`,
whose metadata files give a level's asset ID, and which writes files whole or not at all.

## Must not

- Hold a game's types: it reaches a game's components through reflection only, the world's
  registry and reflected values (`docs/adr/0110-how-the-editor-runs-a-games-code.md`).
- Change a world that something else changes meanwhile: what it undoes is what it did.
- Depend on a GPU, a window or the platform: the editor's window comes with Stage 6c, in a crate
  or a program above this one.
