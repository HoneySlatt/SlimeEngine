//! X11's layer under an X server of the test's own, without a window manager, as Xvfb gives, or
//! with one played by another client: what another client sees of the window, and what the layer
//! tells its handler. `test = false`:
//! it needs the server that `DISPLAY` names, never the person's own, and runs in the series' column
//! of X11 (`scripts/compositors.sh x11`), with `DESKTOP_STARTUP_ID` set to a start's id that holds
//! a quote and a backslash, and `WAYLAND_DISPLAY` unset, one test at a time (`--test-threads=1`):
//! one covers the whole screen. One test runs another again with `WAYLAND_DISPLAY` naming no
//! compositor. The keyboard's tests type through `xdotool`, and compose from the tests' own file,
//! which `XCOMPOSEFILE` names (`tests/compose/Compose`); the scale's set the root window's X
//! resources, and leave them as they found them.

#![cfg(target_os = "linux")]

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use raw_window_handle::RawWindowHandle;
use slime_input::{ButtonState, CursorShape, Key, KeyEvent};
use uwui_time::Instant as HostInstant;
use uwui_windowing::{
    Configured, Control, Drawn, Handler, Happened, Input, Presentation, Stopped, Window,
};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::present::{self, ConnectionExt as _};
use x11rb::protocol::xfixes::ConnectionExt as _;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConfigureWindowAux,
    ConnectionExt as _, CreateWindowAux, EventMask, PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

/// What the layer told, turn by turn.
#[derive(Default, Debug)]
struct Told {
    configured: Vec<Configured>,
    /// How the window drew as it opened, then each change.
    drawn: Vec<Drawn>,
    /// When each change of how it draws was read, after the opening's.
    drawn_at: Vec<Instant>,
    inputs: Vec<Input>,
    /// Each signal's time, in nanoseconds of the host's clock, with the moment it was read.
    signals: Vec<(u64, Instant)>,
    exposed: usize,
    turns: usize,
}

/// Records what comes for `run_for`, asking a signal at its first turn and again at each signal
/// when `frames`; sends the window's id as it opens. Ends sooner when `stop` says so, and shows the
/// cursor's shapes that `shapes` gives as they come.
struct Recorder {
    told: Told,
    frames: bool,
    run_for: Duration,
    window_id: Option<Sender<u32>>,
    started: Option<Instant>,
    stop: Option<Receiver<()>>,
    shapes: Option<Receiver<CursorShape>>,
}

impl Recorder {
    fn new(run_for: Duration, frames: bool, window_id: Option<Sender<u32>>) -> Self {
        Self {
            told: Told::default(),
            frames,
            run_for,
            window_id,
            started: None,
            stop: None,
            shapes: None,
        }
    }
}

