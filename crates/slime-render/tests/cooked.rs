//! Loading a cooked scene: a directory of cooked data written in a temporary directory with the
//! formats of `slime-assets`, loaded onto the GPU, its meshes placed by their nodes, each loaded
//! once, its lights in world space, and drawn; loads polled, side by side, one of them dropped, and
//! a panic of a task of a load raised again; a texture of the wrong kind for its slot refused; a
//! material found by its name, and changed; a scene loaded again, which keeps what did not change,
//! and fails without harm to the scene before; the asset cache, which loads what is asked for once,
//! in batches, keeps the textures it holds, does not ask again for what failed or panicked, loads
//! again after it is cleared, and reloads what changed. Runs on whatever adapter the machine
//! offers.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use slime_assets::{
    AssetId, AssetKind, CookedAssets, CookedMaterial, CookedMesh, CookedScene, CookedTexture,
    LightKind, MANIFEST_FILE, Manifest, ManifestEntry, MaterialTexture, SceneLight, SceneNode,
    TextureEncoding, TextureFilter, TextureSampler, TextureWrap,
};
use slime_color::LinearColor;
use slime_gfx::{GfxError, Gpu, TextureFormat};
use slime_render::glam::{DAffine3, DVec3, Mat4, Vec3};
use slime_render::{
    AssetCache, Camera, DirectionalLight, Instance, Light, LoadedScene, MaterialValues, MeshData,
    RenderError, Renderer, Scene, SceneError, scene_lights,
};

const TEXTURE: AssetId = AssetId::from_u128(1);
const MATERIAL: AssetId = AssetId::from_u128(2);
const MESH: AssetId = AssetId::from_u128(3);
const SCENE: AssetId = AssetId::from_u128(4);

/// A transform, column after column, that moves by `x` along +X and scales by `scale`.
fn placed(x: f32, scale: Vec3) -> [f32; 16] {
    Mat4::from_scale_rotation_translation(scale, Default::default(), Vec3::new(x, 0.0, 0.0))
        .to_cols_array()
}

/// Writes into a directory of its own, named `name`, a cooked scene: a square facing +Z, with a
/// red texture, placed by a node at the origin and by its child 2 m along +X, and a directional
/// light along −Z on a third node, scaled by `light_scale`. Returns the directory.
fn cooked(name: &str, light_scale: Vec3) -> PathBuf {
    let directory = directory(name);
    let red = |side: u32| [255, 0, 0, 255].repeat((side * side) as usize);
    let texture = CookedTexture {
        width: 4,
        height: 4,
        encoding: TextureEncoding::Rgba8Srgb,
        levels: vec![red(4), red(2), red(1)],
    };
    let material = CookedMaterial {
        name: "red".to_owned(),
        metallic: 0.0,
        base_color_texture: Some(MaterialTexture {
            texture: TEXTURE,
            sampler: TextureSampler {
                mag_filter: TextureFilter::Nearest,
                min_filter: TextureFilter::Nearest,
                wrap_u: TextureWrap::Repeat,
                wrap_v: TextureWrap::Repeat,
            },
        }),
        ..CookedMaterial::default()
    };
    let square = MeshData::square();
    let mesh = CookedMesh {
        name: "square".to_owned(),
        material: MATERIAL,
        positions: square.positions,
        normals: square.normals,
        uvs: square.uvs,
        tangents: square.tangents,
        indices: square.indices,
        bounds: [[-0.5, -0.5, 0.0], [0.5, 0.5, 0.0]],
    };
    let node = |name: &str, parent, transform, meshes, light| SceneNode {
        name: name.to_owned(),
        parent,
        transform,
        meshes,
        light,
        extras: Vec::new(),
        components: Vec::new(),
    };
    let scene = CookedScene {
        name: "squares".to_owned(),
        nodes: vec![
            node("first", None, placed(0.0, Vec3::ONE), vec![MESH], None),
            node("second", Some(0), placed(2.0, Vec3::ONE), vec![MESH], None),
            node(
                "sun",
                None,
                placed(0.0, light_scale),
                Vec::new(),
                Some(SceneLight {
                    kind: LightKind::Directional,
                    color: [1.0; 3],
                    intensity: 3.0,
                    range: None,
                }),
            ),
        ],
    };
    let assets: Vec<Written> = vec![
        (
            TEXTURE,
            AssetKind::Texture,
            "texture.ktx2".to_owned(),
            texture.to_ktx2(),
            vec![],
        ),
        (
            MATERIAL,
            AssetKind::Material,
            "red.material".to_owned(),
            material.to_bytes(),
            vec![TEXTURE],
        ),
        (
            MESH,
            AssetKind::Mesh,
            "square.mesh".to_owned(),
            mesh.to_bytes(),
            vec![MATERIAL],
        ),
        (
            SCENE,
            AssetKind::Scene,
            "squares.scene".to_owned(),
            scene.to_bytes(),
            vec![MESH],
        ),
    ];
    write(&directory, assets);
    directory
}

