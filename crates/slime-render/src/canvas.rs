//! The canvas: boxes, their shadows, gradients and images, drawn in two dimensions over a target,
//! as an interface draws (`docs/adr/0137-the-canvas.md`).

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use slime_color::{LinearColor, linear_to_srgb};
use slime_gfx::{
    AddressMode, BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource,
    BindingType, Blend, CullMode, Filter, FragmentState, GfxError, Gpu, GpuTimer,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RenderTarget, Sampler,
    SamplerDescriptor, ShaderModule, ShaderStages, StorageBuffer, Texture, TextureFormat,
    UniformBuffer,
};

use crate::bytes::{push_f32s, push_u32s};
use crate::camera::ImagePart;
use crate::error::RenderError;

/// The bytes of an item as the shader reads it: nine `vec4`s.
const ITEM_SIZE: u64 = 9 * 16;

/// The most items one canvas draws: past them, the last ones are left out, with a warning. Their
/// buffer, 144 bytes an item, grows by powers of two up to 128 MiB, the largest storage buffer that
/// every device of the engine's floor binds.
pub const MAX_CANVAS_ITEMS: usize = 1 << 19;

/// A rectangle of a target, in its pixels, from its top left corner, y down; fractions of a pixel
/// included.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PixelRect {
    /// The left edge.
    pub x: f32,
    /// The top edge.
    pub y: f32,
    /// The width; nothing is drawn where it is not above zero.
    pub width: f32,
    /// The height; nothing is drawn where it is not above zero.
    pub height: f32,
}

impl PixelRect {
    /// The rectangle of `width` × `height` pixels whose top left corner is (`x`, `y`).
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Its edges: left, top, right, bottom.
    fn edges(self) -> [f32; 4] {
        [self.x, self.y, self.x + self.width, self.y + self.height]
    }

    /// Whether it holds no pixel, or holds a value that is not finite.
    fn is_empty(self) -> bool {
        !(self.width > 0.0 && self.height > 0.0 && self.edges().iter().all(|edge| edge.is_finite()))
    }
}

/// The radii of a box's corners, in pixels: top left, top right, bottom right, bottom left. Each is
/// made no larger than half the box's shorter side.
pub type Radii = [f32; 4];

/// A box: a rectangle with rounded corners, filled, and bordered inside its edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasBox {
    /// Where it lies.
    pub rect: PixelRect,
    /// The radii of its corners.
    pub radii: Radii,
    /// Its fill, inside its border.
    pub fill: LinearColor,
    /// Its border's width, in pixels, inside its edges: none at 0. Its inner corners are rounded by
    /// the outer radii less the width.
    pub border_width: f32,
    /// Its border's color.
    pub border: LinearColor,
    /// The rectangle outside which it is not drawn; none for the whole target.
    pub clip: Option<PixelRect>,
}

/// A box filled with a gradient: a rectangle with rounded corners, its corners' colors mixed across
/// it, each point's the mix of the four by its place, as the target mixes colors: on their sRGB
/// encoding into an sRGB target, as CSS mixes a gradient, linear into another
/// (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`). It has no border.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasGradient {
    /// Where it lies.
    pub rect: PixelRect,
    /// The radii of its corners.
    pub radii: Radii,
    /// Its corners' colors: top left, top right, bottom right, bottom left.
    pub corners: [LinearColor; 4],
    /// The rectangle outside which it is not drawn; none for the whole target.
    pub clip: Option<PixelRect>,
}

/// The shadow that a box casts, drawn under it: the box, moved, grown and blurred.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasShadow {
    /// The box that casts it.
    pub rect: PixelRect,
    /// The radii of the box's corners.
    pub radii: Radii,
    /// The shadow's color, its alpha its darkness where it is whole.
    pub color: LinearColor,
    /// How far the shadow lies from the box, in pixels, right and down.
    pub offset: [f32; 2],
    /// How far its edge fades, in pixels: twice the standard deviation of the Gaussian that
    /// blurs it, as CSS's `box-shadow` measures it; a sharp edge at 0.
    pub blur: f32,
    /// How much larger than the box the shadow is on each side, in pixels, before the blur;
    /// smaller where it is negative. Its corners' radii grow by as much, a square corner staying
    /// square, as CSS's `box-shadow` grows them.
    pub spread: f32,
    /// The rectangle outside which it is not drawn; none for the whole target.
    pub clip: Option<PixelRect>,
}

