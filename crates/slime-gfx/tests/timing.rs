//! Timing passes on the GPU: a timed pass reports a duration a few frames later, without
//! waiting, on adapters that support timestamp queries; others give no timer. Several passes of a
//! frame, a depth-only one among them, are timed by a timer each. Runs on whatever adapter the
//! machine offers and prints what it found, so that the checks on lavapipe and the Windows CI job
//! on WARP tell what software drivers support.

use std::time::Duration;

use slime_gfx::{Color, DepthPassDescriptor, Gpu, RenderPassDescriptor, TextureFormat};

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

#[test]
fn three_passes_timed_in_one_frame_each_get_their_own_time() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let Some(shadow) = gpu.create_timer("shadow") else {
        assert!(!gpu.can_time(), "an adapter that can time gives a timer");
        eprintln!("adapter `{}` cannot time its work", gpu.adapter_name());
        return;
    };
    let scene = gpu.create_timer("scene").expect("the adapter can time");
    let tone_mapping = gpu
        .create_timer("tone mapping")
        .expect("the adapter can time");
    let depth = gpu.create_depth_target("shadow map", 64, 64);
    let hdr = gpu.create_render_target("hdr", 64, 64, TextureFormat::Rgba16Float);
    let output = gpu.create_render_target("output", 64, 64, TextureFormat::Rgba8UnormSrgb);

    let mut times = [None; 3];
    for _frame in 0..8 {
        gpu.render_depth(
            &DepthPassDescriptor {
                depth: &depth,
                timer: Some(&shadow),
            },
            |_pass| {},
        );
        for (target, timer) in [(&hdr, &scene), (&output, &tone_mapping)] {
            let pass = RenderPassDescriptor {
                target,
                clear: Some(BLACK),
                depth: None,
                timer: Some(timer),
            };
            gpu.render(&pass, |_pass| {});
        }
        // Waiting is not needed to get the times back, but makes the test deterministic.
        gpu.wait_idle().expect("the GPU finishes the passes");
        for (time, timer) in times.iter_mut().zip([&shadow, &scene, &tone_mapping]) {
            *time = time.or(timer.latest(&gpu));
        }
        if times.iter().all(Option::is_some) {
            break;
        }
    }
    eprintln!(
        "adapter `{}`: shadow, scene and tone-mapping passes timed {times:?}",
        gpu.adapter_name()
    );
    for time in times {
        let time = time.expect("each timed pass gets its time back within a few frames");
        assert!(time < Duration::from_secs(1), "{time:?}");
    }
}
