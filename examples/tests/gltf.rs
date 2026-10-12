//! The reference image test of the glTF scene: the asset of `examples/assets/scene.glb`, loaded by
//! the examples' loader and rendered offscreen from two fixed camera poses, must match
//! `tests/reference/gltf-front.png` and `tests/reference/gltf-back.png` within the tolerance of
//! ADR-0018 and ADR-0028.
//!
//! The references were rendered with lavapipe, like the others. To render them again, after a
//! deliberate change of the asset or of the renderer:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test gltf
//! ```

#[path = "../common/orbit.rs"]
mod orbit;
#[path = "../common/reference_image.rs"]
mod reference_image;
#[path = "../common/sample_scene.rs"]
mod sample_scene;

use slime_gfx::{Gpu, TextureFormat};

use sample_scene::SampleScene;

const SIZE: u32 = 256;

#[test]
fn the_gltf_scene_matches_its_reference_images() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target =
        gpu.create_render_target("reference glTF", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb);
    let mut scene = SampleScene::new(&gpu, target.format(), sample_scene::ASSET)
        .expect("the sample asset loads");
    let bounds = *scene.bounds();
    // Seven primitives, one per object of the asset, and the triangles of their meshes.
    for (name, yaw, pitch) in [("gltf-front", 0.6, 0.45), ("gltf-back", 3.6, 0.25)] {
        let camera = orbit::camera_at(bounds.center(), bounds.radius() * 2.2, yaw, pitch);
        let stats = scene.render(&gpu, &target, camera);
        assert_eq!((stats.draw_calls, stats.triangles), (7, 2274));
        let image = gpu
            .read_render_target(&target)
            .expect("the image is read back");
        if reference_image::update_requested() {
            reference_image::write(name, &image);
        } else {
            reference_image::check(name, name, &image, gpu.adapter_name());
        }
    }
}
