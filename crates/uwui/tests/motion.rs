//! The look's foundations, without a GPU (`docs/adr/0149-the-looks-foundations.md`): transitions
//! of colors over the time that the host gives, which less motion drops; a section that grows
//! and shrinks to its height; an opacity that multiplies what an element and its children draw;
//! the focus shown to the keyboard and not to the pointer; boxes placed on whole pixels.

use std::time::Duration;

use slime_color::LinearColor;
use slime_input::{ButtonState, Key, KeyCode, KeyEvent, MouseButton, NamedKey};
use uwui::display::{DisplayList, Primitive};
use uwui::preferences::Preferences;
use uwui::style::{Dimension, FlexDirection, StyleBuilder};
use uwui::text::Fonts;
use uwui::theme::{StyleSheet, Theme};
use uwui::time::{FrameShown, Instant};
use uwui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

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
    let root = ui.add_root("main", settings, |s| s.direction(FlexDirection::Column));
    (ui, root)
}

/// Writes `width` by `height` logical pixels that does not shrink, of the tests' font.
fn sized(s: StyleBuilder, width: f32, height: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(height))
        .shrink(0.0)
        .fonts(vec!["Noto Sans".to_owned()])
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
button {
    select kind=Button
    paint background="#000000"
    transition duration=(s)0.2 easing=Linear
}

button-hovered {
    select kind=Button { states Hovered; }
    paint background="#ffffff"
}
"##;

#[test]
fn a_color_moves_over_the_time_given_and_less_motion_drops_it() {
    let mut at = Instant::ZERO;
    let (mut ui, root) = ui(BUTTONS, 1.0);
    ui.build(root, 0, |children| {
        children.add(
            "button",
            Element::button("go").style(|s| sized(s, 100.0, 30.0)),
            |_| {},
        );
    });
    ui.frame();
    ui.pointer_moved(50.0, 15.0);
    // The hover starts from black: no time has passed.
    let (list, stats) = ui.frame();
    assert_eq!(fill(&list, 100.0), LinearColor::rgba(0.0, 0.0, 0.0, 1.0));
    assert!(stats.animating);
    // Halfway, then at the end, linear in the colors' light.
    at += Duration::from_millis(100);
    ui.set_frame_shown(FrameShown::simulated(at));
    let (list, _) = ui.frame();
    assert!(
        (fill(&list, 100.0).r - 0.5).abs() < 1e-3,
        "{:?}",
        fill(&list, 100.0)
    );
    at += Duration::from_millis(150);
    ui.set_frame_shown(FrameShown::simulated(at));
    let (list, stats) = ui.frame();
    assert_eq!(fill(&list, 100.0), LinearColor::rgba(1.0, 1.0, 1.0, 1.0));
    assert!(!stats.animating);
    // Less motion: the pointer leaves, and the button is black at once.
    ui.set_preferences(Preferences {
        reduced_motion: true,
        ..Preferences::default()
    });
    ui.pointer_moved(300.0, 200.0);
    let (list, stats) = ui.frame();
    assert_eq!(fill(&list, 100.0), LinearColor::rgba(0.0, 0.0, 0.0, 1.0));
    assert!(!stats.animating);
}

#[test]
fn a_new_element_shows_its_style_at_once() {
    let (mut ui, root) = ui(BUTTONS, 1.0);
    ui.build(root, 0, |children| {
        children.add(
            "button",
            Element::button("go").style(|s| sized(s, 100.0, 30.0)),
            |_| {},
        );
    });
    let (list, stats) = ui.frame();
    assert_eq!(fill(&list, 100.0), LinearColor::rgba(0.0, 0.0, 0.0, 1.0));
    assert!(!stats.animating);
}

#[test]
fn an_opacity_multiplies_what_an_element_and_its_children_draw() {
    let (mut ui, root) = ui("", 1.0);
    let outer = Element::container().style(|s| {
        sized(s, 200.0, 100.0)
            .background(LinearColor::rgba(1.0, 0.0, 0.0, 1.0))
            .opacity(0.5)
    });
    let inner = Element::container().style(|s| {
        sized(s, 50.0, 50.0)
            .background(LinearColor::rgba(0.0, 1.0, 0.0, 0.8))
            .opacity(0.5)
    });
    ui.build(root, 0, |children| {
        children.add("outer", outer, |children| {
            children.add("inner", inner, |_| {});
        });
    });
    let (list, _) = ui.frame();
    assert!((fill(&list, 200.0).a - 0.5).abs() < 1e-6);
    // Its own and its parent's: 0.8 × 0.5 × 0.5.
    assert!((fill(&list, 50.0).a - 0.2).abs() < 1e-6);
}

