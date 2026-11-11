//! A prototype thrown away after measuring (M6d.4a, ADR-0242): the canvas's two screens of the
//! benchmark `render.canvas` drawn with three layouts of its items, S (storage buffers, today's),
//! I (instances, one format a kind) and T (items in a texture), and S2, S reading its items' fields
//! one by one as T does, on four devices, `vulkan`, `vulkan-webgl2`, `gl` and `gl-es`; their
//! images compared with S's, their frames timed, and their pipelines counted against WebGL2's
//! limits; with `--effect`, the same for the effects (`effects.rs`). `README.md` says how to run
//! it and reads its output.

mod blur;
mod canvas;
mod effects;
mod gpu;
mod instances;
mod instances_fx;
mod path;
mod scenes;
mod storage;
mod storage_fx;
mod texture;
mod texture_fx;

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use canvas::Canvas;
use gpu::{Config, FrameCost, Gpu, Images, Layout, Target, Timer, Uploads};

/// The size at which images are compared.
const IMAGE_SIZE: (u32, u32) = (1280, 800);
/// The sizes timed: the hardware tiers' screens.
const SIZES: [(u32, u32); 3] = [(1280, 800), (1920, 1080), (3840, 2160)];
/// The largest difference allowed between a layout's image and S's, in steps of 1/255.
const TOLERANCE: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Editor,
    Game,
    /// Every kind, the gradient included: compared, not timed.
    Kinds,
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Game => "game",
            Self::Kinds => "kinds",
        }
    }

    fn canvas(self, (width, height): (u32, u32)) -> Canvas {
        let (w, h) = (width as f32, height as f32);
        let mut canvas = Canvas::default();
        match self {
            Self::Editor => canvas::editor_screen(&mut canvas, gpu::GLYPHS, &gpu::pictures(), w, h),
            Self::Game => canvas::game_screen(&mut canvas, gpu::GLYPHS, gpu::ICONS, w, h),
            Self::Kinds => canvas::kinds_screen(&mut canvas, gpu::GLYPHS, gpu::ICONS, w, h),
        }
        canvas
    }
}

/// A layout as the runs use it: its name, the layout (one object, or one per canvas a frame
/// draws), and the staging of its uploads, its own.
struct Entry<L> {
    name: &'static str,
    layout: L,
    uploads: Uploads,
}

impl<L> Entry<L> {
    fn new(name: &'static str, layout: L) -> Self {
        Self {
            name,
            layout,
            uploads: Uploads::default(),
        }
    }
}

struct Options {
    config: Config,
    /// Frames measured, of each layout.
    frames: usize,
    /// Frames drawn first, of each layout.
    warm_up: usize,
    any_adapter: bool,
    /// The effects to run in place of the screens, if any.
    effects: Vec<effects::Effect>,
}

const USAGE: &str = "usage: canvas-prototype <vulkan|vulkan-webgl2|gl|gl-es> [--frames N] \
                     [--warm-up N] [--any-adapter] [--effect transform|group|blur|path|grow|all]...";

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let config = args
        .next()
        .and_then(|name| Config::parse(&name))
        .ok_or(USAGE)?;
    let mut options = Options {
        config,
        frames: 30,
        warm_up: 10,
        any_adapter: false,
        effects: Vec::new(),
    };
    while let Some(arg) = args.next() {
        let mut number = || {
            args.next()
                .and_then(|value| value.parse().ok())
                .ok_or(format!("{arg} needs a number"))
        };
        match arg.as_str() {
            "--frames" => options.frames = number()?,
            "--warm-up" => options.warm_up = number()?,
            "--any-adapter" => options.any_adapter = true,
            "--effect" => options.effects.extend(
                args.next()
                    .and_then(|name| effects::Effect::parse(&name))
                    .ok_or(USAGE)?,
            ),
            _ => return Err(USAGE.to_owned()),
        }
    }
    Ok(options)
}