impl Handler for Recorder {
    fn opened(&mut self, window: &mut Window<'_>, drawn: Drawn) -> Result<(), Stopped> {
        self.told.drawn.push(drawn);
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
        if let Some(drawn) = happened.drawn {
            self.told.drawn.push(drawn);
            self.told.drawn_at.push(Instant::now());
        }
        self.told
            .inputs
            .extend(happened.inputs.into_iter().map(|(_, input)| input));
        if let Some(shapes) = &self.shapes {
            for shape in shapes.try_iter() {
                window.set_cursor(shape);
            }
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
        let stopped = self
            .stop
            .as_ref()
            .is_some_and(|stop| stop.try_recv().is_ok());
        if stopped || started.elapsed() >= self.run_for {
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
        // A hidden time marked from when the server has hidden the window, to before it is
        // asked to show it again: a server late to read a request, as under load, still sends
        // the refreshes' notices meanwhile, and the signal held while hidden comes as the window
        // shows again.
        connection.unmap_window(window).unwrap().check().unwrap();
        marks.push(("unmapped", start.elapsed()));
        std::thread::sleep(Duration::from_millis(500));
        marks.push(("mapped", start.elapsed()));
        connection.map_window(window).unwrap();
        connection.flush().unwrap();
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
        connection.map_window(cover).unwrap().check().unwrap();
        marks.push(("covered", start.elapsed()));
        std::thread::sleep(Duration::from_millis(500));
        marks.push(("uncovered", start.elapsed()));
        connection.destroy_window(cover).unwrap();
        connection.flush().unwrap();
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

/// Runs `xdotool` with `arguments`, which gives the server's input through XTest.
fn xdotool(arguments: &[&str]) {
    let status = std::process::Command::new("xdotool")
        .args(arguments)
        .status()
        .expect("xdotool runs");
    assert!(status.success(), "xdotool {arguments:?} failed");
}

/// The keycodes of the server's keymap that no keysym is given to, with the keysyms per keycode.
fn free_keycodes(connection: &impl Connection) -> (Vec<u8>, u8) {
    let setup = connection.setup();
    let (first, last) = (setup.min_keycode, setup.max_keycode);
    let mapping = connection
        .get_keyboard_mapping(first, last - first + 1)
        .unwrap()
        .reply()
        .unwrap();
    let per = mapping.keysyms_per_keycode;
    let free = mapping
        .keysyms
        .chunks(usize::from(per))
        .zip(first..=last)
        .filter(|(keysyms, _)| keysyms.iter().all(|&keysym| keysym == 0))
        .map(|(_, keycode)| keycode)
        .collect();
    (free, per)
}

/// Gives `keycode` the keysym `keysym` alone, or none for 0, through the core protocol, which the
/// server tells XKB's clients as a change of its keymap.
fn map_keycode(connection: &impl Connection, keycode: u8, keysym: u32, per: u8) {
    let mut keysyms = vec![0; usize::from(per)];
    keysyms[0] = keysym;
    if per > 1 {
        keysyms[1] = keysym;
    }
    connection
        .change_keyboard_mapping(1, keycode, per, &keysyms)
        .unwrap()
        .check()
        .unwrap();
}

/// The text of each key pressed, in order, its repeats marked.
fn typed(inputs: &[Input]) -> Vec<String> {
    inputs
        .iter()
        .filter_map(|input| match input {
            Input::Key(KeyEvent {
                state: ButtonState::Pressed,
                text,
                logical,
                repeat,
                ..
            }) => Some(match (text, repeat) {
                (Some(text), false) => text.clone(),
                (Some(text), true) => format!("{text} repeat"),
                (None, _) if matches!(logical, Key::Dead(_)) => "dead".to_owned(),
                (None, _) => "none".to_owned(),
            }),
            _ => None,
        })
        .collect()
}

/// Keys typed through XTest while the pointer lies over the window, without a window manager: the
/// keys go to the window under the pointer, which the server tells by a focus. The keymap changes
/// as a client maps keysyms that it lacked; a key held repeats by the server's own repeat, without
/// releases between; a dead key composes with the next from the tests' own compose file.
#[test]
fn keys_typed_reach_the_window_with_the_servers_keymap_and_repeat() {
    server_settles();
    let compose = std::env::var("XCOMPOSEFILE").expect("run with XCOMPOSEFILE set");
    assert!(
        compose.ends_with("tests/compose/Compose"),
        "XCOMPOSEFILE names the tests' own file: {compose}"
    );
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let typist = std::thread::spawn(move || {
        let (connection, _) = x11rb::connect(None).expect("the X server");
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        std::thread::sleep(Duration::from_millis(300));
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 160, 120)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        xdotool(&["key", "a"]);
        xdotool(&["key", "shift+a"]);
        // `b` held for longer than the server's delay before it repeats.
        xdotool(&["keydown", "b"]);
        std::thread::sleep(Duration::from_millis(900));
        xdotool(&["keyup", "b"]);
        // Two keysyms that the keymap lacks, given to free keycodes: the window reads its new
        // keymap before the keys come.
        let (free, per) = free_keycodes(&connection);
        let [dead, circumflexed, ..] = free[..] else {
            panic!("the server's keymap has no two free keycodes: {free:?}");
        };
        const DEAD_CIRCUMFLEX: u32 = 0xfe52;
        const ECIRCUMFLEX: u32 = 0xea;
        map_keycode(&connection, dead, DEAD_CIRCUMFLEX, per);
        map_keycode(&connection, circumflexed, ECIRCUMFLEX, per);
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "ecircumflex"]);
        xdotool(&["key", "dead_circumflex", "e"]);
        std::thread::sleep(Duration::from_millis(200));
        map_keycode(&connection, dead, 0, per);
        map_keycode(&connection, circumflexed, 0, per);
        // Shift held as the pointer leaves for the root window, and the focus with it.
        xdotool(&["keydown", "shift"]);
        std::thread::sleep(Duration::from_millis(200));
        connection
            .warp_pointer(
                x11rb::NONE,
                connection.setup().roots[0].root,
                0,
                0,
                0,
                0,
                1000,
                800,
            )
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["keyup", "shift"]);
        let _ = stop_sender.send(());
    });
    let mut recorder = Recorder::new(Duration::from_secs(15), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    typist.join().expect("the typist");
    let inputs = &recorder.told.inputs;
    // Shift held as the focus left: told released after the focus, and nothing after.
    let left = inputs
        .iter()
        .position(|input| *input == Input::Focus(false))
        .expect("the focus lost");
    let held_last = inputs[..left].iter().rev().find_map(|input| match input {
        Input::Modifiers(held) => Some(*held),
        _ => None,
    });
    assert!(
        held_last.is_some_and(|held| held.shift),
        "Shift held as the focus left: {inputs:?}"
    );
    assert_eq!(
        inputs[left + 1..],
        [Input::Modifiers(slime_input::Modifiers::default())],
        "the modifiers told released as the focus left, then nothing"
    );
    let typed = typed(inputs);
    eprintln!("typed: {typed:?}");
    let focus: Vec<bool> = inputs
        .iter()
        .filter_map(|input| match input {
            Input::Focus(given) => Some(*given),
            _ => None,
        })
        .collect();
    assert_eq!(focus, [true, false], "the focus, given then lost");
    let first_key = inputs
        .iter()
        .position(|input| matches!(input, Input::Key(_)))
        .expect("a key");
    let focused = inputs
        .iter()
        .position(|input| *input == Input::Focus(true))
        .expect("the focus");
    assert!(focused < first_key, "the focus before the keys");
    // Shift held is told as a modifier before `A`.
    assert!(
        inputs
            .iter()
            .any(|input| matches!(input, Input::Modifiers(held) if held.shift)),
        "Shift held is told: {inputs:?}"
    );
    let repeats = typed.iter().filter(|text| *text == "b repeat").count();
    assert!(repeats >= 2, "the server repeats `b` held: {typed:?}");
    let b_held: Vec<&Input> = inputs
        .iter()
        .filter(|input| {
            matches!(input, Input::Key(KeyEvent { text: Some(text), .. }) if text == "b")
                || matches!(input, Input::Key(KeyEvent { logical: Key::Character(c), state: ButtonState::Released, .. }) if c == "b")
        })
        .collect();
    let releases = b_held
        .iter()
        .filter(|input| {
            matches!(
                input,
                Input::Key(KeyEvent {
                    state: ButtonState::Released,
                    ..
                })
            )
        })
        .count();
    assert_eq!(releases, 1, "one release of `b`, none between its repeats");
    let without_repeats: Vec<&str> = typed
        .iter()
        .map(String::as_str)
        .filter(|text| !text.ends_with(" repeat") && *text != "none")
        .collect();
    assert_eq!(
        without_repeats,
        ["a", "A", "b", "ê", "dead", "ê"],
        "the keys' texts, the keymap followed, a dead key composed"
    );
    println!("keys: typed, repeated by the server, the keymap followed, composed");
}

/// What the server shows as the cursor now: its image's size, hot spot and pixels.
fn cursor_image(connection: &impl Connection) -> (u16, u16, u16, u16, Vec<u32>) {
    let image = connection
        .xfixes_get_cursor_image()
        .unwrap()
        .reply()
        .unwrap();
    (
        image.width,
        image.height,
        image.xhot,
        image.yhot,
        image.cursor_image,
    )
}

/// Each shape that the host asks shows over the window, from the person's theme or the core
/// cursor font: as XFixes shows another client the cursor, the I-beam and both resizing arrows
/// differ from the arrow, which differs from the root window's cursor.
#[test]
fn the_cursor_shows_each_shape_asked_over_the_window() {
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (shape_sender, shape_receiver) = mpsc::channel();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let watcher = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        connection
            .xfixes_query_version(4, 0)
            .unwrap()
            .reply()
            .expect("XFixes");
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let warp = |to: u32, x: i16, y: i16| {
            connection
                .warp_pointer(x11rb::NONE, to, 0, 0, 0, 0, x, y)
                .unwrap()
                .check()
                .unwrap();
            std::thread::sleep(Duration::from_millis(200));
        };
        warp(root, 1000, 800);
        let outside = cursor_image(&connection);
        warp(window, 160, 120);
        // Before the host asks any shape: the arrow, set as the window opened.
        let opened = cursor_image(&connection);
        let mut shown = Vec::new();
        for shape in [
            CursorShape::Default,
            CursorShape::Text,
            CursorShape::ResizeHorizontal,
            CursorShape::ResizeVertical,
            CursorShape::Default,
        ] {
            shape_sender.send(shape).unwrap();
            std::thread::sleep(Duration::from_millis(250));
            shown.push((shape, cursor_image(&connection)));
        }
        let _ = stop_sender.send(());
        (outside, opened, shown)
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    recorder.shapes = Some(shape_receiver);
    let (_, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    let (outside, opened, shown) = watcher.join().expect("the watcher");
    let image = |index: usize| &shown[index].1;
    let size = |image: &(u16, u16, u16, u16, Vec<u32>)| (image.0, image.1, image.2, image.3);
    eprintln!(
        "outside {:?}; shown {:?}",
        size(&outside),
        shown
            .iter()
            .map(|(shape, image)| (shape, size(image)))
            .collect::<Vec<_>>()
    );
    assert_ne!(
        *image(0),
        outside,
        "the arrow over the window, not the root's cursor"
    );
    assert_eq!(
        opened,
        *image(0),
        "the arrow from the window's opening, before any shape asked"
    );
    for index in 1..=3 {
        assert_ne!(
            image(index),
            image(0),
            "{:?} is not the arrow",
            shown[index].0
        );
    }
    assert_ne!(image(1), image(2), "the I-beam is not an arrow to resize");
    assert_ne!(image(2), image(3), "the two arrows to resize differ");
    assert_eq!(image(4), image(0), "the arrow again");
    println!("cursor: by shape");
}

/// Sets the root window's X resources to `resources`, or deletes them for none.
fn set_resources(connection: &impl Connection, resources: Option<&[u8]>) {
    let root = connection.setup().roots[0].root;
    match resources {
        Some(resources) => connection
            .change_property8(
                PropMode::REPLACE,
                root,
                AtomEnum::RESOURCE_MANAGER,
                AtomEnum::STRING,
                resources,
            )
            .unwrap()
            .check()
            .unwrap(),
        None => connection
            .delete_property(root, AtomEnum::RESOURCE_MANAGER.into())
            .unwrap()
            .check()
            .unwrap(),
    }
}

/// The root window's X resources set by a test, given back as it found them as this drops,
/// however the test ends.
struct ResourcesSet<C: Connection> {
    connection: C,
    found: Option<Vec<u8>>,
}

impl<C: Connection> ResourcesSet<C> {
    /// Sets the X resources to `resources` on `connection`, keeping those that it finds.
    fn new(connection: C, resources: &[u8]) -> Self {
        let root = connection.setup().roots[0].root;
        let found = connection
            .get_property(
                false,
                root,
                AtomEnum::RESOURCE_MANAGER,
                AtomEnum::STRING,
                0,
                u32::MAX / 4,
            )
            .unwrap()
            .reply()
            .unwrap();
        let found = (found.type_ != x11rb::NONE).then_some(found.value);
        set_resources(&connection, Some(resources));
        Self { connection, found }
    }
}

impl<C: Connection> Drop for ResourcesSet<C> {
    fn drop(&mut self) {
        set_resources(&self.connection, self.found.as_deref());
    }
}

/// The scale from the X resources' `Xft.dpi` over 96: the window opens at its logical size times
/// it; changed, it is read again as the window takes the focus, not before, and the window keeps
/// its pixels, its logical size following.
#[test]
fn the_scale_follows_the_x_resources_dpi() {
    scale_follows_the_x_resources_dpi();
    println!("scale: 1.5");
}

/// The scale's test, which another runs again where the server holds X resources already.
fn scale_follows_the_x_resources_dpi() {
    server_settles();
    let (connection, _) = x11rb::connect(None).expect("the X server");
    let resources = ResourcesSet::new(connection, b"Xft.dpi:\t144\nXft.antialias:\t1\n");
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let changer = std::thread::spawn(move || {
        let (connection, _) = x11rb::connect(None).expect("the X server");
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        std::thread::sleep(Duration::from_millis(300));
        set_resources(&connection, Some(b"Xft.dpi:\t192\n"));
        // Read again as the window takes the keyboard's focus, here as the pointer enters it.
        std::thread::sleep(Duration::from_millis(300));
        let focused = Instant::now();
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 100, 100)
            .unwrap()
            .check()
            .unwrap();
        let root = connection.setup().roots[0].root;
        std::thread::sleep(Duration::from_millis(300));
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 800)
            .unwrap()
            .check()
            .unwrap();
        focused
    });
    let (recorder, ran) = uwui_windowing::run(
        opening(false),
        Recorder::new(Duration::from_millis(1800), false, Some(window_sender)),
    );
    ran.expect("the run");
    let focused = changer.join().expect("the changer");
    drop(resources);
    let told = &recorder.told;
    assert!(
        told.drawn_at.iter().all(|at| *at >= focused),
        "the scale changed before the window took the focus"
    );
    eprintln!("drawn {:?}; configured {:?}", told.drawn, told.configured);
    assert_eq!(
        told.drawn.first().copied(),
        Some(Drawn {
            pixels: [480, 360],
            scale: 1.5
        }),
        "opened at 320 x 240 logical pixels times 1.5"
    );
    assert_eq!(told.configured.first().map(|c| c.size), Some([320, 240]));
    assert_eq!(
        told.drawn.last().copied(),
        Some(Drawn {
            pixels: [480, 360],
            scale: 2.0
        }),
        "the same pixels at the new scale"
    );
    assert_eq!(told.configured.last().map(|c| c.size), Some([240, 180]));
}

/// A handler that waits for nothing but the platform, counting its turns: it sends its window's
/// id and its waker as the window opens, and ends at the turn after `stop`.
struct Sleeper {
    opened: Option<Sender<(u32, std::task::Waker)>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    turns: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    inputs: Vec<Input>,
}

