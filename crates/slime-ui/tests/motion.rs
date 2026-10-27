//! The look's foundations, without a GPU (`docs/adr/0149-the-looks-foundations.md`): transitions
//! of colors over the time that the program gives, which less motion drops; a section that grows
//! and shrinks to its height; an opacity that multiplies what an element and its children draw;
//! the focus shown to the keyboard and not to the pointer; boxes placed on whole pixels.

use std::time::Duration;

use slime_color::LinearColor;
use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use slime_ui::display::{DisplayList, Primitive};
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::theme::{StyleSheet, Theme};
use slime_ui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

/// An interface whose font is the tests' Noto Sans, styled by `sheet`, and its root of 400 by 300
/// logical pixels at `scale`.
fn ui(sheet: &str, scale: f32) -> (Ui, ElementId) {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    let mut ui = Ui::new(fonts);
    ui.set_style(
        &[StyleSheet::from_text("motion.style", sheet)],
        &[Theme::from_text("empty.theme", "")],
    )
    .expect("the style sheet");
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let mut column = Style::default();
    column.layout.direction = FlexDirection::Column;
    let root = ui.add_root(settings, column);
    (ui, root)
}

/// A style of `width` by `height` logical pixels that does not shrink.
fn sized(width: f32, height: f32) -> Style {
    let mut style = Style::default();
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
    style.layout.shrink = 0.0;
    style.text.style.fonts = vec!["Noto Sans".to_owned()];
    style
}

/// The fill of the first box of `list` as wide as `width` pixels.
fn fill(list: &DisplayList, width: f32) -> LinearColor {
    list.items()
        .iter()
        .find_map(|item| match &item.primitive {
            Primitive::Box(primitive) if primitive.rect.width == width => Some(primitive.fill),
            _ => None,
        })
        .expect("the box")
}

const BUTTONS: &str = r##"
[button]
select = { kind = "Button" }
style.paint.background = "#000000"
style.transition = { duration = 0.2, easing = "Linear" }

[button-hovered]
select = { kind = "Button", states = ["Hovered"] }
style.paint.background = "#ffffff"
"##;

#[test]
fn a_color_moves_over_the_time_given_and_less_motion_drops_it() {
    let (mut ui, root) = ui(BUTTONS, 1.0);
    ui.build(root, 0, |children| {
        children.add("button", Element::button("go", sized(100.0, 30.0)), |_| {});
    });
    ui.frame();
    ui.pointer_moved(50.0, 15.0);
    // The hover starts from black: no time has passed.
    let (list, stats) = ui.frame();
    assert_eq!(fill(&list, 100.0), LinearColor::rgba(0.0, 0.0, 0.0, 1.0));
    assert!(stats.animating);
    // Halfway, then at the end, linear in the colors' light.
    ui.advance(Duration::from_millis(100));
    let (list, _) = ui.frame();
    assert!(
        (fill(&list, 100.0).r - 0.5).abs() < 1e-3,
        "{:?}",
        fill(&list, 100.0)
    );
    ui.advance(Duration::from_millis(150));
    let (list, stats) = ui.frame();
    assert_eq!(fill(&list, 100.0), LinearColor::rgba(1.0, 1.0, 1.0, 1.0));
    assert!(!stats.animating);
    // Less motion: the pointer leaves, and the button is black at once.
    ui.set_reduced_motion(true);
    ui.pointer_moved(300.0, 200.0);
    let (list, stats) = ui.frame();
    assert_eq!(fill(&list, 100.0), LinearColor::rgba(0.0, 0.0, 0.0, 1.0));
    assert!(!stats.animating);
}

#[test]
fn a_new_element_shows_its_style_at_once() {
    let (mut ui, root) = ui(BUTTONS, 1.0);
    ui.build(root, 0, |children| {
        children.add("button", Element::button("go", sized(100.0, 30.0)), |_| {});
    });
    let (list, stats) = ui.frame();
    assert_eq!(fill(&list, 100.0), LinearColor::rgba(0.0, 0.0, 0.0, 1.0));
    assert!(!stats.animating);
}

#[test]
fn an_opacity_multiplies_what_an_element_and_its_children_draw() {
    let (mut ui, root) = ui("", 1.0);
    let mut outer = sized(200.0, 100.0);
    outer.paint.background = LinearColor::rgba(1.0, 0.0, 0.0, 1.0);
    outer.paint.opacity = 0.5;
    let mut inner = sized(50.0, 50.0);
    inner.paint.background = LinearColor::rgba(0.0, 1.0, 0.0, 0.8);
    inner.paint.opacity = 0.5;
    ui.build(root, 0, |children| {
        children.add("outer", Element::container(outer), |children| {
            children.add("inner", Element::container(inner), |_| {});
        });
    });
    let (list, _) = ui.frame();
    assert!((fill(&list, 200.0).a - 0.5).abs() < 1e-6);
    // Its own and its parent's: 0.8 × 0.5 × 0.5.
    assert!((fill(&list, 50.0).a - 0.2).abs() < 1e-6);
}

