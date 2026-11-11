//! The effects, each chosen by `--effect` (README): a transform per item, a group drawn apart, a
//! blur of what lies behind, a path, and one element animated over a still screen that grows. Each
//! runs on every layout the device takes, the first four on the effects' pipelines (S on Vulkan
//! alone), the growing screen on the screens' pipelines. Each image at 1280 × 800 is compared with
//! S's on `vulkan`; each frame is timed at the three sizes.

use std::time::{Duration, Instant};

use crate::blur::{self, Backdrop, Blur};
use crate::canvas::{Canvas, ImageId};
use crate::gpu::{
    self, Config, FxLayout, Gpu, Images, Layout, Recorded, Recorder, Rewrite, Target, Timer,
};
use crate::{
    IMAGE_SIZE, SIZES, TOLERANCE, difference, instances, instances_fx, one_line, output_dir, path,
    read_png, scenes, spread, storage, storage_fx, texture, texture_fx, varyings, write_png,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Transform,
    Group,
    Blur,
    Path,
    Grow,
}

impl Effect {
    pub const ALL: [Self; 5] = [
        Self::Transform,
        Self::Group,
        Self::Blur,
        Self::Path,
        Self::Grow,
    ];

    pub fn parse(name: &str) -> Option<Vec<Self>> {
        if name == "all" {
            return Some(Self::ALL.to_vec());
        }
        Self::ALL
            .into_iter()
            .find(|effect| effect.name() == name)
            .map(|effect| vec![effect])
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Transform => "transform",
            Self::Group => "group",
            Self::Blur => "blur",
            Self::Path => "path",
            Self::Grow => "grow",
        }
    }
}

/// What every effect runs with.
pub struct Context<'a> {
    pub gpu: &'a Gpu,
    pub config: Config,
    pub images: &'a Images,
    pub frames: usize,
    pub warm_up: usize,
}

impl Context<'_> {
    /// The screens' images, which items name by their index.
    fn views(&self) -> Vec<&wgpu::TextureView> {
        self.images.views.iter().collect()
    }
}

/// Runs `effect`; false if a check failed.
pub fn run(cx: &Context<'_>, effect: Effect) -> Result<bool, String> {
    println!("\n# effect {}", effect.name());
    match effect {
        Effect::Transform => single(cx, effect),
        Effect::Path => single(cx, effect),
        Effect::Group => group(cx),
        Effect::Blur => blur(cx),
        Effect::Grow => grow(cx),
    }
}

/// The effects' layout `name`.
fn make(gpu: &Gpu, name: &str) -> Result<Box<dyn FxLayout>, String> {
    Ok(match name {
        "S" => Box::new(storage_fx::StorageFx::new(gpu)?),
        "I" => Box::new(instances_fx::InstancesFx::new(gpu)?),
        "T" => Box::new(texture_fx::TextureFx::new(gpu)?),
        _ => unreachable!("no layout {name}"),
    })
}

/// The effects' layouts the device takes, by name, each made `copies` times: S on `vulkan` alone,
/// where it must be built; elsewhere its error is printed.
fn fx_layouts(
    cx: &Context<'_>,
    copies: usize,
) -> Result<Vec<(&'static str, Vec<Box<dyn FxLayout>>)>, String> {
    let mut layouts = Vec::new();
    for name in ["S", "I", "T"] {
        let made: Result<Vec<_>, String> = (0..copies).map(|_| make(cx.gpu, name)).collect();
        match (made, name, cx.config) {
            (Ok(made), "S", Config::Vulkan) | (Ok(made), "I" | "T", _) => {
                layouts.push((name, made))
            }
            (Ok(_), _, _) => println!(
                "S fx\tbuilt on {}, but it runs on vulkan alone",
                cx.gpu.label
            ),
            (Err(error), "S", config) if config != Config::Vulkan => println!(
                "S fx\tcannot be built on {}, as expected: {}",
                cx.gpu.label,
                one_line(&error)
            ),
            (Err(error), name, _) => return Err(format!("{name} fx cannot be built: {error}")),
        }
    }
    cx.gpu.check()?;
    Ok(layouts)
}

