//! Timing passes on the GPU: a timed pass reports a duration a few frames later, without
//! waiting, on adapters that support timestamp queries; others give no timer. Several passes of a
//! frame, a depth-only one among them, are timed by a timer each, or by one timer when they share a
//! submission. Runs on whatever adapter the machine offers and prints what it found, so that the
//! checks on lavapipe and the Windows CI job on WARP tell what software drivers support.

use std::time::Duration;

use slime_gfx::{DepthPassDescriptor, Gpu, LinearColor, RenderPassDescriptor, TextureFormat};

/// Checks that `time` is one that a pass of these tests can take: more than nothing and less than
/// a second. A timer whose timestamps are written in the wrong order, or not read at all, gives 0;
/// passes like these took at least 1 µs on lavapipe, RADV and WARP. The only true 0 is a
/// pass that runs across a wrap of the timestamp counter, about once an hour on Intel's Vulkan
/// driver (`docs/PLAN.md`, known debt), which passes this short are very unlikely to meet.
#[track_caller]
fn assert_plausible(time: Duration) {
    assert!(
        !time.is_zero(),
        "a timed pass takes some time; 0 means the timer measured nothing"
    );
    assert!(
        time < Duration::from_secs(1),
        "a pass that clears 64x64 pixels takes less than a second: {time:?}"
    );
}

#[test]
fn a_timed_pass_reports_how_long_it_took_when_the_adapter_can_tell() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    eprintln!(
        "adapter `{}` can time its work: {}",
        gpu.adapter_name(),
        gpu.can_time()
    );
    let Some(timer) = gpu
        .create_timer("test", 1)
        .expect("the GPU has memory for it")
    else {
        assert!(!gpu.can_time(), "an adapter that can time gives a timer");
        return;
    };
    assert!(timer.latest(0).is_none(), "nothing has been timed yet");

    let target = gpu
        .create_render_target("timed", 64, 64, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let mut measured = None;
    for frame in 0..8 {
        let pass = RenderPassDescriptor {
            target: &target,
            clear: Some(LinearColor::BLACK),
            depth: None,
            timer: Some(&timer),
            resolve: None,
        };
        gpu.render(&pass, |_pass| {});
        // Waiting is not needed to get a time back, but makes the test deterministic.
        gpu.wait_idle().expect("the GPU finishes the pass");
        measured = timer.latest(0);
        if measured.is_some() {
            eprintln!(
                "time of a pass that clears 64x64 pixels, read at frame {frame}: {measured:?}"
            );
            break;
        }
    }
    assert_plausible(measured.expect("the time of a timed pass comes back within a few frames"));
}

#[test]
fn a_time_comes_back_with_later_frames_without_waiting() {
    // What the renderer relies on: it never waits for the GPU, and reads its timer after each
    // frame's submission, which is when wgpu collects the passes the GPU has finished.
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let Some(timer) = gpu
        .create_timer("frames", 1)
        .expect("the GPU has memory for it")
    else {
        assert!(!gpu.can_time(), "an adapter that can time gives a timer");
        eprintln!("adapter `{}` cannot time its work", gpu.adapter_name());
        return;
    };
    let target = gpu
        .create_render_target("timed", 64, 64, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: Some(&timer),
        resolve: None,
    };
    // Frames a millisecond apart, so that the GPU, however slow, finishes a pass well before the
    // last of them.
    let frames = 1000;
    let mut measured = None;
    for frame in 0..frames {
        gpu.render(&pass, |_pass| {});
        measured = timer.latest(0);
        if measured.is_some() {
            eprintln!("time of a pass, read without waiting at frame {frame}: {measured:?}");
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_plausible(
        measured.unwrap_or_else(|| panic!("no time came back within {frames} frames")),
    );
}

#[test]
fn three_passes_timed_in_one_frame_each_get_their_own_time() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let Some(shadow) = gpu
        .create_timer("shadow", 1)
        .expect("the GPU has memory for it")
    else {
        assert!(!gpu.can_time(), "an adapter that can time gives a timer");
        eprintln!("adapter `{}` cannot time its work", gpu.adapter_name());
        return;
    };
    let scene = gpu
        .create_timer("scene", 1)
        .expect("the GPU has memory for it")
        .expect("the adapter can time");
    let tone_mapping = gpu
        .create_timer("tone mapping", 1)
        .expect("the GPU has memory for it")
        .expect("the adapter can time");
    let depth = gpu
        .create_depth_target("shadow map", 64, 64)
        .expect("the GPU has memory for it");
    let hdr = gpu
        .create_render_target("hdr", 64, 64, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let output = gpu
        .create_render_target("output", 64, 64, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");

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
                clear: Some(LinearColor::BLACK),
                depth: None,
                timer: Some(timer),
                resolve: None,
            };
            gpu.render(&pass, |_pass| {});
        }
        // Waiting is not needed to get the times back, but makes the test deterministic.
        gpu.wait_idle().expect("the GPU finishes the passes");
        for (time, timer) in times.iter_mut().zip([&shadow, &scene, &tone_mapping]) {
            *time = time.or(timer.latest(0));
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
        assert_plausible(time.expect("each timed pass gets its time back within a few frames"));
    }
}

#[test]
fn three_passes_of_one_submission_each_get_their_own_time_from_one_timer() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let Some(timer) = gpu
        .create_timer("frame", 3)
        .expect("the GPU has memory for it")
    else {
        assert!(!gpu.can_time(), "an adapter that can time gives a timer");
        eprintln!("adapter `{}` cannot time its work", gpu.adapter_name());
        return;
    };
    let depth = gpu
        .create_depth_target("shadow map", 64, 64)
        .expect("the GPU has memory for it");
    let hdr = gpu
        .create_render_target("hdr", 64, 64, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let output = gpu
        .create_render_target("output", 64, 64, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");

    let mut times = [None; 3];
    for _frame in 0..8 {
        gpu.submit("frame", |commands| {
            commands.render_depth(
                &DepthPassDescriptor {
                    depth: &depth,
                    timer: Some(&timer),
                },
                |_pass| {},
            );
            for target in [&hdr, &output] {
                let pass = RenderPassDescriptor {
                    target,
                    clear: Some(LinearColor::BLACK),
                    depth: None,
                    timer: Some(&timer),
                    resolve: None,
                };
                commands.render(&pass, |_pass| {});
            }
        });
        // Waiting is not needed to get the times back, but makes the test deterministic.
        gpu.wait_idle().expect("the GPU finishes the passes");
        times = [timer.latest(0), timer.latest(1), timer.latest(2)];
        if times.iter().all(Option::is_some) {
            break;
        }
    }
    eprintln!(
        "adapter `{}`: shadow, scene and tone-mapping passes of one submission timed {times:?}",
        gpu.adapter_name()
    );
    for time in times {
        assert_plausible(time.expect("each timed pass gets its time back within a few frames"));
    }
}
