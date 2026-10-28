//! Play: the game that the editor plays (`docs/adr/0164-play.md`), started through
//! `slime_channel::launch_unless` and driven over the development channel by a thread of its own,
//! its log read from its standard error by another, so that a game that is slow to answer, hangs or
//! crashes never holds the editor's window, and a game that does not stop when asked is killed.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use slime_app::Waker;
use slime_channel::{Client, Reply, Request, launch_unless};

/// How long the game may take to say where it listens: a large zone loads before its first frame.
const START_TIMEOUT: Duration = Duration::from_secs(120);

/// How long the game may take to end once it is asked to stop, before it is killed.
pub(crate) const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the thread looks whether the game ended, and asks how its time stands, while nothing
/// is asked.
const POLL: Duration = Duration::from_millis(200);

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
    /// The game asked to stop, and killed if it does not within [`STOP_TIMEOUT`].
    Stop,
}

/// What Play's threads tell the editor of the game that it plays.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Event {
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
    /// Starts the game that `command` runs, as `slime_channel::launch_unless` does, on a thread of
    /// its own, which drives it, its standard error read as its log by another; each event that
    /// they tell wakes the editor through `waker`. `directory`, Play's private copies and save, is
    /// removed as the player is dropped.
    ///
    /// # Panics
    ///
    /// If the system refuses a new thread.
    pub(crate) fn start(mut command: Command, directory: PathBuf, waker: Waker) -> Self {
        let (controls, asked) = mpsc::channel();
        let (told, events) = mpsc::channel();
        let process: Process = Arc::new(Mutex::new(None));
        command.stderr(Stdio::piped());
        let tell = move |event: Event| {
            // The editor gone, nothing is told.
            let _ = told.send(event);
            waker.wake();
        };
        let shared = Arc::clone(&process);
        thread::Builder::new()
            .name("play".to_owned())
            .spawn(move || drive(&mut command, &asked, &shared, &tell))
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

/// Starts the game, then drives it until it ends: `asked` gives the controls, `tell` tells the
/// events, `process` holds the game once it listens. The editor gone, which drops the controls'
/// sender, ends the start or the drive, the player's drop killing the game.
fn drive(
    command: &mut Command,
    asked: &Receiver<Control>,
    process: &Process,
    tell: &(impl Fn(Event) + Clone + Send + 'static),
) {
    // A stop asked, or the editor gone, while the game starts gives the start up.
    let stopped = || {
        matches!(
            asked.try_recv(),
            Ok(Control::Stop) | Err(TryRecvError::Disconnected)
        )
    };
    let mut launched = match launch_unless(command, START_TIMEOUT, stopped) {
        Ok(launched) => launched,
        Err(error) => {
            tell(Event::Ended {
                how: format!("the game did not start: {error}"),
                cleanly: false,
            });
            return;
        }
    };
    if let Some(stderr) = launched.child.stderr.take() {
        let tell = tell.clone();
        let _ = thread::Builder::new()
            .name("play log".to_owned())
            .spawn(move || read_log(stderr, &tell));
    }
    let (address, token) = (launched.address, launched.token.clone());
    *process.lock().unwrap_or_else(PoisonError::into_inner) = Some(launched.child);
    let mut client = match Client::connect(address, &token) {
        Ok(client) => client,
        Err(error) => {
            // A game that crashed as it started, or one that does not answer.
            tell(end(process, "ended", Duration::from_secs(1), || {
                Event::Ended {
                    how: format!("the game does not answer: {error}"),
                    cleanly: false,
                }
            }));
            return;
        }
    };
    tell(Event::Started {
        address: address.to_string(),
        token: token.to_string(),
    });
    // Game time as last told: running at its speed, as the game starts.
    let mut time = (false, 1.0);
    loop {
        let control = match asked.recv_timeout(POLL) {
            Ok(control) => Some(control),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let requests = match &control {
            None => Vec::new(),
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
            // Ended by itself, or killed by the stop's watch.
            let wait = STOP_TIMEOUT + Duration::from_secs(1);
            tell(end(process, "stopped", wait, || Event::Ended {
                how: "the game did not stop: it was killed".to_owned(),
                cleanly: false,
            }));
            return;
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
