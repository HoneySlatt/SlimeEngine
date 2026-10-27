//! Floating panels, without a GPU (`docs/adr/0150-layers.md`): a panel that fills its floating
//! layer; moved by its bar, within the window, and sized by its corner, no smaller than its least,
//! from either direction; folded to its bar by its mark, its layer as tall as the bar; raised above
//! another by a press; its meaning.

use slime_input::{ButtonState, MouseButton};
use slime_ui::style::{Dimension, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{Element, ElementId, Layer, LayoutDirection, Place, Role, RootSettings, Ui};

/// An interface of 400 by 300, its root holding nothing: the interface and its root.
fn interface(direction: LayoutDirection) -> (Ui, ElementId) {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    let mut ui = Ui::new(fonts);
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root(settings, Style::default());
    ui.frame();
    (ui, root)
}

/// Opens a floating layer over `over` at `origin`, of 200 by 150, holding a panel titled `title`
/// that holds a box of 50 by 50: the layer and the panel.
fn open(ui: &mut Ui, over: ElementId, origin: [f32; 2], title: &str) -> (ElementId, ElementId) {
    let layer = ui.open_layer(
        Layer {
            place: Place::Floating {
                over,
                origin,
                size: [200.0, 150.0],
            },
            modal: false,
            dismiss: false,
        },
        Style::default(),
    );
    let mut style = Style::default();
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    style.layout.padding.top = 4.0;
    let mut panel = None;
    ui.build(layer, 0, |children| {
        panel = Some(children.add(
            "panel",
            Element::floating_panel(title, true, style),
            |panel| {
                let mut boxed = Style::default();
                boxed.layout.size = [Dimension::Pixels(50.0), Dimension::Pixels(50.0)];
                panel.add("box", Element::container(boxed), |_| {});
            },
        ));
    });
    ui.frame();
    (layer, panel.expect("built"))
}

/// The pointer pressed at `from`, moved to `to`, and let go.
fn drag(ui: &mut Ui, from: (f32, f32), to: (f32, f32)) {
    ui.pointer_moved(from.0, from.1);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(to.0, to.1);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
}

fn origin_and_size(ui: &Ui, layer: ElementId) -> ([f32; 2], [f32; 2]) {
    match ui.layer_place(layer) {
        Some(Place::Floating { origin, size, .. }) => (origin, size),
        other => panic!("a floating layer, not {other:?}"),
    }
}

#[test]
fn a_panel_fills_its_layer_and_its_bar_moves_it_within_the_window() {
    let (mut ui, root) = interface(LayoutDirection::LeftToRight);
    let (layer, panel) = open(&mut ui, root, [50.0, 40.0], "Color");
    let rect = ui.rect(panel).expect("laid out");
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (50.0, 40.0, 200.0, 150.0)
    );
    // The bar, away from its mark.
    drag(&mut ui, (150.0, 50.0), (180.0, 70.0));
    assert_eq!(origin_and_size(&ui, layer).0, [80.0, 60.0]);
    let rect = ui.rect(panel).expect("laid out");
    assert_eq!((rect.x, rect.y), (80.0, 60.0));
    assert!(ui.take_actions().is_empty());
    // Far past the window's corner: within it.
    drag(&mut ui, (180.0, 70.0), (900.0, 900.0));
    assert_eq!(origin_and_size(&ui, layer).0, [200.0, 150.0]);
}

#[test]
fn its_corner_sizes_it_no_smaller_than_its_least_from_either_direction() {
    let (mut ui, root) = interface(LayoutDirection::LeftToRight);
    let (layer, _) = open(&mut ui, root, [50.0, 40.0], "Color");
    // The corner at the end of its bottom, a control's size square.
    drag(&mut ui, (245.0, 185.0), (285.0, 195.0));
    assert_eq!(origin_and_size(&ui, layer), ([50.0, 40.0], [240.0, 160.0]));
    drag(&mut ui, (285.0, 195.0), (0.0, 0.0));
    let least = Style::default().control.size;
    assert_eq!(origin_and_size(&ui, layer).1, [least * 8.0, least * 4.0]);
    // From right to left, the corner is at its bottom's left, and it grows towards it.
    let (mut ui, root) = interface(LayoutDirection::RightToLeft);
    let (layer, panel) = open(&mut ui, root, [150.0, 40.0], "Color");
    let rect = ui.rect(panel).expect("laid out");
    assert_eq!(rect.x, 150.0);
    drag(&mut ui, (155.0, 185.0), (115.0, 185.0));
    assert_eq!(origin_and_size(&ui, layer), ([110.0, 40.0], [240.0, 150.0]));
}

