//! The fonts that text is laid out with, and the faces that laid-out text names.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use icu_properties::CodePointMapData;
use icu_properties::props::Script;
use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{FontContext, FontData, LayoutContext};
use swash::StringId;

use super::layout::GlyphRun;

/// The fonts that text is laid out with: those registered from their bytes
/// ([`Fonts::register`]), and the system's when the program asks for them
/// ([`Fonts::with_system_fonts`]). It holds what laying text out reuses from one text to the next,
/// and the faces that laid-out text names by [`FontId`]: a [`TextLayout`](super::TextLayout), and
/// the keys of its glyphs, are to be used with the `Fonts` that laid it out.
///
/// A style names its fonts in a stack ([`TextStyle::fonts`](super::TextStyle::fonts)): a character
/// takes the first font of the stack that has it. A family that these fonts do not have is left out
/// of the stack, with a warning in the log the first time. Without the system's fonts, a character
/// that no font of the stack has is the first font's missing glyph, glyph 0, and a text whose stack
/// names no family that these fonts have draws nothing. With the system's fonts, the system finds a
/// font for the characters that the stack does not cover; the font found for a writing system and a
/// language is named for that writing system's characters from then on, after the stack's own, so
/// that the system's fallback, slow for some scripts, is asked once for each.
pub struct Fonts {
    pub(crate) context: FontContext,
    pub(crate) layouts: LayoutContext<()>,
    /// The faces that laid-out text uses, by [`FontId`]: a few, one per font and weight drawn,
    /// which a search finds faster than a map whose key would be allocated for each run.
    faces: Vec<Face>,
    /// With the system's fonts, the families that their fallback found.
    found: Option<Found>,
    /// The families named in a stack that these fonts do not have, warned of once.
    unknown: HashSet<String>,
}

/// For a writing system and a language, the family that the system's fallback drew its characters
/// with first.
type Found = HashMap<(Script, Option<String>), String>;

/// A face of a font, as laid-out text names it: one font of a file, at one point of its
/// variations, a variable font's weight among them. Two runs of text in the same face have the same
/// `FontId`; a `FontId` is valid with the [`Fonts`] that gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontId(pub(crate) u32);

/// What a [`FontId`] names.
pub(crate) struct Face {
    pub(crate) data: Blob<u8>,
    pub(crate) index: u32,
    /// The point of the variations, normalized as the font's tables give them.
    pub(crate) coords: Vec<i16>,
    /// Whether the face holds color glyphs: bitmaps (`CBDT`, `sbix`) or layers (`COLR`).
    pub(crate) color: bool,
    /// The family's name, as fontique, and so a stack, names it.
    pub(crate) family: String,
}

/// Why a font cannot be registered.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    /// The bytes hold no font that the engine reads: an OpenType or TrueType font, or a collection
    /// of them.
    #[error("the data holds no font: an OpenType or TrueType font, or a collection of them")]
    NoFont,
}

impl Fonts {
    /// Fonts without any of the system's: only those that [`Fonts::register`] gives, as a game
    /// draws with the fonts that it ships.
    pub fn new() -> Self {
        Self::with(false)
    }

    /// Fonts that fall back on the system's for what the registered fonts do not cover, as the
    /// editor does for the writing systems that it does not ship. The system's fonts are found
    /// through fontconfig on Linux, loaded as the program runs: without it, there are none. On
    /// Windows, through DirectWrite.
    pub fn with_system_fonts() -> Self {
        Self::with(true)
    }

