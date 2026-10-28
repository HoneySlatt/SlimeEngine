//! *Hunna Wood*'s zone opened in the editor, with the game's types, and written back: its comments,
//! written by hand, stay above what they describe, and the zone written opens again into the same
//! world (`docs/adr/0134-the-editors-core.md`); moved by gestures and steps, copied and pasted,
//! every step undone, it is written back as it was
//! (`docs/adr/0161-the-open-step-and-the-handles.md`,
//! `docs/adr/0162-creating-copying-and-arranging.md`). It needs the zone cooked, as
//! `scripts/check.sh` cooks it into `target/cooked/hunnawood`.

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

/// The zone moved and scaled by gestures, as the handles give them, three of its placements in
/// one step, those three copied and pasted twice, then every step undone: written back, its file
/// is what the zone written unchanged is, byte for byte
/// (`docs/adr/0161-the-open-step-and-the-handles.md`,
/// `docs/adr/0162-creating-copying-and-arranging.md`).
#[test]
#[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
fn the_zone_moved_then_every_step_undone_is_written_back_unchanged() {
    use slime_reflect::{Path as ValuePath, Reflect};
    use slime_world::{Command, Transform};

    let directory =
        std::env::temp_dir().join(format!("hunnawood-editor-undone-{}", std::process::id()));
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
    // The zone as the editor writes it unchanged, which its writer lays out its own way.
    editor.write(&mut world).expect("written");
    let unchanged = fs::read_to_string(&file).expect("written");
    // Three placements of the zone, by the order of their identities.
    let root = editor.root(&world).expect("the zone's root");
    let mut placed: Vec<_> = world
        .children(root)
        .filter(|&child| {
            world.placement(child).is_some() && world.get::<Transform>(child).is_some()
        })
        .filter_map(|child| world.persistent_id(child))
        .collect();
    placed.sort_by_key(ToString::to_string);
    assert!(placed.len() >= 3, "{placed:?}");
    let moved = |world: &World, step: f64| -> Vec<Command> {
        placed[..3]
            .iter()
            .map(|id| {
                let entity = world.find(id).expect("alive");
                let mut transform = *world.get::<Transform>(entity).expect("placed");
                transform.translation.x += step;
                transform.scale *= 1.5;
                Command::SetProperty {
                    entity: id.clone(),
                    component: "Transform".to_owned(),
                    path: ValuePath::default(),
                    value: transform.to_value(),
                }
            })
            .collect()
    };
    // A gesture over a few frames, each from where the drag started and in place of the last,
    // made one step.
    let frames = [0.5, 1.0, 2.0].map(|step| moved(&world, step));
    for commands in frames {
        editor.gesture(&mut world, commands).expect("live");
    }
    editor.end_gesture(&mut world, "Move").expect("one step");
    // Another cancelled, and a step of an agent's.
    let commands = moved(&world, -3.0);
    editor.gesture(&mut world, commands).expect("live");
    editor.cancel_gesture(&mut world);
    let commands = moved(&world, 1.0);
    editor
        .apply(&mut world, "Move again", commands, None)
        .expect("a step");
    // Three placements copied, pasted twice, each paste one step under new identities
    // (`docs/adr/0162-creating-copying-and-arranging.md`).
    let copied = editor.copy(&world, &placed[..3]).expect("copied");
    for _ in 0..2 {
        let pasted = editor.paste(&mut world, "Paste", &copied).expect("pasted");
        assert_eq!(pasted.len(), 3);
        assert!(pasted.iter().all(|id| world.find(id).is_some()));
    }
    assert_eq!(
        editor.done().collect::<Vec<_>>(),
        ["Move", "Move again", "Paste", "Paste"]
    );
    while editor.undo(&mut world).expect("undone") {}
    let left_out = editor.write(&mut world).expect("written");
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(fs::read_to_string(&file).expect("written"), unchanged);
    fs::remove_dir_all(&directory).expect("removed");
}
