//! The `application` example in a Wayland compositor, which the environment names: it starts,
//! draws and rests, none of its threads waking in 10 s (`docs/adr/0185-rest-checked-in-ci.md`);
//! where the compositor takes a person's input, the pointer put on its button *Add* hovers it,
//! which draws frames, and it rests again once the hover's transition ends; a termination signal
//! then asks it to close, and it ends cleanly. Weston takes no input: there the pointer's part does
//! not run.
//!
//! The example writes its protocol with `WAYLAND_DEBUG=client` into a file of the test's own, read
//! for the images it presented, the pointer's events and where its window lies, then written on
//! the test's standard error, where `scripts/compositors.sh` reads who draws its bar.
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
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
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

/// Waits until `pid` stays still for a second, for at most `SETTLE`, then watches it for `REST`:
/// what woke, if anything did.
fn at_rest(pid: u32) -> Result<(), String> {
    let deadline = Instant::now() + SETTLE;
    let mut before = threads(pid)?;
    loop {
        std::thread::sleep(STILL);
        let after = threads(pid)?;
        let still = after == before;
        before = after;
        if still {
            break;
        }
        if Instant::now() >= deadline {
            return Err(format!("never still for {STILL:?} within {SETTLE:?}"));
        }
    }
    std::thread::sleep(REST);
    let after = threads(pid)?;
    if after == before {
        Ok(())
    } else {
        Err(format!(
            "it woke in {REST:?}: before {before:?}, after {after:?}"
        ))
    }
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

/// Moves the pointer to `x` and `y` of the output through the series' input, if it takes input.
fn move_pointer(x: u32, y: u32) -> bool {
    let Some(socket) = std::env::var_os("SLIME_COMPOSITOR_INPUT") else {
        return false;
    };
    let mut stream = UnixStream::connect(socket).expect("the input's server");
    writeln!(stream, "move-to {x} {y}").expect("the command is sent");
    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .expect("the input's server answers");
    assert_eq!(answer.trim(), "done", "the pointer did not move");
    true
}

/// Ends `child` if the test fails before it does.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_application_rests_wakes_for_the_pointer_and_ends_on_a_signal() {
    assert!(
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        "the test runs in a Wayland compositor, which WAYLAND_DISPLAY names"
    );
    let log_path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("rest-application.log");
    let log_file = std::fs::File::create(&log_path).expect("the example's log");
    let read_log = || std::fs::read_to_string(&log_path).expect("the example's log reads");
    let child = Command::new(example())
        .env("WAYLAND_DEBUG", "client")
        .stdout(Stdio::piped())
        .stderr(log_file)
        .spawn()
        .expect("the example starts");
    let mut running = Running(child);
    let pid = running.0.id();
    let mut lines = BufReader::new(running.0.stdout.take().expect("its output"));
    let mut line = String::new();
    lines.read_line(&mut line).expect("its first line");
    assert_eq!(line.trim(), "started", "the example did not start");

    if let Err(error) = at_rest(pid) {
        panic!("the example is not at rest after its start: {error}");
    }
    assert!(
        commits(&read_log()) > 0,
        "the example presented no image: {}",
        log_path.display()
    );

    // The output's middle, which the window covers in each compositor of the series: the window's
    // place on the output follows from where the pointer lies in it.
    if move_pointer(960, 540) {
        std::thread::sleep(Duration::from_secs(1));
        let log = read_log();
        let [x, y] = pointer_in_window(&log).unwrap_or_else(|| {
            panic!(
                "the pointer never reached the window: {}",
                log_path.display()
            )
        });
        let before = commits(&log);
        let to = [960.0 - x + ADD[0], 540.0 - y + ADD[1]].map(|side| side.round() as u32);
        assert!(move_pointer(to[0], to[1]));
        let deadline = Instant::now() + Duration::from_secs(5);
        while commits(&read_log()) == before {
            assert!(
                Instant::now() < deadline,
                "the pointer on the button drew no frame: {}",
                log_path.display()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Err(error) = at_rest(pid) {
            panic!("the example is not at rest after the pointer hovered its button: {error}");
        }
    }

    let killed = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("kill runs");
    assert!(killed.success());
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = running.0.try_wait().expect("the example's status") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "the example did not end on SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut rest = String::new();
    lines.read_to_string(&mut rest).expect("its last lines");
    eprint!("{}", read_log());
    assert!(status.success(), "the example ended with {status}");
    assert_eq!(
        rest.lines().collect::<Vec<_>>(),
        ["closing asked", "stopped"],
        "the example was not asked to close, then stopped"
    );
}
