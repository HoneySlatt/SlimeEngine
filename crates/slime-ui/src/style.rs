//! What an element looks like and how it is laid out: its style, typed, in logical pixels
//! (`docs/adr/0140-the-tree-of-elements.md`).
//!
//! A style is grouped by what a change makes again: its layout, which flexbox and grid lay out,
//! its paint, which only draws but for its border's width, inside which what scrolls is cut, and
//! its text. Sides and corners are named by the start and the end
//! of a line, not left and right, so that a root laid out from right to left mirrors them.

use slime_color::LinearColor;
use slime_reflect::Reflect;

use crate::text::TextStyle;

/// How an element is laid out and drawn.
#[derive(Clone, Debug, Default, PartialEq, Reflect)]
#[reflect(name = "UiStyle", default)]
pub struct Style {
    /// How it and its children are laid out.
    pub layout: LayoutStyle,
    /// How it is drawn, which lays nothing out again.
    pub paint: PaintStyle,
    /// The text of a label or a button: its fonts and size, which lay it out, and its color.
    pub text: TextLook,
    /// The parts of a control that are not its box: a check box's mark, a slider's track and
    /// thumb.
    pub control: ControlStyle,
    /// How a change of its style shows: at once, or over a time
    /// (`docs/adr/0149-the-looks-foundations.md`).
    pub transition: Transition,
}

/// How a change of an element's style shows: its colors and its opacity going from what they showed
/// to what the new style gives over `duration`, along `easing`, as its state changes under the
/// pointer, and a section growing or shrinking to its height as it opens or folds. Its layout, its
/// text's fonts and size, and its border's width change at once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(name = "UiTransition", default)]
pub struct Transition {
    /// How long a change takes, in seconds: 0, the default, for at once.
    pub duration: f32,
    /// How it goes over that time.
    pub easing: Easing,
}

/// How a transition goes from its start to its end over its time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(name = "UiEasing")]
pub enum Easing {
    /// At one speed.
    Linear,
    /// Fast, then slowing to its end: what a change that answers the user does.
    #[default]
    EaseOut,
    /// Slow at both ends: what a change that moves things does.
    EaseInOut,
}

impl Easing {
    /// How far a transition has gone, from 0 to 1, when `time`, from 0 to 1, of it has passed.
    #[must_use]
    pub fn at(self, time: f32) -> f32 {
        let time = time.clamp(0.0, 1.0);
        match self {
            Self::Linear => time,
            // A cubic that leaves at three times the linear speed and stops at its end.
            Self::EaseOut => 1.0 - (1.0 - time).powi(3),
            Self::EaseInOut => {
                if time < 0.5 {
                    4.0 * time * time * time
                } else {
                    1.0 - (-2.0 * time + 2.0).powi(3) / 2.0
                }
            }
        }
    }
}

/// How an element and its children are laid out, by CSS's flexbox or grid, in logical pixels.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "UiLayout", default)]
pub struct LayoutStyle {
    /// How its children are laid out, or whether it is laid out at all.
    pub display: Display,
    /// The axis along which a flexbox lays its children out, and their order.
    pub direction: FlexDirection,
    /// Whether a flexbox's children wrap onto more lines when they do not fit.
    pub wrap: bool,
    /// How children share the space along the main axis.
    pub justify: Justify,
    /// How children are placed across the main axis, unless they say otherwise.
    pub align_items: Align,
    /// How this element is placed across its parent's main axis; its parent's `align_items` when
    /// none.
    pub align_self: Option<Align>,
    /// The space between rows and between columns.
    pub gap: [f32; 2],
    /// The space inside its edges, around its children.
    pub padding: Sides,
    /// The space outside its edges.
    pub margin: Sides,
    /// Its width and height.
    pub size: [Dimension; 2],
    /// Its smallest width and height.
    pub min_size: [Dimension; 2],
    /// Its largest width and height.
    pub max_size: [Dimension; 2],
    /// How much of the space left along its parent's main axis it takes, against its siblings.
    pub grow: f32,
    /// How much it gives up when its siblings lack space.
    pub shrink: f32,
    /// Its size along its parent's main axis before it grows or shrinks.
    pub basis: Dimension,
    /// A grid's columns, from the start.
    pub columns: Vec<Track>,
    /// A grid's rows, from the top.
    pub rows: Vec<Track>,
    /// Where it lies among its grid parent's columns.
    pub column: GridPlace,
    /// Where it lies among its grid parent's rows.
    pub row: GridPlace,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            direction: FlexDirection::Row,
            wrap: false,
            justify: Justify::Start,
            align_items: Align::Stretch,
            align_self: None,
            gap: [0.0; 2],
            padding: Sides::default(),
            margin: Sides::default(),
            size: [Dimension::Auto; 2],
            min_size: [Dimension::Auto; 2],
            max_size: [Dimension::Auto; 2],
            grow: 0.0,
            shrink: 1.0,
            basis: Dimension::Auto,
            columns: Vec::new(),
            rows: Vec::new(),
            column: GridPlace::default(),
            row: GridPlace::default(),
        }
    }
}

