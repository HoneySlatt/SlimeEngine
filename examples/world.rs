//! Draws the examples' yard from a world (`docs/adr/0084-what-the-world-draws.md`): a scene file
//! that places the cooked sample scene and four lamps, a prefab of its own, one lamp red and one
//! without its light (`docs/adr/0095-cooked-scene-files.md`). It is spawned in the world that
//! `slime-app` keeps, through a library of the cooked scenes, its meshes loaded by an asset cache
//! in the background, and drawn through `slime-world-render`, its sun given once, to the sky and to
//! the lights. The glowing ring floats up and down at the fixed step, its light with it, and each
//! frame draws it between its last two places. The camera turns around the scene. After a lost GPU
//! device, it goes on: the cache loads the world's meshes again, and the world is untouched. F2
//! shows the dev overlay's world panel, which lists the entities by name and edits their
//! components (`slime-devui`). Escape, closing the window or Ctrl+C quits.
//!
//! When it quits, it saves its world (`docs/adr/0087-the-worlds-documents.md`): what differs from
//! the world as it spawns it at the start, the ring's place and what the panel changed. It loads
//! the save into a world spawned anew, and the run fails if that world is not the one saved. It
//! also writes the yard back as a scene file (`docs/adr/0092-a-world-written-back-as-a-scene.md`),
//! spawns that file, and the run fails if the world spawned is not the one written.
//! `SLIME_LOG=debug` shows the save and the file.
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
//! With `--watch`, a change of the cooked data, as `slime-cook watch` makes it, is drawn: the cache
//! loads again the meshes, materials and textures that changed, and a scene that changed, the lamp
//! that the yard places among them, carries the run into the world spawned anew from it
//! (`World::carry`): every lamp follows the lamp but where the yard overrides it.
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
#[path = "common/yard.rs"]
mod yard;

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
use slime_world::{Entity, Light, PersistentId, SceneLibrary, Transform, World};
use slime_world_render::FrameBuilder;

use sample_scene::{BLOOM, EXPOSURE, SKY};
use yard::YARD;

const USAGE: &str = "usage: world [--assets <directory>] [--frames <count>] [--headless] [--watch]";

/// The key the yard is spawned under: its entities' identities start with it.
const KEY: &str = "yard";
/// How high the ring floats above and below where the file puts it, in metres.
const FLOAT_HEIGHT: f64 = 0.3;
/// How long the ring takes to rise and fall back.
const FLOAT_PERIOD: f64 = 3.0;
/// Radians per second at which the camera turns around the scene.
const TURN_SPEED: f32 = 0.2;

struct WorldExample {
    /// The cooked data the yard in the world was spawned from: with `--watch`, the library goes
    /// back to it when the scenes that changed cannot be spawned, so that it holds the scenes of
    /// the world that runs.
    assets: CookedAssets,
    /// The scenes of the cooked data, the yard's and those it places.
    scenes: SceneLibrary,
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
    /// With `--watch`, what tells when the cooked data changes.
    watch: Option<ManifestWatch>,
}

impl WorldExample {
    /// With a window, makes the renderer and the overlay with the GPU device of `ctx`.
    fn open(&mut self, ctx: &Context) -> Result<(), AppError> {
        if let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        {
            self.renderer = Some(Renderer::new(gpu, format)?);
            self.overlay = Some(Overlay::new(gpu, format, window.scale_factor())?);
        }
        Ok(())
    }

    /// Spawns the yard in `world`, as [`start`] does, and finds its ring.
    fn spawn(&mut self, world: &mut World) -> Result<(), AppError> {
        start(world, &mut self.scenes)?;
        let centre = ring(world).map(|(_, height)| height);
        self.found(world, centre);
        Ok(())
    }

    /// Finds the ring of the yard in `world`, which floats around `centre`, where the world as the
    /// example starts it puts the ring, and frames the world.
    fn found(&mut self, world: &mut World, centre: Option<f64>) {
        self.ring = ring(world)
            .zip(centre)
            .map(|((ring, _), centre)| (ring, centre));
        world.update_transforms();
        self.framing = framing(world);
    }

