//! Themes in KDL: each value in the group of its kind, a stack's names followed once, and each
//! refusal at its line (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).

#[path = "../../slime-assets/tests/common/damage.rs"]
mod damage;

use std::time::{Duration as Time, Instant};

use slime_color::LinearColor;
use uwui::DocumentErrors;
use uwui::text::Fonts;
use uwui::theme::{Duration, Fallback, Length, Number, StyleSheet, Theme, Token, Tokens};
use uwui::tree::{Element, ElementId, LayoutDirection, RootSettings, Ui};

const FAST: Token<Duration> = Token::new("fast", Fallback::Value(std::time::Duration::ZERO));
const GAP: Token<Length> = Token::new("gap", Fallback::Value(0.0));
const WEIGHT: Token<Number> = Token::new("weight", Fallback::Value(0.0));
const TOKENS: Tokens = Tokens::new(&[&FAST, &GAP, &WEIGHT]);

/// An interface that declares [`TOKENS`], and its root, given `themes`, each `(name, text)`,
/// stacked, and a sheet whose box names `$fill` if `fill`.
fn styled(themes: &[(&str, &str)], fill: bool) -> Result<(Ui, ElementId), DocumentErrors> {
    let mut ui = Ui::new(Fonts::new());
    ui.declare_tokens(&TOKENS)
        .expect("tokens of their own names");
    let settings = RootSettings {
        size: [100.0, 100.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, |s| {
        s.padding_top(GAP)
            .transition_duration(FAST)
            .font_weight(WEIGHT)
    });
    ui.build(root, 0, |children| {
        children.add("box", Element::container(), |_| {});
    });
    let sheet = "box {\n    select kind=Container\n    paint background=$fill\n}\n";
    let sheets: Vec<StyleSheet> = fill
        .then(|| StyleSheet::from_text("box.style", sheet))
        .into_iter()
        .collect();
    let themes: Vec<Theme> = themes
        .iter()
        .map(|(name, text)| Theme::from_text(*name, *text))
        .collect();
    ui.set_style(&sheets, &themes)?;
    ui.frame();
    Ok((ui, root))
}

/// The errors that `themes` give, as their text, one a line.
fn refused(themes: &[(&str, &str)]) -> String {
    match styled(themes, false) {
        Ok(_) => panic!("{themes:?} are taken"),
        Err(errors) => errors.to_string(),
    }
}

#[test]
fn a_theme_gives_each_kind_in_its_group_and_tokens_read_them() {
    let theme = "\
color {
    fill \"#ff0000\"
}
length {
    gap (px)6
}
duration {
    fast (ms)120
}
number {
    weight 500
}
text {
    tick \"\\u{E06C}\"
}
";
    let (ui, root) = styled(&[("a.theme", theme)], true).expect("a theme");
    let style = ui.style(root).expect("styled");
    assert_eq!(style.layout.padding.top, 6.0);
    assert_eq!(style.transition.duration, 0.12);
    assert_eq!(style.text.style.weight, 500.0);
    let fill = ui
        .style(ui.children(root)[0])
        .expect("styled")
        .paint
        .background;
    assert_eq!(fill, LinearColor::from_srgb8(255, 0, 0));
}

#[test]
fn a_duration_in_seconds_and_a_name_take_their_group_s_unit() {
    let theme = "duration {\n    slow (s)0.2\n    fast $slow\n}\nlength {\n    space 4\n    gap $space\n}\n";
    let (ui, root) = styled(&[("a.theme", theme)], false).expect("a theme");
    let style = ui.style(root).expect("styled");
    assert_eq!(style.transition.duration, 0.2);
    assert_eq!(style.layout.padding.top, 4.0);
}

