//! X11's layer under an X server of the test's own, without a window manager, as Xvfb gives, or
//! with one played by another client: what another client sees of the window, and what the layer
//! tells its handler. `test = false`:
//! it needs the server that `DISPLAY` names, never the person's own, and runs in the series' column
//! of X11 (`scripts/compositors.sh x11`), with `DESKTOP_STARTUP_ID` set to a start's id that holds
//! a quote and a backslash, and `WAYLAND_DISPLAY` unset, one test at a time (`--test-threads=1`):
//! one covers the whole screen. One test runs another again with `WAYLAND_DISPLAY` naming no
//! compositor.

#![cfg(target_os = "linux")]

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use raw_window_handle::RawWindowHandle;
use uwui_time::Instant as HostInstant;
use uwui_windowing::{
    Configured, Control, Drawn, Handler, Happened, Presentation, Stopped, Window,
};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::present::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConfigureWindowAux,
    ConnectionExt as _, CreateWindowAux, EventMask, PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

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

enum ToDriver {
    Window(u32),
    Asked(u64),
}

struct Host {
    to_driver: Sender<ToDriver>,
    ready: Receiver<()>,
    asked: Vec<(u64, HostInstant)>,
    presentations: Vec<Presentation>,
    sizes: Vec<[u32; 2]>,
    started: Option<Instant>,
    asked_after_resize: bool,
}

impl Host {
    fn ask(&mut self, window: &mut Window<'_>) {
        let image = window.ask_signal();
        self.asked.push((image, window.now()));
        let _ = self.to_driver.send(ToDriver::Asked(image));
    }
}

impl Handler for Host {
    fn opened(&mut self, window: &mut Window<'_>, _: Drawn) -> Result<(), Stopped> {
        if let (_, RawWindowHandle::Xcb(handle)) = window.raw_handles() {
            let _ = self.to_driver.send(ToDriver::Window(handle.window.get()));
        }
        Ok(())
    }

    fn turn(&mut self, window: &mut Window<'_>, happened: Happened) -> Control {
        let started = *self.started.get_or_insert_with(Instant::now);
        self.presentations
            .extend(happened.presentations.iter().copied());
        if let Some(configured) = happened.configured {
            self.sizes.push(configured.size);
            // The new size drawn: the next image asked, as a host does.
            if configured.size != [320, 240] && !self.asked_after_resize {
                self.asked_after_resize = true;
                self.ask(window);
            }
        }
        if self.ready.try_recv().is_ok() {
            self.ask(window);
        }
        if started.elapsed() >= Duration::from_millis(2000) {
            return Control::Exit;
        }
        Control::Wait(Some(window.now() + Duration::from_millis(5)))
    }

    fn closing(&mut self) {}
}

