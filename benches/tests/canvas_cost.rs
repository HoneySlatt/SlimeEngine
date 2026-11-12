//! What the renderer costs an application that draws only interfaces, against the canvas alone
//! (ADR-0180): the time to build each and draw a first canvas, and the memory and GPU memory they
//! take. Ignored by `cargo test`: `scripts/canvas-cost.sh` runs each part in a process of its own
//! and gathers the figures. Here, among the benchmarks, the one client of the engine that holds
//! both the renderer and uwui's canvas
//! (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`).

use std::time::Instant;

use slime_app::InterfaceRenderer;
use slime_color::LinearColor;
use slime_gfx::{Gpu, TextureFormat};
use slime_render::Renderer;
use uwui_render::canvas::{Canvas, CanvasBox, CanvasGradient, CanvasShadow, PixelRect};

const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// The process's resident memory (`VmRSS`), in KiB.
fn resident_kib() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("the process reads itself");
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|value| value.trim().strip_suffix("kB"))
        .and_then(|value| value.trim().parse().ok())
        .expect("a resident size in kB")
}

/// The video memory that the process asked for (`drm-total-vram`), in KiB, summed over the files
/// through which it opened a GPU, each client of a device once; `None` where the driver gives no
/// figure, as lavapipe opens no GPU. It counts what the driver keeps, the shaders' code among it,
/// which [`Gpu::memory_in_use`] does not. The kernel writes bytes, `KiB`, `MiB` or `GiB`
/// (`drm_print_memory_stats`), as `crates/slime-mcp/tests/resources.rs` reads them too.
fn video_kib() -> Option<u64> {
    let mut clients = std::collections::BTreeMap::new();
    for file in std::fs::read_dir("/proc/self/fd").ok()?.flatten() {
        let opens_a_gpu =
            std::fs::read_link(file.path()).is_ok_and(|target| target.starts_with("/dev/dri"));
        let info = format!("/proc/self/fdinfo/{}", file.file_name().to_string_lossy());
        let Some(info) = opens_a_gpu
            .then(|| std::fs::read_to_string(info).ok())
            .flatten()
        else {
            continue;
        };
        let field = |name: &str| {
            info.lines()
                .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
                .map(str::trim)
        };
        let (Some(client), Some(size)) = (field("drm-client-id"), field("drm-total-vram")) else {
            continue;
        };
        let (figure, unit) = size.split_once(' ').unwrap_or((size, ""));
        let figure: u64 = figure.parse().expect("a size");
        let kib = match unit {
            "" => figure.div_ceil(1024),
            "KiB" => figure,
            "MiB" => figure * 1024,
            "GiB" => figure * 1024 * 1024,
            unit => panic!("an unknown unit {unit:?}"),
        };
        clients.insert(
            (field("drm-pdev").map(str::to_owned), client.to_owned()),
            kib,
        );
    }
    (!clients.is_empty()).then(|| clients.values().sum())
}

/// A canvas of a few boxes, enough to use the canvas's pipeline once.
fn first_canvas() -> Canvas {
    let rect = PixelRect::new(40.0, 40.0, 400.0, 240.0);
    let mut canvas = Canvas::new();
    canvas.draw_shadow(&CanvasShadow {
        rect,
        radii: [12.0; 4],
        color: LinearColor::BLACK,
        offset: [0.0, 6.0],
        blur: 18.0,
        spread: 0.0,
        clip: None,
    });
    canvas.draw_gradient(&CanvasGradient {
        rect,
        radii: [12.0; 4],
        corners: [LinearColor::rgb(0.2, 0.3, 0.8); 4],
        clip: None,
    });
    canvas.draw_box(&CanvasBox {
        rect: PixelRect::new(60.0, 60.0, 120.0, 32.0),
        radii: [6.0; 4],
        fill: LinearColor::rgb(0.9, 0.9, 0.9),
        border_width: 1.0,
        border: LinearColor::BLACK,
        clip: None,
    });
    canvas
}

/// Builds `which` on a device already open, draws a first canvas, and prints what both took.
fn measure(which: &str) {
    let resident_at_start = resident_kib();
    let opened = Instant::now();
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let open_ms = opened.elapsed().as_secs_f64() * 1e3;
    let target = gpu
        .create_render_target("cost", 1280, 800, FORMAT)
        .expect("the GPU has memory for it");
    gpu.wait_idle().expect("the GPU finishes");
    let resident_before = resident_kib();
    let gpu_before = gpu.memory_in_use();
    let video_before = video_kib();

    let started = Instant::now();
    // What draws the first canvas: nothing, the canvas alone, through the engine's bridge to uwui,
    // or the same beside the whole renderer, as an application that draws its interfaces with the
    // renderer made would. The bridge's atlas makes no GPU object before a glyph asks for one.
    let canvas_renderer = || InterfaceRenderer::new(&gpu, FORMAT).expect("the canvas is built");
    let (renderer, mut drawer) = match which {
        "device" => (None, None),
        "canvas" => (None, Some(canvas_renderer())),
        "renderer" => (
            Some(Renderer::new(&gpu, FORMAT).expect("the renderer is built")),
            Some(canvas_renderer()),
        ),
        _ => panic!("no part {which}"),
    };
    let built_ms = started.elapsed().as_secs_f64() * 1e3;
    let canvas = first_canvas();
    if let Some(drawer) = &mut drawer {
        drawer
            .draw_canvas(&gpu, &canvas, &[], &target)
            .expect("drawn");
    }
    gpu.wait_idle().expect("the GPU finishes");
    let total_ms = started.elapsed().as_secs_f64() * 1e3;
    let gpu_kib = match (gpu_before, gpu.memory_in_use()) {
        (Some(before), Some(after)) => (after.saturating_sub(before) / 1024).to_string(),
        _ => "-".to_owned(),
    };
    let video = match (video_before, video_kib()) {
        (Some(before), Some(after)) => after.saturating_sub(before).to_string(),
        _ => "-".to_owned(),
    };
    println!(
        "cost {which} adapter={:?} open_ms={open_ms:.2} device_and_target_resident_kib={} device_and_target_video_kib={} built_ms={built_ms:.2} \
         first_canvas_ms={total_ms:.2} resident_kib={} gpu_kib={gpu_kib} video_kib={video}",
        gpu.adapter_name(),
        resident_before.saturating_sub(resident_at_start),
        video_before.map_or("-".to_owned(), |kib| kib.to_string()),
        resident_kib().saturating_sub(resident_before),
    );
    drop((renderer, drawer));
}

#[test]
#[ignore = "a measure, run by scripts/canvas-cost.sh"]
fn cost_of_the_device() {
    measure("device");
}

#[test]
#[ignore = "a measure, run by scripts/canvas-cost.sh"]
fn cost_of_the_canvas() {
    measure("canvas");
}

#[test]
#[ignore = "a measure, run by scripts/canvas-cost.sh"]
fn cost_of_the_renderer() {
    measure("renderer");
}