/// An image that a [`Canvas`] draws: a texture, as the glyphs of a text's atlas or a picture, or a
/// render target that a view drew, as an editor's viewport shows.
#[derive(Clone, Copy, Debug)]
pub enum CanvasImage<'a> {
    /// A texture. One of one channel, `R8Unorm`, is how much of each texel a shape covers, drawn in
    /// a color ([`ImageKind::Coverage`]); one of colors is drawn as it is ([`ImageKind::Color`]),
    /// its colors not multiplied by their alpha, decoded first if they are sRGB. Its texels are
    /// filtered before they are multiplied by their alpha: a transparent texel beside an opaque one
    /// lends it its color, so that a transparent texel should hold its neighbours' colors, or the
    /// edge between them darkens.
    Texture(&'a Texture),
    /// A render target, of one sample per pixel, that an earlier pass drew into. It cannot be the
    /// target that the canvas draws into.
    Target(&'a RenderTarget),
}

/// An image that a [`Canvas`] holds, by the order it was added in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(u32);

/// How an image's texels are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageKind {
    /// The red of each texel is how much of it a shape covers, drawn in the quad's color: a glyph.
    Coverage,
    /// Each texel is a color, drawn as it is, faded by the alpha of the quad's color.
    Color,
}

/// A part of an image drawn into a rectangle of the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageQuad {
    /// Where it is drawn.
    pub rect: PixelRect,
    /// The image.
    pub image: ImageId,
    /// The part of the image drawn, read at its first level, its own texels, filtered between them:
    /// an image drawn much smaller than it is shows steps.
    pub part: ImagePart,
    /// How its texels are drawn.
    pub kind: ImageKind,
    /// The color of a coverage, or, of an image of colors, its alpha alone, which fades it.
    pub color: LinearColor,
    /// The rectangle outside which it is not drawn; none for the whole target.
    pub clip: Option<PixelRect>,
}

/// What an interface draws in a frame: boxes, shadows, gradients and images, in the order they are
/// added, each over what was added before it, each clipped to its rectangle
/// ([`Renderer::draw_canvas`](crate::Renderer::draw_canvas)). Coordinates are the target's pixels,
/// from its top left corner, y down. A canvas borrows the images it draws, for the frame.
#[derive(Debug, Default)]
pub struct Canvas<'a> {
    images: Vec<CanvasImage<'a>>,
    items: Vec<Item>,
    /// The image of each item, if it has one, beside it.
    item_images: Vec<Option<ImageId>>,
}

/// An item as the shader reads it.
#[derive(Clone, Copy, Debug)]
struct Item {
    quad: [f32; 4],
    shape: [f32; 4],
    radii: [f32; 4],
    /// The fill, or the shadow's or the quad's color, straight: not multiplied by its alpha, which
    /// [`item_bytes`] does, as the target mixes colors.
    color: [f32; 4],
    /// A box's border's color, straight; a shadow's exponents (`canvas.wgsl`).
    border: [f32; 4],
    uv: [f32; 4],
    clip: [f32; 4],
    widths: [f32; 4],
    /// What the item is, the index of its transform and that of its layer: the identity and the
    /// root's until Stage 7 (`docs/adr/0132-the-engines-ui-framework.md`).
    kind: [u32; 4],
}

const BOX: u32 = 0;
const SHADOW: u32 = 1;
const COVERAGE: u32 = 2;
const IMAGE: u32 = 3;
/// A gradient keeps its corners' colors in `color`, `border`, `uv` and `widths` (`canvas.wgsl`).
const GRADIENT: u32 = 4;

/// A clip that holds every pixel.
const NO_CLIP: [f32; 4] = [-f32::MAX, -f32::MAX, f32::MAX, f32::MAX];

impl<'a> Canvas<'a> {
    /// An empty canvas.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `image`, which quads then draw by the id given.
    ///
    /// # Panics
    ///
    /// Past 2³² images.
    pub fn add_image(&mut self, image: CanvasImage<'a>) -> ImageId {
        let id = ImageId(u32::try_from(self.images.len()).expect("fewer than 2³² images"));
        self.images.push(image);
        id
    }

