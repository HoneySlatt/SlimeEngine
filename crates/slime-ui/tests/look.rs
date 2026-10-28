//! The editor's look, without a GPU (`docs/adr/0148-widgets.md`,
//! `docs/adr/0149-the-looks-foundations.md`): its fonts registered under their families, which hold
//! the writing systems that it promises, at the weights that it asks of their variable fonts, their
//! digits of one width; its style sheet and stacked themes, which style each kind of element, its
//! states and its classes, and dim what lies below a modal layer, in each theme of colors and
//! density, a project's accent followed by what the look computes from it; Lucide's icons, by name,
//! and the controls' ticks and chevrons drawn from them. Built with the feature `editor-look`
//! alone.

use slime_color::{LinearColor, Oklch};
use slime_ui::display::Primitive;
use slime_ui::look::{self, Colors, Density, FIXED_FAMILY, TEXT_FAMILY};
use slime_ui::style::Style;
use slime_ui::text::{Fonts, TextBounds, TextStyle};
use slime_ui::theme::Theme;
use slime_ui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

/// A color given as sRGB's 0xRRGGBB.
fn srgb(hex: u32) -> LinearColor {
    LinearColor::from_srgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// `color` lighter by `by` of OKLCH's lightness, as the look's `lighten` computes it.
fn lighten(color: LinearColor, by: f32) -> LinearColor {
    let color = color.to_oklch();
    Oklch {
        l: color.l + by,
        ..color
    }
    .to_linear_in_gamut()
}

/// Whether two colors are within 1/1000 on each component.
fn near(a: LinearColor, b: LinearColor) -> bool {
    [(a.r, b.r), (a.g, b.g), (a.b, b.b), (a.a, b.a)]
        .iter()
        .all(|(x, y)| (x - y).abs() < 1e-3)
}

#[test]
fn the_fonts_hold_latin_vietnamese_and_cyrillic_and_vary_in_weight() {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    for family in [TEXT_FAMILY, FIXED_FAMILY] {
        let mut style = TextStyle {
            fonts: vec![family.to_owned()],
            ..TextStyle::default()
        };
        for text in ["Inspector 0123", "Tiếng Việt", "Свойства"] {
            let layout = fonts.lay_out(text, &style, 1.0, TextBounds::default());
            for run in layout.runs() {
                assert_eq!(fonts.family(run.font), family, "{text}");
                // Glyph 0 is the font's mark of a character that it lacks.
                assert!(run.glyphs.iter().all(|glyph| glyph.id != 0), "{text}");
            }
        }
        // A heavier weight is another point of the variable font's axis: another face.
        let regular = fonts.lay_out("Weight", &style, 1.0, TextBounds::default());
        style.weight = 600.0;
        let heavier = fonts.lay_out("Weight", &style, 1.0, TextBounds::default());
        assert_ne!(regular.runs()[0].font, heavier.runs()[0].font, "{family}");
        assert_eq!(fonts.family(heavier.runs()[0].font), family);
    }
}

#[test]
fn geist_s_digits_take_one_width_with_tnum() {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut style = TextStyle {
        fonts: vec![TEXT_FAMILY.to_owned()],
        ..TextStyle::default()
    };
    let width = |fonts: &mut Fonts, style: &TextStyle, text: &str| {
        fonts
            .lay_out(text, style, 1.0, TextBounds::default())
            .width()
    };
    // Proportional by default: a 1 is narrower than a 0.
    assert!(width(&mut fonts, &style, "111") < width(&mut fonts, &style, "000"));
    style.features = vec!["tnum".to_owned()];
    let (ones, zeros) = (
        width(&mut fonts, &style, "111"),
        width(&mut fonts, &style, "000"),
    );
    assert!((ones - zeros).abs() < 0.01, "{ones} {zeros}");
}

/// An interface in the look, its themes `themes`, whose root holds `elements`, keyed by their
/// places: the interface and the elements' ids.
fn styled(themes: &[Theme], elements: Vec<Element>) -> (Ui, Vec<ElementId>) {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = Ui::new(fonts);
    ui.set_style(&[look::style_sheet()], themes)
        .expect("the look's documents are valid");
    let settings = RootSettings {
        size: [800.0, 600.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        for (index, element) in elements.into_iter().enumerate() {
            ids.push(children.add(index, element, |_| {}));
        }
    });
    ui.frame();
    (ui, ids)
}

/// One element of each kind that the look styles, and a panel.
fn every_kind() -> Vec<Element> {
    vec![
        Element::label("Name", Style::default()),
        Element::label("entity-0042", Style::default()).class("fixed"),
        Element::button("Apply", Style::default()),
        Element::check_box("Casts shadows", true, Style::default()),
        Element::slider(0.5, 0.0, 1.0, 0.0, Style::default()),
        Element::section("Transform", true, Style::default()),
        Element::number_field(1.5, 0.1, Style::default()),
        Element::container(Style::default()).class("panel"),
        Element::text_field("name", Style::default()),
    ]
}

#[test]
fn the_style_sheet_styles_each_kind_its_states_and_its_classes() {
    let themes = look::themes(Colors::Dark, Density::Comfortable);
    let (mut ui, ids) = styled(&themes, every_kind());
    let style = |ui: &Ui, index: usize| ui.style(ids[index]).expect("styled").clone();
    let label = style(&ui, 0);
    assert_eq!(label.text.style.fonts, [TEXT_FAMILY]);
    assert_eq!(label.text.style.features, ["tnum"]);
    assert_eq!(label.text.color, srgb(0xE6E7EB));
    assert_eq!(style(&ui, 1).text.style.fonts, [FIXED_FAMILY]);
    // Numbers in the font of fixed width, a section's title heavier and slower to change.
    assert_eq!(style(&ui, 6).text.style.fonts, [FIXED_FAMILY]);
    let section = style(&ui, 5);
    assert_eq!(section.text.style.weight, 600.0);
    assert_eq!(section.transition.duration, 0.2);
    let check_box = style(&ui, 3);
    assert_eq!(check_box.control.fill, srgb(0x4C8DFF));
    assert_eq!(check_box.control.mark, srgb(0xF5F6F8));
    assert_eq!(check_box.transition.duration, 0.12);
    assert_eq!(style(&ui, 4).control.radius, 7.0);
    assert_eq!(style(&ui, 7).paint.background, srgb(0x1E1F24));
    // Focused by the program, as by the keyboard, a button changes the color of its border, not
    // its width, to the accent lightened.
    let before = style(&ui, 2).paint;
    ui.set_focus(Some(ids[2]));
    ui.frame();
    ui.set_reduced_motion(true);
    let focused = style(&ui, 2).paint;
    assert!(
        near(focused.border, lighten(srgb(0x4C8DFF), 0.08)),
        "{focused:?}"
    );
    assert_eq!(focused.border_width, before.border_width);
}

#[test]
fn a_chosen_tab_takes_the_panels_color_and_the_others_a_quieter_text() {
    let themes = look::themes(Colors::Dark, Density::Comfortable);
    let bar = Element::tab_bar(1, Style::default());
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = Ui::new(fonts);
    ui.set_style(&[look::style_sheet()], &themes)
        .expect("the look's documents are valid");
    let settings = RootSettings {
        size: [800.0, 600.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, Style::default());
    let mut tabs = Vec::new();
    ui.build(root, 0, |children| {
        children.add("tabs", bar, |bar| {
            for title in ["Scene", "Lighting"] {
                tabs.push(bar.add(title, Element::tab(title, Style::default()), |_| {}));
            }
        });
    });
    ui.frame();
    let [first, chosen] = [tabs[0], tabs[1]].map(|tab| ui.style(tab).expect("styled").clone());
    assert_eq!(chosen.paint.background, srgb(0x1E1F24));
    assert_eq!(chosen.text.color, srgb(0xE6E7EB));
    assert_eq!(first.text.color, srgb(0x9A9EA8));
}

#[test]
fn every_theme_of_colors_and_density_styles_every_kind() {
    for colors in [Colors::Dark, Colors::Light, Colors::HighContrast] {
        for density in [Density::Comfortable, Density::Compact] {
            let (ui, ids) = styled(&look::themes(colors, density), every_kind());
            let label = ui.style(ids[0]).expect("styled");
            let panel = ui.style(ids[7]).expect("styled");
            // Text that reads on its surface: far apart in lightness.
            let (text, ground) = (
                label.text.color.to_oklch(),
                panel.paint.background.to_oklch(),
            );
            assert!(
                (text.l - ground.l).abs() > 0.55,
                "{colors:?}: {text:?} on {ground:?}"
            );
            let size = ui.style(ids[3]).expect("styled").control.size;
            let expected = match density {
                Density::Comfortable => 14.0,
                Density::Compact => 12.0,
            };
            assert_eq!(size, expected, "{colors:?} {density:?}");
        }
    }
}

#[test]
fn a_project_s_accent_changes_every_color_that_follows_it() {
    let mut themes = look::themes(Colors::Dark, Density::Comfortable);
    themes.push(Theme::from_text("project.theme", "accent = \"#2BB673\"\n"));
    let (mut ui, ids) = styled(&themes, every_kind());
    let check_box = ui.style(ids[3]).expect("styled").clone();
    assert_eq!(check_box.control.fill, srgb(0x2BB673));
    // The field's focus, the accent lightened: green now.
    ui.set_reduced_motion(true);
    ui.set_focus(Some(ids[8]));
    ui.frame();
    let border = ui.style(ids[8]).expect("styled").paint.border;
    assert!(near(border, lighten(srgb(0x2BB673), 0.08)), "{border:?}");
}

#[test]
fn the_controls_draw_their_ticks_and_chevrons_from_lucide_which_names_its_icons() {
    let themes = look::themes(Colors::Dark, Density::Comfortable);
    let (mut ui, ids) = styled(&themes, every_kind());
    let icons = &ui.style(ids[3]).expect("styled").icons;
    assert_eq!(icons.font, look::ICON_FAMILY);
    assert_eq!(
        [&icons.tick, &icons.down, &icons.right, &icons.left].map(|glyph| glyph.chars().next()),
        ["check", "chevron-down", "chevron-right", "chevron-left"].map(look::icon)
    );
    // Lucide draws each of them, and holds all of its icons.
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let style = TextStyle {
        fonts: vec![look::ICON_FAMILY.to_owned()],
        ..TextStyle::default()
    };
    for name in [
        "check",
        "chevron-down",
        "folder",
        "eye",
        "lock",
        "circle-chevrons-up",
    ] {
        let glyph = look::icon(name).expect("an icon of Lucide").to_string();
        let layout = fonts.lay_out(&glyph, &style, 1.0, TextBounds::default());
        let run = &layout.runs()[0];
        assert_eq!(fonts.family(run.font), look::ICON_FAMILY, "{name}");
        assert!(run.glyphs.iter().all(|glyph| glyph.id != 0), "{name}");
    }
    // The checked check box draws its tick, and the open section its chevron, as texts in Lucide.
    let (list, _) = ui.frame();
    let icons_drawn = list
        .items()
        .iter()
        .filter(|item| match &item.primitive {
            Primitive::Text(text) => text
                .layout
                .runs()
                .iter()
                .all(|run| ui.fonts().family(run.font) == look::ICON_FAMILY),
            _ => false,
        })
        .count();
    assert_eq!(icons_drawn, 2);
}

#[test]
fn the_look_styles_what_opens_over_a_panel() {
    use slime_ui::style::Dimension;
    use slime_ui::tree::ItemMark;
    let themes = look::themes(Colors::Dark, Density::Comfortable);
    let elements = vec![
        Element::menu(Style::default()),
        Element::menu_item("Save", "Ctrl+S", ItemMark::None, Style::default()),
        Element::choice(vec!["Low".to_owned()], 0, Style::default()),
        Element::container(Style::default()).class("tooltip"),
        Element::floating_panel("Color", true, Style::default()),
        Element::menu_bar(Style::default()),
    ];
    let (ui, ids) = styled(&themes, elements);
    let style = |index: usize| ui.style(ids[index]).expect("styled");
    // A menu floats over what it opens on: a shadow, a least width, its items in a column.
    let menu = style(0);
    assert!(menu.paint.shadow.is_some());
    assert_eq!(menu.layout.min_size[0], Dimension::Pixels(180.0));
    assert_eq!(
        menu.layout.direction,
        slime_ui::style::FlexDirection::Column
    );
    // An item's shortcut in the quieter color, as a list to choose from's chevron.
    assert!(near(style(1).control.track, style(2).control.mark));
    assert!(style(3).paint.shadow.is_some());
    assert_eq!(style(3).layout.max_size[0], Dimension::Pixels(280.0));
    assert!(style(4).paint.shadow.is_some());
    assert_ne!(style(5).paint.background, style(0).paint.background);
}

#[test]
fn a_modal_layer_dims_the_window_below_it_and_another_does_not() {
    use slime_ui::Rect;
    use slime_ui::tree::{Layer, Place};
    let themes = look::themes(Colors::Dark, Density::Comfortable);
    let (mut ui, ids) = styled(&themes, vec![Element::button("Open", Style::default())]);
    // The boxes that cover the whole window of 800 by 600, with the layer of `modal` open.
    let mut covering = |modal: bool| {
        let place = Place::Floating {
            over: ids[0],
            origin: [200.0, 150.0],
            size: [300.0, 200.0],
        };
        let layer = ui.open_layer(
            "layer",
            Layer {
                place,
                modal,
                dismiss: true,
            },
            Style::default(),
        );
        let (list, _) = ui.frame();
        ui.remove(layer);
        let window = Rect::new(0.0, 0.0, 800.0, 600.0);
        list.items()
            .iter()
            .filter_map(|item| match &item.primitive {
                Primitive::Box(primitive) if primitive.rect == window => Some(primitive.fill),
                _ => None,
            })
            .collect::<Vec<LinearColor>>()
    };
    // The dark theme's scrim, black at three fifths: behind the layer, over the window.
    let scrim = covering(true);
    assert_eq!(scrim.len(), 1);
    assert!(near(
        scrim[0],
        LinearColor::rgba(0.0, 0.0, 0.0, 0x99 as f32 / 255.0)
    ));
    assert!(covering(false).is_empty());
}
