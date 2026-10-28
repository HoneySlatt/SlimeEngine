//! Play: the game that the editor plays (`docs/adr/0164-play.md`), built by cargo as it starts and
//! again as its code changes (`docs/adr/0166-building-the-game-again.md`), started through
//! `slime_channel::launch_unless` and driven over the development channel by a thread of its own,
//! its log read from its standard error by another, so that a game that is slow to build or to
//! answer, hangs or crashes never holds the editor's window, and a game that does not stop when
//! asked is killed.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use slime_app::Waker;
use slime_channel::{Client, Reply, Request, launch_unless};

use crate::program::{Build, Played};

/// How long the game may take to say where it listens: a large zone loads before its first frame.
const START_TIMEOUT: Duration = Duration::from_secs(120);

/// How long the game may take to end once it is asked to stop, before it is killed.
pub(crate) const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the thread looks whether the game ended, and asks how its time stands, while nothing
/// is asked.
const POLL: Duration = Duration::from_millis(200);

/// How often the thread looks whether the game's sources changed, while the game runs.
const WATCH: Duration = Duration::from_secs(1);

/// What the editor asks of the game that it plays.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Control {
    /// The texts of the commands that steps applied while the game runs, for each step in their
    /// order, sent one step at a time, so that a step that the game refuses holds up no other.
    Apply(Vec<Vec<String>>),
    /// Game time stands still, or runs again.
    Pause(bool),
    /// Game time paused, then one fixed step run.
    Step,
    /// Game time runs at this speed, a finite number above zero.
    Speed(f64),
    /// The game asked to stop, and killed if it does not within [`STOP_TIMEOUT`]; a build under way
    /// stopped.
    Stop,
    /// The level written again into Play's directory, as the editor holds it, once the game built
    /// again did not save its run: the new build starts a new run on it.
    Written,
}

/// What Play's threads tell the editor of the game that it plays.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Event {
    /// cargo builds the game's program.
    Building,
    /// cargo built it, or found nothing to build.
    Built,
    /// cargo could not build it: what it wrote on its standard error, its errors among them, or
    /// why it did not run. As Play starts, Play ends; while the game runs, it plays on.
    BuildFailed(Vec<String>),
    /// The game, built again, stopped, and its new build starts, resuming the run that it saved;
    /// or, when it did not save it, why, and the new build waits for the level written again,
    /// [`Control::Written`], to start a new run on it.
    Restarting { unsaved: Option<String> },
    /// The game listens, and answers: where, and its token, which a tool connects to it with.
    Started { address: String, token: String },
    /// Game time as the game says it stands, after a control and once another tool changed it:
    /// paused or not, and its speed.
    Time { paused: bool, speed: f64 },
    /// Lines of the game's log, the oldest first, as it wrote them to its standard error.
    Log(Vec<String>),
    /// The game refused a control, or did not answer it: why.
    Refused(String),
    /// The game ended, or could not start: how, and whether as it should, stopped or quit with
    /// success.
    Ended { how: String, cleanly: bool },
}

/// What Play's thread needs to start the game, and to start it again as its code changes.
pub(crate) struct Game {
    /// The command that starts it, [`Program::play`](crate::Program::play).
    pub(crate) play: fn(&Played<'_>) -> Command,
    /// The cooked data that the editor reads.
    pub(crate) assets: PathBuf,
    /// Whether the editor runs without a window.
    pub(crate) headless: bool,
    /// How cargo builds the game, [`Program::build`](crate::Program::build).
    pub(crate) build: Option<Build>,
}

/// The game's process, once it said where it listens, which a stop kills when it does not end in
/// time, and the player dropped at once.
type Process = Arc<Mutex<Option<Child>>>;

/// The game that the editor plays, and the threads that drive it: dropped, it kills the game, if
/// it still runs, and removes Play's directory.
#[derive(Debug)]
pub(crate) struct Player {
    controls: Sender<Control>,
    events: Receiver<Event>,
    process: Process,
    directory: PathBuf,
}

impl Player {
    /// Builds `game`, if it says how, then starts it on Play's `directory`, its private copies and
    /// save, on a thread of its own, which drives it and builds it again as its code changes, its
    /// standard error read as its log by another; each event that they tell wakes the editor
    /// through `waker`. `directory` is removed as the player is dropped.
    ///
    /// # Panics
    ///
    /// If the system refuses a new thread.
    pub(crate) fn start(game: Game, directory: PathBuf, waker: Waker) -> Self {
        let (controls, asked) = mpsc::channel();
        let (told, events) = mpsc::channel();
        let process: Process = Arc::new(Mutex::new(None));
        let tell = move |event: Event| {
            // The editor gone, nothing is told.
            let _ = told.send(event);
            waker.wake();
        };
        let shared = Arc::clone(&process);
        let played = directory.clone();
        thread::Builder::new()
            .name("play".to_owned())
            .spawn(move || drive(&game, &played, &asked, &shared, &tell))
            .expect("a thread can be started");
        Self {
            controls,
            events,
            process,
            directory,
        }
    }

