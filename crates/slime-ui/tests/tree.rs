//! The tree of elements built, laid out by flexbox and grid, built again and drawn into a display
//! list, without a GPU, its texts in the Latin test font of `tests/fonts`.

use slime_color::LinearColor;
use slime_ui::Rect;
use slime_ui::display::Primitive;
use slime_ui::style::{
    Align, Dimension, Display, FlexDirection, GridPlace, LayoutStyle, PaintStyle, Shadow, Sides,
    Style, TextLook, Track,
};
use slime_ui::text::{Fonts, TextStyle};
use slime_ui::theme::{StyleSheet, Theme};
use slime_ui::tree::{Element, ElementId, ElementState, Key, LayoutDirection, RootSettings, Ui};

/// An interface whose texts are laid out with the Latin test font.
fn ui() -> Ui {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    Ui::new(fonts)
}

/// A root of `width` by `height` logical pixels.
fn root(width: f32, height: f32, scale: f32, direction: LayoutDirection) -> RootSettings {
    RootSettings {
        size: [width, height],
        scale,
        direction,
        origin: [0.0; 2],
        modal: false,
    }
}

/// A style of `width` by `height` logical pixels.
fn sized(width: f32, height: f32) -> Style {
    Style {
        layout: LayoutStyle {
            size: [Dimension::Pixels(width), Dimension::Pixels(height)],
            ..LayoutStyle::default()
        },
        ..Style::default()
    }
}

/// A style whose box is filled with `color`.
fn filled(color: LinearColor) -> Style {
    Style {
        paint: PaintStyle {
            background: color,
            ..PaintStyle::default()
        },
        ..Style::default()
    }
}

/// A text style of the test font.
fn text_look() -> TextLook {
    TextLook {
        style: TextStyle {
            fonts: vec!["Noto Sans".to_owned()],
            ..TextStyle::default()
        },
        ..TextLook::default()
    }
}

