//! Display lists drawn through the canvas, read back from a target of floats and compared with the
//! same glyphs rasterized and laid over black by the CPU: the text of each writing system of
//! slime-ui's tests, at scales 1 and 2; an atlas within its budget, its glyphs that no draw asked
//! for the longest leaving first; images as the program gives them; a lost device, after which a
//! new renderer draws the text again; and a tree of elements drawn where its layout places it. Runs
//! on whatever adapter the machine offers.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use slime_color::LinearColor;
use slime_gfx::{FloatImage, GfxError, Gpu, RenderPassDescriptor, RenderTarget, TextureFormat};
use slime_render::{CanvasImage, RenderError, Renderer};
use slime_ui::Rect;
use slime_ui::display::{
    BoxPrimitive, DisplayList, ImageKey, ImageKind, ImagePrimitive, TextPrimitive,
};
use slime_ui::style::{Align, Dimension, Style};
use slime_ui::text::{Fonts, GlyphPixels, GlyphRasterizer, TextBounds, TextLayout, TextStyle};
use slime_ui::tree::{Element, LayoutDirection, RootSettings, Ui};
use slime_ui_render::{ATLAS_PAGE_SIZE, UiRenderer};

const FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 160;

/// Each writing system of slime-ui's tests: its text's file, its stack, its language.
const SCRIPTS: &[(&str, [&str; 2], &str)] = &[
    ("english", ["Noto Sans", "Noto Sans"], "en"),
    ("arabic", ["Noto Sans Arabic", "Noto Sans"], "ar"),
    ("hebrew", ["Noto Sans Hebrew", "Noto Sans"], "he"),
    ("hindi", ["Noto Sans Devanagari", "Noto Sans"], "hi"),
    ("thai", ["Noto Sans Thai", "Noto Sans"], "th"),
    ("japanese", ["Noto Sans CJK JP", "Noto Sans"], "ja"),
    ("chinese", ["Noto Sans CJK SC", "Noto Sans"], "zh-CN"),
    ("emoji", ["Noto Sans", "Noto Color Emoji"], "en"),
];

fn tests() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../slime-ui/tests")
}

/// The fonts of slime-ui's tests.
fn fonts() -> Fonts {
    let mut fonts = Fonts::new();
    for entry in std::fs::read_dir(tests().join("fonts")).expect("the fonts are there") {
        let path = entry.expect("a file").path();
        if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ttf" | "otf")
        ) {
            fonts
                .register(std::fs::read(&path).expect("readable"))
                .expect("a font");
        }
    }
    fonts
}

/// The first sentence of a writing system's text, laid out 600 logical pixels wide at `scale`.
fn sentence(
    fonts: &mut Fonts,
    (name, stack, locale): (&str, [&str; 2], &str),
    scale: f32,
) -> TextLayout {
    let text = std::fs::read_to_string(tests().join(format!("texts/{name}.txt"))).expect("a text");
    let style = TextStyle {
        fonts: stack.iter().map(|&family| family.to_owned()).collect(),
        size: 16.0,
        locale: Some(locale.to_owned()),
        ..TextStyle::default()
    };
    let sentence = text.lines().next().expect("a sentence");
    fonts.lay_out(sentence, &style, scale, TextBounds::width(600.0 / scale))
}

struct Setup {
    gpu: Gpu,
    renderer: Renderer,
    ui: UiRenderer,
}

impl Setup {
    fn new(budget: u64) -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
        Self {
            gpu,
            renderer,
            ui: UiRenderer::new(budget),
        }
    }

    fn target(&self) -> RenderTarget {
        let target = self
            .gpu
            .create_render_target("ui", WIDTH, HEIGHT, FORMAT)
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

    /// `list` drawn over black, read back, and what drawing it reported.
    fn draw(
        &mut self,
        fonts: &Fonts,
        list: &DisplayList,
    ) -> (FloatImage, slime_ui_render::UiStats) {
        let target = self.target();
        let stats = self
            .ui
            .draw(
                &self.gpu,
                &mut self.renderer,
                fonts,
                list,
                |_| None,
                &target,
            )
            .expect("drawn");
        let image = self
            .gpu
            .read_float_render_target(&target)
            .expect("read back");
        (image, stats)
    }
}

