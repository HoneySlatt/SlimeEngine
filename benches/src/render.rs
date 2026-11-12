//! The renderer's benchmarks: a scene of many objects, drawn at the hardware tiers' resolutions, and
//! its pipelines made again from their shaders.

use std::f32::consts::PI;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use slime_app::{InterfaceImage, InterfaceRenderer};
use slime_color::LinearColor;
use slime_gfx::{GfxError, Gpu, RenderTarget, Texture, TextureFormat};
use slime_render::glam::{DAffine3, DVec3, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, Instance, Light, Material, MaterialData, MeshData,
    RenderStats, Renderer, Scene, ShaderSources, Sky, ViewDraw,
};
use uwui_render::canvas::{
    Canvas, CanvasBox, CanvasShadow, CanvasStats, ImageId, ImageKind, ImagePart, ImageQuad,
    PixelRect,
};

use crate::measure::{Context, Row, ms, percentile, timed};

/// The sizes of the frames drawn: the screens of the hardware tiers of ADR-0114, handheld, modest
/// and high, whose budgets `docs/measurements.md` compares the figures with.
pub const TIERS: [(u32, u32); 3] = [(1280, 800), (1920, 1080), (3840, 2160)];
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

/// Where the camera of [`instances`] looks.
#[derive(Clone, Copy)]
pub enum View {
    /// At the grid, from above one side of it: every cube is drawn.
    Grid,
    /// Up at the sky, from high above the grid's middle: no cube is in view nor casts a shadow
    /// into it, so that the frame's time is mostly that of the commands encoded and submitted.
    Sky,
}

/// `count` cubes on a square grid, 3 m apart, of eight materials, in turn, seen as `view` says,
/// under the examples' sky and its sun: `frames` frames drawn into a target of each size of
/// [`TIERS`], each waited for on the GPU before the next.
pub fn instances(
    context: &mut Context,
    count: usize,
    frames: usize,
    view: View,
) -> Result<Vec<Row>, String> {
    let frames = context.runs(frames);
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
    let environment = Environment::new(gpu, &sky).map_err(error)?;

    let side = (count as f64).sqrt().ceil() as usize;
    let spacing = 3.0;
    let half = side as f64 * spacing / 2.0;
    let instances: Vec<Instance> = (0..count)
        .map(|index| {
            let (x, z) = ((index % side) as f64, (index / side) as f64);
            let at = DVec3::new(x * spacing - half, 0.5, z * spacing - half);
            Instance::new(&mesh, &materials[index % 8], DAffine3::from_translation(at))
        })
        .collect();
    let camera = match view {
        View::Grid => Camera {
            eye: DVec3::new(0.0, half * 0.6, half * 1.4),
            target: DVec3::ZERO,
            ..Camera::default()
        },
        View::Sky => Camera {
            eye: DVec3::new(0.0, 100.0, 0.0),
            target: DVec3::new(0.0, 200.0, 1.0),
            up: Vec3::Z,
            ..Camera::default()
        },
    };
    let lights = [Light::Directional(sun)];
    let mut scene = Scene::new(instances.clone());
    scene.lights = lights.to_vec();
    scene.environment = Some(environment.clone());
    scene.exposure = EXPOSURE;
    scene.bloom = 0.04;

    let mut rows = Vec::new();
    for (width, height) in TIERS {
        let target = gpu
            .create_render_target("benchmark frame", width, height, FORMAT)
            .map_err(|error| error.to_string())?;
        let at = format!("at {width} × {height}");
        // A view for each size: one view drawn at another size would draw into its first targets
        // until the new size held still (ADR-0127), stretched.
        let mut render_view = renderer.create_view(gpu, "benchmark").map_err(error)?;
        let (mut cpu, mut whole) = (Vec::new(), Vec::new());
        let (mut scene_gpu, mut passes_gpu) = (Vec::new(), Vec::new());
        let mut last = None;
        for frame in 0..WARM_UP + frames {
            let (stats, duration) = timed(|| -> Result<_, String> {
                let mut draws = [ViewDraw::new(&mut render_view, camera, &target)];
                renderer.render(gpu, &scene, &mut draws).map_err(error)?;
                gpu.wait_idle().map_err(|error| error.to_string())?;
                Ok(render_view.stats())
            });
            let stats = stats?;
            if frame >= WARM_UP {
                cpu.push(stats.cpu_time);
                whole.push(duration);
                scene_gpu.extend(stats.scene_gpu_time);
                passes_gpu.extend(passes_gpu_time(&stats));
                last = Some(stats);
            }
        }
        let stats = last.expect("at least one frame is measured");
        assert_eq!(
            render_view.render_size(),
            Some((width, height)),
            "the frames are drawn at the size of the target"
        );
        rows.extend(Row::times(&format!("CPU of `Renderer::render` {at}"), &cpu));
        rows.extend(Row::times(
            &format!("frame, until the GPU finished it, {at}"),
            &whole,
        ));
        // Each frame reports the latest times that came back from the GPU, a few frames old.
        for (what, times) in [
            ("scene pass on the GPU", &scene_gpu),
            ("passes on the GPU, summed", &passes_gpu),
        ] {
            if !times.is_empty() {
                rows.push(Row::new(
                    format!(
                        "{what} {at}, median of what {} frames reported",
                        times.len()
                    ),
                    ms(percentile(times, 0.5)),
                ));
            }
        }
        rows.push(Row::count(
            format!("draw calls of the scene {at}"),
            stats.draw_calls,
        ));
        rows.push(Row::count(
            format!("draw calls of the shadow cascades {at}"),
            stats.shadow_draw_calls,
        ));
        rows.push(Row::count(
            format!("instances that the scene pass drew {at}"),
            stats.instances,
        ));
    }
    Ok(rows)
}

