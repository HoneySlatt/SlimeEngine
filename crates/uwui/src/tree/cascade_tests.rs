//! The typed cascade checked against a model of it (`docs/adr/0229-styles-typed-as-declarations.md`):
//! random rules and own styles, over every leaf of [`Style`] as its derive lists them, compose as
//! the model says, each rule's declarations applied in their order, then each leaf of the own style
//! that is not its default, an own style's arrays set by element
//! (`docs/adr/0231-an-elements-own-style-written-in-code.md`); and every leaf through the mask
//! order, the rules' states against the own style's.
//!
//! The model was checked to compose as the reflected cascade that the typed one replaced, on that
//! cascade's 3,000 seeded cases, before reflection left uwui (M6d.3c3); the fixtures of the
//! `widgets` example's styles (`examples/tests/widgets.rs`) outlive both. The model shares the
//! derive's `apply` and `declared` with the cascade: it checks the order in which the cascade
//! applies declarations, not the declarations themselves, which
//! `a_style_s_declarations_applied_to_another_make_it` checks apart. Each leaf takes the values of
//! three styles, not every variant of an enum: the order does not depend on the values.

use std::panic::Location;

use slime_color::LinearColor;

use super::Ui;
use crate::style::{
    Align, Dimension, Display, Easing, FlexDirection, GridPlace, Justify, OwnStyle, Same, Shadow,
    Sides, Style, StyleBuilder, StyleDeclaration, Track, state_bit,
};
use crate::text::{Alignment, Direction, Fonts};
use crate::theme::{self, KindName, Rule, Selector, StateName};

/// A generator of numbers that repeats from its seed: xorshift, as `tests/dock.rs`'s.
struct Numbers(u64);

impl Numbers {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }

    fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }
}

/// The values that random rules and own styles give each leaf, by the leaf's number: the default
/// style's, [`every_leaf_changed`]'s and [`every_leaf_odd`]'s where it differs from the default.
fn values() -> Vec<Vec<StyleDeclaration>> {
    let default = Style::default();
    let changed = every_leaf_changed();
    let mut values = vec![Vec::new(); Style::LEAVES];
    for (style, other) in [
        (&default, &changed),
        (&changed, &default),
        (&every_leaf_odd(), &default),
    ] {
        style.declared(other, &mut |declaration| {
            values[declaration.leaf()].push(declaration);
        });
    }
    values
}

/// A part of a style, as a rule sets one: each leaf with a chance of one in three, its value one of
/// `values`'.
fn random_part(values: &[Vec<StyleDeclaration>], numbers: &mut Numbers) -> Vec<StyleDeclaration> {
    let mut part = Vec::new();
    for choices in values {
        if numbers.chance(3) {
            part.push(choices[numbers.below(choices.len())].clone());
        }
    }
    part
}

#[test]
fn random_rules_and_own_styles_compose_as_the_model_says() {
    let values = values();
    assert!(
        values.iter().all(|choices| choices.len() >= 2),
        "each leaf takes two values at least"
    );
    let default = Style::default();
    let mut ui = Ui::new(Fonts::new());
    // A container's base is the default style: it reads no color that a role gives.
    let base = ui.base_of[KindName::Container.index()];
    let mut numbers = Numbers(0x2545_f491_4f6c_dd1d);
    for case in 0..3_000 {
        let rules: Vec<Rule> = (0..numbers.below(5))
            .map(|_| Rule {
                selector: Selector::default(),
                declarations: random_part(&values, &mut numbers),
            })
            .collect();
        // An own style at its default, or the default with some of its leaves set.
        let mut code = default.clone();
        if !numbers.chance(4) {
            for declaration in random_part(&values, &mut numbers) {
                declaration.apply(&mut code);
            }
        }
        let mut want = default.clone();
        for rule in &rules {
            for declaration in &rule.declarations {
                declaration.apply(&mut want);
            }
        }
        code.declared(&default, &mut |declaration| declaration.apply(&mut want));
        ui.rules = rules;
        let selected: Vec<u32> = (0..ui.rules.len() as u32).collect();
        let mut own = OwnStyle::default();
        own.extend(StyleBuilder::new(Location::caller()).with_style(&code));
        let typed = ui.compose(base, &selected, &own, 0).0;
        // Compared by `Debug`, apart from the cascade's own `Same`: it writes `-0.0` apart from
        // `0.0`, and each float that is a number by the shortest text that reads back to it.
        assert_eq!(
            format!("{typed:?}"),
            format!("{want:?}"),
            "case {case}: the rules {:?} and the own style {code:?}",
            ui.rules
        );
    }
}