fn main() -> ExitCode {
    let result = options().and_then(|options| run(&options));
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("a check failed: see the lines marked FAIL");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Where the images are written: the workspace's `target/canvas-prototype/`.
fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/canvas-prototype")
}

/// Runs everything on `options.config`'s device; false if a check failed.
fn run(options: &Options) -> Result<bool, String> {
    let config = options.config;
    let gpu = gpu::open(config, options.any_adapter)?;
    let label = gpu.label.as_str();
    let info = &gpu.info;
    println!(
        "adapter\t{}\tbackend {:?}\ttype {:?}\tdriver {} {}",
        info.name, info.backend, info.device_type, info.driver, info.driver_info
    );
    let limits = &gpu.limits;
    println!(
        "device\t{label}\tinter-stage variables {}\tvertex attributes {}\tstride {}\tstorage buffers \
         a stage {}\tuniform binding {} B\ttexture side {}\tbuffer {} B\ttimestamps {}\tcopies' \
         start {}",
        limits.max_inter_stage_shader_variables,
        limits.max_vertex_attributes,
        limits.max_vertex_buffer_array_stride,
        limits.max_storage_buffers_per_shader_stage,
        limits.max_uniform_buffer_binding_size,
        limits.max_texture_dimension_2d,
        limits.max_buffer_size,
        if gpu.can_time() { "yes" } else { "none" },
        match (gpu.can_time(), gpu.times_in_encoders()) {
            (false, _) => "not timed",
            (true, true) => "a timestamp in the encoder",
            (true, false) => "an empty render pass's first timestamp",
        },
    );
    let images = Images::new(&gpu);
    gpu.check()?;
    let mut ok = true;
    if !options.effects.is_empty() {
        let cx = effects::Context {
            gpu: &gpu,
            config,
            images: &images,
            frames: options.frames,
            warm_up: options.warm_up,
        };
        for &effect in &options.effects {
            ok &= effects::run(&cx, effect)?;
        }
        return Ok(ok);
    }

    let mut layouts: Vec<Entry<Box<dyn Layout>>> = Vec::new();
    for (name, source) in [("S", storage::SOURCE), ("S2", storage::FIELDS_SOURCE)] {
        match (storage::Storage::new(&gpu, &images, name, source), config) {
            (Ok(layout), Config::Vulkan) => layouts.push(Entry::new(name, Box::new(layout))),
            (Ok(_), _) => println!("{name}\tbuilt on {label}, but it runs on vulkan alone"),
            (Err(error), Config::Vulkan) => return Err(format!("{name} cannot be built: {error}")),
            (Err(error), _) => println!(
                "{name}\tcannot be built on {label}, as expected: {}",
                one_line(&error)
            ),
        }
    }
    match instances::Instances::new(&gpu, &images) {
        Ok(layout) => layouts.push(Entry::new("I", Box::new(layout))),
        Err(error) => {
            println!("I\tFAIL: cannot be built: {}", one_line(&error));
            ok = false;
        }
    }
    match texture::Texture::new(&gpu, &images) {
        Ok(layout) => layouts.push(Entry::new("T", Box::new(layout))),
        Err(error) => {
            println!("T\tFAIL: cannot be built: {}", one_line(&error));
            ok = false;
        }
    }
    gpu.check()?;

    print_limits(&gpu);
    ok &= compare_images(&gpu, config, &mut layouts)?;
    if options.frames > 0 {
        time(&gpu, &mut layouts, options)?;
    }
    Ok(ok)
}