impl Handler for Sleeper {
    fn opened(&mut self, window: &mut Window<'_>, _: Drawn) -> Result<(), Stopped> {
        if let (Some(sender), (_, RawWindowHandle::Xcb(handle))) =
            (&self.opened, window.raw_handles())
        {
            let _ = sender.send((handle.window.get(), window.waker()));
        }
        Ok(())
    }

    fn turn(&mut self, _: &mut Window<'_>, happened: Happened) -> Control {
        self.turns.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inputs
            .extend(happened.inputs.into_iter().map(|(_, input)| input));
        if self.stop.load(std::sync::atomic::Ordering::SeqCst) {
            Control::Exit
        } else {
            Control::Wait(None)
        }
    }

    fn closing(&mut self) {}
}

/// A window at rest, the pointer far from it: a click on another window, here the root, is
/// nothing of the window's, and must not turn its loop.
#[test]
fn a_click_elsewhere_on_the_screen_turns_nothing() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    server_settles();
    let stop = Arc::new(AtomicBool::new(false));
    let turns = Arc::new(AtomicUsize::new(0));
    let (opened_sender, opened_receiver) = mpsc::channel::<(u32, std::task::Waker)>();
    let (thread_stop, thread_turns) = (Arc::clone(&stop), Arc::clone(&turns));
    let clicker = std::thread::spawn(move || {
        let (_window, waker) = opened_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window");
        let count = || thread_turns.load(Ordering::SeqCst);
        // Far from the window, which lies at the screen's top left, 320 x 240.
        xdotool(&["mousemove", "1500", "900"]);
        std::thread::sleep(Duration::from_millis(700));
        let at_rest = count();
        std::thread::sleep(Duration::from_millis(500));
        let still = count();
        xdotool(&["click", "1"]);
        std::thread::sleep(Duration::from_millis(500));
        let clicked = count();
        // The wheel turned over the root window: buttons 4 and 5, twice each.
        xdotool(&["click", "--repeat", "2", "4"]);
        xdotool(&["click", "--repeat", "2", "5"]);
        std::thread::sleep(Duration::from_millis(500));
        let wheeled = count();
        // Shift pressed and released, the pointer still far from the window, which the
        // server's focus does not give the keys.
        xdotool(&["key", "shift"]);
        std::thread::sleep(Duration::from_millis(500));
        let shifted = count();
        thread_stop.store(true, Ordering::SeqCst);
        waker.wake();
        (at_rest, still, clicked, wheeled, shifted)
    });
    let sleeper = Sleeper {
        opened: Some(opened_sender),
        stop,
        turns,
        inputs: Vec::new(),
    };
    let (sleeper, ran) = uwui_windowing::run(opening(false), sleeper);
    ran.expect("the run");
    let (at_rest, still, clicked, wheeled, shifted) = clicker.join().expect("the clicker");
    eprintln!(
        "turns: at rest {at_rest}, 500 ms later {still}, after a click elsewhere {clicked}, \
         after four notches of the wheel elsewhere {wheeled}, after Shift typed elsewhere \
         {shifted}; inputs told {:?}",
        sleeper.inputs
    );
    assert_eq!(still, at_rest, "the window turned while nothing happened");
    assert_eq!(
        clicked,
        still,
        "a click on another window turned the window's loop {} times",
        clicked - still
    );
    assert_eq!(
        wheeled,
        clicked,
        "the wheel over another window turned the window's loop {} times",
        wheeled - clicked
    );
    assert_eq!(
        shifted,
        wheeled,
        "Shift typed into another window turned the window's loop {} times",
        shifted - wheeled
    );
    assert!(
        sleeper.inputs.is_empty(),
        "a window without the focus was told {:?}",
        sleeper.inputs
    );
}

/// A window at rest while another client names the root window, as a window manager's status
/// bar does each second: a property of the root other than the X resources must not turn the
/// window's loop.
#[test]
fn a_status_written_on_the_root_window_turns_nothing() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    server_settles();
    let stop = Arc::new(AtomicBool::new(false));
    let turns = Arc::new(AtomicUsize::new(0));
    let (opened_sender, opened_receiver) = mpsc::channel::<(u32, std::task::Waker)>();
    let (thread_stop, thread_turns) = (Arc::clone(&stop), Arc::clone(&turns));
    let namer = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let (_window, waker) = opened_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window");
        std::thread::sleep(Duration::from_millis(700));
        let at_rest = thread_turns.load(Ordering::SeqCst);
        for second in 0..5 {
            connection
                .change_property8(
                    PropMode::REPLACE,
                    root,
                    AtomEnum::WM_NAME,
                    AtomEnum::STRING,
                    format!("12:0{second} | 42%").as_bytes(),
                )
                .unwrap()
                .check()
                .unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
        let named = thread_turns.load(Ordering::SeqCst);
        // The root's name as the server had it, none.
        connection
            .delete_property(root, AtomEnum::WM_NAME.into())
            .unwrap()
            .check()
            .unwrap();
        thread_stop.store(true, Ordering::SeqCst);
        waker.wake();
        (at_rest, named)
    });
    let sleeper = Sleeper {
        opened: Some(opened_sender),
        stop,
        turns,
        inputs: Vec::new(),
    };
    let (_, ran) = uwui_windowing::run(opening(false), sleeper);
    ran.expect("the run");
    let (at_rest, named) = namer.join().expect("the namer");
    eprintln!("turns: at rest {at_rest}, after the root named five times {named}");
    assert_eq!(
        named,
        at_rest,
        "the root's name written five times turned the window's loop {} times",
        named - at_rest
    );
}

/// The focus that the layer tells while other clients move the server's focus and the pointer
/// stays over the window: every key that reaches the window comes while it is told focused.
#[test]
fn a_key_reaches_the_window_only_while_it_is_told_focused() {
    use x11rb::protocol::xproto::InputFocus;
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let mover = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        // Another client's window, far from the layer's.
        let other = connection.generate_id().unwrap();
        connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                other,
                root,
                1200,
                700,
                100,
                100,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )
            .unwrap()
            .check()
            .unwrap();
        connection.map_window(other).unwrap().check().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let focus = |to: u32| {
            connection
                .set_input_focus(InputFocus::POINTER_ROOT, to, x11rb::CURRENT_TIME)
                .unwrap()
                .check()
                .unwrap();
            std::thread::sleep(Duration::from_millis(300));
        };
        // The pointer over the window, the server's focus following it.
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 160, 120)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "a"]);
        // Another window takes the focus: its keys go there.
        focus(other);
        xdotool(&["key", "b"]);
        // The window's own focus.
        focus(window);
        xdotool(&["key", "c"]);
        // The root window's: the keys go to the window under the pointer, this one.
        focus(root);
        xdotool(&["key", "d"]);
        // The server's focus following the pointer again, as the next tests find it.
        focus(u32::from(InputFocus::POINTER_ROOT));
        let _ = stop_sender.send(());
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    mover.join().expect("the mover");
    let mut focused = false;
    let mut told = Vec::new();
    let mut unfocused_keys = Vec::new();
    for input in &recorder.told.inputs {
        match input {
            Input::Focus(given) => {
                focused = *given;
                told.push(format!("focus {given}"));
            }
            Input::Key(KeyEvent {
                state: ButtonState::Pressed,
                text: Some(text),
                ..
            }) => {
                told.push(format!("key {text}"));
                if !focused {
                    unfocused_keys.push(text.clone());
                }
            }
            _ => {}
        }
    }
    eprintln!("told: {told:?}");
    assert!(
        unfocused_keys.is_empty(),
        "keys reached the window while it was told unfocused: {unfocused_keys:?}; told {told:?}"
    );
}

/// Caps Lock locked by its key, then unlocked: XKB's locked modifiers reach the keymap's state,
/// as its depressed ones do.
#[test]
fn a_locked_modifier_types_its_characters() {
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let typist = std::thread::spawn(move || {
        let (connection, _) = x11rb::connect(None).expect("the X server");
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        std::thread::sleep(Duration::from_millis(300));
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 160, 120)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        xdotool(&["key", "Caps_Lock"]);
        xdotool(&["key", "a"]);
        xdotool(&["key", "Caps_Lock"]);
        xdotool(&["key", "a"]);
        std::thread::sleep(Duration::from_millis(200));
        let _ = stop_sender.send(());
    });
    let mut recorder = Recorder::new(Duration::from_secs(15), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    typist.join().expect("the typist");
    let typed: Vec<String> = typed(&recorder.told.inputs)
        .into_iter()
        .filter(|text| text != "none")
        .collect();
    eprintln!("typed: {typed:?}");
    assert_eq!(typed, ["A", "a"], "Caps Lock locked, then unlocked");
}

/// The scale's test, run where the server holds X resources already, as a person's session does:
/// they are as it found them afterwards, as this file's first lines say.
#[test]
fn the_scales_test_leaves_the_x_resources_as_it_found_them() {
    server_settles();
    let (connection, screen) = x11rb::connect(None).expect("the X server");
    let root = connection.setup().roots[screen].root;
    let found = b"Xcursor.size:\t24\n";
    set_resources(&connection, Some(found));
    scale_follows_the_x_resources_dpi();
    let after = connection
        .get_property(
            false,
            root,
            AtomEnum::RESOURCE_MANAGER,
            AtomEnum::ANY,
            0,
            1024,
        )
        .unwrap()
        .reply()
        .unwrap()
        .value;
    set_resources(&connection, None);
    assert_eq!(
        String::from_utf8_lossy(&after),
        String::from_utf8_lossy(found),
        "the X resources that the test found"
    );
}

