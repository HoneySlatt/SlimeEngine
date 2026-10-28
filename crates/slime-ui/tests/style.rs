//! Style sheets and themes: rules that select elements by kind, classes, key, states and ancestor,
//! applied in their order, the element's own style over them, their values named by a theme or
//! given in sRGB; the elements styled again only where something changed; errors that name the
//! document and keep the style that the interface had.

use slime_color::LinearColor;
use slime_ui::style::{Dimension, Style};
use slime_ui::text::Fonts;
use slime_ui::theme::{StyleSheet, Theme};
use slime_ui::tree::{Element, ElementId, ElementState, LayoutDirection, RootSettings, Ui};

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
    let root = ui.add_root("main", settings, Style::default());
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
[box]
select = { kind = "Container" }
style.paint.background = "$surface"
style.layout.padding = { start = 4, end = 4, top = "$gap", bottom = "$gap" }

[panel]
select = { classes = ["panel"] }
style.paint = { background = "$panel", radii = ["$radius", "$radius", "$radius", "$radius"] }

[toolbar-item]
select = { inside = { class = "toolbar" } }
style.paint.background = "#102030"

[named]
select = { key = "special" }
style.paint.border_width = 3

[pressed]
select = { states = ["Pressed"] }
style.paint.background = "$accent"
"##;

const DARK: &str = r##"
surface = "#202020"
panel = "#303030"
accent = "#4c6ef5"
radius = 8
gap = 2
"##;

const LIGHT: &str = r##"
surface = "#f0f0f0"
panel = "#e0e0e0"
accent = "#1c3ed5"
radius = 4
gap = 6
"##;

/// Builds a box, a panel, a toolbar holding an item, and a box keyed `special`.
fn build(ui: &mut Ui, root: ElementId, revision: u64, panel: Style) -> Vec<ElementId> {
    let mut ids = Vec::new();
    ui.build(root, revision, |children| {
        ids.push(children.add("box", Element::container(Style::default()), |_| {}));
        let element = Element::container(panel).class("panel");
        ids.push(children.add("panel", element, |_| {}));
        let toolbar = Element::container(Style::default()).class("toolbar");
        children.add("toolbar", toolbar, |children| {
            ids.push(children.add("item", Element::container(Style::default()), |_| {}));
        });
        ids.push(children.add("special", Element::container(Style::default()), |_| {}));
    });
    ids
}

#[test]
fn rules_select_by_kind_class_key_state_and_ancestor_in_the_order_of_their_file() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, Style::default());
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
    let mut panel = Style::default();
    panel.paint.background = LinearColor::rgba(1.0, 0.0, 0.0, 1.0);
    panel.layout.size[0] = Dimension::Pixels(120.0);
    let ids = build(&mut ui, root, 0, panel);
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
    let ids = build(&mut ui, root, 0, Style::default());
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
        children.add("box", Element::container(Style::default()), |_| {});
        children.add("panel", Element::container(Style::default()), |_| {});
        children.add(
            "toolbar",
            Element::container(Style::default()),
            |children| {
                children.add("item", Element::container(Style::default()), |_| {});
            },
        );
        children.add("special", Element::container(Style::default()), |_| {});
    });
    let (_, stats) = ui.frame();
    assert_eq!(stats.styled, 3);
    assert_eq!(fill(&ui, ids[1]), srgb(0xf0f0f0));
    assert_eq!(fill(&ui, ids[2]), srgb(0xf0f0f0));
}

#[test]
fn style_sheets_and_themes_are_read_from_their_binary_encoding_as_from_their_text() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, Style::default());
    let sheet = slime_serialize::encode(SHEET).expect("a document");
    let theme = slime_serialize::encode(DARK).expect("a document");
    ui.set_style(
        &[StyleSheet::from_binary("ui.style", sheet)],
        &[Theme::from_binary("dark.theme", theme)],
    )
    .expect("a style sheet and a theme");
    ui.frame();
    assert_eq!(fill(&ui, ids[1]), srgb(0x303030));
}

