//! The canvas: boxes, their shadows, gradients and images, drawn in two dimensions over a target,
//! as an interface draws (`docs/adr/0137-the-canvas.md`).
//!
//! A [`Canvas`] is data alone: the items of a frame, in their order, each image named by its index
//! ([`ImageId`]) in the list of images given when it is drawn. A [`CanvasRenderer`] draws it over a
//! target, with one pipeline, its shader [`SHADER`], the items read by their index from a storage
//! buffer (`docs/adr/0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md`). It
//! draws with wgpu, recording into its host's encoder
//! (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`).
//!
//! [`ImagePart`] and the writer of values as bytes, `push_f32s`, mirror `slime-render`'s:
//! `uwui-render` leaves the engine's repository with uwui, and cannot depend on its renderer
//! (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`).

use std::num::NonZeroU64;
use std::sync::{Mutex, PoisonError, mpsc};
use std::time::Duration;

use slime_color::{LinearColor, linear_to_srgb};

use crate::gpu::{DrawContext, Error, within_checks, within_memory};

/// The canvas's shader, in WGSL, which [`CanvasRenderer::new`] compiles: `canvas.wgsl`, embedded in
/// the program when it is compiled.
pub const SHADER: &str = include_str!("canvas.wgsl");

/// The label of the canvas's shader, which a shader's error names ([`Error::Shader`]'s
/// `label`).
const SHADER_LABEL: &str = "canvas.wgsl";

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

/// The image that an [`ImageQuad`] draws: its index in the list of images that
/// [`CanvasRenderer::draw`] is given, `ImageId(0)` the first. A canvas holds no image: the same
/// canvas may be drawn with other images of the same order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u32);

/// How an image's texels are drawn: the display list's own kinds. Of [`ImageKind::Coverage`], the
/// red of each texel is how much of it a shape covers, drawn in the quad's color, as a glyph is.
pub use uwui::display::ImageKind;

/// A part of an image, in fractions of the whole image's width and height, from its top left
/// corner; [`ImagePart::WHOLE`] is the whole image. What an [`ImageQuad`] shows of its image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePart {
    /// The part's left edge, from 0 at the image's left edge to 1 at its right.
    pub x: f32,
    /// The part's top edge, from 0 at the image's top edge to 1 at its bottom.
    pub y: f32,
    /// The part's width, positive, a fraction of the image's width.
    pub width: f32,
    /// The part's height, positive, a fraction of the image's height.
    pub height: f32,
}

