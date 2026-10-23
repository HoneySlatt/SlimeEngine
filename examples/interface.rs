//! The interface's canvas (`docs/adr/0137-the-canvas.md`), drawn by the renderer over a window or
//! into a PNG file: panels with rounded corners, borders and shadows; a list that scrolls, cut by
//! its panel's clip; buttons; images of colors, and images of coverage drawn in a color, as glyphs
//! will be; a translucent card over the rest; and, in a panel, the shapes of the image tests, which
//! the renderer draws into a target of its own each frame, the panel showing that target as an
//! image, as the editor's viewport will. Colors mix as sRGB values, as design tools mix them. Text
//! comes with Stage 6b's next milestone.
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
use slime_render::{
    Canvas, CanvasBox, CanvasImage, CanvasShadow, ImageKind, ImagePart, ImageQuad, PixelRect,
    RenderError,
};
use slime_world::World;

use shapes::Shapes;

const USAGE: &str = "usage: interface [--frames <count>] | --output <file.png> [--scale <factor>]";
/// The size of the image that `--output` writes, in logical pixels, and the moment it shows.
const IMAGE_SIZE: (u32, u32) = (1280, 800);
const IMAGE_TIME: f32 = 0.7;
/// The side of an icon of the atlas of colors, and of the image of coverage, in texels.
const ICON: u32 = 32;
const MASK: u32 = 64;

/// What the interface draws with: the renderer, which the shapes hold, the target their view is
/// drawn into, and the images.
struct Interface {
    shapes: Shapes,
    viewport: Option<RenderTarget>,
    /// Sixteen icons of 32 × 32 texels in colors, four by four.
    icons: Texture,
    /// A ring and a tick, how much of each texel they cover.
    mask: Texture,
}

impl Interface {
    fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let side = ICON * 4;
        let icons: Vec<u8> = (0..side * side)
            .flat_map(|texel| {
                let (x, y) = (texel % side, texel / side);
                let (column, row) = (x / ICON, y / ICON);
                let (u, v) = ((x % ICON) as f32 + 0.5, (y % ICON) as f32 + 0.5);
                // A disc of each icon's color, fading to its edge, on nothing.
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
        let mask: Vec<u8> = (0..MASK * MASK)
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
        let mask =
            gpu.create_texture_with_levels("mask", MASK, MASK, TextureFormat::R8Unorm, &[&mask])?;
        Ok(Self {
            shapes: Shapes::new(gpu, format)?,
            viewport: None,
            icons,
            mask,
        })
    }

    /// Draws the interface over `target`, `time` seconds after the start, at `scale` pixels per
    /// logical pixel: first the shapes into the viewport's target, then the canvas.
    fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        time: f32,
        scale: f32,
    ) -> Result<(), RenderError> {
        let (width, height) = (target.width() as f32, target.height() as f32);
        // A rectangle in logical pixels, on the target.
        let at = |x: f32, y: f32, w: f32, h: f32| {
            PixelRect::new(x * scale, y * scale, w * scale, h * scale)
        };
        let (logical_width, logical_height) = (width / scale, height / scale);
        let list = at(12.0, 52.0, 260.0, logical_height - 64.0);
        let view = at(
            284.0,
            52.0,
            (logical_width - 296.0).max(1.0),
            (logical_height - 204.0).max(1.0),
        );
        let shelf_top = logical_height - 140.0;

        // The view, drawn into its own target at the size of its panel, less a border.
        let inside = PixelRect::new(
            view.x + 6.0 * scale,
            view.y + 6.0 * scale,
            view.width - 12.0 * scale,
            view.height - 12.0 * scale,
        );
        let size = (inside.width.max(1.0) as u32, inside.height.max(1.0) as u32);
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

        let mut canvas = Canvas::new();
        let icons = canvas.add_image(CanvasImage::Texture(&self.icons));
        let mask = canvas.add_image(CanvasImage::Texture(&self.mask));
        let shown = canvas.add_image(CanvasImage::Target(viewport));
        let plain = |rect: PixelRect, radius: f32, fill: LinearColor| CanvasBox {
            rect,
            radii: [radius * scale; 4],
            fill,
            border_width: 0.0,
            border: LinearColor::BLACK,
            clip: None,
        };
        let panel = |canvas: &mut Canvas<'_>, rect: PixelRect| {
            canvas.draw_shadow(&CanvasShadow {
                rect,
                radii: [8.0 * scale; 4],
                color: LinearColor::rgba(0.0, 0.0, 0.0, 0.45),
                offset: [0.0, 4.0 * scale],
                blur: 18.0 * scale,
                spread: 0.0,
                clip: None,
            });
            canvas.draw_box(&CanvasBox {
                border_width: scale,
                border: srgb(0x3A3D43),
                ..plain(rect, 8.0, srgb(0x26282C))
            });
        };

