//! The canvas (`docs/adr/0137-the-canvas.md`), read back from a target of floats: a box fills
//! inside its rounded corners and its border inside its edges; a shadow fades from under its box,
//! where its offset puts it; a gradient mixes its corners' colors; a clip cuts what lies outside
//! it; an image is drawn in a color or as it is, and so is a view's target; items are drawn in
//! their order, few draws for many items; shapes drawn at twice the scale keep their place; the
//! canvas's shader is replaced with the others; a lost device stops it, and a new renderer draws it
//! again. Runs on whatever adapter the machine offers.

use slime_color::LinearColor;
use slime_gfx::{FloatImage, GfxError, Gpu, RenderPassDescriptor, RenderTarget, TextureFormat};
use slime_render::{
    Canvas, CanvasBox, CanvasGradient, CanvasImage, CanvasShadow, ImageKind, ImagePart, ImageQuad,
    MAX_CANVAS_ITEMS, PixelRect, RenderError, Renderer, ShaderSources,
};

const SIZE: u32 = 64;
const FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const BLUE: LinearColor = LinearColor::rgb(0.0, 0.0, 1.0);
const RED: LinearColor = LinearColor::rgb(1.0, 0.0, 0.0);
const GREY: LinearColor = LinearColor::rgb(0.5, 0.5, 0.5);

struct Setup {
    gpu: Gpu,
    renderer: Renderer,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
        Self { gpu, renderer }
    }

    /// A target of `SIZE` × `SIZE` pixels, cleared to black.
    fn target(&self, label: &str) -> RenderTarget {
        let target = self
            .gpu
            .create_render_target(label, SIZE, SIZE, FORMAT)
            .expect("the GPU has memory for it");
        self.gpu.render(
            &RenderPassDescriptor {
                target: &target,
                clear: Some(LinearColor::BLACK),
                depth: None,
                timer: None,
                resolve: None,
            },
            |_| {},
        );
        target
    }

    /// `canvas` drawn over a black target, read back.
    fn draw(&mut self, canvas: &Canvas<'_>) -> FloatImage {
        let target = self.target("canvas");
        self.renderer
            .draw_canvas(&self.gpu, canvas, &target)
            .expect("drawn");
        self.gpu
            .read_float_render_target(&target)
            .expect("read back")
    }
}

/// The red, green, blue and alpha of the pixel at (`x`, `y`).
fn pixel(image: &FloatImage, (x, y): (u32, u32)) -> [f32; 4] {
    let at = ((y * image.width + x) * 4) as usize;
    [
        image.pixels[at],
        image.pixels[at + 1],
        image.pixels[at + 2],
        image.pixels[at + 3],
    ]
}

fn close(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 0.02
}

fn shape(rect: PixelRect) -> CanvasBox {
    CanvasBox {
        rect,
        radii: [0.0; 4],
        fill: BLUE,
        border_width: 0.0,
        border: RED,
        clip: None,
    }
}

#[test]
fn a_box_fills_inside_its_rounded_corners_and_its_border_inside_its_edges() {
    let mut setup = Setup::new();
    let mut canvas = Canvas::new();
    canvas.draw_box(&CanvasBox {
        radii: [16.0, 16.0, 0.0, 0.0],
        border_width: 4.0,
        ..shape(PixelRect::new(8.0, 8.0, 48.0, 48.0))
    });
    let image = setup.draw(&canvas);
    // The fill, inside the border.
    assert_eq!(pixel(&image, (32, 32))[..3], [0.0, 0.0, 1.0]);
    // The border, 4 pixels wide inside each edge: its left at x 8 to 11, its bottom at y 52 to 55.
    for at in [(9, 40), (11, 40), (32, 54)] {
        let [r, g, b, _] = pixel(&image, at);
        assert!(close(r, 1.0) && g == 0.0 && b == 0.0, "{at:?}: {r} {g} {b}");
    }
    assert_eq!(pixel(&image, (12, 40))[..3], [0.0, 0.0, 1.0]);
    // Outside the box, and in its top left corner beyond the arc, nothing; its bottom left corner
    // is square.
    for at in [(4, 32), (32, 60), (9, 9), (10, 10)] {
        assert_eq!(pixel(&image, at)[..3], [0.0; 3], "{at:?}");
    }
    assert!(close(pixel(&image, (8, 55))[0], 1.0));
    // Along the arc of the top left corner, centred on (24, 24), radius 16: an edge between, a
    // pixel covered in part.
    let [r, ..] = pixel(&image, (12, 13));
    assert!(r > 0.05 && r < 0.95, "{r}");
}

