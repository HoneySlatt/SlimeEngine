//! The benchmarks of loading: a load started with a large manifest, and meshes loaded while frames
//! are drawn: *Hunna Wood*'s zone, and squares textured with textures of 4096².

use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use slime_assets::{
    AssetId, AssetKind, CookedAssets, CookedMaterial, CookedMesh, CookedTexture, MANIFEST_FILE,
    Manifest, ManifestEntry, MaterialTexture, TextureEncoding, TextureFilter, TextureSampler,
    TextureWrap,
};
use slime_render::glam::DAffine3;
use slime_render::glam::DVec3;
use slime_render::{
    AssetCache, Camera, DirectionalLight, Instance, Light, MaterialData, MeshData, Priority,
    Renderer, Scene, Sky, View, ViewDraw,
};
use slime_world::{Meshes, Transform, World};
use slime_world_render::FrameBuilder;

use crate::measure::{Context, LOAD_LIMIT, Row, meshes_of, ms, percentile, timed};
use crate::render::{EXPOSURE, FORMAT, sky_and_sun};
use crate::world::spawn_sun;

/// How far apart the zone's meshes are placed, in metres: more than the largest of them.
const ZONE_SPACING: f64 = 30.0;

/// The directory of a manifest of `count` meshes, none of whose files exist: a load started from
/// it fails in its thread, after the work on the calling thread that the benchmark times.
fn large_manifest(count: u128) -> Result<CookedAssets, String> {
    let directory = Context::root().join(format!("target/benches/manifest-{count}"));
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let mut manifest = Manifest::new();
    for index in 1..=count {
        manifest.insert(
            AssetId::from_u128(index),
            ManifestEntry {
                kind: AssetKind::Mesh,
                file: format!("{index}.mesh"),
                size: 160,
                needs: Vec::new(),
                source: "benchmark.gltf".to_owned(),
            },
        );
    }
    fs::write(directory.join(MANIFEST_FILE), manifest.to_bytes())
        .map_err(|error| error.to_string())?;
    CookedAssets::open(&directory).map_err(|error| error.to_string())
}

/// `AssetCache::request` for one mesh, from cooked data whose manifest lists 10 000, then 100 000
/// meshes: five loads each, each of another mesh, the time on the calling thread.
pub fn batch(context: &mut Context) -> Result<Vec<Row>, String> {
    let renderer = Renderer::new(&context.gpu, FORMAT).map_err(|error| error.to_string())?;
    let mut rows = Vec::new();
    for count in [10_000, 100_000] {
        let mut cache = AssetCache::new(&renderer, large_manifest(count)?);
        let mut times = Vec::new();
        let mut requests = Vec::new();
        for mesh in 1..=5 {
            let (request, duration) =
                timed(|| cache.request(&context.gpu, [AssetId::from_u128(mesh)], Priority::Now));
            requests.push(request);
            times.push(duration);
        }
        // The loads fail, their files missing: their threads end before the next manifest.
        while cache.loading() > 0 {
            let _ = cache.poll(&context.gpu);
            thread::sleep(Duration::from_millis(1));
        }
        rows.push(Row::new(
            format!("a load started, the manifest of {count} assets, median of 5"),
            ms(percentile(&times, 0.5)),
        ));
    }
    Ok(rows)
}

/// *Hunna Wood*'s zone, cooked in `target/cooked/hunnawood`: each of its meshes placed once, on
/// a grid, then drawn as [`drawn_while_loading`] says. Not the zone's scene, whose components of
/// the game's only the game registers: the zone's bytes, laid out otherwise. It runs only when
/// asked for, since the zone is fetched and cooked first.
pub fn zone(context: &mut Context) -> Result<Vec<Row>, String> {
    let directory = Context::root().join("target/cooked/hunnawood");
    if !directory.join(MANIFEST_FILE).is_file() {
        return Err(format!(
            "{} holds no cooked zone: fetch it with `scripts/fetch-assets.sh zone/`, then cook it \
             with `cargo run --release -p slime-cook -- cook games/hunnawood/assets/zone \
             target/cooked/hunnawood`",
            directory.display()
        ));
    }
    drawn_while_loading(context, &directory, ZONE_SPACING, "cooked zone opened")
}

/// How many meshes [`textures`] loads, each with a material and a texture of its own.
const TEXTURED_MESHES: u128 = 16;
/// The side of their textures, in texels: the largest that a cooked texture has.
const TEXTURE_SIDE: u32 = 4096;

