//! Values written as documents and read back: every registered type, floats to their bits, earlier
//! versions, aliases, and documents that are not ones.

use glam::{DVec3, Quat, Vec3};
use slime_reflect::{Kind, Problem, Range, Reflect, Registry, TypeInfo, Value, diff};
use slime_serialize::{
    Composer, Document, MAX_DEPTH, MAX_SIZE, Mode, ReadProblem, References, read, read_file,
    read_partial, write, write_partial,
};

/// A colour.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Tint")]
struct Tint {
    r: f32,
    g: f32,
    b: f32,
}

/// What a lamp does, with fields or without.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Mode")]
enum Mode2 {
    Off,
    #[reflect(alias = "Steady")]
    Fixed(f32),
    Blinking {
        period: f32,
        tint: Tint,
    },
}

/// A tuple struct of one field, written as that field.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Label")]
struct Label(String);

/// A unit struct.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Marker")]
struct Marker;

/// A table larger than one line, in an array.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Spot")]
struct Spot {
    at: Vec3,
    mode: Mode2,
}

/// A value of every kind that documents hold.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Everything")]
struct Everything {
    on: bool,
    small: i8,
    count: i32,
    big: i64,
    byte: u8,
    huge: u64,
    ratio: f32,
    precise: f64,
    text: String,
    reach: Option<f32>,
    nothing: Option<f32>,
    nested: Option<Option<u8>>,
    maybe: Vec<Option<u16>>,
    offset: DVec3,
    axis: Vec3,
    turn: Quat,
    tint: Tint,
    mode: Mode2,
    modes: Vec<Mode2>,
    weights: Vec<f32>,
    corners: [u16; 2],
    pair: (String, f64),
    label: Label,
    marker: Marker,
    spots: Vec<Spot>,
    #[reflect(range(min = 0.0, max = 1.0), unit = "m")]
    bounded: f32,
}

fn everything() -> Everything {
    Everything {
        on: true,
        small: -128,
        count: -3,
        big: i64::MIN,
        byte: 255,
        huge: u64::MAX,
        ratio: 0.1,
        precise: 1e-7,
        text: "a \"quoted\" \\ line\nand é, \u{1}".to_owned(),
        reach: Some(4.5),
        nothing: None,
        nested: Some(None),
        maybe: vec![Some(3), None],
        offset: DVec3::new(1.0, -2.5, 1e20),
        axis: Vec3::Y,
        turn: Quat::from_rotation_y(0.5),
        tint: Tint {
            r: 1.0,
            g: 0.66,
            b: 0.36,
        },
        mode: Mode2::Blinking {
            period: 0.5,
            tint: Tint {
                r: 0.0,
                g: 1.0,
                b: 0.0,
            },
        },
        modes: vec![Mode2::Off, Mode2::Fixed(2.0)],
        weights: vec![0.25, 0.75],
        corners: [1, 2],
        pair: ("ratio".to_owned(), 0.5),
        label: Label("door".to_owned()),
        marker: Marker,
        spots: vec![
            Spot {
                at: Vec3::ZERO,
                mode: Mode2::Off,
            },
            Spot {
                at: Vec3::X,
                mode: Mode2::Fixed(1.0),
            },
        ],
        bounded: 0.5,
    }
}

/// A registry of the test's types, and of one of references to assets, which no Rust type of
/// these tests is.
fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register::<Everything>();
    registry
        .register_dynamic(TypeInfo {
            name: "test::Mesh".to_owned(),
            aliases: Vec::new(),
            version: 1,
            doc: String::new(),
            default: None,
            kind: Kind::Asset,
        })
        .expect("a description");
    registry
}

/// Writes `value` of `ty`, reads it back, and writes it again: the same value, floats by their
/// bits, and the same bytes. Returns the text.
fn round_trip(registry: &Registry, ty: &str, value: &Value) -> String {
    let text = write(registry, ty, value).unwrap_or_else(|error| panic!("{ty}: {error}"));
    assert!(!text.contains('\r'), "{ty}: LF line endings");
    let read = read(registry, ty, &text, Mode::Strict)
        .unwrap_or_else(|error| panic!("{ty}: {error}\n{text}"));
    assert!(read.warnings.is_empty());
    assert!(
        diff(value, &read.value).is_empty(),
        "{ty}: {value:?} came back as {:?}\n{text}",
        read.value
    );
    let again = write(registry, ty, &read.value).expect("the value read");
    assert_eq!(again, text, "{ty}: the same bytes");
    text
}

