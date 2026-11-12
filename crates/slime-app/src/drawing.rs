//! The bridge through which the engine draws uwui
//! (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`): a display list, or a
//! canvas, drawn by `uwui-render` over one of the engine's targets, with the engine's images.

use slime_gfx::{GfxError, Gpu, GpuId, RenderTarget, Texture, TextureFormat, WgpuLoan};
use uwui::display::{DisplayList, ImageKey};
use uwui::text::Fonts;
use uwui_render::canvas::{Canvas, CanvasStats};
use uwui_render::{DrawContext, UiRenderer, UiStats};

/// The label of the encoder through which the bridge draws, in GPU capture tools and in
/// [`GfxError::OutOfMemory`].
const LABEL: &str = "interface";

/// What the engine, as uwui's host, draws uwui with: the dev overlay, the editor's window, the
/// examples and the benchmarks draw their interfaces through it
/// (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`). It draws a display list
/// ([`InterfaceRenderer::draw`]), or a canvas that a program fills itself
/// ([`InterfaceRenderer::draw_canvas`]), over a target of one format, with the engine's images,
/// textures and targets that views drew ([`InterfaceImage`]), through `uwui-render`'s
/// [`UiRenderer`], whose glyphs' atlas it keeps. `uwui-render` draws with wgpu, whose device the
/// engine lends it through `slime-gfx`'s guarded door ([`Gpu::with_wgpu`]): each draw records into
/// an encoder of its own, which the door submits, and pays the door's check of the device.
///
/// It holds GPU objects of the device it was made with: after a lost device, it is dropped before
/// the device is replaced, and one made with the new device draws again, rasterizing its glyphs
/// again. It is to be used with the same [`Fonts`] each time.
pub struct InterfaceRenderer {
    renderer: UiRenderer,
    /// The device that made its wgpu objects, which carry no identity of their own: each draw checks
    /// that it draws with that device, where wgpu, each of whose instances numbers its objects from
    /// zero, would act on another device's objects of the same numbers
    /// (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`).
    made_with: GpuId,
}

