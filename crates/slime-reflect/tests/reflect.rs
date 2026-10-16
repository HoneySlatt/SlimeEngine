//! Reflection through types that derive it: their descriptions, their values, their paths, their
//! registry.

use slime_reflect::{
    FieldInfo, Kind, Path, Problem, Range, Reflect, Registry, RegistryError, TypeInfo, Value,
    apply, diff,
};

/// The colour of a light.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "Tint")]
struct Tint {
    r: f32,
    g: f32,
    b: f32,
}

/// A light, of one of three kinds.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Lamp", alias = "test::Lantern", version = 2)]
enum Lamp {
    /// A light from everywhere.
    Ambient,
    /// A light from a point.
    Point {
        /// Its colour.
        tint: Tint,
        /// How strong it is.
        #[reflect(range(min = 0.0), unit = "cd", alias = "power")]
        intensity: f32,
        /// How far it reaches, if it stops.
        #[reflect(range(above = 0.0), unit = "m")]
        range: Option<f32>,
    },
    /// A light in a cone.
    #[reflect(alias = "Cone")]
    Spot(#[reflect(range(min = 0.0, max = std::f32::consts::FRAC_PI_2))] f32),
}

/// A post that holds lamps.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Post")]
struct Post {
    /// How tall it is.
    #[reflect(range(min = 0.0), unit = "m")]
    height: f64,
    /// Its lamps.
    lamps: Vec<Lamp>,
    /// What it is called, or `post`.
    #[reflect(default)]
    name: String,
    /// The frame it was last lit at, which is not saved.
    #[reflect(transient)]
    lit_at: u32,
    /// What the game keeps beside it, never described.
    #[reflect(skip)]
    cache: Vec<u8>,
}

/// A marker without data.
#[derive(Debug, Default, PartialEq, Reflect)]
#[reflect(name = "test::Mark")]
struct Mark;

/// Settings that data may leave out, field by field.
#[derive(Debug, PartialEq, Reflect)]
#[reflect(name = "test::Settings", default)]
struct Settings {
    volume: f32,
    #[reflect(alias = "full_screen")]
    fullscreen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            fullscreen: true,
        }
    }
}

fn post() -> Post {
    Post {
        height: 3.5,
        lamps: vec![
            Lamp::Point {
                tint: Tint {
                    r: 1.0,
                    g: 0.8,
                    b: 0.6,
                },
                intensity: 30.0,
                range: None,
            },
            Lamp::Spot(0.4),
            Lamp::Ambient,
        ],
        name: "north".to_owned(),
        lit_at: 7,
        cache: vec![1, 2, 3],
    }
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register::<Post>();
    registry
}

fn path(text: &str) -> Path {
    text.parse().expect("a path")
}

#[test]
fn a_registered_type_lists_its_properties_with_their_kinds_and_attributes() {
    let registry = registry();
    // Its fields' types, and theirs, are registered with it.
    for name in [
        "test::Post",
        "f64",
        "Vec<test::Lamp>",
        "test::Lamp",
        "Tint",
        "f32",
        "Option<f32>",
    ] {
        assert!(registry.get(name).is_some(), "{name}");
    }
    let lamp = registry
        .get("test::Lantern")
        .expect("found by its alias")
        .info();
    assert_eq!((lamp.name.as_str(), lamp.version), ("test::Lamp", 2));
    assert_eq!(lamp.doc, "A light, of one of three kinds.");
    let Kind::Enum(variants) = &lamp.kind else {
        panic!("an enum");
    };
    let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["Ambient", "Point", "Spot"]);
    let intensity = &variants[1].fields[1];
    assert_eq!(intensity.name, "intensity");
    assert_eq!(intensity.ty, "f32");
    assert_eq!(intensity.doc, "How strong it is.");
    assert_eq!(intensity.unit.as_deref(), Some("cd"));
    assert_eq!(intensity.aliases, ["power"]);
    assert_eq!(
        intensity.range,
        Some(Range {
            min: 0.0,
            ..Range::ALL
        })
    );
    assert!(variants[1].fields[2].range.is_some_and(|r| r.min_excluded));
    let Kind::Struct(fields) = &registry.get("test::Post").expect("registered").info().kind else {
        panic!("a struct");
    };
    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["height", "lamps", "name", "lit_at"],
        "`cache` is skipped"
    );
    assert!(fields[2].default && fields[3].transient);
    // Every type a field names is registered.
    for registration in registry.iter() {
        if let Kind::Struct(fields) = &registration.info().kind {
            for field in fields {
                assert!(registry.get(&field.ty).is_some(), "{}", field.ty);
            }
        }
    }
}

