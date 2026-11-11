//! The canvas's items, copied from `crates/slime-render/src/canvas.rs` onto plain types, and the
//! two screens of the benchmark `render.canvas`, copied from `benches/src/render.rs`. The item
//! building, its clip cut, its runs and its bytes are unchanged; images are named by their index in
//! the prototype's table of textures ([`crate::gpu::Images`]) rather than added to the canvas.
//!
//! The effects add what only their pipelines draw (`effects.rs`): the kinds PATH, GLASS and
//! LAYER, a table of transforms that items name, the paths' segments, and quads kept whole rather
//! than cut to their clips.

use std::ops::Range;

/// A colour, linear, its components not multiplied by its alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl LinearColor {
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

/// The sRGB encoding of a linear value from 0 to 1, as `slime-color` computes it.
pub fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

/// A rectangle of a target, in its pixels, from its top left corner, y down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PixelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PixelRect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Its edges: left, top, right, bottom.
    pub fn edges(self) -> [f32; 4] {
        [self.x, self.y, self.x + self.width, self.y + self.height]
    }

    /// Whether it holds no pixel, or holds a value that is not finite.
    fn is_empty(self) -> bool {
        !(self.width > 0.0 && self.height > 0.0 && self.edges().iter().all(|edge| edge.is_finite()))
    }
}

/// The radii of a box's corners: top left, top right, bottom right, bottom left.
pub type Radii = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasBox {
    pub rect: PixelRect,
    pub radii: Radii,
    pub fill: LinearColor,
    pub border_width: f32,
    pub border: LinearColor,
    pub clip: Option<PixelRect>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasGradient {
    pub rect: PixelRect,
    pub radii: Radii,
    /// Top left, top right, bottom right, bottom left.
    pub corners: [LinearColor; 4],
    pub clip: Option<PixelRect>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasShadow {
    pub rect: PixelRect,
    pub radii: Radii,
    pub color: LinearColor,
    pub offset: [f32; 2],
    pub blur: f32,
    pub spread: f32,
    pub clip: Option<PixelRect>,
}

/// A part of an image, in fractions of its size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePart {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ImagePart {
    pub const WHOLE: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };
}

/// An image, by its index in the prototype's table of textures ([`crate::gpu::Images`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageKind {
    Coverage,
    Color,
    /// A group drawn apart ([`LAYER`]).
    Layer,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageQuad {
    pub rect: PixelRect,
    pub image: ImageId,
    pub part: ImagePart,
    pub kind: ImageKind,
    pub color: LinearColor,
    pub clip: Option<PixelRect>,
}

/// An item as `canvas.wgsl` reads it: nine `vec4`s (see that file for each field's meaning).
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub quad: [f32; 4],
    pub shape: [f32; 4],
    pub radii: [f32; 4],
    /// Straight: [`item_bytes`] mixes it as the target mixes colours.
    pub color: [f32; 4],
    pub border: [f32; 4],
    pub uv: [f32; 4],
    pub clip: [f32; 4],
    pub widths: [f32; 4],
    /// The kind, the transform's index, the layer's index.
    pub kind: [u32; 4],
}

/// The bytes of an item in S's storage buffer and in T's texture.
pub const ITEM_SIZE: usize = 9 * 16;

pub const BOX: u32 = 0;
pub const SHADOW: u32 = 1;
pub const COVERAGE: u32 = 2;
pub const IMAGE: u32 = 3;
pub const GRADIENT: u32 = 4;
/// How many kinds the screens of `render.canvas` draw; each kind is its own index below this.
pub const KINDS: usize = 5;
/// The effects' kinds (`effects.rs`), which only the effects' pipelines draw. A filled path: its
/// segments are [`Canvas::segments`] from `kind[2]`, `kind[3]` of them.
pub const PATH: u32 = 5;
/// A box whose fill is its tint laid over an image, the blurred region behind it (`blur.rs`):
/// a box's fields, and in `uv` the map from a point to the image's coordinates, `(p - uv.xy) *
/// uv.zw`.
pub const GLASS: u32 = 6;
/// An image whose texels are already multiplied by their alpha and encoded as the target is, a
/// group drawn apart: drawn faded by the item's alpha.
pub const LAYER: u32 = 7;
/// How many kinds the effects' pipelines draw.
pub const FX_KINDS: usize = 8;

