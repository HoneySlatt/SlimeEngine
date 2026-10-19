//! Cooked files damaged (`docs/adr/0096-the-files-the-engine-reads.md`): each reader of cooked data,
//! given a valid file cut at every byte, its bytes changed and mutated at random, refuses it or
//! reads it, and never panics; a texture that announces more than its file holds is refused before
//! its levels are copied; and a directory of cooked data opens only the files of its own that the
//! manifest names.

#[path = "common/damage.rs"]
mod damage;

use std::fs;
use std::path::{Path, PathBuf};

use damage::damage;
use slime_assets::{
    AssetId, AssetKind, CookedAssets, CookedError, CookedMaterial, CookedMesh, CookedScene,
    CookedTexture, Extra, LightKind, LoadError, MANIFEST_FILE, Manifest, ManifestEntry,
    MaterialTexture, SceneLight, SceneNode, TextureEncoding, TextureFilter, TextureSampler,
    TextureWrap,
};

/// A texture of `encoding`, `width` × `height`, every level down to one texel filled with its
/// index.
fn texture(encoding: TextureEncoding, width: u32, height: u32) -> CookedTexture {
    let count = (u32::BITS - width.max(height).leading_zeros()) as usize;
    let levels = (0..count)
        .map(|index| {
            let (w, h) = ((width >> index).max(1), (height >> index).max(1));
            vec![index as u8 + 1; encoding.level_size(w, h)]
        })
        .collect();
    CookedTexture {
        width,
        height,
        encoding,
        levels,
    }
}

#[test]
fn a_damaged_texture_is_refused_or_read_never_a_panic() {
    for file in [
        texture(TextureEncoding::Rgba8Srgb, 5, 3).to_ktx2(),
        texture(TextureEncoding::Bc7, 8, 4).to_ktx2(),
    ] {
        damage(&file, CookedTexture::from_ktx2);
    }
}

/// A KTX2 file of a texture of 4 × 4 texels that announces `levels` levels, each of the same `size`
/// bytes, which follow its header, its index of levels and its data format descriptor.
fn overlapping(levels: usize, size: usize) -> Vec<u8> {
    let valid = texture(TextureEncoding::Rgba8, 4, 4).to_ktx2();
    let word = |at: usize| u32::from_le_bytes(valid[at..at + 4].try_into().expect("4 bytes"));
    let descriptor = &valid[word(48) as usize..][..word(52) as usize];
    let mut file = valid[..80].to_vec();
    file[40..44].copy_from_slice(&(levels as u32).to_le_bytes());
    let start = 80 + levels * 24;
    file[48..52].copy_from_slice(&(start as u32).to_le_bytes());
    let data = (start + descriptor.len()).next_multiple_of(4);
    for _ in 0..levels {
        for value in [data, size, size] {
            file.extend_from_slice(&(value as u64).to_le_bytes());
        }
    }
    file.extend_from_slice(descriptor);
    file.resize(data + size, 0);
    file
}

#[test]
fn a_texture_that_announces_more_levels_than_its_file_holds_is_refused_before_they_are_copied() {
    // A file of 1 MiB whose 4000 levels are each the same 1 MiB: copied before they were checked,
    // they took 4 GiB.
    let file = overlapping(4000, 1 << 20);
    let start = std::time::Instant::now();
    let error = CookedTexture::from_ktx2(&file).expect_err("refused");
    assert!(error.to_string().contains("4000 levels"), "{error}");
    assert!(
        start.elapsed() < std::time::Duration::from_secs(1),
        "refused in {:?}: its levels were copied",
        start.elapsed()
    );
}

