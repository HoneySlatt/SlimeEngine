# hunnawood-editor

*Hunna Wood*'s editor: the engine's editor (`slime-editor`), which knows the game's types through
its library ([ADR-0110](../../docs/adr/0110-how-the-editor-runs-a-games-code.md)). Today the
editor's program without a window, which opens the game's zone and waits for a tool, an agent
through the engine's MCP server, to change it; its window comes with Stage 6c
([ADR-0135](../../docs/adr/0135-the-editors-program.md)).

## Running it

It opens [`../hunnawood/assets/zone/zone.scene`](../hunnawood/assets/zone/zone.scene) under the key
that the game spawns it under, `zone`, the land and the lantern that the zone places read from the
game's cooked data, which the game's README says how to fetch and cook. It writes the zone back to
that file when a tool asks: give it a copy, with its metadata file, unless the change is meant for
the repository.

```sh
cargo run -p hunnawood-editor -- --level /tmp/zone/zone.scene --journal /tmp/zone.journal
```

The options are the editor's program's (`slime_editor::run`): `--level`, `--assets`, `--journal`
and `--discard`. Its journal, `slime-editor/<the zone's asset ID>.journal` in the user's data
directory, or one of its own for a copy of the zone, unless `--journal` names another file, keeps what it did not write, which it recovers as
it starts again. Ctrl+C stops it. An agent starts it through the MCP server's `start_game`, as it
starts the game, and drives it with the server's editor's tools (`crates/slime-mcp/README.md`).

`tests/zone.rs` opens the zone, writes it back, checks that every comment of the file stays above
what it describes, and opens it again; it needs the zone cooked, and `scripts/check.sh` runs it with
the game's tests that read the zone (`cargo test -p hunnawood-editor -- --ignored`), as it runs
the MCP server's session on a copy of the zone (`crates/slime-mcp/README.md`).

## Dependencies

- `hunnawood`: the game's library, its types (`rules::register`) and where its cooked data is.
- `slime-editor`: the engine's editor, its program.

## Must not

- Be a dependency of the game: a shipped game holds nothing of its editor.
- Reach a type of the game but through the registry: the editor's code serves every game.
