//! Draws a scene cooked by the asset pipeline with the renderer, lit in physical units by the
//! lights its nodes place (the asset's sun, the light of its glowing ring and a spot light), or by
//! the sample's sun for a scene without lights, seen from a camera that orbits it until the mouse
//! takes over: drag with the left button to turn around it, roll the wheel to come nearer or go
//! farther. A dev overlay shows what each frame costs, with the GPU time of each pass, a slider
//! that changes the exposure, as a photographer's exposure value, one that changes the bloom's
//! strength and, for the sample's asset, one that changes how strongly its ring glows, through its
//! material, found by its name; F1 hides and shows it, and F6 gives its sliders the keyboard's
//! focus, then gives it back to the camera. It loads the scene in the background, its
//! frames showing the sky's horizon until the scene is loaded. The sample of Stages 3a, 3b, 4a and 4b.
//! Escape, closing the window or Ctrl+C quits.
//!
//! It reads cooked data, which `slime-cook` makes first:
//!
//! ```sh
//! cargo run -p slime-cook -- cook examples/assets target/cooked/examples
//! cargo run --example scene                              # the scene of examples/assets/scene.glb
//! cargo run --example scene -- --frames 120              # closes after 120 frames of the scene
//! cargo run --example scene -- --assets <dir> --scene <id>   # another cooked scene, by its ID
//! cargo run --example scene -- --watch                   # loads it and the shaders again as they change
//! ```
//!
//! Without `--assets`, it reads `assets` beside the program if there is one, as where CI ships it,
//! and `target/cooked/examples` otherwise.

#[path = "common/assets_directory.rs"]
mod assets_directory;
#[path = "common/orbit.rs"]
mod orbit;
#[path = "common/sample_scene.rs"]
mod sample_scene;

use std::f32::consts::TAU;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use slime_app::{AppConfig, AppError, Context, Presentation};
use slime_assets::{AssetId, CookedAssets, ManifestWatch};
use slime_color::LinearColor;
use slime_devui::{Corner, Overlay, PanelId, PanelSpec, render_figures};
use slime_gfx::{RenderPassDescriptor, RenderTarget};
use slime_platform::{
    ButtonState, Event, KeyCode, KeyEvent, MouseButton, ScrollDelta, WindowConfig,
};
use slime_render::glam::DVec3;
use slime_render::{
    Camera, Material, MaterialValues, RenderStats, Renderer, SceneLoad, ShaderSources, ShaderWatch,
};
use slime_ui::action::{Action, ActionKind, FieldValue, Intent};
use slime_ui::style::Style;
use slime_ui::tree::{Element, ElementId, Ui};
use slime_world::World;

use sample_scene::{BLOOM, EXPOSURE, SKY, SampleScene};

const USAGE: &str =
    "usage: scene [--assets <directory>] [--scene <id>] [--frames <count>] [--watch]";

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
    target: DVec3,
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
    fn new(target: DVec3, radius: f32, distance: f32, yaw: f32, pitch: f32) -> Self {
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
    /// The dev overlay, and its panel of statistics, which F1 shows and hides.
    overlay: Overlay,
    statistics: PanelId,
    /// The sliders of the exposure, the bloom and the ring's glow, as last built, and the
    /// revision of that build.
    sliders: [Option<ElementId>; 3],
    revision: u64,
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
    /// With `--watch`, what tells when the cooked data changes.
    watch: Option<ManifestWatch>,
    /// With `--watch`, what tells when the renderer's shader files change.
    shader_watch: Option<ShaderWatch>,
    /// The load of the scene again, after its cooked data changed.
    reload: Option<SceneLoad>,
}