    /// With `--watch`, takes what changed in the cooked data, once nothing loads: the cache loads
    /// again the meshes, materials and textures that changed, keeping the rest, and the library the
    /// scenes that changed; then, if one did, the run is carried into the yard spawned anew from
    /// them, as it was saved against the yard spawned before and loaded into the new one. If they
    /// cannot be spawned, the library goes back to the scenes of the world that runs, which goes on.
    fn watch(&mut self, ctx: &mut Context) {
        let Some(watch) = &mut self.watch else {
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
        if let (Some(gpu), Some(renderer)) = (ctx.gpu(), &self.renderer) {
            self.cache.reload(renderer, gpu, assets.clone());
        }
        // The world as the example started it from the scenes before they changed, then after.
        let before = made(&mut self.scenes);
        let changed = self.scenes.reload(assets.clone());
        if changed.is_empty() {
            self.assets = assets;
            return;
        }
        let carried = before.and_then(|before| {
            let after = made(&mut self.scenes)?;
            let centre = ring(&after).map(|(_, height)| height);
            let carried = ctx.world().carry(&before, after, &mut self.scenes)?;
            Ok((carried, centre))
        });
        match carried {
            Ok(((world, loaded), centre)) => {
                self.assets = assets;
                if !loaded.set_aside.is_empty() {
                    tracing::warn!(
                        set_aside = ?loaded.set_aside,
                        "the run set aside what the scenes no longer have"
                    );
                }
                *ctx.world_mut() = world;
                self.found(ctx.world_mut(), centre);
                // Its handles named the entities of the world before.
                self.panel = WorldPanel::default();
                tracing::info!(
                    scenes = changed.len(),
                    "the scenes that changed carried the run"
                );
            }
            Err(error) => {
                self.scenes.reload(self.assets.clone());
                tracing::error!(
                    "the scenes that changed cannot be spawned: {error}; the example goes on with \
                     the world it has"
                );
            }
        }
    }
}

/// Spawns the yard of `scenes` in `world` under [`KEY`], its first directional light, the sample's
/// sun, showing in the sky: the world as the example makes it at the start of a run, from which its
/// save differs.
fn start(world: &mut World, scenes: &mut SceneLibrary) -> Result<(), AppError> {
    let yard = scenes.resolve(world.registry(), YARD)?;
    world.spawn_scene(KEY, &yard)?;
    let mut sun = false;
    world.query_mut::<&mut Light>(|_, mut light| {
        // Read first: only the lights written are stamped as changed.
        if matches!(*light, Light::Directional { .. })
            && let Light::Directional { in_sky, .. } = &mut *light
        {
            *in_sky = !sun;
            sun = true;
        }
    });
    Ok(())
}

/// The ring of the yard in `world`, and its height.
fn ring(world: &World) -> Option<(Entity, f64)> {
    let ring = world.find(&PersistentId::new([KEY, "sample", "ring"]))?;
    Some((ring, world.get::<Transform>(ring)?.translation.y))
}

/// A world made as the example starts it ([`start`]).
fn made(scenes: &mut SceneLibrary) -> Result<World, AppError> {
    let mut world = World::new();
    start(&mut world, scenes)?;
    Ok(world)
}

/// Saves `world`, which ran from the yard of `scenes`, as what differs from the world as the
/// example makes it at the start ([`start`]), loads the save into another world made so, and checks
/// that it is the world saved, entity by entity, floats by their bits; then writes the yard back as
/// a scene file, spawns that file, and checks that the world spawned is the world written. Returns
/// the sizes of the save and of the file in bytes; logs their texts at the `debug` level.
fn round_trip(world: &mut World, scenes: &mut SceneLibrary) -> Result<(usize, usize), AppError> {
    let save = world.save(&made(scenes)?, &BTreeMap::new())?;
    tracing::debug!("the save:\n{save}");
    let mut loaded = made(scenes)?;
    let report = loaded.load(&save, scenes)?;
    if !report.set_aside.is_empty() {
        return Err(format!("the save set aside {:?}", report.set_aside).into());
    }
    if let Some(difference) = world.first_difference(&loaded) {
        return Err(format!("the world loaded is not the world saved: {difference}").into());
    }
    // The yard written back, then spawned from the file in place of its source.
    let root = world
        .find(&PersistentId::new([KEY]))
        .ok_or("the yard is gone")?;
    let written = world.write_scene(root)?;
    tracing::debug!("the yard written back:\n{}", written.text);
    if !written.left_out.is_empty() {
        return Err(format!("the yard written back left out {:?}", written.left_out).into());
    }
    scenes.insert_file(YARD, "yard", written.text.clone());
    if let Some(difference) = world.first_difference(&made(scenes)?) {
        return Err(format!(
            "the yard spawned from the file is not the world written: {difference}"
        )
        .into());
    }
    Ok((save.len(), written.text.len()))
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
        if let Some(mut transform) = ctx.world_mut().get_mut::<Transform>(ring) {
            transform.translation.y = height;
        }
        self.floated = (self.floated.0.min(height), self.floated.1.max(height));
    }

    fn update(&mut self, ctx: &mut Context, frame: &FrameTime) {
        // Without a window too, so that a run without one follows the cooked data.
        self.watch(ctx);
        self.yaw += TURN_SPEED * frame.delta().as_secs_f32();
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
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
        let mut frame = match self.builder.build(
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
        match round_trip(ctx.world_mut(), &mut self.scenes) {
            Ok((save, file)) => tracing::info!(
                save,
                file,
                "the world, saved and loaded, is the world saved, and the yard written back spawns \
                 the world written"
            ),
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
    let watch = options
        .watch
        .then(|| ManifestWatch::new(&assets, Instant::now()));
    let mut example = WorldExample {
        cache: AssetCache::new(assets.clone()),
        scenes: SceneLibrary::of(assets.clone()),
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