/// An asset to write: its ID, kind, file name, bytes, and the IDs it needs.
type Written = (AssetId, AssetKind, String, Vec<u8>, Vec<AssetId>);

/// Writes `assets` into `directory`, and the manifest that lists them.
fn write(directory: &Path, assets: Vec<Written>) {
    let mut manifest = Manifest::new();
    for (id, kind, file, bytes, needs) in assets {
        fs::write(directory.join(&file), &bytes).expect("written");
        manifest.insert(
            id,
            ManifestEntry {
                kind,
                file,
                size: bytes.len() as u64,
                needs,
                source: "squares.gltf".to_owned(),
            },
        );
    }
    fs::write(directory.join(MANIFEST_FILE), manifest.to_bytes()).expect("written");
}

/// A new, empty directory for the cooked data named `name`.
fn directory(name: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slime-render-cooked")
        .join(name);
    if directory.exists() {
        fs::remove_dir_all(&directory).expect("the old directory can be removed");
    }
    fs::create_dir_all(&directory).expect("the directory can be created");
    directory
}

/// The colors of [`palette`]'s textures, one per material.
const COLORS: [[u8; 4]; 3] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];

/// Writes into a directory of its own, named `name`, a cooked scene of six squares facing +Z, 2 m
/// apart along +X, each a mesh of its own placed by a node of its own. Square `i` is drawn with
/// material `i % 3`, which reads a texture of `COLORS[i % 3]`, `width` texels wide and one high.
/// Returns the directory and the scene's ID.
fn palette(name: &str, width: u32) -> (PathBuf, AssetId) {
    let directory = directory(name);
    let id = |kind: u128, index: usize| AssetId::from_u128(kind * 100 + index as u128);
    let (texture, material, mesh, scene) = (1, 2, 3, 4);
    let mut assets: Vec<Written> = Vec::new();
    for (index, color) in COLORS.iter().enumerate() {
        let cooked = CookedTexture {
            width,
            height: 1,
            encoding: TextureEncoding::Rgba8Srgb,
            levels: vec![color.repeat(width as usize)],
        };
        let sampler = TextureSampler {
            mag_filter: TextureFilter::Nearest,
            min_filter: TextureFilter::Nearest,
            wrap_u: TextureWrap::ClampToEdge,
            wrap_v: TextureWrap::ClampToEdge,
        };
        let cooked_material = CookedMaterial {
            name: format!("material {index}"),
            metallic: 0.0,
            base_color_texture: Some(MaterialTexture {
                texture: id(texture, index),
                sampler,
            }),
            ..CookedMaterial::default()
        };
        assets.push((
            id(texture, index),
            AssetKind::Texture,
            format!("{index}.ktx2"),
            cooked.to_ktx2(),
            vec![],
        ));
        assets.push((
            id(material, index),
            AssetKind::Material,
            format!("{index}.material"),
            cooked_material.to_bytes(),
            vec![id(texture, index)],
        ));
    }
    let square = MeshData::square();
    let mut nodes = Vec::new();
    for index in 0..6 {
        let cooked = CookedMesh {
            name: format!("square {index}"),
            material: id(material, index % 3),
            positions: square.positions.clone(),
            normals: square.normals.clone(),
            uvs: square.uvs.clone(),
            tangents: square.tangents.clone(),
            indices: square.indices.clone(),
            bounds: [[-0.5, -0.5, 0.0], [0.5, 0.5, 0.0]],
        };
        assets.push((
            id(mesh, index),
            AssetKind::Mesh,
            format!("{index}.mesh"),
            cooked.to_bytes(),
            vec![id(material, index % 3)],
        ));
        nodes.push(SceneNode {
            name: format!("square {index}"),
            parent: None,
            transform: placed(2.0 * index as f32, Vec3::ONE),
            meshes: vec![id(mesh, index)],
            light: None,
            extras: Vec::new(),
            components: Vec::new(),
        });
    }
    let cooked = CookedScene {
        name: "palette".to_owned(),
        nodes,
    };
    assets.push((
        id(scene, 0),
        AssetKind::Scene,
        "palette.scene".to_owned(),
        cooked.to_bytes(),
        (0..6).map(|index| id(mesh, index)).collect(),
    ));
    write(&directory, assets);
    (directory, id(scene, 0))
}