/// Writes `pixels` as `<config>-<layout>-<scene>.png`; compares those of I and T with S's on
/// `vulkan`, S's own being the reference. False if they differ by more than [`TOLERANCE`].
fn compare(cx: &Context<'_>, layout: &str, scene: &str, pixels: &[u8]) -> Result<bool, String> {
    compare_with(cx, layout, scene, scene, pixels)
}

/// As [`compare`], against S's image of `reference`.
fn compare_with(
    cx: &Context<'_>,
    layout: &str,
    scene: &str,
    reference: &str,
    pixels: &[u8],
) -> Result<bool, String> {
    let dir = output_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let file = format!("{}-{layout}-{scene}.png", cx.gpu.label);
    write_png(&dir.join(&file), IMAGE_SIZE, pixels)?;
    let reference_path = dir.join(format!("vulkan-S-{reference}.png"));
    if layout == "S" && scene == reference {
        println!("{}\tS\t{scene}\treference\t-\tok", cx.gpu.label);
        return Ok(true);
    }
    let reference = read_png(&reference_path).map_err(|error| {
        format!(
            "{}: {error}; run `vulkan` first, which writes S's images",
            reference_path.display()
        )
    })?;
    let (largest, over) = difference(&reference, pixels);
    let pass = largest <= TOLERANCE;
    println!(
        "{}\t{layout}\t{scene}\t{largest}\t{over}\t{}",
        cx.gpu.label,
        if pass { "ok" } else { "FAIL" }
    );
    Ok(pass)
}

/// One frame's cost.
struct Sample {
    /// CPU: the canvas's items made (the growing screen only).
    build: Duration,
    /// CPU: the items turned into bytes and written.
    write: Duration,
    bytes: u64,
    recorded: Recorded,
}

fn measure(
    cx: &Context<'_>,
    mut frame: impl FnMut(usize) -> Result<Sample, String>,
) -> Result<Vec<Sample>, String> {
    let mut samples = Vec::with_capacity(cx.frames);
    for index in 0..cx.warm_up + cx.frames {
        let sample = frame(index)?;
        if index >= cx.warm_up {
            samples.push(sample);
        }
    }
    Ok(samples)
}

/// The GPU's time of a frame: its passes' sum, if each was timed.
fn gpu_time(recorded: &Recorded) -> Option<Duration> {
    recorded.passes.iter().copied().sum()
}

fn times_header() {
    println!(
        "config\tlayout\teffect\tvariant\tsize\titems\tdraws\tpipelines\tpasses\tbytes\tbuild p50\t\
         build p90\twrite p50\twrite p90\tencode p50\tencode p90\tGPU p50\tGPU p90\twall p50\twall p90"
    );
}

#[allow(clippy::too_many_arguments)]
fn times_row(
    cx: &Context<'_>,
    layout: &str,
    effect: &str,
    variant: &str,
    size: (u32, u32),
    items: usize,
    samples: &[Sample],
) {
    let of = |field: fn(&Sample) -> Duration| samples.iter().map(field).collect::<Vec<_>>();
    let gpu_times: Option<Vec<Duration>> = samples
        .iter()
        .map(|sample| gpu_time(&sample.recorded))
        .collect();
    let last = samples.last().expect("frames were measured");
    println!(
        "{}\t{layout}\t{effect}\t{variant}\t{}x{}\t{items}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        cx.gpu.label,
        size.0,
        size.1,
        last.recorded.draws,
        last.recorded.pipelines,
        last.recorded.names.len(),
        last.bytes,
        spread(&of(|sample| sample.build)),
        spread(&of(|sample| sample.write)),
        spread(&of(|sample| sample.recorded.encode)),
        gpu_times.map_or("none\tnone".to_owned(), |times| spread(&times)),
        spread(&of(|sample| sample.recorded.wall)),
    );
}

