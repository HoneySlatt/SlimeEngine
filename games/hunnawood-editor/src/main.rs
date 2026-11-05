//! *Hunna Wood*'s editor: the engine's editor, which knows the game's types
//! (`docs/adr/0110-how-the-editor-runs-a-games-code.md`), in a window of its own, or without one
//! with `--headless` (`docs/adr/0135-the-editors-program.md`,
//! `docs/adr/0156-the-editors-window-and-its-operations.md`), on the game's zone, as the library's
//! `program` describes it. A person uses it, and a tool, an agent through the engine's MCP server,
//! drives it. `README.md` beside it says how to run it.

use std::process::ExitCode;

fn main() -> ExitCode {
    slime_editor::run(&hunnawood_editor::program())
}
