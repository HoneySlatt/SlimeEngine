//! Draws a glTF scene with the renderer, lit by a directional light, seen from a camera that
//! orbits it until the mouse takes over: drag with the left button to turn around it, roll the
//! wheel to come nearer or go farther. The sample of Stage 3a. Escape, closing the window or
//! Ctrl+C quits.
//!
//! ```sh
//! cargo run --example scene                              # the scene of examples/assets/scene.glb
//! cargo run --example scene -- --scene other.glb         # a GLB file with embedded buffers and PNG images
//! cargo run --example scene -- --frames 120              # closes itself after 120 frames
//! ```

#[path = "common/error_chain.rs"]
mod error_chain;
#[path = "common/orbit.rs"]
mod orbit;
#[path = "common/sample_scene.rs"]
mod sample_scene;

use std::path::PathBuf;
use std::process::ExitCode;

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context};
use slime_gfx::RenderTarget;
use slime_platform::{
    ButtonState, Event, KeyCode, KeyEvent, MouseButton, ScrollDelta, WindowConfig,
};
use slime_render::glam::Vec3;
use slime_render::{Camera, RenderStats};

use error_chain::error_chain;
use sample_scene::SampleScene;

const USAGE: &str = "usage: scene [--scene <file.glb>] [--frames <count>]";

/// Radians per pixel dragged.
const DRAG_SPEED: f32 = 0.005;
/// Radians per second while the camera orbits on its own.
const ORBIT_SPEED: f32 = 0.3;
/// How much one line of the wheel brings the camera nearer, as a fraction of its distance.
const ZOOM_STEP: f32 = 0.1;
/// Pixels of scrolling that count as one line, for touchpads and precise wheels.
const PIXELS_PER_LINE: f64 = 60.0;
/// The steepest angles above and below the target, in radians.
const PITCH_LIMIT: f32 = 1.45;

/// A camera that orbits the scene on its own until the mouse takes over: dragging with the left
/// button turns it, the wheel brings it nearer or farther.
struct OrbitCamera {
    target: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
    /// Whether the camera still orbits on its own: until the first drag.
    orbiting: bool,
    /// Whether the left button is held.
    dragging: bool,
    /// Where the cursor was last seen, in physical pixels.
    cursor: Option<(f64, f64)>,
}

impl OrbitCamera {
    fn new(target: Vec3, distance: f32, yaw: f32, pitch: f32) -> Self {
        Self {
            target,
            distance,
            yaw,
            pitch,
            orbiting: true,
            dragging: false,
            cursor: None,
        }
    }

    fn handle(&mut self, event: &Event) {
        match event {
            Event::MouseButton {
                button: MouseButton::Left,
                state,
            } => {
                self.dragging = *state == ButtonState::Pressed;
                if self.dragging {
                    self.orbiting = false;
                }
            }
            Event::CursorMoved { x, y } => {
                if let (true, Some((last_x, last_y))) = (self.dragging, self.cursor) {
                    self.yaw -= (x - last_x) as f32 * DRAG_SPEED;
                    self.pitch = (self.pitch + (y - last_y) as f32 * DRAG_SPEED)
                        .clamp(-PITCH_LIMIT, PITCH_LIMIT);
                }
                self.cursor = Some((*x, *y));
            }
            Event::CursorLeft => self.cursor = None,
            Event::MouseWheel(delta) => {
                let lines = match delta {
                    ScrollDelta::Lines { y, .. } => *y,
                    ScrollDelta::Pixels { y, .. } => (*y / PIXELS_PER_LINE) as f32,
                };
                self.distance *= (1.0 - ZOOM_STEP).powf(lines);
            }
            _ => {}
        }
    }

    /// Turns the camera on its own by `seconds` of time, while the mouse has not taken over.
    fn advance(&mut self, seconds: f32) {
        if self.orbiting {
            self.yaw += ORBIT_SPEED * seconds;
        }
    }

    fn camera(&self) -> Camera {
        orbit::camera_at(self.target, self.distance, self.yaw, self.pitch)
    }
}

struct SceneExample {
    /// The GLB file to draw.
    bytes: Vec<u8>,
    scene: Option<SampleScene>,
    camera: OrbitCamera,
    /// How many frames drew the scene.
    drawn: u64,
    /// What the last frame cost.
    stats: RenderStats,
}

impl App for SceneExample {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) else {
            return Err("the scene example needs a window".into());
        };
        let scene = SampleScene::new(gpu, format, &self.bytes)?;
        let bounds = scene.bounds();
        self.camera = OrbitCamera::new(bounds.center(), bounds.radius() * 2.2, 0.6, 0.45);
        self.scene = Some(scene);
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        self.camera.handle(event);
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
        self.camera.advance(frame.delta().as_secs_f32());
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        if let (Some(gpu), Some(scene)) = (ctx.gpu(), &mut self.scene) {
            self.stats = scene.render(gpu, target, self.camera.camera());
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
    let options = match parse_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let bytes = match &options.scene {
        None => sample_scene::ASSET.to_vec(),
        Some(path) => match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("cannot read {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        },
    };

    let config = AppConfig {
        window: Some(WindowConfig {
            title: "SlimeEngine scene example".to_owned(),
            ..WindowConfig::default()
        }),
        frame_limit: options.frame_limit,
        ..AppConfig::default()
    };
    let mut example = SceneExample {
        bytes,
        scene: None,
        camera: OrbitCamera::new(Vec3::ZERO, 1.0, 0.0, 0.0),
        drawn: 0,
        stats: RenderStats::default(),
    };
    match slime_app::run(&mut example, &config) {
        // The automated checks run this example for a number of frames in a compositor without a
        // screen, to prove that the scene is drawn: a run that drew nothing has failed.
        Ok(()) if options.frame_limit.is_some_and(|limit| limit > 0) => {
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

/// The command-line options of the example.
#[derive(Default)]
struct Options {
    scene: Option<PathBuf>,
    frame_limit: Option<u64>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--scene") => {
                options.scene = Some(arguments.next().ok_or("--scene needs a path")?.into());
            }
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                let count = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?;
                options.frame_limit = Some(count);
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(options)
}