/// The step a pass belongs to: a glass's blur gathers its passes.
fn step_of(pass: &str) -> String {
    match pass.strip_prefix("glass ") {
        Some(rest) => format!("glass {} blur", rest.split(' ').next().unwrap_or("")),
        None => pass.to_owned(),
    }
}

/// Each step's GPU time across the frames: its passes summed in each frame.
fn steps_rows(
    cx: &Context<'_>,
    layout: &str,
    effect: &str,
    variant: &str,
    size: (u32, u32),
    samples: &[Sample],
) {
    let first = &samples[0].recorded;
    let mut steps: Vec<(String, usize)> = Vec::new();
    for name in &first.names {
        let step = step_of(name);
        match steps.iter_mut().find(|(known, _)| *known == step) {
            Some((_, count)) => *count += 1,
            None => steps.push((step, 1)),
        }
    }
    for (step, count) in steps {
        let times: Option<Vec<Duration>> = samples
            .iter()
            .map(|sample| {
                sample
                    .recorded
                    .names
                    .iter()
                    .zip(&sample.recorded.passes)
                    .filter(|(name, _)| step_of(name) == step)
                    .map(|(_, time)| *time)
                    .sum::<Option<Duration>>()
            })
            .collect();
        println!(
            "{}\t{layout}\t{effect}\t{variant}\t{}x{}\t{step}\t{count}\t{}",
            cx.gpu.label,
            size.0,
            size.1,
            times.map_or("none\tnone".to_owned(), |times| spread(&times)),
        );
    }
}

fn steps_header() {
    println!("config\tlayout\teffect\tvariant\tsize\tstep\tpasses\tGPU p50\tGPU p90");
}

/// A target of `size`, cleared, drawn into by `record`, read back.
fn image_of(
    gpu: &Gpu,
    size: (u32, u32),
    record: impl FnOnce(&Target) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let target = Target::new(gpu, size, false);
    target.clear(gpu)?;
    record(&target)?;
    target.read(gpu)
}

/// One pass of `layout` over `target`, `canvas` written first.
fn one_pass(
    gpu: &Gpu,
    timer: Option<&Timer>,
    layout: &mut dyn Layout,
    canvas: &Canvas,
    target: &wgpu::TextureView,
    size: (u32, u32),
) -> Result<Sample, String> {
    let start = Instant::now();
    let bytes = layout.upload(gpu, canvas, size);
    let write = start.elapsed();
    let mut recorder = Recorder::new(gpu, timer);
    recorder.pass("canvas", target, wgpu::LoadOp::Load, |pass| {
        layout.encode(pass)
    });
    Ok(Sample {
        build: Duration::ZERO,
        write,
        bytes,
        recorded: recorder.finish()?,
    })
}

