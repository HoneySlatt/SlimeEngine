//! Each target's size and scale, which its host gives and each root on it fills, and the pointer
//! among roots stacked on one target
//! (`docs/adr/0268-each-target-its-size-from-its-host-a-root-fills-it-stacked-roots-take-where-they-act.md`).

use slime_color::LinearColor;
use slime_input::{ButtonState, MouseButton, ScrollDelta};
use uwui::Rect;
use uwui::action::{Action, ActionKind};
use uwui::host::{Person, TargetId, TargetSize};
use uwui::style::{Dimension, StyleBuilder};
use uwui::text::Fonts;
use uwui::tree::{Along, Element, ElementId, Layer, Place, RootSettings, Side, Ui};

const MAIN: TargetId = TargetId::MAIN;

/// `pixels` at `scale`.
fn size(pixels: [u32; 2], scale: f32) -> TargetSize {
    TargetSize::new(pixels, scale).expect("a size")
}

/// Writes `width` by `height` logical pixels, which does not shrink.
fn sized(s: StyleBuilder, width: f32, height: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(height))
        .shrink(0.0)
}

/// An interface of one root on the main target, of 400 by 300 pixels at scale 1, holding a box of
/// 100 by 50 logical pixels: the interface, its root and the box.
fn one_root() -> (Ui, ElementId, ElementId) {
    let mut ui = Ui::new(Fonts::new());
    ui.set_target_size(MAIN, size([400, 300], 1.0));
    let root = ui.add_root("main", RootSettings::on(MAIN), |s| s);
    let mut panel = None;
    ui.build(root, 0, |children| {
        let boxed = Element::container().style(|s| sized(s, 100.0, 50.0));
        panel = Some(children.add("panel", boxed, |_| {}));
    });
    ui.frame(MAIN);
    (ui, root, panel.expect("built"))
}

#[test]
fn a_root_fills_its_target_and_follows_its_size_and_scale_at_the_next_frame() {
    let (mut ui, root, panel) = one_root();
    assert_eq!(ui.rect(root), Some(Rect::new(0.0, 0.0, 400.0, 300.0)));
    // Given, the size waits for the frame; the boxes keep the last one until then.
    ui.set_target_size(MAIN, size([600, 450], 1.5));
    assert_eq!(ui.target_size(MAIN), Some(size([600, 450], 1.5)));
    assert!(ui.needs_frame(MAIN));
    assert_eq!(ui.rect(panel), Some(Rect::new(0.0, 0.0, 100.0, 50.0)));
    let (_, stats) = ui.frame(MAIN);
    assert_eq!(stats.layouts, 1);
    assert_eq!(ui.rect(root), Some(Rect::new(0.0, 0.0, 600.0, 450.0)));
    assert_eq!(ui.rect(panel), Some(Rect::new(0.0, 0.0, 150.0, 75.0)));
}

#[test]
fn a_size_given_again_unchanged_asks_for_no_frame_and_lays_out_nothing() {
    let (mut ui, _, _) = one_root();
    assert!(!ui.needs_frame(MAIN));
    ui.set_target_size(MAIN, size([400, 300], 1.0));
    assert!(!ui.needs_frame(MAIN), "a host gives it at every frame");
    let (_, stats) = ui.frame(MAIN);
    assert_eq!((stats.layouts, stats.painted), (0, 0));
    // Given another, then the one laid out, before a frame: nothing to lay out either.
    ui.set_target_size(MAIN, size([500, 300], 1.0));
    ui.set_target_size(MAIN, size([400, 300], 1.0));
    let (_, stats) = ui.frame(MAIN);
    assert_eq!((stats.layouts, stats.painted), (0, 0));
}

#[test]
fn a_target_given_no_size_lays_out_at_a_pixel_and_one_given_before_its_root_keeps_it() {
    let mut ui = Ui::new(Fonts::new());
    let never_given = ui.add_root("never-given", RootSettings::on(MAIN), |s| s);
    let other = TargetId(5);
    assert_eq!(ui.target_size(other), None);
    ui.set_target_size(other, size([320, 200], 2.0));
    let early = ui.add_root("early", RootSettings::on(other), |s| s);
    ui.frame(MAIN);
    ui.frame(other);
    assert_eq!(ui.rect(never_given), Some(Rect::new(0.0, 0.0, 1.0, 1.0)));
    assert_eq!(ui.rect(early), Some(Rect::new(0.0, 0.0, 320.0, 200.0)));
}

