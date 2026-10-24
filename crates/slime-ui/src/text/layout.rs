//! Text laid out: shaped, ordered, broken into lines, aligned, and cut with an ellipsis where its
//! box asks.

use std::borrow::Cow;
use std::ops::Range;

use parley::{
    Affinity as ParleyAffinity, Alignment as ParleyAlignment, AlignmentOptions, BaseDirection,
    BoundingBox, Cluster, Cursor, FontFamily, FontFamilyName, FontStyle, FontWeight, Language,
    Layout, LineHeight, PositionedLayoutItem, Selection, StyleProperty,
};

use crate::Rect;

use super::fonts::{FontId, Fonts};
use super::raster::{GlyphKey, Representation};
use super::style::{Alignment, Direction, TextBounds, TextStyle};

/// The character that ends a text cut to its box, and what ends it when the fonts lack it.
const ELLIPSIS: &str = "…";
const FULL_STOPS: &str = "...";

/// What a laid-out text holds in memory, in bytes: a part of fixed size, then so much per byte of
/// its texts, per glyph and per run beyond the glyphs and runs that [`TextLayout`] gathers. Fitted
/// to what a counting allocator measured of labels and paragraphs of seven writing systems, not
/// cut, kept within 12 % of it (`docs/measurements.md`, M6b.2).
const FIXED_MEMORY: usize = 3 << 10;
const MEMORY_PER_BYTE: usize = 50;
const PARLEY_PER_GLYPH: usize = 4;
const PARLEY_PER_RUN: usize = 350;

/// How much wider than its box a line may be, in pixels, before it is taken as wider: the widths
/// that shaping adds up are not exact.
const SLACK: f32 = 0.01;

/// A text laid out in a box ([`Fonts::lay_out`]): shaped, put in the order that its directions
/// give, broken into lines and aligned, at a scale. Its coordinates are pixels of the screen, the
/// logical pixels of the style and the box times the scale, from the box's top left corner, y down.
///
/// A text that its box cuts shows its start and an ellipsis ([`TextLayout::shown`]): the lines,
/// the glyphs and every position that the layout gives or takes are those of the text shown.
#[derive(Clone)]
pub struct TextLayout {
    text: String,
    style: TextStyle,
    scale: f32,
    bounds: TextBounds,
    /// The whole text, shaped, broken into lines for the box's width.
    whole: Layout<()>,
    /// The text cut, when it is: where, and what is shown.
    cut: Option<Cut>,
    lines: Vec<TextLine>,
    runs: Vec<GlyphRun>,
}

/// A text cut to its box.
#[derive(Clone)]
struct Cut {
    /// The byte of the whole text where it is cut: the ellipsis replaces it and what follows, the
    /// spaces before it included.
    at: usize,
    /// The text shown: the whole text's start, then the ellipsis, or three full stops when the
    /// fonts lack it.
    text: String,
    layout: Layout<()>,
}

/// A line of a laid-out text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    /// The bytes of the text shown that the line holds.
    pub text: Range<usize>,
    /// The y of its baseline.
    pub baseline: f32,
    /// The y of its top, where the line before it ends.
    pub top: f32,
    /// The y of its bottom.
    pub bottom: f32,
}

/// Glyphs of one face, size and direction, in the order they are drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    /// The face that draws them.
    pub font: FontId,
    /// The size they are drawn at, in pixels of the screen.
    pub size: f32,
    /// What image their face gives them: coverage, or color for a face of color glyphs.
    pub representation: Representation,
    /// Whether the run is written from right to left.
    pub right_to_left: bool,
    /// The bytes of the text shown that the run draws.
    pub text: Range<usize>,
    /// The glyphs, from left to right.
    pub glyphs: Vec<Glyph>,
}

/// A glyph placed in a laid-out text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// The glyph's index in its face.
    pub id: u32,
    /// The x of its origin, in pixels of the screen, a fraction of a pixel included.
    pub x: f32,
    /// The y of its origin: its line's baseline, moved where its font places it, as a mark above
    /// a letter.
    pub y: f32,
}

/// A glyph as it is drawn: the key of its image, and the pixel that its image is placed from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlacedGlyph {
    /// The key of the glyph's image, which holds the fraction of a pixel that its origin falls
    /// on ([`GlyphKey::subpixel`]).
    pub key: GlyphKey,
    /// The whole pixel that the glyph's origin falls in, the x of the image's origin.
    pub x: i32,
    /// The row of its origin, rounded: each glyph's, so that a mark may move by up to half a
    /// pixel against its letter.
    pub y: i32,
}

