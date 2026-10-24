//! What an element looks like and how it is laid out: its style, typed, in logical pixels
//! (`docs/adr/0140-the-tree-of-elements.md`).
//!
//! A style is grouped by what a change makes again: its layout, which flexbox and grid lay out,
//! its paint, which only draws, and its text. Sides and corners are named by the start and the end
//! of a line, not left and right, so that a root laid out from right to left mirrors them.

use slime_color::LinearColor;

use crate::text::TextStyle;

/// How an element is laid out and drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    /// How it and its children are laid out.
    pub layout: LayoutStyle,
    /// How it is drawn, which lays nothing out again.
    pub paint: PaintStyle,
    /// The text of a label or a button: its fonts and size, which lay it out, and its color.
    pub text: TextLook,
}

/// How an element and its children are laid out, by CSS's flexbox or grid, in logical pixels.
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
}

impl Default for PaintStyle {
    fn default() -> Self {
        Self {
            background: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
            border_width: 0.0,
            border: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
            radii: [0.0; 4],
            shadow: None,
        }
    }
}

/// The shadow that an element's box casts.
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// The text of a label or a button.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLook {
    /// Its fonts, size, weight and alignment, in logical pixels.
    pub style: TextStyle,
    /// Its color.
    pub color: LinearColor,
}

impl Default for TextLook {
    fn default() -> Self {
        Self {
            style: TextStyle::default(),
            color: LinearColor::WHITE,
        }
    }
}
