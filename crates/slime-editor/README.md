# slime-editor

SlimeEngine's editor. Today its core, without an interface: a level changed through commands,
undone and redone, journaled, and written back (`docs/adr/0109-stable-references-and-undo.md`,
`docs/adr/0134-the-editors-core.md`); and its program, without a window, which tools and agents
drive through the development channel (`docs/adr/0135-the-editors-program.md`). Its interface
comes with Stage 6c, on the engine's UI framework (`docs/adr/0132-the-engines-ui-framework.md`).

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
  file kept (`slime_serialize::keep_comments`), and never over a file that changed apart from the
  editor since it read or last wrote it (`EditError::Changed`).
- `Editor::keep_journal` keeps a journal of the steps, as text, written as they are made, from the
  level as it is before any step; `Editor::recover` opens the level from the journal and plays it
  again: what recovers an editor that crashed, with its history and its selection, and what
  replays a session of tests or of an agent. `Editor::keep_journal`'s documentation describes a
  journal's entries.
- `run(&Program)`: the editor's program, which a game's editor package calls with the game's
  level, key, cooked data and types, and the `editor` example with the yard's. An application of
  `slime-app` without a window, its frames paced, whose simulation is the editor: it opens the
  level, keeps its journal in the user's data directory, one per level's file, held by this program
  alone, and recovers it as it starts when it holds steps that the level's file does not; a journal
  that it cannot read, play again or write stops it. It makes the commands that it receives steps,
  and answers the editor's requests of the development channel. Options: `--level`, `--assets`,
  `--journal`, `--discard`; `run`'s documentation says how a journal is chosen and recovered.

## Depends on

`slime-world`, whose commands, levels and identities the editor uses through their public API;
`slime-serialize` and `slime-reflect`, for its journal and the comments of levels; `slime-cook`,
whose metadata files give a level's asset ID, and which writes files whole or not at all;
`slime-app`, with its `development` feature, on which its program runs, `slime-channel`, whose
requests it answers, and `slime-assets`, the cooked data of the prefabs that a level places.

## Must not

- Hold a game's types: it reaches a game's components through reflection only, the world's
  registry and reflected values (`docs/adr/0110-how-the-editor-runs-a-games-code.md`).
- Change a world that something else changes meanwhile: what it undoes is what it did.
- Use a GPU or a window in its core, which a test runs without either: its program, on
  `slime-app`, opens them at Stage 6c.