    /// Draws the box `shape`. A box of no pixel, or whose rectangle is not finite, draws nothing.
    pub fn draw_box(&mut self, shape: &CanvasBox) {
        if shape.rect.is_empty() {
            return;
        }
        let edges = shape.rect.edges();
        let width = shape.border_width.max(0.0);
        self.push(
            Item {
                quad: outward(edges),
                shape: edges,
                radii: radii(shape.radii, shape.rect),
                color: straight(shape.fill),
                border: straight(shape.border),
                uv: [0.0; 4],
                clip: clip(shape.clip),
                widths: [width, 0.0, 0.0, 0.0],
                kind: [BOX, 0, 0, 0],
            },
            None,
        );
    }

    /// Draws the gradient `shape`. A gradient of no pixel, or whose rectangle is not finite, draws
    /// nothing.
    pub fn draw_gradient(&mut self, shape: &CanvasGradient) {
        if shape.rect.is_empty() {
            return;
        }
        let edges = shape.rect.edges();
        let [top_left, top_right, bottom_right, bottom_left] = shape.corners.map(straight);
        self.push(
            Item {
                quad: outward(edges),
                shape: edges,
                radii: radii(shape.radii, shape.rect),
                color: top_left,
                border: top_right,
                uv: bottom_right,
                clip: clip(shape.clip),
                widths: bottom_left,
                kind: [GRADIENT, 0, 0, 0],
            },
            None,
        );
    }

    /// Draws the shadow `shadow`. A shadow of a box of no pixel, or that the spread leaves
    /// without one, draws nothing.
    ///
    /// The box blurred by a Gaussian is Raph Levien's approximation of it, whose error against the
    /// box blurred exactly, measured in the alpha of a shadow whose own is 1, is at most: 0.015 for
    /// a square of 24 pixels, its corners of 4, blurred by 8; 0.03 for a panel of 48 × 16, its
    /// corners of 6, blurred by 6; 0.04 for a box of 40 × 24 of corners of 12 and 0; 0.02 for a
    /// box of 8 pixels blurred by 12; and 0.06 for a circle of 32 pixels blurred by 10, the worst
    /// that the canvas's tests found, a blur nearing the radius. The blur also counts a pixel's
    /// own width, so that one of less than a pixel is drawn smooth; a blur of 0, or one that is not
    /// a number, draws a sharp edge, as a box's, and an infinite one nothing.
    pub fn draw_shadow(&mut self, shadow: &CanvasShadow) {
        let spread = shadow.spread;
        let rect = PixelRect::new(
            shadow.rect.x + shadow.offset[0] - spread,
            shadow.rect.y + shadow.offset[1] - spread,
            shadow.rect.width + 2.0 * spread,
            shadow.rect.height + 2.0 * spread,
        );
        if shadow.rect.is_empty() || rect.is_empty() || shadow.blur == f32::INFINITY {
            return;
        }
        let sigma = shadow.blur.max(0.0) / 2.0;
        // Three deviations beyond the box, where the Gaussian leaves nothing to see.
        let reach = 3.0 * sigma;
        let edges = rect.edges();
        let corners = radii(
            shadow
                .radii
                .map(|radius| if radius > 0.0 { radius + spread } else { 0.0 }),
            rect,
        );
        // Raph Levien's approximation, as `canvas.wgsl` reads it. Its deviation is the Gaussian's
        // times √2, widened by a pixel's own, whose variance is 1/12, so that a blur thinner than a
        // pixel is drawn smooth; none for a sharp shadow, whose box is then drawn as a box is.
        let deviation = if sigma > 0.0 {
            (2.0 * sigma * sigma + 1.0 / 6.0).sqrt()
        } else {
            0.0
        };
        let (width, height) = (rect.width, rect.height);
        let shorter = width.min(height);
        let largest = shorter / 2.0;
        let mut radius = [0.0; 4];
        let mut exponent = [2.0; 4];
        for (corner, &corner_radius) in corners.iter().enumerate() {
            let r0 = corner_radius.hypot(1.15 * deviation).min(largest);
            radius[corner] = corner_radius.hypot(2.0 * deviation).min(largest);
            if r0 > 0.0 {
                exponent[corner] = 2.0 * radius[corner] / r0;
            }
        }
        // The longer side pulled in, the box made less long, as a blur makes it look.
        let (mut long_width, mut long_height) = (width, height);
        let mut scale = 1.0;
        if deviation > 0.0 {
            let fade = |side: f32| (-(0.5 * side / deviation).powi(2)).exp();
            let delta = 1.25 * deviation * (fade(width) - fade(height));
            long_width += delta.min(0.0);
            long_height -= delta.max(0.0);
            let mean_radius = corners.iter().sum::<f32>() / 4.0;
            scale = 0.5 * erf(0.5 * (long_width.max(long_height) - 0.5 * mean_radius) / deviation);
        }
        let center = [(edges[0] + edges[2]) / 2.0, (edges[1] + edges[3]) / 2.0];
        self.push(
            Item {
                quad: outward([
                    edges[0] - reach,
                    edges[1] - reach,
                    edges[2] + reach,
                    edges[3] + reach,
                ]),
                shape: [
                    center[0] - long_width / 2.0,
                    center[1] - long_height / 2.0,
                    center[0] + long_width / 2.0,
                    center[1] + long_height / 2.0,
                ],
                radii: radius,
                color: straight(shadow.color),
                border: exponent,
                uv: exponent.map(|exponent| 1.0 / exponent),
                clip: clip(shadow.clip),
                widths: [
                    shorter,
                    scale,
                    if deviation > 0.0 {
                        1.0 / deviation
                    } else {
                        0.0
                    },
                    0.0,
                ],
                kind: [SHADOW, 0, 0, 0],
            },
            None,
        );
    }

