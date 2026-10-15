//! Cooking directories of source images into directories of cooked data, in temporary
//! directories: the same bytes from the same sources, nothing cooked again that has not changed,
//! and textures that a game loads by the image's ID.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use slime_assets::{AssetId, CookedAssets, TextureEncoding};
use slime_cook::{CookError, cook, scan};

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
        let expected = if texture.width == 16 {
            TextureEncoding::Bc7Srgb
        } else {
            // 6 × 5 texels, which blocks of 4 × 4 do not divide.
            TextureEncoding::Rgba8Srgb
        };
        assert_eq!(texture.encoding, expected);
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