#[test]
fn a_shadow_fades_from_under_its_box_where_its_offset_puts_it() {
    let mut setup = Setup::new();
    let mut canvas = Canvas::new();
    canvas.draw_shadow(&CanvasShadow {
        rect: PixelRect::new(16.0, 16.0, 24.0, 24.0),
        radii: [4.0; 4],
        color: LinearColor::rgba(1.0, 1.0, 1.0, 0.8),
        offset: [4.0, 4.0],
        blur: 8.0,
        spread: 0.0,
        clip: None,
    });
    let image = setup.draw(&canvas);
    // Whole under the middle of the box moved by its offset, (32, 32): its alpha.
    assert!(
        close(pixel(&image, (32, 32))[0], 0.8),
        "{:?}",
        pixel(&image, (32, 32))
    );
    // Fading out from there, to nothing three deviations beyond its edge, at x 44 + 12.
    let row: Vec<f32> = (32..64).map(|x| pixel(&image, (x, 32))[0]).collect();
    assert!(
        row.windows(2).all(|pair| pair[1] <= pair[0] + 1e-4),
        "{row:?}"
    );
    assert!(row[44 - 32] > 0.2 && row[44 - 32] < 0.6, "{row:?}");
    assert!(row[60 - 32] < 0.01, "{row:?}");
    // Shifted by its offset: of two points as far from the box, 4 pixels left and right of it, the
    // right one, on the shadow's edge, is the brighter.
    assert!(pixel(&image, (12, 28))[0] < pixel(&image, (44, 28))[0]);

    // Without blur, a sharp edge.
    let mut sharp = Canvas::new();
    sharp.draw_shadow(&CanvasShadow {
        rect: PixelRect::new(16.0, 16.0, 24.0, 24.0),
        radii: [0.0; 4],
        color: LinearColor::WHITE,
        offset: [0.0, 0.0],
        blur: 0.0,
        spread: 2.0,
        clip: None,
    });
    let image = setup.draw(&sharp);
    assert!(close(pixel(&image, (14, 28))[0], 1.0));
    assert_eq!(pixel(&image, (13, 28))[0], 0.0);
}

#[test]
fn a_shadow_is_its_box_blurred_by_a_gaussian_within_a_few_hundredths() {
    let mut setup = Setup::new();
    // A square, a wide panel, a circle, corners of two radii, and a box smaller than its blur, each
    // with the largest error that the approximation makes of it, as `draw_shadow` states: the
    // circle, its blur near its radius, is the worst.
    let shadows = [
        (PixelRect::new(20.0, 20.0, 24.0, 24.0), [4.0; 4], 8.0, 0.015),
        (PixelRect::new(8.0, 24.0, 48.0, 16.0), [6.0; 4], 6.0, 0.03),
        (
            PixelRect::new(16.0, 16.0, 32.0, 32.0),
            [16.0; 4],
            10.0,
            0.06,
        ),
        (
            PixelRect::new(12.0, 20.0, 40.0, 24.0),
            [12.0, 0.0, 12.0, 0.0],
            4.0,
            0.04,
        ),
        (PixelRect::new(28.0, 28.0, 8.0, 8.0), [2.0; 4], 12.0, 0.02),
    ];
    for (rect, radii, blur, largest) in shadows {
        let mut canvas = Canvas::new();
        canvas.draw_shadow(&CanvasShadow {
            rect,
            radii,
            color: LinearColor::WHITE,
            offset: [0.0, 0.0],
            blur,
            spread: 0.0,
            clip: None,
        });
        let image = setup.draw(&canvas);
        // The canvas counts a pixel's own width in the blur, a variance of 1/12.
        let sigma = (f64::from(blur / 2.0).powi(2) + 1.0 / 12.0).sqrt();
        let mut worst = (0.0, (0, 0));
        for y in 0..SIZE {
            for x in 0..SIZE {
                let center = [f64::from(x) + 0.5, f64::from(y) + 0.5];
                let error = (f64::from(pixel(&image, (x, y))[0])
                    - blurred_box(center, rect, radii, sigma))
                .abs();
                if error > worst.0 {
                    worst = (error, (x, y));
                }
            }
        }
        assert!(worst.0 < largest, "{rect:?} {radii:?} {blur}: {worst:?}");
    }
}