impl ImagePart {
    /// The whole image.
    pub const WHOLE: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };
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
/// ([`CanvasRenderer::draw`]). Coordinates are the target's pixels, from its top left corner, y
/// down. It is data alone: its images are named by their index in the list given when it is drawn
/// ([`ImageId`]).
#[derive(Debug, Default)]
pub struct Canvas {
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

impl Canvas {
    /// An empty canvas.
    pub fn new() -> Self {
        Self::default()
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

    /// Draws a part of an image, the one that `quad.image` names in the list that the canvas is
    /// drawn with ([`CanvasRenderer::draw`]). A quad of no pixel draws nothing.
    pub fn draw_image(&mut self, quad: &ImageQuad) {
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

/// What [`CanvasRenderer::draw`] drew, and what it cost.
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
    /// device does not time passes (wgpu's `TIMESTAMP_QUERY`).
    pub gpu_time: Option<Duration>,
}

/// What draws canvases into targets of one format: its pipeline, made from [`SHADER`] or from
/// the source that [`CanvasRenderer::set_shader`] gives, its layouts, its buffers, grown as canvases
/// grow, its sampler, an image for the items that read none, and its timer.
///
/// It holds wgpu objects of the device it was made with, and none of the device itself: after a
/// lost device, it is dropped before the device is replaced, and one made with the new device draws
/// again. On a lost device, it draws nothing and reports nothing: wgpu cannot be asked whether its
/// device is lost, and its host reports the loss.
pub struct CanvasRenderer {
    frame_layout: wgpu::BindGroupLayout,
    image_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
    frame: wgpu::Buffer,
    /// The items' buffer, and its size in bytes.
    items: Option<(wgpu::Buffer, u64)>,
    transforms: wgpu::Buffer,
    sampler: wgpu::Sampler,
    blank: wgpu::TextureView,
    /// Behind a lock, so that the canvas's renderer is shared between threads: a timer receives its
    /// times through a channel.
    timer: Option<Mutex<Timer>>,
    warned_of_items: bool,
}

impl CanvasRenderer {
    /// What draws canvases into targets of `format`, with `device`, its embedded shader,
    /// [`SHADER`], compiled; `queue` writes its first values. Its passes are timed where the device
    /// has wgpu's `TIMESTAMP_QUERY`.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the GPU cannot hold its buffers, textures or queries,
    /// [`Error::Shader`] or [`Error::Pipeline`] if the device refuses its shader or its pipeline,
    /// as Direct3D 12's shader compiler may.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self, Error> {
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("canvas"),
            entries: &[
                buffer_entry(0, wgpu::BufferBindingType::Uniform, 16),
                buffer_entry(
                    1,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    ITEM_SIZE,
                ),
                wgpu::BindGroupLayoutEntry {
                    visibility: wgpu::ShaderStages::VERTEX,
                    ..buffer_entry(2, wgpu::BufferBindingType::Storage { read_only: true }, 32)
                },
            ],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("canvas image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("canvas"),
            bind_group_layouts: &[Some(&frame_layout), Some(&image_layout)],
            immediate_size: 0,
        });
        let pipeline = pipeline(device, &pipeline_layout, format, SHADER)?;
        let (frame, transforms, blank, timer) = within_memory(device, "canvas", || {
            let frame = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("canvas"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let transforms = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("canvas transforms"),
                size: 32,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: true,
            });
            let blank = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("canvas blank"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let timer = device
                .features()
                .contains(wgpu::Features::TIMESTAMP_QUERY)
                .then(|| Timer::new(device, queue));
            (frame, transforms, blank, timer)
        })?;
        // The identity, the one transform until uwui's first stage, written through the buffer's
        // mapping: on a lost device, the buffer is invalid, and the queue would refuse to write it,
        // an error that no loss excuses, where its mapping is refused without a word.
        let mut identity = Vec::new();
        push_f32s(&mut identity, &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        if let Ok(mut mapped) = transforms.get_mapped_range_mut(..) {
            mapped.copy_from_slice(&identity);
            drop(mapped);
            transforms.unmap();
        }
        queue.write_texture(
            blank.as_image_copy(),
            &[255; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("canvas"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        Ok(Self {
            frame_layout,
            image_layout,
            pipeline_layout,
            pipeline,
            format,
            frame,
            items: None,
            transforms,
            sampler,
            blank: blank.create_view(&wgpu::TextureViewDescriptor::default()),
            timer: timer.map(Mutex::new),
            warned_of_items: false,
        })
    }

    /// Makes the pipeline again from `source`, the WGSL of a shader that replaces [`SHADER`], as a
    /// program that reloads its shaders while it runs asks, with `device`, the renderer's. A source
    /// that does not compile, or whose pipeline is refused, leaves the pipeline as it was: the
    /// canvas draws on with it. Its errors are caught in scopes of its own: they never reach the
    /// device's handler, nor a scope of its host.
    ///
    /// # Errors
    ///
    /// [`Error::Shader`] if the shader does not compile, labelled `canvas.wgsl`, with the line and
    /// column of the error; [`Error::Pipeline`] if its pipeline is refused.
    pub fn set_shader(&mut self, device: &wgpu::Device, source: &str) -> Result<(), Error> {
        self.pipeline = pipeline(device, &self.pipeline_layout, self.format, source)?;
        Ok(())
    }

    /// Records `canvas`, drawn over what `target` holds, into `context`'s encoder, in a pass of its
    /// own: after a frame's views, the interface over them (`docs/adr/0137-the-canvas.md`).
    /// `images` are the images that its quads name by their index ([`ImageId`]), each a view of a
    /// texture of the device: of one channel, `R8Unorm`, how much of each texel a shape covers,
    /// drawn in a color ([`ImageKind::Coverage`]); of colors, drawn as they are
    /// ([`ImageKind::Color`]), not multiplied by their alpha, decoded first if the view is sRGB.
    /// Texels are filtered before they are multiplied by their alpha: a transparent texel beside an
    /// opaque one lends it its color, so that a transparent texel should hold its neighbours'
    /// colors, or the edge between them darkens. A view drawn by an earlier pass, as an editor's
    /// viewport, is drawn so too.
    ///
    /// The target is any texture of the renderer's format: a window's image, or one that a panel
    /// shows. Into a target of an sRGB format, the canvas's colors mix as their sRGB values do, as
    /// design tools mix them: it draws through a view of the target without the encoding, its colors
    /// encoded first, which the target must allow, its format without sRGB among its view formats
    /// (`docs/adr/0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md`); into
    /// one of another format, `Rgba16Float`, they mix linearly. The view is made at each draw, and
    /// kept by none.
    ///
    /// The queue writes the items before the host submits the encoder, which it does before this
    /// renderer draws again ([`DrawContext`]). Returns what it drew and the GPU time of an earlier
    /// canvas, the latest that came back once its encoder was submitted and the GPU finished it,
    /// which wgpu sees at a later submission.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the GPU cannot hold the canvas's items: nothing is recorded then.
    ///
    /// # Panics
    ///
    /// If `target` does not have the renderer's format, or holds several samples per pixel. If an
    /// item drawn reads an [`ImageId`] that is not below `images.len()`. wgpu refuses, through the
    /// device's handler or its host's scopes, a target that does not allow its view without sRGB,
    /// an object of another device, and a canvas that draws its own target as an image.
    pub fn draw(
        &mut self,
        context: &mut DrawContext<'_>,
        canvas: &Canvas,
        images: &[&wgpu::TextureView],
        target: &wgpu::Texture,
    ) -> Result<CanvasStats, Error> {
        assert!(
            target.format() == self.format,
            "the renderer draws canvases into {:?} targets, not into one of {:?}",
            self.format,
            target.format()
        );
        assert!(
            target.sample_count() == 1,
            "a canvas draws into targets of one sample per pixel, not of {}",
            target.sample_count()
        );
        let count = canvas.items.len().min(MAX_CANVAS_ITEMS);
        let left_out = canvas.items.len() - count;
        let batches = canvas.batches(count);
        // The image of a run is the image of each of its items that reads one.
        if let Some(ImageId(missing)) = batches
            .iter()
            .filter_map(|&(_, image)| image)
            .find(|image| image.0 as usize >= images.len())
        {
            panic!(
                "an item of the canvas reads image {missing}, beyond the images given ({})",
                images.len()
            );
        }
        if left_out > 0 && !self.warned_of_items {
            tracing::warn!(
                left_out,
                "a canvas draws more than {MAX_CANVAS_ITEMS} items: the last ones are left out (said \
                 once per renderer)"
            );
            self.warned_of_items = true;
        }
        let mut timer = self
            .timer
            .as_ref()
            .map(|timer| timer.lock().unwrap_or_else(PoisonError::into_inner));
        let gpu_time = timer.as_mut().and_then(|timer| timer.latest());
        if count == 0 {
            return Ok(CanvasStats {
                left_out,
                gpu_time,
                ..CanvasStats::default()
            });
        }
        let device = context.device;
        let encode = self.format.is_srgb();
        let written = item_bytes(&canvas.items[..count], encode);
        let bytes = (count as u64 * ITEM_SIZE)
            .next_power_of_two()
            .max(ITEM_SIZE * 64);
        if self.items.as_ref().is_none_or(|&(_, size)| size < bytes) {
            let items = within_memory(device, "canvas items", || {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("canvas items"),
                    size: bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: true,
                })
            })?;
            // A buffer made on a lost device is invalid: it cannot be mapped, and the queue would
            // refuse to write it, an error that no loss excuses. Nothing is drawn then.
            let Ok(mut mapped) = items.get_mapped_range_mut(..) else {
                return Ok(CanvasStats {
                    left_out,
                    gpu_time,
                    ..CanvasStats::default()
                });
            };
            mapped.slice(..written.len()).copy_from_slice(&written);
            drop(mapped);
            items.unmap();
            self.items = Some((items, bytes));
        } else {
            let (items, _) = self.items.as_ref().expect("checked above");
            context.queue.write_buffer(items, 0, &written);
        }
        let (items, _) = self.items.as_ref().expect("made or written above");
        let mut size = Vec::new();
        push_f32s(
            &mut size,
            &[
                target.width() as f32,
                target.height() as f32,
                if encode { 1.0 } else { 0.0 },
                0.0,
            ],
        );
        context.queue.write_buffer(&self.frame, 0, &size);
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("canvas"),
            layout: &self.frame_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: items.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.transforms.as_entire_binding(),
                },
            ],
        });
        let groups: Vec<wgpu::BindGroup> = batches
            .iter()
            .map(|&(_, image)| {
                let view = image.map_or(&self.blank, |id| images[id.0 as usize]);
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("canvas image"),
                    layout: &self.image_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            })
            .collect();
        // An sRGB target is drawn into without its encoding, the shader encoding the colors, so
        // that they mix as sRGB values.
        let view = target.create_view(&wgpu::TextureViewDescriptor {
            label: Some("canvas target"),
            format: Some(self.format.remove_srgb_suffix()),
            ..wgpu::TextureViewDescriptor::default()
        });
        let timed = timer.as_mut().and_then(|timer| timer.take_slot());
        {
            let mut pass = context
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("canvas"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: timed.as_ref().map(TimedSlot::timestamp_writes),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &frame_group, &[]);
            for ((range, _), group) in batches.iter().zip(&groups) {
                pass.set_bind_group(1, group, &[]);
                let first = u32::try_from(range.start * 6).expect("within MAX_CANVAS_ITEMS");
                let last = u32::try_from(range.end * 6).expect("within MAX_CANVAS_ITEMS");
                pass.draw(first..last, 0..1);
            }
        }
        if let Some(timed) = timed {
            timed.read_back(context.encoder);
        }
        Ok(CanvasStats {
            items: count,
            left_out,
            draws: batches.len(),
            gpu_time,
        })
    }
}