/// How an element's children are laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
#[reflect(name = "UiDisplay", default)]
pub enum Display {
    /// In a line along one axis, CSS's flexbox.
    #[default]
    Flex,
    /// In the cells of a grid, CSS's grid.
    Grid,
    /// Not at all: the element and its children take no space and are not drawn.
    None,
}

/// The axis along which a flexbox lays its children out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
#[reflect(name = "UiFlexDirection", default)]
pub enum FlexDirection {
    /// Along a line, from its start.
    #[default]
    Row,
    /// Down, from the top.
    Column,
    /// Along a line, from its end.
    RowReverse,
    /// Up, from the bottom.
    ColumnReverse,
}

/// How children share the space along the main axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
#[reflect(name = "UiJustify", default)]
pub enum Justify {
    /// Packed at the start.
    #[default]
    Start,
    /// Packed at the end.
    End,
    /// Packed in the middle.
    Center,
    /// The first at the start, the last at the end, the space shared between them.
    SpaceBetween,
    /// The space shared around each.
    SpaceAround,
    /// The space shared evenly between them and the edges.
    SpaceEvenly,
}

/// How a child is placed across the main axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
#[reflect(name = "UiAlign", default)]
pub enum Align {
    /// At the start.
    Start,
    /// At the end.
    End,
    /// In the middle.
    Center,
    /// Stretched to fill the line.
    #[default]
    Stretch,
}

/// A size: as its content or its parent makes it, in logical pixels, or a share of its parent's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(name = "UiDimension", default)]
pub enum Dimension {
    /// As the layout makes it.
    #[default]
    Auto,
    /// Logical pixels.
    Pixels(f32),
    /// A share of the parent's size, from 0 to 1.
    Share(f32),
}

/// The four sides of a box, by the start and the end of a line, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(name = "UiSides", default)]
pub struct Sides {
    /// The side where a line starts: left from left to right, right from right to left.
    pub start: f32,
    /// The side where a line ends.
    pub end: f32,
    /// The top.
    pub top: f32,
    /// The bottom.
    pub bottom: f32,
}

impl Sides {
    /// The same space on every side.
    pub const fn all(space: f32) -> Self {
        Self {
            start: space,
            end: space,
            top: space,
            bottom: space,
        }
    }

    /// `across` at the start and the end, `along` at the top and the bottom.
    pub const fn symmetric(across: f32, along: f32) -> Self {
        Self {
            start: across,
            end: across,
            top: along,
            bottom: along,
        }
    }
}

/// A track of a grid: a column or a row.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "UiTrack")]
pub enum Track {
    /// Logical pixels.
    Pixels(f32),
    /// A share of the space that the other tracks leave, against the other shares.
    Fraction(f32),
    /// As its content makes it.
    Auto,
}

