//! The `application` example on the display that the environment names, a Wayland compositor or
//! an X server, as the host's layer chooses it: it starts,
//! draws and rests, none of its threads waking in 10 s (`docs/adr/0185-rest-checked-in-ci.md`);
//! where the compositor takes a person's input, the pointer put on its button *Add* hovers it,
//! which draws frames, and it rests again once the hover's transition ends; then a click in its
//! field, `s` and `a` typed, and a click on *Add*, whose row is named `sa`; a termination signal
//! then asks it to close, and it ends cleanly. Weston takes no input: there the input's part does
//! not run. It writes on its standard output `keys: typed` once the row is added, and how its
//! cursor was set, `cursor: by shape`. The series' input counts the output in logical pixels at a scale of 1: this test runs
//! at that scale.
//!
//! A second test loses the example's GPU device on purpose, then its surface, through its
//! commands on its standard input, and finds after each a new swap chain's images presented, then
//! the example at rest; it writes how long each took, `lose device: drawn again in 120 ms`, and
//! `lost: drawn again`. Two devices more lost are replaced, and the fourth within five minutes
//! ends the run with status 1, the application stopped.
//!
//! A third test, run in a compositor at another scale, finds the window drawn at the scale that
//! the compositor prefers, and the start's token of activation passed to the compositor where it
//! gives `xdg_activation_v1`, then ends it, and writes that scale, `scale: 1.5`, on its standard
//! output.
//!
//! Three more, each in a state directory of its own. The first makes the example panic in its
//! step: the run ends with status 101, the application neither stopped nor dropped, its crash
//! report written; the next start finds that run's marker, then is killed, and the start after
//! finds the killed run's, ends cleanly and counts the three runs and the two crashes. It writes
//! `crashes: reported and found`. The second makes it panic in its start, then in its stop: each
//! run ends with status 101, without another call, each with its report. The third keeps the
//! window open when asked to close: a second termination signal ends it with status 130.
//!
//! A last test runs the blank window, `examples/blank.rs`, which `scripts/host-cost.py` measures
//! the host above: it presents its first image, writes it in its trace, and ends on SIGTERM with
//! status 0.
//!
//! On X11, the first two run under Xvfb, without a window manager: the window lies at the screen's
//! top left, and the pointer reaches it through `scripts/compositor-input.py`'s XTest. The window
//! takes no key and sets no cursor's shape before M6d.7b2: the field's part does not run, and it
//! writes `keys: not run, X11's layer takes no key yet` and `cursor: the server's`. A software
//! driver presents without Present: the trace tells no image shown, and the test writes
//! `period: none, measured from the signals`.
//!
//! On Wayland, the example writes its protocol with `WAYLAND_DEBUG=client` into a file of each
//! test's own,
//! read for the images it presented and their size, the scale preferred, the pointer's events and
//! where its window lies, then written on the test's standard error, where
//! `scripts/compositors.sh` reads who draws its bar; and its trace (`UWUI_TRACE`) beside it, which
//! the first test reads for its frames, an image shown, and, where the series gives input, an input
//! to present.
//!
//! The examples are built first, `cargo build -p uwui-desktop --example application --example
//! blank`;
//! `scripts/check.sh windowed` runs this test in a headless Weston, and `scripts/compositors.sh` in
//! each compositor of the series, whose input reaches it through `scripts/compositor-input.py`'s
//! socket, which `SLIME_COMPOSITOR_INPUT` names.
//!
//! A program at rest leaves its waits for nothing: Linux counts each thread's switches off its core
//! in `/proc/<pid>/task/<tid>/status`, and its time on the processor in `stat`, which reading them
//! does not wake. Every thread counts, the drivers' with the program's own.

#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long the program is watched: the quality bar's window of rest.
const REST: Duration = Duration::from_secs(10);
/// How long it must stay still before it is watched.
const STILL: Duration = Duration::from_secs(1);
/// How long it may take to come to rest: its first frames, on a GPU of the CPU.
const SETTLE: Duration = Duration::from_secs(60);

/// Each thread's switches and time on the processor, by its id.
type Threads = BTreeMap<u32, (u64, u64)>;

/// The display that the example opens its window on, as the host's layer chooses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Display {
    Wayland,
    X11,
}

/// The display that the environment names: a Wayland compositor first, else an X server.
fn display() -> Display {
    let named = |variable| std::env::var_os(variable).is_some_and(|value| !value.is_empty());
    if named("WAYLAND_DISPLAY") || named("WAYLAND_SOCKET") {
        Display::Wayland
    } else {
        assert!(
            named("DISPLAY"),
            "the test runs on a display, which WAYLAND_DISPLAY or DISPLAY names"
        );
        Display::X11
    }
}

