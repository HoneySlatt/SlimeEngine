//! Laid-out text kept from one frame to the next, within a budget.

use std::collections::{BTreeMap, HashMap};

use super::fonts::Fonts;
use super::layout::TextLayout;
use super::style::{TextBounds, TextStyle};

/// The memory that a [`TextCache`] keeps its texts in by default, as [`TextLayout::memory`]
/// estimates it: 32 MiB, some 7 000 labels of 20 characters, or 200 paragraphs of 2 000 Latin
/// characters, 90 of Chinese.
pub const TEXT_CACHE_BUDGET: usize = 32 << 20;

/// Laid-out texts, kept by their text, style and scale within a budget of memory: those used
/// least recently leave first. A text asked for again in another box keeps its shaping: its lines
/// are broken again, and, if that box cuts it, its start laid out again with the ellipsis
/// ([`TextLayout::set_bounds`]). It is to be used with the same [`Fonts`] each time.
pub struct TextCache {
    budget: usize,
    used: usize,
    /// The count of uses, which numbers each and orders the texts.
    clock: u64,
    texts: HashMap<Key, Entry>,
    /// The texts by their last use, oldest first.
    uses: BTreeMap<u64, Key>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    /// The style's fields, its numbers by their bits, since `f32` is neither `Eq` nor `Hash`.
    fonts: Vec<String>,
    numbers: [u32; 4],
    italic: bool,
    alignment: super::Alignment,
    direction: super::Direction,
    locale: Option<String>,
}

struct Entry {
    layout: TextLayout,
    memory: usize,
    used: u64,
}

impl TextCache {
    /// An empty cache that keeps texts within `budget` bytes, as [`TextLayout::memory`] estimates
    /// them. A text larger than the whole budget is laid out, and kept until the next is asked
    /// for.
    pub fn new(budget: usize) -> Self {
        Self {
            budget,
            used: 0,
            clock: 0,
            texts: HashMap::new(),
            uses: BTreeMap::new(),
        }
    }

    /// The text `text` laid out in `style`, at `scale`, in the box `bounds`: kept from before, in
    /// that box or laid out again in this one, or laid out with `fonts` and kept.
    pub fn get(
        &mut self,
        fonts: &mut Fonts,
        text: &str,
        style: &TextStyle,
        scale: f32,
        bounds: TextBounds,
    ) -> &TextLayout {
        let key = Key {
            text: text.to_owned(),
            fonts: style.fonts.clone(),
            numbers: [
                style.size.to_bits(),
                style.weight.to_bits(),
                style.line_height.to_bits(),
                scale.to_bits(),
            ],
            italic: style.italic,
            alignment: style.alignment,
            direction: style.direction,
            locale: style.locale.clone(),
        };
        self.clock += 1;
        let clock = self.clock;
        if let Some(entry) = self.texts.get_mut(&key) {
            self.uses.remove(&entry.used);
            entry.used = clock;
            self.uses.insert(clock, key.clone());
            let before = entry.memory;
            entry.layout.set_bounds(fonts, bounds);
            entry.memory = entry.layout.memory();
            self.used = self.used - before + entry.memory;
        } else {
            let layout = fonts.lay_out(text, style, scale, bounds);
            let memory = layout.memory();
            self.used += memory;
            self.uses.insert(clock, key.clone());
            self.texts.insert(
                key.clone(),
                Entry {
                    layout,
                    memory,
                    used: clock,
                },
            );
        }
        self.evict(clock);
        &self.texts[&key].layout
    }

    /// The memory that the texts kept take, as [`TextLayout::memory`] estimates it.
    pub fn memory(&self) -> usize {
        self.used
    }

    /// How many texts are kept.
    pub fn len(&self) -> usize {
        self.texts.len()
    }

    /// Whether no text is kept.
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }

    /// Lets go of the texts used least recently until the rest fit the budget, but the text used
    /// at `kept`.
    fn evict(&mut self, kept: u64) {
        while self.used > self.budget {
            let Some((&oldest, _)) = self.uses.first_key_value() else {
                break;
            };
            if oldest == kept {
                break;
            }
            let key = self.uses.remove(&oldest).expect("the oldest use");
            let entry = self.texts.remove(&key).expect("a text for each use");
            self.used -= entry.memory;
        }
    }
}

impl Default for TextCache {
    /// A cache of [`TEXT_CACHE_BUDGET`].
    fn default() -> Self {
        Self::new(TEXT_CACHE_BUDGET)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_used_least_recently_leaves_first() {
        let mut fonts = Fonts::new();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fonts/NotoSans.ttf");
        fonts
            .register(std::fs::read(path).expect("a test font"))
            .expect("a font");
        let style = TextStyle {
            fonts: vec!["Noto Sans".to_owned()],
            ..TextStyle::default()
        };
        let one = fonts
            .lay_out("one", &style, 1.0, TextBounds::default())
            .memory();
        let mut cache = TextCache::new(one * 3 + one / 2);
        for text in ["one", "two", "six", "one", "ten"] {
            cache.get(&mut fonts, text, &style, 1.0, TextBounds::default());
        }
        let mut kept: Vec<&str> = cache.texts.keys().map(|key| key.text.as_str()).collect();
        kept.sort_unstable();
        assert_eq!(kept, ["one", "six", "ten"]);
        assert_eq!(cache.uses.len(), 3);
        assert_eq!(
            cache.used,
            cache
                .texts
                .values()
                .map(|entry| entry.memory)
                .sum::<usize>()
        );
    }
}