/// The color of the center of what `renderer` draws of `instance`, seen from 2 m in front of it,
/// lit by a light along −Z.
fn center_of(gpu: &Gpu, renderer: &mut Renderer, instance: Instance<'_>) -> [u8; 3] {
    let target = gpu
        .create_render_target("palette", 16, 16, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let x = instance.transform.translation.x;
    let camera = Camera {
        eye: DVec3::new(x, 0.0, 2.0),
        target: DVec3::new(x, 0.0, 0.0),
        ..Camera::default()
    };
    let lights = [Light::Directional(DirectionalLight {
        direction: Vec3::NEG_Z,
        illuminance: 3.0,
        ..DirectionalLight::default()
    })];
    let instances = [instance];
    let mut scene = Scene::new(camera, &instances);
    scene.lights = &lights;
    scene.exposure = 0.5;
    renderer
        .render(gpu, &target, &scene)
        .expect("the frame is drawn");
    let image = gpu.read_render_target(&target).expect("read back");
    let center = &image.pixels[(8 * 16 + 8) * 4..][..3];
    [center[0], center[1], center[2]]
}

/// Checks that `loaded` is [`palette`]'s scene: six instances, each mesh its own, the materials
/// shared by every third square, and each square drawn in its color.
fn check_palette(gpu: &Gpu, renderer: &mut Renderer, loaded: &LoadedScene) {
    let instances = loaded.instances();
    assert_eq!(instances.len(), 6);
    for (index, instance) in instances.iter().enumerate() {
        assert_eq!(instance.transform.translation.x, 2.0 * index as f64);
        for (other, other_instance) in instances.iter().enumerate() {
            assert_eq!(
                std::ptr::eq(instance.mesh, other_instance.mesh),
                index == other
            );
            assert_eq!(
                std::ptr::eq(instance.material, other_instance.material),
                index % 3 == other % 3
            );
        }
        let [r, g, b] = center_of(gpu, renderer, *instance);
        let lit = [r, g, b].map(|channel| channel > 100);
        let expected = COLORS[index % 3].map(|channel| channel == 255);
        assert_eq!(
            lit,
            [expected[0], expected[1], expected[2]],
            "{index}: {r} {g} {b}"
        );
    }
}

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

#[test]
fn a_cooked_scene_is_placed_lit_and_drawn() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let assets = CookedAssets::open(cooked("drawn", Vec3::ONE)).expect("the manifest is read");
    let loaded = renderer
        .load_scene(&gpu, &assets, SCENE)
        .expect("the scene loads");
    let instances = loaded.instances();
    // The square, loaded once, placed twice; the second by its parent's transform times its own.
    assert_eq!(instances.len(), 2);
    assert!(std::ptr::eq(instances[0].mesh, instances[1].mesh));
    assert_eq!(instances[1].transform.translation.x, 2.0);
    assert_eq!(
        loaded.bounds,
        Some((DVec3::new(-0.5, -0.5, 0.0), DVec3::new(2.5, 0.5, 0.0)))
    );
    let [Light::Directional(sun)] = loaded.lights[..] else {
        panic!("one directional light: {:?}", loaded.lights);
    };
    assert!(sun.direction.abs_diff_eq(Vec3::NEG_Z, 1e-6));
    assert_eq!(loaded.scene.nodes[2].name, "sun");
    // The same lights, read from the cooked scene without loading it.
    assert_eq!(
        scene_lights(&assets.scene(SCENE).expect("the scene is read")).expect("placed"),
        loaded.lights
    );

    // Seen from the front, under its light, the first square shows red.
    let target = gpu
        .create_render_target("cooked", 16, 16, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let camera = Camera {
        eye: DVec3::new(0.0, 0.0, 2.0),
        target: DVec3::ZERO,
        ..Camera::default()
    };
    let mut scene = Scene::new(camera, &instances);
    scene.lights = &loaded.lights;
    scene.exposure = 0.5;
    let mut renderer = renderer;
    renderer
        .render(&gpu, &target, &scene)
        .expect("the frame is drawn");
    let image = gpu.read_render_target(&target).expect("read back");
    let center = &image.pixels[(8 * 16 + 8) * 4..][..4];
    assert!(
        center[0] > 100 && center[1] < 20 && center[2] < 20,
        "{center:?}"
    );
}

#[test]
fn a_light_without_a_direction_and_a_missing_asset_are_refused() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    // The light's node flattens its −Z axis.
    let assets = CookedAssets::open(cooked("flat", Vec3::new(1.0, 1.0, 0.0))).expect("read");
    let Err(error) = renderer.load_scene(&gpu, &assets, SCENE) else {
        panic!("the scene loaded");
    };
    assert!(
        matches!(error, SceneError::Placement { node: 2, .. }),
        "{error}"
    );
    assert!(error.to_string().contains("has no direction"), "{error}");

    // The texture's file is gone.
    let directory = cooked("missing", Vec3::ONE);
    fs::remove_file(directory.join("texture.ktx2")).expect("removed");
    let assets = CookedAssets::open(&directory).expect("read");
    let Err(error) = renderer.load_scene(&gpu, &assets, SCENE) else {
        panic!("the scene loaded");
    };
    assert!(matches!(error, SceneError::Load(_)), "{error}");
    assert!(error.to_string().contains("texture.ktx2"), "{error}");
}

