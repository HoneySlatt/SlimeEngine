//! *Hunna Wood*, a role-playing game made with SlimeEngine, seen in the third person, in a
//! dark-fantasy world. The sample that every stage grows (ADR-0067); `README.md` beside this crate
//! describes what can be played.
//!
//! It reads its zone and its hero from cooked data, which the asset pipeline makes first (see
//! `README.md`):
//!
//! ```sh
//! scripts/fetch-assets.sh zone/
//! cargo run --release -p slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood
//! cargo run -p hunnawood                     # play
//! cargo run -p hunnawood -- --frames 120     # closes itself after 120 frames that draw the zone
//! cargo run -p hunnawood -- --assets <dir>   # reads the cooked data of another directory
//! cargo run -p hunnawood -- --night          # starts at night
//! cargo run -p hunnawood -- --lose-device-at 300   # loses its GPU device after 300 zone frames
//! cargo run -p hunnawood -- --watch          # loads its zone and the shaders again as they change
//! ```
//!
//! W, A, S and D move (Z, Q, S and D on a French keyboard), the left and right arrows turn the
//! camera, E rests by the campfire, which turns the day to night and back, R starts again, F1 shows
//! what each frame costs, Escape quits.

mod assets;
mod camera;
mod controls;
mod rules;
mod statistics;
mod view;
mod zone;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, AppError, Context};
use slime_assets::{AssetId, CookedAssets, CookedScene, ManifestWatch};
use slime_gfx::RenderTarget;
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};
use slime_render::glam::Vec2;
use slime_render::{RenderStats, ShaderSources, ShaderWatch};
use slime_world::World;

use camera::FollowCamera;
use controls::Controls;
use rules::Game;
use statistics::Statistics;
use view::View;

const USAGE: &str = "usage: hunnawood [--assets <directory>] [--frames <count>] [--night] \
                     [--lose-device-at <frame>] [--watch]";
/// The game: its rules, the keys held, the camera, and what draws them once the window is open.
struct HunnaWood {
    /// The cooked data, which the view loads its GPU objects from once the window is open, and
    /// again with a new device.
    assets: CookedAssets,
    /// The game's world, with the zone and the hero, from the start until [`App::init`] gives it
    /// to the engine, which keeps it (`Context::world_mut`).
    world: Option<World>,
    game: Game,
    controls: Controls,
    camera: FollowCamera,
    view: Option<View>,
    /// The dev overlay, shown with F1.
    statistics: Option<Statistics>,
    /// What the last frame cost.
    stats: RenderStats,
    /// The time since the start, which the fire flickers and the key pulses by.
    time: Duration,
    /// How many frames drew the zone.
    drawn: u64,
    /// The number of frames that draw the zone after which the game closes itself, if `--frames`
    /// gives one. A run that sets one fails if no frame drew the zone: the automated checks run the
    /// game for a number of frames in a compositor without a screen, to prove that it draws.
    frame_limit: Option<u64>,
    /// The frame that draws the zone after which the game loses its GPU device on purpose, if
    /// `--lose-device-at` gives one, to check that it goes on with a new one.
    lose_device_at: Option<u64>,
    /// The frames drawn while the zone loads; `None` once they are logged.
    loading: Option<Loading>,
    /// With `--watch`, what tells when the cooked data changes.
    watch: Option<ManifestWatch>,
    /// With `--watch`, what tells when the renderer's shader files change.
    shader_watch: Option<ShaderWatch>,
}

/// The frames drawn while the zone loads, and the longest interval between the starts of two
/// frames, until the first frame that draws the zone has been followed by another: what loading
/// costs the player, logged once it is over.
#[derive(Default)]
struct Loading {
    /// How many frames were drawn without the zone.
    frames: u64,
    /// When the last frame started.
    last: Option<Instant>,
    longest: Duration,
    /// Whether a frame drew the zone.
    shown: bool,
}

