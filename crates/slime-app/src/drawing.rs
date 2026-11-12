//! The bridge through which the engine draws uwui
//! (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`): a display list, or a
//! canvas, drawn by `uwui-render` over one of the engine's targets, with the engine's images.

use slime_gfx::{GfxError, Gpu, RenderTarget, Texture, TextureFormat};
use uwui::display::{DisplayList, ImageKey};
use uwui::text::Fonts;
use uwui_render::canvas::{Canvas, CanvasImage, CanvasStats};
use uwui_render::{UiRenderer, UiStats};

/// What the engine, as uwui's host, draws uwui with: the dev overlay, the editor's window, the
/// examples and the benchmarks draw their interfaces through it
/// (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`). It draws a display list
/// ([`InterfaceRenderer::draw`]), or a canvas that a program fills itself
/// ([`InterfaceRenderer::draw_canvas`]), over a target of one format, with the engine's images,
/// textures and targets that views drew ([`InterfaceImage`]), through `uwui-render`'s
/// [`UiRenderer`], whose glyphs' atlas it keeps. `uwui-render` draws with `slime-gfx`'s handles
/// for now; once it draws on wgpu, at milestone M6d.4b1b (`docs/stage-6d.md`), the engine lends it
/// its device here, through `slime-gfx`'s guarded door.
///
/// It holds GPU objects of the device it was made with: after a lost device, it is dropped before
/// the device is replaced, and one made with the new device draws again, rasterizing its glyphs
/// again. It is to be used with the same [`Fonts`] each time.
pub struct InterfaceRenderer {
    renderer: UiRenderer,
}

/// An image of the engine that an interface draws: a texture, or a render target that a view drew,
/// as an editor's viewport shows; how each is drawn is [`CanvasImage`]'s contract. It names the
/// engine's handles, so that the programs that draw through the bridge keep it when `uwui-render`
/// takes wgpu's views in their place, at milestone M6d.4b1b.
#[derive(Clone, Copy, Debug)]
pub enum InterfaceImage<'a> {
    /// A texture: of one channel, `R8Unorm`, how much of each texel a shape covers, drawn in a
    /// color; of colors, drawn as it is.
    Texture(&'a Texture),
    /// A render target, of one sample per pixel, that an earlier pass drew into. It cannot be the
    /// target drawn into.
    Target(&'a RenderTarget),
}

impl InterfaceRenderer {
    /// What draws interfaces into targets of `format`, the window's
    /// ([`Context::surface_format`](crate::Context::surface_format)) for a windowed program, its
    /// glyphs' atlas within `uwui-render`'s default budget
    /// ([`uwui_render::GLYPH_ATLAS_BUDGET`]).
    ///
    /// # Errors
    ///
    /// As [`UiRenderer::new`]: [`GfxError::OutOfMemory`] if the GPU cannot hold its objects,
    /// [`GfxError::Shader`] or [`GfxError::Pipeline`] if the driver refuses the canvas's shader or
    /// pipeline, and [`GfxError::DeviceLost`] once the device is lost.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, GfxError> {
        Ok(Self {
            renderer: UiRenderer::new(gpu, format)?,
        })
    }

    /// Draws `list` over what `target` holds, its texts with `fonts`, the fonts that laid them
    /// out, and its images as `images` gives them for their keys: [`UiRenderer::draw`], whose
    /// statistics it returns. An image that `images` does not give is left out.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU cannot hold an atlas page or the canvas's items, and
    /// [`GfxError::DeviceLost`] once the device is lost: nothing is drawn then.
    ///
    /// # Panics
    ///
    /// As [`UiRenderer::draw`]: if `target` does not have this renderer's format, or holds several
    /// samples per pixel; if `fonts` are not those that it drew with before, and number fewer
    /// faces.
    pub fn draw<'i>(
        &mut self,
        gpu: &Gpu,
        fonts: &Fonts,
        list: &DisplayList,
        mut images: impl FnMut(ImageKey) -> Option<InterfaceImage<'i>>,
        target: &RenderTarget,
    ) -> Result<UiStats, GfxError> {
        self.renderer.draw(
            gpu,
            fonts,
            list,
            |key| images(key).map(canvas_image),
            target,
        )
    }

    /// Draws `canvas` over what `target` holds, `images` the images that its quads name by their
    /// index: [`UiRenderer::draw_canvas`], whose statistics it returns.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU cannot hold the canvas's items, and
    /// [`GfxError::DeviceLost`] once the device is lost: nothing is drawn then.
    ///
    /// # Panics
    ///
    /// As [`UiRenderer::draw_canvas`]: if `target` does not have this renderer's format, or holds
    /// several samples per pixel; if an item drawn reads an image that is not below
    /// `images.len()`.
    pub fn draw_canvas(
        &mut self,
        gpu: &Gpu,
        canvas: &Canvas,
        images: &[InterfaceImage<'_>],
        target: &RenderTarget,
    ) -> Result<CanvasStats, GfxError> {
        let images: Vec<CanvasImage<'_>> = images.iter().copied().map(canvas_image).collect();
        self.renderer.draw_canvas(gpu, canvas, &images, target)
    }
}

/// `image` as `uwui-render` takes it.
fn canvas_image(image: InterfaceImage<'_>) -> CanvasImage<'_> {
    match image {
        InterfaceImage::Texture(texture) => CanvasImage::Texture(texture),
        InterfaceImage::Target(target) => CanvasImage::Target(target),
    }
}
