# hunnawood-editor

*Hunna Wood*'s editor: the engine's editor (`slime-editor`), which knows the game's types through
its library ([ADR-0110](../../docs/adr/0110-how-the-editor-runs-a-games-code.md)): the editor's
program, which opens the game's zone in a window of its own, its menus, the world panel, a viewport
and a status bar, where a person changes it and a tool, an agent through the engine's MCP server,
drives it alike ([ADR-0135](../../docs/adr/0135-the-editors-program.md),
[ADR-0156](../../docs/adr/0156-the-editors-window-and-its-operations.md)). Its viewport shows the
zone under the game's overcast sky, exposure and bloom by day, which the game's library gives it
([ADR-0158](../../docs/adr/0158-the-viewport.md)); every light that the zone places shines there,
the night's too, which the game's rules hide by day, and the moon, the one light that shows in the
zone's sky, is the sky's sun ([`docs/PLAN.md`](../../docs/PLAN.md), known debt). A click there
selects what the zone places whole, a lantern or the land, which holds every tree, rock and fern,
a click again a level deeper towards what is under the pointer, and what is selected is outlined
([ADR-0159](../../docs/adr/0159-picking-and-the-selection.md)).

## Running it

It opens [`../hunnawood/assets/zone/zone.scene`](../hunnawood/assets/zone/zone.scene) under the key
that the game spawns it under, `zone`, the land and the lantern that the zone places read from the
game's cooked data, which the game's README says how to fetch and cook. It writes the zone back to
that file when a tool asks: give it a copy, with its metadata file, unless the change is meant for
the repository.

```sh
cargo run -p hunnawood-editor -- --level /tmp/zone/zone.scene --journal /tmp/zone.journal
```

The options are the editor's program's (`slime_editor::run`): `--level`, `--assets`, `--journal`,
`--discard` and `--headless`, which runs it without a window. Its journal,
`slime-editor/<the zone's asset ID>.journal` in the user's data directory, or one of its own for a
copy of the zone, unless `--journal` names another file, keeps what it did not write, which it
recovers as it starts again. Closing the window, Ctrl+Q or Ctrl+C quits at once. An agent starts it
through the MCP server's `start_game`, as it starts the game, and drives it with the server's
editor's tools and its operations, run by name (`crates/slime-mcp/README.md`). It does not cook
the zone when its cooked data is missing, which takes minutes: the game's README says how.

`tests/zone.rs` opens the zone, writes it back, checks that every comment of the file stays above
what it describes, and opens it again; then moves and scales three of its placements, as the
handles do, frame by frame through a gesture made one step, cancels another, makes a step as an
agent does, copies the three and pastes them twice, undoes every step, and finds the zone written
back as it was written first; it needs the zone cooked, and `scripts/check.sh` runs it with the
game's tests that read the zone (`cargo test -p hunnawood-editor -- --ignored`), as it runs the MCP
server's session on a copy of the zone (`crates/slime-mcp/README.md`).

## Dependencies

- `hunnawood`: the game's library, its types (`rules::register`) and where its cooked data is.
- `slime-editor`: the engine's editor, its program.

## Must not

- Be a dependency of the game: a shipped game holds nothing of its editor.
- Reach a type of the game but through the registry: the editor's code serves every game.
