//! The screens' data, made in the program at the sizes of the screens of reference, the same at
//! every run: a sequence of numbers from a fixed seed, no chance. Images are keys, decoded nowhere
//! (`docs/adr/0202-how-m6d1-compares-ways-of-writing-an-application.md`).

use std::time::Duration;

use uwui::display::ImageKey;

use super::control::library::Game;
use super::control::songs::Song;

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

/// `count` games, as Slatty's fake profile holds them: a title of two or three words, a cover and
/// a key art.
pub(crate) fn games(count: usize) -> Vec<Game> {
    let mut sequence = Sequence::new(0x5_1A77);
    (0..count)
        .map(|game| {
            let mut title = format!("{} {}", sequence.pick(ADJECTIVES), sequence.pick(NOUNS));
            if sequence.below(3) == 0 {
                title.push_str(&format!(" {}", sequence.below(4) + 2));
            }
            Game {
                title,
                cover: ImageKey(game as u64),
                art: ImageKey(1_000_000 + game as u64),
            }
        })
        .collect()
}

/// `count` songs, as Opium's fake library holds them: albums of ten songs, each by one artist,
/// two to six minutes long, its thumbnail its album's.
pub(crate) fn songs(count: usize) -> Vec<Song> {
    let mut sequence = Sequence::new(0x0_9105);
    (0..count)
        .map(|song| {
            let album = song / 10;
            Song {
                title: format!("{} {}", sequence.pick(ADJECTIVES), sequence.pick(NOUNS)),
                artist: ARTISTS[album % ARTISTS.len()].to_owned(),
                album: format!(
                    "{} of the {}",
                    NOUNS[album % NOUNS.len()],
                    ADJECTIVES[album % 17]
                ),
                length: Duration::from_secs(120 + sequence.below(240) as u64),
                thumbnail: ImageKey(album as u64),
            }
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
