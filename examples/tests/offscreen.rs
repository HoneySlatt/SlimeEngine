//! The reference image test of Stage 2: the samples' triangle, rendered offscreen, must match
//! `tests/reference/triangle.png` within a tolerance.
//!
//! The reference was rendered with lavapipe, Mesa's Vulkan driver that renders on the CPU, which
//! `scripts/check.sh` and Linux CI use. Other drivers (WARP in Windows CI, hardware GPUs) may
//! differ slightly in how they round colors; the tolerance absorbs that. To render the reference
//! again, after a deliberate change of the triangle:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test offscreen
//! ```
//!
//! When the test fails, it writes the image it rendered next to the build artifacts and prints
//! its path, so that the two images can be compared by eye.

#[path = "../common/reference_image.rs"]
mod reference_image;
#[path = "../common/triangle.rs"]
mod triangle;

use slime_gfx::{Gpu, RenderPassDescriptor, TextureFormat};

use triangle::{BACKGROUND, Triangle};

const SIZE: u32 = 256;

#[test]
fn the_triangle_matches_its_reference_image() {
    check_triangle(TextureFormat::Rgba8UnormSrgb, "triangle");
}

/// Most windows show images in blue, green, red order; once read back, the triangle must be the
/// same.
#[test]
fn the_triangle_in_the_usual_window_format_matches_the_same_reference() {
    check_triangle(TextureFormat::Bgra8UnormSrgb, "triangle-bgra");
}

/// Renders the triangle into a target of `format` and compares it with the reference. A rendered
/// image that does not match is written as `<name>.actual.png`.
fn check_triangle(format: TextureFormat, name: &str) {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target = gpu
        .create_render_target("reference triangle", SIZE, SIZE, format)
        .expect("the GPU has memory for it");
    let triangle =
        Triangle::new(&gpu, target.format()).expect("the triangle's GPU objects are valid");
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(BACKGROUND),
        depth: None,
        timer: None,
    };
    gpu.render(&pass, |pass| triangle.draw(pass));
    let image = gpu
        .read_render_target(&target)
        .expect("the image is read back");

    if reference_image::update_requested() {
        // One reference for both formats, written by the test of the format it was made in.
        if format == TextureFormat::Rgba8UnormSrgb {
            reference_image::write("triangle", &image);
        }
        return;
    }
    reference_image::check("triangle", name, &image, gpu.adapter_name());
}
