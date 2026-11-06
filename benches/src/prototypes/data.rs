//! The screens' data, made in the program at the sizes of the screens of reference, the same at
//! every run: a sequence of numbers from a fixed seed, no chance. Images are keys, decoded nowhere
//! (`docs/adr/0202-how-m6d1-compares-ways-of-writing-an-application.md`).

use std::time::Duration;

use uwui::display::ImageKey;

/// A sequence of numbers from a seed, the same at every run: xorshift64*.
struct Sequence(u64);

impl Sequence {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    /// The next number, below `bound`.
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize % bound.max(1)
    }

    fn pick<'a>(&mut self, words: &[&'a str]) -> &'a str {
        words[self.below(words.len())]
    }
}

const ADJECTIVES: &[&str] = &[
    "Ancient", "Broken", "Crimson", "Distant", "Endless", "Fallen", "Golden", "Hidden", "Iron",
    "Lost", "Midnight", "Northern", "Quiet", "Restless", "Silent", "Stellar", "Wild", "Hollow",
];
const NOUNS: &[&str] = &[
    "Kingdom", "Harbor", "Engine", "Garden", "Frontier", "Signal", "Tower", "River", "Forge",
    "Orbit", "Legend", "Shadow", "Voyage", "Citadel", "Machine", "Star", "Tide", "Crown",
];
const ARTISTS: &[&str] = &[
    "The Paper Kites",
    "Low Tide",
    "Mira Vale",
    "North Arcade",
    "Saint Juniper",
    "Velvet Coast",
    "Hollow Pines",
    "Echo Harbor",
    "June Avenue",
    "Static Bloom",
];

/// `count` games, as Slatty's fake profile holds them, each made by `game` of a title of two or
/// three words, a cover and a key art.
pub(crate) fn games<G>(count: usize, game: fn(String, ImageKey, ImageKey) -> G) -> Vec<G> {
    let mut sequence = Sequence::new(0x5_1A77);
    (0..count)
        .map(|place| {
            let mut title = format!("{} {}", sequence.pick(ADJECTIVES), sequence.pick(NOUNS));
            if sequence.below(3) == 0 {
                title.push_str(&format!(" {}", sequence.below(4) + 2));
            }
            game(
                title,
                ImageKey(place as u64),
                ImageKey(1_000_000 + place as u64),
            )
        })
        .collect()
}

/// A song's title, artist, album, length and thumbnail.
pub(crate) type SongParts = (String, String, String, Duration, ImageKey);

/// `count` songs, as Opium's fake library holds them, each made by `song` of its parts: albums of
/// ten songs, each by one artist, two to six minutes long, its thumbnail its album's.
pub(crate) fn songs<S>(count: usize, song: fn(SongParts) -> S) -> Vec<S> {
    let mut sequence = Sequence::new(0x0_9105);
    (0..count)
        .map(|place| {
            let album = place / 10;
            song((
                format!("{} {}", sequence.pick(ADJECTIVES), sequence.pick(NOUNS)),
                ARTISTS[album % ARTISTS.len()].to_owned(),
                format!(
                    "{} of the {}",
                    NOUNS[album % NOUNS.len()],
                    ADJECTIVES[album % 17]
                ),
                Duration::from_secs(120 + sequence.below(240) as u64),
                ImageKey(album as u64),
            ))
        })
        .collect()
}

/// `count` lines of a document, of 0 to 100 characters, as code is.
pub(crate) fn lines(count: usize) -> Vec<String> {
    let mut sequence = Sequence::new(0xD0C);
    (0..count)
        .map(|line| {
            let words = sequence.below(14);
            let mut text = " ".repeat(4 * sequence.below(4));
            for _ in 0..words {
                text.push_str(sequence.pick(NOUNS));
                text.push(' ');
            }
            text.push_str(&format!("// {line}"));
            text
        })
        .collect()
}
