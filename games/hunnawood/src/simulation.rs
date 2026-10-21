//! The game's rules as a simulation that a host runs (`slime_app::Simulation`,
//! `docs/adr/0107-the-execution-model.md`): the hero walks at each fixed step of game time where
//! the keys held ask, relative to where the player looks; the keys pressed rest by the campfire,
//! start again, save and load the run, and quit; the run is saved when the game stops, and, with
//! `--watch`, carried into the scenes that change on disk. It has no GPU and no window: a test runs
//! it with a host of its own, without either.

use std::io;
use std::path::Path;
use std::time::Instant;

use slime_app::{AppError, Simulation, SimulationContext};
use slime_assets::{CookedAssets, ManifestWatch};
use slime_platform::KeyCode;
use slime_world::{Loaded, SceneLibrary, World};

use crate::controls;
use crate::look::Look;
use crate::rules::Game;
use crate::saves::{self, SaveFile};
use crate::zone::Layout;

/// *Hunna Wood*'s rules, which a host runs: the game in its world, the scenes it is a game of, and
/// the save's file.
pub struct HunnaWood {
    /// The game's world, with the zone and the hero, from its making until
    /// [`Simulation::start`] gives it to the host, which keeps it.
    world: Option<World>,
    game: Game,
    /// The layout of the zone of the new game by day that the run started from, which its saves
    /// say ([`saves::save`]).
    layout: Layout,
    /// The scenes of the cooked data, the zone's, those it places and the hero's, of which a new
    /// game by day is made, which saves differ from: the scenes of the run in the world.
    scenes: SceneLibrary,
    /// The save's file, which this copy of the game holds, or why the run is not saved: the
    /// environment gives no data directory and `--save` names no file, another copy of the game
    /// holds the file, or the save there can be neither read nor set aside.
    save_file: Result<SaveFile, String>,
    /// With `--watch`, what tells when the cooked data changes, and the cooked data that the
    /// scenes of the run come from, which the library goes back to when the scenes that changed
    /// cannot be played.
    watch: Option<(ManifestWatch, CookedAssets)>,
}

impl HunnaWood {
    /// The rules of `game` in `world`, a game of `scenes` started from a new game whose zone's
    /// layout is `layout`, saving to `save_file`, following the cooked data that `watch` watches,
    /// if it watches it.
    pub fn new(
        (world, game): (World, Game),
        layout: Layout,
        scenes: SceneLibrary,
        save_file: Result<SaveFile, String>,
        watch: Option<(ManifestWatch, CookedAssets)>,
    ) -> Self {
        Self {
            world: Some(world),
            game,
            layout,
            scenes,
            save_file,
            watch,
        }
    }

    /// Saves the run in `world` to the save's file, as F5 asks and as stopping does.
    fn save_run(&mut self, world: &World) {
        let file = match &self.save_file {
            Ok(file) => file.path(),
            Err(why) => {
                tracing::warn!("the run is not saved: {why}");
                return;
            }
        };
        match self.save_to(world, file) {
            Ok(()) => log_run(
                &self.game,
                world,
                &format!("the run is saved to {}", file.display()),
            ),
            Err(error) => tracing::warn!("the run cannot be saved to {}: {error}", file.display()),
        }
    }

    /// Saves the run in `world` to `file`.
    fn save_to(&self, world: &World, file: &Path) -> Result<(), String> {
        let text = saves::save(world, &self.layout)?;
        saves::write(file, &text).map_err(|error| error.to_string())
    }

    /// Loads the run of the save's file in place of the run in `world`, as F9 asks: the hero is
    /// drawn at once where the save puts it. A save that cannot be resumed leaves the run as it is.
    fn load_run(&mut self, world: &mut World) {
        let file = match &self.save_file {
            Ok(file) => file.path().to_owned(),
            Err(why) => {
                tracing::warn!("no save is loaded: {why}");
                return;
            }
        };
        match resume_from(&file, &mut self.scenes) {
            Resume::Run(run) => self.take(world, *run),
            Resume::Nothing => {}
            Resume::Keep => self.save_file = Err(kept(&file)),
        }
    }