/// wgpu's error on one line.
fn one_line(error: &str) -> String {
    error.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `@location`s of the struct `name` in `source`: its values between the shader's stages,
/// `@builtin(position)` not counted, as WebGL2's 15 count them.
fn varyings(source: &str, name: &str) -> usize {
    let start = source
        .find(&format!("struct {name} {{"))
        .unwrap_or_else(|| panic!("no struct {name}"));
    let body = &source[start..];
    let end = body.find("\n}").expect("the struct ends");
    body[..end].matches("@location(").count()
}

/// Each pipeline against WebGL2's limits.
fn print_limits(gpu: &Gpu) {
    let webgl2 = wgpu::Limits::downlevel_webgl2_defaults();
    println!(
        "\n# limits: each pipeline, against WebGL2's (wgpu 30.0.1, downlevel_webgl2_defaults): {} \
         vertex attributes, a stride of {} bytes, {} inter-stage variables, {} bind groups, {} \
         sampled textures a stage, {} storage buffers, {} bytes a uniform binding, textures of {} \
         texels a side, buffers of {} bytes",
        webgl2.max_vertex_attributes,
        webgl2.max_vertex_buffer_array_stride,
        webgl2.max_inter_stage_shader_variables,
        webgl2.max_bind_groups,
        webgl2.max_sampled_textures_per_shader_stage,
        webgl2.max_storage_buffers_per_shader_stage,
        webgl2.max_uniform_buffer_binding_size,
        webgl2.max_texture_dimension_2d,
        webgl2.max_buffer_size,
    );
    println!(
        "layout\tpipeline\tattributes\tstride\tvaryings\tbind groups\tsampled VS\tsampled FS\t\
         storage VS\tstorage FS\tuniform bytes\tmost items under WebGL2"
    );
    let transforms = gpu::TRANSFORMS * gpu::TRANSFORM_BYTES;
    let on_device = (gpu.limits.max_storage_buffer_binding_size / canvas::ITEM_SIZE as u64)
        .min(storage::MAX_ITEMS as u64);
    for (name, source) in [("S", storage::SOURCE), ("S2", storage::FIELDS_SOURCE)] {
        println!(
            "{name}\tcanvas\t0\t-\t{}\t2\t0\t1\t2\t1\t16\tnone, no storage buffer (on this device: \
             {on_device})",
            varyings(source, "VertexOutput"),
        );
    }
    for format in &instances::FORMATS {
        let stride = instances::stride(format.attributes);
        println!(
            "I\t{}\t{}\t{}\t{}\t{}\t0\t{}\t0\t0\t{}\t{} (buffer / stride)",
            format.name,
            format.attributes.len(),
            stride,
            varyings(instances::SOURCE, format.output),
            if format.image { 2 } else { 1 },
            usize::from(format.image),
            16 + transforms,
            webgl2.max_buffer_size / stride,
        );
    }
    println!(
        "T\tcanvas\t0\t-\t{}\t2\t1\t2\t0\t0\t{}\t{} ({} × {} texels / 9)",
        varyings(texture::SOURCE, "VertexOutput"),
        16 + transforms,
        texture::MAX_ITEMS,
        texture::WIDTH,
        texture::MAX_HEIGHT,
    );
}

/// Draws `canvas` once with `entry`'s layout into a new target of `size`, cleared first, and reads
/// it.
fn image(
    gpu: &Gpu,
    entry: &mut Entry<Box<dyn Layout>>,
    canvas: &Canvas,
    size: (u32, u32),
    srgb: bool,
) -> Result<Vec<u8>, String> {
    let target = Target::new(gpu, size, srgb);
    target.clear(gpu)?;
    gpu::frame(
        gpu,
        entry.layout.as_mut(),
        &mut entry.uploads,
        canvas,
        &target,
        None,
    )?;
    target.read(gpu)
}

/// S's image of `screen`, which `gpu.reference`'s run wrote: lavapipe's, or the GPU's on a run with
/// `--any-adapter`.
fn reference_image(gpu: &Gpu, screen: &str) -> Result<Vec<u8>, String> {
    let path = output_dir().join(format!("{}-S-{screen}.png", gpu.reference));
    read_png(&path).map_err(|error| {
        format!(
            "{}: {error}; run `{}` first, which writes S's images",
            path.display(),
            gpu.reference
        )
    })
}

/// Each layout's image of each screen, written as a PNG and compared with S's on `vulkan`.
fn compare_images(
    gpu: &Gpu,
    config: Config,
    layouts: &mut [Entry<Box<dyn Layout>>],
) -> Result<bool, String> {
    let dir = output_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    println!(
        "\n# images at {} × {}, against S's on {}: the largest difference of a channel, in steps \
         of 1/255, and the pixels where one differs by more than 1; written to {}",
        IMAGE_SIZE.0,
        IMAGE_SIZE.1,
        gpu.reference,
        dir.display()
    );
    println!(
        "{}\tlayout\tscreen\tlargest\tpixels > 1\tverdict",
        gpu.config_header()
    );
    let mut ok = true;
    for screen in [Screen::Editor, Screen::Game, Screen::Kinds] {
        let canvas = screen.canvas(IMAGE_SIZE);
        let mut reference = match config {
            Config::Vulkan => None,
            _ => Some(reference_image(gpu, screen.name())?),
        };
        for entry in layouts.iter_mut() {
            let name = entry.name;
            let pixels = image(gpu, entry, &canvas, IMAGE_SIZE, false)?;
            let path = dir.join(format!("{}-{name}-{}.png", gpu.label, screen.name()));
            write_png(&path, IMAGE_SIZE, &pixels)?;
            if name == "S" {
                // Today's path: an sRGB target drawn through a view without its encoding.
                let srgb = image(gpu, entry, &canvas, IMAGE_SIZE, true)?;
                let same = srgb == pixels;
                println!(
                    "{}\tS (Rgba8UnormSrgb target, Rgba8Unorm view)\t{}\t{}\t-\t{}",
                    gpu.label,
                    screen.name(),
                    if same { "identical bytes" } else { "differs" },
                    if same { "ok" } else { "FAIL" }
                );
                ok &= same;
                reference = Some(pixels);
                continue;
            }
            let reference = reference.as_ref().expect("S is drawn first on vulkan");
            let (largest, over) = difference(reference, &pixels);
            let pass = largest <= TOLERANCE;
            println!(
                "{}\t{name}\t{}\t{largest}\t{over}\t{}",
                gpu.label,
                screen.name(),
                if pass { "ok" } else { "FAIL" }
            );
            ok &= pass;
        }
    }
    Ok(ok)
}

/// The largest difference of a channel between `a` and `b`, and the pixels where one differs by
/// more than 1.
fn difference(a: &[u8], b: &[u8]) -> (u8, usize) {
    let mut largest = 0;
    let mut over = 0;
    for (pa, pb) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
        let worst = pa
            .iter()
            .zip(pb)
            .map(|(&x, &y)| x.abs_diff(y))
            .max()
            .unwrap_or(0);
        largest = largest.max(worst);
        over += usize::from(worst > 1);
    }
    (largest, over)
}