    /// Draws a part of an image. A quad of no pixel draws nothing.
    ///
    /// # Panics
    ///
    /// If its image was not added to this canvas.
    pub fn draw_image(&mut self, quad: &ImageQuad) {
        assert!(
            (quad.image.0 as usize) < self.images.len(),
            "image {} was not added to this canvas, which holds {}",
            quad.image.0,
            self.images.len()
        );
        if quad.rect.is_empty() {
            return;
        }
        let edges = quad.rect.edges();
        let kind = match quad.kind {
            ImageKind::Coverage => COVERAGE,
            ImageKind::Color => IMAGE,
        };
        self.push(
            Item {
                quad: edges,
                shape: edges,
                radii: [0.0; 4],
                color: straight(quad.color),
                border: [0.0; 4],
                uv: [
                    quad.part.x,
                    quad.part.y,
                    quad.part.x + quad.part.width,
                    quad.part.y + quad.part.height,
                ],
                clip: clip(quad.clip),
                widths: [0.0; 4],
                kind: [kind, 0, 0, 0],
            },
            Some(quad.image),
        );
    }

    /// How many items it draws.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether it draws nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Adds `item`, its quad cut down to the pixels that its clip holds in part: none, or a clip
    /// that is empty, and it is left out. An image's coordinates follow its quad's edges.
    fn push(&mut self, mut item: Item, image: Option<ImageId>) {
        let [left, top, right, bottom] = item.quad;
        if !(item.clip[0] < item.clip[2] && item.clip[1] < item.clip[3]) {
            return;
        }
        let clip = outward(item.clip);
        let quad = [
            left.max(clip[0]),
            top.max(clip[1]),
            right.min(clip[2]),
            bottom.min(clip[3]),
        ];
        if !(quad[0] < quad[2] && quad[1] < quad[3]) {
            return;
        }
        if image.is_some() {
            let [u0, v0, u1, v1] = item.uv;
            let u = |x: f32| u0 + (u1 - u0) * (x - left) / (right - left);
            let v = |y: f32| v0 + (v1 - v0) * (y - top) / (bottom - top);
            item.uv = [u(quad[0]), v(quad[1]), u(quad[2]), v(quad[3])];
        }
        item.quad = quad;
        self.items.push(item);
        self.item_images.push(image);
    }

