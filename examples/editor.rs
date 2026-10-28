//! The editor's program on the examples' yard (`docs/adr/0134-the-editors-core.md`,
//! `docs/adr/0135-the-editors-program.md`, `docs/adr/0156-the-editors-window-and-its-operations.md`):
//! it opens the yard's scene file, `examples/assets/yard.scene`, the prefabs that it places read
//! from the examples' cooked data, which it cooks first if it is missing, under the key that the
//! `world` example spawns it under, `yard`, in a window of its own: a menu bar, the world panel, a
//! status bar. A person runs its operations from the menus, the shortcuts and the palette
//! (Ctrl+Shift+P); an agent runs the same through the engine's MCP server
//! (`crates/slime-mcp/README.md`), and makes steps of commands, undoes and redoes them, selects
//! entities and writes the yard back. With `--headless`, it runs without a window, for agents alone.
//! It knows the engine's types only, as a game's editor package knows its game's
//! (`docs/adr/0110-how-the-editor-runs-a-games-code.md`).
//!
//! ```sh
//! cargo run --example editor -- --level /tmp/yard/yard.scene --journal /tmp/yard.journal
//! cargo run --example editor -- --journal /tmp/yard.journal --output /tmp/editor.png --scale 2
//! ```
//!
//! It writes the level that it edits: give it a copy of the yard, with its metadata file, unless
//! the change is meant for the repository. Its journal, in the user's data directory unless
//! `--journal` names another file, keeps what it changed until it writes the level; started again
//! after a crash or a quit, it recovers it, unless `--discard` starts from the file
//! (`slime_editor::run`). `--output <file.png>` draws the window into an image instead, without a
//! window of the system, at `--scale <factor>`, 1 unless given (`slime_editor::window_image`).

#[path = "common/png_file.rs"]
mod png_file;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

fn program() -> slime_editor::Program {
    slime_editor::Program {
        name: "editor",
        level: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/yard.scene")),
        key: "yard",
        assets: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/cooked/examples"
        )),
        sources: Some(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets"
        ))),
        register: |_| {},
    }
}

fn main() -> ExitCode {
    let mut arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some(at) = arguments.iter().position(|argument| argument == "--output") else {
        return slime_editor::run(&program());
    };
    let Some(output) = arguments.get(at + 1).cloned() else {
        eprintln!("--output needs a path");
        return ExitCode::FAILURE;
    };
    arguments.drain(at..=at + 1);
    let mut scale = 1.0;
    if let Some(at) = arguments.iter().position(|argument| argument == "--scale") {
        let given = arguments
            .get(at + 1)
            .and_then(|scale| scale.to_str()?.parse::<f32>().ok());
        match given {
            Some(given) if given.is_finite() && given > 0.0 => scale = given,
            _ => {
                eprintln!("--scale needs a factor above 0");
                return ExitCode::FAILURE;
            }
        }
        arguments.drain(at..=at + 1);
    }
    slime_app::init_diagnostics();
    let drawn = slime_editor::window_image(&program(), arguments, scale).and_then(|image| {
        png_file::write(output.as_ref(), &image).map_err(|error| error.to_string())
    });
    match drawn {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("the editor's window cannot be drawn: {error}");
            ExitCode::FAILURE
        }
    }
}