#[test]
fn a_flexbox_row_is_laid_out_and_mirrored_from_right_to_left() {
    let mut ui = ui();
    let style = Style {
        layout: LayoutStyle {
            padding: Sides {
                start: 10.0,
                end: 30.0,
                top: 20.0,
                bottom: 0.0,
            },
            gap: [0.0, 5.0],
            align_items: Align::Start,
            ..LayoutStyle::default()
        },
        ..Style::default()
    };
    let mut rects = Vec::new();
    for (direction, scale) in [
        (LayoutDirection::LeftToRight, 1.0),
        (LayoutDirection::RightToLeft, 1.0),
        (LayoutDirection::LeftToRight, 2.0),
    ] {
        let root = ui.add_root(root(400.0, 300.0, scale, direction), style.clone());
        let mut ids = Vec::new();
        ui.build(root, 0, |children| {
            ids.push(children.add("a", Element::container(sized(50.0, 40.0)), |_| {}));
            ids.push(children.add("b", Element::container(sized(60.0, 40.0)), |_| {}));
            let mut grows = sized(0.0, 40.0);
            grows.layout.size[0] = Dimension::Auto;
            grows.layout.grow = 1.0;
            ids.push(children.add("c", Element::container(grows), |_| {}));
        });
        rects.push(ids);
    }
    ui.frame();
    let at = |ids: &[ElementId]| {
        ids.iter()
            .map(|&id| ui.rect(id).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        at(&rects[0]),
        [
            Rect::new(10.0, 20.0, 50.0, 40.0),
            Rect::new(65.0, 20.0, 60.0, 40.0),
            Rect::new(130.0, 20.0, 240.0, 40.0),
        ]
    );
    // The start on the right: the padding at the start too.
    assert_eq!(
        at(&rects[1]),
        [
            Rect::new(340.0, 20.0, 50.0, 40.0),
            Rect::new(275.0, 20.0, 60.0, 40.0),
            Rect::new(30.0, 20.0, 240.0, 40.0),
        ]
    );
    assert_eq!(
        at(&rects[2]),
        [
            Rect::new(20.0, 40.0, 100.0, 80.0),
            Rect::new(130.0, 40.0, 120.0, 80.0),
            Rect::new(260.0, 40.0, 480.0, 80.0),
        ]
    );
}

#[test]
fn a_grid_is_laid_out_and_mirrored_from_right_to_left() {
    let mut ui = ui();
    let style = Style {
        layout: LayoutStyle {
            display: Display::Grid,
            columns: vec![Track::Pixels(100.0), Track::Fraction(1.0)],
            rows: vec![Track::Pixels(50.0), Track::Auto],
            gap: [10.0, 20.0],
            ..LayoutStyle::default()
        },
        ..Style::default()
    };
    let mut rects = Vec::new();
    for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
        let root = ui.add_root(root(300.0, 200.0, 1.0, direction), style.clone());
        let mut ids = Vec::new();
        ui.build(root, 0, |children| {
            ids.push(children.add("a", Element::container(Style::default()), |_| {}));
            ids.push(children.add("b", Element::container(Style::default()), |_| {}));
            let mut across = Style::default();
            across.layout.size[1] = Dimension::Pixels(30.0);
            across.layout.column = GridPlace {
                start: Some(1),
                span: 2,
            };
            ids.push(children.add("c", Element::container(across), |_| {}));
        });
        rects.push(ids);
    }
    ui.frame();
    let at = |ids: &[ElementId]| {
        ids.iter()
            .map(|&id| ui.rect(id).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        at(&rects[0]),
        [
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Rect::new(120.0, 0.0, 180.0, 50.0),
            Rect::new(0.0, 60.0, 300.0, 30.0),
        ]
    );
    assert_eq!(
        at(&rects[1]),
        [
            Rect::new(200.0, 0.0, 100.0, 50.0),
            Rect::new(0.0, 0.0, 180.0, 50.0),
            Rect::new(0.0, 60.0, 300.0, 30.0),
        ]
    );
}

#[test]
fn a_label_is_as_large_as_its_text_and_breaks_its_lines_in_a_narrow_box() {
    let mut ui = ui();
    let style = Style {
        layout: LayoutStyle {
            direction: FlexDirection::Column,
            align_items: Align::Start,
            ..LayoutStyle::default()
        },
        ..Style::default()
    };
    let root = ui.add_root(root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight), style);
    let words = "Some words that take more than one line";
    let (mut wide, mut narrow) = (None, None);
    ui.build(root, 0, |children| {
        let look = Style {
            text: text_look(),
            ..Style::default()
        };
        wide = Some(children.add("wide", Element::label(words, look.clone()), |_| {}));
        let mut boxed = look;
        boxed.layout.max_size[0] = Dimension::Pixels(80.0);
        narrow = Some(children.add("narrow", Element::label(words, boxed), |_| {}));
    });
    let (list, _) = ui.frame();
    let (wide, narrow) = (
        ui.rect(wide.unwrap()).unwrap(),
        ui.rect(narrow.unwrap()).unwrap(),
    );
    let laid = ui
        .fonts_mut()
        .lay_out(words, &text_look().style, 1.0, Default::default());
    assert_eq!(
        (wide.width, wide.height),
        (laid.width().ceil(), laid.height().ceil())
    );
    assert!(narrow.width <= 80.0, "{narrow:?}");
    assert!(narrow.height >= 3.0 * wide.height, "{narrow:?} {wide:?}");
    // Each label draws its text at its box's corner, the narrow one broken in its box.
    let texts: Vec<_> = list
        .items()
        .iter()
        .filter_map(|item| match &item.primitive {
            Primitive::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0].origin, [wide.x, wide.y]);
    assert_eq!(texts[1].origin, [narrow.x, narrow.y]);
    assert_eq!(
        texts[1].layout.lines().len() as f32 * wide.height,
        narrow.height
    );
}

