//! Draws cubes with the renderer, seen from a camera that orbits them: the sample of Stage 3a,
//! which grows with the stage. Escape, closing the window or Ctrl+C quits.
//!
//! ```sh
//! cargo run --example scene
//! cargo run --example scene -- --frames 120   # closes itself after 120 frames
//! ```

#[path = "common/cubes.rs"]
mod cubes;
#[path = "common/error_chain.rs"]
mod error_chain;

use std::process::ExitCode;

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context};
use slime_gfx::RenderTarget;
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};
use slime_render::RenderStats;

use cubes::Cubes;
use error_chain::error_chain;

const USAGE: &str = "usage: scene [--frames <count>]";

#[derive(Default)]
struct SceneExample {
    cubes: Option<Cubes>,
    /// Seconds since the start, which places the camera.
    time: f32,
    /// How many frames drew the cubes.
    drawn: u64,
    /// What the last frame cost.
    stats: RenderStats,
}

impl App for SceneExample {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) else {
            return Err("the scene example needs a window".into());
        };
        self.cubes = Some(Cubes::new(gpu, format)?);
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        if let Event::Key(KeyEvent {
            physical: KeyCode::Escape,
            state: ButtonState::Pressed,
            ..
        }) = event
        {
            ctx.request_exit();
        }
    }

    fn update(&mut self, _ctx: &mut Context, frame: &FrameTime) {
        self.time = frame.elapsed().as_secs_f32();
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        if let (Some(gpu), Some(cubes)) = (ctx.gpu(), &mut self.cubes) {
            self.stats = cubes.render(gpu, target, self.time);
            self.drawn += 1;
        }
    }

    fn shutdown(&mut self, _ctx: &mut Context) {
        tracing::info!(
            drawn = self.drawn,
            draw_calls = self.stats.draw_calls,
            triangles = self.stats.triangles,
            "shutdown"
        );
    }
}

fn main() -> ExitCode {
    let frame_limit = match parse_frames() {
        Ok(frames) => frames,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    let config = AppConfig {
        window: Some(WindowConfig {
            title: "SlimeEngine scene example".to_owned(),
            ..WindowConfig::default()
        }),
        frame_limit,
        ..AppConfig::default()
    };
    let mut example = SceneExample::default();
    match slime_app::run(&mut example, &config) {
        // The automated checks run this example for a number of frames in a compositor without a
        // screen, to prove that the scene is drawn: a run that drew nothing has failed.
        Ok(()) if frame_limit.is_some_and(|limit| limit > 0) => {
            if example.drawn == 0 {
                tracing::error!("no frame drew the scene");
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("run failed: {}", error_chain(&error));
            ExitCode::FAILURE
        }
    }
}

fn parse_frames() -> Result<Option<u64>, String> {
    let mut frames = None;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                let count = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?;
                frames = Some(count);
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(frames)
}
