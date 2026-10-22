//! The cooked data of the tests that load it: written into directories of their own, with the
//! formats of `slime-assets`; [`palette`], six squares of three materials and three textures, and
//! what draws one. Shared by `cooked.rs` and `requests.rs` through `#[path]`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use slime_assets::{
    AssetId, AssetKind, CookedMaterial, CookedMesh, CookedScene, CookedTexture, MANIFEST_FILE,
    Manifest, ManifestEntry, MaterialTexture, SceneNode, TextureEncoding, TextureFilter,
    TextureSampler, TextureWrap,
};
use slime_gfx::{Gpu, TextureFormat};
use slime_render::glam::{DVec3, Mat4, Vec3};
use slime_render::{
    AssetCache, Camera, DirectionalLight, Instance, Light, MeshData, Renderer, Scene, SceneError,
    ViewDraw,
};

/// A transform, column after column, that moves by `x` along +X and scales by `scale`.
pub fn placed(x: f32, scale: Vec3) -> [f32; 16] {
    Mat4::from_scale_rotation_translation(scale, Default::default(), Vec3::new(x, 0.0, 0.0))
        .to_cols_array()
}

/// An asset to write: its ID, kind, file name, bytes, and the IDs it needs.
pub type Written = (AssetId, AssetKind, String, Vec<u8>, Vec<AssetId>);

/// Writes `assets` into `directory`, and the manifest that lists them.
pub fn write(directory: &Path, assets: Vec<Written>) {
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
pub fn directory(name: &str) -> PathBuf {
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
pub const COLORS: [[u8; 4]; 3] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];

/// Writes into a directory of its own, named `name`, a cooked scene of six squares facing +Z, 2 m
/// apart along +X, each a mesh of its own placed by a node of its own. Square `i` is drawn with
/// material `i % 3`, which reads a texture of `COLORS[i % 3]`, `width` texels wide and one high.
/// Returns the directory and the scene's ID.
pub fn palette(name: &str, width: u32) -> (PathBuf, AssetId) {
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
pub fn center_of(gpu: &Gpu, renderer: &mut Renderer, instance: Instance) -> [u8; 3] {
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
    let mut scene = Scene::new(instances.clone());
    scene.lights = lights.to_vec();
    scene.exposure = 0.5;
    let mut view = renderer
        .create_view(gpu, "view")
        .expect("the view is created");
    renderer
        .render(
            gpu,
            &scene,
            &mut [ViewDraw::new(&mut view, camera, &target)],
        )
        .expect("the frame is drawn");
    let image = gpu.read_render_target(&target).expect("read back");
    let center = &image.pixels[(8 * 16 + 8) * 4..][..3];
    [center[0], center[1], center[2]]
}

/// Cooks the texture of [`palette`]'s second material, green, again white, under a new name, as
/// the cooker names a changed image; and removes the meshes and the other textures from the disk,
/// so that what reads them fails.
pub fn whiten_the_second_texture(directory: &Path) {
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
pub fn palette_meshes() -> [AssetId; 6] {
    std::array::from_fn(|index| AssetId::from_u128(300 + index as u128))
}

/// Polls `cache` until it neither loads nor reloads, and returns the meshes that failed, each with
/// why.
pub fn settle(cache: &mut AssetCache, gpu: &Gpu) -> Vec<(AssetId, Arc<SceneError>)> {
    let mut errors = Vec::new();
    for _ in 0..10_000 {
        errors.extend(cache.poll(gpu));
        if cache.loading() == 0 && !cache.reloading() {
            return errors;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("the cache still loads after 10 s");
}