/// An affine transform of the plane, `[a, b, c, d, e, f]`: x' = a x + b y + c, y' = d x + e y + f.
pub type Affine = [f32; 6];
pub const IDENTITY: Affine = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// A rotation by `degrees` about `center`.
pub fn rotation_about(center: [f32; 2], degrees: f32) -> Affine {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let [x, y] = center;
    [
        cos,
        -sin,
        x - cos * x + sin * y,
        sin,
        cos,
        y - sin * x - cos * y,
    ]
}

/// A scale by `factor` about `center`.
pub fn scale_about(center: [f32; 2], factor: f32) -> Affine {
    let [x, y] = center;
    [factor, 0.0, x - factor * x, 0.0, factor, y - factor * y]
}

const NO_CLIP: [f32; 4] = [-f32::MAX, -f32::MAX, f32::MAX, f32::MAX];

/// The items a frame draws, in their order, and the image each reads.
#[derive(Debug, Default)]
pub struct Canvas {
    pub items: Vec<Item>,
    pub item_images: Vec<Option<ImageId>>,
    /// The transforms that items name by `kind[1]`: 0 is the identity, `i` is `transforms[i -
    /// 1]`. Only the effects' pipelines read them.
    pub transforms: Vec<Affine>,
    /// The paths' segments, `[x0, y0, x1, y1]` in the item's space.
    pub segments: Vec<[f32; 4]>,
    /// Keep each quad whole rather than cut it to its clip on the CPU: an item whose transform is
    /// set after it is pushed may cross its clip, which stays in the target's pixels.
    pub keep_whole: bool,
}

/// A run of items that one draw of S or T takes: those that read one image, or none, in a row.
pub type Batch = (Range<usize>, Option<ImageId>);

impl Canvas {
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

    /// Raph Levien's approximation of a box blurred by a Gaussian, as `slime-render` computes it.
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
        let reach = 3.0 * sigma;
        let edges = rect.edges();
        let corners = radii(
            shadow
                .radii
                .map(|radius| if radius > 0.0 { radius + spread } else { 0.0 }),
            rect,
        );
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