fn write_png(path: &Path, (width, height): (u32, u32), pixels: &[u8]) -> Result<(), String> {
    let error = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
    let file = File::create(path).map_err(|e| error(&e))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| error(&e))?;
    writer.write_image_data(pixels).map_err(|e| error(&e))?;
    writer.finish().map_err(|e| error(&e))
}

fn read_png(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut reader = png::Decoder::new(BufReader::new(file))
        .read_info()
        .map_err(|error| error.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("too large")?];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|error| error.to_string())?;
    if (info.width, info.height) != IMAGE_SIZE || info.color_type != png::ColorType::Rgba {
        return Err("not an RGBA image of the compared size".to_owned());
    }
    pixels.truncate(info.buffer_size());
    Ok(pixels)
}

/// The value at `share` of `times`, sorted, as `benches/src/measure.rs`'s `percentile` takes it.
fn percentile(times: &[Duration], share: f64) -> Duration {
    let mut sorted = times.to_vec();
    sorted.sort_unstable();
    sorted[((sorted.len() - 1) as f64 * share).round() as usize]
}

fn micros(duration: Duration) -> String {
    format!("{:.1}", duration.as_secs_f64() * 1e6)
}

/// The median and the 90th percentile of `times`, in microseconds.
fn spread(times: &[Duration]) -> String {
    format!(
        "{}\t{}",
        micros(percentile(times, 0.5)),
        micros(percentile(times, 0.9))
    )
}

