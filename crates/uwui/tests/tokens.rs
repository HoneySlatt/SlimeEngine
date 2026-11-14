//! Tokens (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`): an own style
//! names a theme's value by a token of its kind; the interface reads each declared token once, in
//! its kind, when it takes its themes, refuses a value of another kind, and gives a token that the
//! themes lack, or that no list declared, its fallback.

use std::time::Duration as Time;

use slime_color::LinearColor;
use uwui::host::TargetId;
use uwui::style::{Dimension, Style};
use uwui::text::Fonts;
use uwui::theme::{
    self, Color, Duration, Fallback, Length, Number, StyleSheet, Theme, Token, TokenError, Tokens,
};
use uwui::tree::{Element, ElementId, ElementState, LayoutDirection, RootSettings, Ui};

const FACE: Token<Color> = Token::new("test_face", Fallback::Srgb(0xFFB000));
const RING: Token<Color> = Token::new("test_ring", Fallback::Token(&theme::ACCENT));
const ECHO: Token<Color> = Token::new("test_echo", Fallback::Token(&FACE));
const GAP: Token<Length> = Token::new("test_gap", Fallback::Value(5.0));
const DIM: Token<Number> = Token::new("test_dim", Fallback::Value(0.5));
const FADE: Token<Duration> = Token::new("test_fade", Fallback::Value(Time::from_millis(80)));
const TOKENS: Tokens = Tokens::new(&[&FACE, &RING, &ECHO, &GAP, &DIM, &FADE]);

/// The color of sRGB's `0xRRGGBB`, opaque.
fn srgb(hex: u32) -> LinearColor {
    let [_, r, g, b] = hex.to_be_bytes();
    LinearColor::from_srgb8(r, g, b)
}

fn theme(text: &str) -> Theme {
    Theme::from_text("test.theme", text)
}

/// An interface that declares [`TOKENS`] and takes `themes`, whose root holds `elements`: the
/// interface and the elements' ids, after a frame.
fn styled(themes: &[Theme], elements: Vec<Element>) -> (Ui, Vec<ElementId>) {
    let mut ui = Ui::new(Fonts::new());
    ui.declare_tokens(&TOKENS).expect("names of their own");
    ui.set_style(&[], themes).expect("valid themes");
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: uwui::host::TargetId::MAIN,
    };
    let root = ui.add_root("main", settings, |s| s);
    let mut ids = Vec::new();
    ui.build(root, 0, |children| {
        for (index, element) in elements.into_iter().enumerate() {
            ids.push(children.add(index, element, |_| {}));
        }
    });
    ui.frame(TargetId::MAIN);
    (ui, ids)
}

fn style(ui: &Ui, id: ElementId) -> &Style {
    ui.style(id).expect("styled")
}

#[test]
fn each_kind_of_token_sets_its_leaves_from_the_themes() {
    let given = theme(
        "color {\n    test_face \"#2BB673\"\n}\nlength {\n    test_gap 12\n}\n\
         number {\n    test_dim 0.25\n}\nduration {\n    test_fade (ms)300\n}\n",
    );
    let element = Element::container().style(|s| {
        s.background(FACE)
            .padding(GAP)
            .width(GAP)
            .opacity(DIM)
            .transition_duration(FADE)
    });
    let (ui, ids) = styled(&[given], vec![element]);
    let style = style(&ui, ids[0]);
    assert_eq!(style.paint.background, srgb(0x2BB673));
    assert_eq!(style.layout.padding.start, 12.0);
    assert_eq!(style.layout.padding.bottom, 12.0);
    assert_eq!(style.layout.size[0], Dimension::Pixels(12.0));
    assert_eq!(style.paint.opacity, 0.25);
    assert_eq!(style.transition.duration, 0.3);
}

