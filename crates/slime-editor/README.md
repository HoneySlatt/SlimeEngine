# slime-editor

SlimeEngine's editor: its core, a level changed through commands, undone and redone, journaled,
and written back (`docs/adr/0109-stable-references-and-undo.md`,
`docs/adr/0134-the-editors-core.md`); its operations, every action of its menus, shortcuts,
palette and agents, by name; and its program, in a window of its own on the engine's interface,
or without one, which a person uses and tools and agents drive through the development channel
alike (`docs/adr/0135-the-editors-program.md`,
`docs/adr/0156-the-editors-window-and-its-operations.md`). The window shows a menu bar, the dev
overlay's world panel, a viewport in its center and a status bar; the editor's other panels come
with Stage 6c's next milestones.

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
- `Editor::unwritten`: how many steps the level's file does not hold, which the window's title
  and status bar show.
- `Editor::gesture`, `end_gesture` and `cancel_gesture`: the *open step* of a person's gesture,
  its commands applied live, neither journaled nor in the history, made one step as it ends or
  undone as it is cancelled; while it is open, every other change is refused
  (`EditError::Gesture`), and the program keeps a tool's requests waiting
  (`docs/adr/0161-the-open-step-and-the-handles.md`).
- `Editor::copy` and `Editor::paste`: the text of the commands that create entities again, as a
  step of the journal writes them, and that text pasted as one step under new identities, the
  references inside the copy pointed at the copy and a placement's overrides kept
  (`docs/adr/0162-creating-copying-and-arranging.md`).
- `OPERATIONS`, `operation(name)`, `SHORTCUTS` and `BLENDER_SHORTCUTS`: the registry of the
  editor's operations, each its name, `file.save`, its label, its parameters and what it changes
  (`Changes`), of which only steps enter the history; the editor's table of shortcuts,
  `editor.keys`, which binds keys to operations' names and the mouse's buttons to what they do over
  the viewport; and what Blender's way changes of it, `editor-blender.keys`, laid over it when the
  user chooses it.
- `run(&Program)`: the editor's program, which a game's editor package calls with the game's
  level, key, cooked data and types, and the `editor` example with the yard's. An application of
  `slime-app` whose simulation is the editor's core and whose presentation is its window, both
  sharing a value that the core writes and the window reads, the window asking the core through
  requests that one handler answers, as it answers the development channel's; its frames run on
  demand (`AppConfig::on_demand`). It opens the level, keeps its journal in the user's data
  directory, one per level's file, held by this program alone, and recovers it as it starts when it
  holds steps that the level's file does not; a journal that it cannot read, play again or write
  stops it. It opens another level by `file.open`, given a path, or by `file.choose` (Ctrl+O),
  which opens the system's dialog behind a modal layer where the path can be typed too, and runs
  `file.open` with the file chosen; started by a tool, it opens no dialog, and the layer waits for
  the path (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`). It keeps the
  levels opened last in the user's state directory for the File menu, and tells one hint at its
  first run. Its viewport shows the level, lit as `Program::lighting` says, through its own asset
  cache and frame builder, drawn again only when what it shows changed; the right button held
  flies through it with W, A, S, D, Q and E, Alt and the left button orbit, the middle button pans,
  F frames the selection, and tools capture it by the name `viewport`
  (`docs/adr/0158-the-viewport.md`). A click selects the outermost placement of what is drawn
  there, a click again a level deeper, a marquee what it holds, Alt and a click the meshes behind;
  what is selected is outlined, and the View menu shows the base color, the light alone, the
  normals, the shadows and the wireframe of the selection; tools pick in the viewport by its name
  (`docs/adr/0159-picking-and-the-selection.md`). *View* > *Two viewports* shows a second, named
  `viewport 2`, the one navigated drawn at the full rate; *View* > *Keys of Blender* chooses
  Blender's way, kept in the user's state directory for every program of the editor
  (`docs/adr/0160-two-viewports-and-blenders-way.md`). Handles move (W), turn (E) and scale (R)
  what is selected, in the world's axes or the selection's, snapped if *Edit* > *Snap* is ticked,
  each drag one step that Escape cancels; End drops what is selected on what lies below it, Shift
  and End its bottom (`docs/adr/0161-the-open-step-and-the-handles.md`). *Edit* creates an entity
  (Ctrl+Shift+N), duplicates (Ctrl+D), copies and pastes through the system's clipboard (Ctrl+C,
  Ctrl+V), deletes (Delete), groups (Ctrl+G) and repeats the last operation (Shift+R), and Ctrl and
  the arrows, Page Up and Page Down nudge what is selected
  (`docs/adr/0162-creating-copying-and-arranging.md`). *Edit* > *Place a prefab…* places one of the
  cooked data's scenes, found in the palette, and a panel below the world panel shows the history,
  a press on a row going to its step (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`).
  Play (F5, a toolbar above the viewports) runs the game that `Program::play` starts on the level
  as the editor holds it, its steps not written handed over as private copies, paused, stepped,
  at a speed and stopped, its log in a console below the viewports (`docs/adr/0164-play.md`);
  the steps made, undone and done again while it runs reach it, the game's refusals told
  (`docs/adr/0165-editing-while-playing.md`). Play builds the game with cargo as `Program::build`
  says, as it starts it and again as its code changes while it plays, the game then starting again
  on a save of its run (`Played::resume`), a build that fails told
  (`docs/adr/0166-building-the-game-again.md`); the editor's log says how long the game took to
  give control, its time no longer held, which `slime-benches`' `play` benchmarks read.
  Options: `--level`, `--assets`, `--journal`, `--discard`, `--headless`; `run`'s documentation
  says how a journal is chosen and recovered.
- `window_image(&Program, arguments, operations, selection, scale)`: the window drawn into an image
  without a window of the system, as it shows once the level is open, the operations named run
  and the entities named selected, which the image tests compare and the `editor` example's
  `--output` writes; `file.choose` shows the layer that waits for the system's dialog, which no
  window opens there.

## Depends on

`slime-world`, whose commands, levels and identities the editor uses through their public API;
`slime-serialize` and `slime-reflect`, for its journal and the comments of levels; `slime-cook`,
whose metadata files give a level's asset ID, and which writes files whole or not at all;
`slime-app`, with its `development` feature, on which its program runs, `slime-channel`, whose
requests it answers, and `slime-assets`, the cooked data of the prefabs that a level places; for its
window, `slime-ui`, with the editor's look, `slime-ui-render` and `slime-render`, which draw it,
`slime-world-render`, what the viewport draws of the world, `slime-gfx` and `slime-color`, the GPU
and the color an image starts from, `slime-platform`, the window's events, and `slime-devui`, whose
world panel the window shows; `serde_json`, which reads cargo's messages as Play builds the game;
`thiserror`, which derives `EditError`, and `tracing`, the program's log.

## Must not

- Hold a game's types: it reaches a game's components through reflection only, the world's
  registry and reflected values (`docs/adr/0110-how-the-editor-runs-a-games-code.md`).
- Change a world that something else changes meanwhile: what it undoes is what it did.
- Use a GPU or a window in its core, which a test runs without either: only its window's drawing
  does, and its window's model, its menus and requests, runs without a GPU in the tests.
- Let the window change the level but through the core's requests: one handler answers a person
  and an agent alike.