/// The window's smallest size, given in logical pixels, is told the window manager in pixels at
/// the scale of `Xft.dpi`, and again as the scale changes, read as the window takes the focus.
#[test]
fn the_smallest_size_follows_the_scale() {
    use x11rb::properties::WmSizeHints;
    server_settles();
    let (connection, _) = x11rb::connect(None).expect("the X server");
    let resources = ResourcesSet::new(connection, b"Xft.dpi:\t144\n");
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let watcher = std::thread::spawn(move || {
        let (connection, _) = x11rb::connect(None).expect("the X server");
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        std::thread::sleep(Duration::from_millis(300));
        let smallest = || {
            WmSizeHints::get_normal_hints(&connection, window)
                .unwrap()
                .reply()
                .unwrap()
                .and_then(|hints| hints.min_size)
        };
        let at_one_and_a_half = smallest();
        set_resources(&connection, Some(b"Xft.dpi:\t192\n"));
        // Read again as the window takes the focus.
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 100, 100)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let at_two = smallest();
        let root = connection.setup().roots[0].root;
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 800)
            .unwrap()
            .check()
            .unwrap();
        (at_one_and_a_half, at_two)
    });
    let mut opening = opening(false);
    opening.min_size = Some([200, 100]);
    let (_, ran) = uwui_windowing::run(
        opening,
        Recorder::new(Duration::from_millis(1200), false, Some(window_sender)),
    );
    ran.expect("the run");
    let (at_one_and_a_half, at_two) = watcher.join().expect("the watcher");
    drop(resources);
    eprintln!("smallest at 1.5 {at_one_and_a_half:?}, at 2 {at_two:?}");
    assert_eq!(at_one_and_a_half, Some((300, 150)));
    assert_eq!(at_two, Some((400, 200)));
}

/// A handler that waits for nothing but the platform: it sends its window's id and its waker as
/// the window opens, says so and sleeps through the turn after `busy` is set, as a host drawing a
/// frame, and ends at the turn after `stop`.
struct Busy {
    opened: Option<Sender<(u32, std::task::Waker)>>,
    busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    in_turn: Sender<()>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Handler for Busy {
    fn opened(&mut self, window: &mut Window<'_>, _: Drawn) -> Result<(), Stopped> {
        if let (Some(sender), (_, RawWindowHandle::Xcb(handle))) =
            (&self.opened, window.raw_handles())
        {
            let _ = sender.send((handle.window.get(), window.waker()));
        }
        Ok(())
    }

    fn turn(&mut self, _: &mut Window<'_>, _: Happened) -> Control {
        use std::sync::atomic::Ordering;
        if self.busy.swap(false, Ordering::SeqCst) {
            let _ = self.in_turn.send(());
            std::thread::sleep(Duration::from_millis(400));
        }
        if self.stop.load(Ordering::SeqCst) {
            Control::Exit
        } else {
            Control::Wait(None)
        }
    }

    fn closing(&mut self) {}
}

/// A window manager's ping that comes while the host is busy in a turn, after which it waits for
/// nothing: the answer must reach the root window as the loop waits again, not at the next event
/// (a window manager that waits too long says that the application does not answer).
#[test]
fn a_ping_that_comes_during_a_turn_is_answered_at_rest() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    server_settles();
    let stop = Arc::new(AtomicBool::new(false));
    let busy = Arc::new(AtomicBool::new(false));
    let (opened_sender, opened_receiver) = mpsc::channel::<(u32, std::task::Waker)>();
    let (in_turn_sender, in_turn_receiver) = mpsc::channel::<()>();
    let (thread_stop, thread_busy) = (Arc::clone(&stop), Arc::clone(&busy));
    let pinger = std::thread::spawn(move || {
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
        let (protocols, ping) = (atom(b"WM_PROTOCOLS"), atom(b"_NET_WM_PING"));
        connection
            .change_window_attributes(
                root,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY),
            )
            .unwrap()
            .check()
            .unwrap();
        let (window, waker) = opened_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window");
        // At rest first; then a turn that lasts, during which the ping comes.
        std::thread::sleep(Duration::from_millis(300));
        thread_busy.store(true, Ordering::SeqCst);
        waker.wake_by_ref();
        in_turn_receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("the busy turn");
        let event = ClientMessageEvent::new(32, window, protocols, [ping, 4242, window, 0, 0]);
        connection
            .send_event(false, window, EventMask::NO_EVENT, event)
            .unwrap()
            .check()
            .unwrap();
        let sent = Instant::now();
        let mut answered = None;
        while answered.is_none() && sent.elapsed() < Duration::from_secs(2) {
            match connection.poll_for_event().unwrap() {
                Some(Event::ClientMessage(message))
                    if message.type_ == protocols && message.data.as_data32()[1] == 4242 =>
                {
                    answered = Some(sent.elapsed());
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
        thread_stop.store(true, Ordering::SeqCst);
        waker.wake();
        answered
    });
    let handler = Busy {
        opened: Some(opened_sender),
        busy,
        in_turn: in_turn_sender,
        stop,
    };
    let (_, ran) = uwui_windowing::run(opening(false), handler);
    ran.expect("the run");
    let answered = pinger.join().expect("the pinger");
    eprintln!("the ping sent during a turn of 400 ms was answered after {answered:?}");
    assert!(
        answered.is_some_and(|after| after < Duration::from_millis(1500)),
        "the ping that came during a turn was not answered while the window rested: {answered:?}"
    );
}

/// The server's focus moved from the window to follow the pointer, then to the root window and
/// back, the pointer staying over the window: the keys reach the window throughout, and its focus
/// is never told lost.
#[test]
fn the_focus_moved_above_the_window_under_the_pointer_is_not_told_lost() {
    use x11rb::protocol::xproto::InputFocus;
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let mover = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        std::thread::sleep(Duration::from_millis(300));
        let focus = |to: u32| {
            connection
                .set_input_focus(InputFocus::POINTER_ROOT, to, x11rb::CURRENT_TIME)
                .unwrap()
                .check()
                .unwrap();
            std::thread::sleep(Duration::from_millis(300));
        };
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 160, 120)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        focus(window);
        xdotool(&["key", "a"]);
        // From the window to the pointer's: the keys still go to the window under it.
        focus(u32::from(InputFocus::POINTER_ROOT));
        xdotool(&["key", "b"]);
        // To the root window, an ancestor, then back to the pointer's.
        focus(root);
        xdotool(&["key", "c"]);
        focus(u32::from(InputFocus::POINTER_ROOT));
        xdotool(&["key", "d"]);
        let _ = stop_sender.send(());
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    mover.join().expect("the mover");
    let told: Vec<String> = recorder
        .told
        .inputs
        .iter()
        .filter_map(|input| match input {
            Input::Focus(given) => Some(format!("focus {given}")),
            Input::Key(KeyEvent {
                state: ButtonState::Pressed,
                text: Some(text),
                ..
            }) => Some(format!("key {text}")),
            _ => None,
        })
        .collect();
    eprintln!("told: {told:?}");
    assert_eq!(
        told,
        ["focus true", "key a", "key b", "key c", "key d"],
        "the focus was told lost while every key reached the window"
    );
}

/// A handler that waits for nothing but the platform, as a host at rest: it sends its window's id
/// and its waker as the window opens; when `signals`, it asks the signal at its first turn and at
/// each signal, counting them; it ends at the close asked, or at the turn after `stop`.
struct Waiter {
    opened: Option<Sender<(u32, std::task::Waker)>>,
    signals: bool,
    told_signals: usize,
    close_asked: Option<Instant>,
    turns: usize,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Waiter {
    fn new(
        opened: Sender<(u32, std::task::Waker)>,
        signals: bool,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            opened: Some(opened),
            signals,
            told_signals: 0,
            close_asked: None,
            turns: 0,
            stop,
        }
    }
}

impl Handler for Waiter {
    fn opened(&mut self, window: &mut Window<'_>, _: Drawn) -> Result<(), Stopped> {
        if let (Some(sender), (_, RawWindowHandle::Xcb(handle))) =
            (&self.opened, window.raw_handles())
        {
            let _ = sender.send((handle.window.get(), window.waker()));
        }
        Ok(())
    }

    fn turn(&mut self, window: &mut Window<'_>, happened: Happened) -> Control {
        self.turns += 1;
        if happened.signal.is_some() {
            self.told_signals += 1;
        }
        if self.signals && (self.turns == 1 || happened.signal.is_some()) {
            let _ = window.ask_signal();
        }
        if happened.close_asked {
            self.close_asked = Some(Instant::now());
            return Control::Exit;
        }
        if self.stop.load(std::sync::atomic::Ordering::SeqCst) {
            Control::Exit
        } else {
            Control::Wait(None)
        }
    }