impl Loading {
    /// Logs the frames drawn while the zone loaded, and the longest interval between two of them.
    fn log(&self) {
        tracing::info!(
            frames = self.frames,
            longest_interval = ?self.longest,
            "the zone is drawn"
        );
    }
}

impl HunnaWood {
    /// The game in `world`, which `game` plays in, drawn from what `assets` holds, as `options`
    /// ask: for a number of frames, losing its GPU device after some, or loading its data again
    /// when it changes.
    fn new(assets: CookedAssets, world: World, game: Game, options: &Options) -> Self {
        let (_, facing) = game.zone().start(&world);
        Self {
            camera: FollowCamera::looking_along(facing),
            world: Some(world),
            game,
            controls: Controls::default(),
            view: None,
            statistics: None,
            stats: RenderStats::default(),
            time: Duration::ZERO,
            drawn: 0,
            frame_limit: options.frame_limit,
            lose_device_at: options.lose_device_at,
            loading: Some(Loading::default()),
            watch: options
                .watch
                .then(|| ManifestWatch::new(&assets, Instant::now())),
            shader_watch: options
                .watch
                .then(|| ShaderWatch::new(ShaderSources::DIRECTORY, Instant::now())),
            assets,
        }
    }

    /// Takes `assets`, the cooked data read again after it changed: the zone and the hero are
    /// spawned again in the world if their scenes changed, and the view loads again what changed.
    /// The game starts again if the zone's layout changed, the camera with it. A zone or a hero
    /// that cannot be read changes nothing: the data before stays.
    fn take_assets(&mut self, ctx: &mut Context, assets: CookedAssets) {
        let changed = |id: AssetId| {
            let file = |assets: &CookedAssets| assets.manifest().get(id).map(|e| e.file.clone());
            file(&self.assets) != file(&assets)
        };
        let scene = |id: AssetId| -> Result<Option<CookedScene>, String> {
            if !changed(id) {
                return Ok(None);
            }
            assets
                .scene(id)
                .map(Some)
                .map_err(|error| error.to_string())
        };
        let scenes =
            scene(assets::zone_id()).and_then(|zone| Ok((zone, scene(assets::hero_id())?)));
        let (zone, hero) = match scenes {
            Ok(scenes) => scenes,
            Err(error) => {
                tracing::warn!("the cooked data cannot be played: {error}; it stays as it was");
                return;
            }
        };
        if let Some(zone) = zone {
            match self.game.reload_zone(ctx.world_mut(), &zone) {
                Ok(true) => {
                    let (_, facing) = self.game.zone().start(ctx.world());
                    self.camera = FollowCamera::looking_along(facing);
                    tracing::info!("the zone's layout changed: the run starts again");
                }
                Ok(false) => tracing::info!("the zone changed, and the run goes on"),
                Err(error) => {
                    tracing::warn!(
                        "the zone loaded again cannot be played: {error}; it stays as it was"
                    );
                    return;
                }
            }
        }
        if let Some(hero) = hero
            && let Err(error) = self.game.reload_hero(ctx.world_mut(), &hero)
        {
            tracing::warn!("the hero cannot be spawned again: {error}");
        }
        if let (Some(gpu), Some(view)) = (ctx.gpu(), &mut self.view) {
            view.reload(gpu, assets.clone());
        }
        self.assets = assets;
    }
}

/// Moves the hero of `game` in `world` by one fixed step of `step` as `controls` ask, relative to
/// `camera`, once the zone is drawn, as `zone_shown` says: until then, the game waits, so that the
/// hero does not walk where the player cannot see.
fn step(
    game: &Game,
    world: &mut World,
    camera: &FollowCamera,
    controls: &Controls,
    zone_shown: bool,
    step: Duration,
) {
    if zone_shown {
        game.step(
            world,
            camera.on_ground(controls.movement()),
            step.as_secs_f32(),
        );
    }
}

