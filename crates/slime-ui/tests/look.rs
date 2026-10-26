//! The editor's look, without a GPU (`docs/adr/0148-widgets.md`): its fonts registered under their
//! families, which hold the writing systems that it promises, at the weights that it asks of their
//! variable fonts; its style sheet and theme, which style each kind of element, its states and its
//! classes. Built with the feature `editor-look` alone.

use slime_color::LinearColor;
use slime_ui::look::{self, FIXED_FAMILY, TEXT_FAMILY};
use slime_ui::style::Style;
use slime_ui::text::{Fonts, TextBounds, TextStyle};
use slime_ui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

/// A color of the look's theme, given as sRGB's 0xRRGGBB.
fn srgb(hex: u32) -> LinearColor {
    LinearColor::from_srgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
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

/// An interface in the look whose root holds `elements`, keyed by their places: the interface and
/// the elements' ids.
fn styled(elements: Vec<Element>) -> (Ui, Vec<ElementId>) {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = Ui::new(fonts);
    ui.set_style(&[look::style_sheet()], &look::theme())
        .expect("the look's documents are valid");
    let settings = RootSettings {
        size: [800.0, 600.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root(settings, Style::default());
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        for (index, element) in elements.into_iter().enumerate() {
            ids.push(children.add(index, element, |_| {}));
        }
    });
    ui.frame();
    (ui, ids)
}

#[test]
fn the_style_sheet_styles_each_kind_its_states_and_its_classes() {
    let (mut ui, ids) = styled(vec![
        Element::label("Name", Style::default()),
        Element::label("entity-0042", Style::default()).class("fixed"),
        Element::button("Apply", Style::default()),
        Element::check_box("Casts shadows", true, Style::default()),
        Element::slider(0.5, 0.0, 1.0, 0.0, Style::default()),
        Element::section("Transform", true, Style::default()),
        Element::number_field(1.5, 0.1, Style::default()),
        Element::container(Style::default()).class("panel"),
    ]);
    let style = |ui: &Ui, index: usize| ui.style(ids[index]).expect("styled").clone();
    let text = srgb(0xE4E5E9);
    let label = style(&ui, 0);
    assert_eq!(label.text.style.fonts, [TEXT_FAMILY]);
    assert_eq!(label.text.color, text);
    assert_eq!(style(&ui, 1).text.style.fonts, [FIXED_FAMILY]);
    // Numbers in the font of fixed width, a section's title heavier.
    assert_eq!(style(&ui, 6).text.style.fonts, [FIXED_FAMILY]);
    assert_eq!(style(&ui, 5).text.style.weight, 600.0);
    let check_box = style(&ui, 3);
    assert_eq!(check_box.control.fill, srgb(0x4C8DFF));
    assert_eq!(check_box.control.mark, srgb(0xF4F5F7));
    assert_eq!(style(&ui, 4).control.radius, 7.0);
    assert_eq!(style(&ui, 7).paint.background, srgb(0x202126));
    // Focused, a button changes the color of its border, not its width.
    let before = style(&ui, 2).paint;
    ui.set_focus(Some(ids[2]));
    ui.frame();
    let focused = style(&ui, 2).paint;
    assert_eq!(focused.border, srgb(0x7AA9FF));
    assert_eq!(focused.border_width, before.border_width);
}

#[test]
fn a_chosen_tab_takes_the_panels_color_and_the_others_a_quieter_text() {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = Ui::new(fonts);
    ui.set_style(&[look::style_sheet()], &look::theme())
        .expect("the look's documents are valid");
    let settings = RootSettings {
        size: [800.0, 600.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root(settings, Style::default());
    let mut tabs = Vec::new();
    ui.build(root, 0, |children| {
        children.add("tabs", Element::tab_bar(1, Style::default()), |bar| {
            for title in ["Scene", "Lighting"] {
                tabs.push(bar.add(title, Element::tab(title, Style::default()), |_| {}));
            }
        });
    });
    ui.frame();
    let [first, chosen] = [tabs[0], tabs[1]].map(|tab| ui.style(tab).expect("styled").clone());
    assert_eq!(chosen.paint.background, srgb(0x202126));
    assert_eq!(chosen.text.color, srgb(0xE4E5E9));
    assert_eq!(first.text.color, srgb(0x9599A2));
}
