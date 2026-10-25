//! Cooking directories of source images into directories of cooked data, in temporary
//! directories: the same bytes from the same sources, nothing cooked again that has not changed,
//! not even the manifest, but what was damaged, and textures that a game loads by the image's ID;
//! a watch that cooks what changes, and two cooks at once, in two processes.

use std::collections::BTreeMap;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

use slime_assets::{AssetId, AssetKind, CookedAssets, MANIFEST_FILE, TextureEncoding};
use slime_cook::{CookError, cook, scan};
use slime_world::{PersistentId, SceneError, SceneLibrary, World};

/// An empty directory of its own for `name`, under Cargo's directory for temporary files of
/// integration tests.
fn directory(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slime-cook-cook")
        .join(name);
    if path.exists() {
        fs::remove_dir_all(&path).expect("the test's old directory can be removed");
    }
    fs::create_dir_all(&path).expect("the test's directory can be created");
    path
}

/// Writes a PNG image of `width` × `height` opaque texels at `path`, its colors made from `seed`.
fn png(path: &Path, width: u32, height: u32, seed: u8) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the directory can be created");
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgb);
    let mut writer = encoder.write_header().expect("the PNG header is written");
    let texels: Vec<u8> = (0..width * height)
        .flat_map(|i| [(i as u8).wrapping_mul(seed), seed, (i / width) as u8 * 9])
        .collect();
    writer
        .write_image_data(&texels)
        .expect("the PNG image is written");
    writer.finish().expect("the PNG image is finished");
    fs::write(path, bytes).expect("the image is written");
}

/// Every file of `directory` and its bytes, by name.
fn files(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .expect("the directory is read")
        .map(|entry| {
            let entry = entry.expect("the entry is read");
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("the file is read"),
            )
        })
        .collect()
}

/// A directory of sources: two images, one in a subdirectory and of sides that are not multiples
/// of 4.
fn sources(name: &str) -> PathBuf {
    let root = directory(name).join("sources");
    png(&root.join("rock.png"), 16, 8, 3);
    png(&root.join("moss/leaf.png"), 6, 5, 7);
    root
}

#[test]
fn the_same_sources_cook_into_the_same_bytes() {
    let root = sources("same");
    let first = root.with_file_name("first");
    let second = root.with_file_name("second");
    let cooked = cook(&root, &first).expect("the first cook succeeds");
    assert_eq!(cooked.cooked.len(), 2);
    cook(&root, &second).expect("the second cook succeeds");
    let (first, second) = (files(&first), files(&second));
    assert_eq!(
        first.len(),
        3,
        "two textures and the manifest: {:?}",
        first.keys()
    );
    assert_eq!(first, second);
}

#[test]
fn a_second_cook_cooks_nothing_and_a_changed_image_alone_is_cooked_again() {
    let root = sources("again");
    let output = root.with_file_name("cooked");
    cook(&root, &output).expect("the first cook succeeds");
    let again = cook(&root, &output).expect("the second cook succeeds");
    assert!(again.cooked.is_empty(), "{again:?}");
    assert_eq!(again.reused.len(), 2);

    let ids = scan(&root, slime_cook::Mode::Check).expect("scanned");
    let id = |name: &str| {
        ids.sources
            .iter()
            .find(|source| source.path.ends_with(name))
            .expect("the source is there")
            .id
    };
    png(&root.join("rock.png"), 16, 8, 4);
    let changed = cook(&root, &output).expect("the third cook succeeds");
    assert_eq!(changed.cooked, [id("rock.png")]);
    assert_eq!(changed.reused, [id("leaf.png")]);
    // The manifest names the new texture, and the game reads what changed.
    let assets = CookedAssets::open(&output).expect("the manifest is read");
    let rock = assets.texture(id("rock.png")).expect("the texture is read");
    assert_eq!((rock.width, rock.height), (16, 8));
}

