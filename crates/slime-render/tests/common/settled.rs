//! Drawing a view at the size that its output asks for: a view whose size changes draws into its
//! old targets until the new size has held still for `RESIZE_DELAY` (ADR-0127), and the tests that
//! draw one view at several sizes read the image of the new size. Shared by those tests through
//! `#[path]`.

use slime_gfx::{Gpu, RenderTarget};
use slime_render::{Camera, RESIZE_DELAY, Renderer, Scene, View, ViewDraw};

/// Draws `scene` through `view`, at a render scale of 1, from `camera`, into the whole of `target`,
/// again after [`RESIZE_DELAY`] while the view's targets are not the target's size, so that the
/// last frame is drawn at that size.
pub fn draw_settled(
    gpu: &Gpu,
    renderer: &mut Renderer,
    scene: &Scene,
    view: &mut View,
    camera: Camera,
    target: &RenderTarget,
) {
    // At another scale its targets never take the target's size: this would never end.
    assert_eq!(
        view.settings().render_scale,
        1.0,
        "a view at a render scale of 1"
    );
    loop {
        renderer
            .render(gpu, scene, &mut [ViewDraw::new(view, camera, target)])
            .expect("the frame is drawn");
        if view.render_size() == Some((target.width(), target.height())) {
            return;
        }
        std::thread::sleep(RESIZE_DELAY);
    }
}
