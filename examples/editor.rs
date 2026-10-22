//! The editor's program on the examples' yard, without a window
//! (`docs/adr/0134-the-editors-core.md`, `docs/adr/0135-the-editors-program.md`): it opens the
//! yard's scene file, `examples/assets/yard.scene`, the prefabs that it places read from the
//! examples' cooked data, under the key that the `world` example spawns it under, `yard`, and waits
//! for a tool to drive it through the development channel: an agent makes steps of commands, undoes
//! and redoes them, selects entities and writes the yard back, through the engine's MCP server
//! (`crates/slime-mcp/README.md`). It knows the engine's types only, as a game's editor package
//! knows its game's (`docs/adr/0110-how-the-editor-runs-a-games-code.md`).
//!
//! ```sh
//! cargo run -p slime-cook -- cook examples/assets target/cooked/examples
//! cargo run --example editor -- --level /tmp/yard/yard.scene --journal /tmp/yard.journal
//! ```
//!
//! It writes the level that it edits: give it a copy of the yard, with its metadata file, unless
//! the change is meant for the repository. Its journal, in the user's data directory unless
//! `--journal` names another file, keeps what it changed until it writes the level; started again
//! after a crash or a stop, it recovers it, unless `--discard` starts from the file
//! (`slime_editor::run`). Ctrl+C stops it.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    slime_editor::run(&slime_editor::Program {
        name: "editor",
        level: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/yard.scene")),
        key: "yard",
        assets: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/cooked/examples"
        )),
        register: |_| {},
    })
}