#[test]
fn an_image_moved_with_its_metadata_is_not_cooked_again() {
    let root = sources("moved");
    let output = root.with_file_name("cooked");
    cook(&root, &output).expect("the first cook succeeds");
    fs::create_dir_all(root.join("stones")).expect("created");
    fs::rename(root.join("rock.png"), root.join("stones/rock.png")).expect("moved");
    fs::rename(
        root.join("rock.png.meta"),
        root.join("stones/rock.png.meta"),
    )
    .expect("moved");
    let moved = cook(&root, &output).expect("the cook succeeds");
    assert!(moved.cooked.is_empty(), "{moved:?}");
    let assets = CookedAssets::open(&output).expect("the manifest is read");
    let (_, entry) = assets
        .manifest()
        .iter()
        .find(|(_, entry)| entry.source.ends_with("rock.png"))
        .expect("listed");
    assert_eq!(entry.source, "stones/rock.png");
}

#[test]
fn a_game_loads_each_image_s_texture_by_the_image_s_id() {
    let root = sources("load");
    let output = root.with_file_name("cooked");
    cook(&root, &output).expect("the cook succeeds");
    let assets = CookedAssets::open(&output).expect("the manifest is read");
    assert_eq!(assets.manifest().len(), 2);
    for source in scan(&root, slime_cook::Mode::Check)
        .expect("scanned")
        .sources
    {
        let texture = assets.texture(source.id).expect("the texture is read");
        // 16 × 8 texels, and 6 × 5 stretched to 8 × 8, which blocks of 4 × 4 divide.
        assert!(matches!((texture.width, texture.height), (16, 8) | (8, 8)));
        assert_eq!(texture.encoding, TextureEncoding::Bc7Srgb);
    }
    let unknown = AssetId::from_u128(42);
    let error = assets.texture(unknown).expect_err("not cooked");
    assert!(error.to_string().contains("no asset"), "{error}");
}

#[test]
fn two_images_that_share_an_id_are_refused() {
    let root = sources("same-id");
    cook(&root, &root.with_file_name("cooked")).expect("the first cook succeeds");
    fs::copy(root.join("rock.png"), root.join("copy.png")).expect("copied");
    fs::copy(root.join("rock.png.meta"), root.join("copy.png.meta")).expect("copied");
    let error = cook(&root, &root.with_file_name("cooked")).expect_err("refused");
    assert!(matches!(error, CookError::SameId(_)), "{error:?}");
}

#[test]
fn a_damaged_image_fails_the_cook_with_its_path() {
    let root = sources("damaged");
    fs::write(root.join("broken.png"), "not an image").expect("written");
    let error = cook(&root, &root.with_file_name("cooked")).expect_err("refused");
    assert!(error.to_string().contains("broken.png"), "{error}");
}

#[test]
fn a_cook_that_changes_nothing_leaves_the_manifest_alone() {
    let root = sources("manifest");
    let output = root.with_file_name("cooked");
    cook(&root, &output).expect("the first cook succeeds");
    let manifest = output.join(MANIFEST_FILE);
    // Its time of change set long ago, which a manifest written again would not keep, however
    // coarse the file system's clock.
    let written = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(&manifest)
        .and_then(|file| file.set_modified(written))
        .expect("the manifest's time of change is set");
    cook(&root, &output).expect("the second cook succeeds");
    let after = fs::metadata(&manifest)
        .and_then(|metadata| metadata.modified())
        .expect("the manifest is there");
    assert_eq!(written, after, "the manifest was written again");
}