    /// Puts `run`, a run resumed, in place of the run in `world`, its game starting at the time of
    /// day that the run's does.
    fn take(&mut self, world: &mut World, (resumed, mut game): (World, Game)) {
        game.set_starts_at_night(self.game.starts_at_night());
        *world = resumed;
        self.game = game;
        self.look_somewhere(world);
    }

    /// Makes the player look where the hero faces when the run holds no look, as the run of a save
    /// made before the game kept it.
    fn look_somewhere(&self, world: &mut World) {
        if world.get::<Look>(self.game.hero_entity()).is_none() {
            self.game.look_along_hero(world);
        }
    }

    /// With `--watch`, takes the cooked data that changed: when scenes changed, the zone's, those
    /// it places or the hero's, the run in `world` is carried into a new game of the scenes as they
    /// are now, or starts again in it when the zone's layout changed ([`saves::reload`]). Scenes
    /// that cannot be played change nothing: the library goes back to the scenes of the run.
    fn watch(&mut self, world: &mut World) {
        let Some((watch, _)) = &mut self.watch else {
            return;
        };
        let assets = match watch.poll(Instant::now()) {
            Ok(Some(assets)) => assets,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(
                    "the cooked data cannot be read: {error}; the game goes on with what it loaded"
                );
                return;
            }
        };
        tracing::info!("the cooked data changed: what changed loads again");
        let changed = self.scenes.reload(assets.clone());
        if !changed.is_empty() {
            let starts_at_night = self.game.starts_at_night();
            let reloaded = saves::reload(world, &self.layout, &mut self.scenes, starts_at_night);
            let reloaded = match reloaded {
                Ok(reloaded) => reloaded,
                Err(error) => {
                    if let Some((_, assets)) = &self.watch {
                        self.scenes.reload(assets.clone());
                    }
                    tracing::warn!(
                        "the scenes loaded again cannot be played: {error}; they stay as they were"
                    );
                    return;
                }
            };
            for aside in reloaded.loaded.iter().flat_map(|loaded| &loaded.set_aside) {
                tracing::warn!(
                    entity = aside.entity,
                    component = aside.component,
                    "set aside from the run: {}",
                    aside.reason
                );
            }
            *world = reloaded.world;
            self.game = reloaded.game;
            self.layout = reloaded.layout;
            if reloaded.restarted {
                self.game.look_along_hero(world);
                tracing::info!("the zone's layout changed: the run starts again");
            } else {
                self.look_somewhere(world);
                tracing::info!(
                    scenes = changed.len(),
                    "the scenes changed, and the run goes on"
                );
            }
        }
        if let Some((_, watched)) = &mut self.watch {
            *watched = assets;
        }
    }
}

impl Simulation for HunnaWood {
    fn start(
        &mut self,
        _ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        if let Some(run) = self.world.take() {
            *world = run;
        }
        self.look_somewhere(world);
        Ok(())
    }

    fn fixed_step(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        let look = world
            .get::<Look>(self.game.hero_entity())
            .copied()
            .unwrap_or(Look { yaw: 0.0 });
        let direction = look.on_ground(controls::movement(ctx.input()));
        let step = ctx.time().game().step().as_secs_f32();
        self.game.step(world, direction, step);
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        // Where the player looks turns at real time, the game paused or slowed down too.
        let turn = controls::turn(ctx.input());
        if turn != 0.0
            && let Some(mut look) = world.get_mut::<Look>(self.game.hero_entity())
        {
            look.turn(turn, ctx.time().real().delta().as_secs_f32());
        }
        let keys = ctx.input().keys();
        let (quit, restart, rest) = (
            keys.pressed(KeyCode::Escape),
            keys.pressed(KeyCode::KeyR),
            keys.pressed(KeyCode::KeyE),
        );
        let (save, load) = (keys.pressed(KeyCode::F5), keys.pressed(KeyCode::F9));
        if quit {
            ctx.request_exit();
        }
        if restart {
            self.game.restart(world);
            self.game.look_along_hero(world);
        }
        if rest && self.game.rest(world) {
            let time = if self.game.night(world) {
                "night"
            } else {
                "day"
            };
            tracing::info!("the hero rests by the campfire until the {time}");
        }
        if save {
            self.save_run(world);
        }
        if load {
            self.load_run(world);
        }
        self.watch(world);
    }

