//! A reference image test of Stages 3a and 3b: the shapes of `common/shapes.rs` (cubes and a
//! sphere drawn by the renderer, two of them textured, and six small spheres of increasing
//! roughness, three dielectrics and three metals), rendered offscreen at a fixed time, must match
//! `tests/reference/shapes.png` within a tolerance (ADR-0018).
//!
//! The reference was rendered with lavapipe, like the triangle's. To render it again, after a
//! deliberate change of the shapes or of the renderer:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test scene
//! ```

#[path = "../common/reference_image.rs"]
mod reference_image;
#[path = "../common/shapes.rs"]
mod shapes;

use slime_gfx::{Gpu, TextureFormat};

use shapes::Shapes;

const SIZE: u32 = 256;
/// When the camera is caught: a moment where the shapes overlap in the image.
const TIME: f32 = 0.7;

#[test]
fn the_shapes_match_their_reference_image() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target = gpu
        .create_render_target(
            "reference shapes",
            SIZE,
            SIZE,
            TextureFormat::Rgba8UnormSrgb,
        )
        .expect("the GPU has memory for it");
    let mut shapes = Shapes::new(&gpu, target.format()).expect("the shapes' GPU objects are valid");
    let stats = shapes.render(&gpu, &target, TIME);
    // Three cubes of 12 triangles and seven spheres of 720.
    assert_eq!((stats.draw_calls, stats.triangles), (10, 5076));
    let image = gpu
        .read_render_target(&target)
        .expect("the image is read back");

    if reference_image::update_requested() {
        reference_image::write("shapes", &image);
        return;
    }
    reference_image::check("shapes", "shapes", &image, gpu.adapter_name());
}
