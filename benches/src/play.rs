//! Play's time to control (`docs/adr/0110-how-the-editor-runs-a-games-code.md`,
//! `docs/adr/0166-building-the-game-again.md`): an editor's program started as agents start it,
//! without its window, on a copy of a level and with a journal, data and state of the run's own, Play
//! asked through the development channel, and the time read from the editor's log, which Play
//! measures from the request to the first state of the game whose time it no longer holds.
//! The editor's log, the game's in it, is kept in the run's directory, `editor.log`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use slime_channel::{Client, Edit, Reply, Request, launch};

use crate::measure::{Context, Row};

/// How many times Play is measured, after a first one, which may build the game and fills the
/// system's caches.
const RUNS: usize = 5;
/// How long the first Play may take to give control, the game built first.
const FIRST: Duration = Duration::from_secs(1800);
/// How long any other may take, to give control or to stop.
const LIMIT: Duration = Duration::from_secs(300);

/// Play on *Hunna Wood*'s zone, cooked in `target/cooked/hunnawood`, from its editor, which must be
/// built beside this program, in a window, which the game needs.
pub fn zone(_: &mut Context) -> Result<Vec<Row>, String> {
    let root = Context::root();
    let cooked = root.join("target/cooked/hunnawood");
    if !cooked.join(slime_assets::MANIFEST_FILE).is_file() {
        return Err(format!(
            "{} holds no cooked zone: fetch it with `scripts/fetch-assets.sh zone/`, then cook it \
             with `cargo run --release -p slime-cook -- cook games/hunnawood/assets/zone \
             target/cooked/hunnawood`",
            cooked.display()
        ));
    }
    let display = ["WAYLAND_DISPLAY", "DISPLAY"]
        .iter()
        .any(|name| std::env::var_os(name).is_some());
    if !cfg!(windows) && !display {
        return Err(
            "Hunna Wood runs in a window only: run this benchmark where a compositor draws with the \
             GPU, a desktop's (benches/README.md)"
                .to_owned(),
        );
    }
    let directory = fresh("play-zone")?;
    let level = copy(
        &root.join("games/hunnawood/assets/zone/zone.scene"),
        &directory,
    )?;
    measure(
        "hunnawood-editor",
        &level,
        &cooked,
        &directory,
        "the zone",
        "`cargo build --release -p hunnawood-editor`",
    )
}

/// How many entities [`level`] adds to the yard, under [`PARENTS`] parents.
const ENTITIES: usize = 100_000;
/// How many parents they are under.
const PARENTS: usize = 1_000;

/// Play on a level of [`ENTITIES`] entities, the examples' yard with them added, none drawn, its
/// [`PARENTS`] parents with a name each and the others with their parent alone, so that the file
/// holds fewer values than a document may (`slime_serialize::MAX_VALUES`), from the `editor`
/// example, which must be built beside this program, the `world` example playing it without a
/// window.
pub fn level(_: &mut Context) -> Result<Vec<Row>, String> {
    let root = Context::root();
    let directory = fresh("play-level")?;
    let yard = root.join("examples/assets/yard.scene");
    let level = copy(&yard, &directory)?;
    let text = fs::read_to_string(&level).map_err(|error| error.to_string())?;
    let mut entities = String::from("[entities]\n");
    for parent in 0..PARENTS {
        let _ = writeln!(
            entities,
            "\"group {parent}\" = {{ components = {{ Name = \"group {parent}\" }} }}"
        );
        for child in 0..ENTITIES / PARENTS - 1 {
            let _ = writeln!(
                entities,
                "\"item {parent}-{child}\" = {{ parent = \"group {parent}\" }}"
            );
        }
    }
    let text = text.replacen("[entities]\n", &entities, 1);
    fs::write(&level, text).map_err(|error| error.to_string())?;
    measure(
        "examples/editor",
        &level,
        &root.join("target/cooked/examples"),
        &directory,
        "a level of 100 000 entities",
        "`cargo build --release --example editor`",
    )
}

/// `target/benches/<name>`, emptied, with the data and state directories that the editor and the
/// game it plays use in place of the user's.
fn fresh(name: &str) -> Result<PathBuf, String> {
    let directory = Context::root().join("target/benches").join(name);
    let _ = fs::remove_dir_all(&directory);
    for inside in ["data", "state"] {
        fs::create_dir_all(directory.join(inside)).map_err(|error| error.to_string())?;
    }
    Ok(directory)
}

/// Copies the scene file `level`, with its metadata file, into `directory`: the copy's path.
fn copy(level: &Path, directory: &Path) -> Result<PathBuf, String> {
    let name = level.file_name().ok_or("a level is a file")?;
    let copy = directory.join(name);
    fs::copy(level, &copy).map_err(|error| format!("{}: {error}", level.display()))?;
    let mut meta = level.as_os_str().to_owned();
    meta.push(".meta");
    let mut copied = copy.as_os_str().to_owned();
    copied.push(".meta");
    fs::copy(&meta, &copied).map_err(|error| error.to_string())?;
    Ok(copy)
}