/// The effects' pipelines against WebGL2's limits, as `print_limits` counts the screens'.
fn print_fx_limits() {
    println!("\n## limits of the effects' pipelines (the screens' in the main run)");
    println!(
        "layout\tpipeline\tattributes\tstride\tvaryings\tbind groups\tsampled VS\tsampled FS\t\
         storage VS\tstorage FS\tuniform bytes\tvariable data"
    );
    println!(
        "S\tfx\t0\t-\t{}\t2\t0\t1\t2\t2\t{}\titem 144 B, transform 32 B, segment 16 B: storage buffers",
        varyings(storage_fx::SOURCE, "VertexOutput"),
        gpu::FX_FRAME_BYTES,
    );
    for (kind, format) in instances_fx::FORMATS.iter().enumerate() {
        let stride = instances::stride(format.attributes);
        let before = instances::FORMATS
            .get(kind)
            .map(|base| {
                format!(
                    " (screens' {} B, {} attributes, {} varyings)",
                    instances::stride(base.attributes),
                    base.attributes.len(),
                    varyings(instances::SOURCE, base.output)
                )
            })
            .unwrap_or_default();
        println!(
            "I\tfx {}\t{}\t{}\t{}\t{}\t0\t{}\t0\t0\t{}\tinstance {stride} B{before}{}",
            format.name,
            format.attributes.len(),
            stride,
            varyings(instances_fx::SOURCE, format.output),
            if format.image { 2 } else { 1 },
            1 + usize::from(format.image),
            gpu::FX_FRAME_BYTES,
            if format.name == "path" {
                "; segments 16 B, texels of an Rgba32Float texture"
            } else {
                ""
            },
        );
    }
    println!(
        "T\tfx\t0\t-\t{}\t2\t1\t2\t0\t0\t{}\titem {} texels, transform {} texels, segment 1 texel, of one texture of {} texels",
        varyings(texture_fx::SOURCE, "VertexOutput"),
        gpu::FX_FRAME_BYTES,
        texture_fx::ITEM_TEXELS,
        texture_fx::TRANSFORM_TEXELS,
        texture_fx::MAX_TEXELS,
    );
    println!(
        "all\tblur passes, copy\t0\t-\t0\t1\t0\t1\t0\t0\t160\tnone: a triangle that covers the target"
    );
}

/// The transform's and the path's effects: one canvas, one pass.
fn single(cx: &Context<'_>, effect: Effect) -> Result<bool, String> {
    let scene = |(width, height): (u32, u32)| -> Canvas {
        let (w, h) = (width as f32, height as f32);
        match effect {
            Effect::Transform => scenes::transform_screen(gpu::GLYPHS, gpu::ICONS, w, h),
            _ => {
                let mut canvas = Canvas::default();
                path::path_screen(&mut canvas, w, h);
                canvas
            }
        }
    };
    print_fx_limits();
    if effect == Effect::Path {
        println!(
            "path\tshapes 20\tsegments each {:?}",
            path::segment_counts()
        );
    }
    let mut layouts = fx_layouts(cx, 1)?;
    let views = cx.views();
    for (_, made) in &mut layouts {
        made[0].set_images(cx.gpu, &views, &cx.images.sampler);
    }
    println!(
        "\n## images at {} × {}, against S's on vulkan: largest difference, pixels > 1",
        IMAGE_SIZE.0, IMAGE_SIZE.1
    );
    println!("config\tlayout\tscene\tlargest\tpixels > 1\tverdict");
    let mut ok = true;
    let canvas = scene(IMAGE_SIZE);
    for (name, made) in &mut layouts {
        let layout = made[0].as_mut();
        let pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
            gpu::frame(cx.gpu, layout, &canvas, target, None).map(|_| ())
        })?;
        ok &= compare(cx, name, effect.name(), &pixels)?;
        if effect == Effect::Transform {
            // The effects' pipelines draw the game's menu without transforms as the screens' do.
            let mut game = Canvas::default();
            crate::canvas::game_screen(
                &mut game,
                gpu::GLYPHS,
                gpu::ICONS,
                IMAGE_SIZE.0 as f32,
                IMAGE_SIZE.1 as f32,
            );
            let pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
                gpu::frame(cx.gpu, layout, &game, target, None).map(|_| ())
            })?;
            ok &= compare_with(cx, name, "fx-game", "game", &pixels)?;
        }
    }
    if cx.frames == 0 {
        return Ok(ok);
    }
    let timer = Timer::new(cx.gpu);
    println!(
        "\n## times, {} frames after {} of warm-up; microseconds",
        cx.frames, cx.warm_up
    );
    times_header();
    for size in SIZES {
        let target = Target::new(cx.gpu, size, false);
        target.clear(cx.gpu)?;
        let canvas = scene(size);
        for (name, made) in &mut layouts {
            let layout = made[0].as_mut();
            let samples = measure(cx, |_| {
                one_pass(cx.gpu, timer.as_ref(), layout, &canvas, &target.view, size)
            })?;
            times_row(
                cx,
                name,
                effect.name(),
                "-",
                size,
                canvas.items.len(),
                &samples,
            );
        }
    }
    Ok(ok)
}