/// An image of the engine that an interface draws: a texture, or a render target that a view drew,
/// as an editor's viewport shows; how each is drawn is [`uwui_render::canvas::CanvasRenderer::draw`]'s
/// contract. It names the engine's handles, which the bridge gives `uwui-render` as wgpu's views,
/// each checked to be of the device that draws.
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
    /// [`GfxError::OutOfMemory`] if the GPU cannot hold its objects, [`GfxError::Shader`] or
    /// [`GfxError::Pipeline`] if the driver refuses the canvas's shader or pipeline, and
    /// [`GfxError::DeviceLost`] once the device is lost.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, GfxError> {
        let renderer = gpu.with_wgpu(LABEL, |loan| {
            UiRenderer::new(loan.device, loan.queue, WgpuLoan::texture_format(format))
        })?;
        Ok(Self {
            renderer: renderer.map_err(gfx_error)?,
            made_with: gpu.id(),
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
    /// faces. If this renderer, `target` or an image belongs to another `Gpu` than `gpu` and its
    /// clones.
    #[track_caller]
    pub fn draw<'i>(
        &mut self,
        gpu: &Gpu,
        fonts: &Fonts,
        list: &DisplayList,
        mut images: impl FnMut(ImageKey) -> Option<InterfaceImage<'i>>,
        target: &RenderTarget,
    ) -> Result<UiStats, GfxError> {
        gpu.id().owns(self.made_with, "an interface renderer");
        gpu.with_wgpu(LABEL, |loan| {
            let texture = loan.objects.target(target);
            let objects = &loan.objects;
            let view = |image| match image {
                InterfaceImage::Texture(texture) => objects.texture_view(texture),
                InterfaceImage::Target(target) => objects.target_view(target),
            };
            let mut context = DrawContext {
                device: loan.device,
                queue: loan.queue,
                encoder: loan.encoder,
            };
            self.renderer.draw(
                &mut context,
                fonts,
                list,
                |key| images(key).map(view),
                texture,
            )
        })?
        .map_err(gfx_error)
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
    /// `images.len()`. If this renderer, `target` or an image belongs to another `Gpu` than `gpu`
    /// and its clones.
    #[track_caller]
    pub fn draw_canvas(
        &mut self,
        gpu: &Gpu,
        canvas: &Canvas,
        images: &[InterfaceImage<'_>],
        target: &RenderTarget,
    ) -> Result<CanvasStats, GfxError> {
        gpu.id().owns(self.made_with, "an interface renderer");
        gpu.with_wgpu(LABEL, |loan| {
            let texture = loan.objects.target(target);
            let objects = &loan.objects;
            let views: Vec<_> = images
                .iter()
                .map(|&image| match image {
                    InterfaceImage::Texture(texture) => objects.texture_view(texture),
                    InterfaceImage::Target(target) => objects.target_view(target),
                })
                .collect();
            let mut context = DrawContext {
                device: loan.device,
                queue: loan.queue,
                encoder: loan.encoder,
            };
            self.renderer
                .draw_canvas(&mut context, canvas, &views, texture)
        })?
        .map_err(gfx_error)
    }
}

/// `error`, of `uwui-render`, as the engine's other GPU errors are given.
fn gfx_error(error: uwui_render::Error) -> GfxError {
    match error {
        uwui_render::Error::OutOfMemory { label } => GfxError::OutOfMemory { label },
        uwui_render::Error::Shader { label, message } => GfxError::Shader { label, message },
        uwui_render::Error::Pipeline { label, message } => GfxError::Pipeline { label, message },
    }
}

#[cfg(test)]
mod tests {
    use uwui::Rect;
    use uwui::display::BoxPrimitive;
    use uwui_render::canvas::{CanvasRenderer, SHADER};

    use super::*;

    const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

    fn gpu() -> Gpu {
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
    }

    /// A list of one opaque box over the whole of a target of 8 × 8 pixels.
    fn list() -> DisplayList {
        let mut list = DisplayList::new();
        list.push(
            BoxPrimitive {
                rect: Rect::new(0.0, 0.0, 8.0, 8.0),
                radii: [0.0; 4],
                fill: slime_color::LinearColor::rgb(1.0, 0.0, 0.0),
                border_width: 0.0,
                border: slime_color::LinearColor::BLACK,
            },
            None,
        );
        list
    }

    #[test]
    fn the_bridge_draws_through_the_door_and_a_lost_device_is_reported() {
        let gpu = gpu();
        let target = gpu
            .create_render_target("interface", 8, 8, FORMAT)
            .expect("the GPU has memory for it");
        let mut renderer = InterfaceRenderer::new(&gpu, FORMAT).expect("the renderer is made");
        let stats = renderer
            .draw(&gpu, &Fonts::new(), &list(), |_| None, &target)
            .expect("drawn");
        assert_eq!(stats.canvas.items, 1);
        let image = gpu.read_render_target(&target).expect("read back");
        assert_eq!(image.pixels[..4], [255, 0, 0, 255]);
        gpu.lose();
        let error = renderer
            .draw(&gpu, &Fonts::new(), &list(), |_| None, &target)
            .expect_err("the device is lost");
        assert!(matches!(error, GfxError::DeviceLost { .. }), "{error:?}");
        let error = InterfaceRenderer::new(&gpu, FORMAT)
            .err()
            .expect("the device is lost");
        assert!(matches!(error, GfxError::DeviceLost { .. }), "{error:?}");
    }

    #[test]
    #[should_panic(expected = "an interface renderer of another GPU device used with this one")]
    fn a_renderer_drawn_with_another_device_is_refused() {
        let (gpu, other) = (gpu(), gpu());
        let theirs = other
            .create_render_target("interface", 8, 8, FORMAT)
            .expect("the GPU has memory for it");
        let mut renderer = InterfaceRenderer::new(&gpu, FORMAT).expect("the renderer is made");
        let _ = renderer.draw(&other, &Fonts::new(), &list(), |_| None, &theirs);
    }

    /// uwui-render checks its shaders in scopes of its own: a broken one is its error, which the
    /// door, whose scopes would panic on it, never sees.
    #[test]
    fn a_broken_shader_of_the_canvas_stays_out_of_the_door() {
        let gpu = gpu();
        let refused = gpu
            .with_wgpu("checked", |loan| {
                let mut canvas =
                    CanvasRenderer::new(loan.device, loan.queue, WgpuLoan::texture_format(FORMAT))
                        .expect("the canvas is made");
                let refused = canvas.set_shader(loan.device, "@fragment fn main( -> {}");
                canvas
                    .set_shader(loan.device, SHADER)
                    .expect("the embedded shader compiles");
                refused
            })
            .expect("the door saw no error");
        let Err(uwui_render::Error::Shader { label, message }) = refused else {
            panic!("a shader's error: {refused:?}");
        };
        assert_eq!(label, "canvas.wgsl");
        assert!(!message.is_empty());
        assert!(gpu.check().is_ok());
    }
}