#[test]
fn a_token_that_the_themes_lack_takes_its_fallback() {
    let element = Element::container().style(|s| {
        s.background(FACE)
            .border_color(RING)
            .padding(GAP)
            .opacity(DIM)
            .transition_duration(FADE)
    });
    let accent = theme("color {\n    accent \"#123456\"\n}\n");
    let (ui, ids) = styled(&[accent], vec![element]);
    let style = style(&ui, ids[0]);
    // A fallback in sRGB is decoded as a theme's `"#FFB000"` is.
    assert_eq!(style.paint.background, srgb(0xFFB000));
    // A fallback that names a token follows the value that the themes give it.
    assert_eq!(style.paint.border, srgb(0x123456));
    assert_eq!(style.layout.padding.start, 5.0);
    assert_eq!(style.paint.opacity, 0.5);
    assert_eq!(style.transition.duration, 0.08);
}

#[test]
fn a_fallback_that_names_a_token_lacking_takes_that_tokens_fallback_or_its_value() {
    let echo = Element::container().style(|s| s.background(ECHO));
    let (ui, ids) = styled(
        &[theme("color {\n    accent \"#123456\"\n}\n")],
        vec![echo.clone()],
    );
    assert_eq!(style(&ui, ids[0]).paint.background, srgb(0xFFB000));
    let (ui, ids) = styled(
        &[theme("color {\n    test_face \"#2BB673\"\n}\n")],
        vec![echo],
    );
    assert_eq!(style(&ui, ids[0]).paint.background, srgb(0x2BB673));
}

#[test]
fn a_token_named_in_a_state_reads_the_themes_too() {
    let button = Element::button("Save").style(|s| s.hover(|s| s.background(FACE)));
    let (mut ui, ids) = styled(
        &[theme("color {\n    test_face \"#2BB673\"\n}\n")],
        vec![button],
    );
    let state = ElementState {
        hovered: true,
        ..ui.state(ids[0]).expect("an element")
    };
    ui.set_state(ids[0], state);
    ui.frame(TargetId::MAIN);
    assert_eq!(style(&ui, ids[0]).paint.background, srgb(0x2BB673));
}

#[test]
fn a_token_and_a_literal_share_no_style_whatever_their_values() {
    // An own style keeps a token's declaration holding nothing, 0 and transparent, until it is
    // read: a literal of those values must not take the token's style, nor the token the
    // literal's.
    let literal = Element::button("A").style(|s| s.padding(0.0).background(srgb(0xFFB000)));
    let token = Element::button("A").style(|s| s.padding(GAP).background(srgb(0xFFB000)));
    let transparent =
        Element::button("A").style(|s| s.background(LinearColor::rgba(0.0, 0.0, 0.0, 0.0)));
    let face = Element::button("A").style(|s| s.background(FACE));
    let given = theme("length {\n    test_gap 9\n}\ncolor {\n    test_face \"#2BB673\"\n}\n");
    let (ui, ids) = styled(&[given], vec![literal, token, transparent, face]);
    assert_eq!(style(&ui, ids[0]).layout.padding.start, 0.0);
    assert_eq!(style(&ui, ids[1]).layout.padding.start, 9.0);
    assert_eq!(style(&ui, ids[2]).paint.background.a, 0.0);
    assert_eq!(style(&ui, ids[3]).paint.background, srgb(0x2BB673));
}

#[test]
fn a_change_of_themes_gives_a_token_its_new_value_and_leaves_a_literal() {
    let token = Element::container().style(|s| s.padding(GAP).background(FACE));
    let literal = Element::container().style(|s| s.padding(5.0));
    let (mut ui, ids) = styled(&[], vec![token, literal]);
    assert_eq!(style(&ui, ids[0]).layout.padding.start, 5.0);
    ui.set_style(&[], &[theme("length {\n    test_gap 9\n}\n")])
        .expect("a valid theme");
    ui.frame(TargetId::MAIN);
    assert_eq!(style(&ui, ids[0]).layout.padding.start, 9.0);
    assert_eq!(style(&ui, ids[1]).layout.padding.start, 5.0);
}

#[test]
fn a_token_that_no_list_declared_takes_its_fallback_the_themes_unread() {
    const LOOSE: Token<Length> = Token::new("test_loose", Fallback::Value(7.0));
    let element = Element::container().style(|s| s.padding(LOOSE));
    let (ui, ids) = styled(&[theme("length {\n    test_loose 30\n}\n")], vec![element]);
    assert_eq!(style(&ui, ids[0]).layout.padding.start, 7.0);
}

