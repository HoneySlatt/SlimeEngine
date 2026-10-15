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
use slime_assets::CookedAssets;
use slime_gfx::RenderTarget;
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};
use slime_render::RenderStats;

use camera::FollowCamera;
use controls::Controls;
use rules::Game;
use statistics::Statistics;
use view::{View, ZoneLights};
use zone::Zone;

const USAGE: &str = "usage: hunnawood [--assets <directory>] [--frames <count>] [--night]";

/// The game: its rules, the keys held, the camera, and what draws them once the window is open.
struct HunnaWood {
    /// The cooked data, which the view loads its GPU objects from once the window is open.
    assets: CookedAssets,
    /// The zone's lights, which the view takes when the window opens.
    lights: Option<ZoneLights>,
    game: Game,
    controls: Controls,
    camera: FollowCamera,
    view: Option<View>,
    /// The dev overlay, shown with F1.
    statistics: Option<Statistics>,
    /// What the last frame cost.
    stats: RenderStats,
    /// How many frames drew the zone.
    drawn: u64,
    /// The number of frames that draw the zone after which the game closes itself, if `--frames`
    /// gives one. A run that sets one fails if no frame drew the zone: the automated checks run the
    /// game for a number of frames in a compositor without a screen, to prove that it draws.
    frame_limit: Option<u64>,
    /// The frames drawn while the zone loads; `None` once they are logged.
    loading: Option<Loading>,
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
}

impl HunnaWood {
    /// The game in `zone`, starting at night if `night`, which draws what `assets` holds and closes
    /// itself after `frame_limit` frames that draw the zone, if it is given.
    fn new(
        assets: CookedAssets,
        zone: Zone,
        lights: ZoneLights,
        night: bool,
        frame_limit: Option<u64>,
    ) -> Self {
        let game = Game::new(zone);
        Self {
            assets,
            lights: Some(lights),
            camera: FollowCamera::looking_along(game.zone().facing),
            game: if night {
                game.starting_at_night()
            } else {
                game
            },
            controls: Controls::default(),
            view: None,
            statistics: None,
            stats: RenderStats::default(),
            drawn: 0,
            frame_limit,
            loading: Some(Loading::default()),
        }
    }
}

/// Moves the hero of `game` by one fixed step of `step` as `controls` ask, relative to `camera`,
/// once the zone is drawn, as `zone_shown` says: until then, the game waits, so that the hero does
/// not walk where the player cannot see.
fn step(
    game: &mut Game,
    camera: &FollowCamera,
    controls: &Controls,
    zone_shown: bool,
    step: Duration,
) {
    if zone_shown {
        game.step(camera.on_ground(controls.movement()), step.as_secs_f32());
    }
}

impl App for HunnaWood {
    fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
        let (Some(gpu), Some(format), Some(window)) =
            (ctx.gpu(), ctx.surface_format(), ctx.window())
        else {
            return Err("Hunna Wood needs a window".into());
        };
        let lights = self.lights.take().expect("the window opens once");
        self.view = Some(View::new(gpu, format, &self.assets, lights)?);
        self.statistics = Some(Statistics::new(gpu, format, window.scale_factor()));
        if self.frame_limit == Some(0) {
            ctx.request_exit();
        }
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
                KeyCode::KeyE => {
                    if self.game.rest() {
                        let time = if self.game.night() { "night" } else { "day" };
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

    fn fixed_update(&mut self, _ctx: &mut Context, duration: Duration) {
        let zone_shown = self.view.as_ref().is_some_and(View::zone_shown);
        step(
            &mut self.game,
            &self.camera,
            &self.controls,
            zone_shown,
            duration,
        );
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
        let now = Instant::now();
        if let Some(loading) = &mut self.loading {
            if let Some(last) = loading.last {
                loading.longest = loading.longest.max(now - last);
            }
            loading.last = Some(now);
            // The interval that ends here holds the first frame that drew the zone.
            if self.drawn > 0 {
                tracing::info!(
                    frames = loading.frames,
                    longest_interval = ?loading.longest,
                    "the zone is drawn"
                );
                self.loading = None;
            }
        }
        if let Err(error) = view.poll() {
            ctx.exit_with_error(error);
            return;
        }
        // The hero is drawn between its last two fixed steps, the share of a step this frame comes
        // after the last one.
        let alpha = ctx.fixed_step_alpha() as f32;
        let hero = self.game.hero_between(alpha);
        let camera = self.camera.camera(hero.position, self.game.colliders());
        match view.draw(gpu, target, &self.game, alpha, camera) {
            Ok(stats) => {
                self.stats = stats;
                if let Some(statistics) = &mut self.statistics {
                    statistics.draw(gpu, target, &self.stats);
                }
                if view.zone_shown() {
                    self.drawn += 1;
                    if self.frame_limit.is_some_and(|limit| self.drawn >= limit) {
                        ctx.request_exit();
                    }
                } else if let Some(loading) = &mut self.loading {
                    loading.frames += 1;
                }
            }
            // The GPU's memory cannot hold the targets of the window's new size, or the device was
            // lost: the game stops with the error, which `run` logs with its causes.
            Err(error) => ctx.exit_with_error(error),
        }
    }

    fn shutdown(&mut self, ctx: &mut Context) {
        // A run that stops right after the zone's first frame, as the automated checks' do, logs
        // the load here, without the interval that holds that frame.
        if let Some(loading) = self.loading.take()
            && self.drawn > 0
        {
            tracing::info!(
                frames = loading.frames,
                longest_interval = ?loading.longest,
                "the zone is drawn"
            );
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
    // The zone's colliders, places and lights come from the cooked scene, before the window opens.
    let directory = options.assets.unwrap_or_else(assets::default_directory);
    let loaded = CookedAssets::open(&directory)
        .map_err(|error| error.to_string())
        .and_then(|cooked| {
            let scene = cooked
                .scene(assets::zone_id())
                .map_err(|error| error.to_string())?;
            let lights = ZoneLights::of(&scene).map_err(|error| error.to_string())?;
            Ok((cooked, Zone::from_scene(&scene)?, lights))
        });
    let (cooked, zone, lights) = match loaded {
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
        ..AppConfig::default()
    };
    let mut game = HunnaWood::new(cooked, zone, lights, options.night, options.frame_limit);
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
}

/// The options that `arguments`, the command line without the program's name, give.
fn parse_options(arguments: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--night") => options.night = true,
            Some("--assets") => {
                let value = arguments.next().ok_or("--assets needs a directory")?;
                options.assets = Some(PathBuf::from(value));
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

#[cfg(test)]
mod tests {
    use slime_platform::Key;
    use slime_render::glam::Vec2;

    use super::*;

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
    fn the_hero_waits_for_the_zone_to_be_drawn() {
        let far = Vec2::splat(1000.0);
        let mut game = Game::new(Zone {
            half_size: Vec2::splat(50.0),
            colliders: Vec::new(),
            start: Vec2::ZERO,
            facing: Vec2::X,
            item: far,
            door: far,
            campfire: far,
            terrace: far,
            door_colliders: Vec::new(),
            item_nodes: Default::default(),
            door_nodes: Default::default(),
        });
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
            step(&mut game, &camera, &controls, false, tick);
        }
        assert_eq!(game.hero().position, Vec2::ZERO);
        for _ in 0..30 {
            step(&mut game, &camera, &controls, true, tick);
        }
        assert!(game.hero().position.x > 0.5, "{}", game.hero().position);
    }
}
