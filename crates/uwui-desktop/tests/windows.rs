//! The `application` example on Windows, on the desktop that the session shows: it starts, draws
//! and rests, none of its threads running in 10 s (`docs/adr/0185-rest-checked-in-ci.md`); the
//! mouse put on its button *Add*, through `SendInput`, hovers it, which draws frames, and it rests
//! again once the hover's transition ends, its cursor the arrow there and the I-beam over its
//! field, its title its own; resized by its border, it draws while the button is held, within
//! Win32's modal loop of the resize, down to its smallest size and no smaller, and Ctrl+Break then
//! ends the run, the button still held; asked to close by the message that the window's close
//! button sends, `WM_CLOSE`, it ends cleanly, as it does on Ctrl+Break, Windows' termination signal
//! of a console's program.
//!
//! A second test loses the example's GPU device on purpose, then its surface, through its commands
//! on its standard input, and finds frames drawn after each, then the example at rest; it writes how
//! long each took, `lose device: drawn again in 120 ms`. The last two run the blank window,
//! `examples/blank.rs`, which presents its first image, writes it in its trace, and ends with
//! status 0 when asked to close; with `--fullscreen`, its window covers its display, without a
//! frame.
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

use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, LocalFree, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NEW_PROCESS_GROUP, GetThreadDescription, OpenThread, THREAD_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::System::WindowsProgramming::QueryThreadCycleTime;
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CURSORINFO, EnumWindows, GWL_STYLE, GetClientRect, GetCursorInfo, GetSystemMetrics,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IDC_ARROW,
    IDC_IBEAM, IsWindowVisible, LoadCursorW, PostMessageW, SM_CXSCREEN, SM_CYSCREEN,
    SPI_GETDRAGFULLWINDOWS, SPI_SETDRAGFULLWINDOWS, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    SetWindowPos, SystemParametersInfoW, WM_CLOSE, WS_CAPTION, WS_POPUP,
};
use windows_sys::core::{BOOL, PCWSTR, PWSTR};

/// How long the program is watched: the quality bar's window of rest.
const REST: Duration = Duration::from_secs(10);
/// How long it must stay still before it is watched.
const STILL: Duration = Duration::from_secs(1);
/// How long it may take to come to rest: its first frames, on WARP, a GPU of the CPU.
const SETTLE: Duration = Duration::from_secs(60);

/// Where the *Add* button's middle lies in the window, in logical pixels: the bar's padding, then
/// half the button, `examples/application.rs`.
const ADD: [f64; 2] = [12.0 + 44.0, 8.0 + 14.0];

/// Where the field's middle lies: past *Add* and *Remove* and their gaps, then half the field.
const FIELD: [f64; 2] = [12.0 + 88.0 + 8.0 + 88.0 + 8.0 + 100.0, 8.0 + 14.0];

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
    // A process that runs has a thread at least: none read is a read that failed, which would let
    // any two reads match.
    assert!(!threads.is_empty(), "no thread of the process {pid} read");
    threads
}

/// The name of the thread `tid`, as `SetThreadDescription` gave it; `?` for one without.
fn thread_name(tid: u32) -> String {
    // SAFETY: a thread's id; the handle, if any, is closed below.
    let thread = unsafe { OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, tid) };
    if thread.is_null() {
        return "?".to_owned();
    }
    let mut description: PWSTR = std::ptr::null_mut();
    // SAFETY: the live thread's handle, and a live pointer that the call sets to a string of its
    // own, freed below.
    let read = unsafe { GetThreadDescription(thread, &raw mut description) };
    let mut name = String::new();
    if read >= 0 && !description.is_null() {
        // SAFETY: the call's string, ended by a null.
        let length = (0..)
            .take_while(|&at| unsafe { *description.add(at) } != 0)
            .count();
        // SAFETY: as above, `length` units before its null.
        name = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(description, length) });
        // SAFETY: the string that the call allocated.
        unsafe { LocalFree(description.cast()) };
    }
    // SAFETY: the handle opened above.
    unsafe { CloseHandle(thread) };
    if name.is_empty() {
        "?".to_owned()
    } else {
        name
    }
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
    send_mouse(dx, dy, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE);
}

