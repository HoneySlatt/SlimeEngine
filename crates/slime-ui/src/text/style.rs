//! How a text is drawn, and the box it is laid out in.

use std::num::NonZeroUsize;

use slime_reflect::Reflect;

/// How a text is drawn: its fonts, size, weight and slant, the height of its lines, how its lines
/// align and in which direction it is written, and its language.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "UiTextStyle", default)]
pub struct TextStyle {
    /// The font families, by name, in order of preference: a character takes the first font
    /// that has it ([`Fonts`](super::Fonts)).
    pub fonts: Vec<String>,
    /// The size, the font's em, in logical pixels: a layout's scale turns it into pixels of the
    /// screen.
    pub size: f32,
    /// The weight, from 100, thin, to 900, black: 400 is regular, 700 bold. A variable font
    /// draws it on its weight axis; a font of fixed weight is taken as the closest that the family
    /// has, which is not made bolder when it is lighter.
    pub weight: f32,
    /// Whether the text is italic: drawn with the family's italic face. A family without one draws
    /// it upright, its slant not made up.
    pub italic: bool,
    /// The height of a line, as a multiple of the height that the fonts give it, their ascent,
    /// descent and gap: 1.0 for the fonts' own.
    pub line_height: f32,
    /// Where lines go across the box's width.
    pub alignment: Alignment,
    /// The direction the text is written in, which sets where a line starts.
    pub direction: Direction,
    /// The language, a BCP 47 tag, `ja` or `zh-CN`: it chooses the forms that a font has for it,
    /// and the fonts that the system's fallback finds. None for no language.
    pub locale: Option<String>,
}

impl Default for TextStyle {
    /// No fonts, 14 pixels, regular, upright, the fonts' line height, at the start, the direction
    /// of the text, no language.
    fn default() -> Self {
        Self {
            fonts: Vec::new(),
            size: 14.0,
            weight: 400.0,
            italic: false,
            line_height: 1.0,
            alignment: Alignment::Start,
            direction: Direction::Auto,
            locale: None,
        }
    }
}

/// Where the lines of a text go across its box: by the start and the end of a line, which are
/// its left and its right in a text written from left to right, and the reverse in one written
/// from right to left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
#[reflect(name = "UiTextAlignment", default)]
pub enum Alignment {
    /// Lines begin at the start.
    #[default]
    Start,
    /// Lines end at the end.
    End,
    /// Lines are centred.
    Center,
    /// Lines but the last fill the width, their spaces stretched.
    Justify,
}

/// The direction a text is written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
#[reflect(name = "UiTextDirection", default)]
pub enum Direction {
    /// The direction of the text's first letter that has one: right to left for Arabic or
    /// Hebrew, left to right for Latin.
    #[default]
    Auto,
    /// Left to right, whatever the text.
    LeftToRight,
    /// Right to left, whatever the text.
    RightToLeft,
}

/// The box that a text is laid out in: its width, past which lines break, and the most lines that
/// it shows. A text that holds more lines than that, or whose last line shown is wider than the
/// box, is cut, and ends in an ellipsis, `…`, or in three full stops when its fonts lack the
/// ellipsis and have the full stop. A line break that ends the text adds no line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextBounds {
    /// The width in logical pixels; none for lines that break only where the text does.
    pub width: Option<f32>,
    /// The most lines shown; none for all of them. It cuts nothing without a width.
    pub max_lines: Option<NonZeroUsize>,
}

impl TextBounds {
    /// A box `width` logical pixels wide, as many lines as the text takes.
    pub fn width(width: f32) -> Self {
        Self {
            width: Some(width),
            max_lines: None,
        }
    }

    /// A box of one line, `width` logical pixels wide: a label.
    pub fn line(width: f32) -> Self {
        Self {
            width: Some(width),
            max_lines: NonZeroUsize::new(1),
        }
    }
}