/// How much of the box `rect` with corners of `radii`, blurred by a Gaussian of deviation `sigma`,
/// reaches `point`: the integral over the box's rows, a thousand within six deviations, of the
/// Gaussian's integral across each row.
fn blurred_box(point: [f64; 2], rect: PixelRect, radii: [f32; 4], sigma: f64) -> f64 {
    let [left, top, right, bottom] = [
        f64::from(rect.x),
        f64::from(rect.y),
        f64::from(rect.x + rect.width),
        f64::from(rect.y + rect.height),
    ];
    let [top_left, top_right, bottom_right, bottom_left] = radii.map(f64::from);
    // How far a corner of `radius` takes the row at `y` in from its side.
    let inset = |radius: f64, y: f64| {
        let into = (top + radius - y).max(y - (bottom - radius)).max(0.0);
        if radius > 0.0 && into > 0.0 {
            radius - (radius * radius - into * into).max(0.0).sqrt()
        } else {
            0.0
        }
    };
    let low = top.max(point[1] - 6.0 * sigma);
    let high = bottom.min(point[1] + 6.0 * sigma);
    if low >= high {
        return 0.0;
    }
    let rows = 1000;
    let step = (high - low) / f64::from(rows);
    let scale = 1.0 / (std::f64::consts::SQRT_2 * sigma);
    (0..rows)
        .map(|row| {
            let y = low + (f64::from(row) + 0.5) * step;
            let upper = y < (top + bottom) / 2.0;
            let start = left + inset(if upper { top_left } else { bottom_left }, y);
            let end = right - inset(if upper { top_right } else { bottom_right }, y);
            let across = 0.5 * (erf((end - point[0]) * scale) - erf((start - point[0]) * scale));
            let along = (-0.5 * ((point[1] - y) / sigma).powi(2)).exp()
                / ((2.0 * std::f64::consts::PI).sqrt() * sigma);
            across * along * step
        })
        .sum()
}

/// The error function, to 1.5 × 10⁻⁷ (Abramowitz and Stegun, 7.1.26).
fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let polynomial = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    x.signum() * (1.0 - polynomial * (-x * x).exp())
}

