//! Where the examples find the sample's cooked data.

use std::path::PathBuf;

/// The directory of cooked data that an example reads without `--assets`: `assets` beside the
/// program, where CI ships the Windows programs with the sample cooked, or else
/// `target/cooked/examples` in the workspace, where
/// `cargo run -p slime-cook -- cook examples/assets target/cooked/examples` cooks it.
pub fn find() -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|program| Some(program.parent()?.join("assets")));
    match beside {
        Some(directory) if directory.join(slime_assets::MANIFEST_FILE).is_file() => directory,
        _ => PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/cooked/examples"
        )),
    }
}
