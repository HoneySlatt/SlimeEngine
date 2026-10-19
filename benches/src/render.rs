//! The renderer's benchmarks: a scene of many objects, each its own draw, and its pipelines made
//! again from their shaders.

use std::f32::consts::PI;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use slime_color::LinearColor;
use slime_gfx::TextureFormat;
use slime_render::glam::{DAffine3, DVec3, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Light, Material, MaterialData, MeshData, Renderer, Scene,
    ShaderSources, Sky,
};

use crate::measure::{Context, Row, ms, percentile, timed};

/// The size of the frames drawn.
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
/// The format of the frames drawn, an 8-bit sRGB one, as a window's.
pub const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;
/// Frames drawn before the times are taken: the first ones create the renderer's targets and its
/// shadow atlases.
const WARM_UP: usize = 3;
/// The sun's illuminance, in lux, and the exposure that shows a white surface facing it near white.
const SUN_ILLUMINANCE: f32 = 100_000.0;
pub const EXPOSURE: f32 = PI / SUN_ILLUMINANCE;

/// The examples' clear sky, and a sun high above it, which casts shadows over 50 m.
pub fn sky_and_sun() -> (Sky, DirectionalLight) {
    let sun = DirectionalLight {
        direction: Vec3::new(-0.5, -1.0, -0.6),
        color: LinearColor::rgb(1.0, 0.98, 0.9),
        illuminance: SUN_ILLUMINANCE,
        casts_shadows: true,
        shadow_distance: 50.0,
    };
    let sky = Sky {
        zenith: LinearColor::rgb(1700.0, 3900.0, 10_000.0),
        horizon: LinearColor::rgb(7600.0, 9600.0, 12_200.0),
        ground: LinearColor::rgb(3100.0, 2950.0, 2700.0),
        sun: Some(sun),
    };
    (sky, sun)
}

/// A cube of side 1 centred on its origin: four vertices for each of its six faces, so that each
/// face has its own normal.
fn cube() -> MeshData {
    let mut mesh = MeshData {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        tangents: Vec::new(),
        indices: Vec::new(),
    };
    for axis in 0..3 {
        for sign in [1.0_f32, -1.0] {
            let mut normal = [0.0; 3];
            normal[axis] = sign;
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let first = mesh.positions.len() as u32;
            for (a, b) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                let mut position = [0.0; 3];
                position[axis] = 0.5 * sign;
                position[u] = a * sign;
                position[v] = b;
                let mut tangent = [0.0, 0.0, 0.0, 1.0];
                tangent[u] = sign;
                mesh.positions.push(position);
                mesh.normals.push(normal);
                mesh.uvs.push([a + 0.5, 0.5 - b]);
                mesh.tangents.push(tangent);
            }
            mesh.indices
                .extend([0, 1, 2, 0, 2, 3].map(|corner| first + corner));
        }
    }
    mesh
}