#[test]
fn a_scene_loaded_in_the_background_holds_each_asset_once_where_its_nodes_place_it() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("background", 4);
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    let mut load = renderer.load_scene_in_background(&gpu, &assets, scene);
    // A game polls the load at each frame until it gives the scene.
    let loaded = loop {
        if let Some(result) = load.take() {
            break result.expect("the scene loads");
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    assert!(load.take().is_none(), "the result is taken once");
    check_palette(&gpu, &mut renderer, &loaded);
}

#[test]
fn loads_go_on_side_by_side_and_dropping_one_leaves_the_others_alone() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("side by side", 4);
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    let first = renderer.load_scene_in_background(&gpu, &assets, scene);
    let second = renderer.load_scene_in_background(&gpu, &assets, scene);
    // Dropped at once, the third leaves the other two to finish.
    drop(renderer.load_scene_in_background(&gpu, &assets, scene));
    for load in [first, second] {
        let loaded = load.wait().expect("the scene loads");
        check_palette(&gpu, &mut renderer, &loaded);
    }
}

#[test]
fn a_load_on_a_lost_device_ends_with_the_loss_and_never_hangs() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("lost", 4);
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    gpu.lose();
    // The load's tasks find the loss when they create the meshes' and the textures' GPU objects,
    // before any copy reaches the uploader, and the load ends with the loss.
    let Err(error) = renderer.load_scene(&gpu, &assets, scene) else {
        panic!("the scene loaded on a lost device");
    };
    assert!(
        matches!(
            &error,
            SceneError::Render(RenderError::Gfx(GfxError::DeviceLost { .. }))
        ),
        "{error:?}"
    );
    // The asset cache's batches end the same way.
    let mut cache = AssetCache::new(assets);
    cache.request(&renderer, &gpu, palette_meshes());
    let errors = settle(&mut cache);
    assert!(
        errors.iter().all(|error| matches!(
            error,
            SceneError::Render(RenderError::Gfx(GfxError::DeviceLost { .. }))
        )),
        "{errors:?}"
    );
    // The renderer, its uploader's thread with it, goes.
    drop(renderer);
}

