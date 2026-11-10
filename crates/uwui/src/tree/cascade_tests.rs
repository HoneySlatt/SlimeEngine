//! The typed cascade checked against the reflected one that it replaced
//! (`docs/adr/0229-styles-typed-as-declarations.md`): random rules and own styles, over every leaf
//! of [`Style`] as reflection describes it, compose the same both ways, compared bit for bit, an
//! own style's arrays set by element (`docs/adr/0231-an-elements-own-style-written-in-code.md`);
//! and every leaf through the mask order, the rules' states against the own style's.
//!
//! The reflected cascade is kept here as the test's oracle while `slime-reflect` is in uwui; once
//! uwui reads its documents with `serde` (M6d.3), the fixtures of the `widgets` example's styles
//! (`examples/tests/widgets.rs`) outlive it.

use std::panic::Location;

use slime_color::LinearColor;
use slime_reflect::{Kind, Reflect, Registry, Value};

use super::Ui;
use crate::style::{
    Align, Dimension, Display, Easing, FlexDirection, Justify, OwnStyle, Same, Shadow, Style,
    StyleBuilder, StyleDeclaration, Track, state_bit,
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

/// The floats that the values take: signed zeros and a NaN among them, whose bits the cascade
/// keeps.
const FLOATS: [f32; 7] = [0.0, -0.0, 1.0, 0.5, 12.0, f32::NAN, 3.25];
const TEXTS: [&str; 3] = ["", "Geist", "tnum"];

/// A whole value of the type named `ty`, every field and element given.
fn whole(registry: &Registry, ty: &str, numbers: &mut Numbers) -> Value {
    let info = registry
        .get(ty)
        .unwrap_or_else(|| panic!("`{ty}` is registered"))
        .info();
    match &info.kind {
        Kind::Bool => Value::Bool(numbers.chance(2)),
        Kind::UInt { .. } => Value::UInt([0, 1, 2, 7][numbers.below(4)]),
        Kind::Int { .. } => Value::Int([-1, 0, 3][numbers.below(3)]),
        Kind::F32 => Value::F32(FLOATS[numbers.below(FLOATS.len())]),
        Kind::F64 => Value::F64(f64::from(FLOATS[numbers.below(FLOATS.len())])),
        Kind::Text => Value::Text(TEXTS[numbers.below(TEXTS.len())].to_owned()),
        Kind::Struct(fields) => Value::Struct(
            fields
                .iter()
                .map(|field| (field.name.clone(), whole(registry, &field.ty, numbers)))
                .collect(),
        ),
        Kind::Enum(variants) => {
            let variant = &variants[numbers.below(variants.len())];
            let fields = variant
                .fields
                .iter()
                .map(|field| (field.name.clone(), whole(registry, &field.ty, numbers)))
                .collect();
            Value::Enum(variant.name.clone(), fields)
        }
        Kind::Option(inner) => Value::Option(
            numbers
                .chance(2)
                .then(|| Box::new(whole(registry, inner, numbers))),
        ),
        Kind::List { element, len } => {
            let len = len.unwrap_or_else(|| numbers.below(3));
            Value::List(
                (0..len)
                    .map(|_| whole(registry, element, numbers))
                    .collect(),
            )
        }
        _ => panic!("a style holds no `{ty}`"),
    }
}

/// A part of a value of the type named `ty`, as a rule sets one: some of the fields of each struct
/// that the cascade merges field by field, at any depth, and whole what it sets whole, a color
/// included.
fn part(registry: &Registry, ty: &str, numbers: &mut Numbers) -> Value {
    let info = registry.get(ty).expect("registered").info();
    match &info.kind {
        Kind::Struct(fields) if ty != "LinearColor" => {
            let mut set = Vec::new();
            for field in fields {
                if numbers.chance(3) {
                    set.push((field.name.clone(), part(registry, &field.ty, numbers)));
                }
            }
            Value::Struct(set)
        }
        _ => whole(registry, ty, numbers),
    }
}

/// The style that the reflected cascade made of `parts`, the rules' in their order, then of `code`,
/// an element's own, as `Ui::compose` did before the typed cascade; but for an array's elements
/// that `code` leaves at their defaults, which keep the rules', an own style setting an array by
/// element (`docs/adr/0231-an-elements-own-style-written-in-code.md`).
fn reflected(registry: &Registry, parts: &[Value], code: &Style) -> Style {
    let mut style = reflected_whole(registry, parts, code);
    let rules = reflected_whole(registry, parts, &Style::default());
    let default = Style::default();
    let by_element = |made: &mut [f32], code: &[f32], default: &[f32], rules: &[f32]| {
        for (at, made) in made.iter_mut().enumerate() {
            if code[at].same(&default[at]) {
                *made = rules[at];
            }
        }
    };
    by_element(
        &mut style.layout.gap,
        &code.layout.gap,
        &default.layout.gap,
        &rules.layout.gap,
    );
    by_element(
        &mut style.paint.radii,
        &code.paint.radii,
        &default.paint.radii,
        &rules.paint.radii,
    );
    for (made, code, default, rules) in [
        (
            &mut style.layout.size,
            &code.layout.size,
            &default.layout.size,
            &rules.layout.size,
        ),
        (
            &mut style.layout.min_size,
            &code.layout.min_size,
            &default.layout.min_size,
            &rules.layout.min_size,
        ),
        (
            &mut style.layout.max_size,
            &code.layout.max_size,
            &default.layout.max_size,
            &rules.layout.max_size,
        ),
    ] {
        for at in 0..2 {
            if code[at].same(&default[at]) {
                made[at] = rules[at];
            }
        }
    }
    style
}

/// The style that the reflected cascade made of `parts` and `code`, every array of `code` that it
/// sets set whole.
fn reflected_whole(registry: &Registry, parts: &[Value], code: &Style) -> Style {
    if parts.is_empty() {
        return code.clone();
    }
    let default = Style::default().to_value();
    let mut value = default.clone();
    let code_value = code.to_value();
    let mut own = registry
        .difference("UiStyle", &default, &code_value, true)
        .expect("a style");
    if let Some(own) = &mut own {
        whole_colors(registry, "UiStyle", own, &code_value);
    }
    for part in parts.iter().chain(own.as_ref()) {
        registry
            .overlay("UiStyle", &mut value, part)
            .expect("a part of a style");
    }
    Style::from_value(&value).expect("a style")
}

/// Makes whole each color that `part` holds a part of, as `full` holds it: the reflected cascade's
/// own walk, which set an element's own colors whole.
fn whole_colors(registry: &Registry, ty: &str, part: &mut Value, full: &Value) {
    if ty == "LinearColor" {
        *part = full.clone();
        return;
    }
    let Some(registration) = registry.get(ty) else {
        return;
    };
    let (Kind::Struct(fields), Value::Struct(parts), Value::Struct(wholes)) =
        (&registration.info().kind, part, full)
    else {
        return;
    };
    for (name, part) in parts {
        let field = fields.iter().find(|field| field.name == *name);
        let whole = wholes.iter().find(|(whole, _)| whole == name);
        if let (Some(field), Some((_, whole))) = (field, whole) {
            whole_colors(registry, &field.ty, part, whole);
        }
    }
}

/// Whether `a` and `b` are the same bit for bit, as reflection compares them: an oracle apart from
/// the typed cascade's own `Same`.
fn same_bits(registry: &Registry, a: &Style, b: &Style) -> bool {
    registry
        .difference("UiStyle", &a.to_value(), &b.to_value(), true)
        .expect("styles")
        .is_none()
}

#[test]
fn random_rules_and_own_styles_compose_as_the_reflected_cascade_composed_them() {
    let registry = theme::registry();
    let default = Style::default().to_value();
    let mut ui = Ui::new(Fonts::new());
    let mut numbers = Numbers(0x2545_f491_4f6c_dd1d);
    for case in 0..3_000 {
        let parts: Vec<Value> = (0..numbers.below(5))
            .map(|_| part(&registry, "UiStyle", &mut numbers))
            .collect();
        // An own style at its default, or the default with some of its fields set.
        let mut own = default.clone();
        if !numbers.chance(4) {
            let set = part(&registry, "UiStyle", &mut numbers);
            registry
                .overlay("UiStyle", &mut own, &set)
                .expect("a part of a style");
        }
        let code = Style::from_value(&own).expect("a style");
        ui.rules = parts
            .iter()
            .map(|part| Rule {
                selector: Selector::default(),
                declarations: theme::declarations(&registry, &default, part).expect("a whole part"),
            })
            .collect();
        let selected: Vec<u32> = (0..parts.len() as u32).collect();
        // A container's base is the default style: it reads no color that a role gives.
        let base = ui.base_of[KindName::Container.index()];
        let own = OwnStyle::default();
        let own = {
            let mut own = own;
            own.extend(StyleBuilder::new(Location::caller()).with_style(&code));
            own
        };
        let typed = ui.compose(base, &selected, &own, 0);
        let want = reflected(&registry, &parts, &code);
        assert!(
            same_bits(&registry, &typed, &want),
            "case {case}: the rules {parts:?} and the own style {code:?} composed {typed:?}, \
             where the reflected cascade composed {want:?}"
        );
    }
}

#[test]
fn own_styles_that_differ_by_a_signed_zero_share_no_style() {
    let mut ui = Ui::new(Fonts::new());
    let sheet = theme::StyleSheet::from_text(
        "gap.style",
        "[gap]\nselect = {}\nstyle.layout.gap = [4, 4]\n",
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
            ui.compose(base, &selected, &own, held)
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
