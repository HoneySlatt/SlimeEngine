//! The core's roles and each kind's defaults
//! (`docs/adr/0230-the-cores-roles-and-each-kinds-defaults.md`): no color hides in a component's
//! defaults, each taken from its look through a role.
//!
//! The drawing test shows every kind in each configuration of the test of what each kind reads
//! (`reads_tests.rs`), its own style holding no color, under a theme that gives each role a color of
//! its own and no style sheet: every color drawn must be a role's, transparent, or a literal that
//! the test names, the test's own background and the colour picker's content and markers. With the
//! test of what each kind reads, which shows that each leaf that the table lists changes the
//! drawing, the test of the bases shows that each such color is its role's.

use slime_color::LinearColor;
use slime_reflect::{Kind as TypeKind, Reflect, Registry, Value};

use super::reads_tests::{Configuration, DIRECTIONS, base_style, configurations};
use super::{ElementId, RootSettings, Ui};
use crate::display::Primitive;
use crate::look;
use crate::style::Style;
use crate::text::Fonts;
use crate::theme::{
    self, Color, KindName, SELECTION_OPACITY, StyleSheet, Theme, Token, TokenValues,
};

const TRANSPARENT: LinearColor = LinearColor::rgba(0.0, 0.0, 0.0, 0.0);

/// Each role, the name that a theme gives it under, written out, and a color of its own in sRGB,
/// which nothing else draws.
const SENTINELS: [(Token<Color>, &str, u32); 7] = [
    (theme::CANVAS_TEXT, "canvas_text", 0xE0_10_10),
    (theme::GRAY_TEXT, "gray_text", 0x10_E0_10),
    (theme::BUTTON_FACE, "button_face", 0x10_10_E0),
    (theme::ACCENT, "accent", 0xE0_E0_10),
    (theme::HIGHLIGHT, "highlight", 0x10_E0_E0),
    (theme::HIGHLIGHT_TEXT, "highlight_text", 0xE0_10_E0),
    (theme::FOCUS, "focus", 0xF0_80_00),
];