#[test]
fn a_panic_of_a_load_s_task_is_raised_where_the_load_is_taken() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("panic", 4);
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    let previous = renderer
        .load_scene(&gpu, &assets, scene)
        .expect("the scene loads");
    // Reloaded on another device, which cannot read the textures it keeps: wgpu panics when the
    // materials are made, by the task that completes the load, a bug of the caller's.
    let other = self::gpu();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        renderer
            .reload_scene_in_background(&other, &assets, scene, &previous)
            .wait()
    }));
    assert!(panicked.is_err(), "the load panicked");

    // The pool's threads caught the panic: they go on loading.
    let (directory, scene) = palette("after a panic", 4);
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    renderer
        .load_scene(&gpu, &assets, scene)
        .expect("the scene loads");
}

#[test]
fn a_cooked_texture_of_the_wrong_kind_for_its_slot_is_refused() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("wrong kind", 4);
    // Data, where the base color slot reads colors: a file of the same size.
    let data = CookedTexture {
        width: 4,
        height: 1,
        encoding: TextureEncoding::Rgba8,
        levels: vec![COLORS[0].repeat(4)],
    };
    fs::write(directory.join("0.ktx2"), data.to_ktx2()).expect("written");
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    let error = renderer
        .load_scene(&gpu, &assets, scene)
        .err()
        .expect("refused");
    assert!(
        matches!(&error, SceneError::Render(RenderError::InvalidMaterial { reason, .. })
            if reason.contains("its base color texture is Rgba8Unorm")),
        "{error}"
    );
}

#[test]
fn a_material_found_by_its_name_changes_every_square_drawn_with_it() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("by name", 4);
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    let loaded = renderer
        .load_scene(&gpu, &assets, scene)
        .expect("the scene loads");
    assert!(loaded.material("material 3").is_none());
    let green = loaded.material("material 1").expect("the second material");
    let instances = loaded.instances();
    assert!(std::ptr::eq(green, instances[1].material));
    // Its green texture times a base color without green: the squares 1 and 4 turn black.
    let made = green.values();
    green
        .set_values(
            &gpu,
            MaterialValues {
                base_color: LinearColor::rgb(1.0, 0.0, 1.0),
                ..made
            },
        )
        .expect("valid values");
    for (index, instance) in instances.iter().enumerate() {
        let [r, g, b] = center_of(&gpu, &mut renderer, *instance);
        let lit = r.max(g).max(b) > 100;
        assert_eq!(lit, index % 3 != 1, "{index}: {r} {g} {b}");
    }
}

#[test]
fn a_reloaded_scene_draws_the_new_texture_and_keeps_what_did_not_change() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("reload", 4);
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    let previous = renderer
        .load_scene(&gpu, &assets, scene)
        .expect("the scene loads");

    whiten_the_second_texture(&directory);
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    let Err(error) = renderer.load_scene(&gpu, &assets, scene) else {
        panic!("a first load read nothing from the disk");
    };
    assert!(matches!(error, SceneError::Load(_)), "{error}");

    // The reload keeps them, and uploads the new texture alone.
    let reloaded = renderer
        .reload_scene_in_background(&gpu, &assets, scene, &previous)
        .wait()
        .expect("the scene loads again");
    for (index, instance) in reloaded.instances().iter().enumerate() {
        let [r, g, b] = center_of(&gpu, &mut renderer, *instance);
        let lit = [r, g, b].map(|channel| channel > 100);
        let expected = match index % 3 {
            1 => [true; 3],
            _ => COLORS[index % 3].map(|channel| channel == 255)[..3]
                .try_into()
                .expect("three channels"),
        };
        assert_eq!(lit, expected, "{index}: {r} {g} {b}");
    }
    // The scene reloaded is whole, and drawn as before.
    check_palette(&gpu, &mut renderer, &previous);
}

