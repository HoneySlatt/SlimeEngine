//! Style sheets and themes: rules that select elements by kind, classes, key, states and ancestor,
//! applied in their order, the element's own style over them, their values named by a theme or
//! given in sRGB; the elements styled again only where something changed; errors that name the
//! document and keep the style that the interface had.

use slime_color::LinearColor;
use uwui::style::{Dimension, StyleBuilder};
use uwui::text::Fonts;
use uwui::theme::{StyleSheet, Theme};
use uwui::tree::{Element, ElementId, ElementState, LayoutDirection, RootSettings, Ui};

/// An interface of one root of 400 × 300 logical pixels, without fonts: no element here has text.
fn ui() -> (Ui, ElementId) {
    let mut ui = Ui::new(Fonts::new());
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, |s| s);
    (ui, root)
}

/// The color that `hex`, six hexadecimal digits, gives in sRGB.
fn srgb(hex: u32) -> LinearColor {
    LinearColor::from_srgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The fill that `id` is drawn with, as its style says after the last frame.
fn fill(ui: &Ui, id: ElementId) -> LinearColor {
    ui.style(id).expect("an element").paint.background
}

const SHEET: &str = r##"
box {
    select kind=Container
    paint background=$surface
    layout { padding start=4 end=4 top=$gap bottom=$gap; }
}

panel {
    select { classes panel; }
    paint background=$panel { radii $radius $radius $radius $radius; }
}

toolbar-item {
    select { inside class=toolbar; }
    paint background="#102030"
}

named {
    select key=special
    paint border_width=3
}

pressed {
    select { states Pressed; }
    paint background=$accent
}
"##;

const DARK: &str = r##"
color {
    surface "#202020"
    panel "#303030"
    accent "#4c6ef5"
}
length {
    radius 8
    gap 2
}
"##;

const LIGHT: &str = r##"
color {
    surface "#f0f0f0"
    panel "#e0e0e0"
    accent "#1c3ed5"
}
length {
    radius 4
    gap 6
}
"##;

/// Builds a box, a panel that `panel` styles, a toolbar holding an item, and a box keyed `special`.
fn build(
    ui: &mut Ui,
    root: ElementId,
    revision: u64,
    panel: impl FnOnce(StyleBuilder) -> StyleBuilder,
) -> Vec<ElementId> {
    let mut ids = Vec::new();
    ui.build(root, revision, |children| {
        ids.push(children.add("box", Element::container(), |_| {}));
        let element = Element::container().style(panel).class("panel");
        ids.push(children.add("panel", element, |_| {}));
        let toolbar = Element::container().class("toolbar");
        children.add("toolbar", toolbar, |children| {
            ids.push(children.add("item", Element::container(), |_| {}));
        });
        ids.push(children.add("special", Element::container(), |_| {}));
    });
    ids
}

#[test]
fn rules_select_by_kind_class_key_state_and_ancestor_in_the_order_of_their_file() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, |s| s);
    ui.set_style(
        &[StyleSheet::from_text("ui.style", SHEET)],
        &[Theme::from_text("dark.theme", DARK)],
    )
    .expect("a style sheet and a theme");
    ui.frame();
    let [plain, panel, item, special] = [ids[0], ids[1], ids[2], ids[3]];
    assert_eq!(fill(&ui, plain), srgb(0x202020));
    // The panel's rule comes after the boxes': its fill wins, and its corners are the theme's.
    assert_eq!(fill(&ui, panel), srgb(0x303030));
    assert_eq!(ui.style(panel).unwrap().paint.radii, [8.0; 4]);
    // Inside the toolbar, at any depth.
    assert_eq!(fill(&ui, item), srgb(0x102030));
    assert_eq!(ui.style(special).unwrap().paint.border_width, 3.0);
    assert_eq!(ui.style(plain).unwrap().paint.border_width, 0.0);
    // Named values give numbers too, inside a part of a style.
    let padding = ui.style(plain).unwrap().layout.padding;
    assert_eq!((padding.start, padding.top), (4.0, 2.0));
    // A state selects the element while it is in it.
    let pressed = ElementState {
        pressed: true,
        ..ElementState::default()
    };
    ui.set_state(plain, pressed);
    let (_, stats) = ui.frame();
    assert_eq!((stats.styled, stats.painted, stats.layouts), (1, 1, 0));
    assert_eq!(fill(&ui, plain), srgb(0x4c6ef5));
    ui.set_state(plain, ElementState::default());
    ui.frame();
    assert_eq!(fill(&ui, plain), srgb(0x202020));
}