#[test]
fn each_wrong_value_is_refused_at_its_line() {
    let cases: &[(&str, &str)] = &[
        ("colors {\n}\n", "line 1, column 1: `colors` is not a group"),
        (
            "length {\n}\nlength {\n}\n",
            "line 3, column 1: the group `length` is given twice, first at line 1",
        ),
        (
            "length 4 {\n}\n",
            "the group `length` holds its values in a block alone",
        ),
        (
            "length {\n    gap 1\n}\nnumber {\n    gap 2\n}\n",
            "line 5, column 5: `gap` is given twice in this theme, first at line 2",
        ),
        ("color {\n    fill #null\n}\n", "`#null` removes no value"),
        (
            "duration {\n    fast 120\n}\n",
            "a duration writes its unit",
        ),
        (
            "duration {\n    fast (ms)-1\n}\n",
            "a duration is never negative",
        ),
        (
            "duration {\n    fast (px)1\n}\n",
            "a duration is in `(ms)` or `(s)`, not `(px)`",
        ),
        (
            "length {\n    gap (ms)1\n}\n",
            "a length is in logical pixels",
        ),
        (
            "number {\n    weight (px)1\n}\n",
            "a number takes no unit `(px)`",
        ),
        (
            "duration {\n    slow (ms)1\n    fast (ms)$slow\n}\n",
            "a name takes the unit of its value's group",
        ),
        ("length {\n    gap #inf\n}\n", "a number that is not finite"),
        (
            "length {\n    gap \"wide\"\n}\n",
            "the value is not a length",
        ),
        (
            "length {\n    gap 1 2\n}\n",
            "a value is one argument, a length",
        ),
        ("length {\n    gap size=1\n}\n", "not a property"),
        ("color {\n    fill \"#12345\"\n}\n", "is not a color"),
        ("color {\n    fill $\n}\n", "`$` names no value"),
        (
            "color {\n    fill $gap\n}\nlength {\n    gap 1\n}\n",
            "`fill` names `gap`, which is a length, not a color",
        ),
        (
            "color {\n    fill \"alpha(#000000, $tint)\"\n    tint \"#ffffff\"\n}\n",
            "`fill` names `tint`, which is a color, not a number",
        ),
        (
            "color {\n    fill $a\n    a $fill\n}\n",
            "which names it back through other values",
        ),
        (
            "length {\n    gap $nowhere\n}\n",
            "no theme gives a value named `nowhere`",
        ),
        ("color { fill = }\n", "a.theme: line 1,"),
        // A number past what an `f32` holds, which would be taken as infinite.
        ("length {\n    gap 1e39\n}\n", "past what an `f32` holds"),
        (
            "number {\n    weight -1e39\n}\n",
            "past what an `f32` holds",
        ),
        (
            "duration {\n    fast (s)1e30\n}\n",
            "a duration too long to hold",
        ),
        // A color whose function, given a share without bounds, computes channels that are no
        // finite number, written or named.
        (
            "color {\n    fill \"mix(#ff0000, #00ff00, 3e38)\"\n}\n",
            "a color with a channel that is no finite number",
        ),
        (
            "color {\n    fill \"mix(#ff0000, #00ff00, $big)\"\n}\nnumber {\n    big 3e38\n}\n",
            "a color with a channel that is no finite number",
        ),
        (
            "(kind)length {\n}\n",
            "the group `length` holds its values in a block alone",
        ),
        (
            "length {\n    (px)gap 1\n}\n",
            "a value's name takes no type",
        ),
        (
            "length {\n    gap 1 {\n        more 2\n    }\n}\n",
            "a value holds no block",
        ),
        (
            "color {\n    fill (px)\"#000000\"\n}\n",
            "a color takes no unit `(px)`",
        ),
        ("text {\n    tick (px)x\n}\n", "a text takes no unit `(px)`"),
    ];
    for (theme, expected) in cases {
        let error = refused(&[("a.theme", theme)]);
        assert!(error.starts_with("a.theme: line "), "{theme}: {error}");
        assert!(error.contains(expected), "{theme}: {error}");
    }
}

#[test]
fn a_later_theme_that_gives_a_name_in_another_group_is_refused_naming_both_places() {
    let error = refused(&[
        ("base.theme", "length {\n    gap 4\n}\n"),
        ("project.theme", "number {\n    gap 4\n}\n"),
    ]);
    assert_eq!(
        error,
        "project.theme: line 2, column 5: `gap` is a number here, but a length in `base.theme`, \
         at line 2: the first theme to give a name sets its kind"
    );
}

#[test]
fn a_token_given_in_another_group_is_refused_at_its_value() {
    let error = refused(&[("a.theme", "number {\n    gap 4\n}\n")]);
    assert_eq!(
        error,
        "a.theme: line 2, column 5: `gap` is a token, a length that code reads: the themes give \
         it as a number"
    );
}