    fn closing(&mut self) {}
}

/// The window manager's close button, `WM_DELETE_WINDOW`, while the host waits for nothing: the
/// host is told at once, though no other event comes.
#[test]
fn a_close_asked_at_rest_turns_the_loop() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    server_settles();
    let stop = Arc::new(AtomicBool::new(false));
    let (opened_sender, opened_receiver) = mpsc::channel::<(u32, std::task::Waker)>();
    let thread_stop = Arc::clone(&stop);
    let closer = std::thread::spawn(move || {
        let (connection, _) = x11rb::connect(None).expect("the X server");
        let atom = |name: &[u8]| {
            connection
                .intern_atom(false, name)
                .unwrap()
                .reply()
                .unwrap()
                .atom
        };
        let (protocols, delete) = (atom(b"WM_PROTOCOLS"), atom(b"WM_DELETE_WINDOW"));
        let (window, waker) = opened_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window");
        std::thread::sleep(Duration::from_millis(300));
        let event = ClientMessageEvent::new(32, window, protocols, [delete, 0, 0, 0, 0]);
        connection
            .send_event(false, window, EventMask::NO_EVENT, event)
            .unwrap()
            .check()
            .unwrap();
        let sent = Instant::now();
        // Woken after two seconds, if it did not end before.
        std::thread::sleep(Duration::from_secs(2));
        thread_stop.store(true, Ordering::SeqCst);
        waker.wake();
        sent
    });
    let (waiter, ran) = uwui_windowing::run(
        opening(false),
        Waiter::new(opened_sender, false, Arc::clone(&stop)),
    );
    ran.expect("the run");
    let sent = closer.join().expect("the closer");
    let told = waiter.close_asked.map(|at| at.duration_since(sent));
    eprintln!("the close asked at rest was told after {told:?}");
    assert!(
        told.is_some_and(|after| after < Duration::from_secs(1)),
        "the close asked while the host waited for nothing was told after {told:?}"
    );
}

/// The signal asked by a host that waits for nothing else: Present's notice of each refresh ends
/// the wait, a frame after a frame, as a host drawing an animation waits.
#[test]
fn the_signal_asked_at_rest_turns_the_loop_at_each_refresh() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    server_settles();
    let stop = Arc::new(AtomicBool::new(false));
    let (opened_sender, opened_receiver) = mpsc::channel::<(u32, std::task::Waker)>();
    let thread_stop = Arc::clone(&stop);
    let stopper = std::thread::spawn(move || {
        let (_, waker) = opened_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window");
        std::thread::sleep(Duration::from_millis(1000));
        thread_stop.store(true, Ordering::SeqCst);
        waker.wake();
    });
    let (waiter, ran) = uwui_windowing::run(
        opening(false),
        Waiter::new(opened_sender, true, Arc::clone(&stop)),
    );
    ran.expect("the run");
    stopper.join().expect("the stopper");
    eprintln!("signals told in a second: {}", waiter.told_signals);
    assert!(
        waiter.told_signals > 20,
        "a host that waits for its signal alone was told {} in a second",
        waiter.told_signals
    );
}

/// The pointer entering the window and leaving it twice, the server's focus following it, the X
/// resources unchanged: the cursor's shapes are not made again as the window takes the focus,
/// XFixes showing the same cursor over the window each time.
#[test]
fn the_cursors_are_made_again_only_when_the_x_resources_change() {
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let watcher = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        connection
            .xfixes_query_version(4, 0)
            .unwrap()
            .reply()
            .expect("XFixes");
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let warp = |to: u32, x: i16, y: i16| {
            connection
                .warp_pointer(x11rb::NONE, to, 0, 0, 0, 0, x, y)
                .unwrap()
                .check()
                .unwrap();
            std::thread::sleep(Duration::from_millis(300));
        };
        let mut serials = Vec::new();
        for _ in 0..2 {
            warp(window, 160, 120);
            serials.push(
                connection
                    .xfixes_get_cursor_image()
                    .unwrap()
                    .reply()
                    .unwrap()
                    .cursor_serial,
            );
            warp(root, 1000, 800);
        }
        let _ = stop_sender.send(());
        serials
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    let serials = watcher.join().expect("the watcher");
    let focus: Vec<&Input> = recorder
        .told
        .inputs
        .iter()
        .filter(|input| matches!(input, Input::Focus(_)))
        .collect();
    eprintln!("the cursor's serials over the window {serials:?}; focus told {focus:?}");
    assert_eq!(
        focus.len(),
        4,
        "the window took the focus twice and lost it twice"
    );
    assert_eq!(
        serials[0], serials[1],
        "the cursor was made again as the window took the focus, the X resources unchanged"
    );
}

/// A handler that waits for nothing but the platform: it sends its window's id and its waker as
/// the window opens, keeps the moment of each turn told that the window was exposed, and ends at
/// the turn after `stop`.
struct ExposureWaiter {
    opened: Option<Sender<(u32, std::task::Waker)>>,
    exposed: Vec<Instant>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Handler for ExposureWaiter {
    fn opened(&mut self, window: &mut Window<'_>, _: Drawn) -> Result<(), Stopped> {
        if let (Some(sender), (_, RawWindowHandle::Xcb(handle))) =
            (&self.opened, window.raw_handles())
        {
            let _ = sender.send((handle.window.get(), window.waker()));
        }
        Ok(())
    }

    fn turn(&mut self, _: &mut Window<'_>, happened: Happened) -> Control {
        if happened.exposed {
            self.exposed.push(Instant::now());
        }
        if self.stop.load(std::sync::atomic::Ordering::SeqCst) {
            Control::Exit
        } else {
            Control::Wait(None)
        }
    }

    fn closing(&mut self) {}
}

/// Another client's window moved over part of the window, as where no compositor keeps its image:
/// the part that it leaves is exposed, the window staying partly covered, without a change of its
/// visibility; the host that waits for nothing is told at once to draw again.
#[test]
fn an_exposure_alone_turns_a_loop_at_rest() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    server_settles();
    let stop = Arc::new(AtomicBool::new(false));
    let (opened_sender, opened_receiver) = mpsc::channel::<(u32, std::task::Waker)>();
    let thread_stop = Arc::clone(&stop);
    let mover = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let (_, waker) = opened_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window");
        std::thread::sleep(Duration::from_millis(300));
        // A window over the top left of the layer's, which lies at the screen's top left.
        let cover = connection.generate_id().unwrap();
        connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                cover,
                root,
                0,
                0,
                100,
                100,
                0,
                WindowClass::INPUT_OUTPUT,
                x11rb::COPY_FROM_PARENT,
                &CreateWindowAux::new()
                    .override_redirect(1)
                    .background_pixel(0),
            )
            .unwrap()
            .check()
            .unwrap();
        connection.map_window(cover).unwrap().check().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        // Moved right, still over the window: what it covered is exposed.
        let moved = Instant::now();
        connection
            .configure_window(cover, &ConfigureWindowAux::new().x(150))
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        connection.destroy_window(cover).unwrap().check().unwrap();
        thread_stop.store(true, Ordering::SeqCst);
        waker.wake();
        moved
    });
    let handler = ExposureWaiter {
        opened: Some(opened_sender),
        exposed: Vec::new(),
        stop,
    };
    let (handler, ran) = uwui_windowing::run(opening(false), handler);
    ran.expect("the run");
    let moved = mover.join().expect("the mover");
    let after: Vec<Duration> = handler
        .exposed
        .iter()
        .filter(|at| **at >= moved)
        .map(|at| at.duration_since(moved))
        .collect();
    eprintln!("exposures told after the move: {after:?}");
    assert!(
        after
            .first()
            .is_some_and(|first| *first < Duration::from_millis(500)),
        "the part that the other window left was not told exposed at once: {after:?}"
    );
}

/// The focus told and the keys pressed, in order, as words: `focus true`, `key a`.
fn focus_and_keys(inputs: &[Input]) -> Vec<String> {
    inputs
        .iter()
        .filter_map(|input| match input {
            Input::Focus(given) => Some(format!("focus {given}")),
            Input::Key(KeyEvent {
                state: ButtonState::Pressed,
                text: Some(text),
                ..
            }) => Some(format!("key {text}")),
            _ => None,
        })
        .collect()
}

/// The keys pressed while the window was told unfocused, by their text.
fn keys_told_unfocused(inputs: &[Input]) -> Vec<String> {
    let mut focused = false;
    let mut unfocused = Vec::new();
    for input in inputs {
        match input {
            Input::Focus(given) => focused = *given,
            Input::Key(KeyEvent {
                state: ButtonState::Pressed,
                text: Some(text),
                ..
            }) if !focused => unfocused.push(text.clone()),
            _ => {}
        }
    }
    unfocused
}

/// Another client's window, far from the layer's, mapped, as a menu or a dialog of another
/// program: its id.
fn other_window(connection: &impl Connection, root: u32) -> u32 {
    let other = connection.generate_id().unwrap();
    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            other,
            root,
            1200,
            700,
            100,
            100,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().override_redirect(1),
        )
        .unwrap()
        .check()
        .unwrap();
    connection.map_window(other).unwrap().check().unwrap();
    other
}