#[test]
fn an_element_keeps_the_fields_that_its_own_style_sets_and_takes_the_rules_for_the_rest() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, |s| {
        s.background(LinearColor::rgba(1.0, 0.0, 0.0, 1.0))
            .width(Dimension::Pixels(120.0))
    });
    ui.set_style(
        &[StyleSheet::from_text("ui.style", SHEET)],
        &[Theme::from_text("dark.theme", DARK)],
    )
    .expect("a style sheet and a theme");
    ui.frame();
    let style = ui.style(ids[1]).unwrap();
    assert_eq!(
        style.paint.background,
        LinearColor::rgba(1.0, 0.0, 0.0, 1.0)
    );
    assert_eq!(style.layout.size[0], Dimension::Pixels(120.0));
    // What it leaves at its default, the rules give.
    assert_eq!(style.paint.radii, [8.0; 4]);
    assert_eq!(ui.rect(ids[1]).unwrap().width, 120.0);
}

#[test]
fn another_theme_styles_every_element_again_with_its_values() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, |s| s);
    let sheets = [StyleSheet::from_text("ui.style", SHEET)];
    ui.set_style(&sheets, &[Theme::from_text("dark.theme", DARK)])
        .expect("dark");
    ui.frame();
    let (_, stats) = ui.frame();
    assert_eq!((stats.styled, stats.layouts, stats.painted), (0, 0, 0));
    ui.set_style(&sheets, &[Theme::from_text("light.theme", LIGHT)])
        .expect("light");
    let (_, stats) = ui.frame();
    assert_eq!(stats.styled, stats.elements);
    // Its gaps change the padding: laid out again.
    assert_eq!(stats.layouts, 1);
    assert_eq!(fill(&ui, ids[0]), srgb(0xf0f0f0));
    assert_eq!(ui.style(ids[1]).unwrap().paint.radii, [4.0; 4]);
    // A class taken away: the element and what it holds are styled again.
    ui.build(root, 1, |children| {
        children.add("box", Element::container(), |_| {});
        children.add("panel", Element::container(), |_| {});
        children.add("toolbar", Element::container(), |children| {
            children.add("item", Element::container(), |_| {});
        });
        children.add("special", Element::container(), |_| {});
    });
    let (_, stats) = ui.frame();
    assert_eq!(stats.styled, 3);
    assert_eq!(fill(&ui, ids[1]), srgb(0xf0f0f0));
    assert_eq!(fill(&ui, ids[2]), srgb(0xf0f0f0));
}

#[test]
fn a_wrong_style_sheet_or_theme_is_refused_by_name_and_place_and_the_style_is_kept() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, |s| s);
    let theme = Theme::from_text("dark.theme", DARK);
    ui.set_style(
        &[StyleSheet::from_text("ui.style", SHEET)],
        std::slice::from_ref(&theme),
    )
    .expect("a style sheet and a theme");
    ui.frame();
    let cases = [
        (
            "a {\n    select kind=Container\n    paint background=$missing\n}\n",
            "ui.style: line 3,",
            "no theme gives a value named `missing`",
        ),
        (
            "a {\n    paint background=\"#12345\"\n}\n",
            "ui.style: line 2,",
            "is not a color",
        ),
        (
            "a {\n    paint {\n        radii $surface 0 0 0\n    }\n}\n",
            "ui.style: line 3,",
            "`$surface` is a color, where a length goes",
        ),
        (
            "a {\n    select kind=Window\n}\n",
            "ui.style: line 2,",
            "Window",
        ),
        (
            "a {\n    selected\n}\n",
            "ui.style: line 2,",
            "`selected` is none of what `a` holds",
        ),
        // A color is written whole, never as its channels
        // (docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md).
        (
            "a {\n    paint background=0.5\n}\n",
            "ui.style: line 2,",
            "a color goes here",
        ),
        (
            "a {\n    paint {\n        shadow color=1\n    }\n}\n",
            "ui.style: line 3,",
            "a color goes here",
        ),
        // A rule for a kind sets what that kind reads alone.
        (
            "a {\n    select kind=Label\n    control fill=\"#ffffff\"\n}\n",
            "ui.style: line 3,",
            "a rule for `Label` sets `control.fill`, which a `Label` does not read",
        ),
        (
            "a {\n    select kind=Button\n    paint backdrop=\"#000000\"\n}\n",
            "ui.style: line 3,",
            "sets `paint.backdrop`",
        ),
        ("a (\n", "ui.style: line 1,", ""),
        ("rule 3\n", "ui.style: line 1,", "not arguments"),
    ];
    for (sheet, place, problem) in cases {
        let error = ui
            .set_style(
                &[StyleSheet::from_text("ui.style", sheet)],
                std::slice::from_ref(&theme),
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with(place) && error.contains(problem),
            "{sheet}: {error}"
        );
    }
    let error = ui
        .set_style(
            &[StyleSheet::from_text("ui.style", SHEET)],
            &[Theme::from_text("broken.theme", "color { accent = }\n")],
        )
        .unwrap_err();
    assert!(
        error.to_string().starts_with("broken.theme: line 1,"),
        "{error}"
    );
    // The interface keeps its style.
    let (_, stats) = ui.frame();
    assert_eq!(stats.styled, 0);
    assert_eq!(fill(&ui, ids[1]), srgb(0x303030));
}