/// `count` cubes on a square grid, 3 m apart, of eight materials, seen from above one side of the
/// grid, under the examples' sky and its sun: `frames` frames drawn into a target of 1280 × 720,
/// each waited for on the GPU before the next.
pub fn instances(context: &mut Context, count: usize, frames: usize) -> Result<Vec<Row>, String> {
    let gpu = &context.gpu;
    let error = |error: slime_render::RenderError| error.to_string();
    let mut renderer = Renderer::new(gpu, FORMAT).map_err(error)?;
    let mesh = renderer.create_mesh(gpu, "cube", &cube()).map_err(error)?;
    let materials = (0..8)
        .map(|index| {
            let shade = 0.2 + 0.1 * index as f32;
            let data = MaterialData {
                base_color: LinearColor::rgb(shade, 0.5, 1.0 - shade),
                roughness: 0.6,
                ..MaterialData::default()
            };
            renderer.create_material(gpu, &format!("cube {index}"), &data)
        })
        .collect::<Result<Vec<Material>, _>>()
        .map_err(error)?;
    let (sky, sun) = sky_and_sun();
    let environment = renderer.create_environment(gpu, &sky).map_err(error)?;
    let target = gpu
        .create_render_target("benchmark frame", WIDTH, HEIGHT, FORMAT)
        .map_err(|error| error.to_string())?;

    let side = (count as f64).sqrt().ceil() as usize;
    let spacing = 3.0;
    let half = side as f64 * spacing / 2.0;
    let instances: Vec<Instance<'_>> = (0..count)
        .map(|index| {
            let (x, z) = ((index % side) as f64, (index / side) as f64);
            let at = DVec3::new(x * spacing - half, 0.5, z * spacing - half);
            Instance::new(&mesh, &materials[index % 8], DAffine3::from_translation(at))
        })
        .collect();
    let camera = Camera {
        eye: DVec3::new(0.0, half * 0.6, half * 1.4),
        target: DVec3::ZERO,
        ..Camera::default()
    };
    let lights = [Light::Directional(sun)];
    let mut scene = Scene::new(camera, &instances);
    scene.lights = &lights;
    scene.environment = Some(&environment);
    scene.exposure = EXPOSURE;
    scene.bloom = 0.04;

    let (mut cpu, mut whole, mut scene_gpu) = (Vec::new(), Vec::new(), Vec::new());
    let mut last = None;
    for frame in 0..WARM_UP + frames {
        let (stats, duration) = timed(|| -> Result<_, String> {
            let stats = renderer.render(gpu, &target, &scene).map_err(error)?;
            gpu.wait_idle().map_err(|error| error.to_string())?;
            Ok(stats)
        });
        let stats = stats?;
        if frame >= WARM_UP {
            cpu.push(stats.cpu_time);
            whole.push(duration);
            scene_gpu.extend(stats.scene_gpu_time);
            last = Some(stats);
        }
    }
    let stats = last.expect("at least one frame is measured");
    let mut rows = Vec::new();
    rows.extend(Row::times("CPU of `Renderer::render`", &cpu));
    rows.extend(Row::times("frame, until the GPU finished it", &whole));
    // Each frame reports the latest time that came back from the GPU, a few frames old.
    if !scene_gpu.is_empty() {
        rows.push(Row::new(
            format!(
                "scene pass on the GPU, median of what {} frames reported",
                scene_gpu.len()
            ),
            ms(percentile(&scene_gpu, 0.5)),
        ));
    }
    rows.push(Row::new(
        "draw calls: scene, shadow cascades",
        format!("{}, {}", stats.draw_calls, stats.shadow_draw_calls),
    ));
    Ok(rows)
}

/// The renderer's pipelines made again from its embedded shaders, unchanged, five times, then five
/// times after an edit of the scene's shader that adds a function, which changes the code the
/// driver compiles: an edit that no run has made before, so that the driver's cache of compiled
/// shaders does not hold it, as after a change that a developer saves.
pub fn shaders(context: &mut Context) -> Result<Vec<Row>, String> {
    let gpu = &context.gpu;
    let error = |error: slime_render::RenderError| error.to_string();
    let mut renderer = Renderer::new(gpu, FORMAT).map_err(error)?;
    let embedded = ShaderSources::embedded();
    let mut unchanged = Vec::new();
    for _ in 0..5 {
        let (made, duration) = timed(|| renderer.set_shaders(gpu, &embedded));
        made.map_err(error)?;
        unchanged.push(duration);
    }
    let run = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let mut edited = Vec::new();
    for edit in 0..5 {
        let mut sources = embedded.clone();
        sources.scene.push_str(&format!(
            "\nfn benchmark_edit_{run}_{edit}() -> f32 {{\n    return {edit}.0;\n}}\n"
        ));
        let (made, duration) = timed(|| renderer.set_shaders(gpu, &sources));
        made.map_err(error)?;
        edited.push(duration);
    }
    Ok(vec![
        Row::new(
            "pipelines made again, shaders unchanged, median of 5",
            ms(percentile(&unchanged, 0.5)),
        ),
        Row::new(
            "pipelines made again after an edit of the scene's shader, median of 5",
            ms(percentile(&edited, 0.5)),
        ),
    ])
}