/// Another client grabs the keyboard while the window holds the server's focus, as a menu of
/// another program or a screen locker does: until it lets go, every key goes to that client, so
/// the window is told the focus lost, then given back as the grab ends (uwui's `Input::Focus`:
/// `false` as the focus goes to another window or program).
#[test]
fn a_keyboard_grabbed_by_another_client_is_told_a_focus_lost_until_let_go() {
    use x11rb::protocol::xproto::{GrabMode, GrabStatus, InputFocus};
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let grabber = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let menu = other_window(&connection, root);
        // The pointer far from both; the window given the server's focus itself, which goes
        // back to following the pointer once it closes.
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 400)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        connection
            .set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let grabbed = connection
            .grab_keyboard(
                false,
                menu,
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .unwrap()
            .reply()
            .unwrap()
            .status;
        std::thread::sleep(Duration::from_millis(300));
        // Typed into the menu, which holds every key.
        xdotool(&["key", "a"]);
        connection
            .ungrab_keyboard(x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "b"]);
        std::thread::sleep(Duration::from_millis(200));
        let _ = stop_sender.send(());
        grabbed
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    let grabbed = grabber.join().expect("the grabber");
    assert_eq!(grabbed, GrabStatus::SUCCESS, "the keyboard grabbed");
    let told = focus_and_keys(&recorder.told.inputs);
    eprintln!("told: {told:?}");
    assert_eq!(
        told,
        ["focus true", "focus false", "focus true", "key b"],
        "the keyboard that another client held was not told a focus lost"
    );
}

/// Alt held in the window, then Tab, which another client binds with Alt on the root window, as a
/// window manager's switcher: Tab goes to that client through its passive grab, which ends as Tab
/// is let go, the focus staying on the window. The window must be told the focus lost before
/// Alt's release comes back to it, so that the release reaches no bar of menus (uwui's
/// `alt_let_go_after_the_window_lost_the_focus_reaches_no_bar`).
#[test]
fn a_key_bound_with_alt_by_another_client_tells_the_focus_lost_before_alts_release() {
    use x11rb::protocol::xproto::{GrabMode, InputFocus, ModMask};
    const TAB: u32 = 0xff09;
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let binder = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let setup = connection.setup();
        let (first, last) = (setup.min_keycode, setup.max_keycode);
        let mapping = connection
            .get_keyboard_mapping(first, last - first + 1)
            .unwrap()
            .reply()
            .unwrap();
        let tab = mapping
            .keysyms
            .chunks(usize::from(mapping.keysyms_per_keycode))
            .zip(first..=last)
            .find(|(keysyms, _)| keysyms.first() == Some(&TAB))
            .map(|(_, keycode)| keycode)
            .expect("Tab's keycode");
        connection
            .grab_key(
                false,
                root,
                ModMask::M1,
                tab,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .unwrap()
            .check()
            .unwrap();
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 400)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        connection
            .set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["keydown", "alt"]);
        std::thread::sleep(Duration::from_millis(200));
        xdotool(&["key", "Tab"]);
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["keyup", "alt"]);
        std::thread::sleep(Duration::from_millis(300));
        connection
            .ungrab_key(tab, root, ModMask::M1)
            .unwrap()
            .check()
            .unwrap();
        let _ = stop_sender.send(());
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    binder.join().expect("the binder");
    let told: Vec<String> = recorder
        .told
        .inputs
        .iter()
        .filter_map(|input| match input {
            Input::Focus(given) => Some(format!("focus {given}")),
            Input::Key(KeyEvent {
                logical: Key::Named(slime_input::NamedKey::Alt),
                state,
                ..
            }) => Some(format!("alt {state:?}")),
            _ => None,
        })
        .collect();
    eprintln!("told: {told:?}");
    let pressed = told
        .iter()
        .position(|told| told == "alt Pressed")
        .expect("Alt pressed in the window");
    let released = told
        .iter()
        .position(|told| told == "alt Released")
        .expect("Alt let go in the window");
    assert!(
        told[pressed..released]
            .iter()
            .any(|told| told == "focus false"),
        "Alt let go after Tab went to another client, without a focus lost between: {told:?}"
    );
}

/// The server's focus following the pointer over the window while another client grabs the
/// pointer, as a drag that another program starts: the keys still go to the window under the
/// pointer, which must still be told focused.
#[test]
fn a_pointer_grabbed_by_another_client_leaves_the_keys_and_their_focus_to_the_window() {
    use x11rb::protocol::xproto::{GrabMode, GrabStatus, InputFocus};
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let grabber = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let other = other_window(&connection, root);
        connection
            .set_input_focus(
                InputFocus::POINTER_ROOT,
                u32::from(InputFocus::POINTER_ROOT),
                x11rb::CURRENT_TIME,
            )
            .unwrap()
            .check()
            .unwrap();
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 160, 120)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "a"]);
        let grabbed = connection
            .grab_pointer(
                false,
                other,
                EventMask::NO_EVENT,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
                x11rb::NONE,
                x11rb::NONE,
                x11rb::CURRENT_TIME,
            )
            .unwrap()
            .reply()
            .unwrap()
            .status;
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "b"]);
        connection
            .ungrab_pointer(x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "c"]);
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 800)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let _ = stop_sender.send(());
        grabbed
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    let grabbed = grabber.join().expect("the grabber");
    assert_eq!(grabbed, GrabStatus::SUCCESS, "the pointer grabbed");
    let told = focus_and_keys(&recorder.told.inputs);
    eprintln!("told: {told:?}");
    assert!(
        told.contains(&"key b".to_owned()),
        "b reached the window: {told:?}"
    );
    let unfocused = keys_told_unfocused(&recorder.told.inputs);
    assert!(
        unfocused.is_empty(),
        "keys reached the window while it was told unfocused: {unfocused:?}; told {told:?}"
    );
    // The pointer's grab is no move of the pointer's: the focus is never told lost before the
    // pointer leaves the window, after the last key.
    let last_key = told
        .iter()
        .position(|told| told == "key c")
        .expect("c reached the window");
    assert!(
        !told[..last_key].contains(&"focus false".to_owned()),
        "a grab of the pointer told the focus lost: {told:?}"
    );
}

/// A handler that records the inputs told and waits for nothing but the platform: it sends its
/// window's id and its waker as the window opens; at the turn after `busy` is set, it says so and
/// sleeps 800 ms, as a host drawing a long frame, its first after a start; it ends at the turn
/// after `stop`.
struct BusyRecorder {
    opened: Option<Sender<(u32, std::task::Waker)>>,
    busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    in_turn: Sender<()>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    inputs: Vec<Input>,
}

impl Handler for BusyRecorder {
    fn opened(&mut self, window: &mut Window<'_>, _: Drawn) -> Result<(), Stopped> {
        if let (Some(sender), (_, RawWindowHandle::Xcb(handle))) =
            (&self.opened, window.raw_handles())
        {
            let _ = sender.send((handle.window.get(), window.waker()));
        }
        Ok(())
    }

    fn turn(&mut self, _: &mut Window<'_>, happened: Happened) -> Control {
        use std::sync::atomic::Ordering;
        self.inputs
            .extend(happened.inputs.into_iter().map(|(_, input)| input));
        if self.busy.swap(false, Ordering::SeqCst) {
            let _ = self.in_turn.send(());
            std::thread::sleep(Duration::from_millis(800));
        }
        if self.stop.load(Ordering::SeqCst) {
            Control::Exit
        } else {
            Control::Wait(None)
        }
    }

    fn closing(&mut self) {}
}

/// Runs a window whose host, once at rest, is busy in one turn of 800 ms: `scenario` runs on
/// another thread with the window's id, and calls the function it is given to start that turn,
/// which returns once the host is in it. The inputs told, once the turn's events were read.
fn with_a_long_turn(scenario: impl FnOnce(u32, &dyn Fn()) + Send + 'static) -> Vec<Input> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let stop = Arc::new(AtomicBool::new(false));
    let busy = Arc::new(AtomicBool::new(false));
    let (opened_sender, opened_receiver) = mpsc::channel::<(u32, std::task::Waker)>();
    let (in_turn_sender, in_turn_receiver) = mpsc::channel::<()>();
    let (thread_stop, thread_busy) = (Arc::clone(&stop), Arc::clone(&busy));
    let player = std::thread::spawn(move || {
        let (window, waker) = opened_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window");
        std::thread::sleep(Duration::from_millis(300));
        let start_turn = || {
            thread_busy.store(true, Ordering::SeqCst);
            waker.wake_by_ref();
            in_turn_receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("the long turn");
        };
        scenario(window, &start_turn);
        // The turn ends, and the events that came during it are read.
        std::thread::sleep(Duration::from_millis(1500));
        thread_stop.store(true, Ordering::SeqCst);
        waker.wake();
    });
    let handler = BusyRecorder {
        opened: Some(opened_sender),
        busy,
        in_turn: in_turn_sender,
        stop,
        inputs: Vec::new(),
    };
    let (handler, ran) = uwui_windowing::run(opening(false), handler);
    ran.expect("the run");
    player.join().expect("the scenario");
    handler.inputs
}

/// The window given the focus, then Shift and A typed, while its host is busy in a long turn, as
/// a person types at once into a window that a launcher just opened: each key, read after the
/// turn, types what the server's state gave it as it was pressed, `A`.
#[test]
fn keys_typed_as_the_focus_comes_during_a_long_turn_keep_their_modifiers() {
    use x11rb::protocol::xproto::InputFocus;
    server_settles();
    let inputs = with_a_long_turn(|window, start_turn| {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 400)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        start_turn();
        connection
            .set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        xdotool(&["key", "shift+a"]);
    });
    let typed: Vec<String> = typed(&inputs)
        .into_iter()
        .filter(|text| text != "none")
        .collect();
    eprintln!("typed: {typed:?}; told: {:?}", focus_and_keys(&inputs));
    assert_eq!(typed, ["A"], "Shift+A typed as the focus came");
}