/// A uniform or storage buffer of `size` bytes at least, at `binding`, which both stages read.
fn buffer_entry(
    binding: u32,
    ty: wgpu::BufferBindingType,
    size: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(size),
        },
        count: None,
    }
}

/// The canvas's pipeline, from the WGSL `source`, for targets of `format`, which it draws into
/// without their sRGB encoding if they have one; the shader and the pipeline checked in scopes of
/// their own.
fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    source: &str,
) -> Result<wgpu::RenderPipeline, Error> {
    let (module, error) = within_checks(device, || {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(SHADER_LABEL),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        })
    });
    if let Some(error) = error {
        return Err(Error::Shader {
            label: SHADER_LABEL.to_owned(),
            message: error.to_string().trim_end().to_owned(),
        });
    }
    let (pipeline, error) = within_checks(device, || {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("canvas"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format.remove_srgb_suffix(),
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    });
    match error {
        None => Ok(pipeline),
        Some(error) => Err(Error::Pipeline {
            label: "canvas".to_owned(),
            message: error.to_string().trim_end().to_owned(),
        }),
    }
}

/// How many canvases can wait for their times: while that many do, the next ones go untimed.
const TIMER_SLOTS: usize = 4;

/// Times the canvas's passes: a timestamp written when a pass starts and ends, read back once the
/// GPU has finished it, so that nothing waits for the GPU. It never polls the device, on which
/// wgpu panics when the poll finds a lost Vulkan device: wgpu maps a readback once the encoder that
/// fills it is submitted ([`wgpu::CommandEncoder::map_buffer_on_submit`]), and gives it at a later
/// submission that finds the work done. A canvas whose encoder is dropped unsubmitted gives its
/// slot back untimed.
struct Timer {
    queries: wgpu::QuerySet,
    slots: Vec<Slot>,
    /// The slots whose readback came back, sent by wgpu's callbacks: mapped, or not.
    mapped: mpsc::Receiver<(usize, bool)>,
    sender: mpsc::Sender<(usize, bool)>,
    /// Nanoseconds per timestamp tick.
    period: f32,
    /// The time of the latest canvas whose time came back.
    latest: Option<Duration>,
}

