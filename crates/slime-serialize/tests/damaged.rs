//! Documents damaged (`docs/adr/0096-the-files-the-engine-reads.md`): a valid document, as a text
//! and in the binary encoding, cut at every byte, its bytes changed and mutated at random, is
//! refused or read, leniently and strictly, and its comments kept, and never makes the reader
//! panic.

#[path = "../../slime-assets/tests/common/damage.rs"]
mod damage;

use damage::damage;
use slime_reflect::{Reflect, Registry};
use slime_serialize::{Composer, Document, Mode, encode, keep_comments, read, read_partial, write};

/// A colour.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Tint")]
struct Tint {
    r: f32,
    g: f32,
    b: f32,
}

/// How a lamp shines.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Mode")]
enum Glow {
    Off,
    Steady { level: u8 },
    Flicker(f64, String),
}

/// A value of every kind that a document holds.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Lamp")]
struct Lamp {
    name: String,
    #[reflect(range(min = 0.0, max = 100.0))]
    power: f32,
    tint: Tint,
    glows: Vec<Glow>,
    range: Option<f64>,
    lit: bool,
    count: i64,
    corners: [u16; 2],
}

#[test]
fn a_damaged_document_is_refused_or_read_never_a_panic() {
    let mut registry = Registry::new();
    registry.register::<Lamp>();
    let lamp = Lamp {
        name: "porch".to_owned(),
        power: 40.5,
        tint: Tint {
            r: 1.0,
            g: 0.8,
            b: 0.6,
        },
        glows: vec![
            Glow::Off,
            Glow::Steady { level: 3 },
            Glow::Flicker(0.5, "slow".to_owned()),
        ],
        range: Some(12.0),
        lit: true,
        count: -7,
        corners: [1, 65535],
    };
    let text = &write(&registry, "test::Lamp", &lamp.to_value()).expect("written");
    let text_written = &format!("# A lamp.\n{text}");
    assert!(read(&registry, "test::Lamp", text, Mode::Strict).is_ok());
    damage(text.as_bytes(), |bytes| {
        let text = String::from_utf8_lossy(bytes);
        for mode in [Mode::Strict, Mode::Lenient] {
            let _ = read(&registry, "test::Lamp", &text, mode);
            let _ = read_partial(&registry, "test::Lamp", &text, mode);
        }
        // Its comments kept, as the editor keeps a level's, whatever the text before.
        let _ = keep_comments(&text, text_written);
        let _ = keep_comments(text_written, &text);
    });
    // A document of several values, as a save or a scene file is, in the binary encoding.
    let mut composer = Composer::new(&registry);
    for place in ["porch", "gate"] {
        composer
            .put(&["lamps", place], "test::Lamp", &lamp.to_value())
            .expect("put");
    }
    let bytes = encode(&composer.finish().expect("written")).expect("TOML");
    damage(&bytes, |bytes| {
        for mode in [Mode::Strict, Mode::Lenient] {
            if let Ok(mut document) = Document::parse_binary(&registry, bytes, mode) {
                for place in document.keys(&["lamps"]) {
                    let _ = document.read(&["lamps", &place], "test::Lamp", None);
                }
                let _ = document.warnings();
            }
        }
    });
}
