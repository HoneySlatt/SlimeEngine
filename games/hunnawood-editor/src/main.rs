//! *Hunna Wood*'s editor: the engine's editor, which knows the game's types
//! (`docs/adr/0110-how-the-editor-runs-a-games-code.md`), in a window of its own, or without one
//! with `--headless` (`docs/adr/0135-the-editors-program.md`,
//! `docs/adr/0156-the-editors-window-and-its-operations.md`). It opens the game's zone,
//! `assets/zone/zone.scene` of the game's package, under the key that the game spawns it under,
//! `zone`, the land and the lantern that it places read from the game's cooked data, which takes
//! minutes to cook and which the user cooks first. A person uses it, and a tool, an agent through
//! the engine's MCP server, drives it. `README.md` beside it says how to run it.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    slime_editor::run(&slime_editor::Program {
        name: "hunnawood-editor",
        level: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../hunnawood/assets/zone/zone.scene"
        )),
        key: "zone",
        assets: hunnawood::assets::default_directory(),
        sources: None,
        register: hunnawood::rules::register,
    })
}