/// The color that `hex` gives in sRGB, opaque.
fn srgb(hex: u32) -> LinearColor {
    LinearColor::from_srgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The color that [`SENTINELS`] gives `role`.
fn sentinel(role: Token<Color>) -> LinearColor {
    let (_, _, hex) = SENTINELS
        .iter()
        .find(|(given, _, _)| given.name() == role.name())
        .expect("every role has a sentinel");
    srgb(*hex)
}

/// A theme that gives each role its sentinel, under its name as written in [`SENTINELS`].
fn sentinels() -> Theme {
    let colors: String = SENTINELS
        .iter()
        .map(|(_, name, hex)| format!("    {name} \"#{hex:06X}\"\n"))
        .collect();
    Theme::from_text("sentinels.theme", format!("color {{\n{colors}}}\n"))
}

/// An interface styled by the sentinels alone, each role checked to take its sentinel, under its
/// own name.
fn styled_by_sentinels(fonts: Fonts) -> Ui {
    let mut ui = Ui::new(fonts);
    ui.set_style(&[] as &[StyleSheet], &[sentinels()])
        .expect("the sentinels are colors");
    for (role, name, _) in SENTINELS {
        assert_eq!(role.name(), name, "the role's name in themes");
        assert_eq!(
            ui.tokens.color(&role),
            sentinel(role),
            "the role read under `{name}`"
        );
    }
    assert!(ui.warned.is_empty(), "the sentinels give every role");
    ui
}

/// Every color leaf of `value`, a value of the type `ty`, by its path, found through reflection so
/// that a color added to a style is found too.
fn colors(
    registry: &Registry,
    ty: &str,
    value: &Value,
    path: &str,
    out: &mut Vec<(String, Value)>,
) {
    if ty == "LinearColor" {
        out.push((path.to_owned(), value.clone()));
        return;
    }
    let info = registry
        .get(ty)
        .unwrap_or_else(|| panic!("`{ty}` is registered"))
        .info();
    match (&info.kind, value) {
        (TypeKind::Struct(fields), Value::Struct(values)) => {
            for field in fields {
                let (_, held) = values
                    .iter()
                    .find(|(name, _)| *name == field.name)
                    .expect("a whole value");
                let path = if path.is_empty() {
                    field.name.clone()
                } else {
                    format!("{path}.{}", field.name)
                };
                colors(registry, &field.ty, held, &path, out);
            }
        }
        (TypeKind::Option(inner), Value::Option(Some(held))) => {
            colors(registry, inner, held, path, out);
        }
        _ => {}
    }
}

/// Every color of `style`, by its path.
fn style_colors(style: &Style) -> Vec<(String, LinearColor)> {
    let registry = theme::registry();
    let mut found = Vec::new();
    colors(&registry, "UiStyle", &style.to_value(), "", &mut found);
    found
        .into_iter()
        .map(|(path, value)| {
            let color = LinearColor::from_value(&value).expect("a color");
            (path, color)
        })
        .collect()
}

/// The value that `role` gives the leaf `leaf` of a kind's base.
fn as_drawn(leaf: &str, color: LinearColor) -> LinearColor {
    if leaf == "text.selection" {
        LinearColor {
            a: color.a * SELECTION_OPACITY,
            ..color
        }
    } else {
        color
    }
}

#[test]
fn the_default_style_holds_no_color_but_transparent() {
    let found = style_colors(&Style::default());
    assert!(found.len() >= 7, "the style's colors are found: {found:?}");
    for (path, color) in found {
        assert_eq!(color, TRANSPARENT, "`{path}` of the default style");
    }
}

#[test]
fn each_kind_s_base_takes_each_color_that_it_reads_from_its_role_and_no_other() {
    let roles = styled_by_sentinels(Fonts::new()).tokens;
    for kind in KindName::ALL {
        let base = theme::base(kind, &roles);
        let table = theme::read_by(kind);
        for (path, color) in style_colors(&base) {
            // The table names a text's color `text`.
            let leaf = if path == "text.color" { "text" } else { &path };
            match table.iter().find(|(read, _)| *read == leaf) {
                Some((_, Some(role))) => assert_eq!(
                    color,
                    as_drawn(leaf, roles.color(role)),
                    "{kind:?}'s `{path}`, which it reads, is its role's, `{}`",
                    role.name()
                ),
                Some((_, None)) => panic!("{kind:?} reads the color `{path}`, given no role"),
                None => assert_eq!(
                    color, TRANSPARENT,
                    "{kind:?}'s `{path}`, which it does not read"
                ),
            }
        }
        for (leaf, role) in table {
            let color = [
                "text",
                "text.selection",
                "control.track",
                "control.fill",
                "control.mark",
            ]
            .contains(leaf);
            assert_eq!(
                role.is_some(),
                color,
                "{kind:?}: a role for `{leaf}` exactly when it is a color"
            );
        }
    }
}

/// Whether `color`, which a configuration of `kind` drew, is content or a marker that the test
/// names: the test's own background behind its subjects, and, around a color well and its picker,
/// the color edited, the checkerboard behind its alpha and the rings of black and white seen on
/// any color.
fn named_literal(kind: KindName, color: LinearColor, well: Option<LinearColor>) -> bool {
    if color == srgb(0x20_24_28) {
        return true;
    }
    let picker = matches!(kind, KindName::ColorWell | KindName::ColorPart);
    let edited = well.is_some_and(|well| [well.r, well.g, well.b] == [color.r, color.g, color.b]);
    picker
        && (edited
            || super::color::CHECKS.contains(&color)
            || [LinearColor::BLACK, LinearColor::WHITE].contains(&color))
}

/// The colors that `configuration` of `kind` draws in `direction`, under the sentinels' theme and
/// no style sheet, its subject's own style holding no color: each with what drew it, and the color
/// of the well that the configuration edits, if any.
fn drawn(
    kind: KindName,
    configuration: &Configuration,
    direction: super::LayoutDirection,
) -> (Vec<(LinearColor, String)>, Option<LinearColor>) {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = styled_by_sentinels(fonts);
    let root = ui.add_root(
        "main",
        RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction,
            origin: [0.0; 2],
            modal: false,
        },
        |s| s,
    );
    let mut leaves = base_style();
    let default = Style::default();
    leaves.text.color = default.text.color;
    leaves.text.selection = default.text.selection;
    leaves.control.track = default.control.track;
    leaves.control.fill = default.control.fill;
    leaves.control.mark = default.control.mark;
    leaves.paint.backdrop = default.paint.backdrop;
    if !configuration.icons {
        leaves.icons.font.clear();
    }
    (configuration.show)(&mut ui, root, &leaves);
    ui.frame();
    let (list, _) = ui.frame();
    // The color that a well of the tree, or of a layer, edits: what its picker draws as content.
    let mut stack: Vec<ElementId> = std::iter::once(root)
        .chain(ui.layers().iter().copied())
        .collect();
    let mut well = None;
    while let Some(id) = stack.pop() {
        well = well.or_else(|| ui.color(id));
        stack.extend(ui.children(id).iter().copied());
    }
    let mut colors = Vec::new();
    for item in list.items() {
        let what = format!("{:?}", item.primitive);
        let what: String = what.chars().take(120).collect();
        match &item.primitive {
            Primitive::Box(primitive) => {
                colors.push((primitive.fill, what.clone()));
                if primitive.border_width > 0.0 {
                    colors.push((primitive.border, what));
                }
            }
            Primitive::Shadow(primitive) => colors.push((primitive.color, what)),
            Primitive::Text(primitive) => colors.push((primitive.color, what)),
            // A picker's gradients are the color edited, and an image or a surface the program's:
            // content, drawn untinted.
            Primitive::Gradient(_) => assert!(
                matches!(kind, KindName::ColorWell | KindName::ColorPart),
                "{kind:?}, {}: a gradient outside a picker",
                configuration.name
            ),
            Primitive::Image(primitive) => assert!(
                matches!(kind, KindName::Image | KindName::Surface)
                    && primitive.color == LinearColor::WHITE,
                "{kind:?}, {}: an image tinted {:?}",
                configuration.name,
                primitive.color
            ),
        }
    }
    (colors, well)
}

