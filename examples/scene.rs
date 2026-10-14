//! Draws a glTF scene with the renderer, lit in physical units by the lights of the file (the
//! asset's sun, the light of its glowing ring and a spot light), or by the sample's sun for a file
//! without lights, seen from a camera that orbits it until the mouse takes over: drag with the left
//! button to turn around it, roll the wheel to come nearer or go farther. A dev overlay shows what
//! each frame costs, with the GPU time of each pass, a slider that changes the exposure, as a
//! photographer's exposure value, and one that changes the bloom's strength; F1 hides and shows
//! it. The sample of Stages 3a and 3b. Escape, closing the window or Ctrl+C quits.
//!
//! ```sh
//! cargo run --example scene                              # the scene of examples/assets/scene.glb
//! cargo run --example scene -- --scene other.glb         # a GLB file holding its buffer and PNG images in its binary chunk
//! cargo run --example scene -- --frames 120              # closes itself after 120 frames
//! ```

#[path = "common/orbit.rs"]
mod orbit;
#[path = "common/sample_scene.rs"]
mod sample_scene;

use std::f32::consts::TAU;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::process::ExitCode;

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context};
use slime_devui::{Overlay, egui};
use slime_gfx::RenderTarget;
use slime_platform::{
    ButtonState, Event, KeyCode, KeyEvent, MouseButton, ScrollDelta, WindowConfig,
};
use slime_render::glam::Vec3;
use slime_render::{Camera, RenderStats};

use sample_scene::{BLOOM, EXPOSURE, SampleScene};

const USAGE: &str = "usage: scene [--scene <file.glb>] [--frames <count>]";

/// Radians per pixel dragged.
const DRAG_SPEED: f32 = 0.005;
/// Radians per second while the camera orbits on its own.
const ORBIT_SPEED: f32 = 0.3;
/// How much one line of the wheel brings the camera nearer, as a fraction of its distance.
const ZOOM_STEP: f32 = 0.1;
/// The nearest and farthest the wheel brings the camera, in radii of the scene.
const ZOOM_RANGE: (f32, f32) = (0.1, 20.0);
/// Pixels of scrolling that count as one line, for touchpads and precise wheels.
const PIXELS_PER_LINE: f64 = 60.0;
/// The lowest and steepest angles of the camera above the target, in radians: from level with the
/// scene's center, so that the camera stays above the floor (seen from below, the sample's walls
/// and crates flicker where they touch it, `docs/PLAN.md`), to nearly straight down.
const PITCH_RANGE: RangeInclusive<f32> = 0.0..=1.45;
/// The exposure values the slider covers (see [`ev100`]), from beyond direct sunlight to a dim
/// room, so that files lit at other scales than the sample can be exposed too. A higher exposure
/// value is darker, so the range runs from high to low: dragging to the right brightens the image.
const EV100_RANGE: RangeInclusive<f32> = 18.0..=0.0;
/// The bloom strengths the second slider covers: none, to five times the sample's.
const BLOOM_RANGE: RangeInclusive<f32> = 0.0..=0.2;

/// The exposure value at ISO 100 (EV100) that gives `exposure`, in the convention that Frostbite
/// and Filament follow (Lagarde and de Rousiers, "Moving Frostbite to Physically Based Rendering",
/// 2014): the luminance 1.2 × 2^EV100 nits is exposed to 1, so exposure = 1 / (1.2 × 2^EV100). A
/// photographer exposes direct sunlight at EV100 15 ("sunny 16"), a lit room at about 7; one more
/// is one stop darker.
fn ev100(exposure: f32) -> f32 {
    (1.0 / (1.2 * exposure)).log2()
}

/// The exposure of the exposure value `ev100` (see [`ev100`]).
fn exposure(ev100: f32) -> f32 {
    1.0 / (1.2 * ev100.exp2())
}