    fn stop(&mut self, _ctx: &mut SimulationContext<'_>, world: &mut World) {
        // However the run stops: Escape, the window closed, a signal, an error.
        self.save_run(world);
    }

    fn save(&mut self, world: &World, file: &Path) -> Result<(), AppError> {
        self.save_to(world, file)?;
        log_run(
            &self.game,
            world,
            &format!("the run is saved to {}", file.display()),
        );
        Ok(())
    }

    fn load(&mut self, world: &mut World, file: &Path) -> Result<(), AppError> {
        let text = saves::read(file)?;
        let start = saves::start(&mut self.scenes)?;
        let (resumed, game, loaded) = saves::resume(&text, start, &mut self.scenes)?;
        log_loaded(&loaded);
        log_run(
            &game,
            &resumed,
            &format!("the run is loaded from {}", file.display()),
        );
        self.take(world, (resumed, game));
        Ok(())
    }
}

/// What the save's file gives a run.
pub enum Resume {
    /// The run of the save, resumed in a new game.
    Run(Box<(World, Game)>),
    /// No run: the file holds no save, or held one that is set aside, which the next save does not
    /// write over.
    Nothing,
    /// No run, and a save that can be neither read nor set aside, a save not checked as the zone
    /// cannot be played, or what is not a file in its place: the next save would write over it,
    /// which the game must not do.
    Keep,
}

/// The run of the save in `file`, resumed in a new game of `scenes`, if the file holds one that can
/// be resumed ([`saves::resume`]); logged either way. A save that is not resumed is set aside
/// ([`saves::set_aside`]), so that the next save does not write over it, under a name that says why
/// ([`saves::NotResumed::aside`]): `unreadable`, `earlier-version`, `later-version`,
/// `other-layout`, `unknown-content`, `unplayable`. What is not a file, a directory or a pipe, is
/// neither read nor moved.
pub fn resume_from(file: &Path, scenes: &mut SceneLibrary) -> Resume {
    let shown = file.display();
    match std::fs::metadata(file) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            tracing::info!("there is no save in {shown}");
            return Resume::Nothing;
        }
        Ok(metadata) if !metadata.is_file() => {
            tracing::warn!("{shown} is not a file: it is neither read nor set aside");
            return Resume::Keep;
        }
        _ => {}
    }
    let text = match saves::read(file) {
        Ok(text) => text,
        Err(error) => return set_aside(file, "unreadable", &error.to_string()),
    };
    let start = match saves::start(scenes) {
        Ok(start) => start,
        Err(error) => {
            tracing::warn!(
                "the save in {shown} is not resumed: the zone cannot be played: {error}"
            );
            return Resume::Keep;
        }
    };
    match saves::resume(&text, start, scenes) {
        Ok((world, game, loaded)) => {
            log_loaded(&loaded);
            log_run(&game, &world, &format!("the run resumes from {shown}"));
            Resume::Run(Box::new((world, game)))
        }
        Err(why) => set_aside(file, why.aside(), &why.to_string()),
    }
}