/// The group's effect at one size: its texture and its scene.
struct GroupSetup {
    view: wgpu::TextureView,
    scene: scenes::GroupScene,
}

impl GroupSetup {
    fn new(
        cx: &Context<'_>,
        (width, height): (u32, u32),
        layouts: &mut [Box<dyn FxLayout>],
    ) -> Self {
        let (w, h) = (width as f32, height as f32);
        let place = scenes::group_place(w, h);
        let view = blur::render_texture(cx.gpu, "group", (place.width as u32, place.height as u32));
        let mut views = cx.views();
        let layer = ImageId(views.len() as u32);
        views.push(&view);
        for layout in layouts {
            layout.set_images(cx.gpu, &views, &cx.images.sampler);
        }
        let scene = scenes::group_scene(gpu::GLYPHS, gpu::ICONS, layer, w, h);
        assert_eq!(scene.place, place);
        Self { view, scene }
    }

    /// A frame: the group drawn into its texture first if `redraw`, then the screen.
    fn frame(
        &self,
        gpu: &Gpu,
        timer: Option<&Timer>,
        layouts: &mut [Box<dyn FxLayout>],
        target: &wgpu::TextureView,
        size: (u32, u32),
        redraw: bool,
    ) -> Result<Sample, String> {
        let [group, screen] = layouts else {
            unreachable!("two layouts: the group's and the screen's")
        };
        let start = Instant::now();
        let mut bytes = 0;
        if redraw {
            bytes += group.upload(gpu, &self.scene.group, self.scene.size());
        }
        bytes += screen.upload(gpu, &self.scene.screen, size);
        let write = start.elapsed();
        let mut recorder = Recorder::new(gpu, timer);
        if redraw {
            recorder.pass(
                "group",
                &self.view,
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                |pass| group.encode(pass),
            );
        }
        recorder.pass("canvas", target, wgpu::LoadOp::Load, |pass| {
            screen.encode(pass)
        });
        Ok(Sample {
            build: Duration::ZERO,
            write,
            bytes,
            recorded: recorder.finish()?,
        })
    }
}

fn group(cx: &Context<'_>) -> Result<bool, String> {
    let mut layouts = fx_layouts(cx, 2)?;
    println!(
        "\n## images at {} × {}: the frame that draws the group again against S's; the frame that \
         reuses its texture against that frame",
        IMAGE_SIZE.0, IMAGE_SIZE.1
    );
    println!("config\tlayout\tscene\tlargest\tpixels > 1\tverdict");
    let mut ok = true;
    for (name, made) in &mut layouts {
        let setup = GroupSetup::new(cx, IMAGE_SIZE, made);
        let redrawn = image_of(cx.gpu, IMAGE_SIZE, |target| {
            setup
                .frame(cx.gpu, None, made, &target.view, IMAGE_SIZE, true)
                .map(|_| ())
        })?;
        let reused = image_of(cx.gpu, IMAGE_SIZE, |target| {
            setup
                .frame(cx.gpu, None, made, &target.view, IMAGE_SIZE, false)
                .map(|_| ())
        })?;
        let same = redrawn == reused;
        println!(
            "{}\t{name}\tgroup reused\t{}\t-\t{}",
            cx.gpu.label,
            if same { "identical bytes" } else { "differs" },
            if same { "ok" } else { "FAIL" }
        );
        ok &= same;
        ok &= compare(cx, name, "group", &redrawn)?;
    }
    if cx.frames == 0 {
        return Ok(ok);
    }
    let timer = Timer::with_passes(cx.gpu, 2);
    println!(
        "\n## times, {} frames after {} of warm-up; microseconds",
        cx.frames, cx.warm_up
    );
    times_header();
    let mut steps = Vec::new();
    for size in SIZES {
        let target = Target::new(cx.gpu, size, false);
        target.clear(cx.gpu)?;
        for (name, made) in &mut layouts {
            let setup = GroupSetup::new(cx, size, made);
            let items = setup.scene.screen.items.len() + setup.scene.group.items.len();
            for (variant, redraw) in [("group drawn", true), ("group reused", false)] {
                let samples = measure(cx, |_| {
                    setup.frame(cx.gpu, timer.as_ref(), made, &target.view, size, redraw)
                })?;
                times_row(cx, name, "group", variant, size, items, &samples);
                steps.push((*name, variant, size, samples));
            }
        }
    }
    println!("\n## steps: each pass's GPU time");
    steps_header();
    for (name, variant, size, samples) in &steps {
        steps_rows(cx, name, "group", variant, *size, samples);
    }
    Ok(ok)
}

