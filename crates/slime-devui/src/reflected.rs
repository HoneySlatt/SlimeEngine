//! What the editors of reflected values and the world panel share: what the panel calls an
//! entity, the edits set in a value, a rotation's angles, documentation as the plain text of a
//! tooltip, and the values that an option or a variant starts at
//! (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`).

use slime_reflect::{Kind, Path, Range, Reflect, Registry, Value, VariantInfo};
use slime_world::glam::{EulerRot, Quat};
use slime_world::{Entity, Name, World};

/// The types whose values are edited whole, on one line: their parts are numbers that mean little
/// alone, and a rotation's would not stay one if set one at a time.
pub(crate) const WHOLE: [&str; 4] = ["Vec3", "DVec3", "Quat", "LinearColor"];

/// What the panel calls `entity`: its name, or its handle if it has none.
pub(crate) fn name(world: &World, entity: Entity) -> String {
    match world.get::<Name>(entity) {
        Some(Name(name)) if !name.is_empty() => name.clone(),
        _ => format!("{entity:?}"),
    }
}

/// Sets each of `edits`, a part's path and its new value, in `reflected`, the value of `value`,
/// whose type `registry` describes, and `value` to the result: returns whether one was set. One
/// that the type refuses, as a number out of its field's range, is left out.
pub(crate) fn set_edits<T: Reflect>(
    registry: &Registry,
    value: &mut T,
    mut reflected: Value,
    edits: Vec<(Path, Value)>,
) -> bool {
    let ty = T::type_name();
    let mut changed = false;
    for (path, part) in edits {
        changed |= registry.set(&ty, &mut reflected, &path, part).is_ok();
    }
    match T::from_value(&reflected) {
        Ok(new) if changed => {
            *value = new;
            true
        }
        _ => false,
    }
}

/// `rotation`'s angles in degrees, turned about Y, then X, then Z.
pub(crate) fn angles(rotation: Quat) -> [f32; 3] {
    let (y, x, z) = rotation.to_euler(EulerRot::YXZ);
    [y, x, z].map(f32::to_degrees)
}

/// The rotation turned by `angles` in degrees about Y, then X, then Z.
pub(crate) fn rotation(angles: [f32; 3]) -> Quat {
    let [y, x, z] = angles.map(f32::to_radians);
    Quat::from_euler(EulerRot::YXZ, y, x, z).normalize()
}

/// `doc`, documentation as doc comments write it, in Markdown, as the plain text of a tooltip: its
/// lines joined into paragraphs, but for a list's items, each on a line of its own; a link written
/// as its text, `[text](target)` and the intra-doc link of a code span, ``[`Name`]``; and code
/// spans without their backticks.
pub(crate) fn plain(doc: &str) -> String {
    let mut joined = String::with_capacity(doc.len());
    for (index, paragraph) in doc.split("\n\n").enumerate() {
        if index > 0 {
            joined.push_str("\n\n");
        }
        for (index, line) in paragraph.lines().map(str::trim).enumerate() {
            if index > 0 {
                joined.push(if line.starts_with("- ") { '\n' } else { ' ' });
            }
            joined.push_str(line);
        }
    }
    let mut text = String::with_capacity(joined.len());
    let mut rest = joined.as_str();
    while let Some(open) = rest.find('[') {
        text.push_str(&rest[..open]);
        rest = &rest[open..];
        let Some(close) = rest.find(']') else {
            break;
        };
        let (inside, after) = (&rest[1..close], &rest[close + 1..]);
        let target = after
            .strip_prefix('(')
            .and_then(|target| target.find(')').map(|end| &target[end + 1..]));
        let code = inside.len() > 1 && inside.starts_with('`') && inside.ends_with('`');
        match target {
            Some(after_target) => {
                text.push_str(inside);
                rest = after_target;
            }
            None if code => {
                text.push_str(inside);
                rest = after;
            }
            // Brackets of prose, such as an interval.
            None => {
                text.push('[');
                rest = &rest[1..];
            }
        }
    }
    text.push_str(rest);
    text.replace('`', "")
}

/// A value of `variant`, whose fields are those of `values`, the variant that a value holds, of
/// the same name and type, and start as [`initial`] says otherwise; `None` if one cannot.
pub(crate) fn variant_value(
    registry: &Registry,
    variant: &VariantInfo,
    values: &[(String, Value)],
) -> Option<Value> {
    let fields = variant
        .fields
        .iter()
        .map(|field| {
            let kept = values
                .iter()
                .find(|(name, value)| {
                    *name == field.name && registry.check(&field.ty, value).is_ok()
                })
                .map(|(_, value)| value.clone());
            let value = kept.or_else(|| initial(registry, &field.ty, field.range))?;
            Some((field.name.clone(), value))
        })
        .collect::<Option<_>>()?;
    Some(Value::Enum(variant.name.clone(), fields))
}

