//! Opens a window, logs the events it receives (window, keyboard and mouse), and fills it with a
//! solid color. Escape or closing the window quits; cursor motion is logged at the `debug` level.
//!
//! A Wayland compositor shows a window only once the application has given it pixels. Until the
//! engine presents frames with the GPU (Stage 2), this example fills the window from the CPU with
//! `softbuffer`, a dependency of the examples only.
//!
//! ```sh
//! cargo run --example window
//! SLIME_LOG=debug cargo run --example window   # also logs cursor motion
//! cargo run --example window -- --frames 120    # closes itself after 120 frames
//! WAYLAND_DISPLAY= cargo run --example window   # X11, for example through Xwayland
//! ```

use std::num::NonZeroU32;
use std::process::ExitCode;

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, Context};
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, Window, WindowConfig};
use softbuffer::SoftBufferError;

/// The fill color, as `0x00RRGGBB`.
const COLOR: u32 = 0x0030_7a4a;

type Surface = softbuffer::Surface<Window, Window>;

#[derive(Default)]
struct WindowExample {
    surface: Option<Surface>,
    needs_redraw: bool,
}

impl WindowExample {
    fn redraw(&mut self) -> Result<(), SoftBufferError> {
        let Some(surface) = &mut self.surface else {
            return Ok(());
        };
        let size = surface.window().inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            // Nothing to draw into while the window has no area.
            return Ok(());
        };
        surface.resize(width, height)?;
        let mut buffer = surface.buffer_mut()?;
        buffer.fill(COLOR);
        buffer.present()
    }
}

impl App for WindowExample {
    fn init(&mut self, ctx: &mut Context) {
        let Some(window) = ctx.window() else {
            return;
        };
        match create_surface(window) {
            Ok(surface) => {
                self.surface = Some(surface);
                self.needs_redraw = true;
            }
            Err(error) => {
                tracing::error!(%error, "cannot create the drawing surface");
                ctx.request_exit();
            }
        }
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        match event {
            Event::CursorMoved { .. } => tracing::debug!(?event, "event"),
            _ => tracing::info!(?event, "event"),
        }
        match event {
            Event::Resized(_) | Event::ScaleFactorChanged { .. } => self.needs_redraw = true,
            Event::Key(KeyEvent {
                physical: KeyCode::Escape,
                state: ButtonState::Pressed,
                ..
            }) => ctx.request_exit(),
            _ => {}
        }
    }

    fn update(&mut self, ctx: &mut Context, _frame: &FrameTime) {
        if !self.needs_redraw {
            return;
        }
        self.needs_redraw = false;
        if let Err(error) = self.redraw() {
            tracing::error!(%error, "cannot draw the window");
            ctx.request_exit();
        }
    }

    fn shutdown(&mut self, _ctx: &mut Context) {
        // The surface holds clones of the window; release them while the window still exists.
        self.surface = None;
        tracing::info!("shutdown");
    }
}

fn create_surface(window: &Window) -> Result<Surface, SoftBufferError> {
    let context = softbuffer::Context::new(window.clone())?;
    softbuffer::Surface::new(&context, window.clone())
}

fn main() -> ExitCode {
    let frame_limit = match parse_frame_limit() {
        Ok(limit) => limit,
        Err(message) => {
            eprintln!("{message}\nusage: window [--frames <count>]");
            return ExitCode::from(2);
        }
    };

    let config = AppConfig {
        window: Some(WindowConfig {
            title: "SlimeEngine window example".to_owned(),
            ..WindowConfig::default()
        }),
        frame_limit,
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

fn parse_frame_limit() -> Result<Option<u64>, String> {
    let mut limit = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--frames" => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                let frames = value
                    .parse()
                    .map_err(|_| format!("invalid frame count `{value}`"))?;
                limit = Some(frames);
            }
            _ => return Err(format!("unknown argument `{argument}`")),
        }
    }
    Ok(limit)
}

/// The error's message followed by the messages of its sources: "error: cause: cause of cause".
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut chain = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        chain.push_str(": ");
        chain.push_str(&cause.to_string());
        source = cause.source();
    }
    chain
}