/// Reads the threads of `pid`; an error if it ended.
fn threads(pid: u32) -> Result<Threads, String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|error| format!("the program cannot be read: {error}"))?;
    let state = stat
        .rsplit_once(')')
        .and_then(|(_, fields)| fields.split_whitespace().next());
    if matches!(state, Some("Z" | "X")) {
        return Err("the program ended".to_owned());
    }
    let mut threads = Threads::new();
    let tasks = std::fs::read_dir(format!("/proc/{pid}/task"))
        .map_err(|error| format!("the program cannot be read: {error}"))?;
    for task in tasks.flatten() {
        let Some(tid) = task.file_name().to_str().and_then(|tid| tid.parse().ok()) else {
            continue;
        };
        // A thread that ends as it is read is left out, as one that ended before.
        let (Ok(status), Ok(stat)) = (
            std::fs::read_to_string(task.path().join("status")),
            std::fs::read_to_string(task.path().join("stat")),
        ) else {
            continue;
        };
        let switches: u64 = status
            .lines()
            .filter_map(|line| {
                let (name, count) = line.split_once(':')?;
                name.ends_with("voluntary_ctxt_switches")
                    .then(|| count.trim().parse::<u64>().ok())?
            })
            .sum();
        // `utime` and `stime`, the 12th and 13th fields after the program's name.
        let ticks: u64 = stat
            .rsplit_once(')')
            .map(|(_, fields)| {
                fields
                    .split_whitespace()
                    .skip(11)
                    .take(2)
                    .filter_map(|ticks| ticks.parse::<u64>().ok())
                    .sum()
            })
            .unwrap_or(0);
        threads.insert(tid, (switches, ticks));
    }
    Ok(threads)
}

/// Waits until `example` stays still for a second, every frame callback that it asked answered by
/// the compositor, for at most `SETTLE`: its threads as they are then. A compositor that answers
/// late, as Weston on a loaded runner, would otherwise wake it in the window of rest, by a message
/// that it asked for.
fn settled(example: &Example) -> Result<Threads, String> {
    let pid = example.pid();
    let deadline = Instant::now() + SETTLE;
    let mut before = threads(pid)?;
    loop {
        std::thread::sleep(STILL);
        let after = threads(pid)?;
        let still = after == before;
        before = after;
        let awaited = awaited_callbacks(&example.read_log());
        if still && awaited.is_empty() {
            return Ok(before);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "never still for {STILL:?} within {SETTLE:?}, its callbacks {awaited:?} awaited"
            ));
        }
    }
}

/// The frame callbacks that the log asked and the compositor has not answered yet, by their
/// object's number, which the protocol reuses once one is answered.
fn awaited_callbacks(log: &str) -> Vec<String> {
    let mut awaited = Vec::new();
    for line in log.lines() {
        if let Some((_, asked)) = line.split_once(".frame(new id wl_callback#") {
            awaited.push(asked.trim_end_matches(')').to_owned());
        } else if let Some((before, _)) = line.split_once(".done(")
            && let Some((_, id)) = before.rsplit_once("wl_callback#")
        {
            awaited.retain(|asked| asked != id);
        }
    }
    awaited
}

/// Waits until `example` settles, then watches it for `REST`: what woke, if anything did. A wake
/// is told with the threads that woke, by name, and the lines that its protocol's log gained
/// meanwhile, the compositor's messages among them, so that a failure in CI shows its cause.
fn at_rest(example: &Example) -> Result<(), String> {
    let pid = example.pid();
    let before = settled(example)?;
    let logged = example.read_log().len();
    std::thread::sleep(REST);
    let after = threads(pid)?;
    if after == before {
        return Ok(());
    }
    let woke: Vec<String> = after
        .iter()
        .filter(|(tid, counts)| before.get(tid) != Some(counts))
        .map(|(tid, (switches, ticks))| {
            let name = std::fs::read_to_string(format!("/proc/{pid}/task/{tid}/comm"))
                .map_or_else(|_| "?".to_owned(), |name| name.trim().to_owned());
            let (was_switches, was_ticks) = before.get(tid).copied().unwrap_or_default();
            format!(
                "{tid} {name:?}: {} switches, {} ticks",
                switches.saturating_sub(was_switches),
                ticks.saturating_sub(was_ticks)
            )
        })
        .collect();
    let gone: Vec<&u32> = before
        .keys()
        .filter(|tid| !after.contains_key(tid))
        .collect();
    let log = example.read_log();
    let gained = log.get(logged..).unwrap_or("");
    Err(format!(
        "it woke in {REST:?}: {woke:?}, threads ended {gone:?}; its protocol meanwhile:\n{gained}"
    ))
}

/// The example, built in this test's profile. Cargo puts it in `<profile>/examples/`, as its book
/// documents, and, with its default build directory, this test below the same `<profile>`
/// directory, in a layout that Cargo keeps internal: the example is in the test's nearest ancestor
/// that holds one. A build directory moved elsewhere (`build.build-dir`) finds none
/// (`docs/debt.md`).
fn example() -> PathBuf {
    let test = std::env::current_exe().expect("the test's path");
    let name = format!("examples/application{}", std::env::consts::EXE_SUFFIX);
    test.ancestors()
        .map(|directory| directory.join(&name))
        .find(|example| example.is_file())
        .unwrap_or_else(|| {
            panic!(
                "no {name} above {}: cargo build -p uwui-desktop --example application",
                test.display()
            )
        })
}