/// A camera that orbits the scene on its own until the mouse takes over: dragging with the left
/// button turns it, the wheel brings it nearer or farther.
struct OrbitCamera {
    target: Vec3,
    /// How far from `target` the scene reaches.
    radius: f32,
    distance: f32,
    /// Kept within one turn: an `f32` that grew without end would round the steps of each frame
    /// ever more coarsely, until the orbit stopped, after about five days.
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
    fn new(target: Vec3, radius: f32, distance: f32, yaw: f32, pitch: f32) -> Self {
        Self {
            target,
            radius,
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
                    self.yaw = (self.yaw - (x - last_x) as f32 * DRAG_SPEED).rem_euclid(TAU);
                    self.pitch = (self.pitch + (y - last_y) as f32 * DRAG_SPEED)
                        .clamp(*PITCH_RANGE.start(), *PITCH_RANGE.end());
                }
                self.cursor = Some((*x, *y));
            }
            Event::CursorLeft => self.cursor = None,
            Event::MouseWheel(delta) => {
                let lines = match delta {
                    ScrollDelta::Lines { y, .. } => *y,
                    ScrollDelta::Pixels { y, .. } => (*y / PIXELS_PER_LINE) as f32,
                };
                // Kept in a range, from which the wheel can always bring the camera back; `max`
                // and `min` rather than `clamp`, which panics on the NaN of a degenerate scene.
                let (nearest, farthest) = ZOOM_RANGE;
                self.distance = (self.distance * (1.0 - ZOOM_STEP).powf(lines))
                    .max(nearest * self.radius)
                    .min(farthest * self.radius);
            }
            _ => {}
        }
    }

    /// Turns the camera on its own by `seconds` of time, while the mouse has not taken over.
    fn advance(&mut self, seconds: f32) {
        if self.orbiting {
            self.yaw = (self.yaw + ORBIT_SPEED * seconds).rem_euclid(TAU);
        }
    }

    fn camera(&self) -> Camera {
        orbit::camera_at(
            self.target,
            self.radius,
            self.distance,
            self.yaw,
            self.pitch,
        )
    }
}

struct SceneExample {
    /// The GLB file to draw.
    bytes: Vec<u8>,
    scene: Option<SampleScene>,
    camera: OrbitCamera,
    overlay: Option<Overlay>,
    /// Whether the overlay is drawn; F1 toggles it.
    show_overlay: bool,
    /// The time between frames, smoothed, in milliseconds.
    frame_ms: f32,
    /// How many frames drew the scene.
    drawn: u64,
    /// Whether a run that drew nothing has failed. The automated checks run this example for a
    /// number of frames in a compositor without a screen, to prove that the scene is drawn.
    must_draw: bool,
    /// What the last frame cost.
    stats: RenderStats,
    /// The exposure the scene is drawn at, as an exposure value at ISO 100 (see [`ev100`]); the
    /// overlay's slider changes it.
    ev100: f32,
    /// The strength of the bloom (ADR-0040); the overlay's second slider changes it.
    bloom: f32,
}