impl GlyphRun {
    /// The run's glyphs as they are drawn, each at a whole pixel and a key that holds the rest.
    pub fn placed(&self) -> impl Iterator<Item = PlacedGlyph> + '_ {
        self.glyphs.iter().map(|glyph| {
            let (key, x) =
                GlyphKey::at(self.font, glyph.id, self.size, glyph.x, self.representation);
            PlacedGlyph {
                key,
                x,
                y: glyph.y.round() as i32,
            }
        })
    }
}

/// A position in a text: between two characters, attached to the one that follows it,
/// downstream, or to the one before it, upstream. The two differ where the text's direction
/// changes, or at the end of a line that breaks within the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextPosition {
    /// The byte of the text before which the position is.
    pub index: usize,
    /// Which side the position is attached to.
    pub affinity: Affinity,
}

/// The character that a [`TextPosition`] is attached to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
    /// The character that follows it.
    #[default]
    Downstream,
    /// The character before it.
    Upstream,
}

impl Fonts {
    /// Lays out `text` in the style `style`, at `scale` pixels of the screen per logical pixel,
    /// in the box `bounds`.
    pub fn lay_out(
        &mut self,
        text: &str,
        style: &TextStyle,
        scale: f32,
        bounds: TextBounds,
    ) -> TextLayout {
        self.check(&style.fonts);
        let whole = self.shape(text, style, scale, direction(style.direction));
        let mut layout = TextLayout {
            text: text.to_owned(),
            style: style.clone(),
            scale,
            bounds,
            whole,
            cut: None,
            lines: Vec::new(),
            runs: Vec::new(),
        };
        layout.fit(self);
        self.note_fallback(
            layout.shown(),
            &style.fonts,
            style.locale.as_deref(),
            &layout.runs,
        );
        layout
    }

    /// `text` shaped in `style`, at `scale`, in `direction`: with the style's stack, which names,
    /// for the writing systems that the system's fallback found a family for, that family after
    /// its own.
    fn shape(
        &mut self,
        text: &str,
        style: &TextStyle,
        scale: f32,
        direction: BaseDirection,
    ) -> Layout<()> {
        let found = self.found(text, &style.fonts, style.locale.as_deref());
        let names = named(&style.fonts, None);
        let pieces: Vec<(Range<usize>, Vec<FontFamilyName<'_>>)> = found
            .iter()
            .map(|(range, family)| (range.clone(), named(&style.fonts, Some(family))))
            .collect();
        let mut builder = self
            .layouts
            .ranged_builder(&mut self.context, text, scale, true);
        builder.set_base_direction(direction);
        builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Borrowed(
            &names,
        ))));
        for (range, names) in &pieces {
            builder.push(
                StyleProperty::FontFamily(FontFamily::List(Cow::Borrowed(names))),
                range.clone(),
            );
        }
        builder.push_default(StyleProperty::FontSize(style.size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight)));
        if style.italic {
            builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
        }
        builder.push_default(StyleProperty::LineHeight(LineHeight::MetricsRelative(
            style.line_height,
        )));
        if let Some(locale) = &style.locale {
            builder.push_default(StyleProperty::Locale(Language::parse(locale).ok()));
        }
        builder.build(text)
    }
}

/// The text shown, its scale and its box: its shaping is parley's, which says nothing readable.
impl std::fmt::Debug for TextLayout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TextLayout")
            .field("shown", &self.shown())
            .field("scale", &self.scale)
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

impl TextLayout {
    /// The whole text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text shown: the whole text, or, cut, its start and the ellipsis.
    pub fn shown(&self) -> &str {
        self.cut.as_ref().map_or(&self.text, |cut| &cut.text)
    }

    /// Where the whole text is cut, the byte of it that the ellipsis replaces from, if its box
    /// cuts it.
    pub fn cut(&self) -> Option<usize> {
        self.cut.as_ref().map(|cut| cut.at)
    }

    /// The style it is laid out in.
    pub fn style(&self) -> &TextStyle {
        &self.style
    }

    /// The pixels of the screen per logical pixel.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The box it is laid out in.
    pub fn bounds(&self) -> TextBounds {
        self.bounds
    }

    /// The width of its widest line, without the spaces that end a line.
    pub fn width(&self) -> f32 {
        self.shown_layout().width()
    }

    /// The height of its lines together.
    pub fn height(&self) -> f32 {
        self.shown_layout().height()
    }

    /// Whether the text is written from right to left, as its style's direction or its first
    /// letter that has a direction says.
    pub fn is_right_to_left(&self) -> bool {
        self.shown_layout().is_rtl()
    }