#[test]
fn a_list_declared_after_the_themes_is_read_at_their_next_giving() {
    const LATE: Token<Length> = Token::new("test_late", Fallback::Value(7.0));
    const LATE_TOKENS: Tokens = Tokens::new(&[&LATE]);
    let given = theme("length {\n    test_late 30\n}\n");
    let element = Element::container().style(|s| s.padding(LATE));
    let (mut ui, ids) = styled(std::slice::from_ref(&given), vec![element]);
    assert_eq!(style(&ui, ids[0]).layout.padding.start, 7.0);
    ui.declare_tokens(&LATE_TOKENS).expect("a name of its own");
    ui.set_style(&[], &[given]).expect("a valid theme");
    ui.frame(TargetId::MAIN);
    assert_eq!(style(&ui, ids[0]).layout.padding.start, 30.0);
}

#[test]
fn a_value_of_another_kind_not_finite_or_a_negative_duration_refuses_the_themes() {
    for (text, why, said) in [
        (
            "number {\n    test_face 12\n}\n",
            "a number for a color",
            "`test_face` is a token, a color that code reads: the themes give it as a number",
        ),
        (
            "color {\n    test_gap \"#FFFFFF\"\n}\n",
            "a color for a length",
            // The theme kept, below it, gave `test_gap` as a length first.
            "`test_gap` is a color here, but a length in `test.theme`",
        ),
        (
            "length {\n    test_gap #nan\n}\n",
            "a length that is no number",
            "`test_gap`: a number that is not finite",
        ),
        (
            "number {\n    test_dim #inf\n}\n",
            "a number without bounds",
            "`test_dim`: a number that is not finite",
        ),
        (
            "duration {\n    test_fade (s)-0.1\n}\n",
            "a negative duration",
            "`test_fade`: a duration is never negative",
        ),
        (
            "color {\n    size \"#FFFFFF\"\n}\nlength {\n    test_gap $size\n}\n",
            "a name of a color for a length",
            "`test_gap` names `size`, which is a color, not a length",
        ),
    ] {
        let mut ui = Ui::new(Fonts::new());
        ui.declare_tokens(&TOKENS).expect("names of their own");
        let kept = theme("length {\n    test_gap 3\n}\n");
        ui.set_style(&[], std::slice::from_ref(&kept))
            .expect("a valid theme");
        let error = ui
            .set_style(&[], &[kept, Theme::from_text("wrong.theme", text)])
            .expect_err(why);
        assert_eq!(
            error.errors()[0].document,
            "wrong.theme",
            "{why}: the theme at fault"
        );
        assert!(
            error.to_string().contains(said),
            "{why}: the error says why: {error}"
        );
        // The interface keeps the style it had.
        let settings = RootSettings {
            size: [100.0, 100.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
            target: uwui::host::TargetId::MAIN,
        };
        let root = ui.add_root("main", settings, |s| s.padding(GAP));
        ui.frame(TargetId::MAIN);
        assert_eq!(
            ui.style(root).expect("styled").layout.padding.start,
            3.0,
            "{why}: the value it had"
        );
    }
}

#[test]
fn a_name_declared_by_two_lists_is_refused_and_one_token_declared_again_is_one() {
    const OTHER_KIND: Token<Number> = Token::new("test_face", Fallback::Value(1.0));
    const OTHER_FALLBACK: Token<Color> = Token::new("test_face", Fallback::Srgb(0x000000));
    const ROLE: Token<Color> = Token::new("accent", Fallback::Srgb(0x000000));
    let mut ui = Ui::new(Fonts::new());
    ui.declare_tokens(&TOKENS).expect("names of their own");
    ui.declare_tokens(&TOKENS)
        .expect("the same tokens, declared once");
    for (token, why) in [
        (&OTHER_KIND as &'static dyn theme::AnyToken, "another kind"),
        (&OTHER_FALLBACK, "another fallback"),
        (&ROLE, "a role's name"),
    ] {
        let list = Tokens::new(Box::leak(Box::new([token])));
        assert_eq!(
            ui.declare_tokens(&list),
            Err(TokenError::DeclaredTwice {
                name: if why == "a role's name" {
                    "accent"
                } else {
                    "test_face"
                }
            }),
            "{why}"
        );
    }
    const TWICE: Tokens = Tokens::new(&[&OTHER_KIND, &OTHER_KIND]);
    let mut fresh = Ui::new(Fonts::new());
    fresh
        .declare_tokens(&TWICE)
        .expect("one token twice in a list is one token");
    let mixed: Tokens = Tokens::new(&[&GAP, &OTHER_FALLBACK, &FACE]);
    assert!(
        fresh.declare_tokens(&mixed).is_err(),
        "test_face, a number already"
    );
    // A list refused declares none of its tokens: `test_gap` stays undeclared.
    fresh
        .set_style(
            &[] as &[StyleSheet],
            &[theme("length {\n    test_gap 30\n}\n")],
        )
        .expect("a valid theme");
    let settings = RootSettings {
        size: [100.0, 100.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: uwui::host::TargetId::MAIN,
    };
    let root = fresh.add_root("main", settings, |s| s.padding(GAP));
    fresh.frame(TargetId::MAIN);
    assert_eq!(fresh.style(root).expect("styled").layout.padding.start, 5.0);
}

#[test]
fn a_rule_that_names_a_number_that_is_not_finite_refuses_the_themes() {
    let sheet = StyleSheet::from_text(
        "test.style",
        "box {\n    select kind=Container\n    layout { padding top=$space; }\n}\n",
    );
    for text in [
        "length {\n    space #nan\n}\n",
        "length {\n    space #inf\n}\n",
        "length {\n    space #-inf\n}\n",
    ] {
        let mut ui = Ui::new(Fonts::new());
        let error = ui
            .set_style(
                std::slice::from_ref(&sheet),
                &[Theme::from_text("wrong.theme", text)],
            )
            .expect_err(text);
        assert!(
            error.to_string().contains("finite"),
            "{text}: the error says why: {error}"
        );
    }
    let mut ui = Ui::new(Fonts::new());
    ui.set_style(&[sheet], &[theme("length {\n    space 4\n}\n")])
        .expect("a finite number");
}

#[test]
fn two_calls_of_style_each_read_their_own_tokens() {
    let element = Element::container()
        .style(|s| s.padding_top(GAP).background(FACE))
        .style(|s| s.padding_bottom(GAP).border_color(RING).opacity(DIM));
    let given = theme(
        "length {\n    test_gap 10\n}\ncolor {\n    test_face \"#2BB673\"\n    accent \"#123456\"\n}\n\
         number {\n    test_dim 0.25\n}\n",
    );
    let (ui, ids) = styled(&[given], vec![element]);
    let style = style(&ui, ids[0]);
    assert_eq!(
        (style.layout.padding.top, style.layout.padding.bottom),
        (10.0, 10.0)
    );
    assert_eq!(style.paint.background, srgb(0x2BB673));
    assert_eq!(style.paint.border, srgb(0x123456));
    assert_eq!(style.paint.opacity, 0.25);
}

#[test]
fn an_element_built_again_naming_another_token_takes_its_value() {
    const OTHER: Token<Length> = Token::new("test_other", Fallback::Value(1.0));
    const OTHER_TOKENS: Tokens = Tokens::new(&[&OTHER]);
    // One place of the call for both, so that only the token tells them apart.
    let make = |token: Token<Length>| Element::container().style(move |s| s.padding_top(token));
    let mut ui = Ui::new(Fonts::new());
    ui.declare_tokens(&TOKENS).expect("names of their own");
    ui.declare_tokens(&OTHER_TOKENS).expect("a name of its own");
    ui.set_style(
        &[],
        &[theme("length {\n    test_gap 10\n    test_other 20\n}\n")],
    )
    .expect("a valid theme");
    let settings = RootSettings {
        size: [100.0, 100.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: uwui::host::TargetId::MAIN,
    };
    let root = ui.add_root("main", settings, |s| s);
    let mut id = None;
    ui.build(root, 0, |children| {
        id = Some(children.add("subject", make(GAP), |_| {}));
    });
    ui.frame(TargetId::MAIN);
    let id = id.expect("built");
    assert_eq!(style(&ui, id).layout.padding.top, 10.0);
    ui.build(root, 1, |children| {
        children.add("subject", make(OTHER), |_| {});
    });
    ui.frame(TargetId::MAIN);
    assert_eq!(style(&ui, id).layout.padding.top, 20.0);
}

#[test]
fn a_token_that_no_list_declared_takes_a_declared_tokens_value_that_its_fallback_names() {
    const LOOSE_RING: Token<Color> = Token::new("test_loose_ring", Fallback::Token(&theme::ACCENT));
    const LOOSE_ECHO: Token<Color> = Token::new("test_loose_echo", Fallback::Token(&LOOSE_RING));
    let ring = Element::container().style(|s| s.background(LOOSE_RING));
    let echo = Element::container().style(|s| s.background(LOOSE_ECHO));
    let given = theme("color {\n    accent \"#123456\"\n    test_loose_ring \"#2BB673\"\n}\n");
    let (ui, ids) = styled(&[given], vec![ring, echo]);
    // The themes are read for the tokens declared alone: `test_loose_ring`'s value is unread.
    assert_eq!(style(&ui, ids[0]).paint.background, srgb(0x123456));
    assert_eq!(style(&ui, ids[1]).paint.background, srgb(0x123456));
}

#[test]
fn a_token_of_a_declared_name_and_another_kind_takes_its_own_fallback() {
    const WRONG_KIND: Token<Color> = Token::new("test_gap", Fallback::Srgb(0xFFB000));
    let element = Element::container().style(|s| s.background(WRONG_KIND).padding(GAP));
    let (ui, ids) = styled(&[theme("length {\n    test_gap 10\n}\n")], vec![element]);
    assert_eq!(style(&ui, ids[0]).paint.background, srgb(0xFFB000));
    assert_eq!(style(&ui, ids[0]).layout.padding.start, 10.0);
}

#[test]
fn a_list_whose_static_tokens_fall_back_in_a_cycle_is_refused() {
    static LOOP_A: Token<Length> = Token::new("test_loop_a", Fallback::Token(&LOOP_B));
    static LOOP_B: Token<Length> = Token::new("test_loop_b", Fallback::Token(&LOOP_A));
    static LOOPS: Tokens = Tokens::new(&[&LOOP_A]);
    let mut ui = Ui::new(Fonts::new());
    assert_eq!(
        ui.declare_tokens(&LOOPS),
        Err(TokenError::FallbackCycle {
            name: "test_loop_a"
        })
    );
    // Named by an own style undeclared, it takes a length that holds nothing, and the program goes
    // on.
    let element = Element::container().style(|s| s.padding(LOOP_A));
    ui.set_style(&[], &[theme("length {\n    test_gap 10\n}\n")])
        .expect("a valid theme");
    let settings = RootSettings {
        size: [100.0, 100.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
        target: uwui::host::TargetId::MAIN,
    };
    let root = ui.add_root("main", settings, |s| s);
    let mut id = None;
    ui.build(root, 0, |children| {
        id = Some(children.add("subject", element, |_| {}));
    });
    ui.frame(TargetId::MAIN);
    assert_eq!(style(&ui, id.expect("built")).layout.padding.start, 0.0);
}

#[test]
fn a_duration_too_long_to_hold_is_refused_and_told_so() {
    let mut ui = Ui::new(Fonts::new());
    ui.declare_tokens(&TOKENS).expect("names of their own");
    let error = ui
        .set_style(&[], &[theme("duration {\n    test_fade (s)1e30\n}\n")])
        .expect_err("a duration past what a time holds");
    assert!(error.to_string().contains("too long"), "{error}");
}
