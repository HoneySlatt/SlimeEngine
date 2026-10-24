//! An interface drawn by the engine over a window or into a PNG file, from a display list made by
//! hand (`docs/adr/0139-text-drawn-and-the-display-list.md`), which `slime-ui-render` draws through
//! the renderer's canvas (`docs/adr/0137-the-canvas.md`): panels with rounded corners, borders and
//! shadows; a list that scrolls, cut by its panel's clip, its rows named; buttons with their
//! labels; a sentence of each writing system of slime-ui's tests, Arabic and Hebrew from right to
//! left, Hindi, Thai, Japanese, Chinese and emoji, its glyphs kept in the bridge's atlas; images
//! of colors, and images of coverage drawn in a color; a translucent card over the rest; and, in a
//! panel, the shapes of the image tests, which the renderer draws into a target of its own each
//! frame, the panel showing that target as an image, as the editor's viewport will. Colors mix as
//! sRGB values, as design tools mix them. The fonts are those of slime-ui's tests, parts of Noto
//! fonts that hold the characters of its texts alone (`crates/slime-ui/tests/fonts/README.md`),
//! built into the program.
//!
//! ```sh
//! cargo run --example interface                            # in a window; Escape quits
//! cargo run --example interface -- --frames 120            # closes itself after 120 frames
//! cargo run --example interface -- --output interface.png  # one image of 1280 × 800, no window
//! cargo run --example interface -- --output interface@2.png --scale 2
//! ```

#[path = "common/png_file.rs"]
mod png_file;
#[path = "common/shapes.rs"]
mod shapes;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use slime_app::{AppConfig, AppError, Context, Presentation};
use slime_color::LinearColor;
use slime_gfx::{Gpu, RenderTarget, Texture, TextureFormat};
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};
use slime_render::{CanvasImage, RenderError};
use slime_ui::Rect;
use slime_ui::display::{
    BoxPrimitive, DisplayList, ImageKey, ImageKind, ImagePrimitive, ShadowPrimitive, TextPrimitive,
};
use slime_ui::text::{Alignment, Fonts, TEXT_CACHE_BUDGET, TextBounds, TextCache, TextStyle};
use slime_ui_render::UiRenderer;
use slime_world::World;

use shapes::Shapes;

const USAGE: &str = "usage: interface [--frames <count>] | --output <file.png> [--scale <factor>]";
/// The size of the image that `--output` writes, in logical pixels, and the moment it shows.
const IMAGE_SIZE: (u32, u32) = (1280, 800);
const IMAGE_TIME: f32 = 0.7;
/// The side of an icon of the atlas of colors, and of the image of coverage, in texels.
const ICON: u32 = 32;
const MASK: u32 = 64;
/// The keys by which the display list names its images.
const ICONS: ImageKey = ImageKey(1);
const TICK: ImageKey = ImageKey(2);
const VIEW: ImageKey = ImageKey(3);
/// The width of the panel of writing systems, in logical pixels.
const TEXTS_WIDTH: f32 = 360.0;

/// The fonts of slime-ui's tests, built into the program so that it runs anywhere.
const FONTS: &[&[u8]] = &[
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSans.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansArabic.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansHebrew.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansDevanagari.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansThai.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansCJKjp.otf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansCJKsc.otf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoColorEmoji.ttf"),
];

/// Each writing system: the first sentence of its test text, its stack and its language.
const SCRIPTS: &[(&str, [&str; 2], &str)] = &[
    (
        include_str!("../crates/slime-ui/tests/texts/english.txt"),
        ["Noto Sans", "Noto Sans"],
        "en",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/arabic.txt"),
        ["Noto Sans Arabic", "Noto Sans"],
        "ar",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/hebrew.txt"),
        ["Noto Sans Hebrew", "Noto Sans"],
        "he",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/hindi.txt"),
        ["Noto Sans Devanagari", "Noto Sans"],
        "hi",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/thai.txt"),
        ["Noto Sans Thai", "Noto Sans"],
        "th",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/japanese.txt"),
        ["Noto Sans CJK JP", "Noto Sans"],
        "ja",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/chinese.txt"),
        ["Noto Sans CJK SC", "Noto Sans"],
        "zh-CN",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/emoji.txt"),
        ["Noto Sans", "Noto Color Emoji"],
        "en",
    ),
];