/// Presses the mouse's left button where it lies, or releases it.
fn left_button(pressed: bool) {
    let flags = if pressed {
        MOUSEEVENTF_LEFTDOWN
    } else {
        MOUSEEVENTF_LEFTUP
    };
    send_mouse(0, 0, flags);
}

/// Sends one input of the mouse, as its driver would.
fn send_mouse(dx: i32, dy: i32, flags: MOUSE_EVENT_FLAGS) {
    assert!(mouse_input(dx, dy, flags), "the mouse's input was not sent");
}

/// Sends one input of the mouse: whether Windows took it.
fn mouse_input(dx: i32, dy: i32, flags: MOUSE_EVENT_FLAGS) -> bool {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let size = i32::try_from(size_of::<INPUT>()).expect("a small structure");
    // SAFETY: one live input, of its size.
    let sent = unsafe { SendInput(1, &raw const input, size) };
    sent == 1
}

/// The window's rectangle on the screen, its invisible borders of resizing included.
fn window_rect(window: HWND) -> RECT {
    let mut rect = RECT::default();
    // SAFETY: the live window, and a live rectangle that the call fills.
    unsafe { GetWindowRect(window, &raw mut rect) };
    rect
}

/// The width of the window's client area, in pixels.
fn client_width(window: HWND) -> i32 {
    let mut rect = RECT::default();
    // SAFETY: the live window, and a live rectangle that the call fills.
    unsafe { GetClientRect(window, &raw mut rect) };
    rect.right - rect.left
}

/// The window's title.
fn window_title(window: HWND) -> String {
    let mut title = [0_u16; 256];
    // SAFETY: the live window, and a live buffer of the length given.
    let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), 256) };
    String::from_utf16_lossy(&title[..usize::try_from(length).unwrap_or(0)])
}

/// Whether the cursor that the screen shows is the system's `name`.
fn cursor_is(name: PCWSTR) -> bool {
    let mut info = CURSORINFO {
        cbSize: u32::try_from(size_of::<CURSORINFO>()).expect("a small structure"),
        ..Default::default()
    };
    // SAFETY: a live structure whose size its field says; the system's cursor, shared.
    unsafe {
        GetCursorInfo(&raw mut info) != 0 && info.hCursor == LoadCursorW(std::ptr::null_mut(), name)
    }
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
        // The test's own process counts in physical pixels, as the example's window does, so that
        // a display scaled above 100 % moves no point.
        // SAFETY: a constant context that Windows defines; a second call is refused, harmlessly.
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
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
        println!("at rest: {} threads watched", before.len());
        // Read before the watch, so that a thread that ends during it is named.
        let names: BTreeMap<u32, String> =
            before.keys().map(|&tid| (tid, thread_name(tid))).collect();
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
                let name = names.get(tid).map_or("new", String::as_str);
                format!("{tid} {name:?}: {} cycles", cycles.saturating_sub(was))
            })
            .collect();
        let gone: Vec<String> = before
            .keys()
            .filter(|tid| !after.contains_key(tid))
            .map(|tid| format!("{tid} {:?}", names[tid]))
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

    assert_eq!(
        window_title(window),
        "uwui application",
        "the window's title"
    );

    let before = example.frames();
    move_mouse(on_screen(window, ADD));
    let hovered = example.drawn_after(before, "the mouse on the button");
    assert!(
        within_two_seconds(|| cursor_is(IDC_ARROW)),
        "the cursor over the button is not the arrow"
    );
    println!("hover: drawn in {} ms", hovered.as_millis());
    if let Err(error) = example.at_rest() {
        panic!("the example is not at rest after the mouse hovered its button: {error}");
    }
    assert!(
        traced(&example.read_trace(), "input to present") > 0,
        "the hover's frames are in no input to present of the trace"
    );
    move_mouse(on_screen(window, FIELD));
    assert!(
        within_two_seconds(|| cursor_is(IDC_IBEAM)),
        "the cursor over the field is not the I-beam"
    );
    if let Err(error) = example.at_rest() {
        panic!("the example is not at rest with the mouse over its field: {error}");
    }

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

