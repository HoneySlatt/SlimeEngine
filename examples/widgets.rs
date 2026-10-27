//! The widgets of a panel, in the editor's look, over a window or into a PNG file
//! (`docs/adr/0148-widgets.md`): a bar of tabs, two panels side by side and the handle between
//! them, a list of entities and a box of properties, both with their scroll bars, sections that
//! fold, sliders and check boxes, and, in the console's tab, two panels one above the other. Their
//! text is in Geist, and in Geist Mono where it is fixed in width, the fonts that `slime-ui`'s
//! feature `editor-look` builds in with the look's style sheet and theme.
//!
//! Its look follows the system's dark or light preference where the platform tells it, on Windows,
//! and changes with keys (`docs/adr/0149-the-looks-foundations.md`): T switches its colors, dark,
//! light and high contrast, D its density, A its accent, M its transitions on or off; its changes
//! move over a short time, a section folding over a longer one.
//!
//! It is used with the pointer and the keyboard (`docs/adr/0142-input-and-focus.md`): Tab and the
//! arrows move the focus, Enter and Space press what holds it, check a check box, fold a section
//! and choose a tab; the arrows move a slider by its step and a handle, Page Up and Page Down by
//! ten steps, Home and End to their ends; the pointer drags thumbs and handles, and the wheel
//! scrolls. The bar at the bottom shows what the last change did. Escape, nothing focused, closes
//! the window.
//!
//! What opens over the panel (`docs/adr/0150-layers.md`): a bar of menus, which F10 or Alt alone
//! reaches, *File* and *View*, their submenus opening as the pointer rests on their items or the
//! arrows reach them; a menu of an entity's row at the pointer's second button or Shift+F10; a list
//! to choose the quality of shadows from; tooltips on the tabs and on that list, once the pointer
//! rests on them; a floating panel of statistics, which *View* shows, moved by its bar, sized by its
//! corner and folded by its mark. The menus' ticks and chevrons are Lucide's icons. Agents read and use its widgets through the development channel, as
//! `crates/slime-mcp/tests/widgets.rs` does, and screen readers read them
//! (`docs/adr/0146-screen-readers.md`).
//!
//! ```sh
//! cargo run --example widgets                            # in a window
//! cargo run --example widgets -- --frames 120            # closes itself after 120 frames
//! cargo run --example widgets -- --output widgets.png    # one image of 960 × 600, no window
//! cargo run --example widgets -- --output widgets@2.png --scale 2
//! cargo run --example widgets -- --right-to-left         # the layout mirrored, from the right
//! ```
//!
//! Its image tests, `examples/tests/widgets.rs`, draw the same interface and compare it with
//! reference images.

#[path = "common/png_file.rs"]
mod png_file;
#[path = "common/widgets_panel.rs"]
mod widgets_panel;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use slime_app::{AppConfig, AppError, Context, Presentation};
use slime_gfx::{Gpu, RenderTarget, TextureFormat};
use slime_platform::{ColorScheme, Event, WindowConfig};
use slime_render::{RenderError, Renderer};
use slime_ui::look::{Colors, Density};
use slime_ui::tree::{LayoutDirection, Ui};
use slime_ui_render::UiRenderer;
use slime_world::World;

use widgets_panel::WidgetsPanel;

const USAGE: &str = "usage: widgets [--right-to-left] [--frames <count> | --output <file.png> \
                     [--scale <factor>]]";
/// The size of the image that `--output` writes, in logical pixels.
const IMAGE_SIZE: (u32, u32) = (960, 600);

/// What the interface draws with: the renderer, whose canvas draws it, and the bridge, which turns
/// its display list into the canvas and keeps its glyphs.
struct Drawing {
    renderer: Renderer,
    bridge: UiRenderer,
}