#[test]
fn own_styles_that_differ_by_a_signed_zero_share_no_style() {
    let mut ui = Ui::new(Fonts::new());
    let sheet = theme::StyleSheet::from_text(
        "gap.style",
        "gap {\n    select\n    layout { gap 4 4; }\n}\n",
    );
    ui.set_style(&[sheet], &[]).expect("a valid sheet");
    let root = ui.add_root(
        "main",
        super::RootSettings {
            size: [100.0, 100.0],
            scale: 1.0,
            direction: super::LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
            target: crate::host::TargetId::MAIN,
        },
        |s| s,
    );
    let (mut zero_box, mut negative_box) = (None, None);
    ui.build(root, 0, |children| {
        zero_box = Some(children.add(
            "zero",
            super::Element::container().style(|s| s.row_gap(0.0)),
            |_| {},
        ));
        negative_box = Some(children.add(
            "negative",
            super::Element::container().style(|s| s.row_gap(-0.0)),
            |_| {},
        ));
    });
    ui.frame();
    let gap = |id| ui.style(id).expect("styled").layout.gap;
    // Each sets the gap between rows alone, over the rule; the gap between columns stays the
    // rule's. The two differ by the sign of a zero, their bits: they share no style, as they did
    // when styles were shared by equality.
    let zero_gap = gap(zero_box.expect("built"));
    let negative_gap = gap(negative_box.expect("built"));
    assert!(
        zero_gap[0].to_bits() == 0 && zero_gap[1] == 4.0,
        "a gap between rows set to its default, the gap between columns the rule's: {zero_gap:?}"
    );
    assert!(
        negative_gap[0].to_bits() == (-0.0_f32).to_bits() && negative_gap[1] == 4.0,
        "the own style's gap between rows, the rule's between columns: {negative_gap:?}"
    );
}

/// A style whose every leaf holds another value than [`Style::default`]'s.
fn every_leaf_changed() -> Style {
    let color = LinearColor::rgba(0.1, 0.2, 0.3, 0.4);
    let mut style = Style::default();
    let layout = &mut style.layout;
    layout.display = Display::Grid;
    layout.direction = FlexDirection::Column;
    layout.wrap = true;
    layout.justify = Justify::End;
    layout.align_items = Align::Center;
    layout.align_self = Some(Align::End);
    layout.gap = [1.0, 2.0];
    layout.padding = crate::style::Sides::all(1.0);
    layout.margin = crate::style::Sides::all(1.0);
    layout.size = [Dimension::Pixels(1.0); 2];
    layout.min_size = [Dimension::Pixels(1.0); 2];
    layout.max_size = [Dimension::Pixels(1.0); 2];
    layout.grow = 1.0;
    layout.shrink = 0.0;
    layout.basis = Dimension::Pixels(1.0);
    layout.columns = vec![Track::Auto];
    layout.rows = vec![Track::Auto];
    layout.column.start = Some(1);
    layout.column.span = 2;
    layout.row.start = Some(1);
    layout.row.span = 2;
    let paint = &mut style.paint;
    paint.background = color;
    paint.border_width = 1.0;
    paint.border = color;
    paint.radii = [1.0; 4];
    paint.shadow = Some(Shadow::default());
    paint.opacity = 0.5;
    paint.backdrop = color;
    let text = &mut style.text;
    text.style.fonts = vec!["Geist".to_owned()];
    text.style.size = 12.0;
    text.style.weight = 700.0;
    text.style.italic = true;
    text.style.line_height = 1.2;
    text.style.alignment = Alignment::End;
    text.style.direction = Direction::RightToLeft;
    text.style.locale = Some("ja".to_owned());
    text.style.features = vec!["tnum".to_owned()];
    text.color = color;
    text.selection = color;
    let control = &mut style.control;
    control.size = 12.0;
    control.radius = 1.0;
    control.track = color;
    control.fill = color;
    control.mark = color;
    style.transition.duration = 0.1;
    style.transition.easing = Easing::Linear;
    let icons = &mut style.icons;
    for glyph in [
        &mut icons.font,
        &mut icons.tick,
        &mut icons.down,
        &mut icons.right,
        &mut icons.left,
        &mut icons.search,
        &mut icons.clear,
    ] {
        *glyph = "x".to_owned();
    }
    style
}