#[test]
fn a_panel_built_again_keeps_its_elements_and_their_states_by_their_keys() {
    let mut ui = ui();
    let root = ui.add_root(
        root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
    let button = |label: &str| Element::button(label, Style::default());
    let (mut ok, mut cancel) = (None, None);
    ui.build(root, 1, |children| {
        ok = Some(children.add("ok", button("OK"), |_| {}));
        cancel = Some(children.add("cancel", button("Cancel"), |_| {}));
    });
    let (ok, cancel) = (ok.unwrap(), cancel.unwrap());
    let hovered = ElementState {
        hovered: true,
        ..ElementState::default()
    };
    ui.set_state(cancel, hovered);
    let mut help = None;
    let mut kept = None;
    ui.build(root, 2, |children| {
        help = Some(children.add("help", button("Help"), |_| {}));
        kept = Some(children.add("cancel", button("Cancel"), |_| {}));
    });
    assert_eq!(kept, Some(cancel));
    assert_eq!(ui.children(root), [help.unwrap(), cancel]);
    assert_eq!(ui.state(cancel), Some(hovered));
    assert_eq!(
        ui.element(ok),
        None,
        "an element no longer built is removed"
    );
    assert_eq!(ui.path(cancel), Some(vec![Key::from("cancel")]));
    assert_eq!(ui.find(root, &[Key::from("cancel")]), Some(cancel));
    // Another kind under the same key is another element.
    let mut label = None;
    ui.build(root, 3, |children| {
        label = Some(children.add("cancel", Element::label("Cancel", Style::default()), |_| {}));
    });
    assert_ne!(label, Some(cancel));
    assert_eq!(ui.state(label.unwrap()), Some(ElementState::default()));
    assert_eq!(ui.element(cancel), None);
}

#[test]
#[should_panic(expected = "two siblings of the key `a`")]
fn two_siblings_of_one_key_are_refused() {
    let mut ui = ui();
    let root = ui.add_root(
        root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
    ui.build(root, 0, |children| {
        children.add("a", Element::container(Style::default()), |_| {});
        children.add("a", Element::container(Style::default()), |_| {});
    });
}

#[test]
fn a_frame_where_nothing_changed_lays_out_and_draws_nothing() {
    let mut ui = ui();
    let root = ui.add_root(
        root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
    let build = |ui: &mut Ui, revision: u64, color: LinearColor, label: &str| {
        let mut ids = Vec::new();
        ui.build(root, revision, |children| {
            ids.push(children.add("box", Element::container(filled(color)), |_| {}));
            let look = Style {
                text: text_look(),
                ..Style::default()
            };
            ids.push(children.add("label", Element::label(label, look), |_| {}));
            ids.push(children.add("button", Element::button("Go", Style::default()), |_| {}));
        });
        ids
    };
    let red = LinearColor::rgba(1.0, 0.0, 0.0, 1.0);
    let ids = build(&mut ui, 1, red, "Text");
    let (_, stats) = ui.frame();
    assert_eq!(
        (stats.built, stats.layouts, stats.painted, stats.elements),
        (1, 1, 4, 4)
    );
    // Nothing given: nothing done.
    let (_, stats) = ui.frame();
    assert_eq!(
        (stats.built, stats.layouts, stats.lists, stats.painted),
        (0, 0, 0, 0)
    );
    // The same revision: the builder does not run.
    build(&mut ui, 1, red, "Text");
    let (_, stats) = ui.frame();
    assert_eq!((stats.built, stats.layouts, stats.painted), (0, 0, 0));
    // Built again the same: nothing laid out nor drawn.
    build(&mut ui, 2, red, "Text");
    let (_, stats) = ui.frame();
    assert_eq!((stats.built, stats.layouts, stats.painted), (1, 0, 0));
    // A color: drawn again alone.
    build(&mut ui, 3, LinearColor::WHITE, "Text");
    let (_, stats) = ui.frame();
    assert_eq!((stats.layouts, stats.painted), (0, 1));
    // A state: its style made again, the same without rules that select by states, so nothing
    // drawn again.
    ui.set_state(
        ids[2],
        ElementState {
            pressed: true,
            ..ElementState::default()
        },
    );
    let (_, stats) = ui.frame();
    assert_eq!((stats.styled, stats.layouts, stats.painted), (1, 0, 0));
    // A text: laid out again, and drawn again where it moved.
    build(&mut ui, 4, LinearColor::WHITE, "A longer text");
    let (_, stats) = ui.frame();
    assert_eq!((stats.layouts, stats.painted), (1, 2));
}

#[test]
fn a_list_of_100_000_rows_builds_lays_out_and_draws_only_the_rows_that_show() {
    let mut ui = ui();
    let root = ui.add_root(
        root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
    let mut list = None;
    let mut build = |ui: &mut Ui, revision: u64| {
        ui.build(root, revision, |children| {
            let mut style = Style::default();
            style.layout.grow = 1.0;
            list = Some(
                children.add("list", Element::list(100_000, 20.0, style), |rows| {
                    for row in rows.rows() {
                        let look = Style {
                            text: text_look(),
                            ..Style::default()
                        };
                        rows.add(row, Element::label(format!("Row {row}"), look), |_| {});
                    }
                }),
            );
        });
        list.unwrap()
    };
    // The first frame lays the list out; the next builds the rows that it shows.
    let list = build(&mut ui, 0);
    ui.frame();
    assert_eq!(ui.children(list).len(), 0);
    build(&mut ui, 0);
    let (_, stats) = ui.frame();
    assert_eq!((stats.built, stats.layouts, stats.lists), (1, 0, 1));
    assert_eq!(ui.children(list).len(), 15);
    assert_eq!(stats.elements, 2 + 15);
    let row = ui.children(list)[3];
    assert_eq!(ui.rect(row), Some(Rect::new(0.0, 60.0, 400.0, 20.0)));
    // Scrolled far: the rows that show then, at their places, cut to the list.
    ui.scroll_to(list, [0.0, 20.0 * 50_000.0 + 10.0]);
    build(&mut ui, 0);
    let (display, stats) = ui.frame();
    assert_eq!((stats.built, stats.layouts, stats.lists), (1, 0, 1));
    let rows = ui.children(list);
    assert_eq!(rows.len(), 16);
    assert_eq!(
        ui.path(rows[0]),
        Some(vec![Key::from("list"), Key::from(50_000_usize)])
    );
    assert_eq!(ui.rect(rows[0]), Some(Rect::new(0.0, -10.0, 400.0, 20.0)));
    assert_eq!(stats.painted, 16);
    assert_eq!(display.len(), 16);
    let cut = Some(Rect::new(0.0, 0.0, 400.0, 300.0));
    assert!(display.items().iter().all(|item| item.clip == cut));
    // Standing still, it costs nothing.
    build(&mut ui, 0);
    let (_, stats) = ui.frame();
    assert_eq!(
        (stats.built, stats.layouts, stats.lists, stats.painted),
        (0, 0, 0, 0)
    );
    // Past its end, it stops at its last row.
    ui.scroll_to(list, [0.0, 1.0e9]);
    assert_eq!(ui.scroll(list), Some([0.0, 100_000.0 * 20.0 - 300.0]));
    build(&mut ui, 0);
    ui.frame();
    let rows = ui.children(list);
    assert_eq!(
        ui.path(*rows.last().unwrap()).unwrap()[1],
        Key::from(99_999_usize)
    );
    assert_eq!(
        ui.rect(*rows.last().unwrap()),
        Some(Rect::new(0.0, 280.0, 400.0, 20.0))
    );
}

#[test]
fn a_box_that_scrolls_moves_its_content_cut_to_its_edges_without_laying_it_out() {
    let mut ui = ui();
    let column = Style {
        layout: LayoutStyle {
            direction: FlexDirection::Column,
            ..LayoutStyle::default()
        },
        ..Style::default()
    };
    let root = ui.add_root(
        root(200.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        column.clone(),
    );
    let mut scroll = None;
    let mut items = Vec::new();
    ui.build(root, 0, |children| {
        let mut style = column.clone();
        style.layout.size[1] = Dimension::Pixels(100.0);
        scroll = Some(children.add("scroll", Element::scroll(style), |children| {
            for index in 0..5_usize {
                let style = Style {
                    layout: LayoutStyle {
                        size: [Dimension::Auto, Dimension::Pixels(40.0)],
                        shrink: 0.0,
                        ..LayoutStyle::default()
                    },
                    ..filled(LinearColor::WHITE)
                };
                items.push(children.add(index, Element::container(style), |_| {}));
            }
        }));
    });
    let scroll = scroll.unwrap();
    ui.frame();
    assert_eq!(ui.rect(scroll), Some(Rect::new(0.0, 0.0, 200.0, 100.0)));
    assert_eq!(ui.rect(items[1]), Some(Rect::new(0.0, 40.0, 200.0, 40.0)));
    ui.scroll_to(scroll, [0.0, 30.0]);
    let (display, stats) = ui.frame();
    assert_eq!((stats.layouts, stats.painted), (0, 5));
    assert_eq!(ui.rect(items[0]), Some(Rect::new(0.0, -30.0, 200.0, 40.0)));
    let cut = Some(Rect::new(0.0, 0.0, 200.0, 100.0));
    assert!(display.items().iter().all(|item| item.clip == cut));
    // Within its content: 5 items of 40 pixels in a box of 100.
    ui.scroll_to(scroll, [50.0, 500.0]);
    assert_eq!(ui.scroll(scroll), Some([0.0, 100.0]));
}

#[test]
fn the_display_list_draws_each_element_before_its_children_as_its_state_and_direction_say() {
    let mut ui = ui();
    let blue = LinearColor::rgba(0.0, 0.0, 1.0, 1.0);
    let green = LinearColor::rgba(0.0, 1.0, 0.0, 1.0);
    let corners = PaintStyle {
        background: blue,
        radii: [1.0, 2.0, 3.0, 4.0],
        shadow: Some(Shadow {
            color: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
            offset: [3.0, 5.0],
            blur: 4.0,
            spread: 1.0,
        }),
        ..PaintStyle::default()
    };
    let sheet = StyleSheet::from_text(
        "buttons.style",
        "[button]\nselect = { kind = \"Button\" }\nstyle.paint.background = \"$blue\"\n\n\
         [hovered]\nselect = { kind = \"Button\", states = [\"Hovered\"] }\n\
         style.paint.background = \"#00ff00\"\n",
    );
    let theme = Theme::from_text("plain.theme", "blue = \"#0000ff\"\n");
    ui.set_style(&[sheet], &theme)
        .expect("a style sheet and a theme");
    let mut drawn = Vec::new();
    for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
        let mut ui_root = filled(LinearColor::WHITE);
        ui_root.layout.align_items = Align::Start;
        let root = ui.add_root(root(200.0, 100.0, 2.0, direction), ui_root);
        let mut button = None;
        ui.build(root, 0, |children| {
            let mut panel = sized(100.0, 50.0);
            panel.paint = corners;
            children.add("panel", Element::container(panel), |children| {
                // Its fill from the rules, which style its states.
                let style = Style {
                    text: text_look(),
                    ..Style::default()
                };
                button = Some(children.add("button", Element::button("Go", style), |_| {}));
                let mut hidden = filled(blue);
                hidden.layout.display = Display::None;
                children.add("hidden", Element::container(hidden), |_| {});
            });
        });
        ui.set_state(
            button.unwrap(),
            ElementState {
                hovered: true,
                ..ElementState::default()
            },
        );
        drawn.push(button.unwrap());
    }
    let (display, _) = ui.frame();
    let items: Vec<_> = display.items().iter().map(|item| &item.primitive).collect();
    // Per root: its box, the panel's shadow and box, the button's box and text.
    assert_eq!(items.len(), 10);
    for (root, items) in items.chunks(5).enumerate() {
        let Primitive::Box(background) = items[0] else {
            panic!("the root's box first: {:?}", items[0]);
        };
        assert_eq!(background.rect, Rect::new(0.0, 0.0, 400.0, 200.0));
        let (Primitive::Shadow(shadow), Primitive::Box(panel)) = (items[1], items[2]) else {
            panic!("the panel's shadow, then its box: {items:?}");
        };
        let Primitive::Box(button) = items[3] else {
            panic!("the button's box: {:?}", items[3]);
        };
        assert_eq!(button.fill, green, "the hovered button");
        assert!(matches!(items[4], Primitive::Text(_)));
        assert_eq!(Some(button.rect), ui.rect(drawn[root]));
        if root == 0 {
            assert_eq!(panel.rect, Rect::new(0.0, 0.0, 200.0, 100.0));
            assert_eq!(panel.radii, [2.0, 4.0, 6.0, 8.0]);
            assert_eq!(shadow.offset, [6.0, 10.0]);
        } else {
            // The start on the right: the corners and the shadow mirrored.
            assert_eq!(panel.rect, Rect::new(200.0, 0.0, 200.0, 100.0));
            assert_eq!(panel.radii, [4.0, 2.0, 8.0, 6.0]);
            assert_eq!(shadow.offset, [-6.0, 10.0]);
        }
        assert_eq!((shadow.blur, shadow.spread), (8.0, 2.0));
    }
}

#[test]
fn a_root_at_another_scale_is_laid_out_again_in_its_pixels() {
    let mut ui = ui();
    let settings = root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight);
    let root = ui.add_root(settings, Style::default());
    let mut panel = None;
    ui.build(root, 0, |children| {
        panel = Some(children.add("panel", Element::container(sized(100.0, 50.0)), |_| {}));
    });
    ui.frame();
    ui.set_root(
        root,
        RootSettings {
            scale: 1.5,
            ..settings
        },
    );
    let (_, stats) = ui.frame();
    assert_eq!((stats.layouts, stats.painted), (1, 2));
    assert_eq!(
        ui.rect(panel.unwrap()),
        Some(Rect::new(0.0, 0.0, 150.0, 75.0))
    );
    ui.remove(panel.unwrap());
    assert_eq!(ui.element(panel.unwrap()), None);
    assert_eq!(ui.len(), 1);
}

#[test]
fn a_panel_in_a_slot_is_built_by_its_own_revision_and_kept_when_its_parent_is_built_again() {
    let mut ui = ui();
    let root = ui.add_root(
        root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
    let mut panel = None;
    let mut build_root = |ui: &mut Ui, revision: u64| {
        ui.build(root, revision, |children| {
            panel = Some(children.add_slot("panel", Element::container(Style::default())));
        });
        panel.unwrap()
    };
    let panel = build_root(&mut ui, 0);
    let built = std::cell::Cell::new(0);
    let build_panel = |ui: &mut Ui, revision: u64| {
        ui.build(panel, revision, |children| {
            built.set(built.get() + 1);
            children.add("label", Element::label("Panel", Style::default()), |_| {});
        });
    };
    build_panel(&mut ui, 0);
    build_panel(&mut ui, 0);
    assert_eq!(built.get(), 1);
    let label = ui.children(panel)[0];
    assert_eq!(build_root(&mut ui, 1), panel);
    assert_eq!(ui.children(panel), [label]);
    build_panel(&mut ui, 1);
    assert_eq!(built.get(), 2);
    assert_eq!(ui.children(panel), [label]);
}

/// A style that grows to fill its parent's main axis.
fn grows() -> Style {
    let mut style = Style::default();
    style.layout.grow = 1.0;
    style
}

#[test]
fn a_list_in_a_slot_builds_its_rows_with_its_own_builder_and_lays_out_only_them() {
    let mut ui = ui();
    let root = ui.add_root(
        root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
    let (root_built, rows_built) = (std::cell::Cell::new(0), std::cell::Cell::new(0));
    let frame = |ui: &mut Ui, revision: u64, row_height: f32, label: &str| {
        let mut list = None;
        ui.build(root, revision, |children| {
            root_built.set(root_built.get() + 1);
            list = Some(children.add_slot("list", Element::list(1_000, row_height, grows())));
        });
        let list = list.unwrap_or_else(|| ui.children(root)[0]);
        ui.build(list, revision, |rows| {
            rows_built.set(rows_built.get() + 1);
            for row in rows.rows() {
                rows.add(row, Element::container(Style::default()), |children| {
                    let text = if row == 3 { label } else { "Row" };
                    let look = Style {
                        text: text_look(),
                        ..Style::default()
                    };
                    children.add("name", Element::label(text, look), |_| {});
                });
            }
        });
        (list, ui.frame().1)
    };
    frame(&mut ui, 0, 20.0, "Row");
    let (list, stats) = frame(&mut ui, 0, 20.0, "Row");
    assert_eq!((root_built.get(), rows_built.get()), (1, 2));
    assert_eq!(ui.children(list).len(), 15);
    assert_eq!((stats.layouts, stats.lists), (0, 1));
    // A change deep in a row lays out the list's rows alone.
    let (_, stats) = frame(&mut ui, 1, 20.0, "A longer row");
    assert_eq!((stats.layouts, stats.lists), (0, 1));
    // Rows of another height: those kept take it.
    frame(&mut ui, 2, 30.0, "Row");
    let (_, stats) = frame(&mut ui, 2, 30.0, "Row");
    assert_eq!(stats.layouts, 0);
    assert_eq!(ui.children(list).len(), 10);
    let row = ui.children(list)[1];
    assert_eq!(ui.rect(row), Some(Rect::new(0.0, 30.0, 400.0, 30.0)));
}

#[test]
fn a_list_in_the_rows_of_another_shows_its_rows_after_the_other_scrolls() {
    let mut ui = ui();
    let root = ui.add_root(
        root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
    let mut outer = None;
    let mut frame = |ui: &mut Ui| {
        ui.build(root, 0, |children| {
            outer = Some(
                children.add("outer", Element::list(50, 100.0, grows()), |rows| {
                    for row in rows.rows() {
                        rows.add(row, Element::container(Style::default()), |children| {
                            children.add("inner", Element::list(10, 20.0, grows()), |rows| {
                                for row in rows.rows() {
                                    let element = Element::container(filled(LinearColor::WHITE));
                                    rows.add(row, element, |_| {});
                                }
                            });
                        });
                    }
                }),
            );
        });
        ui.frame();
        outer.unwrap()
    };
    for _ in 0..3 {
        frame(&mut ui);
    }
    let outer = frame(&mut ui);
    ui.scroll_to(outer, [0.0, 250.0]);
    for _ in 0..3 {
        frame(&mut ui);
    }
    let rows = ui.children(outer).to_vec();
    assert_eq!(rows.len(), 4, "rows 2 to 5 show");
    for row in rows {
        let inner = ui.children(row)[0];
        let inner_rows = ui.children(inner);
        assert_eq!(
            inner_rows.len(),
            5,
            "each inner list shows 5 rows of 20 pixels"
        );
        let (outer_rect, first) = (ui.rect(row).unwrap(), ui.rect(inner_rows[0]).unwrap());
        assert_eq!(first, Rect::new(0.0, outer_rect.y, 400.0, 20.0));
    }
}

#[test]
fn what_an_element_whose_display_is_none_holds_takes_no_space() {
    let mut ui = ui();
    let root = ui.add_root(
        root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
    let (mut hidden, mut child) = (None, None);
    ui.build(root, 0, |children| {
        let mut style = sized(100.0, 100.0);
        style.layout.display = Display::None;
        hidden = Some(
            children.add("hidden", Element::container(style), |children| {
                let label = Style {
                    text: text_look(),
                    ..sized(50.0, 40.0)
                };
                child = Some(children.add("child", Element::label("Hidden", label), |_| {}));
            }),
        );
    });
    let (list, _) = ui.frame();
    assert!(list.is_empty());
    for id in [hidden.unwrap(), child.unwrap()] {
        let rect = ui.rect(id).unwrap();
        assert_eq!((rect.width, rect.height), (0.0, 0.0));
    }
}

#[test]
fn a_root_turned_to_the_other_direction_draws_every_element_again_mirrored() {
    let mut ui = ui();
    let settings = root(400.0, 300.0, 1.0, LayoutDirection::LeftToRight);
    let root = ui.add_root(settings, Style::default());
    ui.build(root, 0, |children| {
        let mut bar = filled(LinearColor::WHITE);
        bar.layout.size = [Dimension::Share(1.0), Dimension::Pixels(50.0)];
        bar.paint.radii = [8.0, 0.0, 0.0, 0.0];
        children.add("bar", Element::container(bar), |_| {});
    });
    ui.frame();
    ui.set_root(
        root,
        RootSettings {
            direction: LayoutDirection::RightToLeft,
            ..settings
        },
    );
    let (list, stats) = ui.frame();
    assert_eq!((stats.layouts, stats.painted), (1, 2));
    let Primitive::Box(bar) = &list.items()[0].primitive else {
        panic!("the bar's box");
    };
    assert_eq!(bar.rect, Rect::new(0.0, 0.0, 400.0, 50.0));
    assert_eq!(
        bar.radii,
        [0.0, 8.0, 0.0, 0.0],
        "its top start corner on the right"
    );
}

#[test]
#[should_panic(expected = "a root's scale must be a finite number above 0, not 0")]
fn a_root_of_no_scale_is_refused() {
    let mut ui = ui();
    ui.add_root(
        root(400.0, 300.0, 0.0, LayoutDirection::LeftToRight),
        Style::default(),
    );
}

#[test]
fn a_box_that_scrolls_from_right_to_left_scrolls_from_its_start_and_cuts_inside_its_border() {
    let mut ui = ui();
    let root = ui.add_root(
        root(200.0, 100.0, 1.0, LayoutDirection::RightToLeft),
        Style::default(),
    );
    let mut scroll = None;
    let mut items = Vec::new();
    ui.build(root, 0, |children| {
        let mut style = sized(100.0, 50.0);
        style.paint.border_width = 2.0;
        style.paint.border = LinearColor::WHITE;
        scroll = Some(children.add("scroll", Element::scroll(style), |children| {
            for index in 0..3_usize {
                let mut item = Style {
                    layout: sized(60.0, 50.0).layout,
                    ..filled(LinearColor::WHITE)
                };
                item.layout.shrink = 0.0;
                items.push(children.add(index, Element::container(item), |_| {}));
            }
        }));
    });
    ui.frame();
    let scroll = scroll.unwrap();
    assert_eq!(ui.rect(scroll), Some(Rect::new(100.0, 0.0, 100.0, 50.0)));
    ui.scroll_to(scroll, [30.0, 0.0]);
    let (list, _) = ui.frame();
    // 30 pixels from its start, which is on the right: its first item moved right.
    assert_eq!(ui.rect(items[0]), Some(Rect::new(170.0, 0.0, 60.0, 50.0)));
    let inside = Some(Rect::new(102.0, 2.0, 96.0, 46.0));
    let cut: Vec<_> = list.items().iter().skip(1).map(|item| item.clip).collect();
    assert_eq!(cut, [inside; 3]);
    // Within its content: three items of 60 pixels in a box of 100.
    ui.scroll_to(scroll, [500.0, 0.0]);
    assert_eq!(ui.scroll(scroll), Some([80.0, 0.0]));
}