#[test]
fn values_round_trip_and_skipped_fields_come_back_as_their_default() {
    let post = post();
    let value = post.to_value();
    assert_eq!(
        value.get(&path("lamps[0].intensity")),
        Some(&Value::F32(30.0))
    );
    assert_eq!(value.get(&path("lamps[1].0")), Some(&Value::F32(0.4)));
    assert_eq!(value.get(&path("cache")), None);
    let back = Post::from_value(&value).expect("read back");
    assert_eq!(
        back,
        Post {
            cache: Vec::new(),
            ..post
        }
    );
    registry()
        .check("test::Post", &value)
        .expect("a value of its type");
    assert_eq!(Mark::from_value(&Mark.to_value()), Ok(Mark));
}

#[test]
fn aliases_read_the_names_that_values_had_before() {
    let old = Value::Enum(
        "Point".to_owned(),
        vec![
            (
                "tint".to_owned(),
                Tint {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                }
                .to_value(),
            ),
            ("power".to_owned(), Value::F32(5.0)),
        ],
    );
    let Lamp::Point {
        intensity, range, ..
    } = Lamp::from_value(&old).expect("read")
    else {
        panic!("a point light");
    };
    assert_eq!((intensity, range), (5.0, None), "a missing option is None");
    let cone = Value::Enum("Cone".to_owned(), vec![("0".to_owned(), Value::F32(0.2))]);
    assert_eq!(Lamp::from_value(&cone), Ok(Lamp::Spot(0.2)));
    let settings = Value::Struct(vec![("full_screen".to_owned(), Value::Bool(false))]);
    assert_eq!(
        Settings::from_value(&settings),
        Ok(Settings {
            volume: 0.8,
            fullscreen: false
        }),
        "a missing field takes the type's default"
    );
    assert_eq!(Settings::default_value(), Some(Settings::default()));
}

#[test]
fn a_value_that_is_not_one_is_refused_with_the_path_of_the_part_at_fault() {
    let mut value = post().to_value();
    *value.get_mut(&path("lamps[0].tint.g")).expect("there") = Value::Text("green".to_owned());
    let error = Post::from_value(&value).expect_err("text in a colour");
    assert_eq!(error.path().to_string(), "lamps[0].tint.g");
    assert!(matches!(
        error.problem(),
        Problem::WrongKind { found: "text", .. }
    ));
    assert_eq!(
        error.to_string(),
        "`lamps[0].tint.g`: a value of `f32` is expected, not text"
    );
    let unknown = Value::Struct(vec![("weight".to_owned(), Value::F64(1.0))]);
    let error = Post::from_value(&unknown).expect_err("no such field");
    assert!(matches!(error.problem(), Problem::UnknownField { field, .. } if field == "weight"));
    let missing = Value::Struct(vec![("height".to_owned(), Value::F64(1.0))]);
    let error = Post::from_value(&missing).expect_err("no lamps");
    assert_eq!(*error.problem(), Problem::MissingField("lamps".to_owned()));
    let variant = Value::Enum("Torch".to_owned(), Vec::new());
    let error = Lamp::from_value(&variant).expect_err("no such variant");
    assert!(
        matches!(error.problem(), Problem::UnknownVariant { .. }),
        "{error}"
    );
}

#[test]
fn edits_set_a_part_by_path_within_its_range_and_of_its_kind() {
    let registry = registry();
    let mut value = post().to_value();
    let at = |text: &str| path(text);
    registry
        .set(
            "test::Post",
            &mut value,
            &at("lamps[0].intensity"),
            Value::F32(12.0),
        )
        .expect("in range");
    registry
        .set(
            "test::Post",
            &mut value,
            &at("lamps[0].range"),
            Value::Option(Some(Box::new(Value::F32(4.0)))),
        )
        .expect("positive");
    registry
        .set(
            "test::Post",
            &mut value,
            &at("lamps[2]"),
            Lamp::Spot(1.0).to_value(),
        )
        .expect("another variant, whole");
    let post = Post::from_value(&value).expect("read back typed");
    assert_eq!(
        post.lamps[0],
        Lamp::Point {
            tint: Tint {
                r: 1.0,
                g: 0.8,
                b: 0.6
            },
            intensity: 12.0,
            range: Some(4.0)
        }
    );
    assert_eq!(post.lamps[2], Lamp::Spot(1.0));
    let refused = [
        ("lamps[0].intensity", Value::F32(-1.0), "out of range"),
        (
            "lamps[0].range",
            Value::Option(Some(Box::new(Value::F32(0.0)))),
            "not above 0",
        ),
        ("lamps[1].0", Value::F32(2.0), "beyond π/2"),
        ("height", Value::F32(2.0), "an f32 for an f64"),
        ("lamps[0].colour", Value::F32(2.0), "no such field"),
        ("lamps[9].intensity", Value::F32(2.0), "no such lamp"),
        (
            "lamps[1].intensity",
            Value::F32(2.0),
            "a spot light has no intensity",
        ),
        (
            "lamps[2]",
            Lamp::Point {
                tint: Tint {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                },
                intensity: -3.0,
                range: None,
            }
            .to_value(),
            "a whole lamp out of range",
        ),
    ];
    for (text, new, why) in refused {
        let before = value.clone();
        let error = registry
            .set("test::Post", &mut value, &at(text), new)
            .expect_err(why);
        assert!(error.to_string().starts_with('`'), "{why}: {error}");
        assert_eq!(value, before, "{why}: the value stays as it was");
    }
    let error = registry
        .set(
            "test::Post",
            &mut value,
            &at("lamps[2]"),
            Lamp::Point {
                tint: Tint {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                },
                intensity: -3.0,
                range: None,
            }
            .to_value(),
        )
        .expect_err("out of range");
    assert_eq!(error.path().to_string(), "lamps[2].intensity");
}

