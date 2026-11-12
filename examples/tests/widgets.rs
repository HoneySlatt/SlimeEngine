//! The image tests of the `widgets` example (`docs/adr/0148-widgets.md`): its interface, in the
//! editor's look, drawn by uwui's canvas through the engine's bridge, must match the reference
//! images of `tests/reference/` within a tolerance, at 960 × 600 pixels:
//!
//! - `widgets-inspector.png`: the inspector as it opens, in the dark theme;
//! - `widgets-light.png`: the same in the light theme;
//! - `widgets-contrast-compact.png`: the console in high contrast and compact;
//! - `widgets-console.png`: the console, a level of its log hidden by a check box that an action
//!   unchecked;
//! - `widgets-right-to-left.png`: the inspector laid out from the right, a section folded and
//!   another opened, a slider moved and a check box checked;
//! - `widgets-menus.png`: the bar's menu *File* open with its submenu of recent levels;
//! - `widgets-layers.png`: the floating panel of statistics, the list to choose the quality of
//!   shadows from open, and a tab's tooltip (`docs/adr/0150-layers.md`);
//! - `widgets-told.png`: a notification of a level saved and the palette of commands, what is
//!   typed finding its commands (`docs/adr/0151-rich-widgets.md`);
//! - `widgets-picker.png`: the light's color well, its picker open on a color typed as its code
//!   (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`).
//!
//! Transitions are dropped, so that each image shows where they end
//! (`docs/adr/0149-the-looks-foundations.md`).
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
#[path = "../common/style_fixture.rs"]
mod style_fixture;
#[path = "../common/widgets_panel.rs"]
mod widgets_panel;

use slime_app::InterfaceRenderer;
use slime_color::LinearColor;
use slime_gfx::{Gpu, RenderPassDescriptor, TextureFormat};
use uwui::Rect;
use uwui::action::{Action, ActionKind};
use uwui::look::{Colors, Density};
use uwui::preferences::{Preferences, TextSize};
use uwui::style::Display;
use uwui::text::TextBounds;
use uwui::tree::{ElementId, Kind, LayoutDirection};

use widgets_panel::WidgetsPanel;

/// The size of the images, in pixels, at scale 1, the example's own.
const SIZE: (u32, u32) = (960, 600);
const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// Draws `panel` twice, the second time with the rows that the first laid out, and compares the
/// image with the reference `name`.
fn check(panel: &mut WidgetsPanel, name: &str) {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let mut bridge = InterfaceRenderer::new(&gpu, FORMAT).expect("the renderer is created");
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
            .draw(&gpu, panel.ui.fonts(), &display, |_| None, &target)
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
    panel.ui.set_preferences(Preferences {
        reduced_motion: true,
        ..Preferences::default()
    });
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
    press(&mut panel, "main/window/tabs/Console");
    check(&mut panel, "widgets-contrast-compact");
}

#[test]
fn the_console_with_a_level_hidden_matches_its_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    press(&mut panel, "main/window/tabs/Console");
    press(&mut panel, "main/window/Console/split/levels/info");
    check(&mut panel, "widgets-console");
}

#[test]
fn the_inspector_from_the_right_with_its_widgets_used_matches_its_reference_image() {
    let mut panel = panel(LayoutDirection::RightToLeft);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    let properties = "main/window/Inspector/split/properties";
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

#[test]
fn the_bars_menu_and_its_submenu_match_their_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    press(&mut panel, "main/window/menus/File");
    press(&mut panel, "menu/menu/Recent");
    check(&mut panel, "widgets-menus");
}

#[test]
fn a_floating_panel_a_list_open_and_a_tooltip_match_their_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    press(&mut panel, "main/window/menus/View");
    press(&mut panel, "menu/menu/Statistics");
    press(
        &mut panel,
        "main/window/Inspector/split/properties/Rendering/shadow quality/choice",
    );
    // The pointer rests on the console's tab for longer than a tooltip waits.
    let tab = panel
        .ui
        .element_by_path("main/window/tabs/Console")
        .and_then(|tab| panel.ui.rect(tab))
        .expect("the console's tab");
    panel
        .ui
        .pointer_moved(tab.x + tab.width / 2.0, tab.y + tab.height / 2.0);
    panel.ui.advance(std::time::Duration::from_millis(600));
    check(&mut panel, "widgets-layers");
}