/// The columns of times that every table of times shows, which [`cost_columns`] fills.
const COST_HEADER: &str = "write p50\twrite p90\tencode p50\tencode p90\tsubmit p50\tsubmit p90\t\
                           gpu pass p50\tgpu pass p90\tgpu with copies p50\tgpu with copies p90\t\
                           wall p50\twall p90";

/// The median and the 90th percentile of each time of `costs`, as [`COST_HEADER`] names them;
/// `none` for a GPU's time where the device has no timestamps.
fn cost_columns(costs: &[FrameCost]) -> String {
    let cpu =
        |field: fn(&FrameCost) -> Duration| spread(&costs.iter().map(field).collect::<Vec<_>>());
    let gpu = |field: fn(&FrameCost) -> Option<Duration>| {
        costs
            .iter()
            .map(field)
            .collect::<Option<Vec<_>>>()
            .map_or("none\tnone".to_owned(), |times| spread(&times))
    };
    [
        cpu(|cost| cost.write),
        cpu(|cost| cost.recorded.encode),
        cpu(|cost| cost.recorded.submit),
        gpu(|cost| cost.recorded.gpu_pass()),
        gpu(|cost| cost.recorded.with_copies),
        cpu(|cost| cost.recorded.wall),
    ]
    .join("\t")
}

/// Draws `warm_up + frames` rounds of one frame of each of `count` layouts in turn, `frame(layout,
/// round)` drawing one, so that a change of the machine's load falls on every layout alike; gives
/// each layout's costs of the rounds after the warm-up.
fn interleaved(
    count: usize,
    warm_up: usize,
    frames: usize,
    mut frame: impl FnMut(usize, usize) -> Result<FrameCost, String>,
) -> Result<Vec<Vec<FrameCost>>, String> {
    let mut costs: Vec<Vec<FrameCost>> = (0..count).map(|_| Vec::with_capacity(frames)).collect();
    for round in 0..warm_up + frames {
        for (layout, kept) in costs.iter_mut().enumerate() {
            let cost = frame(layout, round)?;
            if round >= warm_up {
                kept.push(cost);
            }
        }
    }
    Ok(costs)
}

/// Each layout's frames of each screen at each size, interleaved: the medians and 90th
/// percentiles.
fn time(
    gpu: &Gpu,
    layouts: &mut [Entry<Box<dyn Layout>>],
    options: &Options,
) -> Result<(), String> {
    let timer = Timer::new(gpu);
    println!(
        "\n# times: {} frames of each layout measured after {} of warm-up, in rounds of one frame \
         of each layout in turn, each frame waited for; p50 and p90 in microseconds; bytes written \
         each frame",
        options.frames, options.warm_up
    );
    println!(
        "{}\tlayout\tscreen\tsize\titems\tdraws\tpipelines\tbytes\t{COST_HEADER}",
        gpu.config_header()
    );
    for size in SIZES {
        let target = Target::new(gpu, size, false);
        target.clear(gpu)?;
        for screen in [Screen::Editor, Screen::Game] {
            let canvas = screen.canvas(size);
            let costs = interleaved(
                layouts.len(),
                options.warm_up,
                options.frames,
                |index, _| {
                    let entry = &mut layouts[index];
                    gpu::frame(
                        gpu,
                        entry.layout.as_mut(),
                        &mut entry.uploads,
                        &canvas,
                        &target,
                        timer.as_ref(),
                    )
                },
            )?;
            for (entry, costs) in layouts.iter().zip(&costs) {
                let last = costs.last().expect("frames were measured");
                println!(
                    "{}\t{}\t{}\t{}x{}\t{}\t{}\t{}\t{}\t{}",
                    gpu.label,
                    entry.name,
                    screen.name(),
                    size.0,
                    size.1,
                    canvas.items.len(),
                    last.recorded.draws,
                    last.recorded.pipelines,
                    last.bytes,
                    cost_columns(costs),
                );
            }
        }
    }
    Ok(())
}