/// [`TEXTURED_MESHES`] squares, each drawn with a material of its own whose base color is a
/// texture of [`TEXTURE_SIDE`] × [`TEXTURE_SIDE`] texels in BC7, every level of detail with it:
/// 21.3 MiB each, 341 MiB in all, loaded and drawn as [`drawn_while_loading`] says. Written once,
/// into `target/benches/textures`, without the cooker, whose BC7 encoder would take seconds per
/// texture: their blocks are a pattern, which the GPU decodes as any other.
pub fn textures(context: &mut Context) -> Result<Vec<Row>, String> {
    let directory = Context::root().join("target/benches/textures");
    if !directory.join(MANIFEST_FILE).is_file() {
        write_textured_squares(&directory)?;
    }
    drawn_while_loading(context, &directory, 2.0, "cooked data opened")
}

/// Writes the cooked data of [`textures`] into `directory`.
fn write_textured_squares(directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let mut manifest = Manifest::new();
    let mut write = |id: u128, kind, file: String, bytes: Vec<u8>, needs| {
        fs::write(directory.join(&file), &bytes).map_err(|error| error.to_string())?;
        manifest.insert(
            AssetId::from_u128(id),
            ManifestEntry {
                kind,
                file,
                size: bytes.len() as u64,
                needs,
                source: "benchmark.gltf".to_owned(),
            },
        );
        Ok::<(), String>(())
    };
    let square = MeshData::square();
    for index in 0..TEXTURED_MESHES {
        let (mesh, material, texture) = (1000 + index, 2000 + index, 3000 + index);
        let levels = (0..)
            .map(|level| TEXTURE_SIDE >> level)
            .take_while(|&side| side > 0)
            .enumerate()
            .map(|(level, side)| {
                let size = TextureEncoding::Bc7Srgb.level_size(side, side);
                // Each texture and each level its own pattern of bytes.
                (0..size)
                    .map(|byte| (byte as u128 * 31 + index * 7 + level as u128) as u8)
                    .collect()
            })
            .collect();
        let cooked = CookedTexture {
            width: TEXTURE_SIDE,
            height: TEXTURE_SIDE,
            encoding: TextureEncoding::Bc7Srgb,
            levels,
        };
        write(
            texture,
            AssetKind::Texture,
            format!("{index}.ktx2"),
            cooked.to_ktx2(),
            Vec::new(),
        )?;
        let slot = MaterialTexture {
            texture: AssetId::from_u128(texture),
            sampler: TextureSampler {
                mag_filter: TextureFilter::Linear,
                min_filter: TextureFilter::Linear,
                wrap_u: TextureWrap::Repeat,
                wrap_v: TextureWrap::Repeat,
            },
        };
        let cooked = CookedMaterial {
            name: format!("textured {index}"),
            base_color_texture: Some(slot),
            metallic: 0.0,
            ..CookedMaterial::default()
        };
        write(
            material,
            AssetKind::Material,
            format!("{index}.material"),
            cooked.to_bytes(),
            vec![AssetId::from_u128(texture)],
        )?;
        let cooked = CookedMesh {
            name: format!("textured {index}"),
            material: AssetId::from_u128(material),
            positions: square.positions.clone(),
            normals: square.normals.clone(),
            uvs: square.uvs.clone(),
            tangents: square.tangents.clone(),
            indices: square.indices.clone(),
            bounds: [[-0.5, -0.5, 0.0], [0.5, 0.5, 0.0]],
        };
        write(
            mesh,
            AssetKind::Mesh,
            format!("{index}.mesh"),
            cooked.to_bytes(),
            vec![AssetId::from_u128(material)],
        )?;
    }
    // The manifest last: a run stopped while writing leaves none, and the next writes again.
    fs::write(directory.join(MANIFEST_FILE), manifest.to_bytes()).map_err(|error| error.to_string())
}

