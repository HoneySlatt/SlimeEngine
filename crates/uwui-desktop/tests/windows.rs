//! The `application` example on Windows, on the desktop that the session shows: it starts, draws
//! and rests, none of its threads running in 10 s (`docs/adr/0185-rest-checked-in-ci.md`); the
//! mouse put on its button *Add*, through `SendInput`, hovers it, which draws frames, and it rests
//! again once the hover's transition ends; asked to close by the message that the window's close
//! button sends, `WM_CLOSE`, it ends cleanly, as it does on Ctrl+Break, Windows' termination signal
//! of a console's program.
//!
//! A second test loses the example's GPU device on purpose, then its surface, through its commands
//! on its standard input, and finds frames drawn after each, then the example at rest; it writes how
//! long each took, `lose device: drawn again in 120 ms`. A last test runs the blank window,
//! `examples/blank.rs`, which presents its first image, writes it in its trace, and ends with
//! status 0 when asked to close.
//!
//! The examples are built first, `cargo build -p uwui-desktop --example application --example
//! blank`, DXC beside them, as CI's Windows job puts it; each run keeps its state, its crash reports
//! and its marker, in a directory of the test's own, which `LOCALAPPDATA` names. Its trace
//! (`UWUI_TRACE`) is read for its frames and for an input to present.
//!
//! A program at rest leaves its waits for nothing: Windows counts each thread's cycles on the
//! processor (`QueryThreadCycleTime`), which reading does not wake. Every thread of the process
//! counts, the drivers' and the system's with the program's own.

#![cfg(windows)]
#![expect(unsafe_code, reason = "Win32's calls, through windows-sys")]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NEW_PROCESS_GROUP, OpenThread, THREAD_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::System::WindowsProgramming::QueryThreadCycleTime;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
    SM_CXSCREEN, SM_CYSCREEN, WM_CLOSE,
};
use windows_sys::core::BOOL;

/// How long the program is watched: the quality bar's window of rest.
const REST: Duration = Duration::from_secs(10);
/// How long it must stay still before it is watched.
const STILL: Duration = Duration::from_secs(1);
/// How long it may take to come to rest: its first frames, on WARP, a GPU of the CPU.
const SETTLE: Duration = Duration::from_secs(60);

/// Where the *Add* button's middle lies in the window, in logical pixels: the bar's padding, then
/// half the button, `examples/application.rs`.
const ADD: [f64; 2] = [12.0 + 44.0, 8.0 + 14.0];

/// Each thread's cycles on the processor, by its id.
type Threads = BTreeMap<u32, u64>;

/// Reads the threads of the process `pid`.
fn threads(pid: u32) -> Threads {
    // SAFETY: a snapshot of every thread of the system, closed below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    assert!(!snapshot.is_null(), "the threads cannot be read");
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).expect("a small structure"),
        ..Default::default()
    };
    let mut threads = Threads::new();
    // SAFETY: the live snapshot, and a live entry whose size its field says.
    let mut more = unsafe { Thread32First(snapshot, &raw mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: a thread's id; the handle, if any, is closed below.
            let thread =
                unsafe { OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, entry.th32ThreadID) };
            // A thread that ends as it is read is left out, as one that ended before.
            if !thread.is_null() {
                let mut cycles = 0_u64;
                // SAFETY: the live thread's handle, and a live count that the call writes.
                if unsafe { QueryThreadCycleTime(thread, &raw mut cycles) } != 0 {
                    threads.insert(entry.th32ThreadID, cycles);
                }
                // SAFETY: the handle opened above.
                unsafe { CloseHandle(thread) };
            }
        }
        // SAFETY: as above.
        more = unsafe { Thread32Next(snapshot, &raw mut entry) } != 0;
    }
    // SAFETY: the snapshot made above.
    unsafe { CloseHandle(snapshot) };
    threads
}