    pub fn draw_image(&mut self, quad: &ImageQuad) {
        if quad.rect.is_empty() {
            return;
        }
        let edges = quad.rect.edges();
        let kind = match quad.kind {
            ImageKind::Coverage => COVERAGE,
            ImageKind::Color => IMAGE,
            ImageKind::Layer => LAYER,
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

    /// A box filled with its tint laid over `behind`, which holds the region `region` of the
    /// target ([`GLASS`]).
    pub fn draw_glass(&mut self, shape: &CanvasBox, behind: ImageId, region: PixelRect) {
        if shape.rect.is_empty() || region.is_empty() {
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
                uv: [region.x, region.y, 1.0 / region.width, 1.0 / region.height],
                clip: clip(shape.clip),
                widths: [width, 0.0, 0.0, 0.0],
                kind: [GLASS, 0, 0, 0],
            },
            Some(behind),
        );
    }

    /// A path filled by the non-zero rule, its `segments` closed contours in the target's
    /// pixels ([`PATH`]).
    pub fn draw_path(
        &mut self,
        segments: &[[f32; 4]],
        color: LinearColor,
        clip_rect: Option<PixelRect>,
    ) {
        if segments.is_empty() || segments.iter().flatten().any(|value| !value.is_finite()) {
            return;
        }
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for &[x0, y0, x1, y1] in segments {
            bounds = [
                bounds[0].min(x0.min(x1)),
                bounds[1].min(y0.min(y1)),
                bounds[2].max(x0.max(x1)),
                bounds[3].max(y0.max(y1)),
            ];
        }
        let first = self.segments.len() as u32;
        self.segments.extend_from_slice(segments);
        // Half a pixel more on each side holds the pixels that the edge covers in part.
        self.push(
            Item {
                quad: outward([
                    bounds[0] - 0.5,
                    bounds[1] - 0.5,
                    bounds[2] + 0.5,
                    bounds[3] + 0.5,
                ]),
                shape: bounds,
                radii: [0.0; 4],
                color: straight(color),
                border: [0.0; 4],
                uv: [0.0; 4],
                clip: clip(clip_rect),
                widths: [0.0; 4],
                kind: [PATH, 0, first, segments.len() as u32],
            },
            None,
        );
    }

    /// Adds a transform that items may name by `kind[1]`; gives its index.
    pub fn add_transform(&mut self, transform: Affine) -> u32 {
        self.transforms.push(transform);
        self.transforms.len() as u32
    }

    /// The transforms by their index: the identity, then [`Canvas::transforms`].
    pub fn transform_table(&self) -> Vec<Affine> {
        std::iter::once(IDENTITY)
            .chain(self.transforms.iter().copied())
            .collect()
    }

    /// Adds `item`, its quad cut down to the pixels that its clip holds in part, unless
    /// [`Canvas::keep_whole`].
    fn push(&mut self, mut item: Item, image: Option<ImageId>) {
        let [left, top, right, bottom] = item.quad;
        if !(item.clip[0] < item.clip[2] && item.clip[1] < item.clip[3]) {
            return;
        }
        if self.keep_whole {
            self.items.push(item);
            self.item_images.push(image);
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
        // A glass's `uv` maps any point: it is not cut.
        if image.is_some() && item.kind[0] != GLASS {
            let [u0, v0, u1, v1] = item.uv;
            let u = |x: f32| u0 + (u1 - u0) * (x - left) / (right - left);
            let v = |y: f32| v0 + (v1 - v0) * (y - top) / (bottom - top);
            item.uv = [u(quad[0]), v(quad[1]), u(quad[2]), v(quad[3])];
        }
        item.quad = quad;
        self.items.push(item);
        self.item_images.push(image);
    }

    /// The runs of the first `count` items that one draw of S or T takes.
    pub fn batches(&self, count: usize) -> Vec<Batch> {
        let mut batches: Vec<Batch> = Vec::new();
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

impl Item {
    /// The item moved by `(dx, dy)` pixels: its quad, its shape and its clip; a glass's map of
    /// its image too.
    pub fn translated(mut self, dx: f32, dy: f32) -> Self {
        let by = |edges: [f32; 4]| [edges[0] + dx, edges[1] + dy, edges[2] + dx, edges[3] + dy];
        self.quad = by(self.quad);
        self.shape = by(self.shape);
        if self.clip != NO_CLIP {
            self.clip = by(self.clip);
        }
        if self.kind[0] == GLASS {
            self.uv[0] -= dx;
            self.uv[1] -= dy;
        }
        self
    }
}

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
pub fn mixed(color: [f32; 4], encode: bool) -> [f32; 4] {
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

fn radii(radii: Radii, rect: PixelRect) -> [f32; 4] {
    let largest = rect.width.min(rect.height) / 2.0;
    radii.map(|radius| radius.max(0.0).min(largest))
}

fn clip(clip: Option<PixelRect>) -> [f32; 4] {
    clip.map_or(NO_CLIP, PixelRect::edges)
}

pub fn outward(edges: [f32; 4]) -> [f32; 4] {
    [
        edges[0].floor(),
        edges[1].floor(),
        edges[2].ceil(),
        edges[3].ceil(),
    ]
}

fn erf(x: f32) -> f32 {
    let y = x.clamp(-10.0, 10.0) * std::f32::consts::FRAC_2_SQRT_PI;
    let yy = y * y;
    let z = y + (0.24295 + (0.03395 + 0.0104 * yy) * yy) * (y * yy);
    z / (1.0 + z * z).sqrt()
}

pub fn push_f32s(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

pub fn push_u32s(bytes: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

/// `items`, laid out as S's and T's shaders read them, their colours mixed as the target mixes
/// them: `slime-render`'s `item_bytes`, unchanged.
pub fn item_bytes(items: &[Item], encode: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(items.len() * ITEM_SIZE);
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
            BOX | GLASS => mix(item.border, 1),
            GRADIENT => mixed(item.border, encode),
            _ => item.border,
        };
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

/// The editor's screen of `render.canvas`, laid out for 1920 × 1080, scaled to `h`: a background,
/// 100 panels with their shadows, 2 000 boxes, 20 000 glyphs, 16 pictures; half the boxes and
/// glyphs clipped to the screen's left three quarters.
pub fn editor_screen(canvas: &mut Canvas, glyphs: ImageId, pictures: &[ImageId], w: f32, h: f32) {
    let scale = h / 1080.0;
    canvas.draw_box(&CanvasBox {
        rect: PixelRect::new(0.0, 0.0, w, h),
        radii: [0.0; 4],
        fill: LinearColor::rgb(0.05, 0.05, 0.06),
        border_width: 0.0,
        border: LinearColor::BLACK,
        clip: None,
    });
    for index in 0..100 {
        let (x, y) = (
            (index % 10) as f32 * w / 10.0,
            (index / 10) as f32 * h / 10.0,
        );
        canvas.draw_shadow(&CanvasShadow {
            rect: PixelRect::new(x + 8.0, y + 8.0, w / 10.0 - 16.0, h / 10.0 - 16.0),
            radii: [6.0 * scale; 4],
            color: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
            offset: [0.0, 4.0 * scale],
            blur: 16.0 * scale,
            spread: 0.0,
            clip: None,
        });
    }
    let clip = |index: usize| {
        index
            .is_multiple_of(2)
            .then(|| PixelRect::new(0.0, 0.0, w * 0.75, h))
    };
    for index in 0..2000 {
        let (column, row) = (index % 8, index / 8);
        canvas.draw_box(&CanvasBox {
            rect: PixelRect::new(
                column as f32 * w / 8.0 + 4.0,
                row as f32 * h / 250.0,
                w / 8.0 - 8.0,
                h / 250.0 - 1.0,
            ),
            radii: [3.0 * scale; 4],
            fill: LinearColor::rgb(0.12, 0.12, 0.14),
            border_width: if index % 4 == 0 { scale } else { 0.0 },
            border: LinearColor::rgb(0.3, 0.3, 0.35),
            clip: clip(index),
        });
    }
    let (glyph_w, glyph_h) = (8.0 * scale, 14.0 * scale);
    let per_row = (w / glyph_w) as usize;
    for index in 0..20_000 {
        let cell = index % 4096;
        let (u, v) = ((cell % 64) as f32 / 64.0, (cell / 64) as f32 / 64.0);
        canvas.draw_image(&ImageQuad {
            rect: PixelRect::new(
                (index % per_row) as f32 * glyph_w,
                (index / per_row) as f32 * glyph_h * 1.3 % h,
                glyph_w,
                glyph_h,
            ),
            image: glyphs,
            part: ImagePart {
                x: u,
                y: v,
                width: 1.0 / 64.0,
                height: 1.0 / 64.0,
            },
            kind: ImageKind::Coverage,
            color: LinearColor::rgb(0.9, 0.9, 0.9),
            clip: clip(index),
        });
    }
    for (index, &image) in pictures.iter().enumerate() {
        canvas.draw_image(&ImageQuad {
            rect: PixelRect::new(
                index as f32 * 132.0 * scale,
                h - 140.0 * scale,
                128.0 * scale,
                128.0 * scale,
            ),
            image,
            part: ImagePart::WHOLE,
            kind: ImageKind::Color,
            color: LinearColor::WHITE,
            clip: None,
        });
    }
}

/// The four panels of the game's menu, laid out for 1920 × 1080: x, y, width, height. The last is
/// the list on the right.
pub const GAME_PANELS: [(f32, f32, f32, f32); 4] = [
    (60.0, 80.0, 820.0, 860.0),
    (920.0, 80.0, 500.0, 520.0),
    (920.0, 640.0, 500.0, 300.0),
    (1460.0, 80.0, 400.0, 860.0),
];

/// The game menu's 16 cards along its bottom, as [`GAME_PANELS`].
pub fn game_cards() -> impl Iterator<Item = (f32, f32, f32, f32)> {
    (0..16).map(|index| (60.0 + index as f32 * 112.0, 960.0, 100.0, 100.0))
}

/// How the game's menu, laid out for 1920 × 1080, is scaled to fit a target and centred.
#[derive(Clone, Copy, Debug)]
pub struct GameLayout {
    pub scale: f32,
    pub left: f32,
    pub top: f32,
}

impl GameLayout {
    pub fn new(w: f32, h: f32) -> Self {
        let scale = (w / 1920.0).min(h / 1080.0);
        let (left, top) = ((w - 1920.0 * scale) / 2.0, (h - 1080.0 * scale) / 2.0);
        Self { scale, left, top }
    }

    /// A rectangle of the layout for 1920 × 1080, in the target's pixels.
    pub fn rect(self, x: f32, y: f32, width: f32, height: f32) -> PixelRect {
        PixelRect::new(
            self.left + x * self.scale,
            self.top + y * self.scale,
            width * self.scale,
            height * self.scale,
        )
    }
}

/// The game's menu of `render.canvas`, laid out for 1920 × 1080, scaled to fit `w` × `h`, centred:
/// some 290 boxes, 20 shadows, 3 900 glyphs and 50 icons.
pub fn game_screen(canvas: &mut Canvas, glyphs: ImageId, icons: ImageId, w: f32, h: f32) {
    let layout = GameLayout::new(w, h);
    let scale = layout.scale;
    let rect = |x: f32, y: f32, width: f32, height: f32| layout.rect(x, y, width, height);
    let plain = |rect: PixelRect, radius: f32, fill: LinearColor| CanvasBox {
        rect,
        radii: [radius * scale; 4],
        fill,
        border_width: 0.0,
        border: LinearColor::BLACK,
        clip: None,
    };
    let mut cell = 0;
    let mut text = |canvas: &mut Canvas,
                    (x, y): (f32, f32),
                    count: usize,
                    per_line: usize,
                    clip: Option<PixelRect>| {
        for index in 0..count {
            let (u, v) = ((cell % 64) as f32 / 64.0, (cell / 64 % 64) as f32 / 64.0);
            cell += 1;
            canvas.draw_image(&ImageQuad {
                rect: rect(
                    x + (index % per_line) as f32 * 8.0,
                    y + (index / per_line) as f32 * 16.0,
                    8.0,
                    14.0,
                ),
                image: glyphs,
                part: ImagePart {
                    x: u,
                    y: v,
                    width: 1.0 / 64.0,
                    height: 1.0 / 64.0,
                },
                kind: ImageKind::Coverage,
                color: LinearColor::rgb(0.9, 0.88, 0.8),
                clip,
            });
        }
    };

    canvas.draw_box(&plain(
        PixelRect::new(0.0, 0.0, w, h),
        0.0,
        LinearColor::rgba(0.0, 0.0, 0.0, 0.6),
    ));
    for (x, y, width, height) in GAME_PANELS.into_iter().chain(game_cards()) {
        canvas.draw_shadow(&CanvasShadow {
            rect: rect(x, y, width, height),
            radii: [12.0 * scale; 4],
            color: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
            offset: [0.0, 8.0 * scale],
            blur: 24.0 * scale,
            spread: 0.0,
            clip: None,
        });
        canvas.draw_box(&CanvasBox {
            border_width: 2.0 * scale,
            border: LinearColor::rgb(0.5, 0.42, 0.3),
            ..plain(
                rect(x, y, width, height),
                12.0,
                LinearColor::rgba(0.08, 0.08, 0.1, 0.92),
            )
        });
        if height == 100.0 {
            text(canvas, (x + 34.0, y + 80.0), 4, 4, None);
        }
    }

    text(canvas, (80.0, 92.0), 20, 20, None);
    let slot = |index: usize| {
        (
            80.0 + (index % 10) as f32 * 76.0,
            120.0 + (index / 10) as f32 * 76.0,
        )
    };
    for index in 0..100 {
        let (x, y) = slot(index);
        canvas.draw_box(&CanvasBox {
            border_width: scale,
            border: LinearColor::rgb(0.35, 0.3, 0.22),
            ..plain(
                rect(x, y, 68.0, 68.0),
                6.0,
                LinearColor::rgb(0.05, 0.05, 0.06),
            )
        });
        canvas.draw_box(&plain(
            rect(x + 4.0, y + 4.0, 60.0, 60.0),
            4.0,
            LinearColor::rgba(0.2, 0.12, 0.3, 0.35),
        ));
        if index < 50 {
            let (u, v) = ((index % 8) as f32 / 8.0, (index / 8 % 8) as f32 / 8.0);
            canvas.draw_image(&ImageQuad {
                rect: rect(x + 6.0, y + 6.0, 56.0, 56.0),
                image: icons,
                part: ImagePart {
                    x: u,
                    y: v,
                    width: 1.0 / 8.0,
                    height: 1.0 / 8.0,
                },
                kind: ImageKind::Color,
                color: LinearColor::WHITE,
                clip: None,
            });
        }
    }
    for index in 0..100 {
        let (x, y) = slot(index);
        text(canvas, (x + 50.0, y + 52.0), 2, 2, None);
    }

    text(canvas, (1480.0, 96.0), 20, 20, None);
    let list = Some(rect(1460.0, 120.0, 400.0, 800.0));
    for row in 0..34 {
        let y = 130.0 + row as f32 * 23.0;
        let shade = if row % 2 == 0 { 0.1 } else { 0.07 };
        canvas.draw_box(&CanvasBox {
            clip: list,
            ..plain(
                rect(1480.0, y, 360.0, 21.0),
                3.0,
                LinearColor::rgb(shade, shade, shade),
            )
        });
        text(canvas, (1484.0, y + 4.0), 44, 44, list);
    }

    for index in 0..30 {
        let (x, y) = (
            936.0 + (index % 3) as f32 * 160.0,
            656.0 + (index / 3) as f32 * 28.0,
        );
        canvas.draw_box(&plain(
            rect(x, y, 156.0, 24.0),
            4.0,
            LinearColor::rgb(0.18, 0.15, 0.12),
        ));
        text(canvas, (x + 38.0, y + 5.0), 10, 10, None);
    }

    text(canvas, (940.0, 96.0), 20, 20, None);
    text(
        canvas,
        (940.0, 124.0),
        32 * 56,
        56,
        Some(rect(920.0, 80.0, 500.0, 520.0)),
    );
}

/// Not a screen of `render.canvas`: a few items of every kind, the gradient among them, which
/// neither screen draws, each also cut by a clip that ends between two pixels and with a
/// translucent colour, so that the image comparison covers every kind's every field. Its images
/// are compared; it is not timed.
pub fn kinds_screen(canvas: &mut Canvas, glyphs: ImageId, icons: ImageId, w: f32, h: f32) {
    canvas.draw_box(&CanvasBox {
        rect: PixelRect::new(0.0, 0.0, w, h),
        radii: [0.0; 4],
        fill: LinearColor::rgb(0.8, 0.82, 0.85),
        border_width: 0.0,
        border: LinearColor::BLACK,
        clip: None,
    });
    let clips = [
        None,
        Some(PixelRect::new(10.3, 10.6, 1000.45, 600.2)),
        Some(PixelRect::new(37.7, 50.25, 61.5, 33.35)),
    ];
    for (row, clip) in clips.into_iter().enumerate() {
        let y = 20.0 + row as f32 * 250.0;
        for column in 0..4 {
            let x = 20.5 + column as f32 * 300.0;
            let alpha = 1.0 - column as f32 * 0.2;
            canvas.draw_shadow(&CanvasShadow {
                rect: PixelRect::new(x, y, 120.3, 80.7),
                radii: [0.0, 6.0, 30.0, 12.5],
                color: LinearColor::rgba(0.1, 0.0, 0.2, 0.6 * alpha),
                offset: [3.0, 7.5],
                blur: column as f32 * 9.0,
                spread: column as f32 - 1.0,
                clip,
            });
            canvas.draw_box(&CanvasBox {
                rect: PixelRect::new(x, y, 120.3, 80.7),
                radii: [0.0, 6.0, 30.0, 12.5],
                fill: LinearColor::rgba(0.9, 0.3, 0.1, alpha),
                border_width: column as f32 * 1.7,
                border: LinearColor::rgba(0.05, 0.4, 0.9, alpha),
                clip,
            });
            canvas.draw_gradient(&CanvasGradient {
                rect: PixelRect::new(x + 140.2, y, 110.6, 90.1),
                radii: [20.0, 0.0, 4.5, 44.0],
                corners: [
                    LinearColor::rgba(1.0, 0.0, 0.0, alpha),
                    LinearColor::rgba(0.0, 1.0, 0.0, 1.0),
                    LinearColor::rgba(0.0, 0.0, 1.0, 0.5 * alpha),
                    LinearColor::rgba(1.0, 1.0, 1.0, alpha),
                ],
                clip,
            });
            for glyph in 0..24 {
                canvas.draw_image(&ImageQuad {
                    rect: PixelRect::new(
                        x + (glyph % 12) as f32 * 9.3,
                        y + 110.4 + (glyph / 12) as f32 * 17.1,
                        8.0 + column as f32,
                        14.0 + column as f32,
                    ),
                    image: glyphs,
                    part: ImagePart {
                        x: (glyph * 3 % 64) as f32 / 64.0,
                        y: (glyph * 7 % 64) as f32 / 64.0,
                        width: 1.0 / 64.0,
                        height: 1.0 / 64.0,
                    },
                    kind: ImageKind::Coverage,
                    color: LinearColor::rgba(0.1, 0.1, 0.4, alpha),
                    clip,
                });
            }
            canvas.draw_image(&ImageQuad {
                rect: PixelRect::new(x + 140.6, y + 100.3, 96.4, 96.4),
                image: icons,
                part: ImagePart {
                    x: 0.125 * column as f32,
                    y: 0.25,
                    width: 0.25,
                    height: 0.25,
                },
                kind: ImageKind::Color,
                color: LinearColor::rgba(1.0, 1.0, 1.0, alpha),
                clip,
            });
        }
    }
}