const SECTIONS: &str = r##"
[section]
select = { kind = "Section" }
style.transition = { duration = 0.2, easing = "Linear" }
"##;

#[test]
fn a_section_grows_and_shrinks_to_its_height_over_its_transition() {
    let (mut ui, root) = ui(SECTIONS, 1.0);
    let mut section = None;
    let mut row = None;
    ui.build(root, 0, |children| {
        let mut style = Style::default();
        style.layout.direction = FlexDirection::Column;
        style.text.style.fonts = vec!["Noto Sans".to_owned()];
        section = Some(children.add(
            "section",
            Element::section("physics", true, style),
            |children| {
                row = Some(children.add("a", Element::container(sized(100.0, 100.0)), |_| {}));
            },
        ));
    });
    ui.frame();
    let (section, row) = (section.expect("built"), row.expect("built"));
    let open = ui.rect(section).expect("laid out").height;
    ui.set_open(section, false);
    ui.frame();
    // Folding: what it holds still shows, cut to its height, which shrinks over the time.
    assert!(ui.rect(row).is_some_and(|rect| rect.height == 100.0));
    ui.advance(Duration::from_millis(100));
    let (_, stats) = ui.frame();
    let half = ui.rect(section).expect("laid out").height;
    assert!(stats.animating);
    assert!(half < open && half > open - 100.0, "{half} of {open}");
    ui.advance(Duration::from_millis(150));
    ui.frame();
    let (_, stats) = ui.frame();
    let folded = ui.rect(section).expect("laid out").height;
    assert!((folded - (open - 100.0)).abs() < 1.0, "{folded} of {open}");
    assert!(!stats.animating);
    assert!(ui.meaning_of(row).is_none(), "folded, it means nothing");
    // Opening: measured at once, then grown from its folded height.
    ui.set_open(section, true);
    let (_, stats) = ui.frame();
    assert!(stats.animating);
    assert!((ui.rect(section).expect("laid out").height - folded).abs() < 1.0);
    ui.advance(Duration::from_millis(250));
    ui.frame();
    ui.frame();
    assert!((ui.rect(section).expect("laid out").height - open).abs() < 1.0);
}

/// A press of a named key.
fn named(key: NamedKey) -> KeyEvent {
    KeyEvent {
        physical: KeyCode::Unidentified,
        logical: Key::Named(key),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
    }
}

#[test]
fn the_focus_shows_to_the_keyboard_and_not_to_the_pointer() {
    let (mut ui, root) = ui("", 1.0);
    let mut buttons = Vec::new();
    ui.build(root, 0, |children| {
        for key in ["a", "b"] {
            buttons.push(children.add(key, Element::button(key, sized(100.0, 30.0)), |_| {}));
        }
    });
    ui.frame();
    // Tab: the first button holds a focus that shows.
    ui.key(&named(NamedKey::Tab));
    ui.frame();
    let state = ui.state(buttons[0]).expect("there");
    assert!(state.focused && state.focus_visible);
    // A press of the pointer on the second: focused, its focus not shown.
    ui.pointer_moved(50.0, 45.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    ui.frame();
    let state = ui.state(buttons[1]).expect("there");
    assert!(state.focused && !state.focus_visible, "{state:?}");
    assert!(!ui.state(buttons[0]).expect("there").focus_visible);
    // Tab again: the focus shows once more.
    ui.key(&named(NamedKey::Tab));
    ui.frame();
    let focus = ui.focus().expect("a focus");
    assert!(ui.state(focus).expect("there").focus_visible);
}

#[test]
fn boxes_lie_on_whole_pixels_at_any_scale() {
    let (mut ui, root) = ui("", 1.5);
    let mut style = sized(10.3, 7.7);
    style.layout.margin.start = 3.3;
    style.layout.margin.top = 1.1;
    style.paint.background = LinearColor::rgba(1.0, 1.0, 1.0, 1.0);
    style.paint.border_width = 0.6;
    style.paint.border = LinearColor::rgba(0.0, 0.0, 0.0, 1.0);
    ui.build(root, 0, |children| {
        children.add("box", Element::container(style), |_| {});
    });
    let (list, _) = ui.frame();
    let boxes: Vec<_> = list
        .items()
        .iter()
        .filter_map(|item| match &item.primitive {
            Primitive::Box(primitive) => Some(*primitive),
            _ => None,
        })
        .collect();
    assert!(!boxes.is_empty());
    for primitive in boxes {
        let rect = primitive.rect;
        for edge in [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height] {
            assert_eq!(edge, edge.round(), "{rect:?}");
        }
        assert_eq!(primitive.border_width, 1.0);
    }
}