/// What the interface draws with: the renderer, which the shapes hold, the target their view is
/// drawn into, the images, the fonts and their laid-out texts, and the bridge with its atlas.
struct Interface {
    shapes: Shapes,
    viewport: Option<RenderTarget>,
    /// Sixteen icons of 32 × 32 texels in colors, four by four.
    icons: Texture,
    /// A ring and a tick, how much of each texel they cover.
    tick: Texture,
    fonts: Fonts,
    texts: TextCache,
    ui: UiRenderer,
    /// The words that name the list's rows and the buttons, those of the English test text, which
    /// its fonts hold.
    words: Vec<String>,
}

impl Interface {
    fn new(
        gpu: &Gpu,
        format: TextureFormat,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let side = ICON * 4;
        let icons: Vec<u8> = (0..side * side)
            .flat_map(|texel| {
                let (x, y) = (texel % side, texel / side);
                let (column, row) = (x / ICON, y / ICON);
                let (u, v) = ((x % ICON) as f32 + 0.5, (y % ICON) as f32 + 0.5);
                // A disc of each icon's color, fading to its edge, on its color, transparent, so
                // that filtering at its edge darkens nothing.
                let distance = ((u - 16.0).powi(2) + (v - 16.0).powi(2)).sqrt();
                let alpha = (14.0 - distance).clamp(0.0, 1.0);
                [
                    (64 + column * 60) as u8,
                    (64 + row * 60) as u8,
                    (240 - column * 40) as u8,
                    (alpha * 255.0) as u8,
                ]
            })
            .collect();
        let icons =
            gpu.create_texture("icons", side, side, TextureFormat::Rgba8UnormSrgb, &icons)?;
        let tick: Vec<u8> = (0..MASK * MASK)
            .map(|texel| {
                let (x, y) = ((texel % MASK) as f32 + 0.5, (texel / MASK) as f32 + 0.5);
                let ring = ((x - 32.0).powi(2) + (y - 32.0).powi(2)).sqrt();
                let ring = 1.0 - ((ring - 26.0).abs() - 3.0).clamp(0.0, 1.0);
                // The tick: two strokes, from (18, 33) to (28, 43), then to (46, 22).
                let stroke = |(ax, ay): (f32, f32), (bx, by): (f32, f32)| {
                    let (dx, dy) = (bx - ax, by - ay);
                    let along =
                        (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                    let (px, py) = (ax + along * dx - x, ay + along * dy - y);
                    1.0 - ((px * px + py * py).sqrt() - 3.0).clamp(0.0, 1.0)
                };
                let tick =
                    stroke((18.0, 33.0), (28.0, 43.0)).max(stroke((28.0, 43.0), (46.0, 22.0)));
                (ring.max(tick) * 255.0) as u8
            })
            .collect();
        let tick =
            gpu.create_texture_with_levels("tick", MASK, MASK, TextureFormat::R8Unorm, &[&tick])?;
        let mut fonts = Fonts::new();
        for font in FONTS {
            fonts.register(font.to_vec())?;
        }
        let words = SCRIPTS[0]
            .0
            .split_whitespace()
            .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|word| word.len() > 3)
            .map(str::to_owned)
            .collect();
        Ok(Self {
            shapes: Shapes::new(gpu, format)?,
            viewport: None,
            icons,
            tick,
            fonts,
            texts: TextCache::new(TEXT_CACHE_BUDGET),
            ui: UiRenderer::default(),
            words,
        })
    }