#[test]
fn its_mark_folds_it_to_its_bar() {
    let (mut ui, root) = interface(LayoutDirection::LeftToRight);
    let (layer, panel) = open(&mut ui, root, [50.0, 40.0], "Color");
    let held = ui.children(panel)[0];
    // The mark, at the bar's start.
    drag(&mut ui, (55.0, 50.0), (55.0, 50.0));
    assert!(!ui.state(panel).expect("there").expanded);
    let rect = ui.rect(layer).expect("laid out");
    assert!(rect.height < 40.0, "as tall as its bar: {rect:?}");
    assert!(ui.meaning_of(held).is_none(), "what it holds is not drawn");
    // Its place is kept, its size too.
    assert_eq!(origin_and_size(&ui, layer), ([50.0, 40.0], [200.0, 150.0]));
    drag(&mut ui, (55.0, 50.0), (55.0, 50.0));
    assert!(ui.state(panel).expect("there").expanded);
    assert_eq!(ui.rect(layer).expect("laid out").height, 150.0);
}

#[test]
fn a_press_raises_a_panel_above_another() {
    let (mut ui, root) = interface(LayoutDirection::LeftToRight);
    let (_, first) = open(&mut ui, root, [20.0, 20.0], "First");
    let (_, second) = open(&mut ui, root, [120.0, 60.0], "Second");
    // Whether the element at `at` lies in `id`: its path starts with `id`'s.
    let within = |ui: &Ui, id: ElementId, at: (f32, f32)| {
        let path = ui.path_text(id).expect("there");
        ui.element_at(at.0, at.1)
            .and_then(|hit| ui.path_text(hit))
            .is_some_and(|hit| hit.starts_with(&path))
    };
    // Where they overlap, the second lies on top, until the first is pressed.
    assert!(within(&ui, second, (150.0, 120.0)));
    drag(&mut ui, (60.0, 100.0), (60.0, 100.0));
    assert!(within(&ui, first, (150.0, 120.0)));
}

#[test]
fn a_panel_means_a_pane_named_by_its_title() {
    let (mut ui, root) = interface(LayoutDirection::LeftToRight);
    let (_, panel) = open(&mut ui, root, [50.0, 40.0], "Color");
    let meaning = ui.meaning_of(panel).expect("drawn");
    assert_eq!((meaning.role, meaning.name.as_str()), (Role::Pane, "Color"));
    assert!(meaning.state.expanded);
    // Opened by the program, not by a press, it leaves the focus where it was: F6 reaches it.
    assert_eq!(ui.focus(), None);
}

#[test]
fn a_raised_panel_keeps_the_menu_that_it_opened_above_it() {
    let (mut ui, root) = interface(LayoutDirection::LeftToRight);
    let (first, panel) = open(&mut ui, root, [20.0, 20.0], "First");
    // A button in the first panel, and a menu beside it, then a second panel over both.
    let mut button = None;
    ui.build(panel, 0, |children| {
        let mut style = Style::default();
        style.layout.size = [Dimension::Pixels(60.0), Dimension::Pixels(24.0)];
        button = Some(children.add("open", Element::button("", style), |_| {}));
    });
    ui.frame();
    let button = button.expect("built");
    let menu = ui.open_layer(
        Layer {
            place: Place::Beside {
                element: button,
                side: slime_ui::tree::Side::Below,
                along: slime_ui::tree::Along::Start,
            },
            modal: false,
            dismiss: true,
        },
        Style::default(),
    );
    ui.build(menu, 0, |children| {
        let mut style = Style::default();
        style.layout.size = [Dimension::Pixels(100.0), Dimension::Pixels(60.0)];
        children.add("item", Element::button("", style), |_| {});
    });
    ui.frame();
    let (second, _) = open(&mut ui, root, [60.0, 60.0], "Second");
    let _ = second;
    // A press on the button raises the first panel, its menu staying above it.
    let at = ui.rect(button).expect("laid out");
    drag(&mut ui, (at.x + 5.0, at.y + 5.0), (at.x + 5.0, at.y + 5.0));
    assert!(ui.rect(menu).is_some(), "a press on its opener keeps it");
    let menu_rect = ui.rect(menu).expect("placed");
    let inside_menu = (menu_rect.x + 5.0, menu_rect.y + 5.0);
    let path = ui.path_text(menu).expect("there");
    let hit = ui
        .element_at(inside_menu.0, inside_menu.1)
        .and_then(|hit| ui.path_text(hit))
        .expect("an element");
    assert!(hit.starts_with(&path), "{hit} under {path}");
    let first_path = ui.path_text(first).expect("there");
    assert!(
        first_path < path,
        "the panel below its menu: {first_path} {path}"
    );
}