/// A list of one white text at `origin`.
fn text_list(layout: &Arc<TextLayout>, origin: [f32; 2]) -> DisplayList {
    let mut list = DisplayList::new();
    list.push(
        TextPrimitive {
            origin,
            layout: Arc::clone(layout),
            color: LinearColor::WHITE,
        },
        None,
    );
    list
}

/// The red of white glyphs laid over black, as the GPU lays them over a target of floats, each
/// glyph's image rasterized by the CPU and placed where the layout puts it, from `origin`.
fn reference(fonts: &Fonts, layout: &TextLayout, origin: [i32; 2]) -> Vec<f32> {
    let mut red = vec![0.0f32; (WIDTH * HEIGHT) as usize];
    let mut rasterizer = GlyphRasterizer::new();
    for run in layout.runs() {
        for glyph in run.placed() {
            let Some(image) = rasterizer.rasterize(fonts, glyph.key) else {
                continue;
            };
            let (left, top) = (
                origin[0] + glyph.x + image.left,
                origin[1] + glyph.y - image.top,
            );
            for row in 0..image.height as i32 {
                for column in 0..image.width as i32 {
                    let (x, y) = (left + column, top + row);
                    if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
                        continue;
                    }
                    let at = (row as u32 * image.width + column as u32) as usize;
                    let (value, alpha) = match &image.pixels {
                        GlyphPixels::Coverage(pixels) => {
                            let coverage = f32::from(pixels[at]) / 255.0;
                            (coverage, coverage)
                        }
                        GlyphPixels::Color(pixels) => {
                            let alpha = f32::from(pixels[at * 4 + 3]) / 255.0;
                            (slime_color::srgb8_to_linear(pixels[at * 4]) * alpha, alpha)
                        }
                    };
                    let pixel = &mut red[(y as u32 * WIDTH + x as u32) as usize];
                    *pixel = value + *pixel * (1.0 - alpha);
                }
            }
        }
    }
    red
}

/// The largest difference between the red of `image` and `expected`, and where it is.
fn worst(image: &FloatImage, expected: &[f32]) -> (f32, (u32, u32)) {
    let mut worst = (0.0, (0, 0));
    for (index, &value) in expected.iter().enumerate() {
        let drawn = image.pixels[index * 4];
        let difference = (drawn - value).abs();
        if difference > worst.0 {
            worst = (difference, (index as u32 % WIDTH, index as u32 / WIDTH));
        }
    }
    worst
}

#[test]
fn the_text_of_each_writing_system_is_drawn_where_its_layout_places_its_glyphs() {
    let mut fonts = fonts();
    let mut setup = Setup::new(slime_ui_render::GLYPH_ATLAS_BUDGET);
    for &script in SCRIPTS {
        for scale in [1.0, 2.0] {
            let layout = Arc::new(sentence(&mut fonts, script, scale));
            // An origin between two pixels is rounded to the nearest.
            let (image, stats) = setup.draw(&fonts, &text_list(&layout, [10.4, 12.6]));
            assert!(stats.glyphs > 0, "{}: no glyph drawn", script.0);
            assert_eq!(stats.left_out_glyphs, 0, "{}", script.0);
            let expected = reference(&fonts, &layout, [10, 13]);
            let (difference, at) = worst(&image, &expected);
            assert!(
                difference < 0.01,
                "{} at scale {scale}: {difference} at {at:?}",
                script.0
            );
        }
    }
}

#[test]
fn a_glyph_drawn_again_is_read_from_the_atlas_not_rasterized_again() {
    let mut fonts = fonts();
    let mut setup = Setup::new(slime_ui_render::GLYPH_ATLAS_BUDGET);
    let layout = Arc::new(sentence(&mut fonts, SCRIPTS[0], 1.0));
    let (_, first) = setup.draw(&fonts, &text_list(&layout, [0.0, 0.0]));
    let (image, again) = setup.draw(&fonts, &text_list(&layout, [0.0, 0.0]));
    assert!(first.rasterized > 0);
    assert_eq!(again.rasterized, 0);
    assert_eq!(again.glyphs, first.glyphs);
    assert_eq!(
        again.atlas_bytes,
        u64::from(ATLAS_PAGE_SIZE * ATLAS_PAGE_SIZE)
    );
    let (difference, at) = worst(&image, &reference(&fonts, &layout, [0, 0]));
    assert!(difference < 0.01, "{difference} at {at:?}");
}