const SECTIONS: &str = r##"
section {
    select kind=Section
    transition duration=(s)0.2 easing=Linear
}
"##;

#[test]
fn a_section_grows_and_shrinks_to_its_height_over_its_transition() {
    let mut at = Instant::ZERO;
    let (mut ui, root) = ui(SECTIONS, 1.0);
    let mut section = None;
    let mut row = None;
    ui.build(root, 0, |children| {
        let physics = Element::section("physics", true).style(|s| {
            s.direction(FlexDirection::Column)
                .fonts(vec!["Noto Sans".to_owned()])
        });
        section = Some(children.add("section", physics, |children| {
            row = Some(children.add(
                "a",
                Element::container().style(|s| sized(s, 100.0, 100.0)),
                |_| {},
            ));
        }));
    });
    ui.frame();
    let (section, row) = (section.expect("built"), row.expect("built"));
    let open = ui.rect(section).expect("laid out").height;
    ui.set_open(section, false);
    ui.frame();
    // Folding: what it holds still shows, cut to its height, which shrinks over the time.
    assert!(ui.rect(row).is_some_and(|rect| rect.height == 100.0));
    at += Duration::from_millis(100);
    ui.set_frame_shown(FrameShown::simulated(at));
    let (_, stats) = ui.frame();
    let half = ui.rect(section).expect("laid out").height;
    assert!(stats.animating);
    assert!(half < open && half > open - 100.0, "{half} of {open}");
    at += Duration::from_millis(150);
    ui.set_frame_shown(FrameShown::simulated(at));
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
    at += Duration::from_millis(250);
    ui.set_frame_shown(FrameShown::simulated(at));
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
            buttons.push(children.add(
                key,
                Element::button(key).style(|s| sized(s, 100.0, 30.0)),
                |_| {},
            ));
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
    let boxed = Element::container().style(|s| {
        sized(s, 10.3, 7.7)
            .margin_start(3.3)
            .margin_top(1.1)
            .background(LinearColor::rgba(1.0, 1.0, 1.0, 1.0))
            .border_width(0.6)
            .border_color(LinearColor::rgba(0.0, 0.0, 0.0, 1.0))
    });
    ui.build(root, 0, |children| {
        children.add("box", boxed, |_| {});
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

/// A rule that sets every color that a style holds, and one that sets them all again under the
/// pointer, which a transition moves between
/// (`docs/adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md`).
const EVERY_COLOR: &str = r##"
all {
    select { classes all; }
    paint background="#000000" border="#000000" backdrop="#000000" {
        shadow color="#000000" blur=4
    }
    text color="#000000" selection="#000000"
    control track="#000000" fill="#000000" mark="#000000"
    transition duration=(s)0.2 easing=Linear
}

all-hovered {
    select { classes all; states Hovered; }
    paint background="#ffffff" border="#ffffff" backdrop="#ffffff" {
        shadow color="#ffffff" blur=4
    }
    text color="#ffffff" selection="#ffffff"
    control track="#ffffff" fill="#ffffff" mark="#ffffff"
}
"##;

#[test]
fn a_transition_moves_every_color_that_a_style_holds() {
    let mut at = Instant::ZERO;
    let (mut ui, root) = ui(EVERY_COLOR, 1.0);
    ui.build(root, 0, |children| {
        children.add(
            "button",
            Element::button("go")
                .class("all")
                .style(|s| sized(s, 100.0, 30.0)),
            |_| {},
        );
    });
    ui.frame();
    let button = ui
        .find(root, &[uwui::tree::Key::from("button")])
        .expect("the button");
    ui.pointer_moved(50.0, 15.0);
    ui.frame();
    at += Duration::from_millis(100);
    ui.set_frame_shown(FrameShown::simulated(at));
    ui.frame();
    let style = ui.style(button).expect("styled");
    let shadow = style.paint.shadow.expect("a shadow").color;
    for (name, color) in [
        ("background", style.paint.background),
        ("border", style.paint.border),
        ("backdrop", style.paint.backdrop),
        ("shadow", shadow),
        ("text", style.text.color),
        ("selection", style.text.selection),
        ("track", style.control.track),
        ("fill", style.control.fill),
        ("mark", style.control.mark),
    ] {
        assert!(
            (color.r - 0.5).abs() < 1e-3,
            "the {name} is halfway, not {color:?}"
        );
    }
}

/// Two buttons of one style, black, that a change of sheets turns `after`, written as a sheet writes
/// a color, over 0.2 s, linear; white under the pointer.
fn two_buttons(after: &str) -> (Ui, ElementId, ElementId) {
    let (mut ui, root) = ui(BUTTONS, 1.0);
    ui.build(root, 0, |children| {
        for key in ["a", "b"] {
            children.add(
                key,
                Element::button("go").style(|s| sized(s, 100.0, 30.0)),
                |_| {},
            );
        }
    });
    ui.frame();
    let changed = BUTTONS.replacen(
        "paint background=\"#000000\"",
        &format!("paint background=\"{after}\""),
        1,
    );
    ui.set_style(
        &[StyleSheet::from_text("changed.style", changed)],
        &[Theme::from_text("empty.theme", "")],
    )
    .expect("the style sheet");
    ui.frame();
    let a = ui.find(root, &[uwui::tree::Key::from("a")]).expect("a");
    let b = ui.find(root, &[uwui::tree::Key::from("b")]).expect("b");
    (ui, a, b)
}

#[test]
fn elements_that_move_alike_share_the_style_that_they_show() {
    let mut at = Instant::ZERO;
    let (mut ui, a, b) = two_buttons("#ffffff");
    at += Duration::from_millis(100);
    ui.set_frame_shown(FrameShown::simulated(at));
    ui.frame();
    let (shown_a, shown_b) = (ui.style(a).expect("a"), ui.style(b).expect("b"));
    assert!((shown_a.paint.background.r - 0.5).abs() < 1e-3);
    assert!(
        std::ptr::eq(shown_a, shown_b),
        "both show one style, made once"
    );
}

#[test]
fn an_element_styled_again_during_its_transition_leaves_the_others() {
    let mut at = Instant::ZERO;
    let (mut ui, a, b) = two_buttons("#808080");
    at += Duration::from_millis(100);
    ui.set_frame_shown(FrameShown::simulated(at));
    ui.frame();
    // The pointer over `a`, halfway: `a` goes on towards its hover, white, `b` towards grey.
    ui.pointer_moved(50.0, 15.0);
    ui.frame();
    let (shown_a, shown_b) = (ui.style(a).expect("a"), ui.style(b).expect("b"));
    assert!(!std::ptr::eq(shown_a, shown_b));
    at += Duration::from_millis(300);
    ui.set_frame_shown(FrameShown::simulated(at));
    let (_, stats) = ui.frame();
    assert!(!stats.animating);
    assert_eq!(
        ui.style(a).expect("a").paint.background,
        LinearColor::rgba(1.0, 1.0, 1.0, 1.0)
    );
    assert_eq!(
        ui.style(b).expect("b").paint.background,
        LinearColor::from_srgb8(128, 128, 128)
    );
}

/// The red channel of `id`'s background, as it shows.
fn red(ui: &Ui, id: ElementId) -> f32 {
    ui.style(id).expect("styled").paint.background.r
}

/// Two buttons, `a` and `b`, in a column, each 100 by 30, of the classes `classes`, none where
/// empty.
fn column_of_two(ui: &mut Ui, root: ElementId, revision: u64, classes: [&str; 2]) {
    ui.build(root, revision, |children| {
        for (key, class) in ["a", "b"].into_iter().zip(classes) {
            let mut element = Element::button("go").style(|s| sized(s, 100.0, 30.0));
            if !class.is_empty() {
                element = element.class(class);
            }
            children.add(key, element, |_| {});
        }
    });
}

/// Two classes, each black, which a change of sheets turns apart.
const TWO_CLASSES: &str = r##"
a {
    select { classes a }
    paint background="#000000"
    transition duration=(s)0.2 easing=Linear
}

b {
    select { classes b }
    paint background="#000000"
    transition duration=(s)0.2 easing=Linear
}
"##;

// Elements that move alike share what they show by the style that they go to, the colors that they
// started from and when they started: each test below needs one of the three.

#[test]
fn elements_from_the_same_colors_at_the_same_time_towards_two_styles_share_nothing() {
    let mut at = Instant::ZERO;
    let (mut ui, root) = ui(TWO_CLASSES, 1.0);
    column_of_two(&mut ui, root, 0, ["a", "b"]);
    ui.frame();
    let changed = TWO_CLASSES
        .replacen("\"#000000\"", "\"#ffffff\"", 1)
        .replacen("\"#000000\"", "\"#808080\"", 1);
    ui.set_style(
        &[StyleSheet::from_text("changed.style", changed)],
        &[Theme::from_text("empty.theme", "")],
    )
    .expect("the style sheet");
    ui.frame();
    at += Duration::from_millis(100);
    ui.set_frame_shown(FrameShown::simulated(at));
    ui.frame();
    let a = ui.find(root, &[uwui::tree::Key::from("a")]).expect("a");
    let b = ui.find(root, &[uwui::tree::Key::from("b")]).expect("b");
    let grey = LinearColor::from_srgb8(128, 128, 128).r;
    assert!((red(&ui, a) - 0.5).abs() < 1e-3, "a is {}", red(&ui, a));
    assert!(
        (red(&ui, b) - grey / 2.0).abs() < 1e-3,
        "b is {}",
        red(&ui, b)
    );
}

#[test]
fn elements_towards_one_style_at_the_same_time_from_two_colors_share_nothing() {
    let mut at = Instant::ZERO;
    let (mut ui, root) = ui(BUTTONS, 1.0);
    column_of_two(&mut ui, root, 0, ["", ""]);
    ui.frame();
    let a = ui.find(root, &[uwui::tree::Key::from("a")]).expect("a");
    let b = ui.find(root, &[uwui::tree::Key::from("b")]).expect("b");
    // `b` hovered, halfway to white.
    ui.pointer_moved(50.0, 45.0);
    ui.frame();
    at += Duration::from_millis(100);
    ui.set_frame_shown(FrameShown::simulated(at));
    ui.frame();
    assert!((red(&ui, b) - 0.5).abs() < 1e-3, "b is {}", red(&ui, b));
    // The base turns grey as the pointer leaves: both go to one style, from two colors.
    let changed = BUTTONS.replacen("\"#000000\"", "\"#808080\"", 1);
    ui.set_style(
        &[StyleSheet::from_text("changed.style", changed)],
        &[Theme::from_text("empty.theme", "")],
    )
    .expect("the style sheet");
    ui.pointer_moved(300.0, 250.0);
    ui.frame();
    at += Duration::from_millis(100);
    ui.set_frame_shown(FrameShown::simulated(at));
    ui.frame();
    let grey = LinearColor::from_srgb8(128, 128, 128).r;
    assert!(
        (red(&ui, a) - grey / 2.0).abs() < 1e-3,
        "a is {}",
        red(&ui, a)
    );
    assert!(
        (red(&ui, b) - (0.5 + grey) / 2.0).abs() < 1e-3,
        "b is {}",
        red(&ui, b)
    );
}

/// A button lit white by a class, which a build gives one button, then the other.
const LIT: &str = r##"
button {
    select kind=Button
    paint background="#000000"
    transition duration=(s)0.2 easing=Linear
}

lit {
    select kind=Button { classes lit }
    paint background="#ffffff"
}
"##;

#[test]
fn elements_towards_one_style_from_one_color_started_apart_share_nothing() {
    let mut at = Instant::ZERO;
    let (mut ui, root) = ui(LIT, 1.0);
    column_of_two(&mut ui, root, 0, ["", ""]);
    ui.frame();
    let a = ui.find(root, &[uwui::tree::Key::from("a")]).expect("a");
    let b = ui.find(root, &[uwui::tree::Key::from("b")]).expect("b");
    column_of_two(&mut ui, root, 1, ["lit", ""]);
    ui.frame();
    at += Duration::from_millis(50);
    ui.set_frame_shown(FrameShown::simulated(at));
    column_of_two(&mut ui, root, 2, ["lit", "lit"]);
    ui.frame();
    at += Duration::from_millis(50);
    ui.set_frame_shown(FrameShown::simulated(at));
    ui.frame();
    assert!((red(&ui, a) - 0.5).abs() < 1e-3, "a is {}", red(&ui, a));
    assert!((red(&ui, b) - 0.25).abs() < 1e-3, "b is {}", red(&ui, b));
}

/// A button transparent white, black under the pointer.
const CLEAR_WHITE: &str = r##"
button {
    select kind=Button
    paint background="#ffffff00"
    transition duration=(s)0.2 easing=Linear
}

button-hovered {
    select kind=Button { states Hovered }
    paint background="#000000"
}
"##;

#[test]
fn the_frame_that_starts_a_transition_shows_exactly_what_showed() {
    let (mut ui, root) = ui(CLEAR_WHITE, 1.0);
    column_of_two(&mut ui, root, 0, ["", ""]);
    ui.frame();
    let a = ui.find(root, &[uwui::tree::Key::from("a")]).expect("a");
    let before = ui.style(a).expect("a").paint.background;
    assert_eq!(before, LinearColor::rgba(1.0, 1.0, 1.0, 0.0));
    ui.pointer_moved(50.0, 15.0);
    ui.frame();
    assert_eq!(ui.style(a).expect("a").paint.background, before);
}