#[test]
fn a_root_moved_to_a_target_of_another_size_takes_it() {
    let (mut ui, root, panel) = one_root();
    let other = TargetId(2);
    ui.set_target_size(other, size([200, 100], 2.0));
    ui.set_root(root, RootSettings::on(other));
    ui.frame(other);
    assert_eq!(ui.rect(root), Some(Rect::new(0.0, 0.0, 200.0, 100.0)));
    assert_eq!(ui.rect(panel), Some(Rect::new(0.0, 0.0, 200.0, 100.0)));
}

/// Two roots on the main target, of 400 by 300 pixels: the lower holds a box of 400 by 100 that
/// draws, at its top, and a button of 100 by 40 below it that draws nothing; the upper a button of
/// 100 by 40 that draws nothing, at its top left, and a box of 100 by 40 that draws, beside it, over
/// the lower's box. The interface, the lower's box and button, the upper's root, button and box.
struct Stacked {
    ui: Ui,
    lower_box: ElementId,
    lower_button: ElementId,
    upper: ElementId,
    upper_button: ElementId,
    upper_box: ElementId,
}

fn stacked() -> Stacked {
    let mut ui = Ui::new(Fonts::new());
    ui.set_target_size(MAIN, size([400, 300], 1.0));
    let grey = LinearColor::rgba(0.5, 0.5, 0.5, 1.0);
    let lower = ui.add_root("lower", RootSettings::on(MAIN), |s| {
        s.direction(uwui::style::FlexDirection::Column)
    });
    let (mut lower_box, mut lower_button) = (None, None);
    ui.build(lower, 0, |children| {
        let drawn = Element::container().style(|s| sized(s, 400.0, 100.0).background(grey));
        lower_box = Some(children.add("box", drawn, |_| {}));
        let bare = Element::button("").style(|s| sized(s, 100.0, 40.0));
        lower_button = Some(children.add("button", bare, |_| {}));
    });
    let upper = ui.add_root("upper", RootSettings::on(MAIN), |s| s);
    let (mut upper_button, mut upper_box) = (None, None);
    ui.build(upper, 0, |children| {
        let bare = Element::button("").style(|s| sized(s, 100.0, 40.0));
        upper_button = Some(children.add("button", bare, |_| {}));
        let drawn = Element::container().style(|s| sized(s, 100.0, 40.0).background(grey));
        upper_box = Some(children.add("box", drawn, |_| {}));
    });
    ui.frame(MAIN);
    Stacked {
        ui,
        lower_box: lower_box.expect("built"),
        lower_button: lower_button.expect("built"),
        upper,
        upper_button: upper_button.expect("built"),
        upper_box: upper_box.expect("built"),
    }
}

#[test]
fn a_root_over_another_takes_the_pointer_where_it_would_take_a_press_and_lets_it_through_elsewhere()
{
    let Stacked {
        mut ui,
        lower_box,
        lower_button,
        upper,
        upper_button,
        upper_box,
    } = stacked();
    // Its button, which draws nothing, over the lower's box that draws: the upper's.
    assert_eq!(ui.element_at(MAIN, 50.0, 20.0), Some(upper_button));
    ui.pointer_moved(50.0, 20.0);
    assert!(ui.pointer_button(MouseButton::Left, ButtonState::Pressed));
    ui.pointer_button(MouseButton::Left, ButtonState::Released);
    assert_eq!(
        ui.take_actions(),
        [Action {
            element: Some(upper_button),
            kind: ActionKind::Press,
        }]
    );
    // Its box that draws: the upper's.
    assert_eq!(ui.element_at(MAIN, 150.0, 20.0), Some(upper_box));
    // Where the upper draws nothing and takes nothing: what lies below, drawn or taking a press.
    assert_eq!(ui.element_at(MAIN, 300.0, 50.0), Some(lower_box));
    assert_eq!(ui.element_at(MAIN, 50.0, 120.0), Some(lower_button));
    // Where neither does: the deepest box of the top root, as a lone root holds it.
    assert_eq!(ui.element_at(MAIN, 300.0, 200.0), Some(upper));
    assert!(!ui.covers(MAIN, 300.0, 200.0));
    assert!(ui.covers(MAIN, 300.0, 50.0));
}