/// A value of the type named `ty` to start a part at, when an option starts holding one or an enum
/// takes a variant with new fields: false, empty text, an empty list, an option that holds none,
/// the first variant that can start, and for a number the first of 0, 1 and the bounds of `range`
/// that `range` holds. `None` for a reference to an asset or an entity, which has no value to
/// start at, and for a value that holds one.
pub(crate) fn initial(registry: &Registry, ty: &str, range: Option<Range>) -> Option<Value> {
    // Each candidate made a number of the field's kind first, as the world checks it in that kind.
    let number = |of_kind: fn(f64) -> Value| {
        let range = range.unwrap_or(Range::ALL);
        [0.0, 1.0, range.min, range.max]
            .into_iter()
            .map(of_kind)
            .find(|number| number.number().is_some_and(|number| range.holds(number)))
            .unwrap_or_else(|| of_kind(0.0))
    };
    Some(match &registry.get(ty)?.info().kind {
        Kind::Bool => Value::Bool(false),
        Kind::Int { .. } => number(|number| Value::Int(number as i64)),
        Kind::UInt { .. } => number(|number| Value::UInt(number as u64)),
        Kind::F32 => number(|number| Value::F32(number as f32)),
        Kind::F64 => number(Value::F64),
        Kind::Text => Value::Text(String::new()),
        Kind::Option(_) => Value::Option(None),
        Kind::List { len: None, .. } => Value::List(Vec::new()),
        Kind::List {
            element,
            len: Some(len),
        } => Value::List(vec![initial(registry, element, range)?; *len]),
        Kind::Struct(fields) => Value::Struct(
            fields
                .iter()
                .map(|field| {
                    Some((
                        field.name.clone(),
                        initial(registry, &field.ty, field.range)?,
                    ))
                })
                .collect::<Option<_>>()?,
        ),
        Kind::Enum(variants) => variants
            .iter()
            .find_map(|variant| variant_value(registry, variant, &[]))?,
        _ => return None,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use slime_assets::{AssetId, Extra};
    use slime_color::LinearColor;
    use slime_world::glam::{DVec3, Vec3};
    use slime_world::{Component, Extras, Hidden, Light, Meshes, Transform};

    use super::*;

    #[test]
    fn a_tooltip_shows_documentation_as_plain_text() {
        let doc = "The scale of its targets, at most\n\
                   [`Gpu::max_texture_dimension`](slime_gfx::Gpu::max_texture_dimension) on a side,\n\
                   as [`View`] says.\n\nWithin [0, 1]:\n- `low`\n- [high](#high)";
        assert_eq!(
            plain(doc),
            "The scale of its targets, at most Gpu::max_texture_dimension on a side, as View \
             says.\n\nWithin [0, 1]:\n- low\n- high"
        );
    }

    /// A component that only derives, with a value of each kind the panel shows.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Everything")]
    struct Everything {
        on: bool,
        count: i32,
        small: u8,
        ratio: f64,
        /// How far it reaches.
        #[reflect(range(above = 0.0), unit = "m")]
        reach: Option<f32>,
        label: String,
        mesh: AssetId,
        target: Entity,
        offset: DVec3,
        axis: Vec3,
        turn: Quat,
        tint: Option<LinearColor>,
        mode: Mode,
        weights: Vec<f32>,
        corners: [u16; 2],
        pair: (String, Extra),
        #[reflect(transient, default)]
        seen: bool,
    }
    impl Component for Everything {}

    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Mode")]
    enum Mode {
        Off,
        Fixed(f32),
        Aimed { at: Entity, speed: f32 },
    }

    /// A world with an entity for each of the engine's components, each variant of a light, and
    /// one for a component that only derives, which refers to the first.
    pub(crate) fn world() -> (World, Vec<Entity>) {
        let mut world = World::new();
        let named = world.spawn();
        world.insert(named, Name("door".to_owned()));
        world.insert(
            named,
            Transform::from_translation(DVec3::new(1.0, 2.0, 3.0)),
        );
        world.insert(named, Hidden);
        world.insert(
            named,
            Meshes {
                ids: vec![AssetId::from_u128(7)],
                casts_shadows: true,
            },
        );
        world.insert(
            named,
            Extras(vec![
                ("open".to_owned(), Extra::Bool(true)),
                ("height".to_owned(), Extra::Number(2.5)),
                ("kind".to_owned(), Extra::Text("oak".to_owned())),
            ]),
        );
        let mut entities = vec![named];
        for light in [
            Light::Directional {
                color: LinearColor::WHITE,
                illuminance: 1.0,
                casts_shadows: true,
                shadow_distance: Some(20.0),
                in_sky: true,
            },
            Light::Point {
                color: LinearColor::WHITE,
                intensity: 30.0,
                range: None,
            },
            Light::Spot {
                color: LinearColor::WHITE,
                intensity: 30.0,
                range: Some(10.0),
                inner_cone_angle: 0.2,
                outer_cone_angle: 0.4,
                casts_shadows: false,
            },
        ] {
            let lit = world.spawn();
            world.insert(lit, light);
            entities.push(lit);
        }
        let everything = world.spawn();
        world.insert(
            everything,
            Everything {
                on: true,
                count: -3,
                small: 7,
                ratio: 0.5,
                reach: Some(4.0),
                label: "every kind".to_owned(),
                mesh: AssetId::from_u128(9),
                target: named,
                offset: DVec3::X,
                axis: Vec3::Y,
                turn: Quat::from_rotation_y(0.5),
                tint: Some(LinearColor::WHITE),
                mode: Mode::Aimed {
                    at: named,
                    speed: 2.0,
                },
                weights: vec![0.25, 0.75],
                corners: [1, 2],
                pair: ("name".to_owned(), Extra::Number(1.0)),
                seen: false,
            },
        );
        entities.push(everything);
        (world, entities)
    }

    #[test]
    fn values_start_as_values_of_their_types() {
        let (world, _) = world();
        let registry = world.registry();
        for registration in registry.iter() {
            let name = &registration.info().name;
            if let Some(value) = initial(registry, name, None) {
                registry
                    .check(name, &value)
                    .unwrap_or_else(|error| panic!("`{name}` starts at a value of it: {error}"));
            }
        }
        let starts = |name: &str| initial(registry, name, None);
        assert_eq!(starts("AssetId"), None, "no asset to refer to");
        assert_eq!(starts("test::Everything"), None, "it holds a reference");
        assert_eq!(
            starts("test::Mode"),
            Some(Value::Enum("Off".to_owned(), Vec::new()))
        );
        assert_eq!(
            starts("[u16; 2]"),
            Some(Value::List(vec![Value::UInt(0); 2]))
        );
        let above = Range {
            min: 0.0,
            min_excluded: true,
            ..Range::ALL
        };
        assert_eq!(initial(registry, "f32", Some(above)), Some(Value::F32(1.0)));
        // A bound that a 32-bit float does not hold: the float nearest it, which the range holds.
        let blend = Range {
            min: 0.7,
            max: 0.9,
            ..Range::ALL
        };
        let start = initial(registry, "f32", Some(blend)).expect("a number");
        assert_eq!(start, Value::F32(0.7));
        assert!(start.number().is_some_and(|number| blend.holds(number)));
    }

    #[test]
    fn a_new_variant_keeps_the_fields_it_shares() {
        let (mut world, entities) = world();
        let point = entities[2];
        let light = world.component_value(point, "Light").expect("held");
        let Value::Enum(_, values) = &light else {
            panic!("a light is an enum");
        };
        let Some(Kind::Enum(variants)) = world.registry().get("Light").map(|r| &r.info().kind)
        else {
            panic!("a light is an enum");
        };
        let variant = |name: &str| {
            let variant = variants.iter().find(|v| v.name == name).expect("a variant");
            variant_value(world.registry(), variant, values).expect("its fields start")
        };
        let (spot, directional) = (variant("Spot"), variant("Directional"));
        // Its two cones start at the same angle, which a spot light cannot have.
        let refused = world
            .set_property(point, "Light", &Path::default(), spot)
            .expect_err("a spot light's inner cone is narrower than its outer one");
        assert!(refused.to_string().contains("its cones"), "{refused}");
        world
            .set_property(point, "Light", &Path::default(), directional)
            .expect("a light");
        assert_eq!(
            world.get::<Light>(point),
            Some(&Light::Directional {
                color: LinearColor::WHITE,
                illuminance: 0.0,
                casts_shadows: false,
                shadow_distance: None,
                in_sky: false,
            }),
            "the color is kept"
        );
        let Some(Kind::Enum(modes)) = world.registry().get("test::Mode").map(|r| &r.info().kind)
        else {
            panic!("an enum");
        };
        assert_eq!(
            variant_value(world.registry(), &modes[1], &[]),
            Some(Value::Enum(
                "Fixed".to_owned(),
                vec![("0".to_owned(), Value::F32(0.0))]
            ))
        );
        assert_eq!(
            variant_value(world.registry(), &modes[2], &[]),
            None,
            "no entity to aim at"
        );
    }

    #[test]
    fn a_rotation_is_edited_as_three_angles() {
        let turned = rotation([30.0, 20.0, 10.0]);
        assert!(turned.is_normalized());
        for (angle, expected) in angles(turned).into_iter().zip([30.0, 20.0, 10.0]) {
            assert!((angle - expected).abs() < 1e-3, "{angle} for {expected}");
        }
        let expected = Quat::from_rotation_y(30f32.to_radians())
            * Quat::from_rotation_x(20f32.to_radians())
            * Quat::from_rotation_z(10f32.to_radians());
        assert!(
            turned.angle_between(expected) < 1e-4,
            "about Y, then X, then Z"
        );
    }
}