/// The buffers of one timed canvas.
struct Slot {
    /// Where its two timestamps are resolved.
    resolve: wgpu::Buffer,
    /// Where they are copied for the CPU to read.
    readback: wgpu::Buffer,
    /// Whether a canvas has taken it and its time has not come back yet.
    in_flight: bool,
}

/// A slot taken by the canvas being recorded.
struct TimedSlot<'t> {
    timer: &'t Timer,
    index: usize,
}

impl Timer {
    /// A timer of the canvas's passes, created inside its caller's scope of memory.
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("canvas"),
            ty: wgpu::QueryType::Timestamp,
            count: 2 * TIMER_SLOTS as u32,
        });
        let size = u64::from(2 * wgpu::QUERY_SIZE);
        let slots = (0..TIMER_SLOTS)
            .map(|_| Slot {
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("canvas timestamps"),
                    size,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("canvas timestamps readback"),
                    size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                in_flight: false,
            })
            .collect();
        let (sender, mapped) = mpsc::channel();
        Self {
            queries,
            slots,
            mapped,
            sender,
            period: queue.get_timestamp_period(),
            latest: None,
        }
    }

    /// The time of the latest canvas whose time came back, the slots that came back freed. Reading
    /// takes no time worth measuring and never waits for the GPU.
    fn latest(&mut self) -> Option<Duration> {
        while let Ok((index, mapped)) = self.mapped.try_recv() {
            let slot = &mut self.slots[index];
            if !mapped {
                slot.in_flight = false;
                continue;
            }
            match slot.readback.get_mapped_range(..) {
                Ok(bytes) => {
                    let timestamp = |at: usize| {
                        let bytes: [u8; 8] = bytes[at..at + 8].try_into().unwrap_or([0; 8]);
                        u64::from_le_bytes(bytes)
                    };
                    let ticks = timestamp(8).saturating_sub(timestamp(0));
                    let nanoseconds = ticks as f64 * f64::from(self.period);
                    self.latest = Some(Duration::from_nanos(nanoseconds as u64));
                }
                // wgpu maps a readback whose device was lost while its work ran, then refuses
                // it: destroyed, it is neither read, unmapped nor used again.
                Err(_) => {
                    tracing::debug!("a canvas's time could not be read back");
                    continue;
                }
            }
            slot.readback.unmap();
            slot.in_flight = false;
        }
        self.latest
    }

    /// A slot for the canvas being recorded, `None` while every slot waits for its time.
    fn take_slot(&mut self) -> Option<TimedSlot<'_>> {
        let index = self.slots.iter().position(|slot| !slot.in_flight)?;
        self.slots[index].in_flight = true;
        Some(TimedSlot { timer: self, index })
    }
}

