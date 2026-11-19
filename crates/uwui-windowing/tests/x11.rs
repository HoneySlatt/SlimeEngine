//! X11's layer under an X server of the test's own, without a window manager, as Xvfb gives:
//! what another client sees of the window, and what the layer tells its handler. `test = false`:
//! it needs the server that `DISPLAY` names, never the person's own, and runs in the series' column
//! of X11 (`scripts/compositors.sh x11`), with `DESKTOP_STARTUP_ID` set to a start's id that holds
//! a quote and a backslash, and `WAYLAND_DISPLAY` unset, one test at a time (`--test-threads=1`):
//! one covers the whole screen. Its last test runs its first again with
//! `WAYLAND_DISPLAY` naming no compositor.

#![cfg(target_os = "linux")]

use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use raw_window_handle::RawWindowHandle;
use uwui_windowing::{Configured, Control, Drawn, Handler, Happened, Stopped, Window};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _, CreateWindowAux,
    EventMask, WindowClass,
};

/// What the layer told, turn by turn.
#[derive(Default, Debug)]
struct Told {
    configured: Vec<Configured>,
    /// Each signal's time, in nanoseconds of the host's clock, with the moment it was read.
    signals: Vec<(u64, Instant)>,
    exposed: usize,
    turns: usize,
}

/// Records what comes for `run_for`, asking a signal at its first turn and again at each signal
/// when `frames`; sends the window's id as it opens.
struct Recorder {
    told: Told,
    frames: bool,
    run_for: Duration,
    window_id: Option<Sender<u32>>,
    started: Option<Instant>,
}

impl Recorder {
    fn new(run_for: Duration, frames: bool, window_id: Option<Sender<u32>>) -> Self {
        Self {
            told: Told::default(),
            frames,
            run_for,
            window_id,
            started: None,
        }
    }
}

impl Handler for Recorder {
    fn opened(&mut self, window: &mut Window<'_>, _: Drawn) -> Result<(), Stopped> {
        if let (Some(sender), (_, RawWindowHandle::Xcb(handle))) =
            (&self.window_id, window.raw_handles())
        {
            let _ = sender.send(handle.window.get());
        }
        Ok(())
    }

    fn turn(&mut self, window: &mut Window<'_>, happened: Happened) -> Control {
        self.told.turns += 1;
        let started = *self.started.get_or_insert_with(Instant::now);
        if let Some(configured) = happened.configured {
            self.told.configured.push(configured);
        }
        if happened.exposed {
            self.told.exposed += 1;
        }
        let first = self.told.turns == 1;
        if let Some(signal) = happened.signal {
            self.told.signals.push((signal.as_nanos(), Instant::now()));
        }
        if self.frames && (first || happened.signal.is_some()) {
            let _ = window.ask_signal();
        }
        if started.elapsed() >= self.run_for {
            return Control::Exit;
        }
        Control::Wait(Some(window.now() + Duration::from_millis(20)))
    }

    fn closing(&mut self) {}
}

/// Waits for the server to have let go of the connections that the test before closed: Xvfb
/// 21.1.24 refuses a connection made just as another closes, about once in such a turn, which a
/// tenth of a second avoids.
fn server_settles() {
    std::thread::sleep(Duration::from_millis(100));
}

fn opening(fullscreen: bool) -> uwui_windowing::Opening {
    uwui_windowing::Opening {
        title: "windowing test".to_owned(),
        app_id: "dev.uwui.WindowingTest".to_owned(),
        size: [320, 240],
        min_size: None,
        fullscreen,
    }
}

/// A window opened filling the screen where no window manager runs, as under Xvfb: the layer's
/// own hint, written before the map, must not come back as the window manager's state.
#[test]
fn without_a_window_manager_the_window_is_not_told_filling_the_screen() {
    server_settles();
    let (recorder, ran) = uwui_windowing::run(
        opening(true),
        Recorder::new(Duration::from_millis(700), false, None),
    );
    ran.expect("the run");
    let told = &recorder.told;
    eprintln!("configured, in order: {:?}", told.configured);
    let last = told.configured.last().expect("a configure");
    assert_eq!(last.size, [320, 240], "no window manager resized it");
    assert!(
        !last.fullscreen,
        "the host is told that the window fills the screen while it stays 320 x 240: {:?}",
        told.configured
    );
}

