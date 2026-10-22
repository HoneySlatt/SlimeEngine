//! G18: a view resized never holds the targets of both sizes at once. On lavapipe, the Vulkan
//! driver that renders on the CPU, a GPU object's memory is the process's: the most address space
//! that the process ever held, `VmPeak` in `/proc/self/status`, measures the peak of a resize
//! exactly, which is why this test is alone in its file, its process holding nothing else. Linux
//! only; on a GPU whose memory is not the process's, it measures nothing, and says so.

#[cfg(target_os = "linux")]
#[path = "common/settled.rs"]
mod settled;

#[cfg(target_os = "linux")]
#[test]
fn a_view_resized_never_holds_the_targets_of_both_sizes() {
    use slime_gfx::{Gpu, RenderTarget, TextureFormat};
    use slime_render::glam::{DAffine3, DQuat, DVec3, Vec3};
    use slime_render::{
        Camera, DirectionalLight, Instance, Light, MaterialData, MeshData, Renderer, Scene, View,
    };

    /// MiB of address space that the process holds now, and the most that it ever held.
    fn address_space() -> (u64, u64) {
        let status = std::fs::read_to_string("/proc/self/status").expect("the process's status");
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .and_then(|value| value.split_whitespace().next()?.parse::<u64>().ok())
                .expect("a field of the status")
                / 1024
        };
        (field("VmSize:"), field("VmPeak:"))
    }

    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let format = TextureFormat::Rgba16Float;
    let mut renderer = Renderer::new(&gpu, format).expect("the renderer is created");
    let mut view = renderer
        .create_view(&gpu, "view")
        .expect("the view is created");
    let square = renderer
        .create_mesh(&gpu, "square", &MeshData::square())
        .expect("the square is valid");
    let white = renderer
        .create_material(&gpu, "white", &MaterialData::default())
        .expect("the GPU has memory for it");
    let flat = DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2);
    let mut scene = Scene::new(vec![Instance::new(
        &square,
        &white,
        DAffine3::from_scale_rotation_translation(DVec3::new(6.0, 6.0, 1.0), flat, DVec3::ZERO),
    )]);
    scene.lights = vec![Light::Directional(DirectionalLight {
        direction: Vec3::new(0.0, -1.0, 0.0),
        illuminance: 3.0,
        casts_shadows: false,
        ..DirectionalLight::default()
    })];
    scene.bloom = 0.04;
    let camera = Camera {
        eye: DVec3::new(0.0, 4.0, 4.0),
        target: DVec3::ZERO,
        ..Camera::default()
    };
    let (width, height) = (2048, 2048);
    let target = |height: u32| {
        gpu.create_render_target("output", width, height, format)
            .expect("the GPU has memory for it")
    };
    let (first, second) = (target(height), target(height - 8));
    // Once the size has held still, when the view makes its targets at that size.
    let mut draw = |view: &mut View, target: &RenderTarget| {
        settled::draw_settled(&gpu, &mut renderer, &scene, view, camera, target);
        gpu.wait_idle().expect("the GPU finishes");
    };

    let (before, _) = address_space();
    draw(&mut view, &first);
    let (drawn, _) = address_space();
    // Four samples of half-precision light and of depth, the light resolved and the bloom's levels:
    // about 230 MiB at this size.
    let frame = drawn.saturating_sub(before);
    if frame < 100 {
        eprintln!(
            "skipped: on {}, the frame's targets are not the process's memory ({frame} MiB)",
            gpu.adapter_name()
        );
        return;
    }
    draw(&mut view, &first);
    let (steady, peak_before) = address_space();
    // The peak so far leaves room to see one of the resize: a peak reached before would hide it.
    assert!(
        peak_before < steady + frame / 2,
        "the process held {peak_before} MiB before the resize, {steady} now"
    );
    draw(&mut view, &second);
    draw(&mut view, &second);
    let (after, peak) = address_space();
    eprintln!(
        "on {}: a frame of {width} × {height} takes {frame} MiB; {steady} MiB held before the \
         resize, {after} after it, {peak} at the most",
        gpu.adapter_name()
    );
    assert!(
        peak < steady.max(after) + frame / 2,
        "the resize held {} MiB more than a frame of {frame} MiB",
        peak.saturating_sub(steady.max(after))
    );
}
