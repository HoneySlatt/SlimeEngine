//! *Hunna Wood*, a role-playing game made with SlimeEngine, seen in the third person, in a
//! dark-fantasy world. The sample that every stage grows (ADR-0067); `README.md` beside this crate
//! describes what can be played.
//!
//! ```sh
//! cargo run -p hunnawood                     # play
//! cargo run -p hunnawood -- --frames 120     # closes itself after 120 frames
//! ```
//!
//! W, A, S and D move (Z, Q, S and D on a French keyboard), the left and right arrows turn the
//! camera, R starts again, F1 shows what each frame costs, Escape quits.

mod camera;
mod controls;
mod rules;
mod statistics;
mod view;
mod zone;

use std::process::ExitCode;
use std::time::Duration;

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context};
use slime_gfx::RenderTarget;
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};
use slime_render::RenderStats;

use camera::FollowCamera;
use controls::Controls;
use rules::Game;
use statistics::Statistics;
use view::View;
use zone::Zone;

const USAGE: &str = "usage: hunnawood [--frames <count>]";

/// The game: its rules, the keys held, the camera, and what draws them once the window is open.
struct HunnaWood {
    game: Game,
    controls: Controls,
    camera: FollowCamera,
    view: Option<View>,
    /// The dev overlay, shown with F1.
    statistics: Option<Statistics>,
    /// What the last frame cost.
    stats: RenderStats,
    /// How many frames were drawn.
    drawn: u64,
    /// Whether a run that drew nothing has failed: the automated checks run the game for a number
    /// of frames in a compositor without a screen, to prove that it draws.
    must_draw: bool,
}

impl HunnaWood {
    fn new(zone: Zone, must_draw: bool) -> Self {
        Self {
            camera: FollowCamera::looking_along(zone.facing),
            game: Game::new(zone),
            controls: Controls::default(),
            view: None,
            statistics: None,
            stats: RenderStats::default(),
            drawn: 0,
            must_draw,
        }
    }
}

impl App for HunnaWood {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        else {
            return Err("Hunna Wood needs a window".into());
        };
        self.view = Some(View::new(gpu, format)?);
        self.statistics = Some(Statistics::new(gpu, format, window.scale_factor()));
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        if let Some(statistics) = &mut self.statistics {
            statistics.handle(event);
        }
        let Event::Key(key) = event else {
            return;
        };
        self.controls.handle(key);
        if let KeyEvent {
            physical,
            state: ButtonState::Pressed,
            repeat: false,
            ..
        } = key
        {
            match physical {
                KeyCode::Escape => ctx.request_exit(),
                KeyCode::KeyR => {
                    self.game.restart();
                    self.camera = FollowCamera::looking_along(self.game.zone().facing);
                }
                KeyCode::F1 => {
                    if let Some(statistics) = &mut self.statistics {
                        statistics.toggle();
                    }
                }
                _ => {}
            }
        }
    }

    fn fixed_update(&mut self, _ctx: &mut Context, step: Duration) {
        let direction = self.camera.on_ground(self.controls.movement());
        self.game.step(direction, step.as_secs_f32());
    }

    fn update(&mut self, _ctx: &mut Context, frame: &FrameTime) {
        let seconds = frame.delta().as_secs_f32();
        self.camera.turn(self.controls.turn(), seconds);
        if let Some(statistics) = &mut self.statistics {
            statistics.frame(seconds);
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let (Some(gpu), Some(view)) = (ctx.gpu(), &mut self.view) else {
            return;
        };
        // The hero is drawn between its last two fixed steps, the share of a step this frame comes
        // after the last one.
        let alpha = ctx.fixed_step_alpha() as f32;
        let hero = self.game.hero_between(alpha);
        let camera = self
            .camera
            .camera(hero.position, &self.game.zone().colliders);
        match view.draw(gpu, target, &self.game, alpha, camera) {
            Ok(stats) => {
                self.stats = stats;
                self.drawn += 1;
                if let Some(statistics) = &mut self.statistics {
                    statistics.draw(gpu, target, &self.stats);
                }
            }
            // The GPU's memory cannot hold the targets of the window's new size, or the device was
            // lost: the game stops with the error, which `run` logs with its causes.
            Err(error) => ctx.exit_with_error(error),
        }
    }

    fn shutdown(&mut self, ctx: &mut Context) {
        tracing::info!(
            drawn = self.drawn,
            draw_calls = self.stats.draw_calls,
            shadow_draw_calls = self.stats.shadow_draw_calls,
            triangles = self.stats.triangles,
            cpu_time = ?self.stats.cpu_time,
            shadow_gpu_time = ?self.stats.shadow_gpu_time,
            scene_gpu_time = ?self.stats.scene_gpu_time,
            sky_gpu_time = ?self.stats.sky_gpu_time,
            resolve_gpu_time = ?self.stats.resolve_gpu_time,
            bloom_gpu_time = ?self.stats.bloom_gpu_time,
            tone_mapping_gpu_time = ?self.stats.tone_mapping_gpu_time,
            "shutdown"
        );
        if self.must_draw && self.drawn == 0 {
            ctx.exit_with_error("no frame drew the game");
        }
    }
}

fn main() -> ExitCode {
    let frame_limit = match parse_options() {
        Ok(frame_limit) => frame_limit,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let config = AppConfig {
        window: Some(WindowConfig {
            title: "Hunna Wood".to_owned(),
            ..WindowConfig::default()
        }),
        frame_limit,
        ..AppConfig::default()
    };
    let mut game = HunnaWood::new(
        Zone::test_ground(),
        frame_limit.is_some_and(|limit| limit > 0),
    );
    match slime_app::run(&mut game, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// The number of frames after which the game closes itself, if `--frames` gives one.
fn parse_options() -> Result<Option<u64>, String> {
    let mut frame_limit = None;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                let count = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?;
                frame_limit = Some(count);
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(frame_limit)
}