#[test]
fn a_wrong_style_sheet_or_theme_is_refused_by_name_and_place_and_the_style_is_kept() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, Style::default());
    let theme = Theme::from_text("dark.theme", DARK);
    ui.set_style(
        &[StyleSheet::from_text("ui.style", SHEET)],
        std::slice::from_ref(&theme),
    )
    .expect("a style sheet and a theme");
    ui.frame();
    let cases = [
        (
            "[a]\nselect = { kind = \"Container\" }\nstyle.paint.background = \"$missing\"\n",
            "ui.style: line 3,",
            "no theme gives a value named `missing`",
        ),
        (
            "[a]\nstyle.paint.background = \"#12345\"\n",
            "ui.style: line 2,",
            "is not a color",
        ),
        (
            "[a]\nstyle.paint.radii = [\"$surface\", 0, 0, 0]\n",
            "ui.style: line 2,",
            "the value `surface` of the theme `dark.theme`",
        ),
        (
            "[a]\nselect = { kind = \"Window\" }\n",
            "ui.style: line 2,",
            "Window",
        ),
        (
            "[a]\nselected = {}\n",
            "ui.style: line 2,",
            "not `selected`",
        ),
        ("[a\n", "ui.style: line 1,", ""),
        ("rule = 3\n", "ui.style: line 1,", "is not a rule"),
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
            &[Theme::from_text("broken.theme", "accent = \n")],
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
    let ids = build(&mut ui, root, 0, Style::default());
    let sheet = "[zeta]\nselect = { kind = \"Container\" }\nstyle.paint.background = \"#111111\"\n\n\
                 [alpha]\nselect = { kind = \"Container\" }\nstyle.paint.background = \"#222222\"\n";
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
    let mut red = Style::default();
    red.paint.background = LinearColor::rgba(1.0, 0.0, 0.0, 1.0);
    let ids = build(&mut ui, root, 0, red);
    ui.set_style(
        &[StyleSheet::from_text("ui.style", SHEET)],
        &[Theme::from_text("dark.theme", DARK)],
    )
    .expect("a style sheet and a theme");
    ui.frame();
    assert_eq!(fill(&ui, ids[1]), LinearColor::rgba(1.0, 0.0, 0.0, 1.0));
    // Its fill back at its default: the rules give theirs.
    build(&mut ui, root, 1, Style::default());
    let (_, stats) = ui.frame();
    assert_eq!((stats.styled, stats.painted), (1, 1));
    assert_eq!(fill(&ui, ids[1]), srgb(0x303030));
}