#[test]
fn a_reload_of_data_that_did_not_change_reads_no_mesh_and_no_texture() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("unchanged", 4);
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    let previous = renderer
        .load_scene(&gpu, &assets, scene)
        .expect("the scene loads");
    for index in 0..6 {
        fs::remove_file(directory.join(format!("{index}.mesh"))).expect("removed");
    }
    for index in 0..3 {
        fs::remove_file(directory.join(format!("{index}.ktx2"))).expect("removed");
    }
    let reloaded = renderer
        .reload_scene_in_background(&gpu, &assets, scene, &previous)
        .wait()
        .expect("everything is kept");
    check_palette(&gpu, &mut renderer, &reloaded);
}

#[test]
fn a_reload_that_fails_leaves_the_scene_before_whole() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, scene) = palette("fails", 4);
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    let previous = renderer
        .load_scene(&gpu, &assets, scene)
        .expect("the scene loads");
    // The second material now names a file that is not there, read for its meshes, which are kept.
    let material = AssetId::from_u128(201);
    let path = directory.join(MANIFEST_FILE);
    let mut manifest = Manifest::from_bytes(&fs::read(&path).expect("read")).expect("a manifest");
    let entry = manifest.get(material).expect("listed").clone();
    manifest.insert(
        material,
        ManifestEntry {
            file: "1-gone.material".to_owned(),
            ..entry
        },
    );
    fs::write(&path, manifest.to_bytes()).expect("written");
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    let Err(error) = renderer
        .reload_scene_in_background(&gpu, &assets, scene, &previous)
        .wait()
    else {
        panic!("the scene loaded without its material");
    };
    assert!(error.to_string().contains("1-gone.material"), "{error}");
    check_palette(&gpu, &mut renderer, &previous);
}

/// Cooks the texture of [`palette`]'s second material, green, again white, under a new name, as
/// the cooker names a changed image; and removes the meshes and the other textures from the disk,
/// so that what reads them fails.
fn whiten_the_second_texture(directory: &Path) {
    let texture = AssetId::from_u128(101);
    let white = CookedTexture {
        width: 4,
        height: 1,
        encoding: TextureEncoding::Rgba8Srgb,
        levels: vec![[255; 4].repeat(4)],
    }
    .to_ktx2();
    fs::write(directory.join("1-white.ktx2"), &white).expect("written");
    let path = directory.join(MANIFEST_FILE);
    let mut manifest = Manifest::from_bytes(&fs::read(&path).expect("read")).expect("a manifest");
    let entry = manifest.get(texture).expect("listed").clone();
    manifest.insert(
        texture,
        ManifestEntry {
            file: "1-white.ktx2".to_owned(),
            size: white.len() as u64,
            ..entry
        },
    );
    fs::write(&path, manifest.to_bytes()).expect("written");
    for file in ["0.ktx2", "2.ktx2"]
        .into_iter()
        .map(str::to_owned)
        .chain((0..6).map(|index| format!("{index}.mesh")))
    {
        fs::remove_file(directory.join(file)).expect("removed");
    }
}

/// The meshes of [`palette`]'s squares, in their order.
fn palette_meshes() -> [AssetId; 6] {
    std::array::from_fn(|index| AssetId::from_u128(300 + index as u128))
}