/// Gives the command `command`, as a shortcut does, and lets the program take what it did.
fn command(panel: &mut WidgetsPanel, command: &str) {
    panel.ui.act(Action {
        element: None,
        kind: ActionKind::Command(command.to_owned()),
    });
    panel.handle_actions();
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
}

#[test]
fn a_notification_and_the_palette_of_commands_match_their_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    command(&mut panel, "save");
    command(&mut panel, "palette");
    // What is typed in the palette's field finds the commands.
    let field = panel
        .ui
        .element_by_path("palette/palette/search")
        .expect("the palette's field");
    assert!(panel.ui.set_value(field, "sw"));
    panel.handle_actions();
    check(&mut panel, "widgets-told");
}

#[test]
fn the_light_s_color_well_and_its_picker_match_their_reference_image() {
    let mut panel = panel(LayoutDirection::LeftToRight);
    panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
    press(
        &mut panel,
        "main/window/Inspector/split/properties/Light/color/well",
    );
    // A color given as a hexadecimal code, which the well and the picker show.
    let code = panel
        .ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.name == "hexadecimal code")
        .expect("the picker's code")
        .id;
    assert!(panel.ui.set_value(code, "#3a9bdc"));
    panel.handle_actions();
    check(&mut panel, "widgets-picker");
}

/// The styles of every element of the inspector and of the console, in each theme of colours and
/// density, as the fixture `tests/reference/widgets-styles.txt` records them
/// (`common/style_fixture.rs`).
#[test]
fn every_element_is_styled_as_its_fixture_records() {
    let mut styled = Vec::new();
    for colors in [Colors::Dark, Colors::Light, Colors::HighContrast] {
        for density in [Density::Comfortable, Density::Compact] {
            for tab in ["Inspector", "Console"] {
                let mut panel = panel(LayoutDirection::LeftToRight);
                panel.set_look(colors, density);
                panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
                press(&mut panel, &format!("main/window/tabs/{tab}"));
                let root = panel.ui.element_by_path("main").expect("the panel's root");
                let prefix = format!("{colors:?} {density:?} {tab}");
                styled.extend(style_fixture::styles(&panel.ui, root, &prefix));
            }
        }
    }
    style_fixture::check("widgets-styles.txt", &styled);
}

/// What `id` draws as a text and the width it has for it, in pixels at scale 1: the label of a
/// label, a button, a toggle, a segment or a tab without a count, in its content box; a check
/// box's, beside its box and the gap after it. None for the other kinds.
fn drawn_text(panel: &WidgetsPanel, id: ElementId, content: Rect) -> Option<(String, f32)> {
    let element = panel.ui.element(id)?;
    Some(match &element.kind {
        Kind::Label(text)
        | Kind::Button { label: text }
        | Kind::ToggleButton { label: text, .. }
        | Kind::Segment { title: text }
        | Kind::Tab {
            title: text,
            count: None,
        } => (text.clone(), content.width),
        Kind::CheckBox { label, .. } => {
            let size = panel.ui.style(id)?.control.size;
            (label.clone(), (content.width - size * 1.5).max(0.0))
        }
        _ => return None,
    })
}

/// Whether `id` scrolls what it holds, which is reached by scrolling.
fn scrolls(panel: &WidgetsPanel, id: ElementId) -> bool {
    panel
        .ui
        .element(id)
        .is_some_and(|element| matches!(element.kind, Kind::Scroll | Kind::List { .. }))
}

/// Whether `inner` lies within `outer`, half a pixel spared for rounding.
fn inside(inner: Rect, outer: Rect) -> bool {
    inner.x >= outer.x - 0.5
        && inner.y >= outer.y - 0.5
        && inner.x + inner.width <= outer.x + outer.width + 0.5
        && inner.y + inner.height <= outer.y + outer.height + 0.5
}

