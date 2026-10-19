//! The benchmarks of loading: a load started with a large manifest, and the meshes of *Hunna
//! Wood*'s zone loaded while frames are drawn.

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use slime_assets::{AssetId, AssetKind, CookedAssets, MANIFEST_FILE, Manifest, ManifestEntry};
use slime_render::glam::DVec3;
use slime_render::{AssetCache, Camera, Renderer, Sky};
use slime_world::{Meshes, Transform, World};
use slime_world_render::FrameBuilder;

use crate::measure::{Context, Row, meshes_of, ms, percentile, timed};
use crate::render::{EXPOSURE, FORMAT, sky_and_sun};

/// How far apart the zone's meshes are placed, in metres: more than the largest of them.
const SPACING: f64 = 30.0;

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
        let mut cache = AssetCache::new(large_manifest(count)?);
        let mut times = Vec::new();
        for mesh in 1..=5 {
            let ((), duration) = timed(|| {
                cache.request(&renderer, &context.gpu, [AssetId::from_u128(mesh)]);
            });
            times.push(duration);
        }
        // The loads fail, their files missing: their threads end before the next manifest.
        while cache.loading() > 0 {
            let _ = cache.poll();
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
/// a grid, then drawn by day at 1280 × 720 from above while they load with their materials and
/// textures, each frame waited for on the GPU, until a frame draws them whole; then five frames
/// more. Not the zone's scene, whose components of the game's only the game registers: the zone's
/// bytes, laid out otherwise. It runs only when asked for, since the zone is fetched and cooked
/// first.
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
    let gpu = &context.gpu;
    let start = Instant::now();
    let (assets, opened) = timed(|| CookedAssets::open(&directory));
    let assets = assets.map_err(|error| error.to_string())?;
    let meshes = meshes_of(&assets);
    let side = (meshes.len() as f64).sqrt().ceil().max(1.0) as usize;
    let mut world = World::new();
    for (index, &mesh) in meshes.iter().enumerate() {
        let entity = world.spawn();
        let (x, z) = ((index % side) as f64, (index / side) as f64);
        world.insert(
            entity,
            Transform::from_translation(DVec3::new(x * SPACING, 0.0, z * SPACING)),
        );
        world.insert(
            entity,
            Meshes {
                ids: vec![mesh],
                casts_shadows: true,
            },
        );
    }
    world.update_transforms();
    let middle = (side as f64 - 1.0) * SPACING / 2.0;
    let mut renderer = Renderer::new(gpu, FORMAT).map_err(|error| error.to_string())?;
    let target = gpu
        .create_render_target("benchmark frame", 1280, 720, FORMAT)
        .map_err(|error| error.to_string())?;
    let mut cache = AssetCache::new(assets);
    let (sky, _) = sky_and_sun();
    let mut builder = FrameBuilder::new(Some(Sky { sun: None, ..sky }));
    let camera = Camera {
        eye: DVec3::new(middle, middle + 20.0, middle * 2.0 + 40.0),
        target: DVec3::new(middle, 0.0, middle),
        ..Camera::default()
    };
    let (mut loading, mut after) = (Vec::new(), Vec::new());
    let mut first_whole = None;
    while after.len() < 5 {
        let frame = builder
            .build(&world, &mut cache, &renderer, gpu, 0.0)
            .map_err(|error| error.to_string())?;
        if let Some(error) = frame.errors.first() {
            return Err(error.to_string());
        }
        let whole = frame.loading == 0;
        let mut scene = frame.scene(camera);
        scene.exposure = EXPOSURE;
        scene.bloom = 0.04;
        let stats = renderer
            .render(gpu, &target, &scene)
            .map_err(|error| error.to_string())?;
        gpu.wait_idle().map_err(|error| error.to_string())?;
        if whole {
            first_whole.get_or_insert(start.elapsed());
            after.push(stats.cpu_time);
        } else {
            loading.push(stats.cpu_time);
        }
    }
    let mut rows = vec![
        Row::new("cooked zone opened", ms(opened)),
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
    }
    rows.extend(Row::times(
        "CPU of `Renderer::render` once they are loaded",
        &after,
    ));
    Ok(rows)
}
