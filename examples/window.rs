//! Opens a window, draws the samples' triangle in it with the GPU, and logs the events it receives
//! (window, keyboard and mouse). Escape, closing the window or Ctrl+C quits; cursor motion is
//! logged at the `debug` level. With `--headless`, the same program runs without a window and
//! without a GPU, as automated checks run it.
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

use slime_app::{App, AppConfig, AppError, Context};
use slime_gfx::{PresentMode, RenderPassDescriptor, RenderTarget};
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};

use triangle::{BACKGROUND, Triangle};

const USAGE: &str = "usage: window [--headless] [--frames <count>] [--present-mode mailbox|fifo]";

#[derive(Default)]
struct WindowExample {
    /// The triangle's GPU objects; `None` when running headless.
    triangle: Option<Triangle>,
    /// How many frames drew the triangle.
    drawn: u64,
}

impl App for WindowExample {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        if let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) {
            self.triangle = Some(Triangle::new(gpu, format)?);
        }
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        match event {
            Event::CursorMoved { .. } => tracing::debug!(?event, "event"),
            _ => tracing::info!(?event, "event"),
        }
        if let Event::Key(KeyEvent {
            physical: KeyCode::Escape,
            state: ButtonState::Pressed,
            ..
        }) = event
        {
            ctx.request_exit();
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        if let (Some(gpu), Some(triangle)) = (ctx.gpu(), &self.triangle) {
            let pass = RenderPassDescriptor {
                target,
                clear: Some(BACKGROUND),
                depth: None,
                timer: None,
            };
            gpu.render(&pass, |pass| triangle.draw(pass));
            self.drawn += 1;
        }
    }

    fn shutdown(&mut self, _ctx: &mut Context) {
        tracing::info!(drawn = self.drawn, "shutdown");
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
    let mut example = WindowExample::default();
    match slime_app::run(&mut example, &config) {
        // The automated checks run this example for a number of frames in a compositor without a
        // screen, to prove that the window is drawn into: a run that drew nothing has failed.
        Ok(()) if config.window.is_some() && options.frame_limit.is_some_and(|limit| limit > 0) => {
            if example.drawn == 0 {
                tracing::error!("no frame drew the triangle");
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
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
