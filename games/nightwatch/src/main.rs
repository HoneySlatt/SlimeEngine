//! *Nightwatch*, a small stealth game made with SlimeEngine: at night, in a courtyard closed by
//! walls, the player crosses from the entrance to the gate. The first game of the engine and the
//! sample that every stage grows (ADR-0052, ADR-0055); `README.md` beside this crate describes it.
//!
//! ```sh
//! cargo run -p nightwatch                     # play
//! cargo run -p nightwatch -- --frames 120     # closes itself after 120 frames
//! ```
//!
//! W, A, S and D move (Z, Q, S and D on a French keyboard), the left and right arrows turn the
//! camera, Escape quits.

mod camera;
mod controls;
mod courtyard;
mod meshes;
mod rules;
mod view;

use std::process::ExitCode;
use std::time::Duration;

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context};
use slime_gfx::RenderTarget;
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};

use camera::FollowCamera;
use controls::Controls;
use courtyard::Courtyard;
use rules::Game;
use view::View;

const USAGE: &str = "usage: nightwatch [--frames <count>]";

/// The game: its rules, the keys held, the camera, and what draws them once the window is open.
struct Nightwatch {
    game: Game,
    controls: Controls,
    camera: FollowCamera,
    view: Option<View>,
    /// How many frames were drawn.
    drawn: u64,
    /// Whether a run that drew nothing has failed: the automated checks run the game for a number
    /// of frames in a compositor without a screen, to prove that it draws.
    must_draw: bool,
}

impl App for Nightwatch {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) else {
            return Err("Nightwatch needs a window".into());
        };
        self.view = Some(View::new(gpu, format)?);
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        let Event::Key(key) = event else {
            return;
        };
        self.controls.handle(key);
        if let KeyEvent {
            physical: KeyCode::Escape,
            state: ButtonState::Pressed,
            ..
        } = key
        {
            ctx.request_exit();
        }
    }

    fn fixed_update(&mut self, _ctx: &mut Context, step: Duration) {
        let direction = self.camera.on_ground(self.controls.movement());
        self.game.step(direction, step.as_secs_f32());
    }

    fn update(&mut self, _ctx: &mut Context, frame: &FrameTime) {
        self.camera
            .turn(self.controls.turn(), frame.delta().as_secs_f32());
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let (Some(gpu), Some(view)) = (ctx.gpu(), &mut self.view) else {
            return;
        };
        let camera = self.camera.camera(self.game.player());
        match view.draw(gpu, target, &self.game, camera) {
            Ok(_) => self.drawn += 1,
            // The GPU's memory cannot hold the targets of the window's new size, or the device was
            // lost: the game stops with the error, which `run` logs with its causes.
            Err(error) => ctx.exit_with_error(error),
        }
    }

    fn shutdown(&mut self, ctx: &mut Context) {
        tracing::info!(drawn = self.drawn, "shutdown");
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
            title: "Nightwatch".to_owned(),
            ..WindowConfig::default()
        }),
        frame_limit,
        ..AppConfig::default()
    };
    let courtyard = Courtyard::nightwatch();
    // The camera starts looking from the entrance towards the gate.
    let camera = FollowCamera::looking_along((courtyard.exit - courtyard.entrance).normalize());
    let mut game = Nightwatch {
        game: Game::new(courtyard),
        controls: Controls::default(),
        camera,
        view: None,
        drawn: 0,
        must_draw: frame_limit.is_some_and(|limit| limit > 0),
    };
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