#[test]
fn the_leaves_of_a_value_are_its_parts_that_hold_no_parts() {
    let registry = registry();
    let leaves = registry
        .leaves("test::Post", &post().to_value())
        .expect("a post");
    let paths: Vec<String> = leaves.iter().map(|leaf| leaf.path.to_string()).collect();
    assert_eq!(
        paths,
        [
            "height",
            "lamps[0].tint.r",
            "lamps[0].tint.g",
            "lamps[0].tint.b",
            "lamps[0].intensity",
            "lamps[0].range",
            "lamps[1].0",
            "lamps[2]",
            "name",
            "lit_at",
        ]
    );
    let intensity = &leaves[4];
    assert_eq!(intensity.ty, "f32");
    assert_eq!(
        intensity.field.as_ref().map(|f| f.name.as_str()),
        Some("intensity")
    );
}

#[test]
fn differences_between_two_values_give_one_back_from_the_other() {
    let before = post();
    let mut after = post();
    after.height = 4.0;
    after.lamps[1] = Lamp::Ambient;
    after.lamps.push(Lamp::Spot(0.1));
    let (from, to) = (before.to_value(), after.to_value());
    let changes = diff(&from, &to);
    let paths: Vec<String> = changes.iter().map(|(path, _)| path.to_string()).collect();
    assert_eq!(paths, ["height", "lamps"], "a longer list differs whole");
    let mut changed = from.clone();
    apply(&mut changed, &changes).expect("applied");
    assert_eq!(Post::from_value(&changed), Post::from_value(&to));
    assert!(diff(&to, &to).is_empty());
}

#[test]
fn a_type_without_a_rust_type_is_registered_checked_and_edited_by_its_description() {
    let mut registry = Registry::new();
    registry.register::<f32>();
    let beacon = TypeInfo {
        name: "script::Beacon".to_owned(),
        aliases: Vec::new(),
        version: 1,
        doc: "A beacon a script describes.".to_owned(),
        default: false,
        kind: Kind::Struct(vec![FieldInfo {
            range: Some(Range {
                min: 0.0,
                ..Range::ALL
            }),
            ..FieldInfo::new("brightness", "f32")
        }]),
    };
    registry
        .register_dynamic(beacon.clone())
        .expect("registered");
    let registration = registry.get("script::Beacon").expect("found");
    assert!(registration.type_id().is_none());
    assert!(registration.from_value(&Value::Struct(Vec::new())).is_err());
    let mut value = Value::Struct(vec![("brightness".to_owned(), Value::F32(1.0))]);
    registry
        .set(
            "script::Beacon",
            &mut value,
            &path("brightness"),
            Value::F32(2.0),
        )
        .expect("set");
    assert!(
        registry
            .set(
                "script::Beacon",
                &mut value,
                &path("brightness"),
                Value::F32(-2.0)
            )
            .is_err()
    );
    assert_eq!(
        registry.register_dynamic(beacon),
        Err(RegistryError::NameTaken {
            name: "script::Beacon".to_owned(),
            by: "script::Beacon".to_owned()
        })
    );
    let orphan = TypeInfo {
        name: "script::Orphan".to_owned(),
        kind: Kind::Option("script::Missing".to_owned()),
        ..registry
            .get("script::Beacon")
            .expect("found")
            .info()
            .clone()
    };
    assert!(matches!(
        registry.register_dynamic(orphan),
        Err(RegistryError::UnknownType { .. })
    ));
    assert!(registry.get("script::Orphan").is_none());
}

#[test]
#[should_panic(expected = "the type `Tint` has the name `Tint`")]
fn two_types_under_one_name_are_refused() {
    /// Another type that takes the name.
    #[derive(Reflect)]
    #[reflect(name = "Tint")]
    struct Impostor;

    let mut registry = Registry::new();
    registry.register::<Tint>();
    registry.register::<Impostor>();
}
