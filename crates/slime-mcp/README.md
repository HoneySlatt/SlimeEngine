# slime-mcp

The engine's MCP server: through it, an AI agent drives a game built with the engine as a person
would, without a person (`docs/adr/0099-ai-agents-drive-the-engine.md`,
`docs/adr/0122-the-development-channel.md`). The Model Context Protocol (MCP) is the open protocol
through which coding agents, Claude Code and Codex among them, call the tools of other programs.

## Status

One game at a time, driven through its development channel (`slime-channel`): started, paused,
stepped, its speed set, its world read and changed, its keys put down and let go, its image
captured, its log and its frames' times read, its run saved and loaded, stopped. Assets cooked and
tests run. No mouse in scripted input yet, and no editor: the editor's commands join the server with
the editor, at Stage 6b.

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
  to it, waiting long enough for `cargo run` to build it first. Give a game a save of its own:
  `["cargo", "run", "-p", "hunnawood", "--", "--new", "--save", "/tmp/agent-run.toml"]`.
  `stop_game` asks it to stop, as closing its window would, and kills it if it does not. A game
  that ended on its own is told at the next tool that asks it, and forgotten.
- `game_state`, `pause`, `resume`, `step` and `set_speed`: the host's controls of game time; a step
  answers once its steps have run.
- `list_entities`, `inspect_entity` and `set_property`: the world, its entities named by their
  persistent identities, `hero`, `zone/door`, its values written in the engine's notation, as a
  scene file holds them.
- `press_key` and `release_key`: keys, named by their position on the keyboard, `KeyW`.
- `capture`: the image that the game shows, as a PNG image.
- `read_logs`: the lines of the game's log after a given line.
- `save_game` and `load_game`: the game's run saved to a file, and loaded from one in place of the
  run, as the game saves and resumes its own runs; a file of the agent's, never the player's save.
- `cook`: cooks a directory of source assets, which a game started with `--watch` loads again.
- `run_tests`: runs `cargo test` with the arguments given.

`tests/drive.rs` plays an agent's session: it starts *Hunna Wood* through the server, pauses it,
reads and changes the hero, walks it, saves the run, walks on and loads it back, captures its image
and reads its log. It needs a Wayland
compositor and the game's zone cooked: `scripts/check.sh` runs it in a headless Weston.

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