impl Drawing {
    fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        Ok(Self {
            renderer: Renderer::new(gpu, format)?,
            bridge: UiRenderer::default(),
        })
    }

    /// Draws `panel` over `target`, at `scale` pixels per logical pixel.
    fn draw(
        &mut self,
        gpu: &Gpu,
        panel: &mut WidgetsPanel,
        target: &RenderTarget,
        scale: f32,
    ) -> Result<(), RenderError> {
        let size = [
            target.width() as f32 / scale,
            target.height() as f32 / scale,
        ];
        let display = panel.frame(size, scale);
        self.bridge.draw(
            gpu,
            &mut self.renderer,
            panel.ui.fonts(),
            &display,
            |_| None,
            target,
        )?;
        Ok(())
    }
}

#[derive(Default)]
struct WidgetsExample {
    panel: Option<(WidgetsPanel, Drawing)>,
    direction: LayoutDirection,
    /// How many frames drew the interface.
    drawn: u64,
    /// Whether a run that drew nothing fails, as the automated checks run it.
    must_draw: bool,
}

impl Presentation for WidgetsExample {
    fn start(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        if let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) {
            let drawing = Drawing::new(gpu, format)?;
            let mut panel = WidgetsPanel::new(self.direction);
            // The system's light colors, where the platform tells them.
            if ctx.window().and_then(|window| window.color_scheme()) == Some(ColorScheme::Light) {
                panel.set_look(Colors::Light, Density::Comfortable);
            }
            self.panel = Some((panel, drawing));
        }
        Ok(())
    }

    fn on_event(&mut self, _ctx: &mut Context, event: &Event) -> bool {
        if let (Event::ColorSchemeChanged(scheme), Some((panel, _))) = (event, &mut self.panel) {
            let colors = match scheme {
                ColorScheme::Dark => Colors::Dark,
                ColorScheme::Light => Colors::Light,
            };
            panel.set_look(colors, Density::Comfortable);
        }
        false
    }

    fn ui(&mut self) -> Option<&mut Ui> {
        self.panel.as_mut().map(|(panel, _)| &mut panel.ui)
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let Some((panel, drawing)) = &mut self.panel else {
            return;
        };
        if panel.handle_actions() {
            ctx.request_exit();
        }
        let scale = ctx
            .window()
            .map_or(1.0, |window| window.scale_factor() as f32);
        if let Some(gpu) = ctx.gpu() {
            match drawing.draw(gpu, panel, target, scale) {
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
        return match write_image(output, &options) {
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
            title: "SlimeEngine widgets example".to_owned(),
            ..WindowConfig::default()
        }),
        frame_limit: options.frame_limit,
        ..AppConfig::default()
    };
    let mut example = WidgetsExample {
        must_draw: options.frame_limit.is_some_and(|limit| limit > 0),
        direction: options.direction,
        ..WidgetsExample::default()
    };
    // Its world holds nothing: what the widgets change is the program's.
    match slime_app::run(&mut (), &mut example, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// Draws the interface, without a window, into an image written to `output`.
fn write_image(output: &Path, options: &Options) -> Result<(), Box<dyn Error + Send + Sync>> {
    let gpu = Gpu::new()?;
    let (width, height) = (
        (IMAGE_SIZE.0 as f32 * options.scale).round() as u32,
        (IMAGE_SIZE.1 as f32 * options.scale).round() as u32,
    );
    let target =
        gpu.create_render_target("widgets", width, height, TextureFormat::Rgba8UnormSrgb)?;
    let mut drawing = Drawing::new(&gpu, target.format())?;
    let mut panel = WidgetsPanel::new(options.direction);
    // The first frame lays the list out, the second builds the rows that it shows.
    drawing.draw(&gpu, &mut panel, &target, options.scale)?;
    drawing.draw(&gpu, &mut panel, &target, options.scale)?;
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
    direction: LayoutDirection,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        frame_limit: None,
        output: None,
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
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
            Some("--right-to-left") => options.direction = LayoutDirection::RightToLeft,
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
        return Err("--frames draws in a window, --output into a file".to_owned());
    }
    if options.output.is_none() && options.scale != 1.0 {
        return Err("--scale goes with --output; a window has its own scale".to_owned());
    }
    Ok(options)
}