/// The texts laid out below `root` that are cut (WCAG's failure F69), whether they show or are
/// scrolled away: a text laid out wider or taller than its element's content box, inside its
/// padding, or an element's box that leaves an ancestor's, up to the first ancestor
/// that scrolls, whose content is reached by scrolling. It reads the texts that [`drawn_text`]
/// gives; `checked` counts them.
fn cut_texts(panel: &mut WidgetsPanel, root: ElementId, checked: &mut usize) -> Vec<String> {
    let mut cut = Vec::new();
    let mut stack = vec![(root, Vec::<ElementId>::new())];
    while let Some((id, ancestors)) = stack.pop() {
        let (Some(rect), Some(style)) = (panel.ui.rect(id), panel.ui.style(id).cloned()) else {
            continue;
        };
        if style.layout.display == Display::None || rect.width <= 0.0 || rect.height <= 0.0 {
            continue;
        }
        let mut below = ancestors.clone();
        below.push(id);
        for &child in panel.ui.children(id) {
            stack.push((child, below.clone()));
        }
        // Its text is laid out inside its padding: taffy is given no border, which paint draws
        // inside the padding.
        let padding = style.layout.padding;
        let content = Rect::new(
            rect.x + padding.start,
            rect.y + padding.top,
            (rect.width - padding.start - padding.end).max(0.0),
            (rect.height - padding.top - padding.bottom).max(0.0),
        );
        let Some((text, width)) = drawn_text(panel, id, content) else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }
        *checked += 1;
        let path = panel.ui.path_text(id).unwrap_or_default();
        let laid =
            panel
                .ui
                .fonts_mut()
                .lay_out(&text, &style.text.style, 1.0, TextBounds::width(width));
        if laid.width() > width + 0.5 || laid.height() > content.height + 0.5 {
            cut.push(format!(
                "{path} {text:?}: {:.1} × {:.1} in {width:.1} × {:.1}",
                laid.width(),
                laid.height(),
                content.height
            ));
        }
        for &ancestor in ancestors.iter().rev() {
            if scrolls(panel, ancestor) {
                break;
            }
            let Some(outer) = panel.ui.rect(ancestor) else {
                break;
            };
            if !inside(rect, outer) {
                let name = panel.ui.path_text(ancestor).unwrap_or_default();
                cut.push(format!("{path} {text:?}: its box leaves {name}'s"));
                break;
            }
        }
    }
    cut
}

/// At 200 % text, WCAG 2.2's 1.4.4, no text of the example is cut, in each tab, with the floating
/// panel of statistics, the sheet of shortcuts and a notification open: boxes of fixed size that
/// hold a text are least sizes, and a list's rows grow with their text
/// (`docs/adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md`). It reads
/// labels, buttons, toggles, segments, tabs without a count and check boxes; not a field's text, a
/// menu item's, a section's or a floating panel's title, a tab's count, nor a list to choose from,
/// which menus and the palette of commands hold.
#[test]
fn at_twice_the_size_of_text_no_text_is_cut() {
    for factor in [1.0, 2.0] {
        let mut cut = Vec::new();
        let mut checked = 0;
        for tab in ["Inspector", "Console", "About"] {
            let mut panel = panel(LayoutDirection::LeftToRight);
            panel.ui.set_preferences(Preferences {
                text_size: TextSize::factor(factor).expect("within its range"),
                reduced_motion: true,
            });
            panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
            press(&mut panel, &format!("main/window/tabs/{tab}"));
            let mut layers = vec!["main"];
            if tab == "Inspector" {
                command(&mut panel, "statistics");
                command(&mut panel, "shortcuts");
                command(&mut panel, "save");
                layers.extend(["statistics", "shortcuts", "notifications"]);
            }
            panel.frame([SIZE.0 as f32, SIZE.1 as f32], 1.0);
            for layer in layers {
                let root = panel.ui.element_by_path(layer);
                let root = root.unwrap_or_else(|| panic!("{tab}: no layer at {layer}"));
                cut.extend(cut_texts(&mut panel, root, &mut checked));
            }
        }
        assert!(
            checked > 120,
            "{checked} texts checked at {factor}: the walk saw too little"
        );
        assert!(cut.is_empty(), "at {factor}, cut:\n{}", cut.join("\n"));
    }
}
