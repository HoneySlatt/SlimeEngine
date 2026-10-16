//! Values written as documents and read back: every registered type, floats to their bits, earlier
//! versions, aliases, and documents that are not ones.

use glam::{DVec3, Quat, Vec3};
use slime_reflect::{Kind, Problem, Range, Reflect, Registry, TypeInfo, Value, diff};
use slime_serialize::{
    MAX_DEPTH, MAX_SIZE, Mode, ReadProblem, read, read_file, read_partial, write,
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
    for number in [f32::MIN_POSITIVE, 1e-45, f32::MAX, 16_777_217.0, 0.3] {
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
fn upgrade_lamp(version: u32, value: Value) -> Result<Value, String> {
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