/// The driver's side: what it presented, and the notices it saw, each (kind, serial, msc).
fn drive(from_host: Receiver<ToDriver>, ready: Sender<()>) -> Vec<(String, u32, u64)> {
    let (connection, screen) = x11rb::connect(None).expect("the X server");
    let setup = &connection.setup().roots[screen];
    let (root, depth) = (setup.root, setup.root_depth);
    let Ok(ToDriver::Window(window)) = from_host.recv_timeout(Duration::from_secs(10)) else {
        panic!("the window's id");
    };
    let eid = connection.generate_id().unwrap();
    connection
        .present_select_input(eid, window, present::EventMask::COMPLETE_NOTIFY)
        .unwrap()
        .check()
        .unwrap();
    let pixmap = connection.generate_id().unwrap();
    connection
        .create_pixmap(depth, pixmap, root, 320, 240)
        .unwrap()
        .check()
        .unwrap();
    let mut seen = Vec::new();
    // The current refresh's count, from a NotifyMSC of the probe's own that fires at once.
    let current_msc = |serial: u32, seen: &mut Vec<(String, u32, u64)>| -> u64 {
        connection
            .present_notify_msc(window, serial, 0, 0, 0)
            .unwrap();
        connection.flush().unwrap();
        loop {
            if let Event::PresentCompleteNotify(notice) = connection.wait_for_event().unwrap() {
                seen.push((format!("{:?}", notice.kind), notice.serial, notice.msc));
                if notice.kind == present::CompleteKind::NOTIFY_MSC && notice.serial == serial {
                    return notice.msc;
                }
            }
        }
    };
    std::thread::sleep(Duration::from_millis(300));
    ready.send(()).unwrap();
    let Ok(ToDriver::Asked(first)) = from_host.recv_timeout(Duration::from_secs(10)) else {
        panic!("the first image asked");
    };
    // The first image, presented three refreshes ahead; then the window resized at once, as a
    // window manager does, while that present waits in the server.
    let msc = current_msc(9001, &mut seen);
    connection
        .present_pixmap(
            window,
            pixmap,
            first as u32,
            0u32,
            0u32,
            0,
            0,
            0u32,
            0u32,
            0u32,
            0,
            msc + 3,
            0,
            0,
            &[],
        )
        .unwrap();
    connection
        .configure_window(window, &ConfigureWindowAux::new().width(400).height(300))
        .unwrap();
    connection.flush().unwrap();
    let Ok(ToDriver::Asked(second)) = from_host.recv_timeout(Duration::from_secs(10)) else {
        panic!("the image asked after the resize");
    };
    // The image drawn at the new size, presented thirty refreshes ahead: half a second.
    let msc = current_msc(9002, &mut seen);
    connection
        .present_pixmap(
            window,
            pixmap,
            second as u32,
            0u32,
            0u32,
            0,
            0,
            0u32,
            0u32,
            0u32,
            0,
            msc + 30,
            0,
            0,
            &[],
        )
        .unwrap();
    connection.flush().unwrap();
    let deadline = Instant::now() + Duration::from_millis(1200);
    while Instant::now() < deadline {
        match connection.poll_for_event().unwrap() {
            Some(Event::PresentCompleteNotify(notice)) => {
                seen.push((format!("{:?}", notice.kind), notice.serial, notice.msc))
            }
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    seen
}

/// A present still waiting in the server as the window's size changes: Present tells it after the
/// ConfigureNotify, as the server executes every present it took at its target refresh. This probe
/// plays the driver from a connection of its own (Present's notices go to every client that asks
/// them, the layer's included), and the host, which asks one image before the resize and one after.
#[test]
fn a_present_in_flight_across_a_resize_is_not_told_for_the_next_image() {
    server_settles();
    let (to_driver, from_host) = mpsc::channel();
    let (ready_sender, ready) = mpsc::channel();
    let driver = std::thread::spawn(move || drive(from_host, ready_sender));
    let host = Host {
        to_driver,
        ready,
        asked: Vec::new(),
        presentations: Vec::new(),
        sizes: Vec::new(),
        started: None,
        asked_after_resize: false,
    };
    let (host, ran) = uwui_windowing::run(
        uwui_windowing::Opening {
            title: "windowing test".to_owned(),
            app_id: "dev.uwui.WindowingTest".to_owned(),
            size: [320, 240],
            min_size: None,
            fullscreen: false,
        },
        host,
    );
    ran.expect("the run");
    let seen = driver.join().unwrap();
    let asked: Vec<(u64, u64)> = host
        .asked
        .iter()
        .map(|(image, at)| (*image, at.as_nanos()))
        .collect();
    let told: Vec<(u64, Option<u64>)> = host
        .presentations
        .iter()
        .map(|told| (told.image, told.shown.map(|at| at.as_nanos())))
        .collect();
    eprintln!("sizes told: {:?}", host.sizes);
    eprintln!("asked (image, host ns): {asked:?}");
    eprintln!("told (image, shown host ns): {told:?}");
    eprintln!("the driver saw (kind, serial, msc): {seen:?}");
    assert_eq!(
        host.asked.len(),
        2,
        "one image before the resize, one after"
    );
    // Each image is told shown, after it was asked; the one after the resize, presented thirty
    // refreshes ahead, half a second or so later, not at the time of the present before it.
    for (index, (image, asked_at)) in host.asked.iter().enumerate() {
        let shown = host
            .presentations
            .iter()
            .find(|presentation| presentation.image == *image)
            .and_then(|presentation| presentation.shown)
            .unwrap_or_else(|| panic!("image {image} is never told shown: {told:?}"));
        let after = shown.as_nanos().saturating_sub(asked_at.as_nanos());
        assert!(
            shown > *asked_at,
            "image {image} is told shown before it was asked: {told:?}"
        );
        if index == 1 {
            assert!(
                after >= 300_000_000,
                "image {image}, presented thirty refreshes after it was asked, is told shown {} ms \
                 after: the time of the present before the resize",
                after / 1_000_000
            );
        }
    }
}

/// What the handler is told of a window that a window manager of the extended hints, played by
/// another client, manages: it names itself on the root and on a window of its own, then gives the
/// window the states named `states` and a size of 640 × 480. It leaves before the test ends.
fn managed(states: &'static [&'static [u8]]) -> Vec<Configured> {
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let manager = std::thread::spawn(move || {
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
        let (check, state) = (atom(b"_NET_SUPPORTING_WM_CHECK"), atom(b"_NET_WM_STATE"));
        let states: Vec<u32> = states.iter().map(|name| atom(name)).collect();
        let own = connection.generate_id().unwrap();
        connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                own,
                root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_ONLY,
                x11rb::COPY_FROM_PARENT,
                &CreateWindowAux::new(),
            )
            .unwrap();
        for named_on in [root, own] {
            connection
                .change_property32(PropMode::REPLACE, named_on, check, AtomEnum::WINDOW, &[own])
                .unwrap();
        }
        connection.flush().unwrap();
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        std::thread::sleep(Duration::from_millis(300));
        connection
            .change_property32(PropMode::REPLACE, window, state, AtomEnum::ATOM, &states)
            .unwrap();
        connection
            .configure_window(window, &ConfigureWindowAux::new().width(640).height(480))
            .unwrap();
        connection.flush().unwrap();
        std::thread::sleep(Duration::from_millis(700));
        // The manager leaves: the next test runs without one.
        connection.delete_property(root, check).unwrap();
        connection.destroy_window(own).unwrap();
        connection.flush().unwrap();
    });
    let (recorder, ran) = uwui_windowing::run(
        opening(false),
        Recorder::new(Duration::from_millis(1200), false, Some(window_sender)),
    );
    ran.expect("the run");
    manager.join().unwrap();
    let told = recorder.told.configured;
    eprintln!("configured, in order: {told:?}");
    told
}

/// A window manager that fills the screen with the window: the handler is told so, the window's
/// state counting once a window manager runs.
#[test]
fn a_window_manager_that_fills_the_screen_with_the_window_is_believed() {
    let told = managed(&[b"_NET_WM_STATE_FULLSCREEN"]);
    assert_eq!(
        told.last(),
        Some(&Configured {
            size: [640, 480],
            fullscreen: true
        }),
        "the window manager's state and size: {told:?}"
    );
}

/// A window manager that maximizes the window, without filling the screen with it: the handler is
/// not told that it fills the screen.
#[test]
fn a_window_maximized_does_not_fill_the_screen() {
    let told = managed(&[
        b"_NET_WM_STATE_MAXIMIZED_VERT",
        b"_NET_WM_STATE_MAXIMIZED_HORZ",
    ]);
    assert_eq!(
        told.last(),
        Some(&Configured {
            size: [640, 480],
            fullscreen: false
        }),
        "the window manager's state and size: {told:?}"
    );
}