#[test]
fn a_damaged_mesh_material_scene_or_manifest_is_refused_or_read_never_a_panic() {
    let mesh = CookedMesh {
        name: "square".to_owned(),
        material: AssetId::from_u128(2),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
        bounds: [[0.0; 3], [1.0, 1.0, 0.0]],
    };
    damage(&mesh.to_bytes(), CookedMesh::from_bytes);

    let sampler = TextureSampler {
        mag_filter: TextureFilter::Linear,
        min_filter: TextureFilter::Nearest,
        wrap_u: TextureWrap::Repeat,
        wrap_v: TextureWrap::ClampToEdge,
    };
    let material = CookedMaterial {
        name: "bark".to_owned(),
        base_color_texture: Some(MaterialTexture {
            texture: AssetId::from_u128(3),
            sampler,
        }),
        alpha_cutoff: Some(0.5),
        ..CookedMaterial::default()
    };
    damage(&material.to_bytes(), CookedMaterial::from_bytes);

    let node = |name: &str, parent| SceneNode {
        name: name.to_owned(),
        parent,
        transform: std::array::from_fn(|index| f32::from(u8::from(index % 5 == 0))),
        meshes: vec![AssetId::from_u128(1)],
        light: Some(SceneLight {
            kind: LightKind::Spot {
                inner_cone_angle: 0.2,
                outer_cone_angle: 0.4,
            },
            color: [1.0, 0.5, 0.25],
            intensity: 100.0,
            range: Some(5.0),
        }),
        extras: vec![
            ("door".to_owned(), Extra::Bool(true)),
            ("height".to_owned(), Extra::Number(2.5)),
            ("key".to_owned(), Extra::Text("brass".to_owned())),
        ],
        components: vec![1, 2, 3],
    };
    let scene = CookedScene {
        name: "ruin".to_owned(),
        nodes: vec![node("walls", None), node("door", Some(0))],
    };
    damage(&scene.to_bytes(), CookedScene::from_bytes);

    let mut manifest = Manifest::new();
    for id in [1, 2] {
        manifest.insert(
            AssetId::from_u128(id),
            ManifestEntry {
                kind: AssetKind::Mesh,
                file: format!("{id}.mesh"),
                size: 160,
                needs: vec![AssetId::from_u128(id + 1)],
                source: "ruin.gltf".to_owned(),
            },
        );
    }
    damage(&manifest.to_bytes(), Manifest::from_bytes);
}

/// A new, empty directory named `name`.
fn directory(name: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slime-assets-damaged")
        .join(name);
    if directory.exists() {
        fs::remove_dir_all(&directory).expect("the old directory can be removed");
    }
    fs::create_dir_all(&directory).expect("the directory can be created");
    directory
}

/// Writes into `directory` a manifest that lists the texture 1 in the file `file`, of `size` bytes.
fn list(directory: &Path, file: &str, size: u64) {
    let mut manifest = Manifest::new();
    manifest.insert(
        AssetId::from_u128(1),
        ManifestEntry {
            kind: AssetKind::Texture,
            file: file.to_owned(),
            size,
            needs: Vec::new(),
            source: "rock.png".to_owned(),
        },
    );
    fs::write(directory.join(MANIFEST_FILE), manifest.to_bytes()).expect("written");
}

#[test]
fn a_manifest_that_names_a_file_outside_its_directory_is_refused() {
    let directory = directory("outside");
    let file = texture(TextureEncoding::Rgba8, 4, 4).to_ktx2();
    // A texture beside the directory, which a name that climbs out of it would reach.
    fs::write(directory.join("../outside.ktx2"), &file).expect("written");
    let outside = directory
        .parent()
        .expect("a parent")
        .join("outside.ktx2")
        .to_string_lossy()
        .into_owned();
    for name in [
        "../outside.ktx2",
        outside.as_str(),
        "",
        ".",
        "..",
        "a/b.ktx2",
        "a\\b.ktx2",
        "c:b.ktx2",
    ] {
        list(&directory, name, file.len() as u64);
        let error = CookedAssets::open(&directory).expect_err(name);
        assert!(
            matches!(&error, LoadError::Cooked { source: CookedError::Malformed { reason, .. }, .. }
                if reason.contains("is not the name of a file")),
            "{name}: {error}"
        );
    }
}

#[test]
fn a_cooked_file_is_checked_before_it_is_read() {
    let directory = directory("checked");
    let file = texture(TextureEncoding::Rgba8, 4, 4).to_ktx2();
    let assets = |name: &str, size: u64| {
        list(&directory, name, size);
        CookedAssets::open(&directory).expect("the manifest is read")
    };
    let id = AssetId::from_u128(1);
    fs::write(directory.join("rock.ktx2"), &file).expect("written");
    assert!(assets("rock.ktx2", file.len() as u64).texture(id).is_ok());
    // Of another size than the manifest says.
    let error = assets("rock.ktx2", file.len() as u64 + 1)
        .texture(id)
        .expect_err("refused");
    assert!(error.to_string().contains("the manifest says"), "{error}");
    // Not a file.
    fs::create_dir(directory.join("folder")).expect("created");
    let error = assets("folder", 0).texture(id).expect_err("refused");
    assert!(error.to_string().contains("not a file"), "{error}");
    // A file that never ends, which a read whole would never finish.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/dev/zero", directory.join("zero")).expect("linked");
        let error = assets("zero", 4096).texture(id).expect_err("refused");
        assert!(error.to_string().contains("not a file"), "{error}");
    }
}
