//! The typed cascade checked against the reflected one that it replaced
//! (`docs/adr/0229-styles-typed-as-declarations.md`): random rules and own styles, over every leaf
//! of [`Style`] as reflection describes it, compose the same both ways, compared bit for bit.
//!
//! The reflected cascade is kept here as the test's oracle while `slime-reflect` is in uwui; once
//! uwui reads its documents with `serde` (M6d.3), the fixtures of the `widgets` example's styles
//! (`examples/tests/widgets.rs`) outlive it.

use slime_reflect::{Kind, Reflect, Registry, Value};

use super::Ui;
use crate::style::Style;
use crate::text::Fonts;
use crate::theme::{self, KindName, Rule, Selector};

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
/// an element's own: as `Ui::compose` did before the typed cascade.
fn reflected(registry: &Registry, parts: &[Value], code: &Style) -> Style {
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
        let typed = ui.compose(base, &selected, &code);
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
    let mut negative = Style::default();
    negative.layout.gap = [-0.0, 0.0];
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
        Style::default(),
    );
    let (mut default_box, mut negative_box) = (None, None);
    ui.build(root, 0, |children| {
        default_box = Some(children.add(
            "default",
            super::Element::container(Style::default()),
            |_| {},
        ));
        negative_box = Some(children.add(
            "negative",
            super::Element::container(negative.clone()),
            |_| {},
        ));
    });
    ui.frame();
    let gap = |id| ui.style(id).expect("styled").layout.gap;
    assert_eq!(gap(default_box.expect("built")), [4.0, 4.0]);
    // The own style's gap differs from the default's by its bits, and is set whole over the rule:
    // the two boxes share no style, as they did when styles were shared by equality.
    let negative_gap = gap(negative_box.expect("built"));
    assert!(
        negative_gap[0].to_bits() == (-0.0_f32).to_bits() && negative_gap[1].to_bits() == 0,
        "the own style's gap, set whole: {negative_gap:?}"
    );
}
