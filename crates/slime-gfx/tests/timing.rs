//! Timing passes on the GPU: a timed pass reports a duration a few frames later, without
//! waiting, on adapters that support timestamp queries; others give no timer. Runs on whatever
//! adapter the machine offers and prints what it found, so that the checks on lavapipe and the
//! Windows CI job on WARP tell what software drivers support.

use std::time::Duration;

use slime_gfx::{Color, Gpu, RenderPassDescriptor, TextureFormat};

const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

#[test]
fn a_timed_pass_reports_how_long_it_took_when_the_adapter_can_tell() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    eprintln!(
        "adapter `{}` can time its work: {}",
        gpu.adapter_name(),
        gpu.can_time()
    );
    let Some(timer) = gpu.create_timer("test") else {
        assert!(!gpu.can_time(), "an adapter that can time gives a timer");
        return;
    };
    assert!(timer.latest(&gpu).is_none(), "nothing has been timed yet");

    let target = gpu.create_render_target("timed", 64, 64, TextureFormat::Rgba8UnormSrgb);
    let mut measured = None;
    for frame in 0..8 {
        let pass = RenderPassDescriptor {
            target: &target,
            clear: Some(BLACK),
            depth: None,
            timer: Some(&timer),
        };
        gpu.render(&pass, |_pass| {});
        // Waiting is not needed to get a time back, but makes the test deterministic.
        gpu.wait_idle().expect("the GPU finishes the pass");
        measured = timer.latest(&gpu);
        if measured.is_some() {
            eprintln!(
                "time of a pass that clears 64x64 pixels, read at frame {frame}: {measured:?}"
            );
            break;
        }
    }
    let time = measured.expect("the time of a timed pass comes back within a few frames");
    assert!(
        time < Duration::from_secs(1),
        "a pass that clears 64x64 pixels takes less than a second: {time:?}"
    );
}