        canvas.draw_box(&plain(
            PixelRect::new(0.0, 0.0, width, height),
            0.0,
            srgb(0x1B1C1F),
        ));
        // The bar and its buttons, the second pressed.
        canvas.draw_box(&plain(
            at(0.0, 0.0, logical_width, 40.0),
            0.0,
            srgb(0x2B2D31),
        ));
        for index in 0..5 {
            let pressed = index == 1;
            canvas.draw_box(&CanvasBox {
                border_width: scale,
                border: if pressed {
                    srgb(0x6C8CFF)
                } else {
                    srgb(0x4E525A)
                },
                ..plain(
                    at(12.0 + index as f32 * 96.0, 6.0, 88.0, 28.0),
                    6.0,
                    if pressed {
                        srgb(0x4C6EF5)
                    } else {
                        srgb(0x3A3D43)
                    },
                )
            });
        }

        // The list: rows that scroll up, cut at the inside of their panel.
        panel(&mut canvas, list);
        let clip = Some(PixelRect::new(
            list.x + scale,
            list.y + 8.0 * scale,
            list.width - 2.0 * scale,
            list.height - 16.0 * scale,
        ));
        let scroll = (time * 24.0) % (28.0 * 16.0);
        for row in 0..48 {
            let y = 60.0 + row as f32 * 28.0 - scroll;
            let selected = row == 5;
            canvas.draw_box(&CanvasBox {
                clip,
                ..plain(
                    at(20.0, y, 244.0, 24.0),
                    4.0,
                    if selected {
                        srgb(0x3B5BDB)
                    } else if row % 2 == 0 {
                        srgb(0x2E3035)
                    } else {
                        srgb(0x292B30)
                    },
                )
            });
            let icon = row % 16;
            let (u, v) = ((icon % 4) as f32 / 4.0, (icon / 4) as f32 / 4.0);
            canvas.draw_image(&ImageQuad {
                rect: at(26.0, y + 4.0, 16.0, 16.0),
                image: icons,
                part: ImagePart {
                    x: u,
                    y: v,
                    width: 0.25,
                    height: 0.25,
                },
                kind: ImageKind::Color,
                color: LinearColor::WHITE,
                clip,
            });
            // Where a row's name will be: a bar of its length.
            canvas.draw_box(&CanvasBox {
                clip,
                ..plain(
                    at(50.0, y + 9.0, 60.0 + (row * 37 % 120) as f32, 6.0),
                    3.0,
                    if selected {
                        srgb(0xDDE4FF)
                    } else {
                        srgb(0x8A8F98)
                    },
                )
            });
        }

        // The view in its panel, and a translucent card over it.
        panel(&mut canvas, view);
        canvas.draw_image(&ImageQuad {
            rect: inside,
            image: shown,
            part: ImagePart::WHOLE,
            kind: ImageKind::Color,
            color: LinearColor::WHITE,
            clip: None,
        });
        let card = PixelRect::new(
            inside.x + 16.0 * scale,
            inside.y + 16.0 * scale,
            220.0 * scale,
            96.0 * scale,
        );
        canvas.draw_shadow(&CanvasShadow {
            rect: card,
            radii: [10.0 * scale; 4],
            color: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
            offset: [0.0, 6.0 * scale],
            blur: 24.0 * scale,
            spread: 0.0,
            clip: None,
        });
        canvas.draw_box(&CanvasBox {
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
        });
        canvas.draw_image(&ImageQuad {
            rect: PixelRect::new(
                card.x + 16.0 * scale,
                card.y + 16.0 * scale,
                64.0 * scale,
                64.0 * scale,
            ),
            image: mask,
            part: ImagePart::WHOLE,
            kind: ImageKind::Coverage,
            color: srgb(0x51CF66),
            clip: None,
        });

        // The shelf: cards of icons and of coverage in colors.
        for index in 0..8 {
            let card = at(284.0 + index as f32 * 112.0, shelf_top, 100.0, 128.0);
            panel(&mut canvas, card);
            let icon = (index * 5) % 16;
            let (u, v) = ((icon % 4) as f32 / 4.0, (icon / 4) as f32 / 4.0);
            canvas.draw_image(&ImageQuad {
                rect: PixelRect::new(
                    card.x + 18.0 * scale,
                    card.y + 12.0 * scale,
                    64.0 * scale,
                    64.0 * scale,
                ),
                image: icons,
                part: ImagePart {
                    x: u,
                    y: v,
                    width: 0.25,
                    height: 0.25,
                },
                kind: ImageKind::Color,
                color: LinearColor::WHITE,
                clip: None,
            });
            let tint = [
                srgb(0xFF6B6B),
                srgb(0xFCC419),
                srgb(0x51CF66),
                srgb(0x339AF0),
            ][index % 4];
            canvas.draw_image(&ImageQuad {
                rect: PixelRect::new(
                    card.x + 34.0 * scale,
                    card.y + 84.0 * scale,
                    32.0 * scale,
                    32.0 * scale,
                ),
                image: mask,
                part: ImagePart::WHOLE,
                kind: ImageKind::Coverage,
                color: tint,
                clip: None,
            });
        }
        self.shapes.renderer().draw_canvas(gpu, &canvas, target)?;
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
fn write_image(output: &Path, scale: f32) -> Result<(), Box<dyn std::error::Error>> {
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