/// How many events named `name` the trace `trace` holds, complete or instant: `frame`, `input to
/// present`, `shown`.
fn traced(trace: &str, name: &str) -> usize {
    let event = format!("{{\"name\":\"{name}\",");
    trace
        .lines()
        .filter(|line| {
            line.starts_with(&event)
                && (line.contains("\"ph\":\"X\"") || line.contains("\"ph\":\"i\""))
        })
        .count()
}

/// Where the *Add* button's middle lies in the window, in logical pixels: the bar's padding, then
/// half the button, `examples/application.rs`.
const ADD: [f64; 2] = [12.0 + 44.0, 8.0 + 14.0];

/// Where the field's middle lies: past *Add* and *Remove* and their gaps, then half the field.
const FIELD: [f64; 2] = [12.0 + 88.0 + 8.0 + 88.0 + 8.0 + 100.0, 8.0 + 14.0];

/// The example's logical width where the compositor leaves its size to it.
const WIDTH: f64 = 720.0;

/// The numbers between the parentheses of the last line of `log` that holds `call`.
fn last_call(log: &str, call: &str) -> Option<Vec<String>> {
    let line = log.lines().rev().find(|line| line.contains(call))?;
    let (_, arguments) = line.split_once(call)?;
    let arguments = arguments.split_once(')')?.0;
    Some(
        arguments
            .split(',')
            .map(|argument| argument.trim().to_owned())
            .collect(),
    )
}

/// The scale that the compositor prefers for the window, in its log: the fractional one, else the
/// integer one of its surface, else the largest of the outputs', else 1.
fn preferred_scale(log: &str) -> f64 {
    let number =
        |call| last_call(log, call).and_then(|arguments| arguments.first()?.parse::<f64>().ok());
    let outputs = log
        .lines()
        .filter(|line| line.contains("wl_output#") && !line.contains(" -> "))
        .filter_map(|line| {
            line.split_once(".scale(")?
                .1
                .split(')')
                .next()?
                .parse::<f64>()
                .ok()
        })
        .reduce(f64::max);
    number(".preferred_scale(")
        .map(|scale| scale / 120.0)
        .or_else(|| number(".preferred_buffer_scale("))
        .or(outputs)
        .unwrap_or(1.0)
}

/// How the window drew, in its log: the width of the last image that its driver made, by wl_shm
/// or by dmabuf; the logical width that the surface shows it at, as its requests say, its
/// viewport's destination or else the image's width over its buffer's scale; and the window's own
/// logical width, the toplevel's last configure's or the example's.
struct Drawn {
    pixels: f64,
    shown: f64,
    logical: f64,
}

impl Drawn {
    fn read(log: &str) -> Option<Self> {
        // wl_shm's width is its third argument, dmabuf's its second.
        let line = log
            .lines()
            .rev()
            .find(|line| line.contains(".create_buffer(") || line.contains(".create_immed("))?;
        let (call, at) = if line.contains(".create_buffer(") {
            (".create_buffer(", 2)
        } else {
            (".create_immed(", 1)
        };
        let pixels: f64 = last_call(line, call)?.get(at)?.parse().ok()?;
        let number = |call| {
            last_call(log, call).and_then(|arguments| arguments.first()?.parse::<f64>().ok())
        };
        // A destination of -1 unsets the viewport's.
        let shown = number(".set_destination(")
            .filter(|&width| width > 0.0)
            .unwrap_or_else(|| pixels / number(".set_buffer_scale(").unwrap_or(1.0));
        let logical = log
            .lines()
            .filter(|line| line.contains("xdg_toplevel#") && line.contains(".configure("))
            .filter_map(|line| {
                line.split_once(".configure(")?
                    .1
                    .split(',')
                    .next()?
                    .trim()
                    .parse::<f64>()
                    .ok()
            })
            .rfind(|&width| width > 0.0)
            .unwrap_or(WIDTH);
        Some(Self {
            pixels,
            shown,
            logical,
        })
    }

    /// The physical pixels per logical one that the window drew at.
    fn scale(&self) -> f64 {
        (self.pixels / self.shown * 100.0).round() / 100.0
    }
}

/// How many images the driver attached to the example's surface so far, in its protocol's log.
fn commits(log: &str) -> usize {
    log.lines()
        .filter(|line| line.contains(".attach(wl_buffer#"))
        .count()
}

/// How many images the driver made for the example's swap chains so far, by wl_shm or by dmabuf,
/// in its protocol's log: a new swap chain makes its own.
fn images_made(log: &str) -> usize {
    log.lines()
        .filter(|line| line.contains(".create_buffer(") || line.contains(".create_immed("))
        .count()
}