/// The window given the focus, a key typed, then the focus given to another window, all while
/// the host is busy in a long turn: read after it, the key that reached the window comes while
/// the window is told focused, as every key does.
#[test]
fn a_key_typed_during_a_focus_come_and_gone_in_a_long_turn_is_told_focused() {
    use x11rb::protocol::xproto::InputFocus;
    server_settles();
    let inputs = with_a_long_turn(|window, start_turn| {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let other = other_window(&connection, root);
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 400)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        start_turn();
        let focus = |to: u32| {
            connection
                .set_input_focus(InputFocus::POINTER_ROOT, to, x11rb::CURRENT_TIME)
                .unwrap()
                .check()
                .unwrap();
        };
        focus(window);
        xdotool(&["key", "b"]);
        focus(other);
        std::thread::sleep(Duration::from_millis(100));
        // The server's focus following the pointer again, as the next tests find it.
        focus(u32::from(InputFocus::POINTER_ROOT));
    });
    let told = focus_and_keys(&inputs);
    eprintln!("told: {told:?}");
    assert!(
        told.contains(&"key b".to_owned()),
        "b reached the window: {told:?}"
    );
    let unfocused = keys_told_unfocused(&inputs);
    assert!(
        unfocused.is_empty(),
        "keys reached the window while it was told unfocused: {unfocused:?}; told {told:?}"
    );
}

/// As `a_key_bound_with_alt_by_another_client_tells_the_focus_lost_before_alts_release`, the
/// pointer over the window: the window must be told the focus lost before Alt's release comes back.
#[test]
fn a_key_bound_with_alt_tells_the_focus_lost_the_pointer_over_the_window() {
    use x11rb::protocol::xproto::{GrabMode, InputFocus, ModMask};
    const TAB: u32 = 0xff09;
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let binder = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let setup = connection.setup();
        let (first, last) = (setup.min_keycode, setup.max_keycode);
        let mapping = connection
            .get_keyboard_mapping(first, last - first + 1)
            .unwrap()
            .reply()
            .unwrap();
        let tab = mapping
            .keysyms
            .chunks(usize::from(mapping.keysyms_per_keycode))
            .zip(first..=last)
            .find(|(keysyms, _)| keysyms.first() == Some(&TAB))
            .map(|(_, keycode)| keycode)
            .expect("Tab's keycode");
        connection
            .grab_key(
                false,
                root,
                ModMask::M1,
                tab,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .unwrap()
            .check()
            .unwrap();
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 160, 120)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        connection
            .set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["keydown", "alt"]);
        std::thread::sleep(Duration::from_millis(200));
        xdotool(&["key", "Tab"]);
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["keyup", "alt"]);
        std::thread::sleep(Duration::from_millis(300));
        connection
            .ungrab_key(tab, root, ModMask::M1)
            .unwrap()
            .check()
            .unwrap();
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 800)
            .unwrap()
            .check()
            .unwrap();
        let _ = stop_sender.send(());
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    binder.join().expect("the binder");
    let told: Vec<String> = recorder
        .told
        .inputs
        .iter()
        .filter_map(|input| match input {
            Input::Focus(given) => Some(format!("focus {given}")),
            Input::Key(KeyEvent {
                logical: Key::Named(slime_input::NamedKey::Alt),
                state,
                ..
            }) => Some(format!("alt {state:?}")),
            _ => None,
        })
        .collect();
    eprintln!("told: {told:?}");
    let pressed = told
        .iter()
        .position(|told| told == "alt Pressed")
        .expect("Alt pressed in the window");
    let released = told
        .iter()
        .position(|told| told == "alt Released")
        .expect("Alt let go in the window");
    assert!(
        told[pressed..released]
            .iter()
            .any(|told| told == "focus false"),
        "Alt let go after Tab went to another client, without a focus lost between: {told:?}"
    );
}

/// What a played focus names: the window, the root window, and another client's window far from
/// the layer's.
struct Places {
    window: u32,
    root: u32,
    other: u32,
}

/// Runs the window while another client plays `scenario`: that client makes a window far from the
/// layer's and, when `framed`, a frame that it reparents the window into, as a window manager
/// does; it puts the pointer over the window, then plays `scenario` with a function that gives the
/// server's focus to a window and lets 300 ms pass. The client's connection, and the windows that
/// it made, last until the run ends; the server's focus follows the pointer again afterwards. The
/// focus told and the keys pressed, in order, as words: `focus true`, `key a`.
fn focus_played(
    framed: bool,
    scenario: impl FnOnce(&dyn Fn(u32), Places) + Send + 'static,
) -> Vec<String> {
    use x11rb::protocol::xproto::InputFocus;
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let (ended_sender, ended_receiver) = mpsc::channel::<()>();
    let player = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let make = |at: (i16, i16), size: (u16, u16), override_redirect: bool| {
            let made = connection.generate_id().unwrap();
            connection
                .create_window(
                    x11rb::COPY_DEPTH_FROM_PARENT,
                    made,
                    root,
                    at.0,
                    at.1,
                    size.0,
                    size.1,
                    0,
                    WindowClass::INPUT_OUTPUT,
                    0,
                    &CreateWindowAux::new().override_redirect(u32::from(override_redirect)),
                )
                .unwrap()
                .check()
                .unwrap();
            made
        };
        let other = make((1200, 700), (100, 100), true);
        connection.map_window(other).unwrap().check().unwrap();
        if framed {
            // A frame at the screen's top left, larger than the window, which it holds 20 pixels in.
            let frame = make((0, 0), (400, 300), false);
            connection
                .reparent_window(window, frame, 20, 20)
                .unwrap()
                .check()
                .unwrap();
            connection.map_window(frame).unwrap().check().unwrap();
        }
        std::thread::sleep(Duration::from_millis(300));
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 160, 120)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let focus = |to: u32| {
            connection
                .set_input_focus(InputFocus::POINTER_ROOT, to, x11rb::CURRENT_TIME)
                .unwrap()
                .check()
                .unwrap();
            std::thread::sleep(Duration::from_millis(300));
        };
        scenario(
            &focus,
            Places {
                window,
                root,
                other,
            },
        );
        let _ = stop_sender.send(());
        let _ = ended_receiver.recv_timeout(Duration::from_secs(15));
        // As the next tests find them: the focus following the pointer, far from where windows open.
        focus(u32::from(InputFocus::POINTER_ROOT));
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 800)
            .unwrap()
            .check()
            .unwrap();
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    let _ = ended_sender.send(());
    ran.expect("the run");
    player.join().expect("the scenario");
    recorder
        .told
        .inputs
        .iter()
        .filter_map(|input| match input {
            Input::Focus(given) => Some(format!("focus {given}")),
            Input::Key(KeyEvent {
                state: ButtonState::Pressed,
                text: Some(text),
                ..
            }) => Some(format!("key {text}")),
            _ => None,
        })
        .collect()
}

/// The window inside a frame of another client's, as a window manager reparents it, the pointer
/// over it: the server's focus on the window, then on the root window, two levels above it, then on
/// another window. The keys reach the window while its own focus or the root's holds, which it is
/// told; the focus on another window takes them.
#[test]
fn the_root_two_levels_above_a_framed_window_gives_it_the_keys() {
    server_settles();
    let told = focus_played(true, |focus, places| {
        focus(places.window);
        xdotool(&["key", "a"]);
        focus(places.root);
        xdotool(&["key", "b"]);
        focus(places.other);
        xdotool(&["key", "c"]);
    });
    eprintln!("told: {told:?}");
    assert_eq!(
        told,
        ["focus true", "key a", "key b", "focus false"],
        "the keys and the focus of a window in a frame"
    );
}

/// The pointer over the window, the server's focus moved from another window to the root window,
/// above the window, as it moves where a window that held it closes and the focus reverts to its
/// parent: the keys go to the window under the pointer, which must be told focused before they
/// come. XInput2 sends the window no focus event for this move (read under Xvfb).
#[test]
fn a_focus_moved_from_another_window_to_the_root_tells_the_window_under_the_pointer_focused() {
    server_settles();
    let told = focus_played(false, |focus, places| {
        focus(places.other);
        xdotool(&["key", "a"]);
        focus(places.root);
        xdotool(&["key", "b"]);
    });
    eprintln!("told: {told:?}");
    assert_eq!(
        told,
        ["focus true", "focus false", "focus true", "key b"],
        "the key that reached the window came while it was told unfocused"
    );
}

/// Shift held as the focus leaves for another window, let go there, then the focus moved to the
/// root window, the pointer over the window: `b`, typed then, reaches the window, which must read
/// it with the modifiers that the server gave it, none, not those held as the focus left.
#[test]
fn a_key_typed_after_the_focus_came_back_through_the_root_takes_the_servers_modifiers() {
    server_settles();
    let told = focus_played(false, |focus, places| {
        xdotool(&["keydown", "shift"]);
        std::thread::sleep(Duration::from_millis(200));
        focus(places.other);
        xdotool(&["keyup", "shift"]);
        focus(places.root);
        xdotool(&["key", "b"]);
    });
    eprintln!("told: {told:?}");
    let keys: Vec<&String> = told.iter().filter(|told| told.starts_with("key")).collect();
    assert_eq!(keys, ["key b"], "b typed without Shift: {told:?}");
}

