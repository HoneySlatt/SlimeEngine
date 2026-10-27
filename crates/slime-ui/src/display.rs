//! What an interface draws in a frame, as data that knows no GPU
//! (`docs/adr/0139-text-drawn-and-the-display-list.md`).

use std::sync::Arc;

use slime_color::LinearColor;

use crate::Rect;
use crate::text::TextLayout;

/// What an interface draws in a frame: boxes, their shadows, texts and images, in the order they
/// are pushed, each over those before it (`docs/adr/0132-the-engines-ui-framework.md`,
/// `docs/adr/0139-text-drawn-and-the-display-list.md`). It is owned data, `Send` and `'static`, so
/// that a frame built on one thread is drawn on another, as the frame contract asks
/// (`docs/adr/0111-the-frame-contract.md`); its texts are shared with the
/// [`TextCache`](crate::text::TextCache) that laid them out. Coordinates are pixels of the screen,
/// from its top left corner, y down: an interface lays itself out at the screen's scale.
/// `slime-ui-render` draws it through the renderer's canvas.
///
/// ```
/// use slime_color::LinearColor;
/// use slime_ui::Rect;
/// use slime_ui::display::{BoxPrimitive, DisplayList};
///
/// let mut list = DisplayList::new();
/// list.push(
///     BoxPrimitive {
///         rect: Rect::new(8.0, 8.0, 200.0, 40.0),
///         radii: [6.0; 4],
///         fill: LinearColor::rgb(0.1, 0.1, 0.12),
///         border_width: 1.0,
///         border: LinearColor::rgb(0.3, 0.3, 0.35),
///     },
///     None,
/// );
/// assert_eq!(list.len(), 1);
/// ```
#[derive(Clone, Debug, Default)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
}

impl DisplayList {
    /// An empty display list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `primitive`, cut to `clip` when there is one, at the identity transform and in the
    /// root layer.
    pub fn push(&mut self, primitive: impl Into<Primitive>, clip: Option<Rect>) {
        self.items.push(DisplayItem {
            primitive: primitive.into(),
            clip,
            transform: TransformId::IDENTITY,
            layer: LayerId::ROOT,
        });
    }

    /// Its items, in the order they are drawn.
    pub fn items(&self) -> &[DisplayItem] {
        &self.items
    }

    /// How many items it holds.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether it draws nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// An item of a [`DisplayList`]: what it draws, the rectangle it is cut to, its transform and its
/// layer.
#[derive(Clone, Debug)]
pub struct DisplayItem {
    /// What it draws.
    pub primitive: Primitive,
    /// The rectangle outside which it is not drawn; none for the whole screen.
    pub clip: Option<Rect>,
    /// How it is placed: the identity until Stage 7 gives the interface its transforms
    /// (`docs/adr/0132-the-engines-ui-framework.md`).
    pub transform: TransformId,
    /// The layer it is drawn in: the root's until Stage 7.
    pub layer: LayerId,
}

/// What an item draws.
#[derive(Clone, Debug)]
pub enum Primitive {
    /// A box.
    Box(BoxPrimitive),
    /// The shadow that a box casts.
    Shadow(ShadowPrimitive),
    /// A box filled with a gradient.
    Gradient(GradientPrimitive),
    /// A text.
    Text(TextPrimitive),
    /// A part of an image.
    Image(ImagePrimitive),
}

impl Primitive {
    /// The rectangle it covers, in pixels, before its clip: a box's, the box that casts a shadow,
    /// without the blur that spreads past it, the box that a text's glyphs lie in, empty for an
    /// empty text, an image's.
    pub(crate) fn bounds(&self) -> Rect {
        match self {
            Self::Box(primitive) => primitive.rect,
            Self::Shadow(primitive) => primitive.rect,
            Self::Gradient(primitive) => primitive.rect,
            Self::Text(primitive) => {
                let drawn = primitive.layout.drawn_box();
                Rect::new(
                    primitive.origin[0] + drawn.x,
                    primitive.origin[1] + drawn.y,
                    drawn.width,
                    drawn.height,
                )
            }
            Self::Image(primitive) => primitive.rect,
        }
    }

    /// It drawn `opacity` as opaque: the alpha of its colors multiplied by it.
    pub(crate) fn faded(mut self, opacity: f32) -> Self {
        if opacity >= 1.0 {
            return self;
        }
        match &mut self {
            Self::Box(primitive) => {
                primitive.fill.a *= opacity;
                primitive.border.a *= opacity;
            }
            Self::Shadow(primitive) => primitive.color.a *= opacity,
            Self::Gradient(primitive) => {
                for corner in &mut primitive.corners {
                    corner.a *= opacity;
                }
            }
            Self::Text(primitive) => primitive.color.a *= opacity,
            Self::Image(primitive) => primitive.color.a *= opacity,
        }
        self
    }