    /// Draws the interface over `target`, `time` seconds after the start, at `scale` pixels per
    /// logical pixel: first the shapes into the viewport's target, then the display list.
    fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        time: f32,
        scale: f32,
    ) -> Result<(), RenderError> {
        let (width, height) = (target.width() as f32, target.height() as f32);
        // A rectangle in logical pixels, on the target.
        let at =
            |x: f32, y: f32, w: f32, h: f32| Rect::new(x * scale, y * scale, w * scale, h * scale);
        let (logical_width, logical_height) = (width / scale, height / scale);
        let list_panel = at(12.0, 52.0, 260.0, logical_height - 64.0);
        let texts_panel = at(
            logical_width - TEXTS_WIDTH - 12.0,
            52.0,
            TEXTS_WIDTH,
            logical_height - 64.0,
        );
        let view = at(
            284.0,
            52.0,
            (logical_width - 296.0 - TEXTS_WIDTH - 12.0).max(1.0),
            (logical_height - 204.0).max(1.0),
        );
        let shelf_top = logical_height - 140.0;

        // The view, drawn into its own target at the size of its panel, less a border.
        let inside = Rect::new(
            view.x + 6.0 * scale,
            view.y + 6.0 * scale,
            (view.width - 12.0 * scale).max(1.0),
            (view.height - 12.0 * scale).max(1.0),
        );
        let size = (inside.width as u32, inside.height as u32);
        if self
            .viewport
            .as_ref()
            .is_none_or(|viewport| (viewport.width(), viewport.height()) != size)
        {
            self.viewport =
                Some(gpu.create_render_target("viewport", size.0, size.1, target.format())?);
        }
        let viewport = self.viewport.as_ref().expect("made above");
        self.shapes.render(gpu, viewport, time)?;

        let Self {
            shapes,
            viewport,
            icons,
            tick,
            fonts,
            texts,
            ui,
            words,
        } = self;
        let mut list = DisplayList::new();
        let plain = |rect: Rect, radius: f32, fill: LinearColor| BoxPrimitive {
            rect,
            radii: [radius * scale; 4],
            fill,
            border_width: 0.0,
            border: LinearColor::BLACK,
        };
        let panel = |list: &mut DisplayList, rect: Rect| {
            list.push(
                ShadowPrimitive {
                    rect,
                    radii: [8.0 * scale; 4],
                    color: LinearColor::rgba(0.0, 0.0, 0.0, 0.45),
                    offset: [0.0, 4.0 * scale],
                    blur: 18.0 * scale,
                    spread: 0.0,
                },
                None,
            );
            list.push(
                BoxPrimitive {
                    border_width: scale,
                    border: srgb(0x3A3D43),
                    ..plain(rect, 8.0, srgb(0x26282C))
                },
                None,
            );
        };
        // A text laid out at the screen's scale, kept from frame to frame.
        let mut lay_out = |text: &str, style: &TextStyle, bounds: TextBounds| {
            texts.get(fonts, text, style, scale, bounds).clone()
        };
        // The style of the labels, in the English test text's font.
        let label = |size: f32| TextStyle {
            fonts: vec!["Noto Sans".to_owned()],
            size,
            ..TextStyle::default()
        };
        let text = |layout, origin: [f32; 2], color| TextPrimitive {
            origin,
            layout,
            color,
        };

        list.push(
            plain(Rect::new(0.0, 0.0, width, height), 0.0, srgb(0x1B1C1F)),
            None,
        );
        // The bar and its buttons, the second pressed.
        list.push(
            plain(at(0.0, 0.0, logical_width, 40.0), 0.0, srgb(0x2B2D31)),
            None,
        );
        for index in 0..5 {
            let pressed = index == 1;
            let button = at(12.0 + index as f32 * 96.0, 6.0, 88.0, 28.0);
            list.push(
                BoxPrimitive {
                    border_width: scale,
                    border: if pressed {
                        srgb(0x6C8CFF)
                    } else {
                        srgb(0x4E525A)
                    },
                    ..plain(
                        button,
                        6.0,
                        if pressed {
                            srgb(0x4C6EF5)
                        } else {
                            srgb(0x3A3D43)
                        },
                    )
                },
                None,
            );
            let name = lay_out(&words[index * 7], &label(13.0), TextBounds::line(80.0));
            let origin = [
                button.x + (button.width - name.width()) / 2.0,
                button.y + (button.height - name.height()) / 2.0,
            ];
            list.push(text(name, origin, srgb(0xE3E5E8)), None);
        }

        // The list: rows that scroll up, cut at the inside of their panel.
        panel(&mut list, list_panel);
        let clip = Some(Rect::new(
            list_panel.x + scale,
            list_panel.y + 8.0 * scale,
            list_panel.width - 2.0 * scale,
            list_panel.height - 16.0 * scale,
        ));
        let scroll = (time * 24.0) % (28.0 * 16.0);
        for row in 0..48 {
            let y = 60.0 + row as f32 * 28.0 - scroll;
            let selected = row == 5;
            let fill = if selected {
                srgb(0x3B5BDB)
            } else if row % 2 == 0 {
                srgb(0x2E3035)
            } else {
                srgb(0x292B30)
            };
            list.push(plain(at(20.0, y, 244.0, 24.0), 4.0, fill), clip);
            let icon = row % 16;
            list.push(
                ImagePrimitive {
                    rect: at(26.0, y + 4.0, 16.0, 16.0),
                    image: ICONS,
                    part: Rect::new((icon % 4) as f32 / 4.0, (icon / 4) as f32 / 4.0, 0.25, 0.25),
                    kind: ImageKind::Color,
                    color: LinearColor::WHITE,
                },
                clip,
            );
            let name = lay_out(
                &words[row % words.len()],
                &label(13.0),
                TextBounds::line(200.0),
            );
            let origin = [50.0 * scale, (y + 12.0) * scale - name.height() / 2.0];
            let color = if selected {
                srgb(0xFFFFFF)
            } else {
                srgb(0xC9CDD4)
            };
            list.push(text(name, origin, color), clip);
        }

        // A sentence of each writing system, laid out across the panel, from its start: from the
        // right for Arabic and Hebrew.
        panel(&mut list, texts_panel);
        let texts_clip = Some(texts_panel);
        let mut y = texts_panel.y + 12.0 * scale;
        for &(script, stack, locale) in SCRIPTS {
            let style = TextStyle {
                fonts: stack.iter().map(|&family| family.to_owned()).collect(),
                size: 14.0,
                alignment: Alignment::Start,
                locale: Some(locale.to_owned()),
                ..TextStyle::default()
            };
            let sentence = script.lines().next().unwrap_or_default();
            let layout = lay_out(sentence, &style, TextBounds::width(TEXTS_WIDTH - 24.0));
            let height = layout.height();
            list.push(
                text(layout, [texts_panel.x + 12.0 * scale, y], srgb(0xE3E5E8)),
                texts_clip,
            );
            y += height + 10.0 * scale;
        }

        // The view in its panel, and a translucent card over it.
        panel(&mut list, view);
        list.push(
            ImagePrimitive {
                rect: inside,
                image: VIEW,
                part: Rect::new(0.0, 0.0, 1.0, 1.0),
                kind: ImageKind::Color,
                color: LinearColor::WHITE,
            },
            None,
        );
        let card = Rect::new(
            inside.x + 16.0 * scale,
            inside.y + 16.0 * scale,
            220.0 * scale,
            96.0 * scale,
        );
        list.push(
            ShadowPrimitive {
                rect: card,
                radii: [10.0 * scale; 4],
                color: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
                offset: [0.0, 6.0 * scale],
                blur: 24.0 * scale,
                spread: 0.0,
            },
            None,
        );
        list.push(
            BoxPrimitive {
                border_width: scale,
                border: LinearColor {
                    a: 0.6,
                    ..srgb(0x5F6368)
                },
                ..plain(
                    card,
                    10.0,
                    LinearColor {
                        a: 0.8,
                        ..srgb(0x202226)
                    },
                )
            },
            None,
        );
        list.push(
            ImagePrimitive {
                rect: Rect::new(
                    card.x + 16.0 * scale,
                    card.y + 16.0 * scale,
                    64.0 * scale,
                    64.0 * scale,
                ),
                image: TICK,
                part: Rect::new(0.0, 0.0, 1.0, 1.0),
                kind: ImageKind::Coverage,
                color: srgb(0x51CF66),
            },
            None,
        );
        let caption = lay_out(&words[3], &label(16.0), TextBounds::line(120.0));
        let origin = [
            card.x + 92.0 * scale,
            card.y + card.height / 2.0 - caption.height() / 2.0,
        ];
        list.push(text(caption, origin, srgb(0xE3E5E8)), None);

        // The shelf: cards of icons and of coverage in colors.
        for index in 0..8 {
            let card = at(284.0 + index as f32 * 112.0, shelf_top, 100.0, 128.0);
            if card.x + card.width > texts_panel.x {
                break;
            }
            panel(&mut list, card);
            let icon = (index * 5) % 16;
            list.push(
                ImagePrimitive {
                    rect: Rect::new(
                        card.x + 18.0 * scale,
                        card.y + 12.0 * scale,
                        64.0 * scale,
                        64.0 * scale,
                    ),
                    image: ICONS,
                    part: Rect::new((icon % 4) as f32 / 4.0, (icon / 4) as f32 / 4.0, 0.25, 0.25),
                    kind: ImageKind::Color,
                    color: LinearColor::WHITE,
                },
                None,
            );
            let tint = [
                srgb(0xFF6B6B),
                srgb(0xFCC419),
                srgb(0x51CF66),
                srgb(0x339AF0),
            ][index % 4];
            list.push(
                ImagePrimitive {
                    rect: Rect::new(
                        card.x + 34.0 * scale,
                        card.y + 84.0 * scale,
                        32.0 * scale,
                        32.0 * scale,
                    ),
                    image: TICK,
                    part: Rect::new(0.0, 0.0, 1.0, 1.0),
                    kind: ImageKind::Coverage,
                    color: tint,
                },
                None,
            );
        }

        let viewport = viewport.as_ref().expect("made above");
        ui.draw(
            gpu,
            shapes.renderer(),
            fonts,
            &list,
            |key| match key {
                ICONS => Some(CanvasImage::Texture(icons)),
                TICK => Some(CanvasImage::Texture(tick)),
                VIEW => Some(CanvasImage::Target(viewport)),
                _ => None,
            },
            target,
        )?;
        Ok(())
    }
}