/// A style whose leaves hold a third value where they can, apart from the default's and
/// [`every_leaf_changed`]'s: signed zeros, values that are not a number, other variants and empty
/// texts, whose bits the cascade keeps.
fn every_leaf_odd() -> Style {
    let color = LinearColor::rgba(-0.0, f32::NAN, 3.25, 0.0);
    let mut style = every_leaf_changed();
    let layout = &mut style.layout;
    layout.display = Display::None;
    layout.direction = FlexDirection::ColumnReverse;
    layout.justify = Justify::Center;
    layout.align_items = Align::Start;
    layout.align_self = Some(Align::Stretch);
    layout.gap = [-0.0, f32::NAN];
    layout.padding = Sides {
        start: -0.0,
        end: f32::NAN,
        top: 3.25,
        bottom: 0.5,
    };
    layout.margin = Sides::all(f32::NAN);
    layout.size = [Dimension::Share(0.5), Dimension::Pixels(-0.0)];
    layout.min_size = [Dimension::Pixels(f32::NAN), Dimension::Share(-0.0)];
    layout.max_size = [Dimension::Share(1.0), Dimension::Pixels(12.0)];
    layout.grow = -0.0;
    layout.shrink = f32::NAN;
    layout.basis = Dimension::Share(0.25);
    layout.columns = vec![Track::Fraction(1.0), Track::Pixels(-0.0)];
    layout.rows = vec![Track::Pixels(f32::NAN)];
    layout.column = GridPlace {
        start: Some(7),
        span: 0,
    };
    layout.row = GridPlace {
        start: Some(0),
        span: 3,
    };
    let paint = &mut style.paint;
    paint.background = color;
    paint.border_width = -0.0;
    paint.border = color;
    paint.radii = [-0.0, f32::NAN, 0.5, 12.0];
    paint.shadow = Some(Shadow {
        color,
        offset: [-0.0, f32::NAN],
        blur: 3.25,
        spread: -1.0,
    });
    paint.opacity = f32::NAN;
    paint.backdrop = color;
    let text = &mut style.text;
    text.style.fonts = vec!["Geist".to_owned(), String::new()];
    text.style.size = -0.0;
    text.style.weight = f32::NAN;
    text.style.line_height = -0.0;
    text.style.alignment = Alignment::Center;
    text.style.locale = Some(String::new());
    text.style.features = vec![String::new()];
    text.color = color;
    text.selection = color;
    let control = &mut style.control;
    control.size = -0.0;
    control.radius = f32::NAN;
    control.track = color;
    control.fill = color;
    control.mark = color;
    style.transition.duration = -0.0;
    style.transition.easing = Easing::EaseOut;
    style.icons.font = "tnum".to_owned();
    style
}

/// The derive's `declared` and `apply`, which the model of the cascade shares with it, checked
/// apart from both: the declarations of a style against another, applied to the other, make the
/// style, compared by `Debug`, so that a leaf that `declared` leaves out or `apply` does not set is
/// found.
#[test]
fn a_style_s_declarations_applied_to_another_make_it() {
    let styles = [Style::default(), every_leaf_changed(), every_leaf_odd()];
    for (to_number, to) in styles.iter().enumerate() {
        for (from_number, from) in styles.iter().enumerate() {
            let mut made = from.clone();
            to.declared(from, &mut |declaration| declaration.apply(&mut made));
            assert_eq!(
                format!("{made:?}"),
                format!("{to:?}"),
                "style {to_number} declared against style {from_number}"
            );
        }
    }
}

/// A rule that sets `declaration`, for elements in the state `state`, or in any for none.
fn rule(state: Option<StateName>, declaration: &StyleDeclaration) -> Rule {
    Rule {
        selector: Selector {
            states: state.into_iter().collect(),
            ..Selector::default()
        },
        declarations: vec![declaration.clone()],
    }
}

#[test]
fn every_leaf_takes_the_mask_order() {
    let changed = every_leaf_changed();
    let default = Style::default();
    let (mut other, mut back) = (Vec::new(), Vec::new());
    changed.declared(&default, &mut |declaration| other.push(declaration));
    default.declared(&changed, &mut |declaration| back.push(declaration));
    assert_eq!(
        other.len(),
        Style::LEAVES,
        "every leaf changed, one declaration each"
    );
    let mut ui = Ui::new(Fonts::new());
    let base = ui.base_of[KindName::Container.index()];
    let hovered = Some(StateName::Hovered);
    for (set, at_default) in other.iter().zip(&back) {
        assert_eq!(
            set.leaf(),
            at_default.leaf(),
            "both in the order of the leaves"
        );
        let mut by_rule = default.clone();
        set.apply(&mut by_rule);
        let leaf = set.path();
        let compose = |ui: &mut Ui, rules: Vec<Rule>, own: OwnStyle, held: u8| {
            ui.rules = rules;
            let selected: Vec<u32> = (0..ui.rules.len() as u32).collect();
            ui.compose(base, &selected, &own, held).0
        };
        let own_base = OwnStyle::of([(None, at_default.clone())]);
        let own_hover = OwnStyle::of([(hovered, at_default.clone())]);
        let hover = state_bit(StateName::Hovered);
        // A rule's state beats a base set in code.
        let made = compose(&mut ui, vec![rule(hovered, set)], own_base.clone(), 0);
        assert!(
            made.same(&by_rule),
            "`{leaf}`: a rule's state over the own base"
        );
        // A stateless rule after it gives the field back to the own base, set to its default.
        let rules = vec![rule(hovered, set), rule(None, set)];
        let made = compose(&mut ui, rules, own_base.clone(), 0);
        assert!(
            made.same(&default),
            "`{leaf}`: the own base over a stateless rule after"
        );
        // A field set in code to its default beats a rule.
        let made = compose(&mut ui, vec![rule(None, set)], own_base, 0);
        assert!(
            made.same(&default),
            "`{leaf}`: the own base, at its default, over a rule"
        );
        // An own state that holds beats a rule's state.
        let made = compose(&mut ui, vec![rule(hovered, set)], own_hover.clone(), hover);
        assert!(
            made.same(&default),
            "`{leaf}`: an own state over a rule's state"
        );
        // One that does not hold leaves the rule's.
        let made = compose(&mut ui, vec![rule(None, set)], own_hover, 0);
        assert!(
            made.same(&by_rule),
            "`{leaf}`: an own state that does not hold"
        );
    }
}

