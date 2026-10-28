//! The image tests of the editor's window (`docs/adr/0156-the-editors-window-and-its-operations.md`):
//! the `editor` example's window on the examples' yard, as it shows once the yard is open, and with
//! the modal layer that waits for the system's dialog to open a level, the window dimmed below it
//! (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`), drawn without a window of the
//! system by `slime_editor::window_image`, must match the reference images of `tests/reference/`
//! within a tolerance, at scales 1 and 2. The level is named by a path relative to the examples'
//! package, the directory that the tests run in, which the status bar shows as it is.
//!
//! The references were drawn with lavapipe, as the triangle's were (`tests/offscreen.rs`). To draw
//! them again, after a deliberate change of the window or of the look:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test editor
//! ```

#[path = "../common/cooked.rs"]
#[expect(dead_code, reason = "these tests cook the sample themselves")]
mod cooked;
#[path = "../common/reference_image.rs"]
mod reference_image;
#[path = "../common/sample_scene.rs"]
#[expect(
    dead_code,
    reason = "the editor lights the yard with the sample's sky, exposure and bloom alone"
)]
mod sample_scene;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The editor's program on the yard, as the example runs it, its data cooked into `assets`.
fn program(assets: &Path) -> slime_editor::Program {
    slime_editor::Program {
        name: "editor",
        level: PathBuf::from("assets/yard.scene"),
        key: "yard",
        assets: assets.to_owned(),
        sources: None,
        register: |_| {},
        lighting: slime_editor::Lighting {
            sky: Some(sample_scene::SKY),
            exposure: sample_scene::EXPOSURE,
            bloom: sample_scene::BLOOM,
        },
    }
}

/// Draws the window at `scale`, once `operations` ran, and compares it with the reference `name`.
fn check(name: &str, operations: &[&str], scale: f32) {
    let directory = cooked::directory(name);
    let assets = directory.join("cooked");
    slime_cook::cook(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")),
        &assets,
    )
    .expect("the sample cooks");
    let arguments: Vec<OsString> = vec![
        "--journal".into(),
        directory.join("yard.journal").into_os_string(),
    ];
    let image = slime_editor::window_image(&program(&assets), arguments, operations, scale)
        .expect("the window drawn");
    let [width, height] = slime_editor::IMAGE_SIZE;
    assert_eq!(
        (image.width, image.height),
        (
            (width as f32 * scale) as u32,
            (height as f32 * scale) as u32
        )
    );
    if reference_image::update_requested() {
        reference_image::write(name, &image);
        return;
    }
    let gpu =
        slime_gfx::Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
    reference_image::check(name, name, &image, gpu.adapter_name());
}

#[test]
fn the_window_matches_its_reference_image() {
    check("editor-window", &[], 1.0);
}

#[test]
fn the_window_at_scale_2_matches_its_reference_image() {
    check("editor-window-scale-2", &[], 2.0);
}

#[test]
fn the_window_waiting_for_the_system_s_dialog_matches_its_reference_image() {
    check("editor-choose", &["file.choose"], 1.0);
}
