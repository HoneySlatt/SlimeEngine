# slime-mcp

The engine's MCP server: through it, an AI agent drives a game built with the engine as a person
would, without a person (`docs/adr/0099-ai-agents-drive-the-engine.md`,
`docs/adr/0122-the-development-channel.md`). The Model Context Protocol (MCP) is the open protocol
through which coding agents, Claude Code and Codex among them, call the tools of other programs.

## Status

One game at a time, driven through its development channel (`slime-channel`): started, paused,
stepped, its speed set, its world read and changed, its keys put down and let go, its image
captured, the settings of its views changed, its reports read, the entity at a pixel picked, its
interface read and used, its log and its frames' times read, its run saved and loaded, stopped. An
editor too, started as a game is, its level edited through steps of commands, undone and redone,
its history and journal read, its selection set and its level written back
(`docs/adr/0135-the-editors-program.md`). Assets cooked and tests run. No mouse in scripted input
yet: an interface is used through its elements' actions.

## Role

- Speaks MCP on its standard input and output, as agents start their servers: it opens no port of
  its own.
- Starts the game that an agent names, with a new token in its environment, and connects to the
  development channel that the game opens; the game's standard error, its log, goes to a file of
  the system's temporary directory, which `start_game` names and which stays there once the game
  ended. A game still running when the agent goes is stopped. After a request that failed, the
  server opens a new connection to the game, so that a late answer is not read as the next one's.
- Runs the engine's cooker and `cargo test` in the directory where it runs: run it from the
  repository's root.

## Giving it to an agent

An agent starts the server as a program of its own. Give it the command `cargo`, with the arguments
`run --quiet -p slime-mcp`, in the repository's root. With Claude Code, from the repository's root:

```sh
claude mcp add slime-engine -- cargo run --quiet -p slime-mcp
```

The first call builds the server. A game that the agent starts is built by its command too, when
the command is `cargo run`: its default features build it with the development channel
(`--no-default-features` leaves it out, and such a build never answers).

