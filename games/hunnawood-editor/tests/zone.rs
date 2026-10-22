//! *Hunna Wood*'s zone opened in the editor, with the game's types, and written back: its comments,
//! written by hand, stay above what they describe, and the zone written opens again into the same
//! world (`docs/adr/0134-the-editors-core.md`). It needs the zone cooked, as `scripts/check.sh`
//! cooks it into `target/cooked/hunnawood`.

use std::fs;
use std::path::{Path, PathBuf};

use slime_assets::CookedAssets;
use slime_editor::Editor;
use slime_world::{SceneLibrary, World};

/// The zone's scene file, in the game's sources.
fn zone() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../hunnawood/assets/zone/zone.scene")
}

/// The zone opened from `file` into a world of the game's types.
fn open(file: &Path) -> (World, Editor) {
    let assets =
        CookedAssets::open(hunnawood::assets::cooked_in_target()).expect("the zone is cooked");
    let mut world = World::new();
    hunnawood::rules::register(&mut world);
    let editor =
        Editor::open(&mut world, SceneLibrary::of(assets), file, "zone").expect("the zone opens");
    (world, editor)
}

/// Each block of comment lines of `text`, in its order, with the path of what follows it: the
/// table, or the key, that it describes, `changed."land/moon".Light.Directional` whether the key
/// is written in the table's header or below it.
fn comments(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let (mut table, mut block) = (String::new(), Vec::new());
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with('#') {
            block.push(line);
            continue;
        }
        let path = if let Some(header) = line.strip_prefix('[') {
            table = header.trim_end_matches(']').to_owned();
            table.clone()
        } else {
            let key = line.split(" = ").next().expect("a key");
            format!("{table}.{key}")
        };
        if !block.is_empty() {
            found.push((block.join("\n"), path));
            block.clear();
        }
    }
    found
}

#[test]
#[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
fn the_zone_written_back_keeps_its_comments_and_opens_again_the_same() {
    let directory =
        std::env::temp_dir().join(format!("hunnawood-editor-zone-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a directory of the test's own");
    let file = directory.join("zone.scene");
    let original = fs::read_to_string(zone()).expect("the zone's file");
    fs::write(&file, &original).expect("copied");
    fs::copy(
        zone().with_extension("scene.meta"),
        directory.join("zone.scene.meta"),
    )
    .expect("its metadata copied");

    let (mut world, mut editor) = open(&file);
    let left_out = editor.write(&mut world).expect("written");
    assert!(left_out.is_empty(), "{left_out:?}");
    let written = fs::read_to_string(&file).expect("written");
    let kept = comments(&original);
    assert_eq!(kept.len(), 6, "the zone's comments: {kept:?}");
    assert_eq!(comments(&written), kept, "{written}");
    let (mut again, editor_again) = open(&file);
    let root = |world: &World, editor: &Editor| editor.root(world).expect("the zone's root");
    let (before, after) = (root(&world, &editor), root(&again, &editor_again));
    assert_eq!(
        again.write_scene(after).expect("written").text,
        world.write_scene(before).expect("written").text
    );
    fs::remove_dir_all(&directory).expect("removed");
}
