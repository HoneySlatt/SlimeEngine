#![doc = include_str!("../README.md")]

mod atlas;

use std::collections::HashMap;

use slime_gfx::{Gpu, RenderTarget};
use slime_render::{
    Canvas, CanvasBox, CanvasGradient, CanvasImage, CanvasShadow, CanvasStats, ImagePart,
    ImageQuad, PixelRect, RenderError, Renderer,
};
use slime_ui::Rect;
use slime_ui::display::{DisplayList, ImageKey, ImageKind, Primitive};
use slime_ui::text::Fonts;

pub use atlas::{ATLAS_PAGE_SIZE, GLYPH_ATLAS_BUDGET};

use atlas::{Atlas, Placed};

/// Draws display lists ([`DisplayList`]) through the renderer's canvas
/// ([`Renderer::draw_canvas`]), the glyphs of their texts kept in an atlas: textures that hold
/// each glyph's image once, rasterized the first time a draw asks for it, in pages of coverage
/// and pages of colors for emoji, within a budget of memory, the glyphs that no draw asked for the
/// longest leaving first (`docs/adr/0139-text-drawn-and-the-display-list.md`). Past the budget, a
/// glyph that finds no room takes the place of the glyphs of its kind that no draw asked for the
/// longest, those of the draw under way kept; then pages of either kind that the draw draws nothing
/// from are let go, so that pages of one kind never keep the other out. A glyph that still finds no
/// room is left out of that draw, and one larger than a page of every draw ([`UiStats`]).
///
/// It holds GPU objects: after a lost device, a new one draws again, rasterizing its glyphs
/// again, as the renderer is made again. It is to be used with the same [`Fonts`] each time: its
/// atlas knows a glyph by the face that those fonts number.
pub struct UiRenderer {
    atlas: Atlas,
}

/// What [`UiRenderer::draw`] drew, and what it cost.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiStats {
    /// What the canvas drew, its GPU time among it.
    pub canvas: CanvasStats,
    /// The glyphs given to the canvas, whose clips may still leave them out.
    pub glyphs: usize,
    /// The glyphs that this draw rasterized: those that the atlas did not know, whether they then
    /// found room, drew nothing, or were left out.
    pub rasterized: usize,
    /// The glyphs left out, which the atlas had no room for within its budget, the glyphs of this
    /// draw kept, or larger than a page; a warning says so the first time.
    pub left_out_glyphs: usize,
    /// The image items left out, whose images the program did not give.
    pub missing_images: usize,
    /// The memory that the atlas's pages take.
    pub atlas_bytes: u64,
}

impl UiRenderer {
    /// A renderer of display lists whose atlas takes at most `budget` bytes of GPU memory, in
    /// pages of [`ATLAS_PAGE_SIZE`] texels a side, made as glyphs need them: by default
    /// [`GLYPH_ATLAS_BUDGET`].
    pub fn new(budget: u64) -> Self {
        Self {
            atlas: Atlas::new(budget),
        }
    }