#[test]
fn the_rows_of_a_list_restyled_lay_out_the_list_s_rows_alone() {
    let (mut ui, root) = ui();
    let sheet = "[row]\nselect = { classes = [\"row\"] }\nstyle.layout.padding.top = \"$pad\"\n";
    let theme = |pad: u32| Theme::from_text("pad.theme", format!("pad = {pad}\n"));
    let sheets = [StyleSheet::from_text("rows.style", sheet)];
    ui.set_style(&sheets, &[theme(2)]).expect("a style sheet");
    let mut list = None;
    let mut build = |ui: &mut Ui| {
        ui.build(root, 0, |children| {
            let mut style = Style::default();
            style.layout.grow = 1.0;
            list = Some(
                children.add("list", Element::list(1_000, 20.0, style), |rows| {
                    for row in rows.rows() {
                        let element = Element::container(Style::default()).class("row");
                        rows.add(row, element, |_| {});
                    }
                }),
            );
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
fn a_shadow_set_in_part_takes_the_defaults_of_what_it_leaves_out_and_a_name_gives_a_whole_one() {
    let (mut ui, root) = ui();
    let ids = build(&mut ui, root, 0, Style::default());
    let sheet = "[box]\nselect = { kind = \"Container\" }\n\
                 style.paint.shadow = [{ offset = [0, 4], blur = 18 }]\n\n\
                 [panel]\nselect = { classes = [\"panel\"] }\nstyle.paint.shadow = \"$card\"\n";
    let theme = "card = { color = \"#00000080\", offset = [0, 6], blur = 24, spread = 2 }\n";
    ui.set_style(
        &[StyleSheet::from_text("shadows.style", sheet)],
        &[Theme::from_text("shadows.theme", theme)],
    )
    .expect("a style sheet and a theme");
    ui.frame();
    let plain = ui.style(ids[0]).unwrap().paint.shadow.expect("a shadow");
    assert_eq!(
        (plain.offset, plain.blur, plain.spread),
        ([0.0, 4.0], 18.0, 0.0)
    );
    assert_eq!(plain.color, LinearColor::rgba(0.0, 0.0, 0.0, 0.0));
    let card = ui.style(ids[1]).unwrap().paint.shadow.expect("a shadow");
    assert_eq!(
        (card.offset, card.blur, card.spread),
        ([0.0, 6.0], 24.0, 2.0)
    );
}

/// The fill of a box styled by the `$fill` of `themes`, stacked.
fn computed(themes: &[(&str, &str)]) -> Result<LinearColor, String> {
    let (mut ui, root) = ui();
    let sheet = "[box]\nselect = { kind = \"Container\" }\nstyle.paint.background = \"$fill\"\n";
    let themes: Vec<Theme> = themes
        .iter()
        .map(|(name, text)| Theme::from_text(*name, *text))
        .collect();
    ui.set_style(&[StyleSheet::from_text("box.style", sheet)], &themes)
        .map_err(|error| error.to_string())?;
    let mut id = None;
    ui.build(root, 0, |children| {
        id = Some(children.add("box", Element::container(Style::default()), |_| {}));
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

#[test]
fn a_theme_s_values_name_others_and_compute_colors_in_oklch() {
    let base = "accent = \"#4c8dff\"\npanel = \"#202126\"\nhalf = \"50%\"\n";
    // A name that names a name.
    let named = computed(&[(
        "a.theme",
        &format!("{base}fill = \"$link\"\nlink = \"$accent\"\n"),
    )]);
    assert_eq!(named, Ok(srgb(0x4c8dff)));
    // Lighter and darker by a share of OKLCH's lightness, the hue kept.
    let accent = srgb(0x4c8dff).to_oklch();
    for (call, by) in [
        ("lighten($accent, 0.1)", 0.1),
        ("darken($accent, 10%)", -0.1),
    ] {
        let color = computed(&[("a.theme", &format!("{base}fill = \"{call}\"\n"))])
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
        &format!("{base}fill = \"mix($accent, $panel, $half)\"\n"),
    )])
    .expect("computed")
    .to_oklch();
    assert!(
        (mixed.l - (accent.l + panel.l) / 2.0).abs() < 2e-3,
        "{mixed:?}"
    );
    let soft = computed(&[(
        "a.theme",
        &format!("{base}fill = \"alpha(lighten($accent, 0.05), 20%)\"\n"),
    )])
    .expect("computed");
    assert!((soft.a - 0.2).abs() < 1e-6, "{soft:?}");
    // A mix with a grey keeps the color's hue rather than turning through the grey's.
    let warm = computed(&[("a.theme", "fill = \"mix(#e8a33d, #808080, 30%)\"\n")])
        .expect("computed")
        .to_oklch();
    assert!(
        (warm.h - srgb(0xe8a33d).to_oklch().h).abs() < 1.0,
        "{warm:?}"
    );
}

#[test]
fn stacked_themes_give_the_last_value_and_what_names_it_follows() {
    let base = "accent = \"#4c8dff\"\nfill = \"lighten($accent, 0.1)\"\n";
    let first = computed(&[("base.theme", base)]).expect("computed");
    let green = computed(&[
        ("base.theme", base),
        ("user.theme", "accent = \"#2bb673\"\n"),
    ])
    .expect("computed");
    let expected =
        computed(&[("one.theme", "fill = \"lighten(#2bb673, 0.1)\"\n")]).expect("computed");
    assert!(near(green, expected), "{green:?} {expected:?}");
    assert!(!near(green, first));
    // A later theme that gives the value itself wins over the earlier computation.
    let given = computed(&[("base.theme", base), ("user.theme", "fill = \"#ff0000\"\n")]);
    assert_eq!(given, Ok(srgb(0xff0000)));
}

#[test]
fn a_theme_that_names_itself_a_missing_value_or_calls_wrongly_is_refused_by_its_value() {
    let cases = [
        (
            "a = \"$b\"\nb = \"$a\"\nfill = \"#000000\"\n",
            "names itself",
        ),
        (
            "other = \"lighten($nowhere, 0.1)\"\nfill = \"#000000\"\n",
            "no theme gives a value named `nowhere`",
        ),
        (
            "fill = \"mix(#000000, #ffffff)\"\n",
            "`mix` takes two colors and a share",
        ),
        ("fill = \"glow(#000000, 1)\"\n", "no color function `glow`"),
        ("fill = \"lighten(#000000, much)\"\n", "`much` is no share"),
    ];
    for (theme, expected) in cases {
        let error = computed(&[("bad.theme", theme)]).expect_err(theme);
        assert!(error.contains("bad.theme"), "{error}");
        assert!(error.contains(expected), "{theme}: {error}");
    }
}