    /// The runs of items that one draw takes: those that read one image, or none, in a row.
    fn batches(&self, count: usize) -> Vec<(std::ops::Range<usize>, Option<ImageId>)> {
        let mut batches: Vec<(std::ops::Range<usize>, Option<ImageId>)> = Vec::new();
        for (index, &image) in self.item_images[..count].iter().enumerate() {
            match batches.last_mut() {
                Some((range, batch)) if image.is_none() || *batch == image || batch.is_none() => {
                    range.end = index + 1;
                    if batch.is_none() {
                        *batch = image;
                    }
                }
                _ => batches.push((index..index + 1, image)),
            }
        }
        batches
    }
}

/// `color`, its components made no less than 0 and its alpha no more than 1.
fn straight(color: LinearColor) -> [f32; 4] {
    [
        color.r.max(0.0),
        color.g.max(0.0),
        color.b.max(0.0),
        color.a.clamp(0.0, 1.0),
    ]
}

/// `color`, straight, as the shader lays it over the target: multiplied by its alpha, and
/// sRGB-encoded first, each component no more than 1, if `encode`.
fn mixed(color: [f32; 4], encode: bool) -> [f32; 4] {
    let [r, g, b, alpha] = color;
    let value = |component: f32| {
        if encode {
            linear_to_srgb(component.min(1.0))
        } else {
            component
        }
    };
    [value(r) * alpha, value(g) * alpha, value(b) * alpha, alpha]
}

/// `radii`, each made no larger than half the shorter side of `rect`, and not negative: 0 where it
/// is not a number.
fn radii(radii: Radii, rect: PixelRect) -> [f32; 4] {
    let largest = rect.width.min(rect.height) / 2.0;
    radii.map(|radius| radius.max(0.0).min(largest))
}

fn clip(clip: Option<PixelRect>) -> [f32; 4] {
    clip.map_or(NO_CLIP, PixelRect::edges)
}

/// `edges` grown to whole pixels: those whose centre lies less than half a pixel outside them, and
/// which a shape or a clip that ends there covers in part, are then inside.
fn outward(edges: [f32; 4]) -> [f32; 4] {
    [
        edges[0].floor(),
        edges[1].floor(),
        edges[2].ceil(),
        edges[3].ceil(),
    ]
}

/// The error function, as `canvas.wgsl` computes it.
fn erf(x: f32) -> f32 {
    let y = x.clamp(-10.0, 10.0) * std::f32::consts::FRAC_2_SQRT_PI;
    let yy = y * y;
    let z = y + (0.24295 + (0.03395 + 0.0104 * yy) * yy) * (y * yy);
    z / (1.0 + z * z).sqrt()
}

/// What [`Renderer::draw_canvas`](crate::Renderer::draw_canvas) drew, and what it cost.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CanvasStats {
    /// The items drawn.
    pub items: usize,
    /// The items left out, past [`MAX_CANVAS_ITEMS`].
    pub left_out: usize,
    /// The draws it took: one per run of items that read one image.
    pub draws: usize,
    /// How long a canvas took on the GPU: the latest that the renderer drew whose time came back, a
    /// few canvases ago, which is not this frame's total where it draws several; none where the
    /// adapter does not time passes.
    pub gpu_time: Option<Duration>,
}

/// What drawing canvases needs, which the renderer keeps: its pipeline, its layouts, its buffers,
/// grown as canvases grow, its sampler, an image for the items that read none, and its timer.
pub(crate) struct CanvasRenderer {
    frame_layout: BindGroupLayout,
    image_layout: BindGroupLayout,
    pipeline: RenderPipeline,
    format: TextureFormat,
    frame: UniformBuffer,
    /// The items' buffer, and its size in bytes.
    items: Option<(StorageBuffer, u64)>,
    transforms: StorageBuffer,
    sampler: Sampler,
    blank: Texture,
    /// Behind a lock, so that the renderer is shared between threads: a timer reads its times
    /// through cells.
    timer: Option<Mutex<GpuTimer>>,
    warned_of_items: bool,
}

