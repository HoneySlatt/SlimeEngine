//! The image tests of the `widgets` example (`docs/adr/0148-widgets.md`): its interface, in the
//! editor's look, drawn by the renderer's canvas, must match the reference images of
//! `tests/reference/` within a tolerance. The inspector as it opens, in the dark theme and in the
//! light one; the console in high contrast and compact; the console, a level of its log hidden by a
//! check box that an action unchecked; the inspector laid out from the right, a section folded and
//! another opened, a slider moved and a check box checked. Transitions are dropped, so that each
//! image shows where they end (`docs/adr/0149-the-looks-foundations.md`).
//!
//! The references were drawn with lavapipe, as the triangle's were (`tests/offscreen.rs`). To draw
//! them again, after a deliberate change of the example or of the look:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test widgets
//! ```

#[path = "../common/reference_image.rs"]
mod reference_image;
#[path = "../common/widgets_panel.rs"]
mod widgets_panel;

use slime_color::LinearColor;
use slime_gfx::{Gpu, RenderPassDescriptor, TextureFormat};
use slime_render::Renderer;
use slime_ui::action::{Action, ActionKind};
use slime_ui::look::{Colors, Density};
use slime_ui::tree::LayoutDirection;
use slime_ui_render::UiRenderer;

use widgets_panel::WidgetsPanel;

/// The size of the images, in pixels, at scale 1, the example's own.
const SIZE: (u32, u32) = (960, 600);
const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// Draws `panel` twice, the second time with the rows that the first laid out, and compares the
/// image with the reference `name`.
fn check(panel: &mut WidgetsPanel, name: &str) {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let mut bridge = UiRenderer::default();
    let target = gpu
        .create_render_target(name, SIZE.0, SIZE.1, FORMAT)
        .expect("the GPU has memory for it");
    for _ in 0..2 {
        let display = panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
        gpu.render(
            &RenderPassDescriptor {
                target: &target,
                clear: Some(LinearColor::BLACK),
                depth: None,
                timer: None,
                resolve: None,
            },
            |_| {},
        );
        bridge
            .draw(
                &gpu,
                &mut renderer,
                panel.ui.fonts(),
                &display,
                |_| None,
                &target,
            )
            .expect("drawn");
    }
    let image = gpu
        .read_render_target(&target)
        .expect("the image is read back");
    if reference_image::update_requested() {
        reference_image::write(name, &image);
        return;
    }
    reference_image::check(name, name, &image, gpu.adapter_name());
}

/// Presses the element at `path`, as an agent does, and lets the program take what it did.
fn press(panel: &mut WidgetsPanel, path: &str) {
    let element = panel.ui.element_by_path(path);
    assert!(element.is_some(), "no element at {path}");
    panel.ui.act(Action {
        element,
        kind: ActionKind::Press,
    });
    panel.handle_actions();
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
}

/// The example's interface laid out in `direction`, its transitions dropped, so that each image
/// shows where they end.
fn panel(direction: LayoutDirection) -> WidgetsPanel {
    let mut panel = WidgetsPanel::new(direction);
    panel.ui.set_reduced_motion(true);
    panel
}

#[test]
fn the_inspector_matches_its_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    check(&mut panel, "widgets-inspector");
}

#[test]
fn the_inspector_in_the_light_theme_matches_its_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    panel.set_look(Colors::Light, Density::Comfortable);
    check(&mut panel, "widgets-light");
}

#[test]
fn the_console_in_high_contrast_and_compact_matches_its_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    panel.set_look(Colors::HighContrast, Density::Compact);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    press(&mut panel, "#0/window/tabs/Console");
    check(&mut panel, "widgets-contrast-compact");
}

#[test]
fn the_console_with_a_level_hidden_matches_its_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    press(&mut panel, "#0/window/tabs/Console");
    press(&mut panel, "#0/window/Console/split/levels/info");
    check(&mut panel, "widgets-console");
}

#[test]
fn the_inspector_from_the_right_with_its_widgets_used_matches_its_reference_image() {
    let mut panel = panel(LayoutDirection::RightToLeft);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    let properties = "#0/window/Inspector/split/properties";
    press(&mut panel, &format!("{properties}/Transform"));
    press(&mut panel, &format!("{properties}/Physics"));
    press(
        &mut panel,
        &format!("{properties}/Rendering/receives shadows"),
    );
    let slider = panel
        .ui
        .element_by_path(&format!("{properties}/Light/intensity/slider"))
        .expect("the slider of intensity");
    assert!(panel.ui.set_value(slider, "7.5"));
    panel.handle_actions();
    check(&mut panel, "widgets-right-to-left");
}