/// The blur's effect at one size: the canvas's texture, each glass's backdrop, the scene.
struct BlurSetup {
    canvas: wgpu::TextureView,
    backdrops: Vec<Backdrop>,
    behind: Canvas,
    front: Canvas,
    copy: wgpu::BindGroup,
}

impl BlurSetup {
    fn new(
        cx: &Context<'_>,
        blur: &Blur,
        size: (u32, u32),
        layouts: &mut [Box<dyn FxLayout>],
    ) -> Self {
        let (w, h) = (size.0 as f32, size.1 as f32);
        let canvas = blur::render_texture(cx.gpu, "canvas", size);
        let radius = scenes::blur_radius(h);
        let backdrops: Vec<Backdrop> = scenes::glass_panels(w, h)
            .iter()
            .enumerate()
            .map(|(index, &panel)| {
                blur.backdrop(
                    cx.gpu,
                    &canvas,
                    size,
                    panel,
                    radius,
                    &format!("glass {index}"),
                )
            })
            .collect();
        let mut views = cx.views();
        let first = views.len() as u32;
        views.extend(backdrops.iter().map(|backdrop| &backdrop.result));
        for layout in layouts {
            layout.set_images(cx.gpu, &views, blur.sampler());
        }
        let images: [(ImageId, crate::canvas::PixelRect); 4] =
            std::array::from_fn(|index| (ImageId(first + index as u32), backdrops[index].region));
        let (behind, front) =
            scenes::blur_scene(gpu::GLYPHS, gpu::ICONS, &gpu::pictures(), &images, w, h);
        Self {
            copy: blur.copy_group(cx.gpu, &canvas),
            canvas,
            backdrops,
            behind,
            front,
        }
    }

    /// A frame: what lies behind into the canvas's texture, each glass's backdrop blurred, the
    /// rest of the canvas over it, then the canvas's texture copied onto `target`.
    #[allow(clippy::too_many_arguments)]
    fn frame(
        &self,
        gpu: &Gpu,
        timer: Option<&Timer>,
        blur: &Blur,
        layouts: &mut [Box<dyn FxLayout>],
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) -> Result<Sample, String> {
        let [behind, front] = layouts else {
            unreachable!("two layouts: behind the glasses and in front")
        };
        let start = Instant::now();
        let bytes = behind.upload(gpu, &self.behind, size) + front.upload(gpu, &self.front, size);
        let write = start.elapsed();
        let mut recorder = Recorder::new(gpu, timer);
        recorder.pass(
            "behind",
            &self.canvas,
            wgpu::LoadOp::Clear(gpu::CLEAR),
            |pass| behind.encode(pass),
        );
        for backdrop in &self.backdrops {
            for pass in &backdrop.passes {
                recorder.pass(
                    &pass.name,
                    blur.target(pass),
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    |render| blur.record(render, pass),
                );
            }
        }
        recorder.pass("front", &self.canvas, wgpu::LoadOp::Load, |pass| {
            front.encode(pass)
        });
        recorder.pass(
            "copy",
            target,
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            |pass| blur.record_copy(pass, &self.copy),
        );
        Ok(Sample {
            build: Duration::ZERO,
            write,
            bytes,
            recorded: recorder.finish()?,
        })
    }
}

