//! Draws a scene cooked by the asset pipeline with the renderer, lit in physical units by the
//! lights its nodes place (the asset's sun, the light of its glowing ring and a spot light), or by
//! the sample's sun for a scene without lights, seen from a camera that orbits it until the mouse
//! takes over: drag with the left button to turn around it, roll the wheel to come nearer or go
//! farther. A dev overlay shows what each frame costs, with the GPU time of each pass, a slider
//! that changes the exposure, as a photographer's exposure value, one that changes the bloom's
//! strength and, for the sample's asset, one that changes how strongly its ring glows, through its
//! material, found by its name; F1 hides and shows it. It loads the scene in the background, its
//! frames showing the sky's horizon until the scene is loaded. The sample of Stages 3a, 3b and 4a.
//! Escape, closing the window or Ctrl+C quits.
//!
//! It reads cooked data, which `slime-cook` makes first:
//!
//! ```sh
//! cargo run -p slime-cook -- cook examples/assets target/cooked/examples
//! cargo run --example scene                              # the scene of examples/assets/scene.glb
//! cargo run --example scene -- --frames 120              # closes after 120 frames of the scene
//! cargo run --example scene -- --assets <dir> --scene <id>   # another cooked scene, by its ID
//! ```
//!
//! Without `--assets`, it reads `assets` beside the program if there is one, as where CI ships it,
//! and `target/cooked/examples` otherwise.

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
use slime_assets::{AssetId, CookedAssets};
use slime_devui::{Overlay, egui};
use slime_gfx::{LinearColor, RenderPassDescriptor, RenderTarget};
use slime_platform::{
    ButtonState, Event, KeyCode, KeyEvent, MouseButton, ScrollDelta, WindowConfig,
};
use slime_render::glam::Vec3;
use slime_render::{Camera, Material, MaterialValues, RenderStats, Renderer, SceneLoad};

use sample_scene::{BLOOM, EXPOSURE, SKY, SampleScene};

const USAGE: &str = "usage: scene [--assets <directory>] [--scene <id>] [--frames <count>]";

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
/// The name of the sample's material that its ring glows with (`assets/make_scene.py`).
const RING: &str = "teal";
/// How strongly the ring glows, as the third slider covers it: from none to four times as strongly
/// as the asset makes it.
const GLOW_RANGE: RangeInclusive<f32> = 0.0..=4.0;

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
    /// The cooked data, and the scene to draw.
    assets: CookedAssets,
    scene_id: AssetId,
    /// The renderer, and the load of the scene that it runs in the background, until the scene is
    /// loaded.
    loading: Option<(Renderer, SceneLoad)>,
    scene: Option<SampleScene>,
    camera: OrbitCamera,
    overlay: Option<Overlay>,
    /// Whether the overlay is drawn; F1 toggles it.
    show_overlay: bool,
    /// The time between frames, smoothed, in milliseconds.
    frame_ms: f32,
    /// How many frames drew the scene.
    drawn: u64,
    /// The number of frames that draw the scene after which the example closes itself, if
    /// `--frames` gives one. A run that sets one fails if no frame drew the scene: the automated
    /// checks run this example for a number of frames in a compositor without a screen, to prove
    /// that the scene is drawn.
    frame_limit: Option<u64>,
    /// What the last frame cost.
    stats: RenderStats,
    /// The exposure the scene is drawn at, as an exposure value at ISO 100 (see [`ev100`]); the
    /// overlay's slider changes it.
    ev100: f32,
    /// The strength of the bloom (ADR-0040); the overlay's second slider changes it.
    bloom: f32,
    /// The material that the scene's ring glows with, and its values as loaded, if the scene has
    /// one.
    ring: Option<(Material, MaterialValues)>,
    /// How many times as strongly as loaded the ring glows; the overlay's third slider changes it.
    glow: f32,
}