/// Where the pointer last entered or moved in the window, in the window's logical pixels: the
/// last two numbers of the last `wl_pointer`'s `enter` or `motion` in the log.
fn pointer_in_window(log: &str) -> Option<[f64; 2]> {
    let line = log.lines().rev().find(|line| {
        line.contains("wl_pointer#") && (line.contains(".enter(") || line.contains(".motion("))
    })?;
    let arguments = line.rsplit_once('(')?.1.trim_end().trim_end_matches(')');
    let mut numbers = arguments
        .rsplit(',')
        .map(|number| number.trim().parse::<f64>());
    let y = numbers.next()?.ok()?;
    let x = numbers.next()?.ok()?;
    Some([x, y])
}

/// Gives `command` to the series' input, if the compositor takes input, `scripts/compositor-input.py`.
fn input(command: &str) -> bool {
    let Some(socket) = std::env::var_os("SLIME_COMPOSITOR_INPUT") else {
        return false;
    };
    let mut stream = UnixStream::connect(socket).expect("the input's server");
    writeln!(stream, "{command}").expect("the command is sent");
    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .expect("the input's server answers");
    assert_eq!(
        answer.trim(),
        "done",
        "the input's server did not run {command:?}"
    );
    true
}

/// Moves the pointer to `x` and `y` of the output through the series' input, if it takes input.
fn move_pointer(x: u32, y: u32) -> bool {
    input(&format!("move-to {x} {y}"))
}

/// The example running, its protocol logged into a file named for the test, its output read on
/// a thread of its own so that the test waits for each line with a bound.
struct Example {
    child: Child,
    lines: mpsc::Receiver<String>,
    log: PathBuf,
    /// Its standard input, where it takes commands, with `--commands`.
    commands: Option<ChildStdin>,
    display: Display,
}

impl Example {
    /// Starts the example, its log in `log`, a file of the test's directory, and its start's
    /// token of activation `token`, and waits for its first line.
    fn start(log: &str, token: &str) -> Self {
        Self::start_with(log, token, false)
    }

    /// Starts the example as [`start`](Self::start) does, taking commands on its input if
    /// `commands`, in a state directory of its own, empty, named after `log`.
    fn start_with(log: &str, token: &str, commands: bool) -> Self {
        let state = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("state-{log}"));
        let _ = std::fs::remove_dir_all(&state);
        Self::start_in(log, token, commands, &state)
    }

    /// Starts the example as [`start_with`](Self::start_with) does, in the state directory
    /// `state`, where it keeps its crash reports, never the user's own.
    fn start_in(log: &str, token: &str, commands: bool, state: &std::path::Path) -> Self {
        let example = Self::spawn(log, token, commands, state, &[]);
        let started = example
            .lines
            .recv_timeout(SETTLE)
            .expect("the example writes its first line");
        assert_eq!(started, "started", "the example did not start");
        example
    }

    /// Starts the example as [`start_in`](Self::start_in) does, with `arguments` more, without
    /// waiting for its first line.
    fn spawn(
        log: &str,
        token: &str,
        commands: bool,
        state: &std::path::Path,
        arguments: &[&str],
    ) -> Self {
        let display = display();
        let log = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(log);
        let log_file = std::fs::File::create(&log).expect("the example's log");
        let mut command = Command::new(example());
        command.args(arguments);
        if commands {
            command.arg("--commands").stdin(Stdio::piped());
        }
        let mut child = command
            .env("UWUI_TRACE", log.with_extension("trace.json"))
            .env("WAYLAND_DEBUG", "client")
            .env("XDG_ACTIVATION_TOKEN", token)
            .env("XDG_STATE_HOME", state)
            .stdout(Stdio::piped())
            .stderr(log_file)
            .spawn()
            .expect("the example starts");
        let output = BufReader::new(child.stdout.take().expect("its output"));
        let (send, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in output.lines().map_while(Result::ok) {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let commands = child.stdin.take();
        Self {
            child,
            lines,
            log,
            commands,
            display,
        }
    }

    /// Gives it `command` on its input, then waits for the line that it writes as it takes it.
    fn command(&mut self, command: &str, taken: &str) {
        let input = self.commands.as_mut().expect("started with its commands");
        writeln!(input, "{command}").expect("the command is written");
        input.flush().expect("the command is sent");
        let line = self
            .lines
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("the example did not take {command:?}"));
        assert_eq!(line, taken, "the example did not take {command:?}");
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Its protocol's log so far.
    fn read_log(&self) -> String {
        std::fs::read_to_string(&self.log).expect("the example's log reads")
    }

    /// Its trace so far, which the host writes from the window's opening.
    fn read_trace(&self) -> String {
        std::fs::read_to_string(self.log.with_extension("trace.json")).unwrap_or_default()
    }

    /// How many images it presented so far, and how many its swap chains made: on Wayland, as
    /// its protocol's log tells; on X11, the frames that its trace tells, for both.
    fn presented(&self) -> (usize, usize) {
        match self.display {
            Display::Wayland => {
                let log = self.read_log();
                (commits(&log), images_made(&log))
            }
            Display::X11 => {
                let frames = traced(&self.read_trace(), "frame");
                (frames, frames)
            }
        }
    }

    /// Where its window's top left lies on the screen, in pixels at a scale of 1, on X11: as
    /// `xdotool` finds it by the process that `_NET_WM_PID` names.
    fn x11_origin(&self) -> [f64; 2] {
        let xdotool = |arguments: &[&str]| {
            let output = Command::new("xdotool")
                .args(arguments)
                .output()
                .expect("xdotool runs");
            String::from_utf8_lossy(&output.stdout).into_owned()
        };
        let pid = self.pid().to_string();
        let found = xdotool(&["search", "--sync", "--pid", &pid]);
        let window = found
            .lines()
            .next()
            .unwrap_or_else(|| panic!("xdotool finds no window of the example"));
        let geometry = xdotool(&["getwindowgeometry", "--shell", window]);
        let value = |name: &str| {
            geometry
                .lines()
                .find_map(|line| line.strip_prefix(name)?.parse::<f64>().ok())
                .unwrap_or_else(|| panic!("xdotool gives no {name} of the window: {geometry}"))
        };
        [value("X="), value("Y=")]
    }

    /// Sends it `signal`, `TERM` or `KILL`.
    fn signal(&self, signal: &str) {
        let sent = Command::new("kill")
            .args([&format!("-{signal}"), &self.pid().to_string()])
            .status()
            .expect("kill runs");
        assert!(sent.success(), "SIG{signal} was not sent");
    }

    /// Ends it by SIGTERM, which asks it to close: its last lines, once it ended with success.
    fn end_on_sigterm(mut self) -> Vec<String> {
        self.signal("TERM");
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("the example's status") {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "the example did not end on SIGTERM"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        // Its output ended with it: the thread sends what is left, then stops.
        let rest = self.lines.iter().collect();
        eprint!("{}", self.read_log());
        assert!(status.success(), "the example ended with {status}");
        rest
    }
}

impl Example {
    /// Waits for it to end by itself, for at most 10 s: its status and its last lines.
    fn end_by_itself(&mut self) -> (std::process::ExitStatus, Vec<String>) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("the example's status") {
                break status;
            }
            assert!(Instant::now() < deadline, "the example did not end");
            std::thread::sleep(Duration::from_millis(50));
        };
        (status, self.lines.iter().collect())
    }
}

