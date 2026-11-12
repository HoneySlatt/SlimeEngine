//! `LinearColor` as reflection describes it, kept as a fixture
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`): its description, its
//! fields' documentation included but not the type's own words, the types it registers, its
//! default, its values and its refusals. A scene as text, a scene
//! cooked and the dev overlay read a color through these alone, so that a description that gives
//! the same lines reads and writes them the same.
//!
//! The fixture, `crates/slime-reflect/tests/reference/linear_color.txt`, was recorded while
//! `slime-color` derived the description, before `slime-reflect` described it by hand.
//! `SLIME_UPDATE_REFERENCE_IMAGES` writes it again: a change of the description changes it on
//! purpose, and changes what scenes hold.

use std::fmt::Write as _;
use std::path::Path;

use slime_color::LinearColor;
use slime_reflect::{Reflect, Registry, Value};

/// Colors whose bits the values must keep: signed zeros, values past 1, a value that is not a
/// number, infinities and the smallest normal float.
const SAMPLES: [LinearColor; 6] = [
    LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
    LinearColor::rgba(-0.0, 1.0, 0.5, 1.0),
    LinearColor::rgba(1.0e4, 0.214_041_14, 0.003_035_27, 0.25),
    LinearColor::rgba(f32::NAN, 0.1, 0.2, 0.3),
    LinearColor::rgba(
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        f32::MAX,
    ),
    LinearColor::WHITE,
];

/// The fixture's lines.
fn lines() -> String {
    let mut out = String::new();
    writeln!(out, "# type_name\n{}", LinearColor::type_name()).expect("written");
    // The type's documentation is its rustdoc's, in `slime-color`, which a description written
    // elsewhere sums up: the fixture records that it has one, not its words.
    let mut info = LinearColor::type_info();
    writeln!(out, "# documented\n{}", !info.doc.is_empty()).expect("written");
    info.doc.clear();
    writeln!(out, "# type_info\n{info:#?}").expect("written");
    let mut registry = Registry::new();
    registry.register::<LinearColor>();
    let mut names: Vec<&str> = registry
        .iter()
        .map(|registration| registration.info().name.as_str())
        .collect();
    names.sort_unstable();
    writeln!(out, "# registered\n{names:?}").expect("written");
    writeln!(out, "# default\n{:?}", LinearColor::default_value()).expect("written");
    writeln!(out, "# upgrade\n{}", LinearColor::upgrade().is_some()).expect("written");
    writeln!(out, "# values").expect("written");
    for color in SAMPLES {
        let value = color.to_value();
        let back = LinearColor::from_value(&value).expect("a color read back");
        let bits = |color: LinearColor| [color.r, color.g, color.b, color.a].map(f32::to_bits);
        assert_eq!(bits(back), bits(color), "{color:?} read back to its bits");
        writeln!(out, "{value:?}").expect("written");
    }
    writeln!(out, "# refused").expect("written");
    let field = |name: &str, value: Value| (name.to_owned(), value);
    let refused = [
        Value::F32(1.0),
        Value::Struct(vec![
            field("r", Value::F32(1.0)),
            field("g", Value::F32(1.0)),
            field("b", Value::F32(1.0)),
        ]),
        Value::Struct(vec![
            field("r", Value::F32(1.0)),
            field("g", Value::F32(1.0)),
            field("b", Value::F32(1.0)),
            field("a", Value::F32(1.0)),
            field("x", Value::F32(1.0)),
        ]),
        Value::Struct(vec![
            field("r", Value::Text("red".to_owned())),
            field("g", Value::F32(1.0)),
            field("b", Value::F32(1.0)),
            field("a", Value::F32(1.0)),
        ]),
        Value::Struct(vec![
            field("r", Value::F64(0.5)),
            field("g", Value::F32(1.0)),
            field("b", Value::F32(1.0)),
            field("a", Value::F32(1.0)),
        ]),
    ];
    for value in refused {
        let read = LinearColor::from_value(&value);
        writeln!(out, "{value:?}\n  {read:?}").expect("written");
    }
    out
}

#[test]
fn linear_color_is_described_as_the_fixture_records() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../slime-reflect/tests/reference/linear_color.txt");
    let lines = lines();
    if std::env::var_os("SLIME_UPDATE_REFERENCE_IMAGES").is_some() {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("its folder is made");
        std::fs::write(&path, &lines).expect("the fixture is written");
        return;
    }
    let recorded = std::fs::read_to_string(&path).expect("the fixture is read");
    for (number, (line, want)) in lines.lines().zip(recorded.lines()).enumerate() {
        assert_eq!(line, want, "line {} of {}", number + 1, path.display());
    }
    assert_eq!(
        lines.lines().count(),
        recorded.lines().count(),
        "the lines of {}",
        path.display()
    );
}
