//! Loading a cooked scene: a directory of cooked data written in a temporary directory with the
//! formats of `slime-assets`, loaded onto the GPU, its meshes placed by their nodes, each loaded
//! once, its lights in world space, and drawn. Runs on whatever adapter the machine offers.

use std::fs;
use std::path::{Path, PathBuf};

use slime_assets::{
    AssetId, AssetKind, CookedAssets, CookedMaterial, CookedMesh, CookedScene, CookedTexture,
    LightKind, MANIFEST_FILE, Manifest, ManifestEntry, MaterialTexture, SceneLight, SceneNode,
    TextureEncoding, TextureFilter, TextureSampler, TextureWrap,
};
use slime_gfx::{Gpu, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{Camera, Light, MeshData, Renderer, Scene, SceneError};

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
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slime-render-cooked")
        .join(name);
    if directory.exists() {
        fs::remove_dir_all(&directory).expect("the old directory can be removed");
    }
    fs::create_dir_all(&directory).expect("the directory can be created");
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
    let mut manifest = Manifest::new();
    for (id, kind, file, bytes, needs) in [
        (
            TEXTURE,
            AssetKind::Texture,
            "texture.ktx2",
            texture.to_ktx2(),
            vec![],
        ),
        (
            MATERIAL,
            AssetKind::Material,
            "red.material",
            material.to_bytes(),
            vec![TEXTURE],
        ),
        (
            MESH,
            AssetKind::Mesh,
            "square.mesh",
            mesh.to_bytes(),
            vec![MATERIAL],
        ),
        (
            SCENE,
            AssetKind::Scene,
            "squares.scene",
            scene.to_bytes(),
            vec![MESH],
        ),
    ] {
        fs::write(directory.join(file), &bytes).expect("written");
        manifest.insert(
            id,
            ManifestEntry {
                kind,
                file: file.to_owned(),
                size: bytes.len() as u64,
                needs,
                source: "squares.gltf".to_owned(),
            },
        );
    }
    fs::write(directory.join(MANIFEST_FILE), manifest.to_bytes()).expect("written");
    directory
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