#[test]
fn past_its_budget_the_atlas_lets_go_of_the_glyphs_that_no_draw_asked_for_the_longest() {
    let mut fonts = fonts();
    // One page of coverage, and the English text at a size whose glyphs would fill some 40 % of it,
    // then at a size larger by one pixel, whose glyphs are others of the same footprint: one fits,
    // the two do not.
    let page = u64::from(ATLAS_PAGE_SIZE * ATLAS_PAGE_SIZE);
    let mut setup = Setup::new(page);
    let text = std::fs::read_to_string(tests().join("texts/english.txt")).expect("a text");
    let sentence = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let lay_out = |fonts: &mut Fonts, size: f32| {
        let style = TextStyle {
            fonts: vec!["Noto Sans".to_owned()],
            size,
            ..TextStyle::default()
        };
        Arc::new(fonts.lay_out(&sentence, &style, 1.0, TextBounds::default()))
    };
    let footprint = |fonts: &Fonts, layout: &TextLayout| {
        let mut rasterizer = GlyphRasterizer::new();
        let keys: std::collections::HashSet<_> = layout
            .runs()
            .iter()
            .flat_map(|run| run.placed())
            .map(|glyph| glyph.key)
            .collect();
        keys.into_iter()
            .filter_map(|key| rasterizer.rasterize(fonts, key))
            .map(|image| u64::from((image.width + 2) * (image.height + 2)))
            .sum::<u64>()
    };
    let size = (8..200)
        .map(|size| size as f32 * 4.0)
        .find(|&size| {
            let layout = lay_out(&mut fonts, size);
            footprint(&fonts, &layout) > page * 4 / 10
        })
        .expect("a size fills 40 % of a page");
    let (first, second) = (lay_out(&mut fonts, size), lay_out(&mut fonts, size + 1.0));
    let (_, drawn) = setup.draw(&fonts, &text_list(&first, [0.0, 0.0]));
    assert_eq!(drawn.left_out_glyphs, 0);
    // The second's glyphs take the place of the first's, which no draw asks for any more.
    let (_, drawn) = setup.draw(&fonts, &text_list(&second, [0.0, 0.0]));
    assert_eq!(drawn.left_out_glyphs, 0);
    assert_eq!(drawn.atlas_bytes, page);
    let (_, again) = setup.draw(&fonts, &text_list(&first, [0.0, 0.0]));
    assert!(again.rasterized > 0, "the first's glyphs were let go");
    assert_eq!(again.left_out_glyphs, 0);

    // A draw that asks for both draws what fits, and says what it left out.
    let mut list = text_list(&first, [0.0, 0.0]);
    list.push(
        TextPrimitive {
            origin: [0.0, 0.0],
            layout: second,
            color: LinearColor::WHITE,
        },
        None,
    );
    let (_, crowded) = setup.draw(&fonts, &list);
    assert!(crowded.left_out_glyphs > 0);
    assert!(crowded.glyphs > 0);
}

#[test]
fn an_image_is_drawn_as_the_program_gives_it_and_one_it_does_not_give_is_left_out() {
    let fonts = fonts();
    let mut setup = Setup::new(slime_ui_render::GLYPH_ATLAS_BUDGET);
    let red = setup
        .gpu
        .create_texture(
            "red",
            1,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[255, 0, 0, 255],
        )
        .expect("the GPU has memory for it");
    let mut list = DisplayList::new();
    list.push(
        BoxPrimitive {
            rect: Rect::new(0.0, 0.0, 64.0, 64.0),
            radii: [0.0; 4],
            fill: LinearColor::rgb(0.0, 0.0, 1.0),
            border_width: 0.0,
            border: LinearColor::BLACK,
        },
        Some(Rect::new(0.0, 0.0, 32.0, 64.0)),
    );
    for (key, x) in [(7, 64.0), (8, 96.0)] {
        list.push(
            ImagePrimitive {
                rect: Rect::new(x, 0.0, 32.0, 32.0),
                image: ImageKey(key),
                part: Rect::new(0.0, 0.0, 1.0, 1.0),
                kind: ImageKind::Color,
                color: LinearColor::WHITE,
            },
            None,
        );
    }
    let target = setup.target();
    let stats = setup
        .ui
        .draw(
            &setup.gpu,
            &mut setup.renderer,
            &fonts,
            &list,
            |key| (key == ImageKey(7)).then_some(CanvasImage::Texture(&red)),
            &target,
        )
        .expect("drawn");
    assert_eq!(stats.missing_images, 1);
    let image = setup
        .gpu
        .read_float_render_target(&target)
        .expect("read back");
    let pixel = |x: u32, y: u32| {
        let at = ((y * WIDTH + x) * 4) as usize;
        [image.pixels[at], image.pixels[at + 1], image.pixels[at + 2]]
    };
    // The box, cut to its clip; the image given; nothing where the other was.
    assert_eq!(pixel(16, 16), [0.0, 0.0, 1.0]);
    assert_eq!(pixel(48, 16), [0.0; 3]);
    assert_eq!(pixel(80, 16), [1.0, 0.0, 0.0]);
    assert_eq!(pixel(112, 16), [0.0; 3]);
}