    /// Asks `control` of the game; nothing once the game ended. A stop kills the game
    /// [`STOP_TIMEOUT`] later if it has not ended by then, whatever the thread that drives it
    /// waits on.
    pub(crate) fn control(&self, control: Control) {
        let stop = control == Control::Stop;
        let _ = self.controls.send(control);
        if stop {
            let process = Arc::clone(&self.process);
            let _ = thread::Builder::new()
                .name("play stop".to_owned())
                .spawn(move || {
                    thread::sleep(STOP_TIMEOUT);
                    kill(&process);
                });
        }
    }

    /// What the threads told since the last call, the oldest first.
    pub(crate) fn events(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    /// Play's directory, which holds the level's private copy and the save.
    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }
}

impl Drop for Player {
    /// The game killed, if it still runs, as the editor that plays it quits, and Play's directory
    /// removed.
    fn drop(&mut self) {
        kill(&self.process);
        let _ = fs::remove_dir_all(&self.directory);
    }
}

/// Kills the game of `process` if it still runs.
fn kill(process: &Process) {
    let mut process = process.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(child) = process.as_mut()
        && matches!(child.try_wait(), Ok(None))
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// How the game of `process` ended, if it did.
fn status(process: &Process) -> Option<std::process::ExitStatus> {
    process
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .and_then(|child| child.try_wait().ok().flatten())
}

/// Builds `game`, starts it on Play's `directory`, then drives it until it ends, building it again
/// and starting it again as its code changes: `asked` gives the controls, `tell` tells the events,
/// `process` holds the game once it listens. The editor gone, which drops the controls' sender,
/// ends the build, the start or the drive, the player's drop killing the game.
fn drive(
    game: &Game,
    directory: &Path,
    asked: &Receiver<Control>,
    process: &Process,
    tell: &(impl Fn(Event) + Clone + Send + 'static),
) {
    // The controls asked while the game builds or starts, but a stop, kept for the drive: a step
    // sent while the game built again starts again reaches the new game.
    let pending = RefCell::new(VecDeque::new());
    // A stop asked, or the editor gone, while the game builds or starts gives it up.
    let stopped = || {
        loop {
            match asked.try_recv() {
                Ok(Control::Stop) | Err(TryRecvError::Disconnected) => return true,
                Ok(control) => pending.borrow_mut().push_back(control),
                Err(TryRecvError::Empty) => return false,
            }
        }
    };
    // The program that cargo built, and its sources, which the thread watches.
    let mut built = None;
    let mut sources = None;
    if let Some(build) = &game.build {
        tell(Event::Building);
        let mut cargo = match Cargo::start(build) {
            Ok(cargo) => cargo,
            Err(error) => {
                tell(Event::BuildFailed(vec![format!(
                    "cargo did not run: {error}"
                )]));
                tell(Event::Ended {
                    how: "the game could not be built".to_owned(),
                    cleanly: false,
                });
                return;
            }
        };
        let outcome = loop {
            if let Some(outcome) = cargo.finished() {
                break outcome;
            }
            if stopped() {
                tell(Event::Ended {
                    how: "Play was stopped while the game was built".to_owned(),
                    cleanly: true,
                });
                return;
            }
            thread::sleep(Duration::from_millis(20));
        };
        match outcome {
            Ok(program) => {
                tell(Event::Built);
                sources = Some(Sources::of(&program, cargo.since));
                built = Some(program);
            }
            Err(lines) => {
                tell(Event::BuildFailed(lines));
                tell(Event::Ended {
                    how: "the game's code does not build".to_owned(),
                    cleanly: false,
                });
                return;
            }
        }
    }
    let mut starts = 1;
    // When the program that runs was last written: a build that leaves another starts it again.
    let mut running = built.as_deref().and_then(modified);
    let started = start(
        game,
        directory,
        built.as_deref(),
        starts,
        false,
        &stopped,
        process,
        tell,
    );
    let mut client = match started {
        Ok(client) => client,
        Err(ended) => {
            tell(ended);
            return;
        }
    };
    // Game time as last told: running at its speed, as the game starts.
    let mut time = (false, 1.0);
    // The build under way, and when the sources were last looked at.
    let mut cargo: Option<Cargo> = None;
    let mut looked = Instant::now();
    loop {
        let waiting = pending.borrow_mut().pop_front();
        let control = match waiting.map_or_else(|| asked.recv_timeout(POLL), Ok) {
            Ok(control) => Some(control),
            Err(RecvTimeoutError::Timeout) => None,
            // The build under way, dropped, is stopped.
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let requests = match &control {
            None | Some(Control::Written) => Vec::new(),
            Some(Control::Apply(steps)) => steps
                .iter()
                .map(|commands| Request::Apply {
                    commands: commands.clone(),
                })
                .collect(),
            Some(Control::Pause(true)) => vec![Request::Pause],
            Some(Control::Pause(false)) => vec![Request::Resume],
            Some(Control::Step) => vec![Request::Pause, Request::Step { steps: 1 }],
            Some(Control::Speed(speed)) => vec![Request::Speed { speed: *speed }],
            Some(Control::Stop) => vec![Request::Stop],
        };
        for request in &requests {
            match client.request(request) {
                Ok((Reply::Failed { message }, _)) => tell(Event::Refused(message)),
                Ok(_) => {}
                Err(error) => tell(Event::Refused(format!("the game does not answer: {error}"))),
            }
        }
        if control == Some(Control::Stop) {
            drop(cargo.take());
            // Ended by itself, or killed by the stop's watch.
            let wait = STOP_TIMEOUT + Duration::from_secs(1);
            tell(end(process, "stopped", wait, || Event::Ended {
                how: "the game did not stop: it was killed".to_owned(),
                cleanly: false,
            }));
            return;
        }
        // The build under way, once it ended: a program other than the one that runs starts again
        // on a save of the run; a build that failed leaves the game playing.
        if let Some(outcome) = cargo.as_mut().and_then(Cargo::finished) {
            let since = cargo
                .take()
                .map_or_else(SystemTime::now, |cargo| cargo.since);
            match outcome {
                Ok(program) => {
                    tell(Event::Built);
                    sources = Some(Sources::of(&program, since));
                    let again = modified(&program) != running || Some(&program) != built.as_ref();
                    if again {
                        built = Some(program);
                        let unsaved = save(&mut client, directory).err();
                        tell(Event::Restarting {
                            unsaved: unsaved.clone(),
                        });
                        let _ = client.request(&Request::Stop);
                        let wait = STOP_TIMEOUT + Duration::from_secs(1);
                        let _ = end(process, "stopped", wait, || Event::Refused(String::new()));
                        let _ = fs::remove_dir_all(copies(directory, starts));
                        // A new run starts on the level as the editor holds it, which the editor
                        // writes again, the steps sent meanwhile in it.
                        if unsaved.is_some() && !written(asked, &pending) {
                            tell(Event::Ended {
                                how: "Play was stopped while the game was built again".to_owned(),
                                cleanly: true,
                            });
                            return;
                        }
                        starts += 1;
                        running = built.as_deref().and_then(modified);
                        let resume = unsaved.is_none();
                        let started = start(
                            game,
                            directory,
                            built.as_deref(),
                            starts,
                            resume,
                            &stopped,
                            process,
                            tell,
                        );
                        client = match started {
                            Ok(client) => client,
                            Err(ended) => {
                                tell(ended);
                                return;
                            }
                        };
                        // Game time as it stood, which the new game is told, and the toolbar
                        // once the game answers how it stands.
                        if time.0 {
                            let _ = client.request(&Request::Pause);
                        }
                        if time.1 != 1.0 {
                            let _ = client.request(&Request::Speed { speed: time.1 });
                        }
                        time = (false, 1.0);
                    }
                }
                Err(lines) => {
                    if let Some(sources) = &mut sources {
                        sources.since = since;
                    }
                    tell(Event::BuildFailed(lines));
                }
            }
        } else if cargo.is_none() && looked.elapsed() >= WATCH {
            looked = Instant::now();
            if let (Some(build), Some(watched)) = (&game.build, &mut sources)
                && watched.changed()
            {
                match Cargo::start(build) {
                    Ok(started) => {
                        tell(Event::Building);
                        cargo = Some(started);
                    }
                    Err(error) => {
                        watched.since = SystemTime::now();
                        tell(Event::BuildFailed(vec![format!(
                            "cargo did not run: {error}"
                        )]));
                    }
                }
            }
        }
        // Game time as the game says it stands: after a control, which the game may have
        // refused, and as another tool changes it.
        if let Ok((Reply::State { state }, _)) = client.request(&Request::State)
            && (control.is_some() || time != (state.paused, state.speed))
        {
            time = (state.paused, state.speed);
            tell(Event::Time {
                paused: state.paused,
                speed: state.speed,
            });
        }
        if let Some(status) = status(process) {
            tell(Event::Ended {
                how: format!("the game ended: {status}"),
                cleanly: status.success(),
            });
            return;
        }
    }
}

/// Waits until the editor wrote the level again, [`Control::Written`]: `false` once Stop is asked,
/// or the editor is gone. The steps sent before, which the level written holds, are dropped; the
/// other controls are kept in `pending`.
fn written(asked: &Receiver<Control>, pending: &RefCell<VecDeque<Control>>) -> bool {
    pending
        .borrow_mut()
        .retain(|control| !matches!(control, Control::Apply(_)));
    loop {
        match asked.recv() {
            Ok(Control::Written) => return true,
            Ok(Control::Stop) | Err(_) => return false,
            Ok(Control::Apply(_)) => {}
            Ok(control) => pending.borrow_mut().push_back(control),
        }
    }
}

/// When `file` was last written, if the system says.
fn modified(file: &Path) -> Option<SystemTime> {
    fs::metadata(file)
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// Asks the game of `client` to save its run into Play's save, in `directory`: why it did not, if it
/// did not.
fn save(client: &mut Client, directory: &Path) -> Result<(), String> {
    let file = directory.join("save.toml");
    let file = file
        .to_str()
        .ok_or("the save's path is not text")?
        .to_owned();
    match client.request(&Request::Save { file }) {
        Ok((Reply::Done, _)) => Ok(()),
        Ok((Reply::Failed { message }, _)) => Err(message),
        Ok(_) => Err("the game answered as to another request".to_owned()),
        Err(error) => Err(format!("the game does not answer: {error}")),
    }
}

/// Starts `game` on Play's `directory`, the `starts`-th time, its program the one that cargo
/// built, `built`, if any, resuming Play's save or not, as `slime_channel::launch_unless` does,
/// given up once `stopped` says so; its log read on a thread of its own, the process kept in
/// `process`. Gives a client connected to it, or the event of its end.
#[expect(
    clippy::too_many_arguments,
    reason = "what a start needs, from the one function that starts the game"
)]
fn start(
    game: &Game,
    directory: &Path,
    built: Option<&Path>,
    starts: u32,
    resume: bool,
    stopped: &impl Fn() -> bool,
    process: &Process,
    tell: &(impl Fn(Event) + Clone + Send + 'static),
) -> Result<Client, Event> {
    let program = built
        .map(|built| started(built, directory, starts))
        .transpose()
        .map_err(|error| Event::Ended {
            how: format!("the game's program could not be copied: {error}"),
            cleanly: false,
        })?;
    let save = directory.join("save.toml");
    let played = Played {
        assets: &game.assets,
        save: &save,
        resume,
        headless: game.headless,
        built: program.as_deref(),
    };
    let mut command = (game.play)(&played);
    command
        .env(slime_world::PLAY_SCENES_ENV_VAR, directory)
        .stderr(Stdio::piped());
    no_console(&mut command);
    let mut launched =
        launch_unless(&mut command, START_TIMEOUT, stopped).map_err(|error| Event::Ended {
            how: format!("the game did not start: {error}"),
            cleanly: false,
        })?;
    if let Some(stderr) = launched.child.stderr.take() {
        let tell = tell.clone();
        let _ = thread::Builder::new()
            .name("play log".to_owned())
            .spawn(move || read_log(stderr, &tell));
    }
    let (address, token) = (launched.address, launched.token.clone());
    *process.lock().unwrap_or_else(PoisonError::into_inner) = Some(launched.child);
    let client = Client::connect(address, &token).map_err(|error| {
        // A game that crashed as it started, or one that does not answer.
        end(process, "ended", Duration::from_secs(1), || Event::Ended {
            how: format!("the game does not answer: {error}"),
            cleanly: false,
        })
    })?;
    tell(Event::Started {
        address: address.to_string(),
        token: token.to_string(),
    });
    Ok(client)
}

/// The program to start for the one that cargo built, `built`: itself, but on Windows, where a copy
/// of it, of the same name, and of the libraries beside it, the shader compiler's among them, in a
/// directory of Play's `directory` of the start's count, `starts`, leaves the one built free for
/// the next build to replace, which Windows forbids while it runs.
fn started(built: &Path, directory: &Path, starts: u32) -> io::Result<PathBuf> {
    if !cfg!(windows) {
        return Ok(built.to_path_buf());
    }
    let copies = copies(directory, starts);
    fs::create_dir_all(&copies)?;
    let name = built.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let copy = copies.join(name);
    fs::copy(built, &copy)?;
    if let Some(beside) = built.parent() {
        for entry in fs::read_dir(beside)? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
            {
                fs::copy(&path, copies.join(path.file_name().unwrap_or_default()))?;
            }
        }
    }
    Ok(copy)
}

/// The directory of Play's `directory` that holds the copies of the `starts`-th start, on Windows.
fn copies(directory: &Path, starts: u32) -> PathBuf {
    directory.join(format!("program-{starts}"))
}

/// Starts `command` without a console window on Windows; nothing elsewhere.
fn no_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW`, of the flags with which Windows creates a process.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// How the game of `process` ended, waiting at most `wait` for it to end by itself, its status
/// told then after `verb`, `ended` or `stopped`; `killed` otherwise, the game killed.
fn end(process: &Process, verb: &str, wait: Duration, killed: impl Fn() -> Event) -> Event {
    let until = Instant::now() + wait;
    loop {
        if let Some(status) = status(process) {
            return Event::Ended {
                how: format!("the game {verb}: {status}"),
                cleanly: status.success(),
            };
        }
        if Instant::now() >= until {
            kill(process);
            return killed();
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Tells the lines that the game writes to its standard error, its log, in batches of those that
/// come together, until it closes it.
fn read_log(stderr: impl std::io::Read + Send + 'static, tell: &impl Fn(Event)) {
    let (lines, batch) = mpsc::channel::<String>();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                return;
            }
        }
    });
    while let Ok(first) = batch.recv() {
        let mut lines = vec![first];
        lines.extend(batch.try_iter());
        tell(Event::Log(lines));
    }
    let _ = reader.join();
}

/// A build of the game's program under way: cargo, which is stopped if it is dropped before it
/// ends, and the threads that read what it writes.
struct Cargo {
    child: Child,
    /// When it started: a source changed since, while it built, is built again.
    since: SystemTime,
    /// The program that its messages say it built.
    program: Option<JoinHandle<Option<PathBuf>>>,
    /// The lines that it writes on its standard error, its errors among them.
    errors: Option<JoinHandle<Vec<String>>>,
}

impl Cargo {
    /// Starts cargo on `build`, in the profile that the editor's own program was built in, its
    /// messages in JSON.
    ///
    /// # Errors
    ///
    /// When cargo cannot be started.
    fn start(build: &Build) -> io::Result<Self> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
        command
            .args([
                "build",
                "--quiet",
                "--message-format=json-render-diagnostics",
            ])
            .args(std::env::current_exe().map_or_else(|_| Vec::new(), |own| profile(&own)))
            .args(build.arguments)
            .current_dir(&build.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        no_console(&mut command);
        let since = SystemTime::now();
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().map(BufReader::new);
        let stderr = child.stderr.take().map(BufReader::new);
        let program = thread::spawn(move || {
            stdout?
                .lines()
                .map_while(Result::ok)
                .filter_map(|line| executable(&line))
                .last()
        });
        let errors = thread::spawn(move || {
            stderr.map_or_else(Vec::new, |stderr| {
                stderr.lines().map_while(Result::ok).collect()
            })
        });
        Ok(Self {
            child,
            since,
            program: Some(program),
            errors: Some(errors),
        })
    }

    /// What the build gave once cargo ended, none while it runs: the program built, or the lines
    /// that cargo wrote on its standard error, its errors among them.
    fn finished(&mut self) -> Option<Result<PathBuf, Vec<String>>> {
        let status = match self.child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => return None,
            Err(error) => {
                return Some(Err(vec![format!("cargo could not be waited for: {error}")]));
            }
        };
        let program = self.program.take()?.join().ok().flatten();
        let errors = self.errors.take()?.join().unwrap_or_default();
        Some(match program {
            Some(program) if status.success() => Ok(program),
            _ if status.success() => Err(vec![
                "cargo built no program: the build's arguments name none".to_owned(),
            ]),
            _ if errors.is_empty() => Err(vec![format!("cargo failed: {status}")]),
            _ => Err(errors),
        })
    }
}

impl Drop for Cargo {
    /// cargo stopped if it still builds.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// The arguments that make cargo build in the profile that the editor's own program, `own`, was
/// built in, which names the directory that holds it, or that holds its directory of examples or,
/// for a test program, of dependencies: none for `debug`, cargo's default.
fn profile(own: &Path) -> Vec<String> {
    let mut directory = own.parent();
    if matches!(
        directory
            .and_then(Path::file_name)
            .and_then(|name| name.to_str()),
        Some("examples" | "deps")
    ) {
        directory = directory.and_then(Path::parent);
    }
    match directory
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    {
        None | Some("debug") => Vec::new(),
        Some("release") => vec!["--release".to_owned()],
        Some(profile) => vec!["--profile".to_owned(), profile.to_owned()],
    }
}

/// The program that a message of cargo, `line`, in JSON, says it built: none for a message of
/// another kind, or for what is not a program, a library.
fn executable(line: &str) -> Option<PathBuf> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    if message["reason"] != "compiler-artifact" {
        return None;
    }
    message["executable"].as_str().map(PathBuf::from)
}

/// The sources of the game's program, as the file of its dependencies that cargo writes beside it
/// lists them, and when the build that made it started: a source changed since is built again.
#[derive(Debug)]
struct Sources {
    files: Vec<PathBuf>,
    since: SystemTime,
}

impl Sources {
    /// The sources of `program`, made by a build that started at `since`, and the manifests and
    /// locks of cargo above them, which the file of its dependencies does not list: none when that
    /// file cannot be read.
    fn of(program: &Path, since: SystemTime) -> Self {
        let mut files = fs::read_to_string(program.with_extension("d"))
            .map(|text| listed(&text))
            .unwrap_or_default();
        let directories: BTreeSet<&Path> = files
            .iter()
            .flat_map(|file| file.ancestors().skip(1))
            .collect();
        let manifests: Vec<PathBuf> = directories
            .into_iter()
            .flat_map(|directory| ["Cargo.toml", "Cargo.lock"].map(|name| directory.join(name)))
            .filter(|file| file.is_file())
            .collect();
        files.extend(manifests);
        Self { files, since }
    }

    /// Whether a source changed since the build started; a source that is gone is not counted, so
    /// that a build that fails for it is not run again until another source changes.
    fn changed(&self) -> bool {
        self.files.iter().any(|file| {
            fs::metadata(file)
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| modified > self.since)
        })
    }
}

/// The files that the file of dependencies `text` lists after each target, `target: a b c`, in
/// make's notation, a space in a path written `\ `: a target's colon is followed by a space, where
/// a drive's on Windows is not.
fn listed(text: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let Some((_, list)) = line.split_once(": ") else {
            continue;
        };
        let mut file = String::new();
        let mut characters = list.chars().peekable();
        while let Some(character) = characters.next() {
            match character {
                '\\' if characters.peek() == Some(&' ') => {
                    file.push(' ');
                    characters.next();
                }
                ' ' => {
                    if !file.is_empty() {
                        files.push(PathBuf::from(std::mem::take(&mut file)));
                    }
                }
                character => file.push(character),
            }
        }
        if !file.is_empty() {
            files.push(PathBuf::from(file));
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_of_dependencies_are_read_as_cargo_writes_them() {
        let text = "/w/target/debug/world: /w/examples/world.rs /w/crates/a\\ b/src/lib.rs \
                    /w/README.md\n";
        assert_eq!(
            listed(text),
            [
                PathBuf::from("/w/examples/world.rs"),
                PathBuf::from("/w/crates/a b/src/lib.rs"),
                PathBuf::from("/w/README.md"),
            ]
        );
        // Windows: a drive's colon, and backslashes that separate the parts of a path.
        let text = "C:\\w\\target\\debug\\game.exe: C:\\w\\src\\main.rs C:\\w\\my\\ game\\lib.rs\n";
        assert_eq!(
            listed(text),
            [
                PathBuf::from("C:\\w\\src\\main.rs"),
                PathBuf::from("C:\\w\\my game\\lib.rs"),
            ]
        );
        assert!(listed("# env-dep:A=1\n").is_empty());
    }

    #[test]
    fn cargo_builds_in_the_profile_of_the_editor_s_program() {
        let profile = |path: &str| profile(Path::new(path));
        assert!(profile("/w/target/debug/hunnawood-editor").is_empty());
        assert!(profile("/w/target/debug/examples/editor").is_empty());
        assert!(profile("/w/target/debug/deps/slime_editor-0123").is_empty());
        assert_eq!(profile("/w/target/release/examples/editor"), ["--release"]);
        assert_eq!(profile("/w/target/ci/editor"), ["--profile", "ci"]);
    }

    #[test]
    fn cargo_s_messages_give_the_program_built() {
        let program = r#"{"reason":"compiler-artifact","target":{"name":"world"},"executable":"/w/target/debug/examples/world","fresh":false}"#;
        assert_eq!(
            executable(program),
            Some(PathBuf::from("/w/target/debug/examples/world"))
        );
        let library = r#"{"reason":"compiler-artifact","target":{"name":"slime_app"},"executable":null,"fresh":true}"#;
        assert_eq!(executable(library), None);
        assert_eq!(
            executable(r#"{"reason":"build-finished","success":true}"#),
            None
        );
        assert_eq!(executable("not a message"), None);
    }

    #[test]
    fn a_source_changed_after_the_build_started_is_seen() {
        let directory =
            std::env::temp_dir().join(format!("slime-editor-sources-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a directory of the test's own");
        let source = directory.join("main.rs");
        fs::write(&source, "fn main() {}\n").expect("written");
        let program = directory.join("game");
        let gone = directory.join("gone.rs");
        fs::write(
            program.with_extension("d"),
            format!(
                "{}: {} {}\n",
                program.display(),
                source.display(),
                gone.display()
            ),
        )
        .expect("written");
        let later = SystemTime::now() + Duration::from_secs(60);
        let sources = Sources::of(&program, later);
        assert_eq!(sources.files, [source.clone(), gone]);
        assert!(
            !sources.changed(),
            "nothing changed since; a source gone not counted"
        );
        assert!(Sources::of(&program, SystemTime::UNIX_EPOCH).changed());
        assert!(
            Sources::of(&directory.join("other"), later)
                .files
                .is_empty()
        );
        let _ = fs::remove_dir_all(&directory);
    }
}