#[test]
fn rules_apply_in_the_order_of_their_file_whatever_their_names() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, |s| s);
    let sheet = "zeta {\n    select kind=Container\n    paint background=\"#111111\"\n}\n\
                 alpha {\n    select kind=Container\n    paint background=\"#222222\"\n}\n";
    ui.set_style(
        &[StyleSheet::from_text("order.style", sheet)],
        &[Theme::from_text("empty.theme", "")],
    )
    .expect("a style sheet");
    ui.frame();
    assert_eq!(fill(&ui, ids[0]), srgb(0x222222));
}

#[test]
fn an_element_whose_own_style_changes_is_styled_again() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, |s| {
        s.background(LinearColor::rgba(1.0, 0.0, 0.0, 1.0))
    });
    ui.set_style(
        &[StyleSheet::from_text("ui.style", SHEET)],
        &[Theme::from_text("dark.theme", DARK)],
    )
    .expect("a style sheet and a theme");
    ui.frame();
    assert_eq!(fill(&ui, ids[1]), LinearColor::rgba(1.0, 0.0, 0.0, 1.0));
    // Its fill back at its default: the rules give theirs.
    build(&mut ui, root, 1, |s| s);
    let (_, stats) = ui.frame();
    assert_eq!((stats.styled, stats.painted), (1, 1));
    assert_eq!(fill(&ui, ids[1]), srgb(0x303030));
}

#[test]
fn the_rows_of_a_list_restyled_lay_out_the_list_s_rows_alone() {
    let (mut ui, root) = ui();
    let sheet = "row {\n    select { classes row; }\n    layout { padding top=$pad; }\n}\n";
    let theme = |pad: u32| Theme::from_text("pad.theme", format!("length {{ pad {pad} }}\n"));
    let sheets = [StyleSheet::from_text("rows.style", sheet)];
    ui.set_style(&sheets, &[theme(2)]).expect("a style sheet");
    let mut list = None;
    let mut build = |ui: &mut Ui| {
        ui.build(root, 0, |children| {
            let rows_list = Element::list(1_000, 20.0).style(|s| s.grow(1.0));
            list = Some(children.add("list", rows_list, |rows| {
                for row in rows.rows() {
                    let element = Element::container().class("row");
                    rows.add(row, element, |_| {});
                }
            }));
        });
        ui.frame()
    };
    build(&mut ui);
    build(&mut ui);
    ui.set_style(&sheets, &[theme(6)]).expect("a style sheet");
    let (_, stats) = build(&mut ui);
    assert_eq!((stats.layouts, stats.lists), (0, 1));
    let row = ui.children(list.unwrap())[0];
    assert_eq!(ui.style(row).unwrap().layout.padding.top, 6.0);
}

#[test]
fn a_shadow_set_in_part_takes_the_defaults_of_what_it_leaves_out() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, |s| s);
    let sheet = "box {\n    select kind=Container\n    paint {\n        \
                 shadow blur=18 { offset 0 4; }\n    }\n}\n";
    ui.set_style(&[StyleSheet::from_text("shadows.style", sheet)], &[])
        .expect("a style sheet");
    ui.frame();
    let plain = ui.style(ids[0]).unwrap().paint.shadow.expect("a shadow");
    assert_eq!(
        (plain.offset, plain.blur, plain.spread),
        ([0.0, 4.0], 18.0, 0.0)
    );
    assert_eq!(plain.color, LinearColor::rgba(0.0, 0.0, 0.0, 0.0));
}

/// The fill of a box styled by the `$fill` of `themes`, stacked.
fn computed(themes: &[(&str, &str)]) -> Result<LinearColor, String> {
    let (mut ui, root) = ui();
    let sheet = "box {\n    select kind=Container\n    paint background=$fill\n}\n";
    let themes: Vec<Theme> = themes
        .iter()
        .map(|(name, text)| Theme::from_text(*name, *text))
        .collect();
    ui.set_style(&[StyleSheet::from_text("box.style", sheet)], &themes)
        .map_err(|error| error.to_string())?;
    let mut id = None;
    ui.build(root, 0, |children| {
        id = Some(children.add("box", Element::container(), |_| {}));
    });
    ui.frame();
    Ok(fill(&ui, id.expect("built")))
}