/// Whether Windows shows a window's content while the person drags its border, rather than an
/// outline that resizes it at the release: the system's setting, on by default on Windows 10 and
/// 11. A test that asks it, `UWUI_TEST_SETS_DRAG_FULL_WINDOWS=1`, as CI's throwaway runner does,
/// turns it on for the session, unsaved; elsewhere it is never changed.
fn drag_full_windows() -> bool {
    let mut on: BOOL = 0;
    // SAFETY: a read of the setting into a live `BOOL`.
    unsafe {
        SystemParametersInfoW(SPI_GETDRAGFULLWINDOWS, 0, (&raw mut on).cast(), 0);
    }
    if on != 0 {
        return true;
    }
    if std::env::var_os("UWUI_TEST_SETS_DRAG_FULL_WINDOWS").is_some_and(|value| value == "1") {
        // SAFETY: the setting, turned on for the session alone: no profile is written.
        let set =
            unsafe { SystemParametersInfoW(SPI_SETDRAGFULLWINDOWS, 1, std::ptr::null_mut(), 0) };
        println!("drag full windows: off, turned on for the session");
        return set != 0;
    }
    false
}

/// Waits until `done` holds, for at most 2 s: whether it did.
fn within_two_seconds(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !done() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

/// The mouse's left button, held while this lives: released however the test ends, so that a
/// failure leaves no button down for the tests after it.
struct Held {
    /// Where the mouse lies.
    at: [i32; 2],
}

impl Drop for Held {
    fn drop(&mut self) {
        // Sent without asserting: a refused input during a panic's unwinding would abort the run.
        let _ = mouse_input(0, 0, MOUSEEVENTF_LEFTUP);
    }
}

/// Moves `window` to the screen's top left, so that its right border lies on the screen, whose
/// width CI's runner keeps at 1024 pixels, puts the mouse on that border, in its invisible band of
/// resizing, halfway down, waits until `example` is still, and presses the left button there.
fn hold_right_border(example: &Example, window: HWND) -> Held {
    assert!(
        drag_full_windows(),
        "Windows shows an outline, not the window, while a border is dragged: turn on \"Show \
         window contents while dragging\", or set UWUI_TEST_SETS_DRAG_FULL_WINDOWS=1 on a \
         throwaway machine"
    );
    // SAFETY: the example's live window, moved and neither sized nor raised.
    let moved = unsafe {
        SetWindowPos(
            window,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
    assert_ne!(moved, 0, "the window was not moved");
    let rect = window_rect(window);
    let grip = [rect.right - 3, (rect.top + rect.bottom) / 2];
    move_mouse(grip);
    if let Err(error) = example.settled() {
        panic!("the example is not still with the mouse on its border: {error}");
    }
    left_button(true);
    std::thread::sleep(Duration::from_millis(200));
    Held { at: grip }
}

/// Drags the mouse, its button held, `by` pixels across from `from`, in six steps.
fn drag(from: [i32; 2], by: i32) -> [i32; 2] {
    for step in 1..=6 {
        move_mouse([from[0] + by * step / 6, from[1]]);
        std::thread::sleep(Duration::from_millis(100));
    }
    [from[0] + by, from[1]]
}

#[test]
fn the_application_draws_while_the_person_resizes_its_window_down_to_its_smallest() {
    let example = Example::start("resize", &[]);
    let window = window_of(example.pid());
    let width = client_width(window);
    let held = hold_right_border(&example, window);
    let grip = held.at;
    // Counted once the example is still and the button pressed: only frames that the drag draws
    // follow.
    let before = example.frames();
    let at = drag(grip, -60);
    // The button still held: Win32's own modal loop of the resize runs, and the host draws only
    // where the layer turns it from there.
    let resized = within_two_seconds(|| client_width(window) < width);
    let drew = within_two_seconds(|| example.frames() > before);
    let during = example.frames();
    // Far past the example's smallest width, 480 logical pixels, at any scale.
    // SAFETY: the example's live window.
    let scale = f64::from(unsafe { GetDpiForWindow(window) }.max(1)) / 96.0;
    let smallest = (480.0 * scale).round() as i32;
    drag(at, -(client_width(window) - smallest + 100));
    let held_at_smallest = within_two_seconds(|| client_width(window) == smallest);
    drop(held);
    assert!(
        resized,
        "the border's drag from {grip:?} did not resize the window, {} pixels wide",
        client_width(window)
    );
    assert!(
        drew,
        "no frame was drawn while the border was held: {before} frames before, {during} during"
    );
    assert!(
        held_at_smallest,
        "the window is {} pixels wide, not its smallest, {smallest}",
        client_width(window)
    );
    println!("resized: {} frames during the drag", during - before);
    // Off the border, where Windows showed its arrows of resizing, onto the window's content: the
    // window's own cursor again, which its procedure sets.
    move_mouse(on_screen(window, ADD));
    assert!(
        within_two_seconds(|| cursor_is(IDC_ARROW)),
        "the cursor stayed Windows' own after the border"
    );
    if let Err(error) = example.at_rest() {
        panic!("the example is not at rest after its resize: {error}");
    }
    example.close();
    let (success, _) = example.ended();
    assert!(success, "the example did not end with success");
}

#[test]
fn ctrl_break_during_a_resize_ends_the_run_with_the_border_still_held() {
    let example = Example::start("ctrl-break-in-resize", &[]);
    let window = window_of(example.pid());
    let width = client_width(window);
    let held = hold_right_border(&example, window);
    drag(held.at, -30);
    assert!(
        within_two_seconds(|| client_width(window) < width),
        "the border's drag did not resize the window"
    );
    // SAFETY: the example's own process group, which `spawn` made.
    let sent = unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, example.pid()) };
    assert_ne!(sent, 0, "Ctrl+Break was not sent");
    // Win32's modal loop of the resize, which the run's end ends, the button still held.
    let ended = example.ended();
    drop(held);
    assert_eq!(
        ended,
        (
            true,
            ["closing asked", "stopped", "dropped"]
                .map(str::to_owned)
                .to_vec()
        ),
        "Ctrl+Break during the resize did not end the run"
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

#[test]
fn the_blank_window_opens_filling_its_display_without_borders() {
    let example = Example::spawn("blank", "blank-fullscreen", &["--fullscreen"]);
    let window = window_of(example.pid());
    let deadline = Instant::now() + SETTLE;
    while example.frames() == 0 {
        assert!(
            Instant::now() < deadline,
            "the blank window traced no frame within {SETTLE:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut monitor = MONITORINFO {
        cbSize: u32::try_from(size_of::<MONITORINFO>()).expect("a small structure"),
        ..Default::default()
    };
    // SAFETY: the live window's monitor, and a live structure whose size its field says.
    unsafe {
        GetMonitorInfoW(
            MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
            &raw mut monitor,
        )
    };
    let rect = window_rect(window);
    let area = monitor.rcMonitor;
    assert_eq!(
        [rect.left, rect.top, rect.right, rect.bottom],
        [area.left, area.top, area.right, area.bottom],
        "the window does not cover its display"
    );
    // SAFETY: the live window's style.
    let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) } as u32;
    assert!(
        style & WS_POPUP != 0 && style & WS_CAPTION == 0,
        "the window keeps a frame: its style {style:#x}"
    );
    example.close();
    let (success, _) = example.ended();
    assert!(success, "the blank window did not end with status 0");
}
