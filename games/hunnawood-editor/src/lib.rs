//! *Hunna Wood*'s editor, as a library: the description of the editor's program on the game's
//! zone, which the program `hunnawood-editor` runs, and which the engine's benchmarks open to
//! measure the editor's interface on the zone (`docs/adr/0186-counters-and-ratios-in-ci.md`).

use std::path::PathBuf;

/// The editor's program on the game's zone, `assets/zone/zone.scene` of the game's package,
/// under the key that the game spawns it under, `zone`, the land and the lantern that it places
/// read from the game's cooked data, which takes minutes to cook and which the user cooks first;
/// the game's types registered, the zone lit as the game lights it by day, and the game played
/// and built as Play asks.
pub fn program() -> slime_editor::Program {
    slime_editor::Program {
        name: "hunnawood-editor",
        level: PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../hunnawood/assets/zone/zone.scene"
        )),
        key: "zone",
        assets: hunnawood::assets::default_directory(),
        sources: None,
        register: hunnawood::rules::register,
        // Lit as the game lights the zone by day.
        lighting: slime_editor::Lighting {
            sky: Some(hunnawood::OVERCAST),
            exposure: hunnawood::DAY_EXPOSURE,
            bloom: hunnawood::BLOOM,
        },
        // The game, which Play builds, on a new run saved to Play's own save, or resuming that
        // save once its code changed; it has no run without a window.
        play: Some(|played| {
            let mut command = std::process::Command::new(played.program("hunnawood"));
            command.arg("--assets").arg(played.assets);
            if !played.resume {
                command.arg("--new");
            }
            command.arg("--save").arg(played.save);
            command
        }),
        build: Some(slime_editor::Build {
            directory: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            arguments: &["--package", "hunnawood", "--bin", "hunnawood"],
        }),
        layout: None,
    }
}