impl Drop for Example {
    /// Ends the example if the test fails before it does.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn the_application_rests_wakes_for_the_pointer_and_ends_on_a_signal() {
    let example = Example::start("rest-application.log", "");
    let shown = example.log.display().to_string();
    if let Err(error) = at_rest(&example) {
        panic!("the example is not at rest after its start: {error}");
    }
    assert!(
        example.presented().0 > 0,
        "the example presented no image: {shown}"
    );
    assert!(
        traced(&example.read_trace(), "frame") > 0,
        "the example's trace holds no frame: {shown}"
    );
    // Every compositor of the series tells when an image showed (`wp_presentation`); X11 only
    // where the driver presents through Present, which a software one does not.
    if example.display == Display::Wayland {
        assert!(
            traced(&example.read_trace(), "shown") > 0,
            "the example's trace tells no image shown: {shown}"
        );
    }

    // The output's middle, which the window covers in each compositor of the series: the window's
    // place on the output follows from where the pointer lies in it, both in logical pixels at
    // the scale of 1 that the series' input counts in.
    let mut typed = false;
    if move_pointer(960, 540) {
        std::thread::sleep(Duration::from_secs(1));
        // Where the window's top left lies on the output.
        let origin = match example.display {
            Display::Wayland => {
                let [x, y] = pointer_in_window(&example.read_log())
                    .unwrap_or_else(|| panic!("the pointer never reached the window: {shown}"));
                [960.0 - x, 540.0 - y]
            }
            Display::X11 => example.x11_origin(),
        };
        let before = example.presented().0;
        let place =
            |at: [f64; 2]| [origin[0] + at[0], origin[1] + at[1]].map(|side| side.round() as u32);
        let add = place(ADD);
        assert!(move_pointer(add[0], add[1]));
        let deadline = Instant::now() + Duration::from_secs(5);
        while example.presented().0 == before {
            assert!(
                Instant::now() < deadline,
                "the pointer on the button drew no frame: {shown}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Err(error) = at_rest(&example) {
            panic!("the example is not at rest after the pointer hovered its button: {error}");
        }
        assert!(
            traced(&example.read_trace(), "input to present") > 0,
            "the hover's frames are in no input to present of the trace: {shown}"
        );

        // A name typed into the field, and a row added under it: the keyboard's way whole, which
        // X11's layer takes from M6d.7b2.
        let field = place(FIELD);
        if example.display == Display::X11 {
            println!("keys: not run, X11's layer takes no key yet");
            println!("cursor: the server's");
            // Present tells the images shown where the driver presents through it, with RandR's
            // period where it gives one.
            if traced(&example.read_trace(), "shown") > 0 {
                println!("period: images shown told by Present");
            } else {
                println!("period: none, measured from the signals");
            }
            assert_eq!(
                example.end_on_sigterm(),
                ["closing asked", "stopped", "dropped"],
                "the example was not asked to close, then stopped"
            );
            return;
        }
        assert!(move_pointer(field[0], field[1]));
        assert!(input("click"));
        assert!(input("key s"));
        assert!(input("key a"));
        assert!(move_pointer(add[0], add[1]));
        assert!(input("click"));
        let added = example
            .lines
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("the click on Add added no row: {shown}"));
        assert_eq!(added, "added sa", "the field did not take the keys typed");
        println!("keys: typed");
        typed = true;
        if let Err(error) = at_rest(&example) {
            panic!("the example is not at rest after the keys typed: {error}");
        }
    }

