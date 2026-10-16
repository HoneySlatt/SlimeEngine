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
#[reflect(
    name = "test::Lamp",
    alias = "test::Lantern",
    version = 2,
    upgrade = upgrade_lamp
)]
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

/// Brings a lamp from version 1, at which a spot light's angle was in degrees, to version 2.
fn upgrade_lamp(version: u32, value: Value, _partial: bool) -> Result<Value, String> {
    match (version, value) {
        (1, Value::Struct(mut variants)) => {
            for (variant, angle) in &mut variants {
                // A float written without a fraction comes as an integer.
                let degrees = match &*angle {
                    Value::F64(degrees) => *degrees,
                    &Value::Int(degrees) => degrees as f64,
                    _ => continue,
                };
                if matches!(variant.as_str(), "Spot" | "Cone") {
                    *angle = Value::F64(degrees.to_radians());
                }
            }
            Ok(Value::Struct(variants))
        }
        (1, value) => Ok(value),
        (version, _) => Err(format!("no lamp had version {version}")),
    }
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
    assert!(fields[2].default.is_some() && fields[3].transient);
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
        default: None,
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

/// A cone whose inner angle must stay below its outer one, which its check says.
#[derive(Debug, PartialEq, Reflect)]
#[reflect(name = "test::Cone", check = Cone::check)]
struct Cone {
    #[reflect(range(min = 0.0, max = 1.5))]
    inner: f32,
    #[reflect(range(min = 0.0, max = 1.5))]
    outer: f32,
    /// How many wicks it holds, at most ten.
    #[reflect(range(max = 10))]
    wicks: u8,
    /// Its colour, if it is not white.
    tint: Option<Tint>,
    /// Its sizes.
    #[reflect(range(above = 0.0))]
    sizes: Vec<f32>,
    /// A variant whose name is a keyword.
    kind: Kind2,
}

impl Cone {
    fn check(&self) -> Result<(), String> {
        (self.inner < self.outer)
            .then_some(())
            .ok_or_else(|| format!("{} is not below {}", self.inner, self.outer))
    }
}

#[derive(Debug, PartialEq, Reflect)]
#[reflect(name = "test::Kind2")]
enum Kind2 {
    r#Static,
    Moving,
}

fn cone() -> Cone {
    Cone {
        inner: 0.2,
        outer: 0.6,
        wicks: 2,
        tint: Some(Tint {
            r: 1.0,
            g: 0.5,
            b: 0.25,
        }),
        sizes: vec![1.0, 2.0],
        kind: Kind2::r#Static,
    }
}

#[test]
fn a_type_s_check_refuses_what_breaks_its_rules_when_it_is_read() {
    let mut value = cone().to_value();
    assert_eq!(Cone::from_value(&value), Ok(cone()));
    *value.get_mut(&path("inner")).expect("there") = Value::F32(0.9);
    let error = Cone::from_value(&value).expect_err("wider inside");
    assert_eq!(
        *error.problem(),
        Problem::Invalid("0.9 is not below 0.6".to_owned())
    );
    // A variant whose name is a keyword keeps its name, without `r#`.
    assert_eq!(
        cone().to_value().get(&path("kind")),
        Some(&Value::Enum("Static".to_owned(), Vec::new()))
    );
}

#[test]
fn a_path_goes_through_an_option_that_holds_a_value() {
    let mut registry = Registry::new();
    registry.register::<Cone>();
    let mut value = cone().to_value();
    assert_eq!(value.get(&path("tint.g")), Some(&Value::F32(0.5)));
    registry
        .set("test::Cone", &mut value, &path("tint.g"), Value::F32(0.75))
        .expect("set through the option");
    assert_eq!(
        Cone::from_value(&value)
            .expect("read")
            .tint
            .map(|tint| tint.g),
        Some(0.75)
    );
    let leaves = registry.leaves("test::Cone", &value).expect("a cone");
    let paths: Vec<String> = leaves.iter().map(|leaf| leaf.path.to_string()).collect();
    assert_eq!(
        paths,
        [
            "inner", "outer", "wicks", "tint.r", "tint.g", "tint.b", "sizes[0]", "sizes[1]", "kind"
        ]
    );
    // An option that holds none is a leaf, and a path through it names nothing.
    registry
        .set("test::Cone", &mut value, &path("tint"), Value::Option(None))
        .expect("cleared");
    assert!(
        registry
            .set("test::Cone", &mut value, &path("tint.g"), Value::F32(0.1))
            .is_err()
    );
    let paths: Vec<String> = registry
        .leaves("test::Cone", &value)
        .expect("a cone")
        .iter()
        .map(|leaf| leaf.path.to_string())
        .collect();
    assert!(paths.contains(&"tint".to_owned()));
    // An error within an option names the path through it.
    *value.get_mut(&path("tint")).expect("there") = Value::Option(Some(Box::new(Value::Struct(
        vec![("r".to_owned(), Value::F32(1.0))],
    ))));
    let error = Cone::from_value(&value).expect_err("no green");
    assert_eq!(error.path().to_string(), "tint");
    assert_eq!(*error.problem(), Problem::MissingField("g".to_owned()));
}

#[test]
fn ranges_hold_finite_numbers_of_the_field_s_kind_and_bind_list_elements() {
    let mut registry = Registry::new();
    registry.register::<Cone>();
    let mut value = cone().to_value();
    let refused = [
        ("wicks", Value::F32(20.0), "a value of `u8`"),
        ("wicks", Value::UInt(11), "11 is not ≤ 10"),
        ("wicks", Value::UInt(300), "300 is not from 0 to 255"),
        ("sizes[1]", Value::F32(0.0), "0 is not > 0"),
        (
            "inner",
            Value::F32(f32::INFINITY),
            "inf is not a finite number",
        ),
        (
            "sizes[0]",
            Value::F32(f32::NAN),
            "NaN is not a finite number",
        ),
    ];
    for (at, new, message) in refused {
        let error = registry
            .set("test::Cone", &mut value, &path(at), new)
            .expect_err(message);
        assert!(error.to_string().contains(message), "{at}: {error}");
    }
    registry
        .set("test::Cone", &mut value, &path("sizes[1]"), Value::F32(3.0))
        .expect("positive");
}

#[test]
fn a_field_given_twice_is_refused() {
    let tint = Value::Struct(vec![
        ("r".to_owned(), Value::F32(1.0)),
        ("g".to_owned(), Value::F32(1.0)),
        ("b".to_owned(), Value::F32(1.0)),
        ("r".to_owned(), Value::F32(0.0)),
    ]);
    let error = Tint::from_value(&tint).expect_err("red twice");
    assert_eq!(*error.problem(), Problem::DuplicateField("r".to_owned()));
    let registry = registry();
    let error = registry.check("Tint", &tint).expect_err("red twice");
    assert_eq!(*error.problem(), Problem::DuplicateField("r".to_owned()));
    // A field under its name and its alias at once.
    let both = Value::Struct(vec![
        ("volume".to_owned(), Value::F32(0.5)),
        ("fullscreen".to_owned(), Value::Bool(true)),
        ("full_screen".to_owned(), Value::Bool(false)),
    ]);
    let error = Settings::from_value(&both).expect_err("given twice");
    assert_eq!(
        *error.problem(),
        Problem::DuplicateField("fullscreen".to_owned())
    );
}

#[test]
fn a_description_that_is_not_one_is_refused() {
    let mut registry = Registry::new();
    registry.register::<f32>();
    let described = |name: &str, kind: Kind| TypeInfo {
        name: name.to_owned(),
        aliases: Vec::new(),
        version: 1,
        doc: String::new(),
        default: None,
        kind,
    };
    for (info, reason) in [
        (
            described("script::Odd", Kind::Int { bits: 0 }),
            "integers of 0 bits",
        ),
        (
            described("script::Wide", Kind::UInt { bits: 128 }),
            "integers of 128 bits",
        ),
        (
            described(
                "script::Twice",
                Kind::Struct(vec![FieldInfo::new("a", "f32"), FieldInfo::new("a", "f32")]),
            ),
            "two fields are named `a`",
        ),
        (
            described(
                "script::Heavy",
                Kind::Struct(vec![FieldInfo {
                    default: Some(Value::Bool(true)),
                    ..FieldInfo::new("mass", "f32")
                }]),
            ),
            "the default of the field `mass`",
        ),
        (
            TypeInfo {
                default: Some(Value::Text("yes".to_owned())),
                ..described("script::Flag", Kind::Bool)
            },
            "its default",
        ),
        (
            described(
                "script::Dim",
                Kind::Struct(vec![FieldInfo {
                    default: Some(Value::F32(5.0)),
                    range: Some(Range {
                        min: 0.0,
                        min_excluded: false,
                        max: 1.0,
                    }),
                    ..FieldInfo::new("level", "f32")
                }]),
            ),
            "5 is not from 0 to 1",
        ),
        (
            described(
                "script::Reserved",
                Kind::Struct(vec![FieldInfo::new("$value", "f32")]),
            ),
            "`$value` starts with `$`",
        ),
    ] {
        let error = registry.register_dynamic(info).expect_err(reason);
        assert!(error.to_string().contains(reason), "{error}");
    }
    assert!(registry.get("script::Odd").is_none());
    assert!(registry.get("script::Flag").is_none(), "taken out again");
}

#[test]
#[should_panic(expected = "has the name `test::Lamp`")]
fn an_alias_that_another_type_has_is_refused() {
    /// A type whose alias is the lamp's name.
    #[derive(Reflect)]
    #[reflect(name = "test::Torch", alias = "test::Lamp")]
    struct Torch;

    let mut registry = registry();
    registry.register::<Torch>();
}

#[test]
fn unit_structs_and_empty_lists_have_no_parts_to_go_through() {
    let mut registry = Registry::new();
    registry.register::<Mark>();
    assert!(
        registry
            .leaves("test::Mark", &Mark.to_value())
            .expect("a mark")
            .is_empty()
    );
    registry.register::<Post>();
    let empty = Post {
        lamps: Vec::new(),
        ..post()
    };
    let paths: Vec<String> = registry
        .leaves("test::Post", &empty.to_value())
        .expect("a post")
        .iter()
        .map(|leaf| leaf.path.to_string())
        .collect();
    assert_eq!(paths, ["height", "lamps", "name", "lit_at"]);
}

#[test]
fn a_value_of_an_earlier_version_is_brought_to_the_type_s_through_its_upgrade() {
    let mut registry = Registry::new();
    registry.register::<Lamp>();
    let spot = |angle| Value::Struct(vec![("Spot".to_owned(), Value::F64(angle))]);
    let upgraded = registry
        .upgrade("test::Lantern", 1, spot(90.0), false)
        .expect("an upgrade from version 1, the type found by its alias");
    assert_eq!(upgraded, spot(std::f64::consts::FRAC_PI_2));
    let ambient = Value::Text("Ambient".to_owned());
    assert_eq!(
        registry.upgrade("test::Lamp", 1, ambient.clone(), false),
        Ok(ambient.clone())
    );
    assert_eq!(
        registry.upgrade("test::Lamp", 2, spot(1.0), false),
        Ok(spot(1.0)),
        "already at 2"
    );
    let problem = |version| {
        registry
            .upgrade("test::Lamp", version, ambient.clone(), false)
            .map_err(|error| error.problem().clone())
    };
    assert_eq!(
        problem(3),
        Err(Problem::NewerVersion { found: 3, known: 2 })
    );
    assert!(matches!(problem(0), Err(Problem::Upgrade { from: 0, .. })));
    // A type with no Rust type behind it has no upgrade.
    registry
        .register_dynamic(TypeInfo {
            name: "test::Bell".to_owned(),
            aliases: Vec::new(),
            version: 2,
            doc: String::new(),
            default: None,
            kind: Kind::Bool,
        })
        .expect("a description");
    let refused = registry
        .upgrade("test::Bell", 1, Value::Bool(true), false)
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "the value: cannot be brought from version 1 of its type: the type has no upgrade"
    );
}

