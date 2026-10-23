//! Glyphs rasterized into images.

use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Format, Vector};

use super::fonts::{FontId, Fonts};

/// How many positions between two pixels a glyph's origin is rasterized at, across the line: a
/// glyph whose origin falls at 10.3 pixels is drawn from pixel 10 with the image of the second
/// position, a quarter of a pixel in. More positions space glyphs more evenly, and hold more
/// images of each glyph.
pub const SUBPIXEL_POSITIONS: u8 = 4;

/// What a glyph's image holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Representation {
    /// How much of each pixel the glyph covers, one byte per pixel, drawn in the text's color.
    Coverage,
    /// The glyph's own colors, four bytes per pixel, as an emoji has them.
    Color,
}

/// What names a glyph's image: its face, its index there, its size, the position between two
/// pixels that its origin falls at, and its representation. Two glyphs with the same key have the
/// same image, which the renderer keeps once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    font: FontId,
    glyph: u16,
    /// The size's bits: sizes that differ by any amount are other keys.
    size: u32,
    subpixel: u8,
    representation: Representation,
}

impl GlyphKey {
    /// The key of the glyph `glyph` of `font` at `size` pixels, whose origin falls at `x`, and the
    /// whole pixel that holds the origin.
    pub(crate) fn at(
        font: FontId,
        glyph: u32,
        size: f32,
        x: f32,
        representation: Representation,
    ) -> (Self, i32) {
        let whole = x.floor();
        let steps = f32::from(SUBPIXEL_POSITIONS);
        // A fraction that rounds to a whole pixel falls on the next one.
        let mut subpixel = ((x - whole) * steps).round();
        let mut pixel = whole as i32;
        if subpixel >= steps {
            subpixel = 0.0;
            pixel += 1;
        }
        let key = Self {
            font,
            // OpenType numbers glyphs in 16 bits.
            glyph: u16::try_from(glyph).unwrap_or(0),
            size: size.to_bits(),
            subpixel: subpixel as u8,
            representation,
        };
        (key, pixel)
    }

    /// The glyph's face.
    pub fn font(&self) -> FontId {
        self.font
    }

    /// The glyph's index in its face.
    pub fn glyph(&self) -> u16 {
        self.glyph
    }

    /// The size, in pixels.
    pub fn size(&self) -> f32 {
        f32::from_bits(self.size)
    }

    /// The position between two pixels that the origin falls at, from 0, on the pixel, to
    /// [`SUBPIXEL_POSITIONS`] − 1.
    pub fn subpixel(&self) -> u8 {
        self.subpixel
    }

    /// What its image holds.
    pub fn representation(&self) -> Representation {
        self.representation
    }
}

/// A glyph's image: its pixels, and where they go from the glyph's origin on its baseline, the
/// pixel that [`PlacedGlyph`](super::PlacedGlyph) gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphImage {
    /// The columns from the origin to the image's left edge, negative to its left.
    pub left: i32,
    /// The rows from the baseline up to the image's top edge.
    pub top: i32,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// The pixels, row by row from the top.
    pub pixels: GlyphPixels,
}

/// The pixels of a glyph's image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlyphPixels {
    /// A byte of coverage per pixel, 0 for none to 255 for whole.
    Coverage(Vec<u8>),
    /// Four bytes per pixel: red, green, blue and alpha, the colors not multiplied by the alpha.
    Color(Vec<u8>),
}

/// Rasterizes glyphs into images, without hinting: their outlines filled with antialiasing, or
/// their color bitmaps and layers. It keeps what it learns of each face from one glyph to the next.
pub struct GlyphRasterizer {
    context: ScaleContext,
}

impl GlyphRasterizer {
    /// A rasterizer that knows no face yet.
    pub fn new() -> Self {
        Self {
            context: ScaleContext::new(),
        }
    }

    /// The image of the glyph that `key` names, a face of `fonts`. None for a glyph that draws
    /// nothing, as a space, or that its face cannot draw as the key asks. A glyph of a color face
    /// that has no colors of its own is drawn as coverage, even when the key asks for color.
    ///
    /// # Panics
    ///
    /// If the key's face was not given by `fonts`.
    pub fn rasterize(&mut self, fonts: &Fonts, key: GlyphKey) -> Option<GlyphImage> {
        let face = fonts.face(key.font);
        let font = swash::FontRef::from_index(face.data.data(), face.index as usize)?;
        let mut scaler = self
            .context
            // Keyed by the font's bytes and index, unique across fonts, so that swash finds what it
            // learnt of the face: a `FontRef` made again takes a new key each time.
            .builder_with_id(font, [face.data.id(), u64::from(face.index)])
            .size(key.size())
            .hint(false)
            .normalized_coords(face.coords.iter())
            .build();
        let sources: &[Source] = match key.representation {
            Representation::Coverage => &[Source::Outline],
            Representation::Color => &[
                Source::ColorOutline(0),
                Source::ColorBitmap(StrikeWith::BestFit),
                Source::Outline,
            ],
        };
        let offset = f32::from(key.subpixel) / f32::from(SUBPIXEL_POSITIONS);
        let image = Render::new(sources)
            .format(Format::Alpha)
            .offset(Vector::new(offset, 0.0))
            .render(&mut scaler, key.glyph)?;
        if image.placement.width == 0 || image.placement.height == 0 {
            return None;
        }
        let pixels = match image.content {
            Content::Mask => GlyphPixels::Coverage(image.data),
            Content::Color => GlyphPixels::Color(image.data),
            // Not asked for: `Format::Alpha` gives no subpixel masks.
            Content::SubpixelMask => return None,
        };
        Some(GlyphImage {
            left: image.placement.left,
            top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            pixels,
        })
    }
}

impl Default for GlyphRasterizer {
    fn default() -> Self {
        Self::new()
    }
}
