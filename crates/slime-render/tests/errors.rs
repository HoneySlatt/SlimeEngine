//! The renderer's creations fail with the GPU layer's error instead of panicking (ADR-0044): here
//! on a lost device, which the test causes by destroying it. Running out of memory, the other
//! cause, is checked in `slime-gfx`, whose tests can make the memory scarce. Runs on whatever
//! adapter the machine offers.

use slime_gfx::{GfxError, Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::{Camera, MaterialData, RenderError, Renderer, Scene, Sky};

const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

fn target(gpu: &Gpu, side: u32) -> RenderTarget {
    gpu.create_render_target("output", side, side, FORMAT)
        .expect("the GPU has memory for it")
}

fn is_lost<T>(result: &Result<T, RenderError>) -> bool {
    matches!(result, Err(RenderError::Gfx(GfxError::DeviceLost { .. })))
}

#[test]
fn creations_on_a_lost_device_return_the_loss() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let (small, larger) = (target(&gpu, 32), target(&gpu, 64));
    let mut scene = Scene::new(Camera::default(), &[]);
    scene.bloom = 0.5;
    renderer
        .render(&gpu, &small, &scene)
        .expect("the frame is drawn");

    // A loss, as a driver reset would cause.
    gpu.lose();

    // A frame of the same size creates nothing, and is not drawn either.
    let frame = renderer.render(&gpu, &small, &scene);
    assert!(is_lost(&frame), "{frame:?}");
    // A frame of a new size creates its targets: it is not drawn, and the renderer does not panic.
    let frame = renderer.render(&gpu, &larger, &scene);
    assert!(is_lost(&frame), "{frame:?}");
    let material = renderer.create_material(&gpu, "lost", &MaterialData::default());
    assert!(is_lost(&material), "{material:?}");
    let grey = Sky {
        zenith: LinearColor::rgb(1000.0, 1000.0, 1000.0),
        horizon: LinearColor::rgb(1000.0, 1000.0, 1000.0),
        ground: LinearColor::rgb(1000.0, 1000.0, 1000.0),
        sun: None,
    };
    let environment = renderer.create_environment(&gpu, &grey);
    assert!(is_lost(&environment), "{environment:?}");
}