/// The server's focus following the pointer, which lies over the window, while another client
/// grabs the keyboard; the pointer leaves the window during the grab, which ends there, then comes
/// back: the server sends the window no event as the grab ends, the focus lying under the pointer
/// elsewhere, and `b`, typed once the pointer is back, reaches the window, which must be told
/// focused before it.
#[test]
fn a_keyboard_grab_that_ends_while_the_pointer_is_elsewhere_gives_the_focus_back() {
    use x11rb::protocol::xproto::{GrabMode, GrabStatus, InputFocus};
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let grabber = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let menu = other_window(&connection, root);
        connection
            .set_input_focus(
                InputFocus::POINTER_ROOT,
                u32::from(InputFocus::POINTER_ROOT),
                x11rb::CURRENT_TIME,
            )
            .unwrap()
            .check()
            .unwrap();
        let warp = |to: u32, x: i16, y: i16| {
            connection
                .warp_pointer(x11rb::NONE, to, 0, 0, 0, 0, x, y)
                .unwrap()
                .check()
                .unwrap();
            std::thread::sleep(Duration::from_millis(300));
        };
        warp(window, 160, 120);
        xdotool(&["key", "a"]);
        let grabbed = connection
            .grab_keyboard(
                false,
                menu,
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .unwrap()
            .reply()
            .unwrap()
            .status;
        std::thread::sleep(Duration::from_millis(300));
        // The pointer leaves the window while the grab holds; the grab ends with it elsewhere.
        warp(root, 1000, 400);
        connection
            .ungrab_keyboard(x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        // Back over the window, which takes the keys again.
        warp(window, 160, 120);
        xdotool(&["key", "b"]);
        std::thread::sleep(Duration::from_millis(200));
        warp(root, 1000, 800);
        let _ = stop_sender.send(());
        grabbed
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    let grabbed = grabber.join().expect("the grabber");
    assert_eq!(grabbed, GrabStatus::SUCCESS, "the keyboard grabbed");
    let told = focus_and_keys(&recorder.told.inputs);
    eprintln!("told: {told:?}");
    assert!(
        told.contains(&"key b".to_owned()),
        "b reached the window: {told:?}"
    );
    let unfocused = keys_told_unfocused(&recorder.told.inputs);
    assert!(
        unfocused.is_empty(),
        "keys reached the window while it was told unfocused: {unfocused:?}; told {told:?}"
    );
}

/// As `keys_typed_as_the_focus_comes_during_a_long_turn_keep_their_modifiers`, once the keys are
/// read: Shift was let go, so the modifiers last told hold none.
#[test]
fn keys_typed_as_the_focus_comes_during_a_long_turn_leave_no_modifier_told_held() {
    use x11rb::protocol::xproto::InputFocus;
    server_settles();
    let inputs = with_a_long_turn(|window, start_turn| {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 400)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        start_turn();
        connection
            .set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        // Shift let go after `a`, as a person types `A`.
        xdotool(&["keydown", "shift"]);
        xdotool(&["key", "a"]);
        xdotool(&["keyup", "shift"]);
    });
    let last_held = inputs.iter().rev().find_map(|input| match input {
        Input::Modifiers(held) => Some(*held),
        _ => None,
    });
    eprintln!("told: {:?}; inputs: {inputs:?}", focus_and_keys(&inputs));
    assert!(
        !last_held.is_some_and(|held| held.shift),
        "Shift let go, still told held: {inputs:?}"
    );
}

/// The server's focus following the pointer, which lies over the window, while another client
/// grabs the keyboard on the root window, above the window, as a program that binds keys where no
/// window manager runs: the server sends the window a focus out then a focus in, both of the mode
/// `Grab`; the window must be told the focus lost until the grab ends.
#[test]
fn a_keyboard_grabbed_on_the_root_while_the_focus_follows_the_pointer_is_told_lost() {
    use x11rb::protocol::xproto::{GrabMode, GrabStatus, InputFocus};
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let grabber = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        connection
            .set_input_focus(
                InputFocus::POINTER_ROOT,
                u32::from(InputFocus::POINTER_ROOT),
                x11rb::CURRENT_TIME,
            )
            .unwrap()
            .check()
            .unwrap();
        connection
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, 160, 120)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "a"]);
        let grabbed = connection
            .grab_keyboard(
                false,
                root,
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .unwrap()
            .reply()
            .unwrap()
            .status;
        std::thread::sleep(Duration::from_millis(300));
        // Typed into the grab's client, Shift held: never this window's.
        xdotool(&["key", "shift+b"]);
        connection
            .ungrab_keyboard(x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "c"]);
        std::thread::sleep(Duration::from_millis(200));
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 800)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let _ = stop_sender.send(());
        grabbed
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    let grabbed = grabber.join().expect("the grabber");
    assert_eq!(grabbed, GrabStatus::SUCCESS, "the keyboard grabbed");
    let told = focus_and_keys(&recorder.told.inputs);
    eprintln!("told: {told:?}");
    assert_eq!(
        told,
        [
            "focus true",
            "key a",
            "focus false",
            "focus true",
            "key c",
            "focus false"
        ],
        "a grab on the root window, above the window under the pointer"
    );
    let shifted = recorder
        .told
        .inputs
        .iter()
        .any(|input| matches!(input, Input::Modifiers(held) if held.shift));
    assert!(
        !shifted,
        "Shift typed into the grab's client was told to this window: {:?}",
        recorder.told.inputs
    );
}

/// The server's focus on another window while another client grabs the keyboard, then given to
/// the window during the grab, as a window manager's switcher picks it before letting the keyboard
/// go: the keys go to the grab's client until it ends, so the window must be told focused only as
/// it ends, before `b`.
#[test]
fn the_focus_given_during_another_clients_keyboard_grab_is_told_as_the_grab_ends() {
    use x11rb::protocol::xproto::{GrabMode, GrabStatus, InputFocus};
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let grabber = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        let menu = other_window(&connection, root);
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 400)
            .unwrap()
            .check()
            .unwrap();
        let focus = |to: u32| {
            connection
                .set_input_focus(InputFocus::POINTER_ROOT, to, x11rb::CURRENT_TIME)
                .unwrap()
                .check()
                .unwrap();
            std::thread::sleep(Duration::from_millis(300));
        };
        focus(menu);
        let grabbed = connection
            .grab_keyboard(
                false,
                menu,
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .unwrap()
            .reply()
            .unwrap()
            .status;
        std::thread::sleep(Duration::from_millis(300));
        focus(window);
        // Typed into the grab's client, Shift held: never this window's.
        xdotool(&["key", "shift+a"]);
        std::thread::sleep(Duration::from_millis(200));
        connection
            .ungrab_keyboard(x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "b"]);
        std::thread::sleep(Duration::from_millis(200));
        focus(u32::from(InputFocus::POINTER_ROOT));
        let _ = stop_sender.send(());
        grabbed
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    let grabbed = grabber.join().expect("the grabber");
    assert_eq!(grabbed, GrabStatus::SUCCESS, "the keyboard grabbed");
    let told = focus_and_keys(&recorder.told.inputs);
    eprintln!("told: {told:?}");
    assert_eq!(
        told,
        ["focus true", "key b", "focus false"],
        "the focus given during the grab"
    );
    let shifted = recorder
        .told
        .inputs
        .iter()
        .any(|input| matches!(input, Input::Modifiers(held) if held.shift));
    assert!(
        !shifted,
        "Shift typed into the grab's client was told to this window: {:?}",
        recorder.told.inputs
    );
}

/// The window holding the server's focus while another client grabs the keyboard on the window
/// itself: the keys go to the grab's client, so the window must be told the focus lost until the
/// grab ends.
#[test]
fn a_keyboard_grabbed_on_the_window_itself_by_another_client_is_told_lost() {
    use x11rb::protocol::xproto::{GrabMode, GrabStatus, InputFocus};
    server_settles();
    let (window_sender, window_receiver) = mpsc::channel::<u32>();
    let (stop_sender, stop_receiver) = mpsc::channel();
    let grabber = std::thread::spawn(move || {
        let (connection, screen) = x11rb::connect(None).expect("the X server");
        let root = connection.setup().roots[screen].root;
        let window = window_receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the window's id");
        connection
            .warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1000, 400)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        connection
            .set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let grabbed = connection
            .grab_keyboard(
                false,
                window,
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .unwrap()
            .reply()
            .unwrap()
            .status;
        std::thread::sleep(Duration::from_millis(300));
        // Typed into the grab's client, Shift held: never this window's.
        xdotool(&["key", "shift+a"]);
        connection
            .ungrab_keyboard(x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        xdotool(&["key", "b"]);
        std::thread::sleep(Duration::from_millis(200));
        let _ = stop_sender.send(());
        grabbed
    });
    let mut recorder = Recorder::new(Duration::from_secs(10), false, Some(window_sender));
    recorder.stop = Some(stop_receiver);
    let (recorder, ran) = uwui_windowing::run(opening(false), recorder);
    ran.expect("the run");
    let grabbed = grabber.join().expect("the grabber");
    assert_eq!(grabbed, GrabStatus::SUCCESS, "the keyboard grabbed");
    let told = focus_and_keys(&recorder.told.inputs);
    eprintln!("told: {told:?}");
    assert_eq!(
        told,
        ["focus true", "focus false", "focus true", "key b"],
        "a grab on the window itself"
    );
    let shifted = recorder
        .told
        .inputs
        .iter()
        .any(|input| matches!(input, Input::Modifiers(held) if held.shift));
    assert!(
        !shifted,
        "Shift typed into the grab's client was told to this window: {:?}",
        recorder.told.inputs
    );
}