    /// It placed on whole pixels of the screen, so that its edges stay crisp at every scale: a box's
    /// or an image's edges rounded to the nearest pixel, a box's border to whole pixels, one at
    /// least. A text's origin is rounded where it is drawn, its glyphs placed within it; a shadow,
    /// blurred, is left as it is.
    pub(crate) fn snapped(mut self) -> Self {
        match &mut self {
            Self::Box(primitive) => {
                primitive.rect = snap(primitive.rect);
                if primitive.border_width > 0.0 {
                    primitive.border_width = primitive.border_width.round().max(1.0);
                }
            }
            Self::Image(primitive) => primitive.rect = snap(primitive.rect),
            Self::Gradient(primitive) => primitive.rect = snap(primitive.rect),
            Self::Shadow(_) | Self::Text(_) => {}
        }
        self
    }
}

/// `rect` with its edges on the nearest whole pixels.
pub(crate) fn snap(rect: Rect) -> Rect {
    let (left, top) = (rect.x.round(), rect.y.round());
    let right = (rect.x + rect.width).round();
    let bottom = (rect.y + rect.height).round();
    Rect::new(left, top, right - left, bottom - top)
}

/// A box: a rectangle with rounded corners, filled, and bordered inside its edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxPrimitive {
    /// Where it lies.
    pub rect: Rect,
    /// The radii of its corners, in pixels: top left, top right, bottom right, bottom left.
    pub radii: [f32; 4],
    /// Its fill, inside its border.
    pub fill: LinearColor,
    /// Its border's width, in pixels, inside its edges: none at 0.
    pub border_width: f32,
    /// Its border's color.
    pub border: LinearColor,
}

/// A box filled with a gradient: a rectangle with rounded corners, its corners' colors mixed across
/// it by each point's place, as the interface blends, on their sRGB encoding
/// (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`). It has no border.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientPrimitive {
    /// Where it lies.
    pub rect: Rect,
    /// The radii of its corners, in pixels: top left, top right, bottom right, bottom left.
    pub radii: [f32; 4],
    /// Its corners' colors: top left, top right, bottom right, bottom left.
    pub corners: [LinearColor; 4],
}

/// The shadow that a box casts, drawn under it: the box, moved, grown and blurred.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowPrimitive {
    /// The box that casts it.
    pub rect: Rect,
    /// The radii of the box's corners, in pixels.
    pub radii: [f32; 4],
    /// The shadow's color, its alpha its darkness where it is whole.
    pub color: LinearColor,
    /// How far the shadow lies from the box, in pixels, right and down.
    pub offset: [f32; 2],
    /// How far its edge fades, in pixels: twice the standard deviation of the Gaussian that blurs
    /// it, as CSS's `box-shadow` measures it.
    pub blur: f32,
    /// How much larger than the box the shadow is on each side, in pixels, before the blur.
    pub spread: f32,
}

/// A text laid out, drawn in one color.
#[derive(Clone, Debug)]
pub struct TextPrimitive {
    /// Where the top left corner of its layout's box lies.
    pub origin: [f32; 2],
    /// The text, laid out at the screen's scale.
    pub layout: Arc<TextLayout>,
    /// The color of its glyphs; of a glyph drawn in color, an emoji, its alpha alone, which fades
    /// it.
    pub color: LinearColor,
}

/// A part of an image drawn into a rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePrimitive {
    /// Where it is drawn.
    pub rect: Rect,
    /// The image, which the program that draws the list names, and gives when it is drawn.
    pub image: ImageKey,
    /// The part of the image drawn, in fractions of its width and height from its top left corner.
    pub part: Rect,
    /// How its texels are drawn.
    pub kind: ImageKind,
    /// The color of a coverage, or, of an image of colors, its alpha alone, which fades it.
    pub color: LinearColor,
}

/// An image, as the program that builds a display list names it: the program that draws the list
/// gives the image of each key then, a texture it holds or one that its assets hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageKey(pub u64);

/// How an image's texels are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageKind {
    /// Each texel is how much of it a shape covers, drawn in the item's color: an icon of one
    /// color.
    Coverage,
    /// Each texel is a color, drawn as it is, faded by the alpha of the item's color.
    Color,
}

/// How an item is placed: the identity alone until Stage 7 gives the interface its transforms
/// (`docs/adr/0132-the-engines-ui-framework.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransformId(u32);

impl TransformId {
    /// The identity: an item stays where its coordinates put it.
    pub const IDENTITY: Self = Self(0);
}

/// The layer an item is drawn in: the root's alone until Stage 7 gives the interface its layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(u32);

impl LayerId {
    /// The root layer, which every item is drawn in.
    pub const ROOT: Self = Self(0);
}

impl From<BoxPrimitive> for Primitive {
    fn from(primitive: BoxPrimitive) -> Self {
        Self::Box(primitive)
    }
}

impl From<ShadowPrimitive> for Primitive {
    fn from(primitive: ShadowPrimitive) -> Self {
        Self::Shadow(primitive)
    }
}

impl From<TextPrimitive> for Primitive {
    fn from(primitive: TextPrimitive) -> Self {
        Self::Text(primitive)
    }
}

impl From<ImagePrimitive> for Primitive {
    fn from(primitive: ImagePrimitive) -> Self {
        Self::Image(primitive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_display_list_is_sent_to_another_thread() {
        fn sent<T: Send + Sync + 'static>() {}
        sent::<DisplayList>();
    }
}