impl App for SceneExample {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        else {
            return Err("the scene example needs a window".into());
        };
        // The scene loads in the background: the window shows at once, and `render` takes the
        // scene once it is loaded.
        let renderer = Renderer::new(gpu, format)?;
        let load = renderer.load_scene_in_background(gpu, &self.assets, self.scene_id);
        self.loading = Some((renderer, load));
        self.overlay = Some(Overlay::new(gpu, format, window.scale_factor()));
        if self.frame_limit == Some(0) {
            ctx.request_exit();
        }
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
        let Some(gpu) = ctx.gpu() else {
            return;
        };
        if let Some((_, load)) = &mut self.loading {
            match load.take() {
                // Until the scene is loaded, the frame shows a plain color: the sky's horizon,
                // exposed but not tone mapped.
                None => {
                    let horizon = SKY.horizon;
                    let pass = RenderPassDescriptor {
                        target,
                        clear: Some(LinearColor::rgb(
                            horizon.r * EXPOSURE,
                            horizon.g * EXPOSURE,
                            horizon.b * EXPOSURE,
                        )),
                        depth: None,
                        timer: None,
                        resolve: None,
                    };
                    gpu.render(&pass, |_| {});
                    return;
                }
                Some(loaded) => {
                    let (renderer, _) = self.loading.take().expect("the scene was loading");
                    match loaded.map_err(AppError::from).and_then(|loaded| {
                        self.ring = loaded
                            .material(RING)
                            .map(|ring| (ring.clone(), ring.values()));
                        SampleScene::new(gpu, renderer, loaded)
                    }) {
                        Ok(scene) => {
                            let (center, radius) =
                                (scene.bounds().center(), scene.bounds().radius());
                            self.camera = OrbitCamera::new(center, radius, radius * 2.2, 0.6, 0.45);
                            self.scene = Some(scene);
                        }
                        Err(error) => {
                            ctx.exit_with_error(error);
                            return;
                        }
                    }
                }
            }
        }
        let Some(scene) = &mut self.scene else {
            return;
        };
        // The ring's light, which stands for its glow in the asset, stays as loaded.
        if let Some((ring, loaded)) = &self.ring {
            let values = MaterialValues {
                emissive_strength: loaded.emissive_strength * self.glow,
                ..*loaded
            };
            if let Err(error) = ring.set_values(gpu, values) {
                ctx.exit_with_error(error);
                return;
            }
        }
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
            let has_ring = self.ring.is_some();
            let glow = &mut self.glow;
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
                            "sky computed on the CPU, once the scene was loaded: {:.1} ms",
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
                        // For a scene with the sample's ring: how many times as strongly as loaded
                        // it glows.
                        if has_ring {
                            let shown = *glow;
                            ui.add(
                                egui::Slider::new(glow, GLOW_RANGE)
                                    .show_value(false)
                                    .text(format!("ring's glow × {shown:.2}")),
                            );
                        }
                        ui.label("F1 hides this");
                    });
            });
        }
        if self.frame_limit.is_some_and(|limit| self.drawn >= limit) {
            ctx.request_exit();
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
        if self.frame_limit.is_some_and(|limit| limit > 0) && self.drawn == 0 {
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
    let directory = options.assets.unwrap_or_else(default_assets);
    let assets = match CookedAssets::open(&directory) {
        Ok(assets) => assets,
        Err(error) => {
            eprintln!(
                "{error}\nfor the sample: cargo run -p slime-cook -- cook examples/assets \
                 target/cooked/examples"
            );
            return ExitCode::FAILURE;
        }
    };

    let config = AppConfig {
        window: Some(WindowConfig {
            title: "SlimeEngine scene example".to_owned(),
            ..WindowConfig::default()
        }),
        // The example counts its frames itself: those that draw the scene.
        ..AppConfig::default()
    };
    let mut example = SceneExample {
        assets,
        scene_id: options.scene.unwrap_or(sample_scene::SCENE),
        loading: None,
        scene: None,
        camera: OrbitCamera::new(Vec3::ZERO, 1.0, 1.0, 0.0, 0.0),
        overlay: None,
        show_overlay: true,
        ev100: ev100(EXPOSURE),
        bloom: BLOOM,
        ring: None,
        glow: 1.0,
        frame_ms: 0.0,
        drawn: 0,
        frame_limit: options.frame_limit,
        stats: RenderStats::default(),
    };
    match slime_app::run(&mut example, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// The directory of cooked data read without `--assets`: `assets` beside the program, where CI
/// ships the Windows programs with the sample cooked, or else `target/cooked/examples` in the
/// workspace, where the command that the module documents cooks it.
fn default_assets() -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|program| Some(program.parent()?.join("assets")));
    match beside {
        Some(directory) if directory.join(slime_assets::MANIFEST_FILE).is_file() => directory,
        _ => PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/cooked/examples"
        )),
    }
}

/// The command-line options of the example.
#[derive(Default)]
struct Options {
    assets: Option<PathBuf>,
    scene: Option<AssetId>,
    frame_limit: Option<u64>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--assets") => {
                options.assets = Some(arguments.next().ok_or("--assets needs a directory")?.into());
            }
            Some("--scene") => {
                let value = arguments.next().ok_or("--scene needs an ID")?;
                let id = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("`{}` is not an asset ID", value.display()))?;
                options.scene = Some(id);
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
