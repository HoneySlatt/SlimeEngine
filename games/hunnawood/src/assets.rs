//! Where the game finds its cooked data, and the IDs of what it loads from it: the zone and the
//! hero, which `games/hunnawood/zone/build_zone.py` exports and `slime-cook` cooks.

use std::path::PathBuf;
use std::str::FromStr;

use slime_assets::{AssetId, MANIFEST_FILE};

/// How to make the cooked data that the game reads, for its messages.
pub const HOW_TO_COOK: &str = "from the repository's root, fetch the zone with \
     `scripts/fetch-assets.sh zone/`, then cook it with `cargo run --release -p slime-cook -- cook \
     games/hunnawood/assets/zone target/cooked/hunnawood`";

/// The ID that a metadata file of the zone's export holds: the ID of its file's default scene.
fn id(metadata: &str) -> AssetId {
    let text = metadata
        .trim()
        .trim_start_matches("id = \"")
        .trim_end_matches('"');
    AssetId::from_str(text).expect("the zone's metadata files hold an ID each")
}

/// The zone's scene.
pub fn zone_id() -> AssetId {
    id(include_str!("../assets/zone/hunna_wood.gltf.meta"))
}

/// The hero's scene.
pub fn hero_id() -> AssetId {
    id(include_str!("../assets/zone/hero.gltf.meta"))
}

/// `target/cooked/hunnawood` in the repository, where [`HOW_TO_COOK`] cooks the zone.
pub fn cooked_in_target() -> PathBuf {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/cooked/hunnawood").into()
}

/// The directory of cooked data that the game reads unless told another: `hunnawood` beside the
/// program when it holds a manifest, as where CI ships the game, and [`cooked_in_target`]
/// otherwise.
pub fn default_directory() -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|program| Some(program.parent()?.join("hunnawood")));
    match beside {
        Some(directory) if directory.join(MANIFEST_FILE).is_file() => directory,
        _ => cooked_in_target(),
    }
}
