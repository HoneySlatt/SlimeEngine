//! The reader given hostile input: damaged copies of documents, random bytes and random sequences
//! of KDL's tokens. It never panics, and what it reads is written and read again to the same tree.
//!
//! The random inputs come from a seed, new at each run, printed when a case fails:
//! `UWUI_KDL_SEED=<seed> cargo test -p uwui-kdl --test hostile` replays it. `read_and_round_trip`
//! takes bytes, as a fuzzer guided by coverage gives them, which may call it unchanged
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).

mod common;

use std::panic::{self, AssertUnwindSafe};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[path = "../../slime-assets/tests/common/damage.rs"]
mod damage;

/// Reads `bytes` as a document and, if it reads, checks that its normal form reads again to the
/// same tree.
fn read_and_round_trip(bytes: &[u8]) {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    let Ok(document) = uwui_kdl::read(text) else {
        return;
    };
    for node in document.nodes() {
        for entry in node.entries() {
            let _ = entry.value().to_string();
        }
    }
    let written = document.normal_form();
    let again = uwui_kdl::read(&written)
        .unwrap_or_else(|error| panic!("the normal form of {text:?} does not read: {error}"));
    assert_eq!(
        common::tree(&again),
        common::tree(&document),
        "the tree of {text:?} read again"
    );
    assert_eq!(
        again.normal_form(),
        written,
        "the normal form of {text:?} read again"
    );
}

/// A document of the kind that uwui reads: rules, values, annotations, every form of string.
const DOCUMENT: &str = r##"// A rule of a style sheet.
menu {
    select kind=Menu
    paint background=$raised border_width=1 {
        radii $radius_l $radius_l 4 4
        shadow color="#00000070" blur=(px)18 /-spread=0 { offset 0 $shadow_y }
    }
    layout direction=Column { min_size (px)$menu_width Auto }
}
/* A theme's values. */
duration { fast (ms)120; slow (s)0.3 }
text { tick "\u{E06C}" note #"raw \ text"# long """
    two
      lines
    """ }
numbers 0xff -0b101 1_000 1.5e-3 #inf #nan #true #false #null
"##;

#[test]
fn a_damaged_document_is_refused_or_read_never_panicking() {
    damage::damage(DOCUMENT.as_bytes(), read_and_round_trip);
}

/// How long each random test gives the reader, on a debug build.
const BUDGET: Duration = Duration::from_secs(2);

/// The most bytes of a random input.
const LONGEST: usize = 256;

/// Gives `check` random inputs of up to [`LONGEST`] bytes for [`BUDGET`], from a seed taken from
/// `UWUI_KDL_SEED` or from the clock; panics with the seed and the input if `check` panics on one.
fn random_inputs(check: impl Fn(&[u8])) {
    let seed = std::env::var("UWUI_KDL_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(1, |time| time.as_nanos() as u64)
        })
        | 1;
    // Xorshift: numbers that look random, the same for the same seed.
    let mut state = seed;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let start = Instant::now();
    let mut input = Vec::with_capacity(LONGEST);
    while start.elapsed() < BUDGET {
        input.clear();
        let length = (next() % (LONGEST as u64 + 1)) as usize;
        input.extend((0..length).map(|_| next() as u8));
        if panic::catch_unwind(AssertUnwindSafe(|| check(&input))).is_err() {
            panic!("the reader failed on {input:?}, from the seed UWUI_KDL_SEED={seed}");
        }
    }
}

#[test]
fn random_bytes_never_make_the_reader_panic() {
    // Random bytes are seldom UTF-8: the bytes that are not become U+FFFD, so that the reader
    // reads every input.
    random_inputs(|bytes| read_and_round_trip(String::from_utf8_lossy(bytes).as_bytes()));
}

/// Pieces of KDL's syntax, which random sequences of make far more documents that read than random
/// bytes do.
const TOKENS: [&str; 40] = [
    "node",
    " ",
    "\t",
    "\n",
    "\r\n",
    ";",
    "{",
    "}",
    "=",
    "(",
    ")",
    "\"",
    "\"\"\"",
    "#\"",
    "\"#",
    "\\",
    "\\n",
    "\\u{1F600}",
    "#",
    "#true",
    "#null",
    "#inf",
    "/-",
    "//",
    "/*",
    "*/",
    "1",
    "-",
    "0x",
    "0b",
    "_",
    ".",
    "e",
    "+",
    "é",
    "\u{FEFF}",
    "\u{2028}",
    "a=b",
    "$x",
    "\\\n",
];

#[test]
fn random_sequences_of_kdl_s_tokens_never_make_the_reader_panic() {
    random_inputs(|choices| {
        let text: String = choices
            .iter()
            .map(|&choice| TOKENS[usize::from(choice) % TOKENS.len()])
            .collect();
        read_and_round_trip(text.as_bytes());
    });
}