    // The cursor that the host set, each shape of `wp_cursor_shape_v1` by its number: 9 is the
    // I-beam, which the field asks while the pointer lies over it.
    let log = example.read_log();
    let shapes: Vec<String> = log
        .lines()
        .filter(|line| line.contains("wp_cursor_shape_device_v1#") && line.contains(".set_shape("))
        .filter_map(|line| last_call(line, ".set_shape(")?.get(1).cloned())
        .collect();
    let entered = log.contains("wl_pointer#") && log.contains(".enter(");
    let cursor = match (shapes.is_empty(), entered) {
        (false, _) => "by shape",
        (true, true) => "the compositor's",
        (true, false) => "not run, the pointer never entered",
    };
    if !shapes.is_empty() && typed {
        assert!(
            shapes.iter().any(|shape| shape == "9"),
            "the cursor over the field was never the I-beam, its shapes {shapes:?}: {shown}"
        );
    }
    println!("cursor: {cursor}");
    assert_eq!(
        example.end_on_sigterm(),
        ["closing asked", "stopped", "dropped"],
        "the example was not asked to close, then stopped"
    );
}

/// Gives `example` `command`, which it answers with `taken`, then waits until a new swap chain's
/// image is presented, for at most 10 s: how long that took from the command, to 50 ms. On X11,
/// whose trace tells no swap chain, until an image is presented after the command.
fn drawn_again(example: &mut Example, command: &str, taken: &str) -> Duration {
    let (presented, made) = example.presented();
    let asked = Instant::now();
    example.command(command, taken);
    let deadline = asked + Duration::from_secs(10);
    loop {
        let (now_presented, now_made) = example.presented();
        if now_presented > presented && now_made > made {
            return asked.elapsed();
        }
        assert!(
            Instant::now() < deadline,
            "after {command:?}, no new swap chain's image was presented: {}",
            example.log.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_application_draws_again_after_its_device_and_its_surface_are_lost() {
    let mut example = Example::start_with("lost-application.log", "", true);
    if let Err(error) = settled(&example) {
        panic!("the example never settled: {error}");
    }
    for (command, taken) in [
        ("lose device", "losing the device"),
        ("lose surface", "losing the surface"),
    ] {
        let took = drawn_again(&mut example, command, taken);
        // The surface given up is made again a second later.
        println!("{command}: drawn again in {} ms", took.as_millis());
        if let Err(error) = settled(&example) {
            panic!("the example never settled after {command:?}: {error}");
        }
    }
    // Once, after both: a recovery that keeps waking would wake it still.
    if let Err(error) = at_rest(&example) {
        panic!("the example is not at rest after its losses: {error}");
    }
    println!("lost: drawn again");
    // Two devices more are replaced; the fourth lost within five minutes ends the run, the
    // application stopped all the same.
    for _ in 0..2 {
        drawn_again(&mut example, "lose device", "losing the device");
    }
    example.command("lose device", "losing the device");
    let (status, rest) = example.end_by_itself();
    assert_eq!(
        status.code(),
        Some(1),
        "the fourth device lost did not end the run with EXIT_HOST_FAILED"
    );
    assert_eq!(
        rest,
        ["stopped", "dropped"],
        "the application was not stopped"
    );
    let log = example.read_log();
    assert!(
        log.contains("the GPU's device was lost again after 3 new ones within 5 minutes"),
        "the host did not tell why it ended: {}",
        example.log.display()
    );
}

/// The files in `directory` whose names start with `start`, sorted.
fn files(directory: &std::path::Path, start: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(start))
        .map(|entry| entry.path())
        .collect();
    files.sort();
    files
}

#[test]
fn a_panic_leaves_its_report_and_the_next_start_finds_each_runs_marker() {
    let state = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("state-crashes");
    let _ = std::fs::remove_dir_all(&state);
    let crashes = state.join("dev.uwui.Application").join("crashes");

    // A panic in the application's step ends the run with EXIT_PANICKED, the application neither
    // stopped nor dropped, its report written and its marker left.
    let mut example = Example::start_in("panic-application.log", "", true, &state);
    example.command("panic", "panicking");
    let (status, rest) = example.end_by_itself();
    let shown = example.log.display().to_string();
    assert_eq!(
        status.code(),
        Some(101),
        "the panic did not end the run with EXIT_PANICKED: {shown}"
    );
    assert!(
        rest.is_empty(),
        "the application was called after its panic: {rest:?}"
    );
    let message = "the application was made to panic by its command";
    assert!(
        example
            .read_log()
            .contains(&format!("uwui's desktop host ended on a panic: {message}")),
        "the host did not tell the panic: {shown}"
    );
    let reports = files(&crashes, "crash-");
    assert_eq!(reports.len(), 1, "no report, or more than one: {reports:?}");
    let report = std::fs::read_to_string(&reports[0]).expect("the report reads");
    for told in [
        format!("message:\n{message}\n"),
        format!(
            "application: dev.uwui.Application {}\n",
            env!("CARGO_PKG_VERSION")
        ),
        "at: crates/uwui-desktop/examples/application.rs:".to_owned(),
        "backtrace:\n".to_owned(),
    ] {
        assert!(
            report.contains(&told),
            "{told:?} not in the report: {report}"
        );
    }
    assert!(
        !report.contains("GPU: none opened yet"),
        "the report names no GPU: {report}"
    );
    if let Some(home) = std::env::var_os("HOME").filter(|home| home.len() > 1) {
        let home = home.to_string_lossy();
        assert!(
            !report.contains(home.as_ref()),
            "the report names the home directory"
        );
    }
    assert_eq!(
        files(&crashes, "session-").len(),
        1,
        "the panic left no marker"
    );

    // The next start finds the panic's marker with its report; then a run killed, as the system's
    // killer of memory would, leaves its own.
    let example = Example::start_in("killed-application.log", "", false, &state);
    let report_name = reports[0]
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    let log = example.read_log();
    assert!(
        log.contains("ended by a panic after") && log.contains(&report_name),
        "the next start did not find the panic's marker: {}",
        example.log.display()
    );
    example.signal("KILL");
    drop(example);

    // The start after finds the killed run's marker, ends cleanly, and the tally counts the three.
    let example = Example::start_in("after-crashes-application.log", "", false, &state);
    assert!(
        example
            .read_log()
            .contains("did not end: it was killed or aborted"),
        "the next start did not find the killed run's marker: {}",
        example.log.display()
    );
    assert_eq!(
        example.end_on_sigterm(),
        ["closing asked", "stopped", "dropped"]
    );
    assert!(
        files(&crashes, "session-").is_empty(),
        "a marker was left after a clean end"
    );
    let hours = std::fs::read_to_string(crashes.join("hours")).expect("the tally reads");
    assert!(
        hours.starts_with("# hours 1\n") && hours.ends_with("sessions 3\ncrashes 2\n"),
        "the runs were not counted: {hours}"
    );
    println!("crashes: reported and found");
}

#[test]
fn a_panic_in_the_start_or_the_stop_ends_the_run_without_another_call() {
    let state = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("state-panic-calls");
    let _ = std::fs::remove_dir_all(&state);
    let crashes = state.join("dev.uwui.Application").join("crashes");
    // In its start: the run ends with EXIT_PANICKED before the application writes anything.
    let mut example = Example::spawn(
        "panic-start-application.log",
        "",
        false,
        &state,
        &["--panic-in", "start"],
    );
    let (status, rest) = example.end_by_itself();
    assert_eq!(
        status.code(),
        Some(101),
        "a panic in the start did not end the run with EXIT_PANICKED: {}",
        example.log.display()
    );
    assert!(
        rest.is_empty(),
        "the application was called after its panic: {rest:?}"
    );
    drop(example);
    // In its stop, once asked to close: neither stopped again nor dropped.
    let example = Example::spawn(
        "panic-stop-application.log",
        "",
        false,
        &state,
        &["--panic-in", "stop"],
    );
    let started = example
        .lines
        .recv_timeout(SETTLE)
        .expect("the example writes its first line");
    assert_eq!(started, "started");
    let mut example = example;
    example.signal("TERM");
    let (status, rest) = example.end_by_itself();
    assert_eq!(
        status.code(),
        Some(101),
        "a panic in the stop did not end the run with EXIT_PANICKED: {}",
        example.log.display()
    );
    assert_eq!(
        rest,
        ["closing asked"],
        "the application was called after its panic"
    );
    let reports = files(&crashes, "crash-");
    assert_eq!(reports.len(), 2, "a report for each panic: {reports:?}");
    for (report, call) in reports.iter().zip(["start", "stop"]) {
        let report = std::fs::read_to_string(report).expect("the report reads");
        assert!(
            report.contains(&format!("made to panic in its {call}")),
            "the report of the panic in its {call}: {report}"
        );
    }
}

#[test]
fn a_second_termination_ends_an_application_that_keeps_its_window() {
    let mut example = Example::start_with("keep-application.log", "", true);
    example.command("keep", "keeping its window");
    example.signal("TERM");
    let asked = example
        .lines
        .recv_timeout(Duration::from_secs(10))
        .expect("the example was asked to close");
    assert_eq!(asked, "closing asked");
    // Past the window within which a signal repeats the first.
    std::thread::sleep(Duration::from_millis(700));
    assert!(
        example
            .child
            .try_wait()
            .expect("the example's status")
            .is_none(),
        "the example ended though it kept its window"
    );
    example.signal("TERM");
    let (status, rest) = example.end_by_itself();
    assert_eq!(
        status.code(),
        Some(130),
        "the second termination did not end the run with EXIT_TERMINATED"
    );
    assert!(
        rest.is_empty(),
        "the application was called after the second signal: {rest:?}"
    );
}

/// A token of activation that no compositor gave, which the example's start passes on all the same.
const TOKEN: &str = "uwui-desktop-test-token";

#[test]
fn the_window_draws_at_the_scale_that_the_compositor_prefers() {
    let example = Example::start("scale-application.log", TOKEN);
    // Settled, the window has its last scale: the compositor tells it once the window shows.
    if let Err(error) = settled(&example) {
        panic!("the example never settled: {error}");
    }
    let log = example.read_log();
    let preferred = preferred_scale(&log);
    let drawn = Drawn::read(&log)
        .unwrap_or_else(|| panic!("the example made no image: {}", example.log.display()));
    // The surface shows the image at the window's size: the scale reached the compositor.
    assert!(
        (drawn.shown - drawn.logical).abs() < 1.0,
        "the surface shows its image {} wide, the window being {}: {}",
        drawn.shown,
        drawn.logical,
        example.log.display()
    );
    let scale = drawn.scale();
    assert!(
        (scale - preferred).abs() < 0.01,
        "the window drew at {scale}, the compositor preferring {preferred}: {}",
        example.log.display()
    );
    println!("scale: {scale:?}");
    // The start's token activates the window, where the compositor gives activation.
    if log.contains("\"xdg_activation_v1\"") {
        assert!(
            log.contains(&format!(".activate(\"{TOKEN}\", wl_surface#")),
            "the start's token did not activate the window: {}",
            example.log.display()
        );
    }
    assert_eq!(
        example.end_on_sigterm(),
        ["closing asked", "stopped", "dropped"]
    );
}

#[test]
fn a_frame_callback_awaits_its_done() {
    // As CI's Weston logged them: two frames asked, then their callbacks answered late, the
    // object numbers reused.
    let asked = "[17:42:13.1] -> wl_surface#7.frame(new id wl_callback#53)
[17:42:13.2] {mesa vk display queue}  -> wl_surface#7.commit()
[17:42:13.4] -> wl_surface#7.frame(new id wl_callback#55)";
    assert_eq!(awaited_callbacks(asked), ["53", "55"]);
    let answered = format!(
        "{asked}
[17:42:15.2] {{Display Queue}} wl_display#1.delete_id(53)
[17:42:15.2] wl_callback#53.done(495841)
[17:42:15.2] wl_callback#55.done(495841)
[17:42:15.3] -> wl_surface#7.frame(new id wl_callback#53)"
    );
    assert_eq!(awaited_callbacks(&answered), ["53"]);
    assert!(awaited_callbacks("").is_empty());
}

/// A program that the test started, ended if the test fails before it does.
struct Started(Child);

impl Drop for Started {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_blank_window_presents_its_first_image_and_ends_on_a_signal() {
    let blank = example().with_file_name("blank");
    assert!(
        blank.exists(),
        "{} is not built: cargo build -p uwui-desktop --example blank",
        blank.display()
    );
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("rest-blank");
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the test's directory");
    let trace = directory.join("trace.json");
    let mut started = Started(
        Command::new(&blank)
            .env("UWUI_TRACE", &trace)
            .env("XDG_STATE_HOME", &directory)
            .stdout(Stdio::null())
            .spawn()
            .expect("the blank window starts"),
    );
    let deadline = Instant::now() + SETTLE;
    while traced(
        &std::fs::read_to_string(&trace).unwrap_or_default(),
        "frame",
    ) == 0
    {
        assert!(
            started.0.try_wait().expect("its status").is_none(),
            "the blank window ended before its first image"
        );
        assert!(
            Instant::now() < deadline,
            "the blank window presented no image within {SETTLE:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let sent = Command::new("kill")
        .args(["-TERM", &started.0.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(sent.success(), "SIGTERM was not sent");
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = started.0.try_wait().expect("its status") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "the blank window did not end on SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "the blank window ended with {status}");
}
