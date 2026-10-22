//! A view whose new size does not fit in the GPU's memory
//! (`docs/adr/0126-views-apart-from-the-renderer.md`, G18): it returns the error and keeps no
//! targets, its old ones given back first. Alone in its test binary: on lavapipe, the memory budget
//! that makes the memory run out is a share of what the whole machine uses, which the other tests
//! of a binary would change while it runs. Runs on whatever adapter the machine offers, on those
//! whose memory wgpu counts.

#[path = "../../slime-gfx/tests/common/memory_budget.rs"]
mod memory_budget;

use memory_budget::{CHILD_ENV, PROBE, run_out_of_memory_in_a_child};
use slime_gfx::{GfxError, Gpu, LossKind, MEMORY_BUDGET_ENV_VAR, RenderTarget, TextureFormat};
use slime_render::{Camera, RESIZE_DELAY, RenderError, Renderer, Scene, ViewDraw};

const MIB: u64 = 1 << 20;

/// G18: a view whose new size does not fit in the GPU's memory returns the error, and keeps no
/// targets, rather than its old ones: it gave them back first. The memory runs out in a child
/// process with a memory budget, as `slime-gfx`'s tests make it run out
/// (`tests/common/memory_budget.rs`), at a budget that holds the view's frame at 1024 ×
/// 1024, about 55 MiB, and not at 8192 × 4096, about 1.9 GiB: a wide window, which leaves room for
/// what wgpu allocates for itself, such as the copies of the values written, whose failure gives
/// the device up, which a budget too small for the test causes.
#[test]
fn a_view_whose_new_size_does_not_fit_keeps_no_targets() {
    const NAME: &str = "a_view_whose_new_size_does_not_fit_keeps_no_targets";
    let Ok(mode) = std::env::var(CHILD_ENV) else {
        let Some(stdout) = run_out_of_memory_in_a_child(NAME) else {
            eprintln!("skipped: no memory budget makes the memory of this driver run out");
            return;
        };
        assert!(stdout.contains("no targets kept"), "{stdout}");
        assert!(stdout.contains("the view goes on"), "{stdout}");
        return;
    };
    let gpu = match Gpu::new() {
        Ok(gpu) => gpu,
        Err(GfxError::RequestDevice(_)) => {
            println!("no device");
            return;
        }
        Err(error) => panic!("a GPU adapter must be available: {error}"),
    };
    if gpu.memory_in_use().is_none() {
        println!("never ran out");
        return;
    }
    // Whether a frame was drawn: `Some(false)` when the memory ran out, `None` when an allocation
    // of wgpu's own passed the budget and gave the device up, which wgpu reports as the driver's
    // loss, `Out of memory`.
    let fits = |result: Result<(), RenderError>| match result {
        Ok(()) => Some(true),
        Err(RenderError::Gfx(GfxError::OutOfMemory { .. })) => Some(false),
        Err(RenderError::Gfx(GfxError::DeviceLost { kind, message }))
            if kind == LossKind::OutOfMemory || message == "Out of memory" =>
        {
            None
        }
        Err(error) => panic!("{error}"),
    };
    let format = TextureFormat::Rgba8UnormSrgb;
    let made = (
        Renderer::new(&gpu, format),
        gpu.create_render_target("small", 1024, 1024, format),
        gpu.create_render_target("large", 8192, 4096, format),
    );
    let (Ok(mut renderer), Ok(small), Ok(large)) = made else {
        println!("no device");
        return;
    };
    let Ok(mut view) = renderer.create_view(&gpu, "view") else {
        println!("no device");
        return;
    };
    // Again once the size has held still, when the view makes its targets at that size.
    let mut draw = |renderer: &mut Renderer, target: &RenderTarget| loop {
        let drawn = renderer.render(
            &gpu,
            &Scene::new(Vec::new()),
            &mut [ViewDraw::new(&mut view, Camera::default(), target)],
        );
        let drawn = gpu.wait_idle().map_err(RenderError::from).and(drawn);
        if drawn.is_err() || view.render_size() == Some((target.width(), target.height())) {
            return drawn;
        }
        std::thread::sleep(RESIZE_DELAY);
    };
    let without_targets = gpu.memory_in_use().expect("counted");
    let Some(true) = fits(draw(&mut renderer, &small)) else {
        println!("no device");
        return;
    };
    let held = gpu.memory_in_use().expect("counted");
    let targets = held - without_targets;
    match fits(draw(&mut renderer, &large)) {
        Some(true) => {
            println!("never ran out");
            return;
        }
        Some(false) => {}
        None => {
            println!("no device");
            return;
        }
    }
    if mode == PROBE {
        return;
    }
    let after = gpu.memory_in_use().expect("counted");
    println!(
        "{} MiB of targets, {} MiB in use with them, {} after the failure",
        targets / MIB,
        held / MIB,
        after / MIB
    );
    // But a few kilobytes that wgpu stages for the values written.
    assert!(
        after + targets <= held + MIB,
        "the old targets are given back: {after} bytes, {targets} of them, {held} before"
    );
    println!("no targets kept");
    assert_eq!(
        fits(draw(&mut renderer, &small)),
        Some(true),
        "the small size fits again"
    );
    println!("the view goes on");
}