The game opens its window on the display that the server's environment names. To keep it off the
desktop, start the server with the environment of a headless Weston, and lavapipe to draw it
([`CONTRIBUTING.md`](../../CONTRIBUTING.md#driving-a-running-game)).

## Entry points

The tools, each described to the agent by the server itself, with its parameters:

- `start_game`: starts a game as its `command` runs it, the program and its arguments, and connects
  to it, waiting long enough for `cargo run` to build it first, or `timeout_seconds`. Give a game a
  save of its own: `["cargo", "run", "-p", "hunnawood", "--", "--new", "--save",
  "/tmp/agent-run.toml"]`. `stop_game` asks it to stop, as closing its window would, and kills it if
  it does not. A game that ended on its own is told at the next tool that asks it, and forgotten.
- `game_state`, `pause`, `resume`, `step` and `set_speed`: the host's controls of game time, and the
  views that the game's last frame drew, with their settings; a step answers once its steps have
  run.
- `list_entities`, `inspect_entity` and `set_property`: the world, its entities, which
  `list_entities` lists by a `filter` on their names and identities, empty for all, named by their
  persistent identities, `hero`, `zone/door`, its values written in the engine's notation, as a
  scene file holds them.
- `press_key` and `release_key`: keys, named by their position on the keyboard, `KeyW`.
- `capture`: the image that the game shows, or the part of it that one of its views draws, named
  as `game_state` lists them (`docs/adr/0126-views-apart-from-the-renderer.md`), as a PNG image.
- `set_view_settings`: a part of a view's settings, as `game_state` writes them, its render scale,
  anti-aliasing, passes, or a mode of debugging that shows the normals, the shadows, the base color
  or the light alone, `debug` set to `"Normals"` (`docs/adr/0127-the-settings-of-a-view.md`,
  `docs/adr/0159-picking-and-the-selection.md`).
- `read_report`: one of the game's reports, as `game_state` lists them: `assets`, its asset cache's
  requests by priority and its memory against its budgets; `views`, its views' statistics
  (`docs/adr/0128-asset-requests.md`).
- `pick`: the entity drawn at a pixel of the game's image, or of one of its views, counted as in
  the image that `capture` gives, by its persistent identity and name, or nothing; the game picks it
  on the GPU, exactly as it draws it, a frame or more later (`docs/adr/0130-picking.md`).
- `find_elements`, `element_at`, `act_on_interface` and `capture_element`: the game's interface, as
  its last frame left it (`docs/adr/0143-what-agents-see-of-the-interface.md`). `find_elements`
  lists its elements of a role, `Button`, and whose names hold a text, each by its path, which names
  it to the other tools, its role, name, what a field, a slider, two panels, a list to choose from
  or a color well hold, its description, which its tooltip gives, the text that a field shows while
  empty, its states, where it shows, and what it takes now; `element_at` names the one at a pixel of
  the image that `capture` gives; `act_on_interface` presses, focuses or scrolls one, gives a field
  a value as a person who replaced its text and committed would (`docs/adr/0146-screen-readers.md`),
  a slider or two panels theirs (`docs/adr/0148-widgets.md`), a list to choose from an option
  (`docs/adr/0150-layers.md`), and a color well a hexadecimal code, `#ff8000`
  (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`); menus, their bar and floating
  panels are pressed as buttons are; or it moves the focus, accepts, cancels or gives a command, as
  keys do; `capture_element` gives the part of the image where one shows.
- `read_logs`: the lines of the game's log after a given line.
- `save_game` and `load_game`: the game's run saved to a file, and loaded from one in place of the
  run, as the game saves and resumes its own runs; a file of the agent's, never the player's save.
- `edit`, `undo`, `redo`, `select`, `editor_history`, `editor_journal` and `write_level`: an
  editor's level, started with `start_game` as a game is, the `editor` example or a game's editor
  package, on a copy of a level and with a journal of the agent's (`--level`, `--journal`): a step
  of commands given as text, whole or not at all, undone and done again, entities selected, the
  steps done and undone, the selection and what the editor recovered as it started, its journal,
  and the level written back, with what its file could not hold.
- `list_operations` and `run_operation`: an editor's operations, every action that its menus, its
  shortcuts and its palette run, listed with their parameters and what they change, and one run
  by its name with its arguments, as a person runs it
  (`docs/adr/0156-the-editors-window-and-its-operations.md`). An editor that the server started
  opens no dialog of the system, which an agent could not see: *Open a level…* waits for the
  level's path, typed in its field, or the agent gives it `file.open`
  (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`).
- `cook`: cooks a directory of source assets, which a game started with `--watch` loads again.
- `run_tests`: runs `cargo test` with the arguments given.

`tests/drive.rs` plays an agent's session: it starts *Hunna Wood* through the server, pauses it,
sets its view to draw without shadows, without anti-aliasing and at half its size, so that each
request waits for a cheap frame, reads and changes the hero, walks it, saves the run, moves it and loads it back, opens the dev
overlay's panels by their commands, sets the speed of game time with a slider and the hero's
direction with the world panel, captures its image and reads its log. It needs a Wayland
compositor, the game built and its zone cooked: `scripts/check.sh` runs it in a headless Weston.
`tests/views.rs` starts the `world` example, which draws the yard and a map in a corner, captures
the whole image and each view, shows the map's normals, reads the example's reports, and picks the
entity at a pixel of the map; it needs the examples' assets cooked, in the directory that its first
argument names, and `scripts/check.sh` runs it there too. `tests/interface.rs` starts the
`interface` example, finds a button of its bar by its role and its name, presses it, reads in the
world the count that the press changed, captures the button and finds it at a pixel, moves the
focus, opens and closes the dialog, gives the bar's field of text a value, which the example reads
as committed, and sees a field of numbers refuse a text that reads as no number; it needs what
`tests/views.rs` needs, and runs beside it. `tests/widgets.rs` starts the `widgets` example and
uses each widget of a panel through its actions: it unchecks a check box and gives a slider a
value, which the example tells in its bar, opens a folded section, moves the handle between two
panels, scrolls the list of entities and chooses a tab, then uses the rich widgets, the menus, the
floating panel, the notifications, the palette of commands and the color well, as its module's
documentation lists; it needs a compositor alone, and runs beside the others.
`tests/edit.rs` starts the `editor` example on a copy of the yard, writes it back unchanged, makes
steps, undoes and redoes them, writes the level, finds again, as the editor starts again, what it
did not write, and, the level written, opens it from its file, where it moves three lamps in one
step, undoes it, and drops a lamp on the ground by the operation's name; it needs no compositor, the
examples' assets cooked. `tests/edit_zone.rs` starts `hunnawood-editor` on a copy
of *Hunna Wood*'s zone, changes one of the game's components, creates an entity that holds
another, undoes, redoes and writes the zone; it needs the zone cooked, and `scripts/check.sh` runs
it with the tests that read the zone.

## Dependencies

- `rmcp`: the official Rust implementation of MCP, its server and its transport on the standard
  input and output; `schemars`, which describes the tools' parameters, comes through it.
- `tokio`: the runtime that `rmcp` runs on, one thread; the requests to the game wait on threads of
  its own.
- `serde`: reads the tools' parameters.
- `base64`: encodes the captures, as MCP carries images.
- `png`: writes the captures.
- `slime-channel`: starts a game with a token and speaks its development channel.

## Must not

- Be linked by a game or ship with one: it is a program apart.
- Open a port, or put a game's token anywhere but in the game's environment.
- Read or change a game but through its development channel.