#[test]
fn a_layer_moved_with_its_root_to_a_target_laid_out_at_another_scale_takes_its_scale() {
    let (mut ui, root, panel) = one_root();
    let other = TargetId(2);
    ui.set_target_size(other, size([800, 600], 2.0));
    ui.frame(other);
    let layer = Layer {
        place: Place::Beside {
            element: panel,
            side: Side::Below,
            along: Along::Start,
        },
        modal: false,
        dismiss: false,
    };
    let menu = ui.open_layer("menu", layer, |s| s);
    let mut item = None;
    ui.build(menu, 0, |children| {
        let boxed = Element::container().style(|s| sized(s, 80.0, 30.0));
        item = Some(children.add("item", boxed, |_| {}));
    });
    ui.frame(MAIN);
    ui.set_root(root, RootSettings::on(other));
    ui.frame(other);
    assert_eq!(ui.rect(panel), Some(Rect::new(0.0, 0.0, 200.0, 100.0)));
    let item = ui.rect(item.expect("built")).expect("laid out");
    assert_eq!((item.width, item.height), (160.0, 60.0));
}

#[test]
fn a_root_set_modal_after_it_was_added_changes_nothing() {
    let Stacked {
        mut ui,
        lower_box,
        upper,
        ..
    } = stacked();
    let modal = RootSettings {
        modal: true,
        ..RootSettings::on(MAIN)
    };
    ui.set_root(upper, modal);
    ui.frame(MAIN);
    assert_eq!(ui.element_at(MAIN, 300.0, 50.0), Some(lower_box));
    assert!(ui.covers(MAIN, 300.0, 50.0));
    assert_eq!(ui.root_settings(upper).map(|root| root.modal), Some(false));
}

#[test]
fn the_wheel_over_an_upper_roots_box_that_scrolls_scrolls_it() {
    let mut ui = Ui::new(Fonts::new());
    ui.set_target_size(MAIN, size([400, 300], 1.0));
    let grey = LinearColor::rgba(0.5, 0.5, 0.5, 1.0);
    let lower = ui.add_root("lower", RootSettings::on(MAIN), |s| s);
    let mut below = None;
    ui.build(lower, 0, |children| {
        let scroll = Element::scroll().style(|s| sized(s, 400.0, 300.0).background(grey));
        below = Some(children.add("scroll", scroll, |children| {
            let tall = Element::container().style(|s| sized(s, 400.0, 1000.0));
            children.add("content", tall, |_| {});
        }));
    });
    let upper = ui.add_root("upper", RootSettings::on(MAIN), |s| s);
    let mut above = None;
    ui.build(upper, 0, |children| {
        let scroll = Element::scroll().style(|s| sized(s, 200.0, 200.0));
        above = Some(children.add("scroll", scroll, |children| {
            let tall = Element::container().style(|s| sized(s, 200.0, 1000.0));
            children.add("content", tall, |_| {});
        }));
    });
    ui.frame(MAIN);
    ui.pointer_moved(50.0, 100.0);
    // Turned down, the content goes up: its offset grows.
    assert!(ui.wheel(ScrollDelta::Lines { x: 0.0, y: -1.0 }));
    let scrolled = |id: Option<ElementId>| ui.scroll(id.expect("built")).unwrap_or_default();
    assert!(scrolled(above)[1] > 0.0, "the upper box scrolls");
    assert_eq!(scrolled(below), [0.0; 2], "the lower stays");
}

#[test]
fn a_drop_reaches_a_target_below_an_upper_root_that_draws_nothing_there() {
    let mut ui = Ui::new(Fonts::new());
    ui.set_target_size(MAIN, size([400, 300], 1.0));
    let lower = ui.add_root("lower", RootSettings::on(MAIN), |s| {
        s.padding(uwui::style::Sides {
            start: 0.0,
            end: 0.0,
            top: 100.0,
            bottom: 0.0,
        })
    });
    let mut target = None;
    ui.build(lower, 0, |children| {
        let drop = Element::container()
            .style(|s| sized(s, 100.0, 40.0))
            .drop_target();
        target = Some(children.add("target", drop, |_| {}));
    });
    let upper = ui.add_root("upper", RootSettings::on(MAIN), |s| s);
    ui.build(upper, 0, |children| {
        let source = Element::button("")
            .style(|s| sized(s, 100.0, 40.0))
            .draggable();
        children.add("source", source, |_| {});
    });
    ui.frame(MAIN);
    ui.pointer_moved(50.0, 20.0);
    ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
    ui.pointer_moved(50.0, 120.0);
    let dragging = ui.dragging().expect("a drag");
    assert_eq!(dragging.target, target);
}