#[test]
fn every_kind_draws_only_its_roles_colors_under_a_theme_that_gives_each_one_its_own() {
    // Each role's sentinel, and the highlight's as a field's selection draws it.
    let mut sentinels: Vec<LinearColor> = SENTINELS
        .iter()
        .map(|(role, _, _)| sentinel(*role))
        .collect();
    sentinels.push(as_drawn("text.selection", sentinel(theme::HIGHLIGHT)));
    for kind in KindName::ALL {
        let mut seen = Vec::new();
        for configuration in configurations(kind) {
            for direction in DIRECTIONS {
                let (colors, well) = drawn(kind, &configuration, direction);
                for (color, what) in colors {
                    assert!(
                        color.a == 0.0
                            || sentinels.contains(&color)
                            || named_literal(kind, color, well),
                        "{kind:?}, {}, {direction:?}: {color:?} is no role's, drawn by {what}",
                        configuration.name
                    );
                    seen.push(color);
                }
            }
        }
        for (leaf, role) in theme::read_by(kind) {
            if let Some(role) = role {
                let color = as_drawn(leaf, sentinel(*role));
                assert!(
                    seen.contains(&color),
                    "{kind:?} reads `{leaf}`, whose role `{}` it never draws",
                    role.name()
                );
            }
        }
    }
}

#[test]
fn an_interface_given_no_theme_takes_the_fallbacks_and_is_warned_of_nothing() {
    let mut ui = Ui::new(Fonts::new());
    ui.set_style(&[], &[]).expect("nothing to refuse");
    let roles = ui.tokens.clone();
    assert_eq!(
        roles,
        TokenValues::fallbacks(&theme::ROLES.each().collect::<Vec<_>>())
    );
    // The colors that the components drew before they took them from roles.
    let grey = LinearColor::rgba(0.3, 0.3, 0.3, 1.0);
    let blue = LinearColor::rgba(0.2, 0.4, 0.9, 1.0);
    let want = [
        (theme::CANVAS_TEXT, LinearColor::WHITE),
        (theme::GRAY_TEXT, grey),
        (theme::BUTTON_FACE, grey),
        (theme::ACCENT, blue),
        (theme::HIGHLIGHT, blue),
        (theme::HIGHLIGHT_TEXT, LinearColor::WHITE),
        (theme::FOCUS, blue),
    ];
    for (role, color) in want {
        assert_eq!(roles.color(&role), color, "`{}`'s fallback", role.name());
    }
    let check_box = theme::base(KindName::CheckBox, &roles);
    assert_eq!(check_box.control.fill, blue);
    assert_eq!(check_box.control.mark, LinearColor::WHITE);
    let field = theme::base(KindName::TextField, &roles);
    assert_eq!(field.text.selection, LinearColor::rgba(0.2, 0.4, 0.9, 0.4));
    assert!(ui.warned.is_empty(), "no theme given, no warning");
}