/// Where a grid's child lies among its columns or rows: from a line, numbered from 1 at the start,
/// or after the children before it when none; across a number of tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
#[reflect(name = "UiGridPlace", default)]
pub struct GridPlace {
    /// The line it starts at.
    pub start: Option<u16>,
    /// How many tracks it spans, at least 1.
    pub span: u16,
}

impl Default for GridPlace {
    fn default() -> Self {
        Self {
            start: None,
            span: 1,
        }
    }
}

/// How an element is drawn: a box behind its children, which lays nothing out.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "UiPaint", default)]
pub struct PaintStyle {
    /// The box's fill; nothing is drawn where it is transparent and there is no border.
    pub background: LinearColor,
    /// The border's width, in logical pixels, inside the box's edges.
    pub border_width: f32,
    /// The border's color.
    pub border: LinearColor,
    /// The radii of its corners, in logical pixels: top start, top end, bottom end, bottom start.
    pub radii: [f32; 4],
    /// The shadow it casts, if any.
    pub shadow: Option<Shadow>,
    /// How opaque it and what it holds are drawn, from 0, unseen, to 1, the default: the alpha of
    /// each of their primitives is multiplied by it, and by their ancestors', so that parts of a
    /// translucent element that overlap show through each other.
    pub opacity: f32,
}

impl Default for PaintStyle {
    fn default() -> Self {
        Self {
            background: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
            border_width: 0.0,
            border: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
            radii: [0.0; 4],
            shadow: None,
            opacity: 1.0,
        }
    }
}

/// The shadow that an element's box casts.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "UiShadow", default)]
pub struct Shadow {
    /// Its color, its alpha its darkness where it is whole.
    pub color: LinearColor,
    /// How far it lies from the box, in logical pixels, towards the end and down.
    pub offset: [f32; 2],
    /// How far its edge fades, in logical pixels, as CSS's `box-shadow` measures it.
    pub blur: f32,
    /// How much larger than the box it is on each side, in logical pixels.
    pub spread: f32,
}

impl Default for Shadow {
    /// No shadow to see: transparent, under the box, sharp.
    fn default() -> Self {
        Self {
            color: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
            offset: [0.0; 2],
            blur: 0.0,
            spread: 0.0,
        }
    }
}

/// The parts of a control drawn apart from its box: a check box's box and its mark, a slider's
/// track, the part of the track up to its value and its thumb.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "UiControl", default)]
pub struct ControlStyle {
    /// The size of the part that the pointer takes, in logical pixels: the side of a check box's
    /// box, the width of a slider's thumb, the side of an item of a menu's mark and chevron, which
    /// lay them out.
    pub size: f32,
    /// The radius of its corners, in logical pixels.
    pub radius: f32,
    /// A check box's box unchecked, a slider's track, an item of a menu's shortcut.
    pub track: LinearColor,
    /// A check box's box checked, the part of a slider's track up to its value.
    pub fill: LinearColor,
    /// A check box's mark, a slider's thumb, an item of a menu's mark and chevron.
    pub mark: LinearColor,
}

impl Default for ControlStyle {
    fn default() -> Self {
        Self {
            size: 16.0,
            radius: 3.0,
            track: LinearColor::rgba(0.3, 0.3, 0.3, 1.0),
            fill: LinearColor::rgba(0.2, 0.4, 0.9, 1.0),
            mark: LinearColor::WHITE,
        }
    }
}

/// The text of a label, a button, a field, a check box's label, a section's title or a tab's.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "UiTextLook", default)]
pub struct TextLook {
    /// Its fonts, size, weight and alignment, in logical pixels.
    pub style: TextStyle,
    /// Its color, which a field's caret and the line under what an input method composes take
    /// too.
    pub color: LinearColor,
    /// The fill behind the text that a field holds selected.
    pub selection: LinearColor,
}

impl Default for TextLook {
    fn default() -> Self {
        Self {
            style: TextStyle::default(),
            color: LinearColor::WHITE,
            selection: LinearColor::rgba(0.2, 0.4, 0.9, 0.4),
        }
    }
}
