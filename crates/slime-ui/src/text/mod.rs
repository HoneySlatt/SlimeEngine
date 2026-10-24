//! Text in every writing system, without a GPU (`docs/adr/0136-the-interfaces-text.md`): fonts
//! loaded from their bytes and named in stacks, text shaped, put in order and broken into lines by
//! parley, cut with an ellipsis to its box, hit at a point, its carets and selections; glyphs
//! rasterized by swash; laid-out text kept within a budget.
//!
//! A text is laid out with [`Fonts::lay_out`], in a [`TextStyle`] and a [`TextBounds`], at a scale
//! of the screen. The [`TextLayout`] gives its lines and its glyph runs, each glyph placed at a
//! whole pixel with a [`GlyphKey`] that names its image, which a [`GlyphRasterizer`] draws.
//!
//! ```
//! use slime_ui::text::{Fonts, GlyphRasterizer, TextBounds, TextStyle};
//!
//! let mut fonts = Fonts::new();
//! let families = fonts.register(std::fs::read("tests/fonts/NotoSans.ttf")?)?;
//! let style = TextStyle {
//!     fonts: families,
//!     ..TextStyle::default()
//! };
//! // A label of one line, 100 logical pixels wide, on a screen of scale 2.
//! let label = fonts.lay_out("Every panel can be docked", &style, 2.0, TextBounds::line(100.0));
//! assert!(label.shown().ends_with('…'));
//! assert!(label.width() <= 200.0);
//!
//! let mut rasterizer = GlyphRasterizer::new();
//! for glyph in label.runs().iter().flat_map(|run| run.placed()) {
//!     // The image goes at (glyph.x + image.left, glyph.y - image.top).
//!     let _image = rasterizer.rasterize(&fonts, glyph.key);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod cache;
mod fonts;
mod layout;
mod raster;
mod style;

pub use cache::{TEXT_CACHE_BUDGET, TextCache};
pub use fonts::{FontError, FontId, Fonts};
pub use layout::{Affinity, Glyph, GlyphRun, PlacedGlyph, TextLayout, TextLine, TextPosition};
pub use raster::{
    GlyphImage, GlyphKey, GlyphPixels, GlyphRasterizer, Representation, SUBPIXEL_POSITIONS,
};
pub use style::{Alignment, Direction, TextBounds, TextStyle};
