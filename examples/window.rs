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
//! ```

#[path = "common/error_chain.rs"]
mod error_chain;
#[path = "common/triangle.rs"]
mod triangle;

use std::process::ExitCode;

use slime_app::{App, AppConfig, AppError, Context};
use slime_gfx::RenderTarget;
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};

use error_chain::error_chain;
use triangle::{BACKGROUND, Triangle};

#[derive(Default)]
struct WindowExample {
    /// The triangle's GPU objects; `None` when running headless.
    triangle: Option<Triangle>,
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
            gpu.render(target, BACKGROUND, |pass| triangle.draw(pass));
        }
    }

    fn shutdown(&mut self, _ctx: &mut Context) {
        tracing::info!("shutdown");
    }
}

fn main() -> ExitCode {
    let options = match parse_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\nusage: window [--headless] [--frames <count>]");
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
        ..AppConfig::default()
    };
    match slime_app::run(&mut WindowExample::default(), &config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("run failed: {}", error_chain(&error));
            ExitCode::FAILURE
        }
    }
}

/// The command-line options of the example.
#[derive(Default)]
struct Options {
    headless: bool,
    frame_limit: Option<u64>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--headless" => options.headless = true,
            "--frames" => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                let frames = value
                    .parse()
                    .map_err(|_| format!("invalid frame count `{value}`"))?;
                options.frame_limit = Some(frames);
            }
            _ => return Err(format!("unknown argument `{argument}`")),
        }
    }
    Ok(options)
}