/// Values of the type named `ty`: of each variant, an option empty and holding one, a list empty
/// and holding two, numbers at their extremes and within the range given.
fn samples(registry: &Registry, ty: &str, range: Option<Range>) -> Vec<Value> {
    let info = registry.get(ty).expect("registered").info();
    let numbers: Vec<f64> = match range {
        Some(range) => [range.min, range.max, 0.0, 1.0]
            .into_iter()
            .filter(|&number| range.contains(number))
            .collect(),
        None => vec![0.0, -1.5, 1.0e300],
    };
    match &info.kind {
        Kind::Bool => vec![Value::Bool(true), Value::Bool(false)],
        Kind::Int { bits } => {
            let max = (1i128 << (bits - 1)) - 1;
            vec![
                Value::Int((-max - 1) as i64),
                Value::Int(max as i64),
                Value::Int(0),
            ]
        }
        Kind::UInt { bits } => {
            let max = (1u128 << bits) - 1;
            vec![Value::UInt(max as u64), Value::UInt(0)]
        }
        Kind::F32 => numbers.iter().map(|&n| Value::F32(n as f32)).collect(),
        Kind::F64 => numbers.iter().map(|&n| Value::F64(n)).collect(),
        Kind::Text => vec![
            Value::Text(String::new()),
            Value::Text("a \"b\"\n".to_owned()),
        ],
        Kind::Asset => vec![Value::Asset(0x850a_d551_5098_42f9_ae42_26ed_6174_951a)],
        Kind::Entity => Vec::new(),
        Kind::Option(inner) => {
            let mut values = vec![Value::Option(None)];
            values.extend(
                samples(registry, inner, range)
                    .into_iter()
                    .take(1)
                    .map(|value| Value::Option(Some(Box::new(value)))),
            );
            values
        }
        Kind::List { element, len } => {
            let one = samples(registry, element, range);
            let count = len.unwrap_or(2);
            let mut values = vec![Value::List(vec![one[0].clone(); count])];
            if len.is_none() {
                values.push(Value::List(Vec::new()));
            }
            values
        }
        Kind::Struct(fields) => vec![Value::Struct(
            fields
                .iter()
                .map(|field| {
                    let value = samples(registry, &field.ty, field.range).swap_remove(0);
                    (field.name.clone(), value)
                })
                .collect(),
        )],
        Kind::Enum(variants) => variants
            .iter()
            .map(|variant| {
                Value::Enum(
                    variant.name.clone(),
                    variant
                        .fields
                        .iter()
                        .map(|field| {
                            let value = samples(registry, &field.ty, field.range).swap_remove(0);
                            (field.name.clone(), value)
                        })
                        .collect(),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[test]
fn every_registered_type_round_trips() {
    let registry = registry();
    let mut seen = 0;
    for registration in registry.iter() {
        let ty = &registration.info().name;
        for value in samples(&registry, ty, None) {
            round_trip(&registry, ty, &value);
            seen += 1;
        }
    }
    assert!(seen > 40, "{seen} values");
    round_trip(&registry, "test::Everything", &everything().to_value());
}

#[test]
fn a_value_is_written_as_the_format_says() {
    let registry = registry();
    let text = round_trip(&registry, "test::Everything", &everything().to_value());
    let expected = r#"on = true
small = -128
count = -3
big = -9223372036854775808
byte = 255
huge = "18446744073709551615"
ratio = 0.1
precise = 1e-7
text = "a \"quoted\" \\ line\nand é, \u0001"
reach = 4.5
nested = []
maybe = [[3], []]
offset = { x = 1.0, y = -2.5, z = 1e20 }
axis = { x = 0.0, y = 1.0, z = 0.0 }
turn = { x = 0.0, y = 0.24740396, z = 0.0, w = 0.9689124 }
tint = { r = 1.0, g = 0.66, b = 0.36 }
modes = ["Off", { Fixed = 2.0 }]
weights = [0.25, 0.75]
corners = [1, 2]
pair = ["ratio", 0.5]
label = "door"
marker = {}
bounded = 0.5

[mode.Blinking]
period = 0.5
tint = { r = 0.0, g = 1.0, b = 0.0 }

[[spots]]
at = { x = 0.0, y = 0.0, z = 0.0 }
mode = "Off"

[[spots]]
at = { x = 1.0, y = 0.0, z = 0.0 }
mode = { Fixed = 1.0 }
"#;
    assert_eq!(text, expected);
}

#[test]
fn floats_come_back_to_the_same_bits() {
    let registry = registry();
    let doubles = [
        0.0,
        -0.0,
        0.1,
        1.0 / 3.0,
        f64::MIN_POSITIVE,
        5e-324,
        f64::MAX,
        f64::MIN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
        1e15,
        1e16,
        123_456_789.123_456_78,
    ];
    for number in doubles {
        round_trip(&registry, "f64", &Value::F64(number));
        round_trip(&registry, "f32", &Value::F32(number as f32));
    }
    // 7.038531e-26 read as an f64 then rounded to 32 bits would be its neighbour.
    let twice_rounded = f32::from_bits(0x15AE_43FD);
    for number in [
        f32::MIN_POSITIVE,
        1e-45,
        f32::MAX,
        16_777_217.0,
        0.3,
        twice_rounded,
        -twice_rounded,
    ] {
        round_trip(&registry, "f32", &Value::F32(number));
    }
}

/// A lamp at version 2: version 1 called its power `watts`, and held its colour as an array.
#[derive(Debug, PartialEq, Reflect)]
#[reflect(name = "test::Lamp", alias = "test::Lantern", version = 2, upgrade = upgrade_lamp)]
struct Lamp {
    power: f32,
    tint: Tint,
    lit: bool,
}

/// Brings a lamp from version 1: `watts` becomes `power`, and an array `tint` a table.
fn upgrade_lamp(version: u32, value: Value, _partial: bool) -> Result<Value, String> {
    let (1, Value::Struct(fields)) = (version, value) else {
        return Err(format!("no lamp of version {version} was a table"));
    };
    let fields = fields
        .into_iter()
        .map(|(name, value)| match (name.as_str(), value) {
            ("watts", value) => Ok(("power".to_owned(), value)),
            ("tint", Value::List(channels)) => {
                let [r, g, b]: [Value; 3] = channels
                    .try_into()
                    .map_err(|_| "a tint of three channels".to_owned())?;
                let channels = vec![
                    ("r".to_owned(), r),
                    ("g".to_owned(), g),
                    ("b".to_owned(), b),
                ];
                Ok(("tint".to_owned(), Value::Struct(channels)))
            }
            (_, value) => Ok((name, value)),
        })
        .collect::<Result<_, String>>()?;
    Ok(Value::Struct(fields))
}

fn lamps() -> Registry {
    let mut registry = Registry::new();
    registry.register::<Lamp>();
    registry
}

#[test]
fn a_value_of_an_earlier_version_is_read_through_its_upgrade() {
    let registry = lamps();
    let lamp = Lamp {
        power: 60.0,
        tint: Tint {
            r: 1.0,
            g: 0.5,
            b: 0.25,
        },
        lit: true,
    };
    // Version 1 wrote no version: a type that a document does not name is at version 1.
    let earlier = "watts = 60.0\ntint = [1.0, 0.5, 0.25]\nlit = true\n";
    let upgraded = read(&registry, "test::Lamp", earlier, Mode::Strict).expect("upgraded");
    assert_eq!(Lamp::from_value(&upgraded.value), Ok(lamp));
    let partial = read_partial(&registry, "test::Lamp", "watts = 75\n", Mode::Strict)
        .expect("a part upgraded");
    assert_eq!(
        partial.value,
        Value::Struct(vec![("power".to_owned(), Value::F32(75.0))])
    );
    let text = write(&registry, "test::Lamp", &upgraded.value).expect("a lamp");
    assert!(text.starts_with("\"$versions\" = { \"test::Lamp\" = 2 }\npower = 60.0\n"));
    // The version under the type's former name.
    let aliased = text.replace("test::Lamp", "test::Lantern");
    let again = read(&registry, "test::Lamp", &aliased, Mode::Strict).expect("by its alias");
    assert!(diff(&again.value, &upgraded.value).is_empty());
}

#[test]
fn later_versions_are_refused() {
    let registry = lamps();
    let newer = "\"$versions\" = { \"test::Lamp\" = 3 }\npower = 1.0\n";
    let error = read(&registry, "test::Lamp", newer, Mode::Lenient).expect_err("version 3");
    assert!(matches!(
        error.problem(),
        ReadProblem::Value(Problem::NewerVersion { found: 3, known: 2 })
    ));
    assert_eq!(error.line_and_column(), Some((1, 32)));
    let format = "\"$format\" = 2\npower = 1.0\n";
    let error = read(&registry, "test::Lamp", format, Mode::Lenient).expect_err("format 2");
    assert!(matches!(
        error.problem(),
        ReadProblem::NewerFormat { found: 2, known: 1 }
    ));
}

#[test]
fn aliases_of_fields_and_variants_read() {
    let registry = registry();
    let read = read(&registry, "test::Mode", "Steady = 3.0", Mode::Strict).expect("an alias");
    assert_eq!(
        read.value,
        Value::Enum("Fixed".to_owned(), vec![("0".to_owned(), Value::F32(3.0))])
    );
}

#[test]
fn a_field_that_the_type_does_not_have_is_an_error_in_a_source_and_a_warning_in_a_save() {
    let registry = registry();
    let text = "r = 1.0\ng = 0.5\nglow = true\nb = 0.25\n";
    let error = read(&registry, "test::Tint", text, Mode::Strict).expect_err("strict");
    assert_eq!(error.line_and_column(), Some((3, 1)));
    assert_eq!(
        error.to_string(),
        "line 3, column 1: no field `glow`, where there are `r`, `g` and `b`"
    );
    let read = read(&registry, "test::Tint", text, Mode::Lenient).expect("lenient");
    assert_eq!(read.warnings.len(), 1);
    assert_eq!(
        Tint::from_value(&read.value),
        Ok(Tint {
            r: 1.0,
            g: 0.5,
            b: 0.25
        })
    );
}

#[test]
fn errors_say_where_the_value_is_wrong() {
    let registry = registry();
    let cases = [
        (
            "r = 1.0\ng = \"green\"\nb = 0.0\n",
            "test::Tint",
            (2, 5),
            "`g`: a value of `f32` is expected, not text",
        ),
        (
            "r = 1.0\nb = 0.0\n",
            "test::Tint",
            (1, 1),
            "the field `g` is missing",
        ),
        (
            "Fixed = 1.0\nOff = 2.0\n",
            "test::Mode",
            (1, 1),
            "a value of `test::Mode` is expected, not a table",
        ),
        (
            "\"$value\" = \"Dim\"",
            "test::Mode",
            (1, 12),
            "no variant `Dim`, where there are",
        ),
        (
            "\"$value\" = 300",
            "u8",
            (1, 12),
            "300 is not from 0 to 255",
        ),
        (
            "\"$value\" = -1",
            "u64",
            (1, 12),
            "a value of `u64` is expected, not an integer",
        ),
        (
            "\"$value\" = \"zz\"",
            "test::Mesh",
            (1, 12),
            "`zz` is not an asset ID",
        ),
        (
            "\"$value\" = 1979-05-27",
            "bool",
            (1, 12),
            "a date or a time",
        ),
        (
            "r = 1.0\ng = 0.5\nb = 0.25\nr = 2.0\n",
            "test::Tint",
            (4, 1),
            "not TOML",
        ),
    ];
    for (text, ty, at, message) in cases {
        let error = read(&registry, ty, text, Mode::Strict).expect_err(text);
        assert_eq!(error.line_and_column(), Some(at), "{text}: {error}");
        assert!(error.to_string().contains(message), "{text}: {error}");
    }
    let ranged = everything_text().replace("bounded = 0.5", "bounded = 1.5");
    let error = read(&registry, "test::Everything", &ranged, Mode::Strict).expect_err("range");
    assert_eq!(error.path().to_string(), "bounded");
    assert!(
        error.to_string().contains("1.5 is not from 0 to 1"),
        "{error}"
    );
}

fn everything_text() -> String {
    write(&registry(), "test::Everything", &everything().to_value()).expect("written")
}

#[test]
fn references_to_entities_are_neither_written_nor_read() {
    let mut registry = Registry::new();
    registry
        .register_dynamic(TypeInfo {
            name: "test::Target".to_owned(),
            aliases: Vec::new(),
            version: 1,
            doc: String::new(),
            default: None,
            kind: Kind::Entity,
        })
        .expect("a description");
    let error = write(&registry, "test::Target", &Value::Entity(1 << 32)).expect_err("written");
    assert!(
        error.to_string().contains("handle holds within one run"),
        "{error}"
    );
    assert!(
        read(
            &registry,
            "test::Target",
            "\"$value\" = 4294967296",
            Mode::Lenient
        )
        .is_err()
    );
}

#[test]
fn documents_that_are_not_ones_give_errors_and_never_panic() {
    let registry = registry();
    let text = everything_text();
    let boundaries: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
    for &end in &boundaries {
        let _ = read(&registry, "test::Everything", &text[..end], Mode::Strict);
        let _ = read_partial(&registry, "test::Everything", &text[..end], Mode::Lenient);
    }
    for &at in &boundaries {
        for replacement in ["\"", "[", "{", "=", "\n", "x", "9", ".", "}", "]", "-"] {
            let mut altered = text.clone();
            let end = at + text[at..].chars().next().map_or(0, char::len_utf8);
            altered.replace_range(at..end, replacement);
            let _ = read(&registry, "test::Everything", &altered, Mode::Strict);
        }
    }
    let nested = |depth| format!("a = {}{}", "[".repeat(depth), "]".repeat(depth));
    for depth in [MAX_DEPTH + 4, 200, 10_000] {
        let error = read(&registry, "test::Tint", &nested(depth), Mode::Strict).expect_err("deep");
        assert!(
            matches!(
                error.problem(),
                ReadProblem::TooDeep { .. } | ReadProblem::Syntax(_)
            ),
            "{depth}: {error}"
        );
    }
    let error = read(
        &registry,
        "test::Tint",
        &nested(MAX_DEPTH + 4),
        Mode::Strict,
    )
    .unwrap_err();
    assert!(matches!(
        error.problem(),
        ReadProblem::TooDeep { limit: MAX_DEPTH }
    ));
    let large = " ".repeat(MAX_SIZE + 1);
    let error = read(&registry, "test::Tint", &large, Mode::Strict).expect_err("too large");
    assert!(matches!(error.problem(), ReadProblem::TooLarge { .. }));
}

#[test]
fn files_are_named_in_their_errors() {
    let directory = std::env::temp_dir().join(format!("slime-serialize-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("a directory of the tests");
    let registry = registry();
    let file = directory.join("tint.toml");
    std::fs::write(&file, "r = 1.0\ng = 0.5\nb = true\n").expect("written");
    let error = read_file(&registry, "test::Tint", &file, Mode::Strict).expect_err("a boolean");
    assert_eq!(error.file(), Some(file.as_path()));
    assert!(
        error
            .to_string()
            .ends_with("tint.toml:3:5: `b`: a value of `f32` is expected, not a boolean"),
        "{error}"
    );
    let large = directory.join("large.toml");
    std::fs::File::create(&large)
        .and_then(|created| created.set_len(MAX_SIZE as u64 + 1))
        .expect("a file larger than the limit");
    let error = read_file(&registry, "test::Tint", &large, Mode::Strict).expect_err("large");
    assert!(matches!(error.problem(), ReadProblem::TooLarge { .. }));
    let missing = read_file(
        &registry,
        "test::Tint",
        &directory.join("none"),
        Mode::Strict,
    )
    .expect_err("no file");
    assert!(matches!(missing.problem(), ReadProblem::Io(_)));
    std::fs::remove_dir_all(&directory).expect("the tests' files removed");
}

#[test]
fn a_field_left_out_takes_its_default() {
    /// Settings, of which data may leave anything out.
    #[derive(Debug, Default, PartialEq, Reflect)]
    #[reflect(name = "test::Settings", default)]
    struct Settings {
        volume: f32,
        #[reflect(default)]
        muted: bool,
        name: Option<String>,
    }
    let mut registry = Registry::new();
    registry.register::<Settings>();
    let read = read(&registry, "test::Settings", "", Mode::Strict).expect("defaults");
    assert_eq!(Settings::from_value(&read.value), Ok(Settings::default()));
    registry
        .check("test::Settings", &read.value)
        .expect("a whole value");
}

#[test]
fn an_option_left_out_holds_none_whatever_its_default() {
    /// Settings whose shadows are on by default.
    #[derive(Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Shadowed", default)]
    struct Shadowed {
        shadow: Option<f32>,
    }
    impl Default for Shadowed {
        fn default() -> Self {
            Self { shadow: Some(2.0) }
        }
    }
    let mut registry = Registry::new();
    registry.register::<Shadowed>();
    let none = Shadowed { shadow: None };
    let text = round_trip(&registry, "test::Shadowed", &none.to_value());
    assert_eq!(text, "");
}

#[test]
fn what_cannot_be_read_back_is_not_written() {
    let registry = registry();
    let mut wrong = everything();
    wrong.bounded = 5.0;
    let error = write(&registry, "test::Everything", &wrong.to_value()).expect_err("range");
    assert_eq!(error.to_string(), "`bounded`: 5 is not from 0 to 1");
    wrong.bounded = f32::NAN;
    let error = write(&registry, "test::Everything", &wrong.to_value()).expect_err("NaN");
    assert!(error.to_string().contains("not a finite number"), "{error}");
    let error = write(&registry, "u8", &Value::UInt(1000)).expect_err("width");
    assert!(
        error.to_string().contains("1000 is not from 0 to 255"),
        "{error}"
    );
    let short = Value::List(vec![Value::UInt(1)]);
    let error = write(&registry, "[u16; 2]", &short).expect_err("length");
    assert!(
        error.to_string().contains("1 elements, where there are 2"),
        "{error}"
    );
    let missing = Value::Struct(vec![("r".to_owned(), Value::F32(1.0))]);
    let error = write(&registry, "test::Tint", &missing).expect_err("a part only");
    assert!(
        error.to_string().contains("the field `g` is missing"),
        "{error}"
    );
}

#[test]
fn a_part_of_a_value_is_written_and_read_back() {
    let registry = registry();
    let part = Value::Struct(vec![
        ("count".to_owned(), Value::Int(7)),
        (
            "tint".to_owned(),
            Value::Struct(vec![("g".to_owned(), Value::F32(0.25))]),
        ),
    ]);
    let text = write_partial(&registry, "test::Everything", &part).expect("a part");
    assert_eq!(text, "count = 7\ntint = { g = 0.25 }\n");
    let read = read_partial(&registry, "test::Everything", &text, Mode::Strict).expect("read");
    assert!(diff(&part, &read.value).is_empty());
    // A tuple is written whole.
    let half = Value::Struct(vec![(
        "pair".to_owned(),
        Value::Struct(vec![("0".to_owned(), Value::Text("a".to_owned()))]),
    )]);
    let error = write_partial(&registry, "test::Everything", &half).expect_err("half a tuple");
    assert!(
        error.to_string().contains("the field `1` is missing"),
        "{error}"
    );
}

/// A light at version 3: version 1 had no `scale`, which is 1 for its lights, and version 2 called
/// `power` `watts`.
#[derive(Debug, PartialEq, Reflect)]
#[reflect(name = "test::Glow", version = 3, upgrade = upgrade_glow)]
struct Glow {
    power: f32,
    scale: f32,
}

/// Brings a glow forward; a whole one of version 1 gains its `scale`, a part does not.
fn upgrade_glow(version: u32, value: Value, partial: bool) -> Result<Value, String> {
    let Value::Struct(mut fields) = value else {
        return Err("a glow is a table".to_owned());
    };
    match version {
        1 if !partial => fields.push(("scale".to_owned(), Value::F64(1.0))),
        1 => {}
        2 => {
            for (name, _) in &mut fields {
                if name == "watts" {
                    *name = "power".to_owned();
                }
            }
        }
        _ => return Err(format!("no glow had version {version}")),
    }
    Ok(Value::Struct(fields))
}

/// A post that holds glows, at version 2: version 1 called them `lights`.
#[derive(Debug, PartialEq, Reflect)]
#[reflect(name = "test::Post", version = 2, upgrade = upgrade_post)]
struct Post {
    #[reflect(alias = "lamps")]
    glows: Vec<Glow>,
}

fn upgrade_post(version: u32, value: Value, _partial: bool) -> Result<Value, String> {
    let (1, Value::Struct(fields)) = (version, value) else {
        return Err(format!("no post of version {version} was a table"));
    };
    Ok(Value::Struct(
        fields
            .into_iter()
            .map(|(name, value)| match name.as_str() {
                "lights" => ("glows".to_owned(), value),
                _ => (name, value),
            })
            .collect(),
    ))
}

#[test]
fn upgrades_bring_nested_values_forward_whole_or_in_part() {
    let mut registry = Registry::new();
    registry.register::<Post>();
    let earlier = "[[lights]]\nwatts = 5\n\n[[lights]]\nwatts = 7.5\n";
    let text = format!("\"$versions\" = {{ \"test::Glow\" = 2 }}\n{earlier}");
    let error = read(&registry, "test::Post", &text, Mode::Strict).expect_err("no scale at 2");
    assert!(
        error.to_string().contains("the field `scale` is missing"),
        "{error}"
    );
    // Version 1 of both, the glows by their earlier name.
    let post = read(&registry, "test::Post", earlier, Mode::Strict).expect("upgraded");
    assert_eq!(
        Post::from_value(&post.value),
        Ok(Post {
            glows: vec![
                Glow {
                    power: 5.0,
                    scale: 1.0
                },
                Glow {
                    power: 7.5,
                    scale: 1.0
                }
            ]
        })
    );
    let part = read_partial(&registry, "test::Glow", "watts = 2.0\n", Mode::Strict)
        .expect("a part, upgraded without the field it lacks");
    assert_eq!(
        part.value,
        Value::Struct(vec![("power".to_owned(), Value::F32(2.0))])
    );
    // A version under an alias of the post, and a field by its alias.
    let aliased = "\"$versions\" = { \"test::Post\" = 2, \"test::Glow\" = 3 }\n[[lamps]]\npower = 1.0\nscale = 2.0\n";
    let by_alias = read(&registry, "test::Post", aliased, Mode::Strict).expect("by its alias");
    assert_eq!(
        Post::from_value(&by_alias.value),
        Ok(Post {
            glows: vec![Glow {
                power: 1.0,
                scale: 2.0
            }]
        })
    );
}

#[test]
fn a_type_that_a_save_names_and_the_program_does_not_know_is_a_warning() {
    let registry = registry();
    let text = "\"$versions\" = { \"test::Gone\" = 2 }\nr = 1.0\ng = 0.5\nb = 0.25\n";
    assert!(read(&registry, "test::Tint", text, Mode::Strict).is_err());
    let read = read(&registry, "test::Tint", text, Mode::Lenient).expect("lenient");
    assert_eq!(read.warnings.len(), 1);
    assert!(
        read.warnings[0]
            .to_string()
            .contains("no type `test::Gone`"),
        "{}",
        read.warnings[0]
    );
}

#[test]
fn a_file_that_is_not_text_is_refused_where_it_stops_being_text() {
    let directory =
        std::env::temp_dir().join(format!("slime-serialize-text-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("a directory of the tests");
    let file = directory.join("binary.toml");
    std::fs::write(&file, b"r = 1.0\ng = \xff\n").expect("written");
    let error = read_file(&registry(), "test::Tint", &file, Mode::Strict).expect_err("binary");
    assert!(matches!(error.problem(), ReadProblem::NotText));
    assert_eq!(error.line_and_column(), Some((2, 5)));
    std::fs::remove_dir_all(&directory).expect("the tests' files removed");
}

#[test]
fn a_part_says_that_an_option_was_emptied() {
    let registry = registry();
    let part = Value::Struct(vec![
        ("reach".to_owned(), Value::Option(None)),
        (
            "nested".to_owned(),
            Value::Option(Some(Box::new(Value::Option(Some(Box::new(Value::UInt(
                3,
            ))))))),
        ),
    ]);
    let text = write_partial(&registry, "test::Everything", &part).expect("a part");
    assert_eq!(text, "reach = []\nnested = [[3]]\n");
    let read = read_partial(&registry, "test::Everything", &text, Mode::Strict).expect("read");
    assert!(diff(&part, &read.value).is_empty(), "{:?}", read.value);
    let set = read_partial(&registry, "test::Everything", "reach = [2.5]", Mode::Strict)
        .expect("an option given a value");
    assert_eq!(
        set.value,
        Value::Struct(vec![(
            "reach".to_owned(),
            Value::Option(Some(Box::new(Value::F32(2.5))))
        )])
    );
}

/// A lamp that hangs from an entity, and flickers in a way that saves leave out.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Hanging")]
struct Hanging {
    from: Option<slime_reflect_entity::Handle>,
    #[reflect(transient, default)]
    flicker: f32,
}

/// Entity handles for the tests: a reference to an entity is a kind of value of its own, which
/// only a world's handles describe; this one does too.
mod slime_reflect_entity {
    use slime_reflect::{Kind, Reflect, ReflectError, Registry, TypeInfo, Value};

    /// The bits of an entity's handle.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Handle(pub u64);

    impl Reflect for Handle {
        fn type_name() -> String {
            "Entity".to_owned()
        }

        fn type_info() -> TypeInfo {
            TypeInfo {
                name: Self::type_name(),
                aliases: Vec::new(),
                version: 1,
                doc: String::new(),
                default: None,
                kind: Kind::Entity,
            }
        }

        fn register_dependencies(_: &mut Registry) {}

        fn to_value(&self) -> Value {
            Value::Entity(self.0)
        }

        fn from_value(value: &Value) -> Result<Self, ReflectError> {
            match value {
                Value::Entity(bits) => Ok(Self(*bits)),
                other => Err(slime_reflect::support::wrong("Entity", other)),
            }
        }
    }
}

/// Names two entities, `porch` and `gate`, whose handles have the bits 1 and 2.
struct Porch;

impl References for Porch {
    fn name(&self, entity: u64) -> Option<String> {
        match entity {
            1 => Some("porch".to_owned()),
            2 => Some("gate".to_owned()),
            _ => None,
        }
    }

    fn entity(&self, name: &str) -> Option<u64> {
        match name {
            "porch" => Some(1),
            "gate" => Some(2),
            _ => None,
        }
    }
}

#[test]
fn a_document_of_several_values_names_entities_as_its_references_do() {
    use slime_reflect_entity::Handle;

    let mut registry = Registry::new();
    registry.register::<Hanging>();
    registry.register::<Tint>();
    let hanging = |from, flicker| Hanging {
        from: Some(Handle(from)),
        flicker,
    };
    // Without references, a reference to an entity is refused.
    let mut refused = Composer::new(&registry);
    let error = refused
        .put(
            &["lamps", "porch"],
            "test::Hanging",
            &hanging(1, 0.0).to_value(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("`lamps.porch.from"), "{error}");
    let mut composer = Composer::new(&registry).naming(&Porch).without_transient();
    let red = Tint {
        r: 1.0,
        g: 0.0,
        b: 0.0,
    };
    composer
        .put(
            &["lamps", "porch"],
            "test::Hanging",
            &hanging(1, 0.7).to_value(),
        )
        .expect("a lamp");
    composer
        .put(
            &["lamps", "gate"],
            "test::Hanging",
            &hanging(2, 0.2).to_value(),
        )
        .expect("a lamp");
    composer
        .put_part(
            &["tints", "porch"],
            "test::Tint",
            &Value::Struct(vec![("r".to_owned(), Value::F32(1.0))]),
        )
        .expect("a part of a tint");
    composer
        .put(&["tints", "gate"], "test::Tint", &red.to_value())
        .expect("a tint");
    // A place taken, or under a value put.
    for place in [&["lamps", "gate"][..], &["tints", "gate", "r"][..]] {
        let error = composer
            .put(place, "test::Tint", &red.to_value())
            .unwrap_err();
        assert!(
            error.to_string().contains("holds a value already"),
            "{error}"
        );
    }
    let text = composer.finish().expect("small");
    assert_eq!(
        text,
        "[lamps]\nporch = { from = \"porch\" }\ngate = { from = \"gate\" }\n\n[tints]\nporch = { r = 1.0 }\ngate = { r = 1.0, g = 0.0, b = 0.0 }\n"
    );
    let mut document =
        Document::parse(&registry, &text, slime_serialize::Mode::Strict).expect("a document");
    assert_eq!(document.keys(&["lamps"]), ["porch", "gate"]);
    assert!(document.keys(&["nothing"]).is_empty());
    let gate = document
        .read(&["lamps", "gate"], "test::Hanging", Some(&Porch))
        .expect("a lamp")
        .expect("there");
    assert_eq!(
        Hanging::from_value(&gate),
        Ok(hanging(2, 0.0)),
        "its flicker's default"
    );
    let porch = document
        .read_part(&["tints", "porch"], "test::Tint", None)
        .expect("a part of a tint");
    assert_eq!(
        porch,
        Some(Value::Struct(vec![("r".to_owned(), Value::F32(1.0))]))
    );
    assert_eq!(
        document
            .read(&["tints", "door"], "test::Tint", None)
            .expect("nothing"),
        None
    );
    // A reference read without references, or to a name that they do not know, is refused.
    let error = document
        .read(&["lamps", "porch"], "test::Hanging", None)
        .unwrap_err();
    assert!(
        error.to_string().starts_with("line 2, column 18: "),
        "placed: {error}"
    );
    let unknown = "lamps = { porch = { from = \"shed\" } }\n";
    let mut document =
        Document::parse(&registry, unknown, slime_serialize::Mode::Strict).expect("a document");
    let error = document
        .read(&["lamps", "porch"], "test::Hanging", Some(&Porch))
        .unwrap_err();
    assert!(
        error.to_string().contains("no entity is named `shed`"),
        "{error}"
    );
}

#[test]
fn a_document_larger_or_deeper_than_a_reader_reads_is_not_written() {
    let mut registry = Registry::new();
    registry.register::<String>();
    let mut composer = Composer::new(&registry);
    composer
        .put(&["text"], "String", &Value::Text("x".repeat(MAX_SIZE)))
        .expect("a text");
    let error = composer.finish().unwrap_err();
    assert!(error.to_string().contains("bytes"), "{error}");
    let keys: Vec<String> = (0..MAX_DEPTH).map(|depth| format!("k{depth}")).collect();
    let place: Vec<&str> = keys.iter().map(String::as_str).collect();
    let mut composer = Composer::new(&registry);
    composer
        .put(&place, "String", &Value::Text("deep".to_owned()))
        .expect("a text");
    let error = composer.finish().unwrap_err();
    assert!(error.to_string().contains("levels deep"), "{error}");
    let mut composer = Composer::new(&registry);
    composer
        .put(&place[1..], "String", &Value::Text("deep".to_owned()))
        .expect("a text");
    let text = composer.finish().expect("as deep as a reader reads");
    assert!(Document::parse(&registry, &text, slime_serialize::Mode::Lenient).is_ok());
}

/// Names one asset, `lamp`, whose ID is 7, as a scene file names the assets of its table; no entity.
struct Table;

impl References for Table {
    fn name(&self, _: u64) -> Option<String> {
        None
    }

    fn entity(&self, _: &str) -> Option<u64> {
        None
    }

    fn asset_name(&self, id: u128) -> Option<String> {
        (id == 7).then(|| "lamp".to_owned())
    }

    fn asset(&self, name: &str) -> Option<u128> {
        (name == "lamp").then_some(7)
    }
}

#[test]
fn a_document_names_assets_by_their_ids_unless_its_references_name_them_otherwise() {
    let registry = registry();
    let mut composer = Composer::new(&registry).naming(&Table);
    composer
        .put(&["porch"], "test::Mesh", &Value::Asset(7))
        .expect("named");
    let error = composer
        .put(&["gate"], "test::Mesh", &Value::Asset(8))
        .unwrap_err();
    assert!(
        error.to_string().contains("no name in the document"),
        "{error}"
    );
    let text = composer.finish().expect("small");
    assert_eq!(text, "porch = \"lamp\"\n");
    let mut document = Document::parse(&registry, &text, Mode::Strict).expect("a document");
    assert_eq!(
        document
            .read(&["porch"], "test::Mesh", Some(&Table))
            .expect("named"),
        Some(Value::Asset(7))
    );
    // Without a naming of their own, and through the default one, assets are their UUIDs.
    let error = document.read(&["porch"], "test::Mesh", None).unwrap_err();
    assert!(error.to_string().contains("is not an asset ID"), "{error}");
    let error = document
        .read(&["porch"], "test::Mesh", Some(&Porch))
        .unwrap_err();
    assert!(
        error.to_string().contains("no asset is named `lamp`"),
        "{error}"
    );
    let mut composer = Composer::new(&registry).naming(&Porch);
    composer
        .put(&["porch"], "test::Mesh", &Value::Asset(7))
        .expect("a UUID");
    assert_eq!(
        composer.finish().expect("small"),
        "porch = \"00000000-0000-0000-0000-000000000007\"\n"
    );
}

#[test]
fn a_document_says_where_its_tables_are_and_places_the_errors_of_its_layout() {
    let registry = registry();
    let text = "count = 3\n\n[porch]\nlamp = 1\n";
    let document = Document::parse(&registry, text, Mode::Strict).expect("a document");
    assert!(document.is_table(&[]));
    assert!(document.is_table(&["porch"]));
    assert!(!document.is_table(&["count"]));
    assert!(!document.is_table(&["shed"]));
    let error = document.error_at(
        &["porch", "lamp"],
        Problem::Invalid("not a lamp".to_owned()),
    );
    assert_eq!(
        error.to_string(),
        "line 4, column 8: `porch.lamp`: not a lamp"
    );
    let error = document.error_at(
        &["count"],
        Problem::UnknownField {
            field: "count".to_owned(),
            known: Vec::new(),
        },
    );
    assert_eq!(error.line_and_column(), Some((1, 1)), "at its key: {error}");
}
