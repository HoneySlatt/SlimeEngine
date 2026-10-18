//! Draws the examples' cooked scene from a world (`docs/adr/0084-what-the-world-draws.md`): spawned
//! in the world that `slime-app` keeps, its meshes loaded by an asset cache in the background, and
//! drawn through `slime-world-render`, its sun given once, to the sky and to the lights. The glowing
//! ring floats up and down at the fixed step, its light with it, and each frame draws it between
//! its last two places. The camera turns around the scene. After a lost GPU device, it goes on: the
//! cache loads the world's meshes again, and the world is untouched. F2 shows the dev overlay's world
//! panel, which lists the entities by name and edits their components (`slime-devui`). Escape,
//! closing the window or Ctrl+C quits.
//!
//! When it quits, it saves its world (`docs/adr/0087-the-worlds-documents.md`): what differs from
//! the world as it spawns it at the start, the ring's place and what the panel changed. It loads
//! the save into a world spawned anew, and the run fails if that world is not the one saved.
//! `SLIME_LOG=debug` shows the save.
//!
//! It reads cooked data, which `slime-cook` makes first:
//!
//! ```sh
//! cargo run -p slime-cook -- cook examples/assets target/cooked/examples
//! cargo run --example world
//! cargo run --example world -- --frames 120      # closes after 120 frames that drew the whole world
//! cargo run --example world -- --headless --frames 600   # the world without a window, 600 frames
//! cargo run --example world -- --watch           # draws again what changes in the cooked data
//! ```
//!
//! Without `--assets`, it reads `assets` beside the program if there is one, as where CI ships it,
//! and `target/cooked/examples` otherwise. Without a window, nothing is drawn: the world runs, the
//! ring floats, and the run says how far it went.

#[path = "common/sample_scene.rs"]
#[expect(
    dead_code,
    reason = "this example takes the sample's sky, exposure and bloom, not its `SampleScene`"
)]
mod sample_scene;

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context};
use slime_assets::{CookedAssets, ManifestWatch};
use slime_devui::{Overlay, WorldPanel};
use slime_gfx::RenderTarget;
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};
use slime_render::glam::DVec3;
use slime_render::{AssetCache, Camera, Renderer};
use slime_world::{Entity, Light, PersistentId, ResolvedScene, SceneLibrary, Transform, World};
use slime_world_render::FrameBuilder;

use sample_scene::{BLOOM, EXPOSURE, SCENE, SKY};

const USAGE: &str = "usage: world [--assets <directory>] [--frames <count>] [--headless] [--watch]";

/// The key the scene is spawned under: its entities' identities start with it.
const KEY: &str = "sample";
/// How high the ring floats above and below where the file puts it, in metres.
const FLOAT_HEIGHT: f64 = 0.3;
/// How long the ring takes to rise and fall back.
const FLOAT_PERIOD: f64 = 3.0;
/// Radians per second at which the camera turns around the scene.
const TURN_SPEED: f32 = 0.2;

struct WorldExample {
    /// The cooked data the scene is spawned from, and its cache loads from.
    assets: CookedAssets,
    /// The renderer, while there is a GPU.
    renderer: Option<Renderer>,
    /// The dev overlay, while there is a GPU, and its world panel, shown with F2.
    overlay: Option<Overlay>,
    panel: WorldPanel,
    panel_shown: bool,
    cache: AssetCache,
    builder: FrameBuilder,
    /// The ring, where the file puts it, if the scene has one.
    ring: Option<(Entity, f64)>,
    /// The time the fixed steps covered, in seconds.
    time: f64,
    /// The lowest and highest the ring floated.
    floated: (f64, f64),
    /// The point the camera looks at, and how far from it the scene reaches.
    framing: (DVec3, f32),
    /// How far the camera has turned around the scene, in radians.
    yaw: f32,
    /// How many frames drew the whole world: every mesh it names held by the cache.
    drawn: u64,
    /// With a window, the number of frames that draw the whole world after which the example
    /// closes itself, if `--frames` gives one; a run that sets one fails if no frame did.
    frame_limit: Option<u64>,
    /// With `--watch`, what tells when the cooked data changes, and the name of the scene's cooked
    /// file, which changes with the scene.
    watch: Option<(ManifestWatch, String)>,
}

impl WorldExample {
    /// With a window, makes the renderer and the overlay with the GPU device of `ctx`.
    fn open(&mut self, ctx: &Context) -> Result<(), AppError> {
        if let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        {
            self.renderer = Some(Renderer::new(gpu, format)?);
            self.overlay = Some(Overlay::new(gpu, format, window.scale_factor()));
        }
        Ok(())
    }

    /// Spawns the scene of `assets` in `world`, as [`start`] does, and finds its ring.
    fn spawn(&mut self, world: &mut World) -> Result<(), AppError> {
        start(world, &self.assets)?;
        self.ring = world
            .find(&PersistentId::new([KEY, "ring"]))
            .and_then(|ring| Some((ring, world.get::<Transform>(ring)?.translation.y)));
        world.update_transforms();
        self.framing = framing(world);
        Ok(())
    }