impl App for HunnaWood {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        if let Some(world) = self.world.take() {
            *ctx.world_mut() = world;
        }
        let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        else {
            return Err("Hunna Wood needs a window".into());
        };
        let (world, hero) = (ctx.world(), self.game.hero_entity());
        let night = self.game.night(world);
        self.view = Some(View::new(gpu, format, &self.assets, world, hero, night)?);
        self.statistics = Some(Statistics::new(gpu, format, window.scale_factor()));
        if self.frame_limit == Some(0) {
            ctx.request_exit();
        }
        Ok(())
    }

    fn gpu_lost(&mut self, _ctx: &mut Context) {
        // Every object of the lost device goes, the view's loads in progress with it, so that the
        // new device opens on the same adapter (ADR-0075). The world, and the run in it, are kept.
        self.view = None;
        if let Some(statistics) = &mut self.statistics {
            statistics.release();
        }
        if let Some(loading) = self.loading.take()
            && loading.shown
        {
            loading.log();
        }
    }

    fn gpu_restored(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        else {
            return Err("Hunna Wood needs a window".into());
        };
        let (world, hero) = (ctx.world(), self.game.hero_entity());
        let night = self.game.night(world);
        self.view = Some(View::new(gpu, format, &self.assets, world, hero, night)?);
        if let Some(statistics) = &mut self.statistics {
            statistics.restore(gpu, format, window.scale_factor());
        }
        self.loading = Some(Loading::default());
        // A reload in progress went with the lost device: the data it loaded is given again.
        if let Some(watch) = &mut self.watch {
            watch.start_from(&self.assets);
        }
        // The new view's renderer has the embedded shaders: the files that differ are given again.
        if let Some(shader_watch) = &mut self.shader_watch {
            shader_watch.start_again();
        }
        let hero = self.game.hero(world);
        tracing::info!(
            x = hero.position.x,
            z = hero.position.y,
            key = self.game.has_item(world),
            door = self.game.door_open(world),
            night,
            "a new GPU device replaces the lost one: the zone loads again, and the run goes on"
        );
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
                    self.game.restart(ctx.world_mut());
                    let (_, facing) = self.game.zone().start(ctx.world());
                    self.camera = FollowCamera::looking_along(facing);
                }
                KeyCode::KeyE => {
                    if self.game.rest(ctx.world_mut()) {
                        let night = self.game.night(ctx.world());
                        let time = if night { "night" } else { "day" };
                        tracing::info!("the hero rests by the campfire until the {time}");
                    }
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

    fn fixed_update(&mut self, ctx: &mut Context, duration: Duration) {
        let zone_shown = self.view.as_ref().is_some_and(View::zone_shown);
        step(
            &self.game,
            ctx.world_mut(),
            &self.camera,
            &self.controls,
            zone_shown,
            duration,
        );
    }

    fn update(&mut self, _ctx: &mut Context, frame: &FrameTime) {
        self.time = frame.elapsed();
        let seconds = frame.delta().as_secs_f32();
        self.camera.turn(self.controls.turn(), seconds);
        if let Some(statistics) = &mut self.statistics {
            statistics.frame(seconds);
        }
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let now = Instant::now();
        if let Some(loading) = &mut self.loading {
            if let Some(last) = loading.last {
                loading.longest = loading.longest.max(now - last);
            }
            loading.last = Some(now);
            // The interval that ends here holds the first frame that drew the zone.
            if loading.shown {
                loading.log();
                self.loading = None;
            }
        }
        if let (Some(shader_watch), Some(gpu), Some(view)) =
            (&mut self.shader_watch, ctx.gpu(), &mut self.view)
        {
            match shader_watch.poll(now) {
                Ok(Some(sources)) => {
                    let started = Instant::now();
                    match view.set_shaders(gpu, &sources) {
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
                    "the shaders cannot be read from {}: {error}; the game draws with those it \
                     has",
                    ShaderSources::DIRECTORY
                ),
            }
        }
        // Once the zone is shown, so that the run goes on from what was drawn, and once nothing
        // loads, so that no load is dropped half-way, which waits for its running tasks.
        if let (Some(watch), Some(view)) = (&mut self.watch, &self.view)
            && view.zone_shown()
            && view.settled()
        {
            match watch.poll(now) {
                Ok(Some(assets)) => {
                    tracing::info!("the cooked data changed: what changed loads again");
                    self.take_assets(ctx, assets);
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(
                    "the cooked data cannot be read: {error}; the game goes on with what it loaded"
                ),
            }
        }
        let seconds = self.time.as_secs_f64();
        view::light_the_fire(ctx.world_mut(), seconds);
        // What moves is drawn between its last two fixed steps, the share of a step this frame
        // comes after the last one.
        let alpha = ctx.fixed_step_alpha();
        let (Some(gpu), Some(view)) = (ctx.gpu(), &mut self.view) else {
            return;
        };
        let world = ctx.world();
        let hero = world
            .interpolated_transform(self.game.hero_entity(), alpha)
            .map_or(Vec2::ZERO, |at| {
                Vec2::new(at.translation.x as f32, at.translation.z as f32)
            });
        let camera = self.camera.camera(hero, &zone::colliders(world));
        let night = self.game.night(world);
        let drawn = view
            .glow(gpu, night, seconds)
            .map_err(AppError::from)
            .and_then(|()| view.draw(gpu, target, world, night, alpha, camera));
        match drawn {
            Ok(stats) => {
                self.stats = stats;
                let lose_asked = match &mut self.statistics {
                    Some(statistics) => statistics.draw(gpu, target, &self.stats),
                    None => false,
                };
                let mut lose_now = lose_asked;
                if view.zone_shown() {
                    self.drawn += 1;
                    if let Some(loading) = &mut self.loading {
                        loading.shown = true;
                    }
                    lose_now |= self.lose_device_at == Some(self.drawn);
                } else if let Some(loading) = &mut self.loading {
                    loading.frames += 1;
                }
                if lose_now {
                    // Once the frame is drawn, as a driver reset would come: the engine opens a
                    // new device before the next frame, and calls `gpu_restored`.
                    tracing::warn!("the GPU device is lost on purpose");
                    gpu.lose();
                }
                if view.zone_shown() && self.frame_limit.is_some_and(|limit| self.drawn >= limit) {
                    ctx.request_exit();
                }
            }
            // The GPU's memory cannot hold the targets of the window's new size, the device was
            // lost, or the zone cannot be loaded: the game stops with the error, which `run` logs
            // with its causes.
            Err(error) => ctx.exit_with_error(error),
        }
    }

    fn shutdown(&mut self, ctx: &mut Context) {
        // A run that stops right after the zone's first frame, as the automated checks' do, logs
        // the load here, without the interval that holds that frame.
        if let Some(loading) = self.loading.take()
            && loading.shown
        {
            loading.log();
        }
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
            ctx.exit_with_error("no frame drew the zone");
        }
    }
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    // The zone and the hero are spawned in the game's world before the window opens, which a zone
    // that cannot be played keeps closed.
    let directory = options
        .assets
        .clone()
        .unwrap_or_else(assets::default_directory);
    let loaded = CookedAssets::open(&directory)
        .map_err(|error| error.to_string())
        .and_then(|cooked| {
            let scene = |id| cooked.scene(id).map_err(|error| error.to_string());
            let (zone, hero) = (scene(assets::zone_id())?, scene(assets::hero_id())?);
            let mut world = World::new();
            let game = Game::spawn(&mut world, &zone, &hero, options.night)?;
            Ok((cooked, world, game))
        });
    let (cooked, world, game) = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!(
                "cannot read the cooked zone in {}: {error}\n{}",
                directory.display(),
                assets::HOW_TO_COOK
            );
            return ExitCode::FAILURE;
        }
    };
    let config = AppConfig {
        window: Some(WindowConfig {
            title: "Hunna Wood".to_owned(),
            ..WindowConfig::default()
        }),
        // The game counts its frames itself: those that draw the zone.
        // It goes on after a lost GPU device, with a new one (ADR-0075).
        restore_lost_device: true,
        ..AppConfig::default()
    };
    let mut game = HunnaWood::new(cooked, world, game, &options);
    match slime_app::run(&mut game, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// The command line's options.
#[derive(Default)]
struct Options {
    /// The directory of cooked data to read, if `--assets` gives one.
    assets: Option<PathBuf>,
    /// The number of frames after which the game closes itself, if `--frames` gives one.
    frame_limit: Option<u64>,
    /// Whether the game starts at night, as `--night` asks.
    night: bool,
    /// The frame that draws the zone after which the game loses its GPU device, if
    /// `--lose-device-at` gives one.
    lose_device_at: Option<u64>,
    /// Whether the game loads its zone and its hero again when their cooked data changes, and the
    /// renderer's shaders when their files do, as `--watch` asks.
    watch: bool,
}

/// The options that `arguments`, the command line without the program's name, give.
fn parse_options(arguments: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--night") => options.night = true,
            Some("--watch") => options.watch = true,
            Some("--assets") => {
                let value = arguments.next().ok_or("--assets needs a directory")?;
                options.assets = Some(PathBuf::from(value));
            }
            Some(name @ ("--frames" | "--lose-device-at")) => {
                let value = arguments
                    .next()
                    .ok_or_else(|| format!("{name} needs a frame count"))?;
                let count = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?;
                if name == "--frames" {
                    options.frame_limit = Some(count);
                } else {
                    options.lose_device_at = Some(count);
                }
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use slime_platform::Key;

    use super::*;
    use crate::rules::tests::zone;

    fn parse(arguments: &[&str]) -> Result<Options, String> {
        parse_options(arguments.iter().map(OsString::from))
    }

    #[test]
    fn night_starts_the_game_at_night() {
        assert!(!parse(&[]).expect("no option").night);
        let options = parse(&["--frames", "2", "--night"]).expect("valid options");
        assert!(options.night);
        assert_eq!(options.frame_limit, Some(2));
        assert!(parse(&["--day"]).is_err());
    }

    #[test]
    fn watch_loads_the_cooked_data_again() {
        assert!(!parse(&[]).expect("no option").watch);
        assert!(parse(&["--watch", "--night"]).expect("valid options").watch);
    }

    #[test]
    fn lose_device_at_takes_a_frame() {
        assert_eq!(parse(&[]).expect("no option").lose_device_at, None);
        let options = parse(&["--lose-device-at", "300"]).expect("a frame");
        assert_eq!(options.lose_device_at, Some(300));
        assert!(parse(&["--lose-device-at"]).is_err());
        assert!(parse(&["--lose-device-at", "soon"]).is_err());
    }

    #[test]
    fn the_hero_waits_for_the_zone_to_be_drawn() {
        let (mut world, game) = zone([0.0, 0.0], Vec::new()).play();
        let camera = FollowCamera::looking_along(Vec2::X);
        let mut controls = Controls::default();
        controls.handle(&KeyEvent {
            physical: KeyCode::KeyW,
            logical: Key::Unidentified,
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
        });
        let tick = Duration::from_secs(1) / 60;
        // Half a second with the key held while the zone loads: the hero stays at its start.
        for _ in 0..30 {
            step(&game, &mut world, &camera, &controls, false, tick);
        }
        assert_eq!(game.hero(&world).position, Vec2::ZERO);
        for _ in 0..30 {
            step(&game, &mut world, &camera, &controls, true, tick);
        }
        let hero = game.hero(&world).position;
        assert!(hero.x > 0.5, "{hero}");
    }
}