    fn with(system: bool) -> Self {
        let collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: system,
        });
        Self {
            context: FontContext {
                collection,
                source_cache: SourceCache::default(),
            },
            layouts: LayoutContext::new(),
            faces: Vec::new(),
            found: system.then(HashMap::new),
            unknown: HashSet::new(),
        }
    }

    /// Registers the fonts that `bytes` hold, a font file or a collection of fonts, and returns
    /// the names of their families, by which a stack names them. Fonts registered are kept as long
    /// as these fonts are.
    ///
    /// # Errors
    ///
    /// [`FontError::NoFont`] if the bytes hold no font.
    pub fn register(
        &mut self,
        bytes: impl AsRef<[u8]> + Send + Sync + 'static,
    ) -> Result<Vec<String>, FontError> {
        let registered = self
            .context
            .collection
            .register_fonts(Blob::new(Arc::new(bytes)), None);
        if registered.is_empty() {
            return Err(FontError::NoFont);
        }
        let mut names = Vec::new();
        for (family, _) in registered {
            if let Some(info) = self.context.collection.family(family) {
                let name = info.name().to_owned();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        Ok(names)
    }

    /// The name of the family of the face `font`, as a stack names it.
    ///
    /// # Panics
    ///
    /// If `font` was not given by these fonts.
    pub fn family(&self, font: FontId) -> &str {
        &self.face(font).family
    }

    /// Whether the face `font` holds color glyphs, as emoji fonts do.
    ///
    /// # Panics
    ///
    /// If `font` was not given by these fonts.
    pub fn is_color(&self, font: FontId) -> bool {
        self.face(font).color
    }

    pub(crate) fn face(&self, font: FontId) -> &Face {
        &self.faces[font.0 as usize]
    }

    /// Warns, once for each, of the families of `stack` that these fonts do not have: they are
    /// left out of it, and a stack of none of them draws nothing.
    pub(crate) fn check(&mut self, stack: &[String]) {
        for family in stack {
            if !self.unknown.contains(family) && self.context.collection.family_id(family).is_none()
            {
                tracing::warn!(%family, "no font of this family: text that names it draws without it");
                self.unknown.insert(family.clone());
            }
        }
    }

    /// With the system's fonts, the pieces of `text` whose writing system, in `locale`, the
    /// system's fallback found a family for before, and that family, which `stack` does not name.
    pub(crate) fn found(
        &self,
        text: &str,
        stack: &[String],
        locale: Option<&str>,
    ) -> Vec<(Range<usize>, String)> {
        let Some(found) = self.found.as_ref().filter(|found| !found.is_empty()) else {
            return Vec::new();
        };
        scripts(text)
            .into_iter()
            .filter_map(|(range, script)| {
                let family = found.get(&(script, locale.map(str::to_owned)))?;
                (!stack.contains(family)).then(|| (range, family.clone()))
            })
            .collect()
    }

    /// Notes, with the system's fonts, the family of each run of `runs`, glyphs of `text`, that
    /// `stack` did not name: the system's fallback found it for the run's writing system in
    /// `locale`, which names it from then on.
    pub(crate) fn note_fallback(
        &mut self,
        text: &str,
        stack: &[String],
        locale: Option<&str>,
        runs: &[GlyphRun],
    ) {
        let Some(found) = self.found.as_mut() else {
            return;
        };
        let scripts = CodePointMapData::<Script>::new();
        for run in runs {
            let family = &self.faces[run.font.0 as usize].family;
            if stack.contains(family) {
                continue;
            }
            // The run's writing system: that of its first letter that has one.
            let script = text
                .get(run.text.clone())
                .unwrap_or_default()
                .chars()
                .map(|character| scripts.get(character))
                .find(|&script| script != Script::Common && script != Script::Inherited);
            if let Some(script) = script {
                found
                    .entry((script, locale.map(str::to_owned)))
                    .or_insert_with(|| family.clone());
            }
        }
    }

    /// The id of the face of `font` at the point `coords` of its variations, given a new one the
    /// first time.
    pub(crate) fn intern(&mut self, font: &FontData, coords: &[i16]) -> FontId {
        let blob = font.data.id();
        let found = self.faces.iter().position(|face| {
            face.data.id() == blob && face.index == font.index && face.coords == coords
        });
        let index = match found {
            Some(index) => index,
            None => {
                let (color, names) = describe(font.data.data(), font.index);
                // The name that fontique knows, so that a stack names the family by it.
                let family = names
                    .iter()
                    .find(|name| self.context.collection.family_id(name).is_some())
                    .or(names.first())
                    .cloned()
                    .unwrap_or_default();
                self.faces.push(Face {
                    data: font.data.clone(),
                    index: font.index,
                    coords: coords.to_vec(),
                    color,
                    family,
                });
                self.faces.len() - 1
            }
        };
        FontId(u32::try_from(index).expect("fewer than 2³² faces"))
    }
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

/// The pieces of `text` by writing system: the characters common to all, spaces and punctuation,
/// and the marks that follow a letter, belong to the piece before them, or to the one after them
/// at the text's start.
fn scripts(text: &str) -> Vec<(Range<usize>, Script)> {
    let map = CodePointMapData::<Script>::new();
    let mut pieces: Vec<(Range<usize>, Script)> = Vec::new();
    for (index, character) in text.char_indices() {
        let script = map.get(character);
        let end = index + character.len_utf8();
        let common = script == Script::Common || script == Script::Inherited;
        match pieces.last_mut() {
            Some((range, last)) if common || *last == script => range.end = end,
            Some((range, last)) if *last == Script::Common => {
                *last = script;
                range.end = end;
            }
            _ => pieces.push((index..end, if common { Script::Common } else { script })),
        }
    }
    pieces
}

/// Whether the face `index` of `data` holds color glyphs, and the names of its family: the
/// typographic family, then the family.
fn describe(data: &[u8], index: u32) -> (bool, Vec<String>) {
    let Some(font) = swash::FontRef::from_index(data, index as usize) else {
        return (false, Vec::new());
    };
    let color = [b"CBDT", b"sbix", b"COLR"]
        .iter()
        .any(|tag| font.table(swash::tag_from_bytes(tag)).is_some());
    let name = |id| {
        font.localized_strings()
            .find_by_id(id, Some("en"))
            .or_else(|| font.localized_strings().find_by_id(id, None))
            .map(|name| name.chars().collect::<String>())
    };
    let names = [StringId::TypographicFamily, StringId::Family]
        .into_iter()
        .filter_map(name)
        .collect();
    (color, names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_is_cut_into_pieces_by_writing_system_its_spaces_and_marks_with_them() {
        let text = "« Ελληνικά, 日本語 です。»";
        let pieces: Vec<(&str, Script)> = scripts(text)
            .into_iter()
            .map(|(range, script)| (&text[range], script))
            .collect();
        assert_eq!(
            pieces,
            [
                ("« Ελληνικά, ", Script::Greek),
                ("日本語 ", Script::Han),
                ("です。»", Script::Hiragana),
            ]
        );
        assert_eq!(scripts("…"), [(0..3, Script::Common)]);
    }
}