    /// With `--watch`, takes what changed in the cooked data, once nothing loads: the cache loads
    /// again the meshes, materials and textures that changed, keeping the rest, and the scene is
    /// spawned again if it changed.
    fn watch(&mut self, ctx: &mut Context) {
        let Some((watch, scene_file)) = &mut self.watch else {
            return;
        };
        if self.cache.loading() > 0 || self.cache.reloading() {
            return;
        }
        let assets = match watch.poll(Instant::now()) {
            Ok(Some(assets)) => assets,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(
                    "the cooked data cannot be read: {error}; the example goes on with what it \
                     loaded"
                );
                return;
            }
        };
        let file = assets
            .manifest()
            .get(SCENE)
            .map(|entry| entry.file.clone())
            .unwrap_or_default();
        let respawn = file != *scene_file;
        *scene_file = file;
        if let (Some(gpu), Some(renderer)) = (ctx.gpu(), &self.renderer) {
            self.cache.reload(renderer, gpu, assets.clone());
        }
        self.assets = assets;
        if respawn && let Err(error) = self.spawn(ctx.world_mut()) {
            tracing::error!("the scene cannot be spawned again: {error}");
        }
    }
}

/// Spawns the scene of `assets` in `world` under [`KEY`], in place of the one spawned before, its
/// first directional light showing in the sky: the world as the example makes it at the start of a
/// run, from which its save differs.
fn start(world: &mut World, assets: &CookedAssets) -> Result<(), AppError> {
    let scene = assets.scene(SCENE)?;
    if let Some(root) = world.find(&PersistentId::new([KEY])) {
        world.despawn(root);
    }
    world.spawn_scene(
        KEY,
        &ResolvedScene::from_cooked(SCENE, &scene, world.registry())?,
    )?;
    let mut sun = false;
    world.query_mut::<&mut Light>(|_, light| {
        if let Light::Directional { in_sky, .. } = light {
            *in_sky = !sun;
            sun = true;
        }
    });
    Ok(())
}

/// Saves `world`, which ran from the scene of `assets`, as what differs from the world as the
/// example makes it at the start ([`start`]), loads the save into another world made so, and checks
/// that it is the world saved, entity by entity, floats by their bits. Returns the save's size in
/// bytes; logs its text at the `debug` level.
fn round_trip(world: &World, assets: &CookedAssets) -> Result<usize, AppError> {
    let made = || -> Result<World, AppError> {
        let mut world = World::new();
        start(&mut world, assets)?;
        Ok(world)
    };
    let save = world.save(&made()?, &BTreeMap::new())?;
    tracing::debug!("the save:\n{save}");
    let mut loaded = made()?;
    // The example spawns no scene while it runs, which loading would spawn again from a library.
    let report = loaded.load(&save, &mut SceneLibrary::new())?;
    if !report.set_aside.is_empty() {
        return Err(format!("the save set aside {:?}", report.set_aside).into());
    }
    if let Some(difference) = world.first_difference(&loaded) {
        return Err(format!("the world loaded is not the world saved: {difference}").into());
    }
    Ok(save.len())
}

/// The point the camera looks at, and how far the scene reaches from it: the box around where the
/// world places its entities, with a metre of margin, since meshes reach beyond their origins.
fn framing(world: &World) -> (DVec3, f32) {
    let mut low = DVec3::splat(f64::INFINITY);
    let mut high = DVec3::splat(f64::NEG_INFINITY);
    for (entity, _) in world.query::<&Transform>() {
        if let Some(at) = world.global_transform(entity) {
            low = low.min(at.translation);
            high = high.max(at.translation);
        }
    }
    if low.x > high.x {
        return (DVec3::ZERO, 1.0);
    }
    (
        (low + high) / 2.0,
        ((high - low).length() / 2.0 + 1.0) as f32,
    )
}