/// Whether two colors are within 1/1000 on each component.
fn near(a: LinearColor, b: LinearColor) -> bool {
    [(a.r, b.r), (a.g, b.g), (a.b, b.b), (a.a, b.a)]
        .iter()
        .all(|(x, y)| (x - y).abs() < 1e-3)
}

/// A theme of `colors`, values of the group `color`, one a line, and of the share `half`.
fn theme(colors: &str) -> String {
    format!("color {{\n{colors}}}\nnumber {{\n    half 0.5\n}}\n")
}

#[test]
fn a_theme_s_values_name_others_and_compute_colors_in_oklch() {
    let base = "accent \"#4c8dff\"\npanel \"#202126\"\n";
    // A name that names a name.
    let named = computed(&[(
        "a.theme",
        &theme(&format!("{base}fill $link\nlink $accent\n")),
    )]);
    assert_eq!(named, Ok(srgb(0x4c8dff)));
    // Lighter and darker by a share of OKLCH's lightness, the hue kept.
    let accent = srgb(0x4c8dff).to_oklch();
    for (call, by) in [
        ("lighten($accent, 0.1)", 0.1),
        ("darken($accent, 10%)", -0.1),
    ] {
        let color = computed(&[("a.theme", &theme(&format!("{base}fill \"{call}\"\n")))])
            .expect("computed")
            .to_oklch();
        assert!(
            (color.l - (accent.l + by)).abs() < 2e-3,
            "{call}: {color:?}"
        );
        assert!((color.h - accent.h).abs() < 1.0, "{call}: {color:?}");
    }
    // A mix halfway, its lightness halfway in OKLCH; an opacity; nested calls; a share by name.
    let panel = srgb(0x202126).to_oklch();
    let mixed = computed(&[(
        "a.theme",
        &theme(&format!("{base}fill \"mix($accent, $panel, $half)\"\n")),
    )])
    .expect("computed")
    .to_oklch();
    assert!(
        (mixed.l - (accent.l + panel.l) / 2.0).abs() < 2e-3,
        "{mixed:?}"
    );
    let soft = computed(&[(
        "a.theme",
        &theme(&format!(
            "{base}fill \"alpha(lighten($accent, 0.05), 20%)\"\n"
        )),
    )])
    .expect("computed");
    assert!((soft.a - 0.2).abs() < 1e-6, "{soft:?}");
    // A mix with a grey keeps the color's hue rather than turning through the grey's.
    let warm = computed(&[("a.theme", &theme("fill \"mix(#e8a33d, #808080, 30%)\"\n"))])
        .expect("computed")
        .to_oklch();
    assert!(
        (warm.h - srgb(0xe8a33d).to_oklch().h).abs() < 1.0,
        "{warm:?}"
    );
}

#[test]
fn stacked_themes_give_the_last_value_and_what_names_it_follows() {
    let base = theme("accent \"#4c8dff\"\nfill \"lighten($accent, 0.1)\"\n");
    let first = computed(&[("base.theme", &base)]).expect("computed");
    let green = computed(&[
        ("base.theme", &base),
        ("user.theme", "color { accent \"#2bb673\"; }\n"),
    ])
    .expect("computed");
    let expected =
        computed(&[("one.theme", &theme("fill \"lighten(#2bb673, 0.1)\"\n"))]).expect("computed");
    assert!(near(green, expected), "{green:?} {expected:?}");
    assert!(!near(green, first));
    // A later theme that gives the value itself wins over the earlier computation.
    let given = computed(&[
        ("base.theme", &base),
        ("user.theme", "color { fill \"#ff0000\"; }\n"),
    ]);
    assert_eq!(given, Ok(srgb(0xff0000)));
}

#[test]
fn a_theme_that_names_itself_a_missing_value_or_calls_wrongly_is_refused_by_its_value() {
    let cases = [
        ("a $b\nb $a\nfill \"#000000\"\n", "names it back"),
        (
            "other \"lighten($nowhere, 0.1)\"\nfill \"#000000\"\n",
            "no theme gives a value named `nowhere`",
        ),
        (
            "fill \"mix(#000000, #ffffff)\"\n",
            "`mix` takes two colors and a share",
        ),
        ("fill \"glow(#000000, 1)\"\n", "no color function `glow`"),
        ("fill \"lighten(#000000, much)\"\n", "`much` is no share"),
    ];
    for (colors, expected) in cases {
        let error = computed(&[("bad.theme", &theme(colors))]).expect_err(colors);
        assert!(error.contains("bad.theme"), "{error}");
        assert!(error.contains(expected), "{colors}: {error}");
    }
}
