//! Loading a cooked scene: a directory of cooked data written in a temporary directory with the
//! formats of `slime-assets`, loaded onto the GPU, its meshes placed by their nodes, each loaded
//! once, its lights in world space, and drawn; loads polled, side by side, one of them dropped, and
//! a panic of a loading thread raised again. Runs on whatever adapter the machine offers.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use slime_assets::{
    AssetId, AssetKind, CookedAssets, CookedMaterial, CookedMesh, CookedScene, CookedTexture,
    LightKind, MANIFEST_FILE, Manifest, ManifestEntry, MaterialTexture, SceneLight, SceneNode,
    TextureEncoding, TextureFilter, TextureSampler, TextureWrap,
};
use slime_gfx::{Gpu, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Light, LoadedScene, MeshData, Renderer, Scene, SceneError,
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
    let x = instance.transform.w_axis.x;
    let camera = Camera {
        eye: Vec3::new(x, 0.0, 2.0),
        target: Vec3::new(x, 0.0, 0.0),
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
    assert_eq!(loaded.instance_nodes(), [0, 1, 2, 3, 4, 5]);
    for (index, instance) in instances.iter().enumerate() {
        assert_eq!(instance.transform.w_axis.x, 2.0 * index as f32);
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
    assert_eq!(instances[1].transform.w_axis.x, 2.0);
    assert_eq!(loaded.instance_nodes(), [0, 1]);
    assert_eq!(
        loaded.bounds,
        Some((Vec3::new(-0.5, -0.5, 0.0), Vec3::new(2.5, 0.5, 0.0)))
    );
    let [Light::Directional(sun)] = loaded.lights[..] else {
        panic!("one directional light: {:?}", loaded.lights);
    };
    assert!(sun.direction.abs_diff_eq(Vec3::NEG_Z, 1e-6));
    assert_eq!(loaded.scene.nodes[2].name, "sun");

    // Seen from the front, under its light, the first square shows red.
    let target = gpu
        .create_render_target("cooked", 16, 16, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 2.0),
        target: Vec3::ZERO,
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
fn a_panic_on_a_loading_thread_is_raised_where_the_load_is_taken() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    // Wider than the device allows: wgpu refuses to create it, which is a bug of the caller's.
    let width = gpu.max_texture_dimension() * 2;
    let (directory, scene) = palette("panic", width);
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        renderer.load_scene(&gpu, &assets, scene)
    })) else {
        panic!("the load did not panic");
    };
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    assert!(message.contains("wgpu error"), "{message}");

    // The renderer's threads caught the panic: they go on loading.
    let (directory, scene) = palette("after a panic", 4);
    let assets = CookedAssets::open(directory).expect("the manifest is read");
    renderer
        .load_scene(&gpu, &assets, scene)
        .expect("the scene loads");
}
