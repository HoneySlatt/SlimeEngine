//! The reference image test of Stage 3a: the cubes of the `scene` example, rendered offscreen at
//! a fixed time, must match `tests/reference/cubes.png` within a tolerance (ADR-0018).
//!
//! The reference was rendered with lavapipe, like the triangle's. To render it again, after a
//! deliberate change of the cubes:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test scene
//! ```

#[path = "../common/cubes.rs"]
mod cubes;
#[path = "../common/reference_image.rs"]
mod reference_image;

use slime_gfx::{Gpu, TextureFormat};

use cubes::Cubes;

const SIZE: u32 = 256;
/// When the camera is caught: a moment where the cubes overlap in the image.
const TIME: f32 = 0.7;

#[test]
fn the_cubes_match_their_reference_image() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target =
        gpu.create_render_target("reference cubes", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb);
    let mut cubes = Cubes::new(&gpu, target.format()).expect("the cubes' GPU objects are valid");
    let stats = cubes.render(&gpu, &target, TIME);
    assert_eq!((stats.draw_calls, stats.triangles), (3, 36));
    let image = gpu
        .read_render_target(&target)
        .expect("the image is read back");

    if reference_image::update_requested() {
        reference_image::write("cubes", &image);
        return;
    }
    reference_image::check("cubes", "cubes", &image, gpu.adapter_name());
}