#[test]
fn a_chain_of_names_past_its_bound_and_functions_nested_too_deep_are_refused_without_overflow() {
    // 65 names, each naming the next.
    let mut chain = String::from("length {\n");
    for at in 0..65 {
        chain.push_str(&format!("    v{at} $v{}\n", at + 1));
    }
    chain.push_str("    v65 1\n}\n");
    assert!(
        refused(&[("a.theme", &chain)]).contains("a chain of more than 64 names"),
        "{chain}"
    );
    // Within the bound, a chain is followed.
    let mut short = String::from("length {\n");
    for at in 0..60 {
        short.push_str(&format!("    v{at} $v{}\n", at + 1));
    }
    short.push_str("    v60 3\n    gap $v0\n}\n");
    let (ui, root) = styled(&[("a.theme", &short)], false).expect("a chain within its bound");
    assert_eq!(ui.style(root).unwrap().layout.padding.top, 3.0);
    // The same chain, its tail's names met first, which a bound on the walk alone would miss.
    let mut tail_first = String::from("length {\n");
    for at in 0..65 {
        tail_first.push_str(&format!("    v{:03} $v{:03}\n", 200 - at, 199 - at));
    }
    tail_first.push_str("    v135 1\n}\n");
    assert!(
        refused(&[("a.theme", &tail_first)]).contains("a chain of more than 64 names"),
        "{tail_first}"
    );
    // Functions nested 20 000 deep, which a reader that recursed on them would overflow on.
    let deep = format!(
        "color {{\n    fill \"{}#000000{}\"\n}}\n",
        "alpha(".repeat(20_000),
        ", 50%)".repeat(20_000)
    );
    assert!(refused(&[("a.theme", &deep)]).contains("colour functions nested deeper than 8"));
    let eight = format!(
        "color {{\n    fill \"{}#000000{}\"\n}}\n",
        "alpha(".repeat(8),
        ", 50%)".repeat(8)
    );
    styled(&[("a.theme", &eight)], true).expect("eight functions nested");
}

#[test]
fn errors_are_gathered_up_to_their_cap() {
    let two = refused(&[("a.theme", "length {\n    a \"x\"\n    b \"y\"\n}\n")]);
    assert_eq!(two.lines().count(), 2, "{two}");
    assert!(
        two.lines().nth(1).unwrap().starts_with("a.theme: line 3,"),
        "{two}"
    );
    let mut many = String::from("length {\n");
    for at in 0..40 {
        many.push_str(&format!("    v{at} \"x\"\n"));
    }
    many.push_str("}\n");
    let errors = styled(&[("a.theme", &many)], false).err().expect("refused");
    assert_eq!(errors.errors().len(), DocumentErrors::MOST);
    assert!(errors.more());
    assert!(errors.to_string().ends_with("and more, past the first 16"));
}

#[test]
fn a_theme_of_many_wrong_values_is_refused_in_time_linear_in_its_size() {
    // Each error past the cap is counted, not placed at its line, which reads the text up to it.
    let mut many = String::from("length {\n");
    for at in 0..50_000 {
        many.push_str(&format!("    v{at:06} #null\n"));
    }
    for at in 0..50_000 {
        many.push_str(&format!("    w{at:06} 4\n"));
    }
    many.push_str("}\n");
    let start = Instant::now();
    let error = refused(&[("a.theme", &many)]);
    assert!(error.ends_with("and more, past the first 16"), "{error}");
    // A debug build reads it in about a second; a time quadratic in the values, in minutes.
    assert!(
        start.elapsed() < Time::from_secs(20),
        "{:?} for 100 000 values",
        start.elapsed()
    );
}

#[test]
fn a_damaged_theme_is_refused_or_read_never_panicking() {
    let mut ui = Ui::new(Fonts::new());
    for theme in [
        include_str!("../look/base.theme.kdl"),
        include_str!("../look/contrast.theme.kdl"),
    ] {
        damage::damage(theme.as_bytes(), |bytes| {
            let Ok(text) = std::str::from_utf8(bytes) else {
                return;
            };
            let _ = ui.set_style(&[], &[Theme::from_text("damaged.theme.kdl", text)]);
        });
    }
}

#[test]
fn a_whole_number_of_a_theme_sets_a_field_of_whole_numbers_and_a_fraction_is_refused() {
    let sheet = "box {\n    select kind=Container\n    layout { column span=$span; }\n}\n";
    let mut ui = Ui::new(Fonts::new());
    let settings = RootSettings {
        size: [100.0, 100.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, |s| s);
    ui.build(root, 0, |children| {
        children.add("box", Element::container(), |_| {});
    });
    let sheets = [StyleSheet::from_text("box.style", sheet)];
    ui.set_style(
        &sheets,
        &[Theme::from_text("a.theme", "number {\n    span 2\n}\n")],
    )
    .expect("a whole number");
    ui.frame();
    let child = ui.children(root)[0];
    assert_eq!(ui.style(child).expect("styled").layout.column.span, 2);
    let error = ui
        .set_style(
            &sheets,
            &[Theme::from_text("a.theme", "number {\n    span 2.5\n}\n")],
        )
        .expect_err("a fraction");
    assert!(
        error
            .to_string()
            .contains("a number that is not whole from 0 to 65535"),
        "{error}"
    );
}
