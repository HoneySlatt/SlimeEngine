//! Cooked data for the tests: a directory of their own for each, and the examples' sources cooked
//! once for the tests of one program.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use slime_assets::CookedAssets;

/// A directory of its own for `name`, emptied, under Cargo's directory for the temporary files of
/// integration tests, apart from the other test programs'.
pub fn directory(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(concat!("slime-examples-", env!("CARGO_CRATE_NAME")))
        .join(name);
    if path.exists() {
        fs::remove_dir_all(&path).expect("the test's old directory can be removed");
    }
    fs::create_dir_all(&path).expect("the test's directory can be created");
    path
}

/// The examples' sources (`examples/assets`) cooked, once for the tests of this program.
pub fn sample() -> &'static CookedAssets {
    static SAMPLE: OnceLock<CookedAssets> = OnceLock::new();
    SAMPLE.get_or_init(|| {
        let cooked = directory("sample");
        slime_cook::cook(
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")),
            &cooked,
        )
        .expect("the sample cooks");
        CookedAssets::open(&cooked).expect("the sample's manifest is read")
    })
}