    /// Draws `list` over what `target` holds, through `renderer`'s canvas: its texts' glyphs from
    /// the atlas, rasterized with `fonts`, the fonts that laid them out, and its images as
    /// `images` gives them for their keys. A text's origin is rounded to a whole pixel, so that its
    /// glyphs, placed at whole pixels within their layout, are drawn texel for pixel. An image
    /// that `images` does not give is left out, as a glyph that the atlas has no room for is
    /// ([`UiStats`]).
    ///
    /// # Errors
    ///
    /// [`RenderError::Gfx`] with [`slime_gfx::GfxError::OutOfMemory`] if the GPU cannot hold an
    /// atlas page or the canvas's items, and [`slime_gfx::GfxError::DeviceLost`] once the device
    /// is lost: nothing is drawn then.
    ///
    /// # Panics
    ///
    /// As [`Renderer::draw_canvas`]: if `target` does not have the renderer's format, or holds
    /// several samples per pixel. If `fonts` are not those that this renderer drew with before,
    /// and number fewer faces: they must be the same each time.
    pub fn draw<'i>(
        &mut self,
        gpu: &Gpu,
        renderer: &mut Renderer,
        fonts: &Fonts,
        list: &DisplayList,
        mut images: impl FnMut(ImageKey) -> Option<CanvasImage<'i>>,
        target: &RenderTarget,
    ) -> Result<UiStats, RenderError> {
        // The atlas first, which rasterizes and writes what the texts lack, then the canvas, which
        // reads its pages.
        self.atlas.begin();
        let mut placed = Placed::default();
        let mut places = Vec::new();
        for item in list.items() {
            if let Primitive::Text(text) = &item.primitive {
                for run in text.layout.runs() {
                    for glyph in run.placed() {
                        places.push(self.atlas.place(gpu, fonts, glyph.key, &mut placed)?);
                    }
                }
            }
        }
        let atlas = &self.atlas;
        let mut canvas = Canvas::new();
        let mut pages = vec![None; atlas.page_count()];
        for (index, texture) in atlas.textures() {
            pages[index] = Some(canvas.add_image(CanvasImage::Texture(texture)));
        }
        let mut given = HashMap::new();
        let mut places = places.into_iter();
        let (mut glyphs, mut missing_images) = (0, 0);
        let page_side = ATLAS_PAGE_SIZE as f32;
        for item in list.items() {
            let clip = item.clip.map(pixel_rect);
            match &item.primitive {
                Primitive::Box(shape) => canvas.draw_box(&CanvasBox {
                    rect: pixel_rect(shape.rect),
                    radii: shape.radii,
                    fill: shape.fill,
                    border_width: shape.border_width,
                    border: shape.border,
                    clip,
                }),
                Primitive::Gradient(gradient) => canvas.draw_gradient(&CanvasGradient {
                    rect: pixel_rect(gradient.rect),
                    radii: gradient.radii,
                    corners: gradient.corners,
                    clip,
                }),
                Primitive::Shadow(shadow) => canvas.draw_shadow(&CanvasShadow {
                    rect: pixel_rect(shadow.rect),
                    radii: shadow.radii,
                    color: shadow.color,
                    offset: shadow.offset,
                    blur: shadow.blur,
                    spread: shadow.spread,
                    clip,
                }),
                Primitive::Text(text) => {
                    let (x, y) = (text.origin[0].round(), text.origin[1].round());
                    for run in text.layout.runs() {
                        for glyph in run.placed() {
                            let Some(place) = places.next().flatten() else {
                                continue;
                            };
                            glyphs += 1;
                            canvas.draw_image(&ImageQuad {
                                rect: PixelRect::new(
                                    x + (glyph.x + place.left) as f32,
                                    y + (glyph.y - place.top) as f32,
                                    place.width as f32,
                                    place.height as f32,
                                ),
                                image: pages[place.page].expect("a page that a glyph lies in"),
                                part: ImagePart {
                                    x: place.x as f32 / page_side,
                                    y: place.y as f32 / page_side,
                                    width: place.width as f32 / page_side,
                                    height: place.height as f32 / page_side,
                                },
                                kind: if place.color {
                                    slime_render::ImageKind::Color
                                } else {
                                    slime_render::ImageKind::Coverage
                                },
                                color: text.color,
                                clip,
                            });
                        }
                    }
                }
                Primitive::Image(image) => {
                    let id = *given.entry(image.image).or_insert_with(|| {
                        images(image.image).map(|given| canvas.add_image(given))
                    });
                    let Some(id) = id else {
                        missing_images += 1;
                        continue;
                    };
                    canvas.draw_image(&ImageQuad {
                        rect: pixel_rect(image.rect),
                        image: id,
                        part: ImagePart {
                            x: image.part.x,
                            y: image.part.y,
                            width: image.part.width,
                            height: image.part.height,
                        },
                        kind: match image.kind {
                            ImageKind::Coverage => slime_render::ImageKind::Coverage,
                            ImageKind::Color => slime_render::ImageKind::Color,
                        },
                        color: image.color,
                        clip,
                    });
                }
            }
        }
        let canvas_stats = renderer.draw_canvas(gpu, &canvas, target)?;
        Ok(UiStats {
            canvas: canvas_stats,
            glyphs,
            rasterized: placed.rasterized,
            left_out_glyphs: placed.left_out,
            missing_images,
            atlas_bytes: atlas.bytes(),
        })
    }
}

impl Default for UiRenderer {
    /// A renderer whose atlas takes at most [`GLYPH_ATLAS_BUDGET`].
    fn default() -> Self {
        Self::new(GLYPH_ATLAS_BUDGET)
    }
}

/// `rect` as the canvas takes it.
fn pixel_rect(rect: Rect) -> PixelRect {
    PixelRect::new(rect.x, rect.y, rect.width, rect.height)
}