/// The states that an own style may write, in the order that ADR-0231 gives them, the last
/// winning: written here apart from the cascade's own list, as the model's.
const ORDER: [StateName; 8] = [
    StateName::Hovered,
    StateName::Focused,
    StateName::FocusVisible,
    StateName::Pressed,
    StateName::Checked,
    StateName::Expanded,
    StateName::Selected,
    StateName::Disabled,
];

#[test]
fn random_rules_with_states_and_own_styles_with_states_compose_as_the_mask_order_says() {
    let changed = every_leaf_changed();
    let default = Style::default();
    // Two values for each leaf: the changed one, and the default.
    let mut values: Vec<[StyleDeclaration; 2]> = Vec::new();
    let mut other = Vec::new();
    changed.declared(&default, &mut |declaration| other.push(declaration));
    let mut back = Vec::new();
    default.declared(&changed, &mut |declaration| back.push(declaration));
    for (set, at_default) in other.into_iter().zip(back) {
        values.push([set, at_default]);
    }
    let mut ui = Ui::new(Fonts::new());
    let base = ui.base_of[KindName::Container.index()];
    let mut numbers = Numbers(0x9e37_79b9_7f4a_7c15);
    for case in 0..2_000 {
        // Three leaves for each case, so that rules and states meet on the same leaves.
        let pool: Vec<usize> = (0..3).map(|_| numbers.below(values.len())).collect();
        let declaration = |numbers: &mut Numbers| {
            let leaf = pool[numbers.below(pool.len())];
            values[leaf][numbers.below(2)].clone()
        };
        // The rules that select the element, each stateful or not: a stateful rule selected is in
        // a state that holds.
        let rules: Vec<Rule> = (0..numbers.below(6))
            .map(|_| {
                let state = numbers.chance(2).then(|| ORDER[numbers.below(ORDER.len())]);
                Rule {
                    selector: Selector {
                        states: state.into_iter().collect(),
                        ..Selector::default()
                    },
                    declarations: (0..1 + numbers.below(3))
                        .map(|_| declaration(&mut numbers))
                        .collect(),
                }
            })
            .collect();
        let own: Vec<(Option<StateName>, StyleDeclaration)> = (0..numbers.below(6))
            .map(|_| {
                let state = numbers.chance(2).then(|| ORDER[numbers.below(ORDER.len())]);
                (state, declaration(&mut numbers))
            })
            .collect();
        let held: Vec<StateName> = ORDER
            .iter()
            .copied()
            .filter(|_| numbers.chance(2))
            .collect();
        let held_bits = held.iter().fold(0, |bits, &state| bits | state_bit(state));
        // The model: for each leaf, the declaration that wins.
        let mut winners: Vec<Option<&StyleDeclaration>> = vec![None; Style::LEAVES];
        let mut by_state = [false; Style::LEAVES];
        for rule in &rules {
            for declaration in &rule.declarations {
                winners[declaration.leaf()] = Some(declaration);
                by_state[declaration.leaf()] = !rule.selector.states.is_empty();
            }
        }
        for (state, declaration) in &own {
            if state.is_none() && !by_state[declaration.leaf()] {
                winners[declaration.leaf()] = Some(declaration);
            }
        }
        for state in ORDER.iter().filter(|state| held.contains(state)) {
            for (written, declaration) in &own {
                if *written == Some(*state) {
                    winners[declaration.leaf()] = Some(declaration);
                }
            }
        }
        let mut want = default.clone();
        for declaration in winners.into_iter().flatten() {
            declaration.apply(&mut want);
        }
        ui.rules = rules;
        let selected: Vec<u32> = (0..ui.rules.len() as u32).collect();
        let own_style = OwnStyle::of(own.clone());
        let made = ui
            .compose(base, &selected, &own_style, held_bits & own_style.states())
            .0;
        assert!(
            made.same(&want),
            "case {case}: rules {:?}, own {own:?}, held {held:?}",
            ui.rules
        );
    }
}
