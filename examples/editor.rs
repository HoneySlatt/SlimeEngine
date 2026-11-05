//! The editor's program on the examples' yard (`docs/adr/0134-the-editors-core.md`,
//! `docs/adr/0135-the-editors-program.md`,
//! `docs/adr/0156-the-editors-window-and-its-operations.md`): it opens the yard's scene file,
//! `examples/assets/yard.scene`, under the key that the `world` example spawns it under, `yard`,
//! the prefabs that it places read from the examples' cooked data, which it cooks first from
//! `examples/assets` if it is missing. It knows the engine's types only, as a game's editor package
//! knows its game's (`docs/adr/0110-how-the-editor-runs-a-games-code.md`).
//!
//! Its window, in the editor's look, holds a menu bar, a status bar, which names the level and says
//! how many steps its file does not hold, as the window's title does, and panels in regions of tabs
//! (`docs/adr/0172-regions-of-tabs.md`): by default, the hierarchy above the components on the
//! left, and the viewport above the history and the console, tabbed.
//!
//! - The hierarchy shows the yard's entities as a tree, which folds, selects, searches names,
//!   `c:` components and `p:` prefabs, renames in place, rearranges by drags, and hides and locks
//!   each entity by the eye and the lock of its row (`docs/adr/0167-the-hierarchy.md`,
//!   `docs/adr/0168-rearranging-the-hierarchy.md`, `docs/adr/0169-the-eye-and-the-lock.md`).
//! - The components, the dev overlay's world panel, change what is selected, each change a step.
//! - The viewport shows the yard lit as the sample is, a grid on the ground and the icons of its
//!   lights, drawn again only when what it shows changes. The right button held flies through it
//!   with W, A, S, D, Q and E, at a speed that the wheel changes and the user's state keeps per
//!   level; Alt and the left button orbit, the middle button pans
//!   (`docs/adr/0158-the-viewport.md`). A click selects what is drawn there, a drag a marquee's,
//!   and what is selected is outlined (`docs/adr/0159-picking-and-the-selection.md`), with handles
//!   that move, turn and scale it, each drag one step that Escape cancels
//!   (`docs/adr/0161-the-open-step-and-the-handles.md`).
//! - The history lists the steps, a press on one going there
//!   (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`); the console shows the log of the
//!   game that Play runs.
//!
//! A tab's context menu moves its panel to another region or an edge of the window, floats it over
//! the window or closes it; a panel closed keeps its place, to open there again. The layout is kept
//! in the user's state directory for the next start
//! (`docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`).
//!
//! Its menus: *File* opens a level, through the system's dialog behind a modal layer where its path
//! can be typed too (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`), lists the
//! levels opened last, kept in the user's state directory, saves and quits. *Edit* undoes, redoes
//! and repeats; creates, places a prefab, duplicates, copies and pastes through the system's
//! clipboard, deletes, renames and groups (`docs/adr/0162-creating-copying-and-arranging.md`);
//! chooses the handles and their axes and snaps; drops what is selected on what lies below it.
//! *View* frames the selection, chooses how the viewport shades and whether it shows the
//! selection's wireframe, shows two viewports, opens the panels, resets the layout, and chooses the
//! keys of most editors or Blender's (`docs/adr/0160-two-viewports-and-blenders-way.md`). *Play*
//! plays, pauses, steps and stops the yard, as does a toolbar above the viewport, with the speeds
//! of game time: Play builds the `world` example with cargo as it starts it, and again as its code
//! changes, the game then starting again (`docs/adr/0166-building-the-game-again.md`), and runs it
//! on the yard as the editor holds it, steps not written included (`docs/adr/0164-play.md`), the
//! steps made while it runs reaching it (`docs/adr/0165-editing-while-playing.md`). *Help* opens
//! the palette of operations (Ctrl+Shift+P) and the sheet of shortcuts (F1);
//! `crates/slime-editor/README.md` lists the shortcuts too, and `crates/slime-editor/editor.keys`
//! binds them.
//!
//! A person runs its operations from the menus, the shortcuts and the palette; an agent runs the
//! same, by name, through the engine's MCP server (`crates/slime-mcp/README.md`), and makes steps
//! of commands, undoes and redoes them, selects entities and writes the yard back, with its
//! comments (`slime_editor::run`). Its frames run only when something asks for one. The first run
//! tells one hint.
//!
//! ```sh
//! cargo run --example editor -- --level /tmp/yard/yard.scene --journal /tmp/yard.journal
//! cargo run --example editor -- --journal /tmp/yard.journal --output /tmp/editor.png --scale 2
//! ```
//!
//! `--level <file>` names the level to edit, the repository's yard by default. The editor writes
//! the level that it edits: give it a copy of the yard, with its metadata file, unless the change
//! is meant for the repository. `--assets <directory>` names the cooked data, by default
//! `target/cooked/examples` in the workspace, as the sources it edits are. Its journal, in the
//! user's data directory unless `--journal <file>` names another file, keeps what it changed until
//! it writes the level; started again after a crash or a quit, it recovers it, unless `--discard`
//! starts from the level's file, the journal set aside (`slime_editor::run`). `--headless` runs it
//! without a window, for agents alone, until Ctrl+C or a tool stops it. `--output <file.png>` draws
//! the window into an image of 1280 × 800 logical pixels instead, as it shows once the yard is
//! open, without a window of the system, at `--scale <factor>` pixels per logical pixel, above 0, 1
//! unless given (`slime_editor::window_image`). Closing the window, Ctrl+Q or Ctrl+C quits at once:
//! the journal keeps what the level's file does not hold.

#[path = "common/png_file.rs"]
mod png_file;
#[path = "common/sample_scene.rs"]
#[expect(
    dead_code,
    reason = "the editor lights the yard with the sample's sky, exposure and bloom alone"
)]
mod sample_scene;

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
        lighting: slime_editor::Lighting {
            sky: Some(sample_scene::SKY),
            exposure: sample_scene::EXPOSURE,
            bloom: sample_scene::BLOOM,
        },
        // The `world` example, which Play builds, and which plays the yard; without a window when
        // the editor runs without one. It does not save its runs.
        play: Some(|played| {
            let mut command = std::process::Command::new(played.program("world"));
            command.arg("--assets").arg(played.assets);
            if played.headless {
                command.arg("--headless");
            }
            command
        }),
        build: Some(slime_editor::Build {
            directory: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            arguments: &["--package", "slime-examples", "--example", "world"],
        }),
        layout: None,
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
    let drawn =
        slime_editor::window_image(&program(), arguments, &[], &[], scale).and_then(|image| {
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