#[test]
fn a_role_that_the_themes_lack_takes_its_fallback_and_is_warned_of_once() {
    let mut ui = Ui::new(Fonts::new());
    let accent = Theme::from_text("accent.theme", "color {\n    accent \"#2BB673\"\n}\n");
    ui.set_style(&[], std::slice::from_ref(&accent))
        .expect("a theme of one color");
    let green = LinearColor::from_srgb8(0x2B, 0xB6, 0x73);
    assert_eq!(ui.tokens.color(&theme::ACCENT), green);
    // A role's fallback that names another role follows the value that the theme gives it.
    assert_eq!(ui.tokens.color(&theme::HIGHLIGHT), green);
    assert_eq!(ui.tokens.color(&theme::FOCUS), green);
    assert_eq!(ui.tokens.color(&theme::CANVAS_TEXT), LinearColor::WHITE);
    let lacking: Vec<&str> = SENTINELS
        .iter()
        .map(|(role, _, _)| role.name())
        .filter(|&role| role != "accent")
        .collect();
    assert_eq!(ui.warned, lacking, "each role lacking, told once");
    ui.set_style(&[], &[accent]).expect("the same theme");
    assert_eq!(ui.warned, lacking, "not told again");
}

#[test]
fn a_role_given_a_value_of_another_type_refuses_the_themes() {
    let mut ui = Ui::new(Fonts::new());
    ui.set_style(&[], &[sentinels()]).expect("the sentinels");
    let before = ui.tokens.clone();
    // A theme's color is written in sRGB, a name or a function: none can be a channel that is no
    // finite number, which the form by channels allowed (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
    for (text, why, said) in [
        (
            "number {\n    accent 12\n}\n",
            "a number",
            // The sentinels, below it, gave `accent` as a color first.
            "`accent` is a number here, but a color in `sentinels.theme`",
        ),
        (
            "number {\n    size 12\n}\ncolor {\n    accent $size\n}\n",
            "a name of a number",
            "`accent` names `size`, which is a number, not a color",
        ),
        (
            "color {\n    accent twelve\n}\n",
            "a text that is no color",
            "`accent`: `twelve` is no color",
        ),
    ] {
        let error = ui
            .set_style(&[], &[sentinels(), Theme::from_text("wrong.theme", text)])
            .expect_err(why);
        assert_eq!(
            error.errors()[0].document,
            "wrong.theme",
            "{why}: the theme at fault"
        );
        let error = error.to_string();
        assert!(error.contains(said), "{why}: the error says why: {error}");
        assert_eq!(ui.tokens, before, "{why}: the roles that it had kept");
    }
}

#[test]
fn kinds_of_other_bases_share_no_style() {
    // The same own style, no rule: a label takes its text's color from its role, a container,
    // which draws no text, keeps it transparent, whichever is styled first.
    for label_first in [true, false] {
        let mut ui = styled_by_sentinels(Fonts::new());
        let root = ui.add_root(
            "main",
            RootSettings {
                size: [400.0, 300.0],
                scale: 1.0,
                direction: super::LayoutDirection::LeftToRight,
                origin: [0.0; 2],
                modal: false,
            },
            |s| s,
        );
        let mut own = Style::default();
        own.layout.gap = [4.0, 4.0];
        let (mut label, mut container) = (None, None);
        ui.build(root, 0, |children| {
            let make_label = |children: &mut super::Children<'_>| {
                children.add(
                    "label",
                    super::Element::label("Text").style(|s| s.with_style(&own)),
                    |_| {},
                )
            };
            let make_container = |children: &mut super::Children<'_>| {
                children.add(
                    "box",
                    super::Element::container().style(|s| s.with_style(&own)),
                    |_| {},
                )
            };
            if label_first {
                label = Some(make_label(children));
                container = Some(make_container(children));
            } else {
                container = Some(make_container(children));
                label = Some(make_label(children));
            }
        });
        ui.frame();
        let text = |id: Option<ElementId>| ui.style(id.expect("built")).expect("styled").text.color;
        assert_eq!(
            text(label),
            sentinel(theme::CANVAS_TEXT),
            "label first: {label_first}"
        );
        assert_eq!(text(container), TRANSPARENT, "label first: {label_first}");
    }
}

#[test]
fn the_look_gives_every_role_in_every_theme() {
    for colors in [
        look::Colors::Dark,
        look::Colors::Light,
        look::Colors::HighContrast,
    ] {
        for density in [look::Density::Comfortable, look::Density::Compact] {
            let mut ui = Ui::new(Fonts::new());
            ui.set_style(&[look::style_sheet()], &look::themes(colors, density))
                .expect("the look's documents are valid");
            assert!(
                ui.warned.is_empty(),
                "{colors:?}, {density:?}: the look lacks {:?}",
                ui.warned
            );
        }
    }
}