/// The sum of the GPU times of the passes of a frame, which a budget of GPU time is compared with:
/// on Vulkan, each time runs from the end of the work before the pass, so that the sum is how long
/// the GPU worked on the frame, without the gaps between its submissions (`GpuTimer`). `None`
/// until each pass that ran has a time; a shadow pass that drew nothing did not run.
fn passes_gpu_time(stats: &RenderStats) -> Option<Duration> {
    let mut sum = Duration::ZERO;
    for (time, ran) in [
        (stats.shadow_gpu_time, stats.shadow_draw_calls > 0),
        (stats.spot_shadow_gpu_time, stats.spot_shadow_draw_calls > 0),
    ] {
        if ran {
            sum += time?;
        }
    }
    for time in [
        stats.scene_gpu_time,
        stats.sky_gpu_time,
        stats.resolve_gpu_time,
        stats.bloom_gpu_time,
        stats.tone_mapping_gpu_time,
    ] {
        sum += time?;
    }
    Some(sum)
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

/// Two screens drawn by the canvas (`docs/adr/0137-the-canvas.md`) at the tiers' sizes, each laid
/// out for 1920 × 1080: an editor's ([`editor_screen`]) and a game's menu ([`game_screen`]). At
/// each, the CPU of `InterfaceRenderer::draw_canvas`, the canvas on the GPU, alone and what it adds
/// to a frame over a view ([`ViewUnder`]), which ADR-0137 bounds as ADR-0245 counts it, and its
/// items, draws and passes.
pub fn canvas(context: &mut Context) -> Result<Vec<Row>, String> {
    let runs = context.runs(CANVAS_FRAMES);
    let gpu = &context.gpu;
    let gfx = |error: GfxError| error.to_string();
    let mut renderer = InterfaceRenderer::new(gpu, FORMAT).map_err(gfx)?;
    let mut view = ViewUnder::new(gpu)?;
    // An atlas of glyphs, 1 024 × 1 024 texels, in cells of 16 × 16 that a glyph covers in part.
    let glyphs: Vec<u8> = (0..1024 * 1024)
        .map(|index| {
            let (x, y) = (index % 16, (index / 1024) % 16);
            if (3..13).contains(&x) && (2..14).contains(&y) {
                255
            } else {
                0
            }
        })
        .collect();
    let glyphs = gpu
        .create_texture_with_levels("glyphs", 1024, 1024, TextureFormat::R8Unorm, &[&glyphs])
        .map_err(gfx)?;
    // An atlas of 64 icons of 64 × 64 texels, in colors.
    let icons: Vec<u8> = (0..512 * 512)
        .flat_map(|texel| {
            let (x, y) = (texel % 512, texel / 512);
            [
                (x % 64 * 4) as u8,
                (y % 64 * 4) as u8,
                ((x / 64 + y / 64 * 8) * 4) as u8,
                255,
            ]
        })
        .collect();
    let icons = gpu
        .create_texture("icons", 512, 512, TextureFormat::Rgba8UnormSrgb, &icons)
        .map_err(gfx)?;
    let pictures: Vec<_> = (0..16)
        .map(|index| {
            let pixels: Vec<u8> = (0..256 * 256)
                .flat_map(|texel| [(texel % 256) as u8, index * 16, (texel / 256) as u8, 255])
                .collect();
            gpu.create_texture("picture", 256, 256, TextureFormat::Rgba8UnormSrgb, &pixels)
        })
        .collect::<Result<_, _>>()
        .map_err(gfx)?;
    let mut rows = Vec::new();
    for (width, height) in TIERS {
        let target = gpu
            .create_render_target("benchmark frame", width, height, FORMAT)
            .map_err(gfx)?;
        let (w, h) = (width as f32, height as f32);
        let at = format!("at {width} × {height}");
        let mut editor = Canvas::new();
        let images = editor_screen(&mut editor, &glyphs, &pictures, w, h);
        rows.extend(measure_canvas(
            gpu,
            &mut renderer,
            &mut view,
            (&editor, &images),
            &target,
            &format!("the editor's screen {at}"),
            runs,
        )?);
        let mut game = Canvas::new();
        let images = game_screen(&mut game, &glyphs, &icons, w, h);
        rows.extend(measure_canvas(
            gpu,
            &mut renderer,
            &mut view,
            (&game, &images),
            &target,
            &format!("the game's menu {at}"),
            runs,
        )?);
    }
    Ok(rows)
}

/// The rows of a screen, a canvas drawn with its images, into `target`, `what` naming it and its
/// size: the CPU of `InterfaceRenderer::draw_canvas`, the canvas on the GPU, alone and over `view`,
/// and its items, draws and passes.
fn measure_canvas(
    gpu: &Gpu,
    renderer: &mut InterfaceRenderer,
    view: &mut ViewUnder,
    (canvas, images): (&Canvas, &[InterfaceImage<'_>]),
    target: &RenderTarget,
    what: &str,
    runs: usize,
) -> Result<Vec<Row>, String> {
    let (cpu, gpu_times, stats) = canvas_frames(
        gpu,
        runs,
        || renderer.draw_canvas(gpu, canvas, images, target),
        |stats| stats.gpu_time,
    )?;
    let mut rows = Vec::from(Row::times(
        &format!("CPU of `InterfaceRenderer::draw_canvas`, {what}"),
        &cpu,
    ));
    if !gpu_times.is_empty() {
        rows.push(Row::new(
            format!(
                "canvas on the GPU, {what}, median of what {} frames reported",
                gpu_times.len()
            ),
            ms(percentile(&gpu_times, 0.5)),
        ));
    }
    rows.push(Row::count(format!("items, {what}"), stats.items));
    rows.push(Row::count(format!("draws, {what}"), stats.draws));
    rows.extend(view.measure(gpu, target, runs, what, || {
        renderer
            .draw_canvas(gpu, canvas, images, target)
            .map_err(|error| error.to_string())
    })?);
    Ok(rows)
}

/// A view that the renderer draws under an interface, as a game's menu lies over its world: the
/// examples' sky and its sun, seen at the horizon, its bloom and its tone mapping, no object
/// (`docs/adr/0245-the-interfaces-gpu-budget-counts-what-it-adds-to-the-frame.md`).
pub struct ViewUnder {
    renderer: Renderer,
    scene: Scene,
}

/// The frames drawn of each kind, with the interface and without it, before one is measured: a
/// frame's times come back once the GPU has finished it, at the next frame, so that the frame
/// measured reports one of its own kind.
const FRAMES_OF_A_KIND: usize = 3;

impl ViewUnder {
    /// The renderer and the scene of the view.
    pub fn new(gpu: &Gpu) -> Result<Self, String> {
        let error = |error: slime_render::RenderError| error.to_string();
        let renderer = Renderer::new(gpu, FORMAT).map_err(error)?;
        let (sky, sun) = sky_and_sun();
        let mut scene = Scene::new(Vec::new());
        scene.lights = vec![Light::Directional(sun)];
        scene.environment = Some(Environment::new(gpu, &sky).map_err(error)?);
        scene.exposure = EXPOSURE;
        scene.bloom = 0.04;
        Ok(Self { renderer, scene })
    }

    /// What `interface`, which draws over `target`, adds to a frame of the view on the GPU, `what`
    /// naming it and its size: frames with the interface and without it, in blocks of
    /// [`FRAMES_OF_A_KIND`] interleaved, `runs` of each, the order of each pair alternating; each
    /// frame's GPU time the sum of its passes', which on Vulkan count the wait at each boundary.
    /// Rows: the median of what the interface added in each pair, the median of its own passes'
    /// time within the frame, and how many passes it took, a counter. None but the counter where
    /// the device does not time passes.
    pub fn measure(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        runs: usize,
        what: &str,
        mut interface: impl FnMut() -> Result<CanvasStats, String>,
    ) -> Result<Vec<Row>, String> {
        let error = |error: slime_render::RenderError| error.to_string();
        // A view for each size: one view drawn at another size would draw into its first targets
        // until the new size held still (ADR-0127), stretched.
        let mut view = self
            .renderer
            .create_view(gpu, "benchmark under an interface")
            .map_err(error)?;
        let camera = Camera {
            eye: DVec3::new(0.0, 2.0, 0.0),
            target: DVec3::new(0.0, 2.5, 10.0),
            ..Camera::default()
        };
        let (mut added, mut own) = (Vec::new(), Vec::new());
        let mut passes = 0;
        for run in 0..runs {
            let mut frame = [None; 2];
            // With the interface first in even runs, last in odd ones.
            for with in [run % 2 == 0, run % 2 == 1] {
                let mut interface_time = None;
                for _ in 0..FRAMES_OF_A_KIND {
                    let mut draws = [ViewDraw::new(&mut view, camera, target)];
                    self.renderer
                        .render(gpu, &self.scene, &mut draws)
                        .map_err(error)?;
                    if with {
                        let stats = interface()?;
                        passes = stats.passes;
                        interface_time = stats.gpu_time;
                    }
                    gpu.wait_idle().map_err(|error| error.to_string())?;
                }
                let view_time = passes_gpu_time(&view.stats());
                frame[usize::from(with)] = if with {
                    view_time.zip(interface_time).map(|(view, ui)| {
                        own.push(ui);
                        view + ui
                    })
                } else {
                    view_time
                };
            }
            if let [Some(without), Some(with)] = frame {
                added.push(with.as_secs_f64() - without.as_secs_f64());
            }
        }
        let mut rows = Vec::new();
        if !added.is_empty() {
            added.sort_unstable_by(f64::total_cmp);
            let median = added[added.len() / 2];
            rows.push(Row::new(
                format!(
                    "what the interface adds to a frame over a view, on the GPU, {what}, median \
                     of {} pairs",
                    added.len()
                ),
                signed_ms(median),
            ));
            rows.push(Row::new(
                format!(
                    "the interface's passes on the GPU within the frame, summed, {what}, median of \
                     {}",
                    own.len()
                ),
                ms(percentile(&own, 0.5)),
            ));
        }
        rows.push(Row::count(
            format!("passes of the interface, {what}"),
            passes,
        ));
        Ok(rows)
    }
}

/// `seconds`, which may be below zero, in microseconds or milliseconds as [`ms`] writes them.
fn signed_ms(seconds: f64) -> String {
    let written = ms(Duration::from_secs_f64(seconds.abs()));
    if seconds < 0.0 {
        format!("-{written}")
    } else {
        written
    }
}

/// How many times a canvas is drawn and measured.
pub const CANVAS_FRAMES: usize = 30;

/// Draws a canvas `runs` times with `draw`, after `WARM_UP` times that are not measured, each
/// frame waited for on the GPU: the CPU time of each, the GPU times that came back, as `gpu_time`
/// reads them from what `draw` returned, and what the last returned.
pub fn canvas_frames<T>(
    gpu: &Gpu,
    runs: usize,
    mut draw: impl FnMut() -> Result<T, GfxError>,
    gpu_time: impl Fn(&T) -> Option<Duration>,
) -> Result<(Vec<Duration>, Vec<Duration>, T), String> {
    let (mut cpu, mut gpu_times) = (Vec::new(), Vec::new());
    let mut last = None;
    for frame in 0..WARM_UP + runs {
        let (stats, duration) = timed(&mut draw);
        let stats = stats.map_err(|error| error.to_string())?;
        gpu.wait_idle().map_err(|error| error.to_string())?;
        if frame >= WARM_UP {
            cpu.push(duration);
            gpu_times.extend(gpu_time(&stats));
            last = Some(stats);
        }
    }
    Ok((cpu, gpu_times, last.expect("frames were measured")))
}

/// An editor's screen, laid out for 1920 × 1080, scaled to `height` and its columns spread over
/// `width`: a background; 100 panels with their shadows; 2 000 boxes with rounded corners, a quarter
/// of them bordered, as rows of lists; 20 000 glyphs from `glyphs`; the 16 `pictures`; half the boxes
/// and glyphs clipped to the screen's left three quarters. Returns the images that its quads name by
/// their index: `glyphs`, then the `pictures`.
fn editor_screen<'a>(
    canvas: &mut Canvas,
    glyphs: &'a Texture,
    pictures: &'a [Texture],
    w: f32,
    h: f32,
) -> Vec<InterfaceImage<'a>> {
    let scale = h / 1080.0;
    let images = std::iter::once(glyphs)
        .chain(pictures)
        .map(InterfaceImage::Texture)
        .collect();
    let glyphs = ImageId(0);
    let pictures: Vec<_> = (1..=pictures.len() as u32).map(ImageId).collect();
    canvas.draw_box(&CanvasBox {
        rect: PixelRect::new(0.0, 0.0, w, h),
        radii: [0.0; 4],
        fill: LinearColor::rgb(0.05, 0.05, 0.06),
        border_width: 0.0,
        border: LinearColor::BLACK,
        clip: None,
    });
    for index in 0..100 {
        let (x, y) = (
            (index % 10) as f32 * w / 10.0,
            (index / 10) as f32 * h / 10.0,
        );
        canvas.draw_shadow(&CanvasShadow {
            rect: PixelRect::new(x + 8.0, y + 8.0, w / 10.0 - 16.0, h / 10.0 - 16.0),
            radii: [6.0 * scale; 4],
            color: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
            offset: [0.0, 4.0 * scale],
            blur: 16.0 * scale,
            spread: 0.0,
            clip: None,
        });
    }
    let clip = |index: usize| {
        index
            .is_multiple_of(2)
            .then(|| PixelRect::new(0.0, 0.0, w * 0.75, h))
    };
    for index in 0..2000 {
        let (column, row) = (index % 8, index / 8);
        canvas.draw_box(&CanvasBox {
            rect: PixelRect::new(
                column as f32 * w / 8.0 + 4.0,
                row as f32 * h / 250.0,
                w / 8.0 - 8.0,
                h / 250.0 - 1.0,
            ),
            radii: [3.0 * scale; 4],
            fill: LinearColor::rgb(0.12, 0.12, 0.14),
            border_width: if index % 4 == 0 { scale } else { 0.0 },
            border: LinearColor::rgb(0.3, 0.3, 0.35),
            clip: clip(index),
        });
    }
    let (glyph_w, glyph_h) = (8.0 * scale, 14.0 * scale);
    let per_row = (w / glyph_w) as usize;
    for index in 0..20_000 {
        let cell = index % 4096;
        let (u, v) = ((cell % 64) as f32 / 64.0, (cell / 64) as f32 / 64.0);
        canvas.draw_image(&ImageQuad {
            rect: PixelRect::new(
                (index % per_row) as f32 * glyph_w,
                (index / per_row) as f32 * glyph_h * 1.3 % h,
                glyph_w,
                glyph_h,
            ),
            image: glyphs,
            part: ImagePart {
                x: u,
                y: v,
                width: 1.0 / 64.0,
                height: 1.0 / 64.0,
            },
            kind: ImageKind::Coverage,
            color: LinearColor::rgb(0.9, 0.9, 0.9),
            clip: clip(index),
        });
    }
    for (index, &image) in pictures.iter().enumerate() {
        canvas.draw_image(&ImageQuad {
            rect: PixelRect::new(
                index as f32 * 132.0 * scale,
                h - 140.0 * scale,
                128.0 * scale,
                128.0 * scale,
            ),
            image,
            part: ImagePart::WHOLE,
            kind: ImageKind::Color,
            color: LinearColor::WHITE,
            clip: None,
        });
    }
    images
}

/// A game's menu over its image, laid out for 1920 × 1080 and scaled to fit `w` × `h`, centred: the
/// image dimmed; four panels and the sixteen cards of a bar, with their shadows; an inventory of 100
/// bordered slots, each with a frame and a count, half of them with an icon of `icons`; 34 rows of
/// statistics and 30 buttons, with their text; a description of 32 lines, clipped to its panel, as
/// the statistics are. Some 290 boxes, 20 shadows, 3 900 glyphs from `glyphs` and 50 icons.
/// Returns the images that its quads name by their index: `glyphs`, then `icons`.
fn game_screen<'a>(
    canvas: &mut Canvas,
    glyphs: &'a Texture,
    icons: &'a Texture,
    w: f32,
    h: f32,
) -> Vec<InterfaceImage<'a>> {
    let scale = (w / 1920.0).min(h / 1080.0);
    let (left, top) = ((w - 1920.0 * scale) / 2.0, (h - 1080.0 * scale) / 2.0);
    // A rectangle of the layout, on the screen.
    let rect = |x: f32, y: f32, width: f32, height: f32| {
        PixelRect::new(
            left + x * scale,
            top + y * scale,
            width * scale,
            height * scale,
        )
    };
    let images = vec![
        InterfaceImage::Texture(glyphs),
        InterfaceImage::Texture(icons),
    ];
    let (glyphs, icons) = (ImageId(0), ImageId(1));
    let plain = |rect: PixelRect, radius: f32, fill: LinearColor| CanvasBox {
        rect,
        radii: [radius * scale; 4],
        fill,
        border_width: 0.0,
        border: LinearColor::BLACK,
        clip: None,
    };
    // `count` glyphs of 8 × 14, `per_line` to a line, the lines 16 apart, from (`x`, `y`).
    let mut cell = 0;
    let mut text = |canvas: &mut Canvas,
                    (x, y): (f32, f32),
                    count: usize,
                    per_line: usize,
                    clip: Option<PixelRect>| {
        for index in 0..count {
            let (u, v) = ((cell % 64) as f32 / 64.0, (cell / 64 % 64) as f32 / 64.0);
            cell += 1;
            canvas.draw_image(&ImageQuad {
                rect: rect(
                    x + (index % per_line) as f32 * 8.0,
                    y + (index / per_line) as f32 * 16.0,
                    8.0,
                    14.0,
                ),
                image: glyphs,
                part: ImagePart {
                    x: u,
                    y: v,
                    width: 1.0 / 64.0,
                    height: 1.0 / 64.0,
                },
                kind: ImageKind::Coverage,
                color: LinearColor::rgb(0.9, 0.88, 0.8),
                clip,
            });
        }
    };

    canvas.draw_box(&plain(
        PixelRect::new(0.0, 0.0, w, h),
        0.0,
        LinearColor::rgba(0.0, 0.0, 0.0, 0.6),
    ));
    let panels = [
        (60.0, 80.0, 820.0, 860.0),
        (920.0, 80.0, 500.0, 520.0),
        (920.0, 640.0, 500.0, 300.0),
        (1460.0, 80.0, 400.0, 860.0),
    ];
    let cards = (0..16).map(|index| (60.0 + index as f32 * 112.0, 960.0, 100.0, 100.0));
    for (x, y, width, height) in panels.into_iter().chain(cards) {
        canvas.draw_shadow(&CanvasShadow {
            rect: rect(x, y, width, height),
            radii: [12.0 * scale; 4],
            color: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
            offset: [0.0, 8.0 * scale],
            blur: 24.0 * scale,
            spread: 0.0,
            clip: None,
        });
        canvas.draw_box(&CanvasBox {
            border_width: 2.0 * scale,
            border: LinearColor::rgb(0.5, 0.42, 0.3),
            ..plain(
                rect(x, y, width, height),
                12.0,
                LinearColor::rgba(0.08, 0.08, 0.1, 0.92),
            )
        });
        if height == 100.0 {
            text(canvas, (x + 34.0, y + 80.0), 4, 4, None);
        }
    }

    // The inventory: its title, then its slots, their frames and icons, then their counts.
    text(canvas, (80.0, 92.0), 20, 20, None);
    let slot = |index: usize| {
        (
            80.0 + (index % 10) as f32 * 76.0,
            120.0 + (index / 10) as f32 * 76.0,
        )
    };
    for index in 0..100 {
        let (x, y) = slot(index);
        canvas.draw_box(&CanvasBox {
            border_width: scale,
            border: LinearColor::rgb(0.35, 0.3, 0.22),
            ..plain(
                rect(x, y, 68.0, 68.0),
                6.0,
                LinearColor::rgb(0.05, 0.05, 0.06),
            )
        });
        canvas.draw_box(&plain(
            rect(x + 4.0, y + 4.0, 60.0, 60.0),
            4.0,
            LinearColor::rgba(0.2, 0.12, 0.3, 0.35),
        ));
        if index < 50 {
            let (u, v) = ((index % 8) as f32 / 8.0, (index / 8 % 8) as f32 / 8.0);
            canvas.draw_image(&ImageQuad {
                rect: rect(x + 6.0, y + 6.0, 56.0, 56.0),
                image: icons,
                part: ImagePart {
                    x: u,
                    y: v,
                    width: 1.0 / 8.0,
                    height: 1.0 / 8.0,
                },
                kind: ImageKind::Color,
                color: LinearColor::WHITE,
                clip: None,
            });
        }
    }
    for index in 0..100 {
        let (x, y) = slot(index);
        text(canvas, (x + 50.0, y + 52.0), 2, 2, None);
    }

    // The statistics, a list cut at its panel's bottom.
    text(canvas, (1480.0, 96.0), 20, 20, None);
    let list = Some(rect(1460.0, 120.0, 400.0, 800.0));
    for row in 0..34 {
        let y = 130.0 + row as f32 * 23.0;
        let shade = if row % 2 == 0 { 0.1 } else { 0.07 };
        canvas.draw_box(&CanvasBox {
            clip: list,
            ..plain(
                rect(1480.0, y, 360.0, 21.0),
                3.0,
                LinearColor::rgb(shade, shade, shade),
            )
        });
        text(canvas, (1484.0, y + 4.0), 44, 44, list);
    }

    // The buttons, three columns of ten.
    for index in 0..30 {
        let (x, y) = (
            936.0 + (index % 3) as f32 * 160.0,
            656.0 + (index / 3) as f32 * 28.0,
        );
        canvas.draw_box(&plain(
            rect(x, y, 156.0, 24.0),
            4.0,
            LinearColor::rgb(0.18, 0.15, 0.12),
        ));
        text(canvas, (x + 38.0, y + 5.0), 10, 10, None);
    }

    // The description, longer than its panel.
    text(canvas, (940.0, 96.0), 20, 20, None);
    text(
        canvas,
        (940.0, 124.0),
        32 * 56,
        56,
        Some(rect(920.0, 80.0, 500.0, 520.0)),
    );
    images
}