/// Starts the editor's program `program`, beside this program, on `level` and the cooked data
/// `assets`, without its window, and Play [`RUNS`] times after a first: the rows of the times to
/// control, and of their part building the game, `what` naming the level, `built` saying how the
/// program is built.
fn measure(
    program: &str,
    level: &Path,
    assets: &Path,
    directory: &Path,
    what: &str,
    built: &str,
) -> Result<Vec<Row>, String> {
    let own = std::env::current_exe().map_err(|error| error.to_string())?;
    let editor = own
        .parent()
        .ok_or("this program's directory")?
        .join(format!("{program}{}", std::env::consts::EXE_SUFFIX));
    if !editor.is_file() {
        return Err(format!("{} is not built: {built} first", editor.display()));
    }
    // The editor's log, the game's in it, kept for what the figures do not say.
    let log = fs::File::create(directory.join("editor.log")).map_err(|error| error.to_string())?;
    let mut command = Command::new(&editor);
    command
        .arg("--level")
        .arg(level)
        .arg("--assets")
        .arg(assets)
        .arg("--journal")
        .arg(directory.join("journal"))
        .arg("--headless")
        .env("XDG_DATA_HOME", directory.join("data"))
        .env("XDG_STATE_HOME", directory.join("state"))
        .env("APPDATA", directory.join("data"))
        .env("LOCALAPPDATA", directory.join("state"))
        .stderr(log);
    let mut launched =
        launch(&mut command, Duration::from_secs(120)).map_err(|error| error.to_string())?;
    let measured = Client::connect(launched.address, &launched.token)
        .map_err(|error| error.to_string())
        .and_then(|mut client| {
            let measured = plays(&mut client);
            let _ = client.request(&Request::Stop);
            measured
        });
    let ended = Instant::now();
    while matches!(launched.child.try_wait(), Ok(None)) {
        if ended.elapsed() > LIMIT {
            let _ = launched.child.kill();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let [after, writing, building] = measured?;
    let mut rows = Vec::new();
    for (part, times) in [
        ("from the request to control", after),
        ("the editor writing the level, of it", writing),
        ("cargo building the game, of it", building),
    ] {
        rows.extend(Row::times(&format!("Play on {what}: {part}"), &times));
    }
    Ok(rows)
}

/// Plays [`RUNS`] times and once before, through the editor that `client` drives: the times to
/// control, and their parts writing the level and building the game, of the runs after the first.
fn plays(client: &mut Client) -> Result<[Vec<Duration>; 3], String> {
    let mut read = 0;
    let mut times = [Vec::new(), Vec::new(), Vec::new()];
    for run in 0..=RUNS {
        operation(client, "play.start")?;
        let limit = if run == 0 { FIRST } else { LIMIT };
        let measured = until(client, &mut read, limit, control)?;
        if run > 0 {
            for (series, time) in times.iter_mut().zip(measured) {
                series.push(time);
            }
        }
        operation(client, "play.stop")?;
        until(client, &mut read, LIMIT, |line| {
            line.contains("Play: the game stopped").then_some(())
        })?;
    }
    Ok(times)
}

/// Runs the editor's operation `name` through `client`.
fn operation(client: &mut Client, name: &str) -> Result<(), String> {
    let request = Request::Edit {
        edit: Edit::Run {
            name: name.to_owned(),
            arguments: Vec::new(),
        },
    };
    match client.request(&request) {
        Ok((Reply::Done, _)) => Ok(()),
        Ok((Reply::Failed { message }, _)) => Err(format!("{name}: {message}")),
        Ok((reply, _)) => Err(format!("{name}: {reply:?}")),
        Err(error) => Err(format!("{name}: {error}")),
    }
}

/// The times that a line of the editor's log gives, when Play says that the game gave control:
/// from the request, and of it writing the level and building the game, in seconds.
fn control(line: &str) -> Option<[Duration; 3]> {
    let rest = line.split("Play: the game gave control ").nth(1)?;
    let seconds: Vec<Duration> = rest
        .split(' ')
        .filter_map(|word| word.parse::<f64>().ok())
        .map(Duration::from_secs_f64)
        .collect();
    seconds.try_into().ok()
}

/// Reads the editor's log through `client`, after the line `read`, until `found` gives something
/// of a line, within `limit`; a line that says that Play ended otherwise fails.
fn until<T>(
    client: &mut Client,
    read: &mut u64,
    limit: Duration,
    found: impl Fn(&str) -> Option<T>,
) -> Result<T, String> {
    let started = Instant::now();
    loop {
        let Ok((Reply::Logs { lines }, _)) = client.request(&Request::Logs { after: *read }) else {
            return Err("the editor did not give its log".to_owned());
        };
        for line in lines {
            *read = line.number;
            if let Some(value) = found(&line.text) {
                return Ok(value);
            }
            let failed = [
                "does not answer",
                "could not be copied",
                "did not stop",
                "does not build",
                "did not start",
                "could not be built",
                "game ended",
            ];
            if line.text.contains("Play: ") && failed.iter().any(|end| line.text.contains(end)) {
                return Err(line.text);
            }
        }
        if started.elapsed() > limit {
            return Err(format!("nothing within {limit:?}"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editor_s_line_gives_the_times() {
        let line = "INFO slime_editor::session: Play: the game gave control 2.125 s after Play \
                    was asked, 0.300 s of it writing the level and 0.140 s building the game";
        assert_eq!(
            control(line),
            Some([2125, 300, 140].map(Duration::from_millis))
        );
        assert_eq!(control("Play: the game runs"), None);
    }
}