#[test]
fn into_an_srgb_target_colors_mix_as_srgb_values_and_into_another_as_linear_ones() {
    let mut setup = Setup::new();
    let gpu = setup.gpu.clone();
    // Black of alpha one half over white, and the same at a pixel that a box covers by half; then
    // an image of a grey of half the light, sRGB's 188.
    let grey = gpu
        .create_texture(
            "grey",
            1,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[188, 188, 188, 255],
        )
        .expect("the GPU has memory for it");
    let mut canvas = Canvas::new();
    let grey_image = canvas.add_image(CanvasImage::Texture(&grey));
    canvas.draw_box(&CanvasBox {
        fill: LinearColor::WHITE,
        ..shape(PixelRect::new(0.0, 0.0, 64.0, 64.0))
    });
    canvas.draw_box(&CanvasBox {
        fill: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
        ..shape(PixelRect::new(0.0, 0.0, 32.0, 64.0))
    });
    canvas.draw_box(&CanvasBox {
        fill: LinearColor::BLACK,
        ..shape(PixelRect::new(32.0, 0.0, 16.5, 64.0))
    });
    canvas.draw_image(&ImageQuad {
        rect: PixelRect::new(56.0, 0.0, 8.0, 64.0),
        image: grey_image,
        part: ImagePart::WHOLE,
        kind: ImageKind::Color,
        color: LinearColor::WHITE,
        clip: None,
    });
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
    let target = gpu
        .create_render_target("srgb", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    renderer.draw_canvas(&gpu, &canvas, &target).expect("drawn");
    let image = gpu.read_render_target(&target).expect("read back");
    // Half of white's 255 as sRGB stores it, where linear mixing stores 188, its encoding of 0.5.
    for x in [16, 48] {
        let at = ((32 * SIZE + x) * 4) as usize;
        let red = image.pixels[at];
        assert!(red.abs_diff(128) <= 1, "{x}: {red}");
    }
    let red = image.pixels[((32 * SIZE + 60) * 4) as usize];
    assert!(red.abs_diff(188) <= 1, "{red}");
    // Into a target of floats, linear: one half.
    let linear = setup.draw(&canvas);
    for x in [16, 48] {
        assert!(close(pixel(&linear, (x, 32))[0], 0.5));
    }
    assert!(close(pixel(&linear, (60, 32))[0], 0.5));
}

#[test]
fn into_an_srgb_target_a_border_s_color_is_encoded_and_a_shadow_keeps_its_shape() {
    let mut setup = Setup::new();
    let gpu = setup.gpu.clone();
    // A border of sRGB's 128, and a sharp shadow whose top left corner is rounded by 16.
    let mut canvas = Canvas::new();
    canvas.draw_box(&CanvasBox {
        fill: LinearColor::BLACK,
        border_width: 4.0,
        border: LinearColor::from_srgb8(128, 128, 128),
        ..shape(PixelRect::new(0.0, 0.0, 32.0, 64.0))
    });
    canvas.draw_shadow(&CanvasShadow {
        rect: PixelRect::new(32.0, 0.0, 32.0, 32.0),
        radii: [16.0, 0.0, 0.0, 0.0],
        color: LinearColor::WHITE,
        offset: [0.0, 0.0],
        blur: 0.0,
        spread: 0.0,
        clip: None,
    });
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
    let target = gpu
        .create_render_target("srgb", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    renderer.draw_canvas(&gpu, &canvas, &target).expect("drawn");
    let image = gpu.read_render_target(&target).expect("read back");
    let red = |(x, y): (u32, u32)| image.pixels[((y * SIZE + x) * 4) as usize];
    assert!(red((1, 32)).abs_diff(128) <= 1, "{}", red((1, 32)));
    // On the arc, the shadow covers as much of the pixel as in a target of floats: white over
    // black is its coverage in either.
    let linear = setup.draw(&canvas);
    let covered = pixel(&linear, (36, 4))[0];
    assert!(covered > 0.05 && covered < 0.95, "{covered}");
    let stored = f32::from(red((36, 4))) / 255.0;
    assert!((stored - covered).abs() < 0.02, "{stored} {covered}");
}

#[test]
fn a_clip_between_two_pixels_covers_them_in_part_and_an_empty_one_leaves_its_item_out() {
    let mut setup = Setup::new();
    let white = |clip: PixelRect| CanvasBox {
        fill: LinearColor::WHITE,
        clip: Some(clip),
        ..shape(PixelRect::new(0.0, 0.0, 64.0, 64.0))
    };
    let mut canvas = Canvas::new();
    canvas.draw_box(&white(PixelRect::new(10.25, 0.0, 20.0, 32.0)));
    // A tenth of a pixel wide.
    canvas.draw_box(&white(PixelRect::new(40.3, 0.0, 0.1, 64.0)));
    let image = setup.draw(&canvas);
    assert!(close(pixel(&image, (10, 16))[0], 0.75));
    assert!(close(pixel(&image, (20, 16))[0], 1.0));
    assert!(close(pixel(&image, (30, 16))[0], 0.25));
    assert_eq!(pixel(&image, (31, 16))[0], 0.0);
    assert!(close(pixel(&image, (40, 48))[0], 0.1));
    // Empty, and inside out.
    let mut empty = Canvas::new();
    empty.draw_box(&white(PixelRect::new(10.5, 0.0, 0.0, 64.0)));
    empty.draw_box(&white(PixelRect::new(10.7, 0.0, -0.4, 64.0)));
    assert!(empty.is_empty());
    let image = setup.draw(&empty);
    assert_eq!(pixel(&image, (10, 32))[0], 0.0);
}

#[test]
fn an_image_cut_by_its_clip_keeps_its_place() {
    let mut setup = Setup::new();
    // Red, red, blue, blue, across 64 pixels, its left half clipped away.
    let texture = setup
        .gpu
        .create_texture(
            "halves",
            4,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[
                255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255,
            ],
        )
        .expect("the GPU has memory for it");
    let mut canvas = Canvas::new();
    let image = canvas.add_image(CanvasImage::Texture(&texture));
    canvas.draw_image(&ImageQuad {
        rect: PixelRect::new(0.0, 0.0, 64.0, 64.0),
        image,
        part: ImagePart::WHOLE,
        kind: ImageKind::Color,
        color: LinearColor::WHITE,
        clip: Some(PixelRect::new(32.0, 0.0, 32.0, 64.0)),
    });
    let drawn = setup.draw(&canvas);
    assert_eq!(pixel(&drawn, (16, 32))[..3], [0.0; 3]);
    let [r, g, b, _] = pixel(&drawn, (48, 32));
    assert!(close(r, 0.0) && g == 0.0 && close(b, 1.0), "{r} {g} {b}");
}

#[test]
fn past_its_bound_a_canvas_leaves_its_last_items_out() {
    let mut setup = Setup::new();
    let mut canvas = Canvas::new();
    for _ in 0..MAX_CANVAS_ITEMS + 3 {
        canvas.draw_box(&shape(PixelRect::new(0.0, 0.0, 1.0, 1.0)));
    }
    let target = setup.target("full");
    let stats = setup
        .renderer
        .draw_canvas(&setup.gpu, &canvas, &target)
        .expect("drawn");
    assert_eq!((stats.items, stats.left_out), (MAX_CANVAS_ITEMS, 3));
}

#[test]
#[should_panic(expected = "the renderer draws canvases into")]
fn a_target_of_another_format_is_refused() {
    let mut setup = Setup::new();
    let target = setup
        .gpu
        .create_render_target("other", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let mut canvas = Canvas::new();
    canvas.draw_box(&shape(PixelRect::new(0.0, 0.0, 8.0, 8.0)));
    let _ = setup.renderer.draw_canvas(&setup.gpu, &canvas, &target);
}

#[test]
#[should_panic(expected = "a canvas draws into targets of one sample per pixel")]
fn a_target_of_several_samples_is_refused() {
    let mut setup = Setup::new();
    let target = setup
        .gpu
        .create_multisampled_render_target("samples", SIZE, SIZE, FORMAT)
        .expect("the GPU has memory for it");
    let mut canvas = Canvas::new();
    canvas.draw_box(&shape(PixelRect::new(0.0, 0.0, 8.0, 8.0)));
    let _ = setup.renderer.draw_canvas(&setup.gpu, &canvas, &target);
}

#[test]
fn a_clip_cuts_what_lies_outside_it() {
    let mut setup = Setup::new();
    let mut canvas = Canvas::new();
    canvas.draw_box(&CanvasBox {
        clip: Some(PixelRect::new(0.0, 0.0, 32.0, 64.0)),
        ..shape(PixelRect::new(8.0, 8.0, 48.0, 48.0))
    });
    canvas.draw_shadow(&CanvasShadow {
        rect: PixelRect::new(0.0, 0.0, 64.0, 8.0),
        radii: [0.0; 4],
        color: LinearColor::WHITE,
        offset: [0.0, 0.0],
        blur: 0.0,
        spread: 0.0,
        clip: Some(PixelRect::new(16.0, 0.0, 16.0, 64.0)),
    });
    let image = setup.draw(&canvas);
    assert_eq!(pixel(&image, (31, 32))[..3], [0.0, 0.0, 1.0]);
    assert_eq!(pixel(&image, (32, 32))[..3], [0.0; 3]);
    assert_eq!(pixel(&image, (20, 4))[..3], [1.0; 3]);
    assert_eq!(pixel(&image, (15, 4))[..3], [0.0; 3]);
    assert_eq!(pixel(&image, (32, 4))[..3], [0.0; 3]);
}

#[test]
fn an_image_is_drawn_in_a_color_or_as_it_is() {
    let mut setup = Setup::new();
    // Two texels of coverage, none then whole; two of colors, opaque red then transparent.
    let coverage = setup
        .gpu
        .create_texture_with_levels("coverage", 2, 1, TextureFormat::R8Unorm, &[&[0, 255]])
        .expect("the GPU has memory for it");
    let colors = setup
        .gpu
        .create_texture(
            "colors",
            2,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[255, 0, 0, 255, 0, 255, 0, 0],
        )
        .expect("the GPU has memory for it");
    let mut canvas = Canvas::new();
    let coverage = canvas.add_image(CanvasImage::Texture(&coverage));
    let colors = canvas.add_image(CanvasImage::Texture(&colors));
    let quad = |image, kind, color, y| ImageQuad {
        rect: PixelRect::new(0.0, y, 64.0, 16.0),
        image,
        part: ImagePart::WHOLE,
        kind,
        color,
        clip: None,
    };
    canvas.draw_image(&quad(coverage, ImageKind::Coverage, GREY, 0.0));
    canvas.draw_image(&quad(
        colors,
        ImageKind::Color,
        LinearColor::rgba(1.0, 1.0, 1.0, 0.5),
        32.0,
    ));
    let image = setup.draw(&canvas);
    // Coverage: none at the left, the color at the right, between in the middle.
    assert_eq!(pixel(&image, (4, 8))[..3], [0.0; 3]);
    assert!(close(pixel(&image, (60, 8))[0], 0.5));
    let middle = pixel(&image, (32, 8))[0];
    assert!(middle > 0.1 && middle < 0.4, "{middle}");
    // Colors: the red, faded by half; the transparent texel lays nothing, not its green.
    let [r, g, b, _] = pixel(&image, (4, 40));
    assert!(close(r, 0.5) && g == 0.0 && b == 0.0, "{r} {g} {b}");
    assert_eq!(pixel(&image, (60, 40))[..3], [0.0; 3]);
}

#[test]
fn a_target_that_a_view_drew_is_drawn_as_an_image_into_a_panel() {
    let mut setup = Setup::new();
    // The view's output: blue at its left half, red at its right.
    let view = setup.target("view");
    let mut drawn = Canvas::new();
    drawn.draw_box(&shape(PixelRect::new(0.0, 0.0, 32.0, 64.0)));
    drawn.draw_box(&CanvasBox {
        fill: RED,
        ..shape(PixelRect::new(32.0, 0.0, 32.0, 64.0))
    });
    setup
        .renderer
        .draw_canvas(&setup.gpu, &drawn, &view)
        .expect("drawn");
    // A panel of 32 × 32 pixels at (16, 16) that shows it, shrunk by half.
    let mut canvas = Canvas::new();
    let shown = canvas.add_image(CanvasImage::Target(&view));
    canvas.draw_image(&ImageQuad {
        rect: PixelRect::new(16.0, 16.0, 32.0, 32.0),
        image: shown,
        part: ImagePart::WHOLE,
        kind: ImageKind::Color,
        color: LinearColor::WHITE,
        clip: None,
    });
    let image = setup.draw(&canvas);
    assert_eq!(pixel(&image, (20, 32))[..3], [0.0, 0.0, 1.0]);
    assert_eq!(pixel(&image, (44, 32))[..3], [1.0, 0.0, 0.0]);
    assert_eq!(pixel(&image, (8, 32))[..3], [0.0; 3]);
}

#[test]
fn items_are_drawn_in_their_order_few_draws_for_many() {
    let mut setup = Setup::new();
    let texture = |label| {
        setup
            .gpu
            .create_texture_with_levels(label, 1, 1, TextureFormat::R8Unorm, &[&[255]])
            .expect("the GPU has memory for it")
    };
    let (first, second) = (texture("first"), texture("second"));
    let mut canvas = Canvas::new();
    let first = canvas.add_image(CanvasImage::Texture(&first));
    let second = canvas.add_image(CanvasImage::Texture(&second));
    let glyph = |image, x| ImageQuad {
        rect: PixelRect::new(x, 40.0, 8.0, 8.0),
        image,
        part: ImagePart::WHOLE,
        kind: ImageKind::Coverage,
        color: LinearColor::WHITE,
        clip: None,
    };
    // A box, a glyph, a box over the first, a glyph: one image, one draw; then the other image.
    canvas.draw_box(&shape(PixelRect::new(0.0, 0.0, 32.0, 32.0)));
    canvas.draw_image(&glyph(first, 0.0));
    canvas.draw_box(&CanvasBox {
        fill: RED,
        ..shape(PixelRect::new(16.0, 16.0, 32.0, 32.0))
    });
    canvas.draw_image(&glyph(first, 16.0));
    canvas.draw_image(&glyph(second, 32.0));
    let target = setup.target("ordered");
    let stats = setup
        .renderer
        .draw_canvas(&setup.gpu, &canvas, &target)
        .expect("drawn");
    assert_eq!((stats.items, stats.draws, stats.left_out), (5, 2, 0));
    let image = setup.gpu.read_float_render_target(&target).expect("read");
    assert_eq!(pixel(&image, (24, 24))[..3], [1.0, 0.0, 0.0]);
    assert_eq!(pixel(&image, (8, 8))[..3], [0.0, 0.0, 1.0]);
    assert_eq!(pixel(&image, (36, 44))[..3], [1.0; 3]);
}

#[test]
fn shapes_drawn_at_twice_the_scale_keep_their_place() {
    let mut setup = Setup::new();
    let at = |scale: f32| {
        let mut canvas = Canvas::new();
        canvas.draw_box(&CanvasBox {
            radii: [6.0 * scale; 4],
            border_width: 2.0 * scale,
            ..shape(PixelRect::new(
                4.0 * scale,
                4.0 * scale,
                20.0 * scale,
                20.0 * scale,
            ))
        });
        canvas
    };
    let one = setup.draw(&at(1.0));
    let two = setup.draw(&at(2.0));
    // A pixel of the first, and the four that it becomes in the second: each the same part.
    for (x, y) in [(14, 14), (5, 14), (14, 5), (5, 5), (8, 6)] {
        let small = pixel(&one, (x, y));
        let large: f32 = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .map(|(dx, dy)| {
                pixel(&two, (2 * x + dx, 2 * y + dy))[0] + pixel(&two, (2 * x + dx, 2 * y + dy))[2]
            })
            .sum::<f32>()
            / 4.0;
        assert!(
            (small[0] + small[2] - large).abs() < 0.15,
            "({x}, {y}): {small:?} against {large}"
        );
    }
}

#[test]
fn the_canvas_s_shader_is_replaced_with_the_others_and_a_broken_one_refused() {
    let mut setup = Setup::new();
    let mut canvas = Canvas::new();
    canvas.draw_box(&shape(PixelRect::new(0.0, 0.0, 64.0, 64.0)));
    let embedded = ShaderSources::embedded();
    let output = "return color * clipped;";
    assert!(embedded.canvas.contains(output));
    let swapped = ShaderSources {
        canvas: embedded
            .canvas
            .replace(output, "return color.bgra * clipped;"),
        ..embedded.clone()
    };
    setup
        .renderer
        .set_shaders(&setup.gpu, &swapped)
        .expect("compiled");
    assert_eq!(pixel(&setup.draw(&canvas), (32, 32))[..3], [1.0, 0.0, 0.0]);
    let broken = ShaderSources {
        canvas: format!("fn broken() {{ let x = ; }}\n{}", embedded.canvas),
        ..embedded
    };
    let error = setup
        .renderer
        .set_shaders(&setup.gpu, &broken)
        .expect_err("refused");
    let RenderError::Gfx(GfxError::Shader { label, .. }) = &error else {
        panic!("{error:?}");
    };
    assert_eq!(label, "canvas.wgsl");
    // The shaders before, kept.
    assert_eq!(pixel(&setup.draw(&canvas), (32, 32))[..3], [1.0, 0.0, 0.0]);
}

#[test]
fn a_lost_device_stops_the_canvas_and_a_new_renderer_draws_it_again() {
    let mut setup = Setup::new();
    let mut canvas = Canvas::new();
    canvas.draw_box(&shape(PixelRect::new(0.0, 0.0, 64.0, 64.0)));
    let target = setup.target("lost");
    setup.gpu.lose();
    let error = setup
        .renderer
        .draw_canvas(&setup.gpu, &canvas, &target)
        .expect_err("the device is lost");
    assert!(
        matches!(error, RenderError::Gfx(GfxError::DeviceLost { .. })),
        "{error:?}"
    );
    let mut again = Setup::new();
    assert_eq!(pixel(&again.draw(&canvas), (32, 32))[..3], [0.0, 0.0, 1.0]);
}

#[test]
fn the_canvas_s_time_on_the_gpu_comes_back_where_the_adapter_times() {
    let mut setup = Setup::new();
    if !setup.gpu.can_time() {
        return;
    }
    let mut canvas = Canvas::new();
    canvas.draw_box(&shape(PixelRect::new(0.0, 0.0, 64.0, 64.0)));
    let target = setup.target("timed");
    let mut timed = None;
    for _ in 0..20 {
        let stats = setup
            .renderer
            .draw_canvas(&setup.gpu, &canvas, &target)
            .expect("drawn");
        setup.gpu.wait_idle().expect("the device is there");
        if stats.gpu_time.is_some() {
            timed = stats.gpu_time;
            break;
        }
    }
    assert!(timed.is_some(), "no time came back in 20 frames");
}

#[test]
fn a_gradient_mixes_its_corners_by_each_point_s_place() {
    let mut setup = Setup::new();
    let gpu = setup.gpu.clone();
    // White, red, black and black at its corners: a square of saturation and value.
    let mut canvas = Canvas::new();
    canvas.draw_gradient(&CanvasGradient {
        rect: PixelRect::new(0.0, 0.0, 64.0, 64.0),
        radii: [0.0; 4],
        corners: [
            LinearColor::WHITE,
            RED,
            LinearColor::BLACK,
            LinearColor::BLACK,
        ],
        clip: None,
    });
    // Into a target of floats, linear: the corners at the corners, their mixes between.
    let linear = setup.draw(&canvas);
    let top_left = pixel(&linear, (0, 0));
    let top_right = pixel(&linear, (63, 0));
    assert!(
        top_left[1] > 0.97 && top_right[1] < 0.03,
        "{top_left:?} {top_right:?}"
    );
    assert!(
        close(top_right[0], 1.0) || top_right[0] > 0.98,
        "{top_right:?}"
    );
    // Half way across the top: white mixed with red by half, green one half.
    let middle = pixel(&linear, (32, 0));
    assert!((middle[1] - 0.5).abs() < 0.03, "{middle:?}");
    // Half way down the left edge: white mixed with black by half.
    let down = pixel(&linear, (0, 32));
    assert!((down[0] - 0.5).abs() < 0.03, "{down:?}");
    assert!(pixel(&linear, (40, 63))[0] < 0.03, "black at the bottom");
    // Into an sRGB target, mixed on the encoding, as CSS mixes: half of 255, not linear's 188.
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
    let target = gpu
        .create_render_target("srgb", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    renderer.draw_canvas(&gpu, &canvas, &target).expect("drawn");
    let image = gpu.read_render_target(&target).expect("read back");
    let green = image.pixels[(32 * 4 + 1) as usize];
    assert!(green.abs_diff(128) <= 3, "{green}");
    // Rounded and clipped as a box is: its corner left out.
    let mut canvas = Canvas::new();
    canvas.draw_gradient(&CanvasGradient {
        rect: PixelRect::new(0.0, 0.0, 64.0, 64.0),
        radii: [16.0; 4],
        corners: [LinearColor::WHITE; 4],
        clip: Some(PixelRect::new(0.0, 0.0, 64.0, 32.0)),
    });
    let rounded = setup.draw(&canvas);
    assert_eq!(pixel(&rounded, (0, 0))[0], 0.0, "outside its corner");
    assert!(close(pixel(&rounded, (32, 16))[0], 1.0));
    assert_eq!(pixel(&rounded, (32, 48))[0], 0.0, "outside its clip");
}