    /// Its lines, top to bottom.
    pub fn lines(&self) -> &[TextLine] {
        &self.lines
    }

    /// Its glyphs, by runs of one face, size and direction, line by line, left to right in each.
    pub fn runs(&self) -> &[GlyphRun] {
        &self.runs
    }

    /// An estimate of the memory it holds, in bytes: its texts, the whole one and the one shown
    /// when cut, what parley keeps of each, its glyphs and its runs. Within some 12 % of what a
    /// text not cut holds, as measured; a cut text's whole layout is counted by its text alone.
    pub fn memory(&self) -> usize {
        let glyphs: usize = self.runs.iter().map(|run| run.glyphs.len()).sum();
        let text = self.text.len() + self.cut.as_ref().map_or(0, |cut| cut.text.len());
        FIXED_MEMORY
            + text * MEMORY_PER_BYTE
            + glyphs * (size_of::<Glyph>() + PARLEY_PER_GLYPH)
            + self.runs.len() * (size_of::<GlyphRun>() + PARLEY_PER_RUN)
            + self.lines.len() * size_of::<TextLine>()
    }

    /// Lays the text out again in the box `bounds`, with the fonts that laid it out. The text is
    /// not shaped again, but when the box cuts it: then its start, up to the cut, is laid out
    /// again with the ellipsis.
    pub fn set_bounds(&mut self, fonts: &mut Fonts, bounds: TextBounds) {
        if bounds != self.bounds {
            self.bounds = bounds;
            self.fit(fonts);
        }
    }

    /// The bytes of the cluster under the point (`x`, `y`): the characters that one or more
    /// glyphs draw together, a letter and its marks, a ligature, an emoji sequence. None where no
    /// glyph is.
    pub fn cluster_at(&self, x: f32, y: f32) -> Option<Range<usize>> {
        Cluster::from_point_exact(self.shown_layout(), x, y)
            .map(|(cluster, _)| cluster.text_range())
    }

    /// The position nearest the point (`x`, `y`), where a click puts a caret: before or after the
    /// cluster under it, whichever side is nearer, in the cluster's direction.
    pub fn position_at(&self, x: f32, y: f32) -> TextPosition {
        let cursor = Cursor::from_point(self.shown_layout(), x, y);
        TextPosition {
            index: cursor.index(),
            affinity: match cursor.affinity() {
                ParleyAffinity::Downstream => Affinity::Downstream,
                ParleyAffinity::Upstream => Affinity::Upstream,
            },
        }
    }

    /// The caret at `position`: one pixel wide, as tall as its line.
    pub fn caret(&self, position: TextPosition) -> Rect {
        let layout = self.shown_layout();
        rect(cursor(layout, position).geometry(layout, 1.0))
    }

    /// The rectangles that show the bytes `range` selected, one per line and direction's piece.
    pub fn selection(&self, range: Range<usize>) -> Vec<Rect> {
        let layout = self.shown_layout();
        let start = Cursor::from_byte_index(layout, range.start, ParleyAffinity::Downstream);
        let end = Cursor::from_byte_index(layout, range.end, ParleyAffinity::Upstream);
        Selection::new(start, end)
            .geometry(layout)
            .into_iter()
            .map(|(bounds, _)| rect(bounds))
            .collect()
    }

    /// The layout of the text shown.
    fn shown_layout(&self) -> &Layout<()> {
        self.cut.as_ref().map_or(&self.whole, |cut| &cut.layout)
    }

    /// Breaks the lines for the box, cuts the text if the box asks, aligns, and gathers the
    /// lines and the glyphs shown.
    fn fit(&mut self, fonts: &mut Fonts) {
        let width = self.bounds.width.map(|width| width * self.scale);
        self.whole.break_all_lines(width);
        self.whole
            .align(alignment(self.style.alignment), AlignmentOptions::default());
        self.cut = match (width, self.bounds.max_lines) {
            (Some(width), Some(max)) => self.cut_to(fonts, width, max.get()),
            _ => None,
        };
        self.gather(fonts);
    }