/// Polls `cache` until it neither loads nor reloads, and returns the errors it gave.
fn settle(cache: &mut AssetCache) -> Vec<SceneError> {
    let mut errors = Vec::new();
    for _ in 0..10_000 {
        errors.extend(cache.poll());
        if cache.loading() == 0 && !cache.reloading() {
            return errors;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("the cache still loads after 10 s");
}

/// Checks that `cache` holds each of [`palette`]'s squares in `indices`, and that each is drawn in
/// its material's color, or white where `white` says so.
fn check_cached(
    gpu: &Gpu,
    renderer: &mut Renderer,
    cache: &AssetCache,
    indices: std::ops::Range<usize>,
    white: impl Fn(usize) -> bool,
) {
    let meshes = palette_meshes();
    for index in indices {
        let (mesh, material) = cache.mesh(meshes[index]).expect("held");
        let placed = DAffine3::from_translation(DVec3::new(2.0 * index as f64, 0.0, 0.0));
        let [r, g, b] = center_of(gpu, renderer, Instance::new(mesh, material, placed));
        let lit = [r, g, b].map(|channel| channel > 100);
        let expected = if white(index) {
            [true; 3]
        } else {
            COLORS[index % 3].map(|channel| channel == 255)[..3]
                .try_into()
                .expect("three channels")
        };
        assert_eq!(lit, expected, "{index}: {r} {g} {b}");
    }
}

#[test]
fn the_cache_loads_what_is_asked_for_once_and_shares_the_materials() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, _) = palette("cache", 4);
    let mut cache = AssetCache::new(CookedAssets::open(&directory).expect("the manifest is read"));
    let meshes = palette_meshes();
    assert!(cache.mesh(meshes[0]).is_none());
    cache.request(&renderer, &gpu, meshes);
    assert_eq!(cache.loading(), 6);
    cache.request(&renderer, &gpu, meshes);
    assert_eq!(cache.loading(), 6, "asked for twice, loaded once");
    assert!(settle(&mut cache).is_empty());
    // Squares 0 and 3 are drawn with the same material, which the cache holds once.
    let material = |index: usize| cache.mesh(meshes[index]).expect("held").1;
    assert!(std::ptr::eq(material(0), material(3)));
    assert!(!std::ptr::eq(material(0), material(1)));
    assert!(std::ptr::eq(
        material(1),
        cache.material(AssetId::from_u128(201)).expect("held")
    ));
    cache.request(&renderer, &gpu, meshes);
    assert_eq!(cache.loading(), 0, "held already");
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
}

#[test]
fn a_second_batch_keeps_the_textures_that_the_cache_holds() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, _) = palette("cache-second", 4);
    let mut cache = AssetCache::new(CookedAssets::open(&directory).expect("the manifest is read"));
    let meshes = palette_meshes();
    cache.request(&renderer, &gpu, meshes[..3].iter().copied());
    assert!(settle(&mut cache).is_empty());
    // The last three squares are drawn with the same materials, whose textures the cache holds:
    // those are not read again.
    for index in 0..3 {
        fs::remove_file(directory.join(format!("{index}.ktx2"))).expect("removed");
    }
    cache.request(&renderer, &gpu, meshes[3..].iter().copied());
    assert_eq!(cache.loading(), 3);
    assert!(settle(&mut cache).is_empty());
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
}

#[test]
fn a_mesh_that_cannot_be_read_is_reported_once_and_not_asked_for_again() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, _) = palette("cache-missing", 4);
    let mut cache = AssetCache::new(CookedAssets::open(&directory).expect("the manifest is read"));
    let missing = AssetId::from_u128(999);
    cache.request(&renderer, &gpu, [missing]);
    let errors = settle(&mut cache);
    assert!(matches!(errors[..], [SceneError::Load(_)]), "{errors:?}");
    cache.request(&renderer, &gpu, [missing]);
    assert_eq!(cache.loading(), 0);
    assert!(cache.poll().is_empty());
    assert!(cache.mesh(missing).is_none());
}