impl CanvasRenderer {
    /// What draws canvases into targets of `format`, with the shader `module`.
    pub(crate) fn new(
        gpu: &Gpu,
        module: &ShaderModule,
        format: TextureFormat,
    ) -> Result<Self, GfxError> {
        let frame_layout = gpu.create_bind_group_layout(
            "canvas",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::VertexAndFragment,
                    ty: BindingType::UniformBuffer {
                        dynamic_offset: false,
                        size: 16,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::VertexAndFragment,
                    ty: BindingType::StorageBuffer { size: ITEM_SIZE },
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visible_to: ShaderStages::Vertex,
                    ty: BindingType::StorageBuffer { size: 32 },
                },
            ],
        )?;
        let image_layout = gpu.create_bind_group_layout(
            "canvas image",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Texture,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Sampler,
                },
            ],
        )?;
        let pipeline = pipeline(gpu, module, format, &frame_layout, &image_layout)?;
        let frame = gpu.create_uniform_buffer("canvas", &[0; 16])?;
        // The identity, the one transform until Stage 7.
        let mut identity = Vec::new();
        push_f32s(&mut identity, &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let transforms = gpu.create_storage_buffer("canvas transforms", 32)?;
        gpu.write_storage_buffer(&transforms, 0, &identity);
        let sampler = gpu.create_sampler(
            "canvas",
            &SamplerDescriptor {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_u: AddressMode::ClampToEdge,
                address_v: AddressMode::ClampToEdge,
            },
        )?;
        let blank =
            gpu.create_texture("canvas blank", 1, 1, TextureFormat::Rgba8Unorm, &[255; 4])?;
        Ok(Self {
            frame_layout,
            image_layout,
            pipeline,
            format,
            frame,
            items: None,
            transforms,
            sampler,
            blank,
            timer: gpu.create_timer("canvas", 1)?.map(Mutex::new),
            warned_of_items: false,
        })
    }

    /// Makes the pipeline again from `module`, as new shaders ask.
    pub(crate) fn set_shader(&mut self, gpu: &Gpu, module: &ShaderModule) -> Result<(), GfxError> {
        self.pipeline = pipeline(
            gpu,
            module,
            self.format,
            &self.frame_layout,
            &self.image_layout,
        )?;
        Ok(())
    }

    /// Draws `canvas` over what `target` holds.
    pub(crate) fn draw(
        &mut self,
        gpu: &Gpu,
        canvas: &Canvas<'_>,
        target: &RenderTarget,
    ) -> Result<CanvasStats, RenderError> {
        assert!(
            target.format() == self.format,
            "the renderer draws canvases into {:?} targets, not into `{}`, which is {:?}",
            self.format,
            target.label(),
            target.format()
        );
        assert!(
            !target.is_multisampled(),
            "a canvas draws into targets of one sample per pixel, not into `{}`",
            target.label()
        );
        gpu.check()?;
        // An sRGB target is drawn into without its encoding, the shader encoding the colors, so
        // that they mix as sRGB values.
        let unencoded = target.without_srgb();
        let target = unencoded.as_ref().unwrap_or(target);
        let count = canvas.items.len().min(MAX_CANVAS_ITEMS);
        let left_out = canvas.items.len() - count;
        if left_out > 0 && !self.warned_of_items {
            tracing::warn!(
                left_out,
                "a canvas draws more than {MAX_CANVAS_ITEMS} items: the last ones are left out (said \
                 once per renderer)"
            );
            self.warned_of_items = true;
        }
        let timer = self
            .timer
            .as_ref()
            .map(|timer| timer.lock().unwrap_or_else(PoisonError::into_inner));
        let gpu_time = timer.as_ref().and_then(|timer| timer.latest(0));
        if count == 0 {
            return Ok(CanvasStats {
                left_out,
                gpu_time,
                ..CanvasStats::default()
            });
        }
        let bytes = (count as u64 * ITEM_SIZE)
            .next_power_of_two()
            .max(ITEM_SIZE * 64);
        if self.items.as_ref().is_none_or(|&(_, size)| size < bytes) {
            self.items = Some((gpu.create_storage_buffer("canvas items", bytes)?, bytes));
        }
        let (items, _) = self.items.as_ref().expect("made above");
        gpu.write_storage_buffer(
            items,
            0,
            &item_bytes(&canvas.items[..count], self.format.is_srgb()),
        );
        let mut size = Vec::new();
        push_f32s(
            &mut size,
            &[
                target.width() as f32,
                target.height() as f32,
                if self.format.is_srgb() { 1.0 } else { 0.0 },
                0.0,
            ],
        );
        gpu.write_uniform_buffer(&self.frame, &size);
        let frame_group = gpu.create_bind_group(
            "canvas",
            &self.frame_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::UniformBuffer(&self.frame),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::StorageBuffer(items),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::StorageBuffer(&self.transforms),
                },
            ],
        )?;
        let batches = canvas.batches(count);
        let groups = batches
            .iter()
            .map(|&(_, image)| {
                let image = match image.map(|id| canvas.images[id.0 as usize]) {
                    None => BindingResource::Texture(&self.blank),
                    Some(CanvasImage::Texture(texture)) => BindingResource::Texture(texture),
                    Some(CanvasImage::Target(source)) => BindingResource::RenderTarget(source),
                };
                gpu.create_bind_group(
                    "canvas image",
                    &self.image_layout,
                    &[
                        BindGroupEntry {
                            binding: 0,
                            resource: image,
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Sampler(&self.sampler),
                        },
                    ],
                )
            })
            .collect::<Result<Vec<BindGroup>, GfxError>>()?;
        let pass = RenderPassDescriptor {
            target,
            clear: None,
            depth: None,
            timer: timer.as_deref(),
            resolve: None,
        };
        gpu.render(&pass, |pass| {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &frame_group, &[]);
            for ((range, _), group) in batches.iter().zip(&groups) {
                pass.set_bind_group(1, group, &[]);
                let first = u32::try_from(range.start * 6).expect("within MAX_CANVAS_ITEMS");
                let last = u32::try_from(range.end * 6).expect("within MAX_CANVAS_ITEMS");
                pass.draw(first..last);
            }
        });
        Ok(CanvasStats {
            items: count,
            left_out,
            draws: batches.len(),
            gpu_time,
        })
    }
}