impl App for WorldExample {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        self.spawn(ctx.world_mut())?;
        self.open(ctx)?;
        if self.frame_limit == Some(0) {
            ctx.request_exit();
        }
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) {
        // Escape leaves a text field of the overlay, or closes a menu of it, rather than quit.
        let typing = self.overlay.as_ref().is_some_and(Overlay::wants_keyboard);
        if let Some(overlay) = &mut self.overlay {
            overlay.handle(event);
        }
        if let Event::Key(KeyEvent {
            physical,
            state: ButtonState::Pressed,
            repeat: false,
            ..
        }) = event
        {
            match physical {
                KeyCode::F2 => self.panel_shown = !self.panel_shown,
                KeyCode::Escape if !typing => ctx.request_exit(),
                _ => {}
            }
        }
    }

    fn fixed_update(&mut self, ctx: &mut Context, step: Duration) {
        self.time += step.as_secs_f64();
        let Some((ring, base)) = self.ring else {
            return;
        };
        let height = base + FLOAT_HEIGHT * (TAU * self.time / FLOAT_PERIOD).sin();
        if let Some(transform) = ctx.world_mut().get_mut::<Transform>(ring) {
            transform.translation.y = height;
        }
        self.floated = (self.floated.0.min(height), self.floated.1.max(height));
    }

    fn update(&mut self, _ctx: &mut Context, frame: &FrameTime) {
        self.yaw += TURN_SPEED * frame.delta().as_secs_f32();
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        self.watch(ctx);
        let (Some(gpu), Some(renderer)) = (ctx.gpu(), self.renderer.as_mut()) else {
            return;
        };
        let (center, radius) = self.framing;
        let (sin, cos) = self.yaw.sin_cos();
        let distance = f64::from(radius) * 2.2;
        let camera = Camera {
            eye: center + DVec3::new(f64::from(sin), 0.45, f64::from(cos)) * distance,
            target: center,
            near: radius / 1000.0,
            ..Camera::default()
        };
        let frame = match self.builder.build(
            ctx.world(),
            &mut self.cache,
            renderer,
            gpu,
            ctx.fixed_step_alpha(),
        ) {
            Ok(frame) => frame,
            Err(error) => {
                ctx.exit_with_error(error);
                return;
            }
        };
        let whole = frame.loading == 0;
        let mut scene = frame.scene(camera);
        scene.exposure = EXPOSURE;
        scene.bloom = BLOOM;
        if let Err(error) = renderer.render(gpu, target, &scene) {
            ctx.exit_with_error(error);
            return;
        }
        let mut edits = Vec::new();
        if let Some(overlay) = &mut self.overlay {
            let (panel, shown, world) = (&mut self.panel, self.panel_shown, ctx.world());
            // Drawn with no panel while hidden, so that egui still takes the events handed to it.
            overlay.render(gpu, target, |egui| {
                if shown {
                    edits.extend(panel.show(egui, world));
                }
            });
        }
        // Applied once the frame is drawn: the next one shows them.
        for edit in edits {
            let world = ctx.world_mut();
            if let Err(error) =
                world.set_property(edit.entity, &edit.component, &edit.path, edit.value)
            {
                self.panel.refused(&error);
            }
        }
        if whole {
            self.drawn += 1;
        }
        if self.frame_limit.is_some_and(|limit| self.drawn >= limit) {
            ctx.request_exit();
        }
    }

    fn gpu_lost(&mut self, _ctx: &mut Context) {
        // Every GPU object goes; the world stays as it is.
        self.renderer = None;
        self.overlay = None;
        self.cache.clear();
        self.builder.release();
    }

    fn gpu_restored(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        self.open(ctx)
    }

    fn shutdown(&mut self, ctx: &mut Context) {
        tracing::info!(
            entities = ctx.world().len(),
            drawn = self.drawn,
            ring_lowest = self.floated.0,
            ring_highest = self.floated.1,
            "shutdown"
        );
        let windowed = ctx.window().is_some();
        if windowed && self.frame_limit.is_some_and(|limit| limit > 0) && self.drawn == 0 {
            ctx.exit_with_error("no frame drew the whole world");
        }
        if self.ring.is_some() && self.time > FLOAT_PERIOD && self.floated.0 >= self.floated.1 {
            ctx.exit_with_error("the ring did not float");
        }
        match round_trip(ctx.world(), &self.assets) {
            Ok(bytes) => tracing::info!(bytes, "the world, saved and loaded, is the world saved"),
            Err(error) => ctx.exit_with_error(error),
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
    let directory = options.assets.clone().unwrap_or_else(default_assets);
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
        window: (!options.headless).then(|| WindowConfig {
            title: "SlimeEngine world example".to_owned(),
            ..WindowConfig::default()
        }),
        // Without a window, every frame counts; with one, the example counts the frames that drew
        // the whole world itself.
        frame_limit: options.frame_limit.filter(|_| options.headless),
        restore_lost_device: true,
        ..AppConfig::default()
    };
    let watch = options.watch.then(|| {
        let file = assets
            .manifest()
            .get(SCENE)
            .map(|entry| entry.file.clone())
            .unwrap_or_default();
        (ManifestWatch::new(&assets, Instant::now()), file)
    });
    let mut example = WorldExample {
        cache: AssetCache::new(assets.clone()),
        assets,
        renderer: None,
        overlay: None,
        panel: WorldPanel::default(),
        panel_shown: false,
        builder: FrameBuilder::new(Some(SKY)),
        ring: None,
        time: 0.0,
        floated: (f64::INFINITY, f64::NEG_INFINITY),
        framing: (DVec3::ZERO, 1.0),
        yaw: 0.6,
        drawn: 0,
        frame_limit: options.frame_limit,
        watch,
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
    frame_limit: Option<u64>,
    headless: bool,
    /// Whether the example draws again what changes in the cooked data.
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
            Some("--headless") => options.headless = true,
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
