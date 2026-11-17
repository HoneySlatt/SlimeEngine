//! The `application` example in a Wayland compositor, which the environment names: it starts,
//! draws and rests, none of its threads waking in 10 s (`docs/adr/0185-rest-checked-in-ci.md`);
//! where the compositor takes a person's input, the pointer put on its button *Add* hovers it,
//! which draws frames, and it rests again once the hover's transition ends; then a click in its
//! field, `s` and `a` typed, and a click on *Add*, whose row is named `sa`; a termination signal
//! then asks it to close, and it ends cleanly. Weston takes no input: there the input's part does
//! not run. It writes on its standard output `keys: typed` once the row is added, and how its
//! cursor was set, `cursor: by shape`. The series' input counts the output in logical pixels at a scale of 1: this test runs
//! at that scale.
//!
//! A second test, run in a compositor at another scale, finds the window drawn at the scale that
//! the compositor prefers, and the start's token of activation passed to the compositor where it
//! gives `xdg_activation_v1`, then ends it, and writes that scale, `scale: 1.5`, on its standard
//! output.
//!
//! The example writes its protocol with `WAYLAND_DEBUG=client` into a file of each test's own,
//! read for the images it presented and their size, the scale preferred, the pointer's events and
//! where its window lies, then written on the test's standard error, where
//! `scripts/compositors.sh` reads who draws its bar.
//!
//! The example is built first, `cargo build -p uwui-desktop --example application`;
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
use std::process::{Child, Command, Stdio};
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

/// The example, built beside this test.
fn example() -> PathBuf {
    let test = std::env::current_exe().expect("the test's path");
    let profile = test
        .parent()
        .and_then(|deps| deps.parent())
        .expect("the test lies in target/<profile>/deps");
    let example = profile.join("examples/application");
    assert!(
        example.exists(),
        "{} is not built: cargo build -p uwui-desktop --example application",
        example.display()
    );
    example
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
}

impl Example {
    /// Starts the example, its log in `log`, a file of the test's directory, and its start's
    /// token of activation `token`, and waits for its first line.
    fn start(log: &str, token: &str) -> Self {
        assert!(
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            "the test runs in a Wayland compositor, which WAYLAND_DISPLAY names"
        );
        let log = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(log);
        let log_file = std::fs::File::create(&log).expect("the example's log");
        let mut child = Command::new(example())
            .env("WAYLAND_DEBUG", "client")
            .env("XDG_ACTIVATION_TOKEN", token)
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
        let example = Self { child, lines, log };
        let started = example
            .lines
            .recv_timeout(SETTLE)
            .expect("the example writes its first line");
        assert_eq!(started, "started", "the example did not start");
        example
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Its protocol's log so far.
    fn read_log(&self) -> String {
        std::fs::read_to_string(&self.log).expect("the example's log reads")
    }

    /// Ends it by SIGTERM, which asks it to close: its last lines, once it ended with success.
    fn end_on_sigterm(mut self) -> Vec<String> {
        let killed = Command::new("kill")
            .args(["-TERM", &self.pid().to_string()])
            .status()
            .expect("kill runs");
        assert!(killed.success());
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
        commits(&example.read_log()) > 0,
        "the example presented no image: {shown}"
    );

    // The output's middle, which the window covers in each compositor of the series: the window's
    // place on the output follows from where the pointer lies in it, both in logical pixels at
    // the scale of 1 that the series' input counts in.
    let mut typed = false;
    if move_pointer(960, 540) {
        std::thread::sleep(Duration::from_secs(1));
        let log = example.read_log();
        let [x, y] = pointer_in_window(&log)
            .unwrap_or_else(|| panic!("the pointer never reached the window: {shown}"));
        let before = commits(&log);
        let place =
            |at: [f64; 2]| [960.0 - x + at[0], 540.0 - y + at[1]].map(|side| side.round() as u32);
        let add = place(ADD);
        assert!(move_pointer(add[0], add[1]));
        let deadline = Instant::now() + Duration::from_secs(5);
        while commits(&example.read_log()) == before {
            assert!(
                Instant::now() < deadline,
                "the pointer on the button drew no frame: {shown}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Err(error) = at_rest(&example) {
            panic!("the example is not at rest after the pointer hovered its button: {error}");
        }

        // A name typed into the field, and a row added under it: the keyboard's way whole.
        let field = place(FIELD);
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
        ["closing asked", "stopped"],
        "the example was not asked to close, then stopped"
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
    assert_eq!(example.end_on_sigterm(), ["closing asked", "stopped"]);
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