/// A color given as sRGB's 0xRRGGBB.
fn srgb(hex: u32) -> LinearColor {
    LinearColor::from_srgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Default)]
struct InterfaceExample {
    interface: Option<Interface>,
    /// How many frames drew the interface.
    drawn: u64,
    /// Whether a run that drew nothing fails, as the automated checks run it.
    must_draw: bool,
}

impl Presentation for InterfaceExample {
    fn start(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        if let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) {
            self.interface = Some(Interface::new(gpu, format)?);
        }
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) -> bool {
        if let Event::Key(KeyEvent {
            physical: KeyCode::Escape,
            state: ButtonState::Pressed,
            ..
        }) = event
        {
            ctx.request_exit();
        }
        false
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let scale = ctx
            .window()
            .map_or(1.0, |window| window.scale_factor() as f32);
        let time = ctx.time().real().elapsed().as_secs_f32();
        if let (Some(gpu), Some(interface)) = (ctx.gpu(), &mut self.interface) {
            match interface.draw(gpu, target, time, scale) {
                Ok(()) => self.drawn += 1,
                Err(error) => tracing::warn!("frame not drawn: {error}"),
            }
        }
    }

    fn stop(&mut self, ctx: &mut Context) {
        tracing::info!(drawn = self.drawn, "stop");
        if self.must_draw && self.drawn == 0 {
            ctx.exit_with_error("no frame drew the interface");
        }
    }
}

