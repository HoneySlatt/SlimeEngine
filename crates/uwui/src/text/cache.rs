//! Laid-out text kept from one frame to the next, within a budget.

use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

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
/// ([`TextLayout::set_bounds`]). It is to be used with the same [`Fonts`] each time. It gives its
/// texts shared, so that a display list holds them without copying them; a text that one still
/// holds is copied before it is laid out again in another box.
pub struct TextCache {
    budget: usize,
    used: usize,
    /// The count of uses, which numbers each and orders the texts.
    clock: u64,
    texts: HashMap<Arc<Key>, Entry>,
    /// The texts by their last use, oldest first.
    uses: BTreeMap<u64, Arc<Key>>,
}

/// A text kept and its style: the fields of its style that lay it out, its numbers by their bits,
/// since `f32` is neither `Eq` nor `Hash`. Equal to a [`Lookup`] of the same values, and hashed as
/// one, so that a text is found without copying its key.
struct Key {
    text: String,
    fonts: Vec<String>,
    numbers: [u32; 4],
    italic: bool,
    alignment: super::Alignment,
    direction: super::Direction,
    locale: Option<String>,
    features: Vec<String>,
}

/// A text and its style as the cache looks them up, borrowed from where they are.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Lookup<'a> {
    text: &'a str,
    fonts: &'a [String],
    numbers: [u32; 4],
    italic: bool,
    alignment: super::Alignment,
    direction: super::Direction,
    locale: Option<&'a str>,
    features: &'a [String],
}

impl<'a> Lookup<'a> {
    fn new(text: &'a str, style: &'a TextStyle, scale: f32) -> Self {
        // Taken apart whole, so that a field added to a text's style is keyed too.
        let TextStyle {
            fonts,
            size,
            weight,
            italic,
            line_height,
            alignment,
            direction,
            locale,
            features,
        } = style;
        Self {
            text,
            fonts,
            numbers: [
                size.to_bits(),
                weight.to_bits(),
                line_height.to_bits(),
                scale.to_bits(),
            ],
            italic: *italic,
            alignment: *alignment,
            direction: *direction,
            locale: locale.as_deref(),
            features,
        }
    }

    /// A key that owns what it looks up.
    fn key(self) -> Key {
        Key {
            text: self.text.to_owned(),
            fonts: self.fonts.to_vec(),
            numbers: self.numbers,
            italic: self.italic,
            alignment: self.alignment,
            direction: self.direction,
            locale: self.locale.map(str::to_owned),
            features: self.features.to_vec(),
        }
    }
}

/// What the map's keys and the lookups share: their fields, borrowed, by which both are hashed and
/// compared.
trait Keyed {
    fn lookup(&self) -> Lookup<'_>;
}

impl Keyed for Key {
    fn lookup(&self) -> Lookup<'_> {
        Lookup {
            text: &self.text,
            fonts: &self.fonts,
            numbers: self.numbers,
            italic: self.italic,
            alignment: self.alignment,
            direction: self.direction,
            locale: self.locale.as_deref(),
            features: &self.features,
        }
    }
}

impl Keyed for Lookup<'_> {
    fn lookup(&self) -> Lookup<'_> {
        *self
    }
}

impl<'a> Borrow<dyn Keyed + 'a> for Arc<Key> {
    fn borrow(&self) -> &(dyn Keyed + 'a) {
        &**self
    }
}

impl Hash for dyn Keyed + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.lookup().hash(state);
    }
}

impl PartialEq for dyn Keyed + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.lookup() == other.lookup()
    }
}

impl Eq for dyn Keyed + '_ {}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.lookup().hash(state);
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.lookup() == other.lookup()
    }
}

impl Eq for Key {}

struct Entry {
    layout: Arc<TextLayout>,
    memory: usize,
    used: u64,
    /// Its key, which its use names too.
    key: Arc<Key>,
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
    /// that box or laid out again in this one, or laid out with `fonts` and kept. A text kept is
    /// found without copying its text nor its style.
    pub fn get(
        &mut self,
        fonts: &mut Fonts,
        text: &str,
        style: &TextStyle,
        scale: f32,
        bounds: TextBounds,
    ) -> &Arc<TextLayout> {
        let lookup = Lookup::new(text, style, scale);
        self.clock += 1;
        let clock = self.clock;
        let key = if let Some(entry) = self.texts.get_mut(&lookup as &dyn Keyed) {
            self.uses.remove(&entry.used);
            entry.used = clock;
            self.uses.insert(clock, Arc::clone(&entry.key));
            // A layout that a display list holds is copied only to be laid out in another box.
            if entry.layout.bounds() != bounds {
                let before = entry.memory;
                Arc::make_mut(&mut entry.layout).set_bounds(fonts, bounds);
                entry.memory = entry.layout.memory();
                self.used = self.used - before + entry.memory;
            }
            Arc::clone(&entry.key)
        } else {
            let layout = Arc::new(fonts.lay_out(text, style, scale, bounds));
            let memory = layout.memory();
            self.used += memory;
            let key = Arc::new(lookup.key());
            self.uses.insert(clock, Arc::clone(&key));
            self.texts.insert(
                Arc::clone(&key),
                Entry {
                    layout,
                    memory,
                    used: clock,
                    key: Arc::clone(&key),
                },
            );
            key
        };
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

    #[test]
    fn a_text_held_elsewhere_is_given_again_as_it_is_and_copied_only_for_another_box() {
        let mut fonts = Fonts::new();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fonts/NotoSans.ttf");
        fonts
            .register(std::fs::read(path).expect("a test font"))
            .expect("a font");
        let style = TextStyle {
            fonts: vec!["Noto Sans".to_owned()],
            ..TextStyle::default()
        };
        let text = "Every panel can be docked";
        let mut cache = TextCache::new(TEXT_CACHE_BUDGET);
        let held = Arc::clone(cache.get(&mut fonts, text, &style, 1.0, TextBounds::default()));
        let again = cache.get(&mut fonts, text, &style, 1.0, TextBounds::default());
        assert!(Arc::ptr_eq(&held, again));
        let narrow = Arc::clone(cache.get(&mut fonts, text, &style, 1.0, TextBounds::width(60.0)));
        assert!(!Arc::ptr_eq(&held, &narrow));
        assert_eq!(held.bounds(), TextBounds::default());
        assert!(narrow.lines().len() > held.lines().len());
    }
}