/// Logs what loading a save said: the scenes that changed since, and what it set aside.
fn log_loaded(loaded: &Loaded) {
    for scene in &loaded.scenes_changed {
        tracing::info!("the scene `{scene}` changed since the save");
    }
    for aside in &loaded.set_aside {
        tracing::warn!(
            entity = aside.entity,
            component = aside.component,
            "set aside from the save: {}",
            aside.reason
        );
    }
}

/// Sets aside the save in `file`, which is not resumed for `why`, under a name that ends in `what`,
/// and logs it: [`Resume::Nothing`] once it is, [`Resume::Keep`] if it cannot be.
fn set_aside(file: &Path, what: &str, why: &str) -> Resume {
    match saves::set_aside(file, what) {
        Ok(aside) => {
            tracing::warn!(
                "the save in {} is not resumed: {why}; it is set aside as {}",
                file.display(),
                aside.display()
            );
            Resume::Nothing
        }
        Err(error) => {
            tracing::warn!(
                "the save in {} is not resumed: {why}; nor can it be set aside: {error}",
                file.display()
            );
            Resume::Keep
        }
    }
}

/// Why the run is not saved to `file`, whose save it keeps.
pub fn kept(file: &Path) -> String {
    format!(
        "the save in {} can be neither resumed nor set aside, and is kept as it is; --save names \
         another file",
        file.display()
    )
}

/// Logs `what`, with where the run that `game` plays in `world` stands.
pub fn log_run(game: &Game, world: &World, what: &str) {
    let hero = game.hero(world);
    tracing::info!(
        x = hero.position.x,
        z = hero.position.y,
        key = game.has_item(world),
        door = game.door_open(world),
        night = game.night(world),
        "{what}"
    );
}

#[cfg(test)]
mod tests {
    use slime_app::{AppConfig, Host};

    use slime_render::glam::Vec2;

    use super::*;
    use crate::rules::tests::zone;
    use crate::rules::{self, HERO_SPEED};
    use crate::zone::cooked;

    /// The rules of a game in a zone without colliders, the hero at its middle facing +x, started
    /// in a host without a window or a GPU.
    fn hosted() -> (Host, HunnaWood) {
        let plan = zone([0.0, 0.0], Vec::new());
        let (world, game) = plan.play();
        let layout = game.zone().layout(&world);
        let mut rules = HunnaWood::new(
            (world, game),
            layout,
            plan.scenes(),
            Err("a test saves nothing".to_owned()),
            None,
        );
        let mut host = Host::new(&AppConfig::default());
        host.start(&mut rules).expect("the rules start");
        (host, rules)
    }

    /// Where the hero of the host's world stands.
    fn hero(host: &Host, rules: &HunnaWood) -> Vec2 {
        rules.game.hero(host.world()).position
    }

    #[test]
    fn the_rules_run_hundreds_of_steps_through_a_host_without_a_gpu() {
        let (mut host, mut rules) = hosted();
        let entity = rules::hero_in(host.world()).expect("a hero");
        let look = *host
            .world()
            .get::<Look>(entity)
            .expect("the player looks where it faces");
        assert!(look.ground_axes().0.abs_diff_eq(Vec2::X, 1e-5), "{look:?}");
        // Five seconds of game time, W held: the hero walks forward, along where the player looks.
        host.input_mut().keys_mut().press(KeyCode::KeyW);
        let step = AppConfig::default().fixed_step;
        for _ in 0..300 {
            host.frame(&mut rules, step);
        }
        let walked = hero(&host, &rules);
        assert!(
            (walked.x - HERO_SPEED * 5.0).abs() < 0.01 && walked.y.abs() < 1e-4,
            "{walked}"
        );
        // Paused, the hero stays where it is, W held or not; one step walks one step.
        host.context_mut().pause();
        for _ in 0..60 {
            host.frame(&mut rules, step);
        }
        assert_eq!(hero(&host, &rules), walked);
        host.context_mut().step();
        host.frame(&mut rules, step);
        let stepped = hero(&host, &rules).x - walked.x;
        assert!((stepped - HERO_SPEED / 60.0).abs() < 1e-4, "{stepped}");
        // The arrows turn where the player looks while the game is paused, at real time.
        host.input_mut().keys_mut().release(KeyCode::KeyW);
        host.input_mut().keys_mut().press(KeyCode::ArrowLeft);
        host.frame(&mut rules, step);
        let turned = *host.world().get::<Look>(entity).expect("a look");
        assert!(turned.yaw > look.yaw, "{turned:?}");
        // R starts again, looking where the hero faces; Escape asks the host to stop.
        host.input_mut().keys_mut().release(KeyCode::ArrowLeft);
        host.input_mut().keys_mut().press(KeyCode::KeyR);
        host.frame(&mut rules, step);
        assert_eq!(hero(&host, &rules), Vec2::ZERO);
        assert_eq!(host.world().get::<Look>(entity), Some(&look));
        assert!(!host.context().exit_requested());
        host.input_mut().keys_mut().press(KeyCode::Escape);
        host.frame(&mut rules, step);
        assert!(host.context().exit_requested());
    }

