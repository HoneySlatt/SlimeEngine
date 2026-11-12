//! The reader of layouts in KDL checked against the reflected reader of layouts in TOML that it
//! replaced (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`): random
//! layouts, as a program may hold them before it normalizes them, written both ways, read to the
//! same layout, bit for bit, and written in KDL and read back to the layout written.
//!
//! The reflected reader is kept here as the test's oracle while `slime-reflect` is in uwui, until
//! M6d.3c3; the editor's fixtures of the layouts that it saved,
//! `crates/slime-editor/tests/reference/layouts/`, outlive it.

use slime_serialize::Mode;

use super::{Floating, Layout, Node, Reading, Share, read, write};

/// A generator of numbers that repeats from its seed: xorshift.
struct Numbers(u64);

impl Numbers {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }

    fn bits(&mut self) -> u32 {
        self.below(1 << 32) as u32
    }
}

/// Panels' keys, a program's and hostile ones: a space, a keyword, a number, a sign that KDL takes
/// as syntax, quotes, a backslash, a newline, a character that a document may not hold as itself,
/// another script, and nothing.
const KEYS: [&str; 14] = [
    "hierarchy",
    "viewport",
    "a panel",
    "true",
    "-1",
    "a#b",
    "a=b",
    "say \"hi\"",
    "back\\slash",
    "two\nlines",
    "\u{7f}",
    "パネル",
    "",
    "null",
];

/// A random float: any pattern of bits, or a value that writing a float gets wrong most often.
fn random_float(numbers: &mut Numbers) -> f32 {
    const SPECIAL: [f32; 10] = [
        0.25,
        0.55,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::MIN_POSITIVE,
        f32::MAX,
        7.038_531e-26,
        -7.038_531e-26,
    ];
    if numbers.below(2) == 0 {
        SPECIAL[numbers.below(SPECIAL.len())]
    } else {
        f32::from_bits(numbers.bits())
    }
}

/// A random key.
fn random_key(numbers: &mut Numbers) -> String {
    KEYS[numbers.below(KEYS.len())].to_owned()
}

/// A random part of a layout, at most `depth` splits deep: splits of none to four children, stacks
/// of none to four panels, a stack's chosen panel any place.
fn random_node(numbers: &mut Numbers, depth: usize) -> Node {
    if depth == 0 || numbers.below(3) == 0 {
        let panels = (0..numbers.below(5)).map(|_| random_key(numbers)).collect();
        let chosen = if numbers.below(2) == 0 {
            numbers.below(5) as u32
        } else {
            numbers.bits()
        };
        return Node::Tabs { panels, chosen };
    }
    Node::Split {
        side_by_side: numbers.below(2) == 0,
        children: (0..numbers.below(5))
            .map(|_| Share {
                share: random_float(numbers),
                node: random_node(numbers, depth - 1),
            })
            .collect(),
    }
}

/// A random layout, not normalized.
fn random_layout(numbers: &mut Numbers) -> Layout {
    Layout {
        root: random_node(numbers, 6),
        floating: (0..numbers.below(4))
            .map(|_| Floating {
                panel: random_key(numbers),
                origin: [random_float(numbers), random_float(numbers)],
                size: [random_float(numbers), random_float(numbers)],
            })
            .collect(),
        closed: (0..numbers.below(4)).map(|_| random_key(numbers)).collect(),
    }
}

/// `layout` as text that tells apart each float's bits, but a value that is not a number's: `Debug`
/// writes the shortest text that reads back to each finite float, `-0.0` apart from `0.0`, and
/// every value that is not a number as `NaN`.
fn bits(layout: &Layout) -> String {
    format!("{layout:?}")
}

#[test]
fn random_layouts_read_as_the_reflected_reader_read_them() {
    let seed = std::env::var("UWUI_DOCK_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |time| time.as_nanos() as u64)
                | 1
        });
    let mut numbers = Numbers(seed);
    for case in 0..1_000 {
        let layout = random_layout(&mut numbers);
        let toml = slime_serialize::write_as(&layout).expect("written as TOML");
        let kdl = write(&layout, None).expect("written as KDL");
        let context = || format!("case {case}, from the seed UWUI_DOCK_SEED={seed}:\nKDL:\n{kdl}");
        let (want, _) = slime_serialize::read_as::<Layout>(&toml, Mode::Lenient)
            .unwrap_or_else(|error| panic!("{}\nTOML read: {error}", context()));
        let got = read("random.layout.kdl", &kdl, Reading::Strict)
            .unwrap_or_else(|error| panic!("{}\nKDL read: {error}", context()));
        assert!(got.warnings.is_none(), "{}", context());
        assert_eq!(bits(&got.layout), bits(&want), "{}", context());
        assert_eq!(bits(&got.layout), bits(&layout), "{}", context());
    }
}