#[test]
fn watch_cooks_a_changed_image_and_only_what_it_makes() {
    let root = sources("watch");
    let output = root.with_file_name("cooked");
    let (sender, results) = mpsc::channel();
    let watched = thread::spawn({
        let (root, output) = (root.clone(), output.clone());
        move || {
            let mut cooks = 0;
            slime_cook::watch(&root, &output, |result| {
                sender.send(result).expect("the test hears");
                cooks += 1;
                if cooks == 2 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
        }
    });
    let wait = || {
        results
            .recv_timeout(Duration::from_secs(60))
            .expect("a cook within a minute")
            .expect("the cook succeeds")
    };
    let first = wait();
    assert_eq!(first.cooked.len(), 2, "{first:?}");
    let ids = scan(&root, slime_cook::Mode::Check).expect("scanned");
    let id = |name: &str| {
        ids.sources
            .iter()
            .find(|source| source.path.ends_with(name))
            .expect("the source is there")
            .id
    };
    // Of other sizes, so that the change shows in more than the time of change.
    png(&root.join("rock.png"), 16, 12, 4);
    let second = wait();
    assert_eq!(second.cooked, [id("rock.png")]);
    assert_eq!(second.reused, [id("leaf.png")]);
    watched
        .join()
        .expect("the watch does not panic")
        .expect("the sources are read");
}

#[test]
fn two_cooks_at_once_leave_the_cooked_data_whole() {
    let root = directory("at once").join("sources");
    // Enough images that the two cooks, which cook them in the same order, write the same file at
    // the same time.
    for index in 0..16 {
        png(
            &root.join(format!("image {index}.png")),
            28,
            28,
            2 * index + 1,
        );
    }
    // The IDs first: two cooks that gave a new source an ID each would give it two.
    scan(&root, slime_cook::Mode::GiveIds).expect("scanned");
    let output = root.with_file_name("cooked");
    let children: Vec<_> = (0..2)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_slime-cook"))
                .arg("cook")
                .arg(&root)
                .arg(&output)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("the cooker starts")
        })
        .collect();
    for child in children {
        let output = child.wait_with_output().expect("the cooker ends");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let assets = CookedAssets::open(&output).expect("the manifest is read");
    assert_eq!(assets.manifest().len(), 16);
    for (id, _) in assets.manifest().iter() {
        assets.texture(id).expect("each texture is whole");
    }
    let leftovers: Vec<String> = files(&output)
        .into_keys()
        .filter(|name| name.ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

/// Copies the examples' sources, a glTF file whose images it holds and two scene files, into a
/// directory of sources named `name`, beside an image of its own, `rock.png`.
fn examples(name: &str) -> PathBuf {
    let root = directory(name).join("sources");
    fs::create_dir_all(&root).expect("the directory can be created");
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets");
    for file in ["scene.glb", "lamp.scene", "yard.scene"] {
        for file in [file.to_owned(), format!("{file}.meta")] {
            fs::copy(examples.join(&file), root.join(&file)).expect("copied");
        }
    }
    png(&root.join("rock.png"), 16, 8, 3);
    root
}

#[test]
fn a_cooked_file_deleted_emptied_or_cut_short_is_cooked_again() {
    let root = examples("cooked-damaged");
    let output = root.with_file_name("cooked");
    cook(&root, &output).expect("cooked");
    let assets = CookedAssets::open(&output).expect("the manifest is read");
    // A file of each kind, and the image's texture.
    let mut damaged = Vec::new();
    for (index, kind) in [
        AssetKind::Mesh,
        AssetKind::SceneFile,
        AssetKind::Scene,
        AssetKind::Material,
        AssetKind::Texture,
    ]
    .into_iter()
    .enumerate()
    {
        let (id, entry) = assets
            .manifest()
            .iter()
            .find(|(_, entry)| {
                entry.kind == kind && (kind != AssetKind::Texture || entry.source == "rock.png")
            })
            .expect("one is cooked");
        let path = output.join(&entry.file);
        match index % 3 {
            0 => fs::remove_file(&path).expect("deleted"),
            1 => fs::write(&path, []).expect("emptied"),
            _ => fs::write(
                &path,
                &fs::read(&path).expect("read")[..entry.size as usize / 2],
            )
            .expect("cut short"),
        }
        damaged.push(id);
    }
    let again = cook(&root, &output).expect("cooked again");
    for id in damaged {
        assert!(
            again.cooked.contains(&id),
            "{id} is cooked again: {again:?}"
        );
    }
    let assets = CookedAssets::open(&output).expect("the manifest is read");
    for (id, entry) in assets.manifest().iter() {
        let read = match entry.kind {
            AssetKind::Texture => assets.texture(id).map(drop),
            AssetKind::Mesh => assets.mesh(id).map(drop),
            AssetKind::Material => assets.material(id).map(drop),
            AssetKind::Scene => assets.scene(id).map(drop),
            _ => assets.scene_file(id).map(drop),
        };
        read.unwrap_or_else(|error| panic!("{id}, {}: {error}", entry.source));
    }
    // A file that never ends in place of a scene file, which a read whole would never finish.
    #[cfg(unix)]
    {
        let (id, entry) = assets
            .manifest()
            .iter()
            .find(|(_, entry)| entry.kind == AssetKind::SceneFile)
            .expect("a scene file");
        let path = output.join(&entry.file);
        fs::remove_file(&path).expect("removed");
        std::os::unix::fs::symlink("/dev/zero", &path).expect("linked");
        let started = std::time::Instant::now();
        let again = cook(&root, &output).expect("cooked again");
        assert!(again.cooked.contains(&id), "{again:?}");
        // Read, it would take seconds, then the memory.
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }
}

/// The IDs that the tests give their scene files, and an image.
const LAMP: AssetId = AssetId::from_u128(0x0001);
const YARD: AssetId = AssetId::from_u128(0x0002);
const ROCK: AssetId = AssetId::from_u128(0x0003);
const ELSEWHERE: AssetId = AssetId::from_u128(0x0004);

/// Writes the source `name` of `root` and its metadata file, which gives it `id`.
fn source(root: &Path, name: &str, id: AssetId, bytes: &[u8]) {
    fs::create_dir_all(root).expect("the directory can be created");
    fs::write(root.join(name), bytes).expect("the source is written");
    fs::write(
        root.join(format!("{name}.meta")),
        slime_cook::metadata::write(id),
    )
    .expect("the metadata file is written");
}

/// A lamp, and a yard that places it, whose table gives the lamp the path `path`.
fn lamp_and_yard(root: &Path, path: &str) {
    source(
        root,
        "lamp.scene",
        LAMP,
        b"[entities.post.components]\nName = \"post\"\n",
    );
    source(
        root,
        "yard.scene",
        YARD,
        format!(
            "[assets]\nlamp = {{ id = \"{LAMP}\", path = \"{path}\" }}\n\n[entities.porch]\n\
             prefab = \"lamp\"\n"
        )
        .as_bytes(),
    );
}

#[test]
fn scene_files_are_cooked_into_their_binary_encoding_and_need_the_assets_of_their_table() {
    let root = directory("scene-files").join("sources");
    let output = root.with_file_name("cooked");
    lamp_and_yard(&root, "lamp.scene");
    let cooked = cook(&root, &output).expect("cooked");
    assert!(cooked.warnings.is_empty(), "{:?}", cooked.warnings);
    let assets = CookedAssets::open(&output).expect("cooked data");
    let entry = assets.manifest().get(YARD).expect("the yard is listed");
    assert_eq!(entry.kind, AssetKind::SceneFile);
    assert_eq!(entry.needs, [LAMP]);
    assert_eq!(entry.source, "yard.scene");
    let text = fs::read_to_string(root.join("yard.scene")).expect("the source is read");
    assert_eq!(
        assets.scene_file(YARD).expect("read"),
        slime_serialize::encode(&text).expect("TOML")
    );
    // A world spawns them from the cooked data.
    let mut world = World::new();
    let mut scenes = SceneLibrary::of(assets);
    let yard = scenes.resolve(world.registry(), YARD).expect("resolved");
    world.spawn_scene("yard", &yard).expect("spawned");
    assert!(
        world
            .find(&PersistentId::new(["yard", "porch", "post"]))
            .is_some()
    );
    // Nothing cooked again; a path that is no longer the source's, a warning.
    assert!(cook(&root, &output).expect("cooked").cooked.is_empty());
    lamp_and_yard(&root, "old/lamp.scene");
    let cooked = cook(&root, &output).expect("cooked");
    assert_eq!(cooked.cooked, [YARD]);
    assert_eq!(cooked.warnings.len(), 1, "{:?}", cooked.warnings);
    assert!(
        cooked.warnings[0].starts_with("yard.scene:2: ")
            && cooked.warnings[0].contains("`old/lamp.scene`, where its source is `lamp.scene`"),
        "{:?}",
        cooked.warnings
    );
}

#[test]
fn scene_files_that_say_something_wrong_are_refused_with_their_file_and_line() {
    let cases = [
        (
            format!("[assets]\nother = {{ id = \"{ELSEWHERE}\" }}\n"),
            "line 2: the asset `other`",
            "is not one that the sources make",
        ),
        (
            "[entities.porch]\nprefab = \"lamp\"\n".to_owned(),
            "line 2",
            "no asset `lamp` in the table of assets",
        ),
        (
            format!(
                "[assets]\nrock = {{ id = \"{ROCK}\" }}\n\n[entities.porch]\nprefab = \"rock\"\n"
            ),
            "line 4",
            "places the asset `rock`, a texture, where a scene goes",
        ),
        (
            "[entities.a]\nparent = \"nowhere\"\n".to_owned(),
            "line 2",
            "no entity `nowhere` in this scene",
        ),
        (
            "[entities.a]\n\n[entities.b]\nparent = \"a/c\"\n".to_owned(),
            "line 4",
            "no entity `a/c` in this scene",
        ),
        (
            "[entities.a]\nparent = \"b\"\n\n[entities.b]\nparent = \"a\"\n".to_owned(),
            "line 2",
            "`a` would be its own ancestor",
        ),
        (
            "[entities.c]\n\n[entities.b]\nparent = \"a\"\n\n[entities.a]\nparent = \"b\"\n"
                .to_owned(),
            "line 4",
            "`b` would be its own ancestor",
        ),
        (
            "[entities.a]\nparent = \"a\"\n".to_owned(),
            "line 2",
            "`a` would be its own ancestor",
        ),
        (
            format!(
                "[assets]\nyard = {{ id = \"{YARD}\" }}\n\n[entities.again]\nprefab = \"yard\"\n"
            ),
            "line 4",
            "scene files that place each other: yard.scene → yard.scene",
        ),
        ("[entities.porch\n".to_owned(), "line 1", "not TOML"),
    ];
    for (index, (yard, line, reason)) in cases.iter().enumerate() {
        let root = directory(&format!("wrong-{index}")).join("sources");
        png(&root.join("rock.png"), 4, 4, 3);
        fs::write(
            root.join("rock.png.meta"),
            slime_cook::metadata::write(ROCK),
        )
        .expect("the metadata file is written");
        source(&root, "yard.scene", YARD, yard.as_bytes());
        let error = cook(&root, &root.with_file_name("cooked")).expect_err(yard);
        let text = error.to_string();
        assert!(
            matches!(error, CookError::Scene { .. })
                && text.contains("yard.scene")
                && text.contains(line)
                && text.contains(reason),
            "{yard}: {text}"
        );
    }
    // Scene files that place each other.
    let root = directory("cycle").join("sources");
    lamp_and_yard(&root, "lamp.scene");
    source(
        &root,
        "lamp.scene",
        LAMP,
        format!("[assets]\nyard = {{ id = \"{YARD}\" }}\n\n[entities.back]\nprefab = \"yard\"\n")
            .as_bytes(),
    );
    let error = cook(&root, &root.with_file_name("cooked")).expect_err("a cycle");
    let text = error.to_string();
    assert!(
        text.contains("lamp.scene: line 4: scene files that place each other: lamp.scene → yard.scene → lamp.scene"),
        "{text}"
    );
}

#[test]
fn a_library_of_cooked_data_reads_again_the_scenes_that_changed_and_those_that_place_them() {
    let root = directory("reload").join("sources");
    let output = root.with_file_name("cooked");
    lamp_and_yard(&root, "lamp.scene");
    cook(&root, &output).expect("cooked");
    let open = || CookedAssets::open(&output).expect("cooked data");
    let mut scenes = SceneLibrary::of(open());
    let name = |scenes: &mut SceneLibrary| {
        let mut world = World::new();
        let yard = scenes.resolve(world.registry(), YARD).expect("resolved");
        world.spawn_scene("yard", &yard).expect("spawned");
        let post = world
            .find(&PersistentId::new(["yard", "porch", "post"]))
            .expect("the post");
        world
            .get::<slime_world::Name>(post)
            .expect("named")
            .0
            .clone()
    };
    assert_eq!(name(&mut scenes), "post");
    assert!(scenes.reload(open()).is_empty(), "nothing changed");
    source(
        &root,
        "lamp.scene",
        LAMP,
        b"[entities.post.components]\nName = \"pillar\"\n",
    );
    cook(&root, &output).expect("cooked again");
    assert_eq!(scenes.reload(open()), [LAMP]);
    assert_eq!(name(&mut scenes), "pillar", "the yard follows its lamp");
    // A change that the world refuses, then the cooked data before it: the library goes back.
    let good = open();
    let lamp = |name: &str| format!("[entities.post.components]\n{name} = \"pillar\"\n");
    source(&root, "lamp.scene", LAMP, lamp("Nam").as_bytes());
    cook(&root, &output).expect("cooked again: the cooker does not know the types");
    assert_eq!(scenes.reload(open()), [LAMP]);
    let error = scenes
        .resolve(World::new().registry(), YARD)
        .expect_err("refused")
        .to_string();
    assert!(error.contains("`Nam`"), "{error}");
    assert_eq!(scenes.reload(good), [LAMP]);
    assert_eq!(name(&mut scenes), "pillar");
    // The source mended: its cooked file is the one before.
    source(&root, "lamp.scene", LAMP, lamp("Name").as_bytes());
    cook(&root, &output).expect("cooked again");
    assert!(scenes.reload(open()).is_empty());
    // A scene inserted by hand stands for the cooked one until a reload changes it.
    scenes.insert_file(
        LAMP,
        "by hand",
        "[entities.post.components]\nName = \"by hand\"\n",
    );
    assert_eq!(name(&mut scenes), "by hand");
    assert!(scenes.reload(open()).is_empty());
    assert_eq!(name(&mut scenes), "by hand");
    // Renamed with its metadata file: its cooked file is the same, its name is not.
    for (from, to) in [
        ("lamp.scene", "column.scene"),
        ("lamp.scene.meta", "column.scene.meta"),
    ] {
        fs::rename(root.join(from), root.join(to)).expect("renamed");
    }
    cook(&root, &output).expect("cooked again");
    assert_eq!(scenes.reload(open()), [LAMP]);
    let column = scenes
        .resolve(World::new().registry(), LAMP)
        .expect("resolved");
    assert_eq!(column.name(), "column");
    assert_eq!(name(&mut scenes), "pillar");
    // The yard gone from the sources: gone from the library.
    for gone in ["yard.scene", "yard.scene.meta"] {
        fs::remove_file(root.join(gone)).expect("removed");
    }
    cook(&root, &output).expect("cooked again");
    assert_eq!(scenes.reload(open()), [YARD]);
    assert!(matches!(
        scenes.resolve(World::new().registry(), YARD),
        Err(SceneError::Missing { .. })
    ));
}

#[test]
fn style_sheets_and_themes_are_cooked_into_their_binary_encoding_their_syntax_checked() {
    const SHEET: AssetId = AssetId::from_u128(0x0011);
    const THEME: AssetId = AssetId::from_u128(0x0012);
    let root = directory("styles").join("sources");
    let output = root.with_file_name("cooked");
    let sheet =
        "[panel]\nselect = { classes = [\"panel\"] }\nstyle.paint.background = \"$surface\"\n";
    let theme = "surface = \"#26282C\"\n";
    source(&root, "interface.style", SHEET, sheet.as_bytes());
    source(&root, "dark.theme", THEME, theme.as_bytes());
    cook(&root, &output).expect("cooked");
    let assets = CookedAssets::open(&output).expect("cooked data");
    for (id, kind, path) in [
        (SHEET, AssetKind::StyleSheet, "interface.style"),
        (THEME, AssetKind::Theme, "dark.theme"),
    ] {
        let entry = assets.manifest().get(id).expect("listed");
        assert_eq!((entry.kind, entry.source.as_str()), (kind, path));
        assert!(entry.needs.is_empty());
    }
    let encoded = |text| slime_serialize::encode(text).expect("TOML");
    assert_eq!(assets.style_sheet(SHEET).expect("read"), encoded(sheet));
    assert_eq!(assets.theme(THEME).expect("read"), encoded(theme));
    assert!(
        assets.style_sheet(THEME).is_err(),
        "a theme is not a style sheet"
    );
    // A document that is not TOML is refused, by its file and its line.
    source(&root, "dark.theme", THEME, b"surface = \n");
    let error = cook(&root, &output).unwrap_err().to_string();
    assert!(
        error.contains("dark.theme") && error.contains("line 1"),
        "{error}"
    );
}
