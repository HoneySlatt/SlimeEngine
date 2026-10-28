//! Opens a window, draws the samples' triangle in it with the GPU, and logs the events it receives
//! (window, keyboard and mouse): a presentation, with no rules (`slime_app::Presentation`). Escape,
//! closing the window or Ctrl+C quits; cursor motion is logged at the `debug` level. While the right
//! button is held, the window holds the cursor, hidden, and the window's title sums the mouse's
//! raw motion, as an editor's camera flies with it (`Window::set_cursor_held`,
//! `docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`); a refusal is logged. With
//! `--headless`, the same program runs its frames without a window and without a GPU, its
//! presentation never called, as automated checks run it.
//!
//! ```sh
//! cargo run --example window
//! SLIME_LOG=debug cargo run --example window   # also logs cursor motion
//! cargo run --example window -- --frames 120    # closes itself after 120 frames
//! WAYLAND_DISPLAY= cargo run --example window   # X11, for example through Xwayland
//! cargo run --example window -- --headless --frames 600   # no window: runs anywhere
//! cargo run --example window -- --present-mode fifo        # every image shown, one per refresh
//! ```

#[path = "common/triangle.rs"]
mod triangle;

use std::ffi::OsStr;
use std::process::ExitCode;

use slime_app::{AppConfig, AppError, Context, Presentation};
use slime_gfx::{PresentMode, RenderPassDescriptor, RenderTarget};
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, MouseButton, WindowConfig};
use slime_world::World;

use triangle::{BACKGROUND, Triangle};

const USAGE: &str = "usage: window [--headless] [--frames <count>] [--present-mode mailbox|fifo]";

#[derive(Default)]
struct WindowExample {
    /// The triangle's GPU objects, once the presentation started.
    triangle: Option<Triangle>,
    /// How many frames drew the triangle.
    drawn: u64,
    /// Whether a run that drew nothing has failed. The automated checks run this example for a
    /// number of frames in a compositor without a screen, to prove that the window is drawn into.
    must_draw: bool,
    /// The mouse's raw motion summed while the cursor is held.
    motion: [f64; 2],
}

impl Presentation for WindowExample {
    fn start(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        if let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) {
            self.triangle = Some(Triangle::new(gpu, format)?);
        }
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) -> bool {
        match event {
            Event::CursorMoved { .. } | Event::RawMotion { .. } => tracing::debug!(?event, "event"),
            _ => tracing::info!(?event, "event"),
        }
        self.hold_the_cursor(ctx, event);
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
        if let Some(window) = ctx.window()
            && window.cursor_held()
        {
            window.set_title(&format!(
                "SlimeEngine window example: raw motion {:.0}, {:.0}",
                self.motion[0], self.motion[1]
            ));
        }
        if let (Some(gpu), Some(triangle)) = (ctx.gpu(), &self.triangle) {
            let pass = RenderPassDescriptor {
                target,
                clear: Some(BACKGROUND),
                depth: None,
                timer: None,
                resolve: None,
            };
            gpu.render(&pass, |pass| triangle.draw(pass));
            self.drawn += 1;
        }
    }

    fn stop(&mut self, ctx: &mut Context) {
        tracing::info!(drawn = self.drawn, "stop");
        if self.must_draw && self.drawn == 0 {
            ctx.exit_with_error("no frame drew the triangle");
        }
    }
}

impl WindowExample {
    /// Holds the cursor while the right button is held, sums the raw motion meanwhile, and lets it
    /// go as the button is let go; the window lets it go itself as it loses the focus.
    fn hold_the_cursor(&mut self, ctx: &mut Context, event: &Event) {
        let Some(window) = ctx.window() else {
            return;
        };
        match event {
            Event::MouseButton {
                button: MouseButton::Right,
                state: ButtonState::Pressed,
                ..
            } => match window.set_cursor_held(true) {
                Ok(()) => self.motion = [0.0; 2],
                Err(error) => tracing::warn!("{}", slime_app::error_chain(&error)),
            },
            Event::MouseButton {
                button: MouseButton::Right,
                state: ButtonState::Released,
                ..
            }
            | Event::Focused(false) => {
                let _ = window.set_cursor_held(false);
                window.set_title("SlimeEngine window example");
            }
            Event::RawMotion { delta } => {
                self.motion[0] += delta[0];
                self.motion[1] += delta[1];
                ctx.request_frame();
            }
            _ => {}
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

    let window = WindowConfig {
        title: "SlimeEngine window example".to_owned(),
        ..WindowConfig::default()
    };
    let config = AppConfig {
        window: (!options.headless).then_some(window),
        frame_limit: options.frame_limit,
        present_mode: options.present_mode,
        ..AppConfig::default()
    };
    let mut example = WindowExample {
        must_draw: config.window.is_some() && options.frame_limit.is_some_and(|limit| limit > 0),
        ..WindowExample::default()
    };
    // The example has no rules: it only draws.
    match slime_app::run(&mut (), &mut example, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// The command-line options of the example.
#[derive(Default)]
struct Options {
    headless: bool,
    frame_limit: Option<u64>,
    present_mode: PresentMode,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--headless") => options.headless = true,
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                let frames = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?;
                options.frame_limit = Some(frames);
            }
            Some("--present-mode") => {
                options.present_mode = match arguments.next().as_deref().and_then(OsStr::to_str) {
                    Some("mailbox") => PresentMode::Mailbox,
                    Some("fifo") => PresentMode::Fifo,
                    _ => return Err("--present-mode needs `mailbox` or `fifo`".to_owned()),
                };
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(options)
}