impl TimedSlot<'_> {
    /// The timestamps that the canvas's pass writes, at its start and its end.
    fn timestamp_writes(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        let first = 2 * self.index as u32;
        wgpu::RenderPassTimestampWrites {
            query_set: &self.timer.queries,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }

    /// Records, after the pass, the copy of its timestamps into the slot's readback, mapped once
    /// `encoder` is submitted and the GPU has finished it.
    fn read_back(self, encoder: &mut wgpu::CommandEncoder) {
        let slot = &self.timer.slots[self.index];
        let first = 2 * self.index as u32;
        encoder.resolve_query_set(&self.timer.queries, first..first + 2, &slot.resolve, 0);
        encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.readback, 0, None);
        let mut returned = Returned {
            sender: self.timer.sender.clone(),
            index: self.index,
        };
        encoder.map_buffer_on_submit(&slot.readback, wgpu::MapMode::Read, .., move |result| {
            returned.send(result.is_ok());
        });
    }
}

/// What gives a timed slot back to its timer: wgpu's callback once the readback is mapped, or its
/// drop, when the encoder is dropped without being submitted, which drops the callback uncalled.
struct Returned {
    sender: mpsc::Sender<(usize, bool)>,
    index: usize,
}

impl Returned {
    /// Gives the slot back, its readback mapped or not.
    fn send(&mut self, mapped: bool) {
        // The renderer may be gone by then: a slot that nobody reads is no loss.
        let _ = self.sender.send((self.index, mapped));
        // Sent once: the drop that follows sends nothing.
        self.index = usize::MAX;
    }
}

impl Drop for Returned {
    fn drop(&mut self) {
        if self.index != usize::MAX {
            let _ = self.sender.send((self.index, false));
        }
    }
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
        // The item's bytes laid out in place, then appended at once: as fast however the
        // compiler inlines a vector's growth.
        let mut laid_out = [0; ITEM_SIZE as usize];
        let floats = [
            item.quad,
            item.shape,
            item.radii,
            mix(item.color, 0),
            border,
            uv,
            item.clip,
            widths,
        ];
        let (words, kind) = laid_out.as_chunks_mut::<4>().0.split_at_mut(32);
        for (word, value) in words.iter_mut().zip(floats.as_flattened()) {
            *word = value.to_le_bytes();
        }
        for (word, value) in kind.iter_mut().zip(item.kind) {
            *word = value.to_le_bytes();
        }
        bytes.extend_from_slice(&laid_out);
    }
    bytes
}

/// Appends `values` as little-endian 32-bit floats.
fn push_f32s(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