/// A window manager's `_NET_WM_PING`, sent to the window as window managers send it: the answer
/// must reach the root window as a ClientMessage (EWMH).
#[test]
fn a_ping_is_answered_on_the_root_window() {
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel();
    let (ready_sender, ready_receiver) = mpsc::channel();
    let pinger = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let protocols = connection
            .intern_atom(false, b"WM_PROTOCOLS")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let ping = connection
            .intern_atom(false, b"_NET_WM_PING")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        connection
            .change_window_attributes(
                root,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY),
            )
            .unwrap()
            .check()
            .unwrap();
        ready_sender.send(()).unwrap();
        let window: u32 = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        // As window managers send it: to the client's window, with no event mask.
        let event = ClientMessageEvent::new(32, window, protocols, [ping, 4242, window, 0, 0]);
        connection
            .send_event(false, window, EventMask::NO_EVENT, event)
            .unwrap()
            .check()
            .unwrap();
        connection.flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Some(event) = connection.poll_for_event().unwrap() {
                if let Event::ClientMessage(message) = event
                    && message.type_ == protocols
                {
                    let data = message.data.as_data32();
                    if data[0] == ping {
                        return Some((message.window, data[1], data[2], message.response_type));
                    }
                }
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        None
    });
    ready_receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the pinger");
    let (_, ran) = uwui_windowing::run(
        opening(false),
        Recorder::new(Duration::from_millis(1500), false, Some(window_sender)),
    );
    ran.expect("the run");
    let answer = pinger.join().unwrap();
    eprintln!("answer: {answer:?}");
    let (to, timestamp, window, response_type) =
        answer.expect("no _NET_WM_PING answer reached the root window");
    assert_eq!(timestamp, 4242);
    assert_ne!(
        to, window,
        "the answer names the root window, not the client's"
    );
    assert_eq!(response_type & 0x7f, 33, "a ClientMessage");
}