/// The meshes of the cooked data in `directory`, each placed once on a grid, `spacing` metres
/// apart, then drawn by day, under a sun that casts shadows, at 1280 × 720 from above while they
/// load with their materials and textures, each frame waited for on the GPU, until a frame draws
/// them whole; then five frames more. The time to that frame runs from the opening of the cooked
/// data, once the renderer is made; `opened` names that opening's row.
fn drawn_while_loading(
    context: &mut Context,
    directory: &Path,
    spacing: f64,
    opened: &str,
) -> Result<Vec<Row>, String> {
    let gpu = &context.gpu;
    let mut renderer = Renderer::new(gpu, FORMAT).map_err(|error| error.to_string())?;
    let mut view = renderer
        .create_view(gpu, "benchmark")
        .map_err(|error| error.to_string())?;
    let target = gpu
        .create_render_target("benchmark frame", 1280, 720, FORMAT)
        .map_err(|error| error.to_string())?;
    let (sky, sun) = sky_and_sun();
    let mut builder = FrameBuilder::new(Some(Sky { sun: None, ..sky }));
    warm_up(
        gpu,
        &mut renderer,
        &mut view,
        &mut builder,
        &target,
        sun,
        directory,
    )?;
    let start = Instant::now();
    let (assets, opening) = timed(|| CookedAssets::open(directory));
    let assets = assets.map_err(|error| error.to_string())?;
    let meshes = meshes_of(&assets);
    let side = (meshes.len() as f64).sqrt().ceil().max(1.0) as usize;
    let mut world = World::new();
    for (index, &mesh) in meshes.iter().enumerate() {
        let entity = world.spawn();
        let (x, z) = ((index % side) as f64, (index / side) as f64);
        world.insert(
            entity,
            Transform::from_translation(DVec3::new(x * spacing, 0.0, z * spacing)),
        );
        world.insert(
            entity,
            Meshes {
                ids: vec![mesh],
                casts_shadows: true,
            },
        );
    }
    spawn_sun(&mut world);
    world.update_transforms();
    let middle = (side as f64 - 1.0) * spacing / 2.0;
    let mut cache = AssetCache::new(&renderer, assets);
    let camera = Camera {
        eye: DVec3::new(middle, middle + 20.0, middle * 2.0 + 40.0),
        target: DVec3::new(middle, 0.0, middle),
        ..Camera::default()
    };
    let (mut loading, mut after, mut frames) = (Vec::new(), Vec::new(), Vec::new());
    let mut first_whole = None;
    while after.len() < 5 {
        let frame_start = Instant::now();
        let frame = builder
            .build(&world, &mut cache, gpu, 0.0)
            .map_err(|error| error.to_string())?;
        if let Some((_, error)) = frame.errors.first() {
            return Err(error.to_string());
        }
        let whole = frame.loading == 0;
        let mut scene = frame.scene(camera.eye);
        scene.exposure = EXPOSURE;
        scene.bloom = 0.04;
        renderer
            .render(
                gpu,
                &scene,
                &mut [ViewDraw::new(&mut view, camera, &target)],
            )
            .map_err(|error| error.to_string())?;
        let stats = view.stats();
        gpu.wait_idle().map_err(|error| error.to_string())?;
        if whole {
            first_whole.get_or_insert(start.elapsed());
            after.push(stats.cpu_time);
        } else if start.elapsed() > LOAD_LIMIT {
            return Err(format!("the meshes did not load in {LOAD_LIMIT:?}"));
        } else {
            loading.push(stats.cpu_time);
            frames.push(frame_start.elapsed());
        }
    }
    let mut rows = vec![
        Row::new(opened, ms(opening)),
        Row::new("meshes placed", meshes.len().to_string()),
        Row::new(
            "first frame that draws them whole, from the opening",
            ms(first_whole.expect("frames were drawn whole")),
        ),
    ];
    if !loading.is_empty() {
        rows.extend(Row::times(
            "CPU of `Renderer::render` while they load",
            &loading,
        ));
        rows.push(Row::new(
            format!(
                "frame while they load, built, drawn and waited for on the GPU, longest of {}",
                frames.len()
            ),
            ms(frames.iter().copied().max().unwrap_or_default()),
        ));
    }
    rows.extend(Row::times(
        "CPU of `Renderer::render` once they are loaded",
        &after,
    ));
    Ok(rows)
}

/// Gives the frame its shape before the load starts, so that what a load costs the frames is not
/// mixed with what a first frame costs: draws a cube that casts a shadow under `sun`, which makes
/// `view`'s targets and the renderer's cascades' atlas, and builds a frame of a world with the sun
/// alone, which makes the sky's environment, with an asset cache of the cooked data in `directory`
/// that it asks nothing of.
fn warm_up(
    gpu: &slime_gfx::Gpu,
    renderer: &mut Renderer,
    view: &mut View,
    builder: &mut FrameBuilder,
    target: &slime_gfx::RenderTarget,
    sun: DirectionalLight,
    directory: &Path,
) -> Result<(), String> {
    let error = |error: slime_render::RenderError| error.to_string();
    let cube = renderer
        .create_mesh(gpu, "warm-up cube", &MeshData::cube())
        .map_err(error)?;
    let white = renderer
        .create_material(gpu, "warm-up", &MaterialData::default())
        .map_err(error)?;
    let instances = [Instance::new(&cube, &white, DAffine3::IDENTITY)];
    let lights = [Light::Directional(sun)];
    let mut scene = Scene::new(instances.clone());
    scene.lights = lights.to_vec();
    let camera = Camera::default();
    renderer
        .render(gpu, &scene, &mut [ViewDraw::new(view, camera, target)])
        .map_err(error)?;
    let mut world = World::new();
    spawn_sun(&mut world);
    world.update_transforms();
    let mut cache = AssetCache::new(
        renderer,
        CookedAssets::open(directory).map_err(|error| error.to_string())?,
    );
    let frame = builder
        .build(&world, &mut cache, gpu, 0.0)
        .map_err(|error| error.to_string())?;
    renderer
        .render(
            gpu,
            &frame.scene(camera.eye),
            &mut [ViewDraw::new(view, camera, target)],
        )
        .map_err(error)?;
    gpu.wait_idle().map_err(|error| error.to_string())
}