fn main() -> ExitCode {
    let options = match parse_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Some(output) = &options.output {
        slime_app::init_diagnostics();
        return match write_image(output, options.scale) {
            Ok(()) => {
                tracing::info!(path = %output.display(), "image written");
                ExitCode::SUCCESS
            }
            Err(error) => {
                tracing::error!("{}", slime_app::error_chain(error.as_ref()));
                ExitCode::FAILURE
            }
        };
    }
    let config = AppConfig {
        window: Some(WindowConfig {
            title: "SlimeEngine interface example".to_owned(),
            ..WindowConfig::default()
        }),
        frame_limit: options.frame_limit,
        ..AppConfig::default()
    };
    let mut example = InterfaceExample {
        must_draw: options.frame_limit.is_some_and(|limit| limit > 0),
        ..InterfaceExample::default()
    };
    match slime_app::run(&mut (), &mut example, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// Draws one frame of the interface, without a window, into an image written to `output`.
fn write_image(output: &Path, scale: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let gpu = Gpu::new()?;
    let (width, height) = (
        (IMAGE_SIZE.0 as f32 * scale).round() as u32,
        (IMAGE_SIZE.1 as f32 * scale).round() as u32,
    );
    let target =
        gpu.create_render_target("interface", width, height, TextureFormat::Rgba8UnormSrgb)?;
    let mut interface = Interface::new(&gpu, target.format())?;
    interface.draw(&gpu, &target, IMAGE_TIME, scale)?;
    let image = gpu.read_render_target(&target)?;
    png_file::write(output, &image)
        .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
    Ok(())
}

/// The command-line options of the example.
struct Options {
    frame_limit: Option<u64>,
    output: Option<PathBuf>,
    scale: f32,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        frame_limit: None,
        output: None,
        scale: 1.0,
    };
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                options.frame_limit = Some(
                    value
                        .to_str()
                        .and_then(|value| value.parse().ok())
                        .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?,
                );
            }
            Some("--output") => {
                options.output = Some(arguments.next().ok_or("--output needs a path")?.into());
            }
            Some("--scale") => {
                options.scale = arguments
                    .next()
                    .and_then(|value| value.to_str()?.parse::<f32>().ok())
                    .filter(|&scale| scale > 0.0 && scale <= 4.0)
                    .ok_or("--scale needs a factor above 0 and up to 4")?;
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    if options.output.is_some() && options.frame_limit.is_some() {
        return Err("--frames draws in a window, --output into a file: not both".to_owned());
    }
    if options.output.is_none() && options.scale != 1.0 {
        return Err("--scale goes with --output; a window has its own scale".to_owned());
    }
    Ok(options)
}