    /// The text cut to `max` lines of `width` pixels, if it holds more, or if its last line
    /// shown is wider.
    fn cut_to(&mut self, fonts: &mut Fonts, width: f32, max: usize) -> Option<Cut> {
        let mut lines = self.whole.len();
        // A text that ends in a line break ends in an empty line, which shows nothing.
        if lines > 1
            && self
                .whole
                .get(lines - 1)
                .is_some_and(|line| line.text_range().is_empty())
        {
            lines -= 1;
        }
        let last = self.whole.get(lines.min(max).checked_sub(1)?)?;
        let metrics = last.metrics();
        if lines <= max && metrics.advance - metrics.hanging_advance <= width + SLACK {
            return None;
        }
        let range = last.text_range();
        // A cut falls between clusters, never within one, which would part a letter from its
        // marks.
        let mut ends: Vec<usize> = last
            .runs()
            .flat_map(|run| run.clusters().map(|cluster| cluster.text_range().start))
            .filter(|&start| start > range.start)
            .chain([range.end])
            .collect();
        ends.sort_unstable();
        ends.dedup();
        let direction = if self.whole.is_rtl() {
            BaseDirection::Rtl
        } else {
            BaseDirection::Ltr
        };
        let ellipsis = ellipsis(fonts, &self.style, self.scale, direction);
        // The longest start of the line that fits with the ellipsis, laid out alone: shaped again,
        // since a letter of Arabic before a cut takes its final form.
        let fits = |layout: &mut Self, fonts: &mut Fonts, end: usize| {
            let piece = cut_text(&layout.text[range.start..end], ellipsis);
            let mut alone = fonts.shape(&piece, &layout.style, layout.scale, direction);
            alone.break_all_lines(None);
            alone.width() <= width + SLACK
        };
        let mut low = 0;
        let mut high = ends.len();
        while low < high {
            let middle = (low + high) / 2;
            if fits(self, fonts, ends[middle]) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        // Where none fits, the line shows the ellipsis alone.
        let mut end = low.checked_sub(1).map_or(range.start, |index| ends[index]);
        loop {
            let text = cut_text(&self.text[..end], ellipsis);
            let mut layout = fonts.shape(&text, &self.style, self.scale, direction);
            layout.break_all_lines(Some(width));
            layout.align(alignment(self.style.alignment), AlignmentOptions::default());
            // Shaped again, the start may break into more lines than it did in the whole text.
            if layout.len() <= max || end <= range.start {
                return Some(Cut {
                    at: self.text[..end].trim_end().len(),
                    text,
                    layout,
                });
            }
            end = ends
                .iter()
                .rev()
                .find(|&&candidate| candidate < end)
                .copied()
                .unwrap_or(range.start);
        }
    }

    /// The lines and glyph runs of the text shown.
    fn gather(&mut self, fonts: &mut Fonts) {
        let layout = self.cut.as_ref().map_or(&self.whole, |cut| &cut.layout);
        self.lines.clear();
        self.runs.clear();
        for line in layout.lines() {
            let metrics = line.metrics();
            self.lines.push(TextLine {
                text: line.text_range(),
                baseline: metrics.baseline,
                top: metrics.block_min_coord,
                bottom: metrics.block_max_coord,
            });
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let coords: Vec<i16> = run
                    .normalized_coords()
                    .iter()
                    .map(|coord| coord.to_bits())
                    .collect();
                let font = fonts.intern(run.font(), &coords);
                let representation = if fonts.is_color(font) {
                    Representation::Color
                } else {
                    Representation::Coverage
                };
                self.runs.push(GlyphRun {
                    font,
                    size: run.font_size(),
                    representation,
                    right_to_left: run.is_rtl(),
                    text: run.text_range(),
                    glyphs: glyph_run
                        .positioned_glyphs()
                        .map(|glyph| Glyph {
                            id: glyph.id,
                            x: glyph.x,
                            y: glyph.y,
                        })
                        .collect(),
                });
            }
        }
    }
}

/// The families of `stack`, then `extra`, as parley names them.
fn named<'a>(stack: &'a [String], extra: Option<&'a String>) -> Vec<FontFamilyName<'a>> {
    stack
        .iter()
        .chain(extra)
        .map(|name| FontFamilyName::Named(Cow::Borrowed(name)))
        .collect()
}

/// `start` with `ellipsis`, the spaces before it left out.
fn cut_text(start: &str, ellipsis: &str) -> String {
    let mut text = start.trim_end().to_owned();
    text.push_str(ellipsis);
    text
}

/// The ellipsis, or three full stops when the fonts of `style` have no glyph for it but have one
/// for the full stop, as some fonts of games; the ellipsis, missing, when they have neither.
fn ellipsis(
    fonts: &mut Fonts,
    style: &TextStyle,
    scale: f32,
    direction: BaseDirection,
) -> &'static str {
    let mut drawn = |text: &str| {
        let mut alone = fonts.shape(text, style, scale, direction);
        alone.break_all_lines(None);
        let mut glyphs = alone
            .lines()
            .flat_map(|line| line.runs().collect::<Vec<_>>())
            .flat_map(|run| {
                run.clusters()
                    .flat_map(|cluster| cluster.glyphs())
                    .collect::<Vec<_>>()
            })
            .peekable();
        glyphs.peek().is_some() && glyphs.all(|glyph| glyph.id != 0)
    };
    if drawn(ELLIPSIS) || !drawn(FULL_STOPS) {
        ELLIPSIS
    } else {
        FULL_STOPS
    }
}