fn blur(cx: &Context<'_>) -> Result<bool, String> {
    let blur = Blur::new(cx.gpu);
    cx.gpu.check()?;
    let mut layouts = fx_layouts(cx, 2)?;
    for size in SIZES {
        let (w, h) = (size.0 as f32, size.1 as f32);
        let radius = scenes::blur_radius(h);
        let sigma = blur::deviation(radius);
        let canvas = blur::render_texture(cx.gpu, "canvas", size);
        let backdrops = scenes::glass_panels(w, h)
            .map(|panel| blur.backdrop(cx.gpu, &canvas, size, panel, radius, "glass"));
        println!(
            "blur\t{}x{}\tradius {radius:.1} px\tdeviation {sigma:.1} px\thalvings {}\tpasses a \
             glass {}\tsamples a blur pass {}\tregions blurred {:?}",
            size.0,
            size.1,
            blur::levels(sigma),
            backdrops[0].passes.len(),
            2 * backdrops[0].taps - 1,
            backdrops.map(|backdrop| (backdrop.region.width as u32, backdrop.region.height as u32)),
        );
    }
    println!(
        "\n## images at {} × {}, against S's on vulkan",
        IMAGE_SIZE.0, IMAGE_SIZE.1
    );
    println!("config\tlayout\tscene\tlargest\tpixels > 1\tverdict");
    let mut ok = true;
    for (name, made) in &mut layouts {
        let setup = BlurSetup::new(cx, &blur, IMAGE_SIZE, made);
        let pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
            setup
                .frame(cx.gpu, None, &blur, made, &target.view, IMAGE_SIZE)
                .map(|_| ())
        })?;
        ok &= compare(cx, name, "blur", &pixels)?;
    }
    if cx.frames == 0 {
        return Ok(ok);
    }
    let timer = Timer::with_passes(cx.gpu, 32);
    println!(
        "\n## times, {} frames after {} of warm-up; microseconds; drawn onto a target that allows \
         nothing but being drawn into",
        cx.frames, cx.warm_up
    );
    times_header();
    let mut steps = Vec::new();
    for size in SIZES {
        let surface = Target::surface(cx.gpu, size);
        for (name, made) in &mut layouts {
            let setup = BlurSetup::new(cx, &blur, size, made);
            let samples = measure(cx, |_| {
                setup.frame(cx.gpu, timer.as_ref(), &blur, made, &surface.view, size)
            })?;
            let items = setup.behind.items.len() + setup.front.items.len();
            times_row(cx, name, "blur", "-", size, items, &samples);
            steps.push((*name, size, samples));
        }
    }
    println!("\n## steps: each step's GPU time, a glass's blur its passes summed");
    steps_header();
    for (name, size, samples) in &steps {
        steps_rows(cx, name, "blur", "-", *size, samples);
    }
    Ok(ok)
}

