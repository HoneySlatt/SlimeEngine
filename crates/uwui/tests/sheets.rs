//! Style sheets in KDL: each field of a style as a property or a child node, a number in its
//! measure, a size's share and a track's fraction, a text named by a theme, and each refusal at its
//! line (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).

#[path = "../../slime-assets/tests/common/damage.rs"]
mod damage;

use std::time::{Duration, Instant};

use slime_color::LinearColor;
use uwui::DocumentErrors;
use uwui::style::{Align, Dimension, Easing, Shadow, Style, Track};
use uwui::text::Fonts;
use uwui::theme::{StyleSheet, Theme};
use uwui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

/// The theme that the sheets below name: a value of each group.
const THEME: &str = r##"
color {
    accent "#4C8DFF"
}
length {
    space 6
}
number {
    weight 600
    span 2
}
duration {
    fast (ms)120
}
text {
    tick "\u{E06C}"
}
"##;

/// An interface of one root of 200 × 200 logical pixels, styled by `sheet` over [`THEME`], holding a
/// container keyed `box` and a row of three containers keyed by number, the second of the class
/// `panel`; and the box.
fn styled(sheet: &str) -> Result<(Ui, ElementId, ElementId), DocumentErrors> {
    let mut ui = Ui::new(Fonts::new());
    ui.set_style(
        &[StyleSheet::from_text("a.style.kdl", sheet)],
        &[Theme::from_text("a.theme.kdl", THEME)],
    )?;
    let settings = RootSettings {
        size: [200.0, 200.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: uwui::host::TargetId::MAIN,
    };
    let root = ui.add_root("main", settings, |s| s);
    let (mut boxed, mut second) = (None, None);
    ui.build(root, 0, |children| {
        boxed = Some(children.add("box", Element::container(), |_| {}));
        children.add("row", Element::container(), |row| {
            for index in 0..3_usize {
                let element = if index == 1 {
                    Element::container().class("panel")
                } else {
                    Element::container()
                };
                let id = row.add(index, element, |_| {});
                if index == 1 {
                    second = Some(id);
                }
            }
        });
    });
    ui.frame();
    Ok((ui, boxed.expect("built"), second.expect("built")))
}

/// The style of the box under `sheet`.
fn box_style(sheet: &str) -> Style {
    let (ui, boxed, _) = styled(sheet).unwrap_or_else(|errors| panic!("{sheet}\n{errors}"));
    ui.style(boxed).expect("styled").clone()
}

/// What refusing `sheet` says.
fn refused(sheet: &str) -> String {
    match styled(sheet) {
        Ok(_) => panic!("taken: {sheet}"),
        Err(errors) => errors.to_string(),
    }
}

#[test]
fn every_kind_of_value_is_read_as_its_field_takes_it() {
    let style = box_style(
        r##"
box {
    select key=box
    layout direction=Column wrap=#true align_self=Center basis=(%)30 grow=2 {
        gap $space (px)3
        padding start=1 end=(px)2 top=$space bottom=-4
        size (%)50 Auto
        min_size $space 10
        columns (fr)1 120 Auto $space
        column start=2 span=$span
    }
    paint background=$accent border="#10203080" opacity=0.5 {
        radii 1 2 3 4
        shadow color=$accent blur=8 { offset 0 $space; }
    }
    text { style weight=$weight italic=#true locale=ja { fonts Geist "Geist Mono"; }; }
    transition duration=$fast easing=Linear
    icons tick=$tick down="$$down" font=lucide
}
"##,
    );
    let layout = &style.layout;
    assert_eq!(layout.align_self, Some(Align::Center));
    assert!(layout.wrap);
    assert_eq!(layout.grow, 2.0);
    // A share divided by 100 in `f64` and rounded once: the `f32` nearest 0.3, which
    // `30.0_f32 * 0.01` misses.
    assert_eq!(layout.basis, Dimension::Share(0.3));
    assert_ne!(30.0_f32 * 0.01, 0.3_f32);
    // And not in `f32`: `(%)0.7` is the `f32` nearest 0.007, which `0.7_f32 / 100.0` misses.
    let thin = box_style("box {\n    layout basis=(%)0.7\n}\n");
    assert_eq!(thin.layout.basis, Dimension::Share(0.007));
    assert_ne!(0.7_f32 / 100.0, 0.007_f32);
    assert_eq!(layout.gap, [6.0, 3.0]);
    assert_eq!(
        [
            layout.padding.start,
            layout.padding.end,
            layout.padding.top,
            layout.padding.bottom
        ],
        [1.0, 2.0, 6.0, -4.0]
    );
    assert_eq!(layout.size, [Dimension::Share(0.5), Dimension::Auto]);
    // A name where a size goes is a length.
    assert_eq!(
        layout.min_size,
        [Dimension::Pixels(6.0), Dimension::Pixels(10.0)]
    );
    assert_eq!(
        layout.columns,
        [
            Track::Fraction(1.0),
            Track::Pixels(120.0),
            Track::Auto,
            Track::Pixels(6.0)
        ]
    );
    assert_eq!((layout.column.start, layout.column.span), (Some(2), 2));
    let accent = LinearColor::from_srgb8(0x4C, 0x8D, 0xFF);
    assert_eq!(style.paint.background, accent);
    let mut border = LinearColor::from_srgb8(0x10, 0x20, 0x30);
    border.a = f32::from(0x80_u8) / 255.0;
    assert_eq!(style.paint.border, border);
    assert_eq!(style.paint.radii, [1.0, 2.0, 3.0, 4.0]);
    // A shadow is set whole: what it leaves out takes the default's.
    assert_eq!(
        style.paint.shadow,
        Some(Shadow {
            color: accent,
            offset: [0.0, 6.0],
            blur: 8.0,
            spread: 0.0,
        })
    );
    assert_eq!(style.text.style.weight, 600.0);
    assert!(style.text.style.italic);
    assert_eq!(style.text.style.locale.as_deref(), Some("ja"));
    assert_eq!(style.text.style.fonts, ["Geist", "Geist Mono"]);
    assert_eq!(style.transition.duration, 0.12);
    assert_eq!(style.transition.easing, Easing::Linear);
    // A text named by a theme, and one that starts with `$`, written `$$`.
    assert_eq!(style.icons.tick, "\u{E06C}");
    assert_eq!(style.icons.down, "$down");
    assert_eq!(style.icons.font, "lucide");
}

#[test]
fn null_empties_an_option_and_a_later_rule_sets_it_again() {
    let style = box_style(
        r##"
box {
    layout align_self=Center { column start=3; }
    paint { shadow blur=4; }
    text { style locale=ja; }
}
box-again {
    select key=box
    layout align_self=#null { column start=#null; }
    paint { shadow #null; }
    text { style locale=#null; }
}
"##,
    );
    assert_eq!(style.layout.align_self, None);
    assert_eq!(style.layout.column.start, None);
    assert_eq!(style.paint.shadow, None);
    assert_eq!(style.text.style.locale, None);
}

#[test]
fn a_selector_names_a_numbered_key_by_its_number() {
    let (ui, boxed, second) = styled(
        r##"
second {
    select key=1
    paint opacity=0.25
}
"##,
    )
    .expect("a sheet");
    assert_eq!(ui.style(second).expect("styled").paint.opacity, 0.25);
    assert_eq!(ui.style(boxed).expect("styled").paint.opacity, 1.0);
    // A class, a state and an ancestor, as the cascade's tests check them; here their notation.
    let (ui, _, second) = styled(
        r##"
panel {
    select { classes panel; inside kind=Container; }
    paint opacity=0.5
}
"##,
    )
    .expect("a sheet");
    assert_eq!(ui.style(second).expect("styled").paint.opacity, 0.5);
}

#[test]
fn each_wrong_value_is_refused_at_its_line() {
    let cases: &[(&str, &str)] = &[
        // The measures of numbers, and their units.
        ("layout { padding top=(%)50; }", "not `(%)`"),
        (
            "layout { size (fr)1 Auto; }",
            "a share of its parent's, `(%)50`, not `(fr)`",
        ),
        (
            "layout { columns (%)50; }",
            "a share of the space left, `(fr)1`, not `(%)`",
        ),
        (
            "layout { size (px)$space Auto; }",
            "a name takes the unit of its value's group",
        ),
        (
            "paint border_width=(px)$space",
            "a name takes the unit of its value's group",
        ),
        ("transition duration=0.2", "a duration writes its unit"),
        (
            "transition duration=(min)1",
            "a duration is in `(ms)` or `(s)`, not `(min)`",
        ),
        ("transition duration=(ms)-5", "a duration is never negative"),
        ("transition duration=(s)1e30", "a duration too long to hold"),
        (
            "transition duration=$space",
            "`$space` is a length, where a duration goes",
        ),
        (
            "paint border_width=$weight",
            "`$weight` is a number, where a length goes",
        ),
        (
            "paint opacity=$space",
            "`$space` is a length, where a number goes",
        ),
        ("paint opacity=(px)1", "a number takes no unit `(px)`"),
        ("paint border_width=1e39", "past what an `f32` holds"),
        ("paint border_width=#nan", "not finite"),
        ("paint border_width=\"4\"", "a length goes here, not \"4\""),
        (
            "layout wrap=\"#true\"",
            "`#true` or `#false` goes here, not \"#true\"",
        ),
        ("layout basis=(px)Auto", "not `(px)`\"Auto\""),
        (
            "layout { column span=2.5; }",
            "a whole number from 0 to 65535 goes here",
        ),
        (
            "layout { column span=(px)2; }",
            "a number takes no unit `(px)`",
        ),
        // Names.
        (
            "paint background=$nothing",
            "no theme gives a value named `nothing`",
        ),
        ("paint background=\"$\"", "`$` names no value"),
        (
            "icons tick=$accent",
            "`$accent` is a color, where a text goes",
        ),
        (
            "paint background=$tick",
            "`$tick` is a text, where a color goes",
        ),
        ("icons tick=(x)\"a\"", "a text takes no type `(x)`"),
        // Values of the wrong kind.
        ("paint background=#null", "a color goes here, not `#null`"),
        ("paint background=\"#12345\"", "is not a color"),
        (
            "paint background=(rgb)\"#123456\"",
            "a color takes no type `(rgb)`",
        ),
        ("layout wrap=1", "`#true` or `#false` goes here, not `1`"),
        (
            "layout direction=Diagonal",
            "`Diagonal` is not one of `Row`, `Column`",
        ),
        ("layout direction=(x)Row", "which takes no type"),
        ("layout { size 1 2 3; }", "`size` holds 2 values, not 3"),
        (
            "layout { gap 1 x=2; }",
            "`gap` holds its values as arguments, not properties",
        ),
        (
            "layout { gap 1 2 { a; }; }",
            "`gap` holds its values as arguments, not in a block",
        ),
        ("layout { gap (t)1 2; }", "not `(t)`"),
        ("text { style { fonts Geist size=2; }; }", "not properties"),
        ("layout { (t)gap 1 2; }", "`gap` takes no type"),
        ("icons tick=1", "a text goes here, not `1`"),
        ("layout direction=1", "goes here, not `1`"),
        ("select kind=#true", "goes here, not `#true`"),
        // Fields in the wrong form, unknown or given twice.
        (
            "layout padding=4",
            "`padding` is written as a child node, `padding …`, not a property",
        ),
        (
            "paint { background red; }",
            "`background` is written as a property, `background=…`",
        ),
        (
            "paint border_width=1 border_width=2",
            "`border_width` is given twice in `paint`",
        ),
        (
            "paint background=\"#ff0000\"\n    paint opacity=1",
            "`paint` is given twice in `a`",
        ),
        ("paint colour=red", "`colour` is none of what `paint` holds"),
        (
            "style { paint; }",
            "`style` is none of what `a` holds: `select`, `layout`",
        ),
        ("(t)paint opacity=1", "`paint` takes no type"),
        ("paint 1", "`paint` holds its fields as properties"),
        (
            "paint { shadow #null blur=1; }",
            "`shadow #null` empties it, and holds nothing else",
        ),
        // Selectors.
        ("select key=\"#3\"", "written as a number, `key=3`"),
        ("select key=-1", "a numbered key is a whole number from 0"),
        ("select key=1.5", "a numbered key is a whole number from 0"),
        ("select { classes $panel; }", "a name never starts with `$`"),
        // Without an escape: `$$` in a name is refused too.
        (
            "select { classes \"$$panel\"; }",
            "a name never starts with `$`",
        ),
        ("select key=$tabs", "a name never starts with `$`"),
        (
            "select { inside class=\"$$bar\"; }",
            "a name never starts with `$`",
        ),
        (
            "select { classes x=1; }",
            "`classes` holds its names as arguments, not properties",
        ),
        ("select { states Asleep; }", "`Asleep` is not one of"),
        ("select kind=Window", "`Window` is not one of"),
        (
            "select { inside kind=Label place=1; }",
            "`place` is none of what `inside` holds",
        ),
        ("select\n    select", "`select` is given twice in a rule"),
        ("select { classes 1; }", "a name goes here, not `1`"),
        ("select { classes (t)a; }", "a name takes no type"),
        (
            "select { inside class=#true; }",
            "a name goes here, not `#true`",
        ),
        (
            "select { (t)classes a; }",
            "`classes` holds its names as arguments alone",
        ),
        (
            "select { classes a { b; }; }",
            "`classes` holds its names as arguments alone",
        ),
        // A rule for a kind sets what that kind reads alone.
        (
            "select kind=Label\n    control fill=\"#ffffff\"",
            "a rule for `Label` sets `control.fill`",
        ),
    ];
    for (body, problem) in cases {
        // The rule's second line holds what is wrong.
        let sheet = format!("first {{\n}}\na {{\n    {body}\n}}\n");
        let line = 4 + body.matches('\n').count();
        let error = refused(&sheet);
        assert!(
            error.starts_with(&format!("a.style.kdl: line {line},")) && error.contains(problem),
            "{body}: {error}"
        );
    }
    // A rule written twice, a rule's arguments, and a text that is not KDL.
    for (sheet, place, problem) in [
        (
            "a {\n}\na {\n}\n",
            "line 3,",
            "the rule `a` is given twice, first at line 1",
        ),
        (
            "a 1 {\n}\n",
            "line 1,",
            "`a` holds its fields as properties",
        ),
        (
            "a select=Button {\n}\n",
            "line 1,",
            "`select` is written as a child node, `select …`, not a property",
        ),
        ("a {\n    paint (\n}\n", "line 2,", ""),
    ] {
        let error = refused(sheet);
        assert!(
            error.starts_with(&format!("a.style.kdl: {place}")) && error.contains(problem),
            "{sheet}: {error}"
        );
    }
}

#[test]
fn the_errors_of_themes_then_sheets_are_gathered_in_their_order() {
    let mut ui = Ui::new(Fonts::new());
    let errors = ui
        .set_style(
            &[
                StyleSheet::from_text(
                    "a.style.kdl",
                    "a {\n    paint opacity=$space\n    layout wrap=1\n}\nb {\n    select kind=X\n}\n",
                ),
                StyleSheet::from_text("b.style.kdl", "c {\n    paint border_width=\"1\"\n}\n"),
            ],
            &[Theme::from_text("a.theme.kdl", THEME)],
        )
        .expect_err("wrong sheets");
    let places: Vec<String> = errors
        .errors()
        .iter()
        .map(|error| {
            let text = error.to_string();
            text[..text.find(", column").expect("a place")].to_owned()
        })
        .collect();
    assert_eq!(
        places,
        [
            "a.style.kdl: line 2",
            "a.style.kdl: line 3",
            "a.style.kdl: line 6",
            "b.style.kdl: line 2"
        ]
    );
    // A theme's errors come first, the sheets read only once the themes are taken.
    let errors = ui
        .set_style(
            &[StyleSheet::from_text(
                "a.style.kdl",
                "a {\n    layout wrap=1\n}\n",
            )],
            &[Theme::from_text(
                "b.theme.kdl",
                "length {\n    gap #null\n}\n",
            )],
        )
        .expect_err("a wrong theme");
    assert_eq!(errors.errors().len(), 1, "{errors}");
    assert!(
        errors.to_string().starts_with("b.theme.kdl: line 2"),
        "{errors}"
    );
}

#[test]
fn a_sheet_of_many_wrong_values_is_refused_in_time_linear_in_its_size() {
    // Each error past the cap is counted, not placed at its line, which reads the text up to it.
    let mut many = String::new();
    for at in 0..20_000 {
        many.push_str(&format!("r{at:05} {{\n    paint opacity=$nothing\n}}\n"));
    }
    let start = Instant::now();
    let error = refused(&many);
    assert!(error.ends_with("and more, past the first 16"), "{error}");
    // A debug build reads it in about a second; a time quadratic in the rules, in minutes.
    assert!(
        start.elapsed() < Duration::from_secs(20),
        "{:?} for 20 000 rules",
        start.elapsed()
    );
}

#[test]
fn a_damaged_sheet_is_refused_or_read_never_panicking() {
    let mut ui = Ui::new(Fonts::new());
    let theme = [Theme::from_text("a.theme.kdl", THEME)];
    // The example's sheet, and a sheet that writes each form: the look's, some 19 kB, takes too
    // long for the damage's every byte.
    for sheet in [
        include_str!("../../../examples/assets/ui/interface.style.kdl"),
        r##"
box {
    select kind=Container key=3 { classes a b; states Hovered; inside kind=Menu class=x; }
    layout basis=(%)30 align_self=#null { size (%)50 $space; columns (fr)1 Auto; column span=$span; }
    paint background="mix($accent, #ffffff, 20%)" { shadow color=$accent { offset 0 1; }; }
    text { style { fonts Geist; }; }
    transition duration=$fast
    icons tick=$tick down="$$d"
}
"##,
    ] {
        damage::damage(sheet.as_bytes(), |bytes| {
            let Ok(text) = std::str::from_utf8(bytes) else {
                return;
            };
            let _ = ui.set_style(&[StyleSheet::from_text("damaged.style.kdl", text)], &theme);
        });
    }
}