#[test]
fn a_lost_device_stops_the_drawing_and_a_new_renderer_draws_the_text_again() {
    let mut fonts = fonts();
    let mut setup = Setup::new(slime_ui_render::GLYPH_ATLAS_BUDGET);
    let layout = Arc::new(sentence(&mut fonts, SCRIPTS[1], 1.0));
    let list = text_list(&layout, [0.0, 0.0]);
    let target = setup.target();
    setup.gpu.lose();
    let error = setup
        .ui
        .draw(
            &setup.gpu,
            &mut setup.renderer,
            &fonts,
            &list,
            |_| None,
            &target,
        )
        .expect_err("the device is lost");
    assert!(
        matches!(error, RenderError::Gfx(GfxError::DeviceLost { .. })),
        "{error:?}"
    );
    let mut again = Setup::new(slime_ui_render::GLYPH_ATLAS_BUDGET);
    let (image, stats) = again.draw(&fonts, &list);
    assert!(stats.rasterized > 0);
    let (difference, at) = worst(&image, &reference(&fonts, &layout, [0, 0]));
    assert!(difference < 0.01, "{difference} at {at:?}");
}

#[test]
fn a_text_cut_by_its_clip_is_drawn_inside_it_alone() {
    let mut fonts = fonts();
    let mut setup = Setup::new(slime_ui_render::GLYPH_ATLAS_BUDGET);
    let layout = Arc::new(sentence(&mut fonts, SCRIPTS[0], 1.0));
    let mut list = DisplayList::new();
    list.push(
        TextPrimitive {
            origin: [0.0, 0.0],
            layout: Arc::clone(&layout),
            color: LinearColor::WHITE,
        },
        Some(Rect::new(0.0, 0.0, 200.0, HEIGHT as f32)),
    );
    let (image, _) = setup.draw(&fonts, &list);
    let mut expected = reference(&fonts, &layout, [0, 0]);
    for (index, value) in expected.iter_mut().enumerate() {
        if index as u32 % WIDTH >= 200 {
            *value = 0.0;
        }
    }
    let (difference, at) = worst(&image, &expected);
    assert!(difference < 0.01, "{difference} at {at:?}");
}

#[test]
fn each_image_is_asked_for_once_a_draw_and_a_glyph_larger_than_a_page_is_left_out() {
    let mut fonts = fonts();
    let mut setup = Setup::new(slime_ui_render::GLYPH_ATLAS_BUDGET);
    let red = setup
        .gpu
        .create_texture(
            "red",
            1,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[255, 0, 0, 255],
        )
        .expect("the GPU has memory for it");
    let mut list = DisplayList::new();
    for x in [0.0, 40.0, 80.0] {
        list.push(
            ImagePrimitive {
                rect: Rect::new(x, 0.0, 32.0, 32.0),
                image: ImageKey(7),
                part: Rect::new(0.0, 0.0, 1.0, 1.0),
                kind: ImageKind::Color,
                color: LinearColor::WHITE,
            },
            None,
        );
    }
    // A letter of 1 500 pixels, whose image no page holds.
    let style = TextStyle {
        fonts: vec!["Noto Sans".to_owned()],
        size: 1500.0,
        ..TextStyle::default()
    };
    let huge = Arc::new(fonts.lay_out("A", &style, 1.0, TextBounds::default()));
    list.push(
        TextPrimitive {
            origin: [0.0, 0.0],
            layout: huge,
            color: LinearColor::WHITE,
        },
        None,
    );
    let target = setup.target();
    for _ in 0..2 {
        let mut asked = 0;
        let stats = setup
            .ui
            .draw(
                &setup.gpu,
                &mut setup.renderer,
                &fonts,
                &list,
                |_| {
                    asked += 1;
                    Some(CanvasImage::Texture(&red))
                },
                &target,
            )
            .expect("drawn");
        assert_eq!(asked, 1);
        assert_eq!(stats.left_out_glyphs, 1);
        assert_eq!(stats.atlas_bytes, 0);
    }
}