fn cursor(layout: &Layout<()>, position: TextPosition) -> Cursor {
    let affinity = match position.affinity {
        Affinity::Downstream => ParleyAffinity::Downstream,
        Affinity::Upstream => ParleyAffinity::Upstream,
    };
    Cursor::from_byte_index(layout, position.index, affinity)
}

fn rect(bounds: BoundingBox) -> Rect {
    Rect::new(
        bounds.x0 as f32,
        bounds.y0 as f32,
        (bounds.x1 - bounds.x0) as f32,
        (bounds.y1 - bounds.y0) as f32,
    )
}

fn alignment(alignment: Alignment) -> ParleyAlignment {
    match alignment {
        Alignment::Start => ParleyAlignment::Start,
        Alignment::End => ParleyAlignment::End,
        Alignment::Center => ParleyAlignment::Center,
        Alignment::Justify => ParleyAlignment::Justify,
    }
}

fn direction(direction: Direction) -> BaseDirection {
    match direction {
        Direction::Auto => BaseDirection::Auto,
        Direction::LeftToRight => BaseDirection::Ltr,
        Direction::RightToLeft => BaseDirection::Rtl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test font of Latin, Noto Sans, registered alone.
    fn latin(fonts: &mut Fonts) {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fonts/NotoSans.ttf");
        fonts
            .register(std::fs::read(path).expect("a test font"))
            .expect("a font");
    }

    fn style(family: &str) -> TextStyle {
        TextStyle {
            fonts: vec![family.to_owned()],
            locale: Some("en".to_owned()),
            ..TextStyle::default()
        }
    }

    #[test]
    fn without_the_system_s_fonts_what_no_font_has_is_missing() {
        let mut fonts = Fonts::new();
        latin(&mut fonts);
        let style = style("Noto Sans");
        // Georgian, which Noto Sans does not hold.
        let layout = fonts.lay_out("ქართული", &style, 1.0, TextBounds::default());
        let glyphs: Vec<u32> = layout
            .runs()
            .iter()
            .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.id))
            .collect();
        assert_eq!(glyphs, [0; 7]);
        assert!(fonts.found("ქართული", &style.fonts, Some("en")).is_empty());
    }

    #[test]
    fn a_stack_of_families_that_the_fonts_lack_draws_nothing() {
        let mut fonts = Fonts::new();
        latin(&mut fonts);
        let layout = fonts.lay_out("Docked", &style("Noto Sanss"), 1.0, TextBounds::default());
        assert!(layout.runs().is_empty());
        assert_eq!(layout.width(), 0.0);
    }

    /// The family of the first run of `text`, laid out in a stack that names no font.
    fn fallen_back(fonts: &mut Fonts, text: &str) -> Option<String> {
        let layout = fonts.lay_out(text, &style("No Such Family"), 1.0, TextBounds::default());
        let run = layout.runs().first()?;
        run.glyphs
            .iter()
            .all(|glyph| glyph.id != 0)
            .then(|| fonts.family(run.font).to_owned())
    }

    /// Where the system has fonts for Greek and Japanese: fontconfig's on Linux, which the dev
    /// shell loads, and DirectWrite's on Windows. Without them, the test says so and checks
    /// nothing.
    #[test]
    fn the_system_s_font_for_a_writing_system_is_found_once_whatever_came_before() {
        let greek = "Ελληνικά";
        let mut alone = Fonts::with_system_fonts();
        let Some(family) = fallen_back(&mut alone, greek) else {
            eprintln!("the system gives no font for Greek here: nothing to fall back on");
            return;
        };
        let mut after = Fonts::with_system_fonts();
        if fallen_back(&mut after, "日本語のテキスト").is_none() {
            eprintln!("the system gives no font for Japanese here: nothing to fall back on");
            return;
        }
        assert_eq!(fallen_back(&mut after, greek).as_ref(), Some(&family));
        // Named for Greek from then on, for that language: the system is not asked again.
        let stack = ["No Such Family".to_owned()];
        assert_eq!(
            after.found(greek, &stack, Some("en")),
            [(0..greek.len(), family)]
        );
        assert!(after.found(greek, &stack, Some("de")).is_empty());
    }
}