/// The example's window: the first visible window of the process `pid`, waited for 60 s at most.
fn window_of(pid: u32) -> HWND {
    struct Search {
        pid: u32,
        found: HWND,
    }
    unsafe extern "system" fn each(window: HWND, search: LPARAM) -> BOOL {
        // SAFETY: the search given below, live during the enumeration.
        let search = unsafe { &mut *(search as *mut Search) };
        let mut pid = 0;
        // SAFETY: a window that the system enumerates, and a live id that the call writes.
        unsafe { GetWindowThreadProcessId(window, &raw mut pid) };
        // SAFETY: as above.
        if pid == search.pid && unsafe { IsWindowVisible(window) } != 0 {
            search.found = window;
            return 0;
        }
        1
    }
    let deadline = Instant::now() + SETTLE;
    loop {
        let mut search = Search {
            pid,
            found: std::ptr::null_mut(),
        };
        // SAFETY: the callback above, given the live search.
        unsafe { EnumWindows(Some(each), (&raw mut search) as LPARAM) };
        if !search.found.is_null() {
            return search.found;
        }
        assert!(
            Instant::now() < deadline,
            "the example shows no window within {SETTLE:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Moves the mouse to `point` of the primary screen, in pixels, as a person's hand would.
fn move_mouse(point: [i32; 2]) {
    // SAFETY: plain reads of the screen's size.
    let size = unsafe { [GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)] };
    // Absolute moves count the primary screen from 0 to 65535 on each side.
    let [dx, dy] = [0, 1].map(|side| point[side] * 65_535 / (size[side] - 1).max(1));
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let size = i32::try_from(size_of::<INPUT>()).expect("a small structure");
    // SAFETY: one live input, of its size.
    let sent = unsafe { SendInput(1, &raw const input, size) };
    assert_eq!(sent, 1, "the mouse's move was not sent");
}

/// Where the point `at` of `window`, in its logical pixels, lies on the screen, in pixels.
fn on_screen(window: HWND, at: [f64; 2]) -> [i32; 2] {
    // SAFETY: the example's live window.
    let scale = f64::from(unsafe { GetDpiForWindow(window) }.max(1)) / 96.0;
    let mut origin = POINT { x: 0, y: 0 };
    // SAFETY: the live window, and a live point that the call moves.
    unsafe { ClientToScreen(window, &raw mut origin) };
    [
        origin.x + (at[0] * scale).round() as i32,
        origin.y + (at[1] * scale).round() as i32,
    ]
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

/// The example named `name`, built in this test's profile: in the test's nearest ancestor that
/// holds `examples/`, as in `rest.rs`.
fn example(name: &str) -> PathBuf {
    let test = std::env::current_exe().expect("the test's path");
    let name = format!("examples/{name}{}", std::env::consts::EXE_SUFFIX);
    test.ancestors()
        .map(|directory| directory.join(&name))
        .find(|example| example.is_file())
        .unwrap_or_else(|| {
            panic!(
                "no {name} above {}: cargo build -p uwui-desktop --example application --example \
                 blank",
                test.display()
            )
        })
}

/// An example running, its output's lines as they come.
struct Example {
    child: Child,
    lines: mpsc::Receiver<String>,
    trace: PathBuf,
    commands: Option<ChildStdin>,
}

impl Example {
    /// Starts the example `name` with `arguments`, in a process group of its own, which Ctrl+Break
    /// reaches alone, its state and trace named after `run`.
    fn spawn(name: &str, run: &str, arguments: &[&str]) -> Self {
        let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let state = directory.join(format!("state-{run}"));
        let _ = std::fs::remove_dir_all(&state);
        std::fs::create_dir_all(&state).expect("the state's directory");
        let trace = directory.join(format!("{run}.trace.json"));
        let _ = std::fs::remove_file(&trace);
        let mut command = Command::new(example(name));
        command.args(arguments);
        if arguments.contains(&"--commands") {
            command.stdin(Stdio::piped());
        }
        let mut child = command
            .env("UWUI_TRACE", &trace)
            .env("LOCALAPPDATA", &state)
            .creation_flags(CREATE_NEW_PROCESS_GROUP)
            .stdout(Stdio::piped())
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
            trace,
            commands,
        }
    }

    /// Starts the application with `arguments` and waits for its first line.
    fn start(run: &str, arguments: &[&str]) -> Self {
        let example = Self::spawn("application", run, arguments);
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

    fn read_trace(&self) -> String {
        std::fs::read_to_string(&self.trace).unwrap_or_default()
    }

    fn frames(&self) -> usize {
        traced(&self.read_trace(), "frame")
    }

    /// Waits until it stays still for a second, for at most `SETTLE`: its threads as they are then.
    fn settled(&self) -> Result<Threads, String> {
        let deadline = Instant::now() + SETTLE;
        let mut before = threads(self.pid());
        loop {
            std::thread::sleep(STILL);
            let after = threads(self.pid());
            if after == before {
                return Ok(before);
            }
            before = after;
            if Instant::now() >= deadline {
                return Err(format!("never still for {STILL:?} within {SETTLE:?}"));
            }
        }
    }

    /// Waits until it settles, then watches it for `REST`: what woke, if anything did.
    fn at_rest(&self) -> Result<(), String> {
        let before = self.settled()?;
        std::thread::sleep(REST);
        let after = threads(self.pid());
        if after == before {
            return Ok(());
        }
        let woke: Vec<String> = after
            .iter()
            .filter(|(tid, cycles)| before.get(tid) != Some(cycles))
            .map(|(tid, cycles)| {
                let was = before.get(tid).copied().unwrap_or_default();
                format!("{tid}: {} cycles", cycles.saturating_sub(was))
            })
            .collect();
        let gone: Vec<&u32> = before
            .keys()
            .filter(|tid| !after.contains_key(tid))
            .collect();
        Err(format!(
            "it woke in {REST:?}: {woke:?}, threads ended {gone:?}"
        ))
    }

    /// Waits until its trace holds more frames than `before`, for at most 10 s: how long it took.
    fn drawn_after(&self, before: usize, what: &str) -> Duration {
        let start = Instant::now();
        while self.frames() <= before {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "{what} drew no frame"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        start.elapsed()
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

    /// Waits for it to end, for at most 10 s: whether it ended with success, and its last lines.
    fn ended(mut self) -> (bool, Vec<String>) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("the example's status") {
                break status;
            }
            assert!(Instant::now() < deadline, "the example did not end");
            std::thread::sleep(Duration::from_millis(50));
        };
        // Its output ended with it: the thread sends what is left, then stops.
        let rest = self.lines.iter().collect();
        (status.success(), rest)
    }

    /// Asks its window to close, as its close button does.
    fn close(&self) {
        let window = window_of(self.pid());
        // SAFETY: the example's window, given a message without pointers.
        let posted = unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
        assert_ne!(posted, 0, "WM_CLOSE was not posted");
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
fn the_application_rests_wakes_for_the_mouse_and_closes_when_asked() {
    let example = Example::start("rest-application", &[]);
    let window = window_of(example.pid());
    if let Err(error) = example.at_rest() {
        panic!("the example is not at rest after its start: {error}");
    }
    assert!(example.frames() > 0, "the example's trace holds no frame");

    let before = example.frames();
    move_mouse(on_screen(window, ADD));
    let hovered = example.drawn_after(before, "the mouse on the button");
    println!("hover: drawn in {} ms", hovered.as_millis());
    if let Err(error) = example.at_rest() {
        panic!("the example is not at rest after the mouse hovered its button: {error}");
    }
    assert!(
        traced(&example.read_trace(), "input to present") > 0,
        "the hover's frames are in no input to present of the trace"
    );

    example.close();
    assert_eq!(
        example.ended(),
        (
            true,
            ["closing asked", "stopped", "dropped"]
                .map(str::to_owned)
                .to_vec()
        ),
        "the example was not asked to close, then stopped, with success"
    );
}

#[test]
fn ctrl_break_asks_the_application_to_close() {
    let example = Example::start("ctrl-break", &[]);
    window_of(example.pid());
    // SAFETY: the example's own process group, which `spawn` made.
    let sent = unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, example.pid()) };
    assert_ne!(
        sent,
        0,
        "Ctrl+Break was not sent: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(
        example.ended(),
        (
            true,
            ["closing asked", "stopped", "dropped"]
                .map(str::to_owned)
                .to_vec()
        ),
        "the example was not asked to close by Ctrl+Break"
    );
}

#[test]
fn the_application_draws_again_after_its_device_and_its_surface_are_lost() {
    let mut example = Example::start("losses", &["--commands"]);
    window_of(example.pid());
    if let Err(error) = example.at_rest() {
        panic!("the example is not at rest after its start: {error}");
    }
    for (command, taken, name) in [
        ("lose device", "losing the device", "lose device"),
        ("lose surface", "losing the surface", "lost"),
    ] {
        let before = example.frames();
        example.command(command, taken);
        let took = example.drawn_after(before, command);
        println!("{name}: drawn again in {} ms", took.as_millis());
        if let Err(error) = example.at_rest() {
            panic!("the example is not at rest after `{command}`: {error}");
        }
    }
    example.close();
    let (success, _) = example.ended();
    assert!(success, "the example did not end with success");
}

#[test]
fn the_blank_window_presents_its_first_image_and_ends_when_asked() {
    let example = Example::spawn("blank", "blank", &[]);
    window_of(example.pid());
    let deadline = Instant::now() + SETTLE;
    while example.frames() == 0 {
        assert!(
            Instant::now() < deadline,
            "the blank window traced no frame within {SETTLE:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    example.close();
    let (success, _) = example.ended();
    assert!(success, "the blank window did not end with status 0");
}