/// The canvas's pipeline, from `module`, for targets of `format`, which it draws into without their
/// sRGB encoding if they have one.
fn pipeline(
    gpu: &Gpu,
    module: &ShaderModule,
    format: TextureFormat,
    frame: &BindGroupLayout,
    image: &BindGroupLayout,
) -> Result<RenderPipeline, GfxError> {
    gpu.create_render_pipeline(&RenderPipelineDescriptor {
        label: "canvas",
        shader: module,
        vertex_entry_point: "vs_main",
        vertex_layout: None,
        fragment: Some(FragmentState {
            entry_point: "fs_main",
            target_format: Some(format.without_srgb().unwrap_or(format)),
            blend: Some(Blend::Premultiplied),
        }),
        bind_group_layouts: &[frame, image],
        depth: None,
        cull: CullMode::None,
        multisampled: false,
    })
}

/// `items`, laid out as the shader reads them, little-endian, their colors mixed as the target
/// mixes them ([`mixed`]): sRGB-encoded if `encode`.
fn item_bytes(items: &[Item], encode: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(items.len() * ITEM_SIZE as usize);
    // Items in a row often share their colors, as the glyphs of a text do: each is mixed once.
    let mut last = [([f32::NAN; 4], [0.0; 4]); 2];
    let mut mix = |color: [f32; 4], slot: usize| {
        if color != last[slot].0 {
            last[slot] = (color, mixed(color, encode));
        }
        last[slot].1
    };
    for item in items {
        let gradient = item.kind[0] == GRADIENT;
        let border = match item.kind[0] {
            BOX => mix(item.border, 1),
            GRADIENT => mixed(item.border, encode),
            _ => item.border,
        };
        // A gradient's corners are colors, mixed as the target mixes them.
        let (uv, widths) = if gradient {
            (mixed(item.uv, encode), mixed(item.widths, encode))
        } else {
            (item.uv, item.widths)
        };
        for vector in [
            item.quad,
            item.shape,
            item.radii,
            mix(item.color, 0),
            border,
            uv,
            item.clip,
            widths,
        ] {
            push_f32s(&mut bytes, &vector);
        }
        push_u32s(&mut bytes, &item.kind);
    }
    bytes
}