#[test]
fn a_panic_while_the_cache_loads_leaves_it_loading_nothing() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, _) = palette("cache-panic", 4);
    let mut cache = AssetCache::new(CookedAssets::open(&directory).expect("the manifest is read"));
    let meshes = palette_meshes();
    cache.request(&renderer, &gpu, [meshes[0]]);
    assert!(settle(&mut cache).is_empty());
    // Square 3 is drawn with square 0's material, which a batch makes again with the texture the
    // cache holds: on another device, which cannot read it, a bug of the caller's that panics.
    let other = self::gpu();
    cache.request(&renderer, &other, [meshes[3]]);
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| settle(&mut cache)));
    assert!(panicked.is_err(), "the batch panicked");
    assert_eq!(cache.loading(), 0, "the batch is over");
    assert!(cache.mesh(meshes[3]).is_none());
    // Its mesh is not asked for again, as for a batch that failed.
    cache.request(&renderer, &gpu, [meshes[3]]);
    assert_eq!(cache.loading(), 0);
}

#[test]
fn a_cleared_cache_holds_nothing_and_loads_again_what_is_asked_for() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, _) = palette("cache-cleared", 4);
    let mut cache = AssetCache::new(CookedAssets::open(&directory).expect("the manifest is read"));
    let meshes = palette_meshes();
    cache.request(&renderer, &gpu, meshes);
    assert!(settle(&mut cache).is_empty());
    // Cleared while a batch loads, and once it holds everything.
    cache.request(&renderer, &gpu, [AssetId::from_u128(999)]);
    cache.clear();
    assert_eq!(cache.loading(), 0);
    assert!(cache.mesh(meshes[0]).is_none() && cache.material(AssetId::from_u128(200)).is_none());
    assert!(cache.poll().is_empty());
    cache.request(&renderer, &gpu, meshes);
    assert!(settle(&mut cache).is_empty());
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
}

#[test]
fn a_reload_keeps_what_did_not_change_and_takes_the_rest_at_once() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, _) = palette("cache-reload", 4);
    let mut cache = AssetCache::new(CookedAssets::open(&directory).expect("the manifest is read"));
    let meshes = palette_meshes();
    cache.request(&renderer, &gpu, meshes);
    assert!(settle(&mut cache).is_empty());
    whiten_the_second_texture(&directory);
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    cache.reload(&renderer, &gpu, assets);
    // Until the reload finishes, the cache gives what it held.
    assert!(cache.reloading());
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
    // The meshes and the textures that did not change are kept: their files are gone.
    assert!(settle(&mut cache).is_empty());
    check_cached(&gpu, &mut renderer, &cache, 0..6, |index| index % 3 == 1);
}

#[test]
fn a_reload_drops_the_meshes_that_the_new_data_no_longer_lists() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, _) = palette("cache-removed", 4);
    let mut cache = AssetCache::new(CookedAssets::open(&directory).expect("the manifest is read"));
    let meshes = palette_meshes();
    cache.request(&renderer, &gpu, meshes);
    assert!(settle(&mut cache).is_empty());
    // The last square is cooked no more: the new manifest does not list it.
    let path = directory.join(MANIFEST_FILE);
    let manifest = Manifest::from_bytes(&fs::read(&path).expect("read")).expect("a manifest");
    let mut listed = Manifest::new();
    for (id, entry) in manifest.iter().filter(|&(id, _)| id != meshes[5]) {
        listed.insert(id, entry.clone());
    }
    fs::write(&path, listed.to_bytes()).expect("written");
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    cache.reload(&renderer, &gpu, assets);
    assert!(cache.mesh(meshes[5]).is_none(), "dropped at once");
    assert!(settle(&mut cache).is_empty(), "the others are loaded again");
    check_cached(&gpu, &mut renderer, &cache, 0..5, |_| false);
    // The reloads after it go through.
    whiten_the_second_texture(&directory);
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    cache.reload(&renderer, &gpu, assets);
    assert!(settle(&mut cache).is_empty());
    check_cached(&gpu, &mut renderer, &cache, 0..5, |index| index % 3 == 1);
}