impl SceneExample {
    /// Reads what the overlay's sliders and keys asked since the last frame, and builds its panel
    /// of statistics, once the scene is loaded. Escape quits while a slider holds the keyboard's
    /// focus too: the overlay gives it as the intent to cancel.
    fn build_overlay(&mut self, ctx: &mut Context) {
        for action in self.overlay.take_actions() {
            match action {
                Action {
                    element: Some(element),
                    kind:
                        ActionKind::Changed(FieldValue::Number(value))
                        | ActionKind::Committed(FieldValue::Number(value)),
                } => {
                    let value = value as f32;
                    match self
                        .sliders
                        .iter()
                        .position(|&slider| slider == Some(element))
                    {
                        Some(0) => self.ev100 = value,
                        Some(1) => self.bloom = value,
                        Some(2) => self.glow = value,
                        _ => {}
                    }
                }
                Action {
                    kind: ActionKind::Intent(Intent::Cancel),
                    ..
                } => ctx.request_exit(),
                _ => {}
            }
        }
        let (Some(gpu), Some(window), Some(scene)) = (ctx.gpu(), ctx.window(), &self.scene) else {
            return;
        };
        self.overlay
            .set_window(window.inner_size(), window.scale_factor());
        let Some(slot) = self.overlay.slot(self.statistics) else {
            return;
        };
        let size = window.inner_size();
        let mut lines = vec![
            gpu.adapter_name().to_owned(),
            format!("frame: {:.2} ms", self.frame_ms),
        ];
        lines.extend(render_figures(&self.stats));
        lines.push(format!(
            "sky computed on the CPU, once the scene was loaded: {:.1} ms",
            scene.environment_time().as_secs_f64() * 1000.0
        ));
        lines.push(format!("{} × {} pixels", size.width, size.height));
        // In exposure values, so that equal drags are equal photographic stops, from 18 at its
        // start down to 0; from no bloom to five times the sample's; for a scene with the
        // sample's ring, how many times as strongly as loaded it glows.
        let mut sliders = vec![
            (
                format!("EV100 {:.1}", self.ev100),
                "EV100",
                self.ev100,
                *EV100_RANGE.end(),
                *EV100_RANGE.start(),
                true,
            ),
            (
                format!("bloom {:.3}", self.bloom),
                "bloom",
                self.bloom,
                *BLOOM_RANGE.start(),
                *BLOOM_RANGE.end(),
                false,
            ),
        ];
        if self.ring.is_some() {
            sliders.push((
                format!("ring's glow × {:.2}", self.glow),
                "ring's glow",
                self.glow,
                *GLOW_RANGE.start(),
                *GLOW_RANGE.end(),
                false,
            ));
        }
        self.revision += 1;
        let mut built = [None; 3];
        self.overlay
            .ui_mut()
            .build(slot, self.revision, |children| {
                for (index, line) in lines.iter().enumerate() {
                    children.add(
                        index,
                        Element::label(line.as_str(), Style::default()),
                        |_| {},
                    );
                }
                for (index, (label, name, value, least, most, reversed)) in
                    sliders.into_iter().enumerate()
                {
                    let text = Element::label(label, Style::default());
                    children.add(format!("{name} label"), text, |_| {});
                    let mut slider = Element::slider(
                        f64::from(value),
                        f64::from(least),
                        f64::from(most),
                        0.0,
                        Style::default(),
                    )
                    .named(name);
                    if reversed {
                        slider = slider.reversed();
                    }
                    built[index] = Some(children.add(name, slider, |_| {}));
                }
                children.add(
                    "hides",
                    Element::label("F1 hides this", Style::default()),
                    |_| {},
                );
            });
        self.sliders = built;
    }
}

impl Presentation for SceneExample {
    fn start(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        let (Some(gpu), Some(format)) = (ctx.gpu(), ctx.surface_format()) else {
            return Err("the scene example needs a window".into());
        };
        // The scene loads in the background: the window shows at once, and the frames take the
        // scene once it is loaded.
        let renderer = Renderer::new(gpu, format)?;
        let load = renderer.load_scene_in_background(gpu, &self.assets, self.scene_id);
        self.loading = Some((renderer, load));
        if self.frame_limit == Some(0) {
            ctx.request_exit();
        }
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) -> bool {
        // The overlay's interface took F1, presses and the wheel over its panel and the keys that
        // its focus takes. The releases of the presses that it did not take and the motion still
        // reach the camera: a drag that starts on the scene ends wherever the button is released.
        self.camera.handle(event);
        if let Event::Key(KeyEvent {
            physical: KeyCode::Escape,
            state: ButtonState::Pressed,
            repeat: false,
            ..
        }) = event
        {
            ctx.request_exit();
        }
        // The example has no rules, which would read the input.
        false
    }

    fn ui(&mut self) -> Option<&mut Ui> {
        Some(self.overlay.ui_mut())
    }

