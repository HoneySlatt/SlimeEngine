//! The program of *Hunna Wood* (`docs/adr/0110-how-the-editor-runs-a-games-code.md`): it reads the
//! command line, makes the run, resumed from the save or new, and runs the game's library, its
//! rules and what the player sees, with `slime_app::run`. `README.md` beside this crate describes
//! what can be played.
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
//! cargo run -p hunnawood -- --night          # a new run starts at night, and so does R
//! cargo run -p hunnawood -- --lose-device-at 300   # loses its GPU device after 300 zone frames
//! cargo run -p hunnawood -- --watch          # loads its zone, hero and shaders again as they change
//! cargo run -p hunnawood -- --new            # starts a new run rather than resume the save
//! cargo run -p hunnawood -- --save <file>    # saves to, and resumes from, another file
//! ```
//!
//! It resumes the run of its save when it starts, and saves the run when it quits; `README.md` says
//! where the save is (`docs/adr/0088-saving-and-resuming-a-run.md`).

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use hunnawood::saves::{self, SaveFile};
use hunnawood::simulation::{Resume, kept, log_run, resume_from};
use hunnawood::{HunnaWood, Screen, assets};
use slime_app::AppConfig;
use slime_assets::{CookedAssets, ManifestWatch};
use slime_platform::WindowConfig;
use slime_world::SceneLibrary;

const USAGE: &str = "usage: hunnawood [--assets <directory>] [--frames <count>] [--night] \
                     [--lose-device-at <frame>] [--watch] [--new] [--save <file>]";

fn main() -> ExitCode {
    // Before the zone is spawned, so that what the world warns of then is logged.
    slime_app::init_diagnostics();
    let options = match parse_options(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    // The zone and the hero are spawned in a game before the window opens, which a zone that cannot
    // be played keeps closed.
    let directory = options
        .assets
        .clone()
        .unwrap_or_else(assets::default_directory);
    let loaded = CookedAssets::open(&directory)
        .map_err(|error| error.to_string())
        .and_then(|cooked| {
            let mut scenes = SceneLibrary::of(cooked.clone());
            let start = saves::start(&mut scenes)?;
            Ok((cooked, scenes, start))
        });
    let (cooked, mut scenes, (mut world, mut game)) = match loaded {
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
    let mut save_file = options
        .save
        .clone()
        .or_else(saves::default_file)
        .ok_or_else(|| {
            "the environment gives no data directory, and --save names no file".to_owned()
        })
        .and_then(SaveFile::hold);
    let resumed = match &save_file {
        Ok(file) if !options.new => resume_from(file.path(), &mut scenes),
        Ok(_) => Resume::Nothing,
        Err(why) => {
            tracing::warn!("no save is resumed, and the run is not saved: {why}");
            Resume::Nothing
        }
    };
    let new_run = !matches!(resumed, Resume::Run(..));
    match resumed {
        Resume::Run(run) => (world, game) = *run,
        Resume::Nothing => {}
        Resume::Keep => save_file = save_file.and_then(|file| Err(kept(file.path()))),
    }
    game.set_starts_at_night(options.night);
    if new_run {
        game.restart(&mut world);
        log_run(&game, &world, "a new run starts");
    }
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
    // Each part follows the cooked data with `--watch`: the rules carry the run into the scenes
    // that change, the screen loads again what it draws.
    let watch = options
        .watch
        .then(|| (ManifestWatch::new(&cooked, Instant::now()), cooked.clone()));
    let mut rules = HunnaWood::new((world, game), scenes, save_file, watch);
    let mut screen = Screen::new(
        cooked,
        options.frame_limit,
        options.lose_device_at,
        options.watch,
    );
    match slime_app::run(&mut rules, &mut screen, &config) {
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
    /// Whether a new run starts at night, and R starts again at night, as `--night` asks.
    night: bool,
    /// The frame that draws the zone after which the game loses its GPU device, if
    /// `--lose-device-at` gives one.
    lose_device_at: Option<u64>,
    /// Whether the game loads its zone and its hero again when their cooked data changes, and the
    /// renderer's shaders when their files do, as `--watch` asks.
    watch: bool,
    /// Whether the game starts a new run rather than resume the save, as `--new` asks.
    new: bool,
    /// The save's file, if `--save` names one.
    save: Option<PathBuf>,
}

/// The options that `arguments`, the command line without the program's name, give.
fn parse_options(arguments: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--night") => options.night = true,
            Some("--watch") => options.watch = true,
            Some("--new") => options.new = true,
            Some("--save") => {
                let value = arguments.next().ok_or("--save needs a file")?;
                options.save = Some(PathBuf::from(value));
            }
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
    fn watch_loads_the_cooked_data_again() {
        assert!(!parse(&[]).expect("no option").watch);
        assert!(parse(&["--watch", "--night"]).expect("valid options").watch);
    }

    #[test]
    fn new_and_save_choose_the_run_and_its_file() {
        let options = parse(&[]).expect("no option");
        assert!(!options.new && options.save.is_none());
        let options = parse(&["--new", "--save", "run.toml"]).expect("valid options");
        assert!(options.new);
        assert_eq!(options.save, Some(PathBuf::from("run.toml")));
        assert!(parse(&["--save"]).is_err());
    }

    #[test]
    fn lose_device_at_takes_a_frame() {
        assert_eq!(parse(&[]).expect("no option").lose_device_at, None);
        let options = parse(&["--lose-device-at", "300"]).expect("a frame");
        assert_eq!(options.lose_device_at, Some(300));
        assert!(parse(&["--lose-device-at"]).is_err());
        assert!(parse(&["--lose-device-at", "soon"]).is_err());
    }
}