/// The startup notification's removal message, as a launcher reassembles it from the root
/// window's ClientMessages: run with `DESKTOP_STARTUP_ID` set by the command.
#[test]
fn the_removal_message_reassembles_to_the_quoted_id() {
    server_settles();
    let id = std::env::var("DESKTOP_STARTUP_ID").expect("run with DESKTOP_STARTUP_ID set");
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (ready_sender, ready_receiver) = mpsc::channel();
    let launcher = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let atom = |name: &[u8]| {
            connection
                .intern_atom(false, name)
                .unwrap()
                .reply()
                .unwrap()
                .atom
        };
        let begin = atom(b"_NET_STARTUP_INFO_BEGIN");
        let more = atom(b"_NET_STARTUP_INFO");
        let property = atom(b"_NET_STARTUP_ID");
        connection
            .change_window_attributes(
                root,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            )
            .unwrap()
            .check()
            .unwrap();
        ready_sender.send(()).unwrap();
        let mut message = Vec::new();
        let mut chunks = 0;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let Some(event) = connection.poll_for_event().unwrap() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            if let Event::ClientMessage(event) = event
                && (event.type_ == begin || event.type_ == more)
            {
                if event.type_ == begin {
                    message.clear();
                }
                chunks += 1;
                let data = event.data.as_data8();
                let end = data.iter().position(|&byte| byte == 0);
                message.extend_from_slice(&data[..end.unwrap_or(20)]);
                if end.is_some() {
                    break;
                }
            }
        }
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let value = connection
            .get_property(false, window, property, AtomEnum::ANY, 0, 1024)
            .unwrap()
            .reply()
            .unwrap()
            .value;
        (
            String::from_utf8_lossy(&message).into_owned(),
            chunks,
            value,
        )
    });
    ready_receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the launcher");
    let (_, ran) = uwui_windowing::run(
        opening(false),
        Recorder::new(Duration::from_millis(1000), false, Some(window_sender)),
    );
    ran.expect("the run");
    let (message, chunks, property) = launcher.join().unwrap();
    eprintln!("message: {message:?} in {chunks} chunks; _NET_STARTUP_ID: {property:?}");
    let mut quoted = String::from("\"");
    for character in id.chars() {
        if character == '"' || character == '\\' {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    assert_eq!(message, format!("remove: ID={quoted}"));
    assert_eq!(property, id.as_bytes());
}

/// The signal while another client unmaps the window, then covers it whole with a window of its
/// own: held while hidden, never lost for good; the window exposed again once uncovered.
#[test]
fn the_signal_waits_while_hidden_and_comes_back() {
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let start = Instant::now();
    let other = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let setup_screen = &connection.setup().roots[screen];
        let (root, width, height) = (
            setup_screen.root,
            setup_screen.width_in_pixels,
            setup_screen.height_in_pixels,
        );
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let mut marks = Vec::new();
        std::thread::sleep(Duration::from_millis(400));
        connection.unmap_window(window).unwrap();
        connection.flush().unwrap();
        marks.push(("unmapped", start.elapsed()));
        std::thread::sleep(Duration::from_millis(500));
        connection.map_window(window).unwrap();
        connection.flush().unwrap();
        marks.push(("mapped", start.elapsed()));
        std::thread::sleep(Duration::from_millis(500));
        let cover = connection.generate_id().unwrap();
        connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                cover,
                root,
                0,
                0,
                width,
                height,
                0,
                WindowClass::INPUT_OUTPUT,
                x11rb::COPY_FROM_PARENT,
                &CreateWindowAux::new()
                    .override_redirect(1)
                    .background_pixel(0),
            )
            .unwrap();
        connection.map_window(cover).unwrap();
        connection.flush().unwrap();
        marks.push(("covered", start.elapsed()));
        std::thread::sleep(Duration::from_millis(500));
        connection.destroy_window(cover).unwrap();
        connection.flush().unwrap();
        marks.push(("uncovered", start.elapsed()));
        marks
    });
    let (recorder, ran) = uwui_windowing::run(
        opening(false),
        Recorder::new(Duration::from_millis(2600), true, Some(window_sender)),
    );
    ran.expect("the run");
    let marks = other.join().unwrap();
    let read: Vec<Duration> = recorder
        .told
        .signals
        .iter()
        .map(|(_, at)| at.duration_since(start))
        .collect();
    let mark = |name: &str| marks.iter().find(|(m, _)| *m == name).unwrap().1;
    let between =
        |from: Duration, to: Duration| read.iter().filter(|at| **at > from && **at < to).count();
    let (unmapped, mapped, covered, uncovered) = (
        mark("unmapped"),
        mark("mapped"),
        mark("covered"),
        mark("uncovered"),
    );
    eprintln!(
        "marks {marks:?}; signals: before {}, unmapped {}, mapped {}, covered {}, uncovered {}; exposed {}",
        between(Duration::ZERO, unmapped),
        between(unmapped + Duration::from_millis(50), mapped),
        between(mapped, covered),
        between(covered + Duration::from_millis(50), uncovered),
        between(uncovered, Duration::from_secs(60)),
        recorder.told.exposed
    );
    assert!(between(Duration::ZERO, unmapped) > 5, "signals while shown");
    assert_eq!(
        between(unmapped + Duration::from_millis(50), mapped),
        0,
        "held while unmapped"
    );
    assert!(between(mapped, covered) > 5, "signals again once mapped");
    assert_eq!(
        between(covered + Duration::from_millis(50), uncovered),
        0,
        "held while covered"
    );
    assert!(
        between(uncovered, Duration::from_secs(60)) > 5,
        "signals again once uncovered"
    );
    assert!(recorder.told.exposed >= 1, "exposed once uncovered");
}

/// Where `WAYLAND_DISPLAY` names a compositor that does not answer, the window opens on the X
/// server that `DISPLAY` names (ADR-0311): this test's own binary, run again with such a name, opens
/// its window there.
#[test]
fn a_wayland_compositor_that_does_not_answer_leaves_the_window_to_x11() {
    server_settles();
    let test = std::env::current_exe().expect("the test's path");
    let ran = std::process::Command::new(test)
        .args([
            "--exact",
            "without_a_window_manager_the_window_is_not_told_filling_the_screen",
        ])
        .env("WAYLAND_DISPLAY", "uwui-no-compositor-answers-here")
        .output()
        .expect("the test runs again");
    let output = String::from_utf8_lossy(&ran.stdout);
    assert!(
        ran.status.success() && output.contains("1 passed"),
        "the window did not open on X11: {output}{}",
        String::from_utf8_lossy(&ran.stderr)
    );
}