#[test]
fn a_page_whose_glyphs_a_draw_does_not_draw_makes_room_for_a_page_of_another_kind() {
    let mut fonts = fonts();
    // Room for one page of colors, or for four of coverage.
    let mut setup = Setup::new(4 << 20);
    let english = Arc::new(sentence(&mut fonts, SCRIPTS[0], 1.0));
    let (_, drawn) = setup.draw(&fonts, &text_list(&english, [0.0, 0.0]));
    assert_eq!(drawn.atlas_bytes, 1 << 20);
    // Emoji alone: their page of colors takes the place of the English text's page of coverage,
    // which this draw does not draw.
    let style = TextStyle {
        fonts: vec!["Noto Color Emoji".to_owned()],
        size: 32.0,
        ..TextStyle::default()
    };
    let emoji = Arc::new(fonts.lay_out(
        "\u{2705}\u{1F7E2}\u{1F4BB}",
        &style,
        1.0,
        TextBounds::default(),
    ));
    let (image, drawn) = setup.draw(&fonts, &text_list(&emoji, [0.0, 0.0]));
    assert_eq!(drawn.left_out_glyphs, 0);
    assert_eq!(drawn.atlas_bytes, 4 << 20);
    let (difference, at) = worst(&image, &reference(&fonts, &emoji, [0, 0]));
    assert!(difference < 0.01, "{difference} at {at:?}");
}

#[test]
fn a_tree_of_elements_is_drawn_where_its_layout_places_it_and_mirrored_from_right_to_left() {
    let mut setup = Setup::new(slime_ui_render::GLYPH_ATLAS_BUDGET);
    let colors = [
        LinearColor::rgba(1.0, 0.0, 0.0, 1.0),
        LinearColor::rgba(0.0, 1.0, 0.0, 1.0),
        LinearColor::rgba(0.0, 0.0, 1.0, 1.0),
    ];
    for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
        let mut ui = Ui::new(fonts());
        let mut row = Style::default();
        row.layout.padding.start = 10.0;
        row.layout.gap = [0.0, 10.0];
        row.layout.align_items = Align::Start;
        let settings = RootSettings {
            size: [WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0],
            scale: 2.0,
            direction,
            origin: [0.0; 2],
            modal: false,
        };
        let root = ui.add_root(settings, row);
        let mut ids = Vec::new();
        ui.build(root, 0, |children| {
            for (index, (color, width)) in colors
                .into_iter()
                .zip([Some(40.0), Some(60.0), None])
                .enumerate()
            {
                let mut style = Style::default();
                style.layout.size[1] = Dimension::Pixels(30.0);
                match width {
                    Some(width) => style.layout.size[0] = Dimension::Pixels(width),
                    None => style.layout.grow = 1.0,
                }
                style.paint.background = color;
                ids.push(children.add(index, Element::container(style), |_| {}));
            }
        });
        let (list, _) = ui.frame();
        let (image, _) = setup.draw(ui.fonts(), &list);
        let pixel = |x: f32, y: f32| {
            let index = (y as usize * WIDTH as usize + x as usize) * 4;
            [0, 1, 2].map(|channel| image.pixels[index + channel])
        };
        let first = ui.rect(ids[0]).expect("laid out");
        let expected = match direction {
            LayoutDirection::LeftToRight => 20.0,
            LayoutDirection::RightToLeft => WIDTH as f32 - 20.0 - 80.0,
        };
        assert_eq!((first.x, first.width), (expected, 80.0), "{direction:?}");
        for (&id, color) in ids.iter().zip(colors) {
            let rect = ui.rect(id).expect("laid out");
            let inside = pixel(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
            assert_eq!(
                inside,
                [color.r, color.g, color.b],
                "{direction:?} {rect:?}"
            );
            assert_eq!(pixel(rect.x + 1.0, rect.y + rect.height + 1.0), [0.0; 3]);
        }
    }
}