    fn extract(&mut self, ctx: &mut Context, _world: &World) {
        let delta = ctx.time().real().delta().as_secs_f32();
        self.camera.advance(delta);
        self.frame_ms = self.frame_ms * 0.9 + delta * 1000.0 * 0.1;
        let Some(gpu) = ctx.gpu() else {
            return;
        };
        if let Some((_, load)) = &mut self.loading {
            // Until the scene is loaded, the frames show a plain color.
            let Some(loaded) = load.take() else {
                return;
            };
            let (renderer, _) = self.loading.take().expect("the scene was loading");
            match loaded.map_err(AppError::from).and_then(|loaded| {
                self.ring = loaded
                    .material(RING)
                    .map(|ring| (ring.clone(), ring.values()));
                SampleScene::new(gpu, renderer, loaded)
            }) {
                Ok(scene) => {
                    let (center, radius) = (scene.bounds().center(), scene.bounds().radius());
                    self.camera = OrbitCamera::new(center, radius, radius * 2.2, 0.6, 0.45);
                    self.scene = Some(scene);
                }
                Err(error) => {
                    ctx.exit_with_error(error);
                    return;
                }
            }
        }
        let Some(scene) = &mut self.scene else {
            return;
        };
        if let Some(shader_watch) = &mut self.shader_watch {
            match shader_watch.poll(Instant::now()) {
                Ok(Some(sources)) => {
                    let started = Instant::now();
                    match scene.set_shaders(gpu, &sources) {
                        Ok(()) => tracing::info!(
                            time = ?started.elapsed(),
                            "the shaders changed: the frames are drawn with them"
                        ),
                        Err(error) => tracing::warn!(
                            "the shaders cannot be used: {}; the frames are drawn as before",
                            slime_app::error_chain(&error)
                        ),
                    }
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(
                    "the shaders cannot be read from {}: {error}; the example draws with those it \
                     has",
                    ShaderSources::DIRECTORY
                ),
            }
        }
        // Once a reload has ended, so that none is dropped half-way, which waits for its tasks.
        if let Some(watch) = &mut self.watch
            && self.reload.is_none()
        {
            match watch.poll(Instant::now()) {
                Ok(Some(assets)) => {
                    tracing::info!("the cooked data changed: the scene loads again");
                    self.reload = Some(scene.reload(gpu, &assets, self.scene_id));
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(
                    "the cooked data cannot be read: {error}; the example goes on with what it \
                     loaded"
                ),
            }
        }
        if let Some(result) = self.reload.as_mut().and_then(SceneLoad::take) {
            self.reload = None;
            // The ring's material is made again with the scene.
            let taken = result.map_err(AppError::from).and_then(|loaded| {
                let ring = loaded
                    .material(RING)
                    .map(|ring| (ring.clone(), ring.values()));
                scene.replace(gpu, loaded)?;
                Ok(ring)
            });
            match taken {
                Ok(ring) => {
                    self.ring = ring;
                    tracing::info!("the scene is drawn again");
                }
                Err(error) => {
                    tracing::warn!("the scene cannot be loaded again: {error}; it stays as it was");
                }
            }
        }
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
        self.build_overlay(ctx);
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let Some(gpu) = ctx.gpu() else {
            return;
        };
        let Some(scene) = &mut self.scene else {
            // Until the scene is loaded, the frame shows a plain color: the sky's horizon, exposed
            // but not tone mapped.
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
        };
        match scene.render(
            gpu,
            target,
            self.camera.camera(),
            exposure(self.ev100),
            self.bloom,
        ) {
            Ok(stats) => self.stats = stats,
            // The GPU's memory cannot hold the targets of the window's new size: the example stops
            // with the error, which `run` logs with its causes. A game could instead draw at a
            // lower resolution. The device lost, the run stops with the loss, which
            // `exit_with_error` leaves to it.
            Err(error) => {
                ctx.exit_with_error(error);
                return;
            }
        }
        self.drawn += 1;
        // Over the scene, with its renderer.
        if let Err(error) = self.overlay.draw(gpu, scene.renderer(), target) {
            ctx.exit_with_error(error);
            return;
        }
        if self.frame_limit.is_some_and(|limit| self.drawn >= limit) {
            ctx.request_exit();
        }
    }

    fn stop(&mut self, ctx: &mut Context) {
        tracing::info!(
            drawn = self.drawn,
            draw_calls = self.stats.draw_calls,
            instances = self.stats.instances,
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
            "stop"
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
    let directory = options
        .assets
        .clone()
        .unwrap_or_else(assets_directory::find);
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
    let watch = options
        .watch
        .then(|| ManifestWatch::new(&assets, Instant::now()));
    let mut overlay = Overlay::new();
    let statistics = overlay.add_panel(PanelSpec {
        title: "Statistics",
        chord: "F1",
        command: "statistics",
        corner: Corner::TopStart,
        size: [420.0, 560.0],
    });
    overlay.open(statistics);
    let mut example = SceneExample {
        assets,
        scene_id: options.scene.unwrap_or(sample_scene::SCENE),
        loading: None,
        scene: None,
        camera: OrbitCamera::new(DVec3::ZERO, 1.0, 1.0, 0.0, 0.0),
        overlay,
        statistics,
        sliders: [None; 3],
        revision: 0,
        ev100: ev100(EXPOSURE),
        bloom: BLOOM,
        ring: None,
        glow: 1.0,
        watch,
        shader_watch: options
            .watch
            .then(|| ShaderWatch::new(ShaderSources::DIRECTORY, Instant::now())),
        reload: None,
        frame_ms: 0.0,
        drawn: 0,
        frame_limit: options.frame_limit,
        stats: RenderStats::default(),
    };
    // The example has no rules: it only shows a scene.
    match slime_app::run(&mut (), &mut example, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// The command-line options of the example.
#[derive(Default)]
struct Options {
    assets: Option<PathBuf>,
    scene: Option<AssetId>,
    frame_limit: Option<u64>,
    /// Whether the example loads the scene again when its cooked data changes, and the renderer's
    /// shaders when their files do.
    watch: bool,
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
            Some("--watch") => options.watch = true,
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