/// The screens' layouts that can write one item alone: S on `vulkan` alone.
fn rewrite_layouts(cx: &Context<'_>) -> Result<Vec<(&'static str, Box<dyn Rewrite>)>, String> {
    let mut layouts: Vec<(&'static str, Box<dyn Rewrite>)> = Vec::new();
    if cx.config == Config::Vulkan {
        layouts.push(("S", Box::new(storage::Storage::new(cx.gpu, cx.images)?)));
    }
    layouts.push(("I", Box::new(instances::Instances::new(cx.gpu, cx.images)?)));
    layouts.push(("T", Box::new(texture::Texture::new(cx.gpu, cx.images)?)));
    cx.gpu.check()?;
    Ok(layouts)
}

/// The frames of the growing screen at which images are compared.
const MOVES: usize = 5;

fn growing(count: usize, (width, height): (u32, u32), frame: usize) -> (Canvas, usize) {
    let mut canvas = Canvas::default();
    let moving = scenes::growing_screen(
        &mut canvas,
        gpu::GLYPHS,
        count,
        width as f32,
        height as f32,
        frame,
    );
    (canvas, moving)
}

fn grow(cx: &Context<'_>) -> Result<bool, String> {
    let mut layouts = rewrite_layouts(cx)?;
    let count = scenes::GROWING[1];
    println!(
        "\n## images at {} × {}, {count} items, the box moved {MOVES} times: the screen made and \
         written again against S's; only the box's bytes written against that",
        IMAGE_SIZE.0, IMAGE_SIZE.1
    );
    println!("config\tlayout\tscene\tlargest\tpixels > 1\tverdict");
    let mut ok = true;
    for (name, layout) in &mut layouts {
        let (rebuilt, _) = growing(count, IMAGE_SIZE, MOVES);
        let rebuilt_pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
            gpu::frame(cx.gpu, layout.as_mut(), &rebuilt, target, None).map(|_| ())
        })?;
        let (mut canvas, moving) = growing(count, IMAGE_SIZE, 0);
        layout.upload(cx.gpu, &canvas, IMAGE_SIZE);
        for frame in 1..=MOVES {
            scenes::move_box(
                &mut canvas,
                moving,
                IMAGE_SIZE.0 as f32,
                IMAGE_SIZE.1 as f32,
                frame,
            );
            layout.rewrite(cx.gpu, &canvas, moving);
        }
        let partial_pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
            let mut recorder = Recorder::new(cx.gpu, None);
            recorder.pass("canvas", &target.view, wgpu::LoadOp::Load, |pass| {
                layout.encode(pass)
            });
            recorder.finish().map(|_| ())
        })?;
        let same = rebuilt_pixels == partial_pixels;
        println!(
            "{}\t{name}\tgrow, the box's bytes alone\t{}\t-\t{}",
            cx.gpu.label,
            if same { "identical bytes" } else { "differs" },
            if same { "ok" } else { "FAIL" }
        );
        ok &= same;
        ok &= compare(cx, name, "grow", &rebuilt_pixels)?;
    }
    if cx.frames == 0 {
        return Ok(ok);
    }
    let timer = Timer::new(cx.gpu);
    println!(
        "\n## times, {} frames after {} of warm-up; microseconds; build: the canvas's items made",
        cx.frames, cx.warm_up
    );
    times_header();
    for count in scenes::GROWING {
        for size in SIZES {
            let (w, h) = (size.0 as f32, size.1 as f32);
            let target = Target::new(cx.gpu, size, false);
            target.clear(cx.gpu)?;
            for (name, layout) in &mut layouts {
                let samples = measure(cx, |frame| {
                    let start = Instant::now();
                    let (canvas, _) = growing(count, size, frame);
                    let build = start.elapsed();
                    let mut sample = one_pass(
                        cx.gpu,
                        timer.as_ref(),
                        layout.as_mut(),
                        &canvas,
                        &target.view,
                        size,
                    )?;
                    sample.build = build;
                    Ok(sample)
                })?;
                times_row(cx, name, "grow", "all written", size, count, &samples);
                let (mut canvas, moving) = growing(count, size, 0);
                layout.upload(cx.gpu, &canvas, size);
                let samples = measure(cx, |frame| {
                    let start = Instant::now();
                    scenes::move_box(&mut canvas, moving, w, h, frame + 1);
                    let build = start.elapsed();
                    let written = Instant::now();
                    let bytes = layout.rewrite(cx.gpu, &canvas, moving);
                    let write = written.elapsed();
                    let mut recorder = Recorder::new(cx.gpu, timer.as_ref());
                    recorder.pass("canvas", &target.view, wgpu::LoadOp::Load, |pass| {
                        layout.encode(pass)
                    });
                    Ok(Sample {
                        build,
                        write,
                        bytes,
                        recorded: recorder.finish()?,
                    })
                })?;
                times_row(cx, name, "grow", "box alone", size, count, &samples);
            }
        }
    }
    Ok(ok)
}