    #[test]
    fn what_is_not_a_file_at_the_save_s_place_is_neither_read_nor_moved() {
        let directory =
            std::env::temp_dir().join(format!("hunnawood-not-a-file-{}", std::process::id()));
        let file = directory.join("save.toml");
        std::fs::create_dir_all(file.join("inside")).expect("a directory in its place");
        let mut scenes = zone([0.0, 0.0], Vec::new()).scenes();
        assert!(matches!(resume_from(&file, &mut scenes), Resume::Keep));
        assert!(file.join("inside").is_dir(), "left where it was");
        assert_eq!(std::fs::read_dir(&directory).expect("listed").count(), 1);
        std::fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn a_save_not_resumed_is_set_aside_under_a_name_that_says_why() {
        let directory =
            std::env::temp_dir().join(format!("hunnawood-set-aside-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a directory");
        let file = directory.join("save.toml");
        let plan = zone([0.0, 0.0], Vec::new());
        let mut scenes = plan.scenes();
        let (world, game) = saves::start(&mut scenes).expect("playable");
        let text = saves::save(&world, &game.zone().layout(&world)).expect("saved");
        let later = text.replacen(
            &format!("version = \"{}\"", saves::THIS_VERSION),
            "version = \"99.0.0\"",
            1,
        );
        for (written, name) in [
            (later.as_str(), "save.toml.later-version"),
            ("not = [ a save", "save.toml.unreadable"),
            ("", "save.toml.unreadable.2"),
            (later.as_str(), "save.toml.later-version.2"),
        ] {
            std::fs::write(&file, written).expect("written");
            assert!(matches!(resume_from(&file, &mut scenes), Resume::Nothing));
            assert!(!file.exists());
            let aside = std::fs::read_to_string(directory.join(name)).expect("set aside");
            assert_eq!(aside, written, "{name}");
        }
        // The save itself is resumed.
        std::fs::write(&file, &text).expect("written");
        assert!(matches!(resume_from(&file, &mut scenes), Resume::Run(_)));
        std::fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn a_save_of_an_earlier_version_of_the_engine_is_set_aside_and_a_new_run_starts() {
        let directory =
            std::env::temp_dir().join(format!("hunnawood-0.3-save-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a directory");
        let file = directory.join("save.toml");
        std::fs::write(&file, include_str!("../fixtures/save-0.3.toml")).expect("written");
        let mut scenes = cooked::zone().1;
        assert!(matches!(resume_from(&file, &mut scenes), Resume::Nothing));
        assert!(!file.exists(), "set aside");
        let aside = directory.join("save.toml.earlier-version");
        assert_eq!(
            std::fs::read_to_string(&aside).expect("set aside"),
            include_str!("../fixtures/save-0.3.toml")
        );
        std::fs::remove_dir_all(&directory).expect("removed");
    }
}