impl App for SceneExample {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        else {
            return Err("the scene example needs a window".into());
        };
        let scene = SampleScene::new(gpu, format, &self.bytes)?;
        let (center, radius) = (scene.bounds().center(), scene.bounds().radius());
        self.camera = OrbitCamera::new(center, radius, radius * 2.2, 0.6, 0.45);
        self.scene = Some(scene);
        self.overlay = Some(Overlay::new(gpu, format, window.scale_factor()));
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        let over_a_panel = match &mut self.overlay {
            Some(overlay) => {
                overlay.handle(event);
                self.show_overlay && overlay.wants_pointer()
            }
            None => false,
        };
        // Presses and the wheel over a panel are the overlay's. Releases and motion still reach
        // the camera: a drag that starts on the scene ends wherever the button is released.
        let for_the_overlay = over_a_panel
            && matches!(
                event,
                Event::MouseButton {
                    state: ButtonState::Pressed,
                    ..
                } | Event::MouseWheel(_)
            );
        if !for_the_overlay {
            self.camera.handle(event);
        }
        if let Event::Key(KeyEvent {
            physical,
            state: ButtonState::Pressed,
            repeat: false,
            ..
        }) = event
        {
            match physical {
                KeyCode::Escape => ctx.request_exit(),
                KeyCode::F1 => self.show_overlay = !self.show_overlay,
                _ => {}
            }
        }
    }

    fn update(&mut self, _ctx: &mut Context, frame: &FrameTime) {
        let delta = frame.delta().as_secs_f32();
        self.camera.advance(delta);
        self.frame_ms = self.frame_ms * 0.9 + delta * 1000.0 * 0.1;
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let (Some(gpu), Some(scene)) = (ctx.gpu(), &mut self.scene) else {
            return;
        };
        match scene.render(
            gpu,
            target,
            self.camera.camera(),
            exposure(self.ev100),
            self.bloom,
        ) {
            Ok(stats) => self.stats = stats,
            // The GPU's memory cannot hold the targets of the window's new size, or the device was
            // lost: the example stops with the error, which `run` logs with its causes. A game could
            // instead draw at a lower resolution.
            Err(error) => {
                ctx.exit_with_error(error);
                return;
            }
        }
        self.drawn += 1;
        if let Some(overlay) = &mut self.overlay {
            let show = self.show_overlay;
            let adapter = gpu.adapter_name();
            let stats = self.stats;
            let environment_time = scene.environment_time();
            let frame_ms = self.frame_ms;
            let size = (target.width(), target.height());
            // The overlay's slider changes it; the scene is drawn with it from the next frame.
            let chosen = &mut self.ev100;
            let bloom = &mut self.bloom;
            // Drawn with no panel while hidden, so that egui still takes the events handed to the
            // overlay as they come, not all at once when it is shown again.
            overlay.render(gpu, target, |ctx| {
                if !show {
                    return;
                }
                egui::Window::new("Statistics")
                    .default_pos(egui::pos2(12.0, 12.0))
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(adapter);
                        ui.label(format!("frame: {frame_ms:.2} ms"));
                        ui.label(format!(
                            "render on the CPU: {:.2} ms",
                            stats.cpu_time.as_secs_f64() * 1000.0
                        ));
                        for (pass, time) in [
                            ("shadow", stats.shadow_gpu_time),
                            ("spot shadows", stats.spot_shadow_gpu_time),
                            ("scene", stats.scene_gpu_time),
                            ("sky", stats.sky_gpu_time),
                            ("resolve", stats.resolve_gpu_time),
                            ("bloom", stats.bloom_gpu_time),
                            ("tone mapping", stats.tone_mapping_gpu_time),
                        ] {
                            ui.label(match time {
                                Some(time) => format!(
                                    "{pass} on the GPU: {:.3} ms",
                                    time.as_secs_f64() * 1000.0
                                ),
                                None => format!("{pass} on the GPU: not measured"),
                            });
                        }
                        ui.label(format!(
                            "{} draw calls, {} triangles; {} draw calls for shadows",
                            stats.draw_calls,
                            stats.triangles,
                            stats.shadow_draw_calls + stats.spot_shadow_draw_calls
                        ));
                        ui.label(format!(
                            "sky computed on the CPU at startup: {:.1} ms",
                            environment_time.as_secs_f64() * 1000.0
                        ));
                        ui.label(format!("{} × {} pixels", size.0, size.1));
                        // In exposure values, so that equal drags are equal photographic stops.
                        // Its value is shown as a label rather than in a field that takes the
                        // keyboard, so that Escape and F1 keep their meaning.
                        let shown = *chosen;
                        ui.add(
                            egui::Slider::new(chosen, EV100_RANGE)
                                .show_value(false)
                                .text(format!("EV100 {shown:.1}")),
                        );
                        // From no bloom to five times the sample's, labelled as the exposure is.
                        let shown = *bloom;
                        ui.add(
                            egui::Slider::new(bloom, BLOOM_RANGE)
                                .show_value(false)
                                .text(format!("bloom {shown:.3}")),
                        );
                        ui.label("F1 hides this");
                    });
            });
        }
    }

    fn shutdown(&mut self, ctx: &mut Context) {
        tracing::info!(
            drawn = self.drawn,
            draw_calls = self.stats.draw_calls,
            shadow_draw_calls = self.stats.shadow_draw_calls,
            spot_shadow_draw_calls = self.stats.spot_shadow_draw_calls,
            triangles = self.stats.triangles,
            cpu_time = ?self.stats.cpu_time,
            shadow_gpu_time = ?self.stats.shadow_gpu_time,
            spot_shadow_gpu_time = ?self.stats.spot_shadow_gpu_time,
            scene_gpu_time = ?self.stats.scene_gpu_time,
            sky_gpu_time = ?self.stats.sky_gpu_time,
            resolve_gpu_time = ?self.stats.resolve_gpu_time,
            bloom_gpu_time = ?self.stats.bloom_gpu_time,
            tone_mapping_gpu_time = ?self.stats.tone_mapping_gpu_time,
            "shutdown"
        );
        if self.must_draw && self.drawn == 0 {
            ctx.exit_with_error("no frame drew the scene");
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
        camera: OrbitCamera::new(Vec3::ZERO, 1.0, 1.0, 0.0, 0.0),
        overlay: None,
        show_overlay: true,
        ev100: ev100(EXPOSURE),
        bloom: BLOOM,
        frame_ms: 0.0,
        drawn: 0,
        must_draw: options.frame_limit.is_some_and(|limit| limit > 0),
        stats: RenderStats::default(),
    };
    match slime_app::run(&mut example, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
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
