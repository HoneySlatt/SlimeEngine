//! The effects, each chosen by `--effect` (README): a transform per item, a group drawn apart, a
//! blur of what lies behind, a path, and one element animated over a still screen that grows. Each
//! runs on every layout the device takes, the first four on the effects' pipelines (S on Vulkan
//! alone), the growing screen on the screens' pipelines. Each image at 1280 × 800 is compared with
//! S's on `vulkan`; each frame is timed at the three sizes, the layouts' frames interleaved.

use std::time::{Duration, Instant};

use crate::blur::{self, Backdrop, Blur};
use crate::canvas::{Canvas, ImageId};
use crate::gpu::{
    self, Config, FrameCost, FxLayout, Gpu, Images, Recorder, Rewrite, Target, Timer,
};
use crate::{
    COST_HEADER, Entry, IMAGE_SIZE, SIZES, TOLERANCE, cost_columns, difference, instances,
    instances_fx, interleaved, one_line, output_dir, path, reference_image, scenes, spread,
    storage, storage_fx, texture, texture_fx, varyings, write_png,
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

/// An effects' layout as the runs use it: one object for each canvas a frame draws.
type FxEntry = Entry<Vec<Box<dyn FxLayout>>>;

/// The effects' layouts the device takes, by name, each made `copies` times: S on `vulkan` alone,
/// where it must be built; elsewhere its error is printed.
fn fx_layouts(cx: &Context<'_>, copies: usize) -> Result<Vec<FxEntry>, String> {
    let mut layouts = Vec::new();
    for name in ["S", "I", "T"] {
        let made: Result<Vec<_>, String> = (0..copies).map(|_| make(cx.gpu, name)).collect();
        match (made, name, cx.config) {
            (Ok(made), "S", Config::Vulkan) | (Ok(made), "I" | "T", _) => {
                layouts.push(Entry::new(name, made))
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
/// `vulkan` (`vulkan-gpu` with `--any-adapter`), S's own being the reference. False if they
/// differ by more than [`TOLERANCE`].
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
    if layout == "S" && scene == reference {
        println!("{}\tS\t{scene}\treference\t-\tok", cx.gpu.label);
        return Ok(true);
    }
    let reference = reference_image(cx.gpu, reference)?;
    let (largest, over) = difference(&reference, pixels);
    let pass = largest <= TOLERANCE;
    println!(
        "{}\t{layout}\t{scene}\t{largest}\t{over}\t{}",
        cx.gpu.label,
        if pass { "ok" } else { "FAIL" }
    );
    Ok(pass)
}

fn images_header(cx: &Context<'_>) {
    println!(
        "{}\tlayout\tscene\tlargest\tpixels > 1\tverdict",
        cx.gpu.config_header()
    );
}

fn times_title(cx: &Context<'_>, more: &str) {
    println!(
        "\n## times, {} frames of each layout after {} of warm-up, in rounds of one frame of each \
         layout in turn; microseconds{more}",
        cx.frames, cx.warm_up
    );
    println!(
        "{}\tlayout\teffect\tvariant\tsize\titems\tdraws\tpipelines\tpasses\tbytes\tbuild p50\t\
         build p90\t{COST_HEADER}",
        cx.gpu.config_header()
    );
}

/// The interleaved frames of `layouts`: `warm_up + frames` rounds of one frame of each.
fn rounds<L>(
    cx: &Context<'_>,
    layouts: &mut [Entry<L>],
    mut frame: impl FnMut(&mut Entry<L>, usize, usize) -> Result<FrameCost, String>,
) -> Result<Vec<Vec<FrameCost>>, String> {
    interleaved(layouts.len(), cx.warm_up, cx.frames, |index, round| {
        frame(&mut layouts[index], index, round)
    })
}

fn times_row(
    cx: &Context<'_>,
    layout: &str,
    effect: &str,
    variant: &str,
    size: (u32, u32),
    items: usize,
    costs: &[FrameCost],
) {
    let builds: Vec<Duration> = costs.iter().map(|cost| cost.build).collect();
    let last = costs.last().expect("frames were measured");
    println!(
        "{}\t{layout}\t{effect}\t{variant}\t{}x{}\t{items}\t{}\t{}\t{}\t{}\t{}\t{}",
        cx.gpu.label,
        size.0,
        size.1,
        last.recorded.draws,
        last.recorded.pipelines,
        last.recorded.names.len(),
        last.bytes,
        spread(&builds),
        cost_columns(costs),
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
    costs: &[FrameCost],
) {
    let first = &costs[0].recorded;
    let mut steps: Vec<(String, usize)> = Vec::new();
    for name in &first.names {
        let step = step_of(name);
        match steps.iter_mut().find(|(known, _)| *known == step) {
            Some((_, count)) => *count += 1,
            None => steps.push((step, 1)),
        }
    }
    for (step, count) in steps {
        let times: Option<Vec<Duration>> = costs
            .iter()
            .map(|cost| {
                cost.recorded
                    .names
                    .iter()
                    .zip(&cost.recorded.passes)
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

fn steps_header(cx: &Context<'_>) {
    println!(
        "{}\tlayout\teffect\tvariant\tsize\tstep\tpasses\tgpu pass p50\tgpu pass p90",
        cx.gpu.config_header()
    );
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
    for entry in &mut layouts {
        entry.layout[0].set_images(cx.gpu, &views, &cx.images.sampler);
    }
    println!(
        "\n## images at {} × {}, against S's on {}: largest difference, pixels > 1",
        IMAGE_SIZE.0, IMAGE_SIZE.1, cx.gpu.reference
    );
    images_header(cx);
    let mut ok = true;
    let canvas = scene(IMAGE_SIZE);
    for entry in &mut layouts {
        let layout = entry.layout[0].as_mut();
        let pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
            gpu::frame(cx.gpu, layout, &mut entry.uploads, &canvas, target, None).map(|_| ())
        })?;
        ok &= compare(cx, entry.name, effect.name(), &pixels)?;
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
                gpu::frame(cx.gpu, layout, &mut entry.uploads, &game, target, None).map(|_| ())
            })?;
            ok &= compare_with(cx, entry.name, "fx-game", "game", &pixels)?;
        }
    }
    if cx.frames == 0 {
        return Ok(ok);
    }
    let timer = Timer::new(cx.gpu);
    times_title(cx, "");
    for size in SIZES {
        let target = Target::new(cx.gpu, size, false);
        target.clear(cx.gpu)?;
        let canvas = scene(size);
        let costs = rounds(cx, &mut layouts, |entry, _, _| {
            gpu::frame(
                cx.gpu,
                entry.layout[0].as_mut(),
                &mut entry.uploads,
                &canvas,
                &target,
                timer.as_ref(),
            )
        })?;
        for (entry, costs) in layouts.iter().zip(&costs) {
            times_row(
                cx,
                entry.name,
                effect.name(),
                "-",
                size,
                canvas.items.len(),
                costs,
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
        entry: &mut FxEntry,
        target: &wgpu::TextureView,
        size: (u32, u32),
        redraw: bool,
    ) -> Result<FrameCost, String> {
        let [group, screen] = entry.layout.as_mut_slice() else {
            unreachable!("two layouts: the group's and the screen's")
        };
        let uploads = &mut entry.uploads;
        let start = Instant::now();
        let mut bytes = 0;
        if redraw {
            bytes += group.upload(gpu, uploads, &self.scene.group, self.scene.size());
        }
        bytes += screen.upload(gpu, uploads, &self.scene.screen, size);
        uploads.flush(gpu)?;
        let write = start.elapsed();
        let mut recorder = Recorder::new(gpu, timer, uploads);
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
        Ok(FrameCost {
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
        "\n## images at {} × {}: the frame that draws the group again against S's on {}; the \
         frame that reuses its texture against that frame",
        IMAGE_SIZE.0, IMAGE_SIZE.1, cx.gpu.reference
    );
    images_header(cx);
    let mut ok = true;
    for entry in &mut layouts {
        let setup = GroupSetup::new(cx, IMAGE_SIZE, &mut entry.layout);
        let redrawn = image_of(cx.gpu, IMAGE_SIZE, |target| {
            setup
                .frame(cx.gpu, None, entry, &target.view, IMAGE_SIZE, true)
                .map(|_| ())
        })?;
        let reused = image_of(cx.gpu, IMAGE_SIZE, |target| {
            setup
                .frame(cx.gpu, None, entry, &target.view, IMAGE_SIZE, false)
                .map(|_| ())
        })?;
        let same = redrawn == reused;
        println!(
            "{}\t{}\tgroup reused\t{}\t-\t{}",
            cx.gpu.label,
            entry.name,
            if same { "identical bytes" } else { "differs" },
            if same { "ok" } else { "FAIL" }
        );
        ok &= same;
        ok &= compare(cx, entry.name, "group", &redrawn)?;
    }
    if cx.frames == 0 {
        return Ok(ok);
    }
    let timer = Timer::with_passes(cx.gpu, 2);
    times_title(cx, "");
    let mut steps = Vec::new();
    for size in SIZES {
        let target = Target::new(cx.gpu, size, false);
        target.clear(cx.gpu)?;
        // Each layout's own group texture, which its images name.
        let setups: Vec<GroupSetup> = layouts
            .iter_mut()
            .map(|entry| GroupSetup::new(cx, size, &mut entry.layout))
            .collect();
        for (variant, redraw) in [("group drawn", true), ("group reused", false)] {
            let costs = rounds(cx, &mut layouts, |entry, index, _| {
                setups[index].frame(cx.gpu, timer.as_ref(), entry, &target.view, size, redraw)
            })?;
            for ((entry, setup), costs) in layouts.iter().zip(&setups).zip(costs) {
                let items = setup.scene.screen.items.len() + setup.scene.group.items.len();
                times_row(cx, entry.name, "group", variant, size, items, &costs);
                steps.push((entry.name, variant, size, costs));
            }
        }
    }
    println!("\n## steps: each pass's GPU time");
    steps_header(cx);
    for (name, variant, size, costs) in &steps {
        steps_rows(cx, name, "group", variant, *size, costs);
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
        entry: &mut FxEntry,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) -> Result<FrameCost, String> {
        let [behind, front] = entry.layout.as_mut_slice() else {
            unreachable!("two layouts: behind the glasses and in front")
        };
        let uploads = &mut entry.uploads;
        let start = Instant::now();
        let bytes = behind.upload(gpu, uploads, &self.behind, size)
            + front.upload(gpu, uploads, &self.front, size);
        uploads.flush(gpu)?;
        let write = start.elapsed();
        let mut recorder = Recorder::new(gpu, timer, uploads);
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
        Ok(FrameCost {
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
        "\n## images at {} × {}, against S's on {}",
        IMAGE_SIZE.0, IMAGE_SIZE.1, cx.gpu.reference
    );
    images_header(cx);
    let mut ok = true;
    for entry in &mut layouts {
        let setup = BlurSetup::new(cx, &blur, IMAGE_SIZE, &mut entry.layout);
        let pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
            setup
                .frame(cx.gpu, None, &blur, entry, &target.view, IMAGE_SIZE)
                .map(|_| ())
        })?;
        ok &= compare(cx, entry.name, "blur", &pixels)?;
    }
    if cx.frames == 0 {
        return Ok(ok);
    }
    let timer = Timer::with_passes(cx.gpu, 32);
    times_title(
        cx,
        "; drawn onto a target that allows nothing but being drawn into",
    );
    let mut steps = Vec::new();
    for size in SIZES {
        let surface = Target::surface(cx.gpu, size);
        // Each layout's own canvas texture and backdrops, which its images name.
        let setups: Vec<BlurSetup> = layouts
            .iter_mut()
            .map(|entry| BlurSetup::new(cx, &blur, size, &mut entry.layout))
            .collect();
        let costs = rounds(cx, &mut layouts, |entry, index, _| {
            setups[index].frame(cx.gpu, timer.as_ref(), &blur, entry, &surface.view, size)
        })?;
        for ((entry, setup), costs) in layouts.iter().zip(&setups).zip(costs) {
            let items = setup.behind.items.len() + setup.front.items.len();
            times_row(cx, entry.name, "blur", "-", size, items, &costs);
            steps.push((entry.name, size, costs));
        }
    }
    println!("\n## steps: each step's GPU time, a glass's blur its passes summed");
    steps_header(cx);
    for (name, size, costs) in &steps {
        steps_rows(cx, name, "blur", "-", *size, costs);
    }
    Ok(ok)
}

/// The screens' layouts that can write one item alone: S on `vulkan` alone.
fn rewrite_layouts(cx: &Context<'_>) -> Result<Vec<Entry<Box<dyn Rewrite>>>, String> {
    let mut layouts: Vec<Entry<Box<dyn Rewrite>>> = Vec::new();
    if cx.config == Config::Vulkan {
        let s = storage::Storage::new(cx.gpu, cx.images, "S", storage::SOURCE)?;
        layouts.push(Entry::new("S", Box::new(s)));
    }
    let i = instances::Instances::new(cx.gpu, cx.images)?;
    layouts.push(Entry::new("I", Box::new(i)));
    let t = texture::Texture::new(cx.gpu, cx.images)?;
    layouts.push(Entry::new("T", Box::new(t)));
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

/// A frame of the growing screen that writes the bytes of the box `moving` of `canvas` alone.
fn box_alone(
    gpu: &Gpu,
    timer: Option<&Timer>,
    entry: &mut Entry<Box<dyn Rewrite>>,
    canvas: &Canvas,
    moving: usize,
    target: &Target,
) -> Result<FrameCost, String> {
    let start = Instant::now();
    let bytes = entry.layout.rewrite(&mut entry.uploads, canvas, moving);
    entry.uploads.flush(gpu)?;
    let write = start.elapsed();
    let layout = &entry.layout;
    let mut recorder = Recorder::new(gpu, timer, &mut entry.uploads);
    recorder.pass("canvas", &target.view, wgpu::LoadOp::Load, |pass| {
        layout.encode(pass)
    });
    Ok(FrameCost {
        build: Duration::ZERO,
        write,
        bytes,
        recorded: recorder.finish()?,
    })
}

fn grow(cx: &Context<'_>) -> Result<bool, String> {
    let mut layouts = rewrite_layouts(cx)?;
    let count = scenes::GROWING[1];
    println!(
        "\n## images at {} × {}, {count} items, the box moved {MOVES} times: the screen made and \
         written again against S's on {}; only the box's bytes written against that",
        IMAGE_SIZE.0, IMAGE_SIZE.1, cx.gpu.reference
    );
    images_header(cx);
    let mut ok = true;
    for entry in &mut layouts {
        let (rebuilt, _) = growing(count, IMAGE_SIZE, MOVES);
        let rebuilt_pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
            gpu::frame(
                cx.gpu,
                entry.layout.as_mut(),
                &mut entry.uploads,
                &rebuilt,
                target,
                None,
            )
            .map(|_| ())
        })?;
        let (mut canvas, moving) = growing(count, IMAGE_SIZE, 0);
        let partial_pixels = image_of(cx.gpu, IMAGE_SIZE, |target| {
            // The whole screen written, then the box's bytes alone after each move: the copies
            // run in this order, at the start of the frame.
            entry
                .layout
                .upload(cx.gpu, &mut entry.uploads, &canvas, IMAGE_SIZE);
            for frame in 1..=MOVES {
                scenes::move_box(
                    &mut canvas,
                    moving,
                    IMAGE_SIZE.0 as f32,
                    IMAGE_SIZE.1 as f32,
                    frame,
                );
                entry.layout.rewrite(&mut entry.uploads, &canvas, moving);
            }
            entry.uploads.flush(cx.gpu)?;
            let layout = &entry.layout;
            let mut recorder = Recorder::new(cx.gpu, None, &mut entry.uploads);
            recorder.pass("canvas", &target.view, wgpu::LoadOp::Load, |pass| {
                layout.encode(pass)
            });
            recorder.finish().map(|_| ())
        })?;
        let same = rebuilt_pixels == partial_pixels;
        println!(
            "{}\t{}\tgrow, the box's bytes alone\t{}\t-\t{}",
            cx.gpu.label,
            entry.name,
            if same { "identical bytes" } else { "differs" },
            if same { "ok" } else { "FAIL" }
        );
        ok &= same;
        ok &= compare(cx, entry.name, "grow", &rebuilt_pixels)?;
    }
    if cx.frames == 0 {
        return Ok(ok);
    }
    let timer = Timer::new(cx.gpu);
    times_title(cx, "; build: the canvas's items made");
    for count in scenes::GROWING {
        for size in SIZES {
            let (w, h) = (size.0 as f32, size.1 as f32);
            let target = Target::new(cx.gpu, size, false);
            target.clear(cx.gpu)?;
            let costs = rounds(cx, &mut layouts, |entry, _, round| {
                let start = Instant::now();
                let (canvas, _) = growing(count, size, round);
                let build = start.elapsed();
                let mut cost = gpu::frame(
                    cx.gpu,
                    entry.layout.as_mut(),
                    &mut entry.uploads,
                    &canvas,
                    &target,
                    timer.as_ref(),
                )?;
                cost.build = build;
                Ok(cost)
            })?;
            for (entry, costs) in layouts.iter().zip(&costs) {
                times_row(cx, entry.name, "grow", "all written", size, count, costs);
            }
            // Each layout's own screen, written whole once, then its box alone each frame.
            let mut screens = Vec::with_capacity(layouts.len());
            for entry in &mut layouts {
                let (canvas, moving) = growing(count, size, 0);
                gpu::frame(
                    cx.gpu,
                    entry.layout.as_mut(),
                    &mut entry.uploads,
                    &canvas,
                    &target,
                    None,
                )?;
                screens.push((canvas, moving));
            }
            let costs = rounds(cx, &mut layouts, |entry, index, round| {
                let (canvas, moving) = &mut screens[index];
                let start = Instant::now();
                scenes::move_box(canvas, *moving, w, h, round + 1);
                let build = start.elapsed();
                let mut cost = box_alone(cx.gpu, timer.as_ref(), entry, canvas, *moving, &target)?;
                cost.build = build;
                Ok(cost)
            })?;
            for (entry, costs) in layouts.iter().zip(&costs) {
                times_row(cx, entry.name, "grow", "box alone", size, count, costs);
            }
        }
    }
    Ok(ok)
}