#[test]
#[should_panic(expected = "the default of the field `scale`: 0 is not from 1 to 2")]
fn a_default_out_of_its_field_s_range_is_a_bug_in_the_type() {
    /// A size whose default breaks its own range.
    #[derive(Default, Reflect)]
    #[reflect(name = "test::Size")]
    struct Size {
        #[reflect(default, range(min = 1.0, max = 2.0))]
        scale: f32,
    }
    Registry::new().register::<Size>();
}

/// A lamp on a post, with a part that saves leave out.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Lit")]
struct Lit {
    lamp: Lamp,
    tint: Tint,
    hours: Vec<u8>,
    #[reflect(transient, default)]
    flicker: f32,
}

#[test]
fn what_changed_is_a_part_which_sets_the_changes_again() {
    let mut registry = Registry::new();
    registry.register::<Lit>();
    let point = |intensity, range| Lamp::Point {
        tint: Tint {
            r: 1.0,
            g: 0.5,
            b: 0.25,
        },
        intensity,
        range,
    };
    let lit = |lamp, g, hours: Vec<u8>, flicker| Lit {
        lamp,
        tint: Tint { r: 1.0, g, b: 0.0 },
        hours,
        flicker,
    };
    let before = lit(point(30.0, Some(5.0)), 0.5, vec![1, 2], 0.0).to_value();
    let difference = |after: &Lit, transient| {
        registry
            .difference("test::Lit", &before, &after.to_value(), transient)
            .expect("lamps")
    };
    // A field of a struct, and of the variant held: each alone; an option and a list whole.
    let after = lit(point(12.0, None), 0.75, vec![1, 2, 3], 0.0);
    let part = difference(&after, false).expect("a change");
    assert_eq!(
        part,
        Value::Struct(vec![
            (
                "lamp".to_owned(),
                Value::Enum(
                    "Point".to_owned(),
                    vec![
                        ("intensity".to_owned(), Value::F32(12.0)),
                        ("range".to_owned(), Value::Option(None)),
                    ]
                )
            ),
            (
                "tint".to_owned(),
                Value::Struct(vec![("g".to_owned(), Value::F32(0.75))])
            ),
            (
                "hours".to_owned(),
                Value::List(vec![Value::UInt(1), Value::UInt(2), Value::UInt(3)])
            ),
        ])
    );
    let mut value = before.clone();
    registry
        .overlay("test::Lit", &mut value, &part)
        .expect("a part of a lit lamp");
    assert_eq!(Lit::from_value(&value), Ok(after));
    // Another variant, whole; a transient field, left out unless asked for.
    let ambient = lit(Lamp::Ambient, 0.5, vec![1, 2], 0.9);
    assert_eq!(
        difference(&ambient, false),
        Some(Value::Struct(vec![(
            "lamp".to_owned(),
            Value::Enum("Ambient".to_owned(), Vec::new())
        )]))
    );
    let flickering = lit(point(30.0, Some(5.0)), 0.5, vec![1, 2], 0.9);
    assert_eq!(
        difference(&flickering, false),
        None,
        "only what saves leave out"
    );
    assert!(difference(&flickering, true).is_some());
    // A part that the type refuses: a number out of its range, a variant missing fields.
    let mut value = before.clone();
    let out_of_range = Value::Struct(vec![(
        "lamp".to_owned(),
        Value::Enum(
            "Point".to_owned(),
            vec![("intensity".to_owned(), Value::F32(-1.0))],
        ),
    )]);
    let refused = registry
        .overlay("test::Lit", &mut value, &out_of_range)
        .unwrap_err();
    assert_eq!(refused.to_string(), "`lamp.intensity`: -1 is not ≥ 0");
    let half = Value::Struct(vec![(
        "lamp".to_owned(),
        Value::Enum("Spot".to_owned(), Vec::new()),
    )]);
    assert!(registry.overlay("test::Lit", &mut value, &half).is_err());
}
