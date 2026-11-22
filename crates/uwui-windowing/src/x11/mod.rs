//! uwui's platform layer on X11, on `x11rb` over libxcb loaded at run time
//! (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`,
//! `docs/adr/0311-m6d7b-cut-in-three-x11s-window-frames-and-pointer-first-on-x11rb.md`).
//!
//! It opens two connections to the server that `DISPLAY` names: its own, which makes the window
//! and reads every event; and Vulkan's driver's, which the window's raw handle gives, and which
//! the driver presents on from threads of its own. The layer never reads the driver's: it waits
//! on its own socket alone, which no other thread drains, so that no event of its own sits read
//! and unseen while the loop waits.
//!
//! - *The window*: its title (`_NET_WM_NAME` and `WM_NAME`), its class, the app id twice, its
//!   minimum size (`WM_NORMAL_HINTS`), filling the screen through `_NET_WM_STATE`, its closing
//!   (`WM_DELETE_WINDOW`) told to the host rather than done, the window manager's pings answered,
//!   its process and machine named, its focus given by the window manager (`WM_HINTS`). Its size
//!   is the window manager's; without a window manager, as under Xvfb, it opens at the size asked
//!   and its requests do nothing.
//! - *Its scale*: the X resources' `Xft.dpi` over 96, the root window's `RESOURCE_MANAGER`, read
//!   again each time the window takes the keyboard's focus: following the property itself would
//!   wake the loop at each change of any property of the root window, as a status bar that writes
//!   the time each second. X11 counts in physical pixels: the window opens at its logical size
//!   times the scale, and keeps its pixels when the scale changes, its logical size following.
//! - *Activation*: `_NET_ACTIVE_WINDOW`, named by the person's last press, and
//!   `_NET_WM_STATE_DEMANDS_ATTENTION`; the start's `DESKTOP_STARTUP_ID` told ended, as the
//!   startup notification's specification asks.
//! - *Its frames' signal*: Present's notices, or the layer's own timer where the server gives no
//!   Present; when each image showed, where the driver presents through Present; the display's
//!   period from RandR (`frames`). A window unmapped, or covered whole where no compositor keeps
//!   it, holds its signal until it shows again; an exposed one asks to be drawn again.
//! - *The pointer*: XInput2's moves, buttons and wheel (`pointer`), each with the server's time
//!   anchored on the host's clock (`crate::anchor`); entering and leaving with their reception's.
//!   Its shapes, from the person's cursor theme (`cursor`).
//! - *Touch*: XInput 2.2's touches, each finger a pointer of its own (`crate::fingers`), with the
//!   server's times; where the server gives XInput 2.1 alone, none.
//! - *The keyboard*: its keymap, keys, modifiers and focus (`keyboard`), the keys with the server's
//!   times, the rest with their reception's.

mod cursor;
mod frames;
mod keyboard;
mod pointer;
mod touch;

use std::ffi::c_void;
use std::num::NonZeroU32;
use std::os::fd::AsFd;
use std::ptr::NonNull;
use std::sync::Arc;
use std::task::Waker;
use std::time::Duration;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle, XcbDisplayHandle, XcbWindowHandle};
use slime_input::{ButtonState, CursorShape};
use uwui_time::{EventTime, Instant};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::properties::{WmHints, WmSizeHints};
use x11rb::protocol::Event;
use x11rb::protocol::present::{self, ConnectionExt as _};
use x11rb::protocol::randr::{self, ConnectionExt as _};
use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::xkb;
use x11rb::protocol::xproto::{
    self, AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
    Visibility, WindowClass,
};
use x11rb::resource_manager::Database;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

use crate::anchor::Anchor;
use crate::clock::{self, Clock, Suspends};
use crate::fingers::Fingers;
use crate::wait::Waits;
use crate::wake::{self, Asks};
use crate::{Activation, Configured, Drawn, Error, Happened, Input, Opening, running};

use cursor::Cursors;
use frames::Frames;
use keyboard::Keys;
use pointer::Pointers;

/// How far before its reception a time of Present may lie and still be read: a time further, or
/// after it, is not on this machine's clock, as from a server elsewhere, and the reception counts.
const PRESENT_TIME_RANGE: Duration = Duration::from_secs(1);

x11rb::atom_manager! {
    /// The atoms that the window's properties and messages name.
    Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        UTF8_STRING,
        _NET_WM_NAME,
        _NET_WM_PID,
        _NET_WM_PING,
        _NET_WM_STATE,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_DEMANDS_ATTENTION,
        _NET_ACTIVE_WINDOW,
        _NET_STARTUP_ID,
        _NET_STARTUP_INFO_BEGIN,
        _NET_STARTUP_INFO,
        _NET_SUPPORTING_WM_CHECK,
    }
}

/// `_NET_WM_STATE`'s actions, from the extended window manager hints.
const STATE_REMOVE: u32 = 0;
const STATE_ADD: u32 = 1;
/// A request's source in the extended window manager hints: an application.
const FROM_APPLICATION: u32 = 1;
/// XInput2's device id for every master device.
const ALL_MASTER_DEVICES: u16 = 1;
/// The server's focus that follows the pointer, as `GetInputFocus` names it.
const POINTER_ROOT: xproto::Window = 1;

/// The X resources' dots per inch at a scale of 1, as Xft and the toolkits that read it count.
const DPI_AT_SCALE_ONE: f64 = 96.0;

/// The scales that `Xft.dpi` may give, beyond which it is held: a resource mistyped still leaves an
/// interface that the person can read and use.
const SCALES: (f64, f64) = (0.5, 8.0);

/// What came since the last turn, gathered from the events as they are read.
#[derive(Debug, Default)]
struct Gathered {
    inputs: Vec<(EventTime, Input)>,
    configured: Option<Configured>,
    drawn: Option<Drawn>,
    close: bool,
    exposed: bool,
}

impl Gathered {
    /// How much is gathered: what changes as anything comes for the host.
    fn count(&self) -> (usize, bool, bool, bool, bool) {
        (
            self.inputs.len(),
            self.configured.is_some(),
            self.drawn.is_some(),
            self.close,
            self.exposed,
        )
    }
}

/// The layer on X11: its connections, the window and its loop's state.
pub(crate) struct Platform {
    connection: XCBConnection,
    /// Vulkan's driver's connection, which the window's raw handle names.
    driver: XCBConnection,
    screen: i32,
    root: xproto::Window,
    window: xproto::Window,
    atoms: Atoms,
    waits: Waits,
    asks: Arc<Asks>,
    suspends: Suspends,
    anchor: Anchor,
    pointers: Pointers,
    /// The fingers on the window, by their device and XInput2's number of the touch.
    fingers: Fingers<(u16, u32)>,
    keys: Keys,
    cursors: Cursors,
    frames: Frames,
    gathered: Gathered,
    /// Whether RandR tells the displays' modes and their changes.
    randr: bool,
    /// The display's period must be read again: the window moved, or the displays changed.
    period_stale: bool,
    /// The window's size in pixels, and whether it fills the screen.
    size: [u32; 2],
    fullscreen: bool,
    /// The physical pixels per logical one, from `Xft.dpi`.
    scale: f64,
    /// The X resources' text last read, which the scale and the cursor's theme follow.
    resources: Vec<u8>,
    /// The smallest size that the person may give the window, in logical pixels.
    min_size: Option<[u32; 2]>,
    /// Whether the window is mapped, and whether it is covered whole.
    mapped: bool,
    covered: bool,
    /// The server's time of the person's last press, which activation names.
    last_press: u32,
    /// The number of the next image presented.
    next_image: u64,
    /// The clock that Present's times are on: Linux's monotonic one.
    present_clock: Option<Clock>,
}

impl Drop for Platform {
    fn drop(&mut self) {
        // The connections close as they drop, after the window, which the server would destroy
        // with its connection anyway.
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}

impl Platform {
    /// Connects to the server, opens the window of `opening` and sets the termination's handler:
    /// the platform, and how the window draws.
    pub(crate) fn open(opening: Opening) -> Result<(Self, Drawn), Error> {
        let waits = Waits::new().map_err(|cause| Error::new("the loop's waits", cause))?;
        let asks = Asks::new(waits.notifier());
        let (connection, screen) = XCBConnection::connect(None)
            .map_err(|cause| Error::new("no X server answers", cause))?;
        let (driver, _) = XCBConnection::connect(None)
            .map_err(|cause| Error::new("no X server answers the driver's connection", cause))?;
        let Some(setup_screen) = connection.setup().roots.get(screen) else {
            return Err(Error("the X server has no such screen".to_owned()));
        };
        let root = setup_screen.root;
        let root_visual = setup_screen.root_visual;
        let atoms = Atoms::new(&connection)
            .map_err(|cause| Error::new("the X server's atoms", cause))?
            .reply()
            .map_err(|cause| Error::new("the X server's atoms", cause))?;
        let resources = read_resources(&connection);
        let database = Database::new_from_data(&resources);
        let scale = scale_of(&database);
        let size = opening
            .size
            .map(|side| pixels_of(side, scale).clamp(1, u32::from(u16::MAX)));
        let window = connection
            .generate_id()
            .map_err(|cause| Error::new("the window's id", cause))?;
        connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                root,
                0,
                0,
                side16(size[0]),
                side16(size[1]),
                0,
                WindowClass::INPUT_OUTPUT,
                root_visual,
                &CreateWindowAux::new()
                    // No background: the server paints nothing over the image before the next.
                    .background_pixmap(x11rb::NONE)
                    .bit_gravity(xproto::Gravity::NORTH_WEST)
                    .event_mask(
                        EventMask::STRUCTURE_NOTIFY
                            | EventMask::PROPERTY_CHANGE
                            | EventMask::VISIBILITY_CHANGE
                            | EventMask::EXPOSURE
                            // The keyboard's focus, which the server tells this way where
                            // XInput2 tells nothing, as for a move to an ancestor.
                            | EventMask::FOCUS_CHANGE,
                    ),
            )
            .map_err(|cause| Error::new("the window", cause))?;
        let keys = Keys::new(&connection);
        let mut cursors = Cursors::new(&connection, screen, window, &database);
        // The arrow of the person's theme, in place of the root window's cursor.
        cursors.show(&connection, CursorShape::Default);
        let mut platform = Self {
            connection,
            driver,
            screen: i32::try_from(screen).unwrap_or(0),
            root,
            window,
            atoms,
            waits,
            asks,
            suspends: Suspends::new(),
            anchor: Anchor::default(),
            pointers: Pointers::default(),
            fingers: Fingers::default(),
            keys,
            cursors,
            frames: Frames::new(false),
            gathered: Gathered::default(),
            randr: false,
            period_stale: true,
            size,
            fullscreen: false,
            scale,
            resources,
            min_size: opening.min_size,
            mapped: false,
            covered: false,
            last_press: 0,
            next_image: 0,
            present_clock: Clock::from_raw(1),
        };
        let startup = std::env::var("DESKTOP_STARTUP_ID").unwrap_or_default();
        platform.describe(&opening, &startup)?;
        platform.select_input()?;
        platform
            .connection
            .map_window(window)
            .map_err(|cause| Error::new("the window", cause))?;
        platform.started(&startup);
        platform
            .connection
            .flush()
            .map_err(|cause| Error::new("the connection to the X server", cause))?;
        platform.frames.set_shown(false);
        // The first turn tells the window's first size, as a Wayland compositor's first configure.
        platform.configured();
        wake::on_termination(&platform.asks);
        let drawn = platform.drawn();
        Ok((platform, drawn))
    }

    /// Gives the window its properties: its title, class, process, protocols, minimum size, and
    /// whether it opens filling the screen.
    fn describe(&mut self, opening: &Opening, startup: &str) -> Result<(), Error> {
        let failed = |cause| Error::new("the window's properties", cause);
        self.set_title_properties(&opening.title).map_err(failed)?;
        // The instance's name, then the class's, each ended by a zero.
        let class = format!("{0}\0{0}\0", opening.app_id);
        let atoms = self.atoms;
        let connection = &self.connection;
        let window = self.window;
        connection
            .change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                class.as_bytes(),
            )
            .map_err(failed)?;
        connection
            .change_property32(
                PropMode::REPLACE,
                window,
                atoms.WM_PROTOCOLS,
                AtomEnum::ATOM,
                &[atoms.WM_DELETE_WINDOW, atoms._NET_WM_PING],
            )
            .map_err(failed)?;
        // The process, which a window manager may end when the window stops answering its pings,
        // with the machine that it runs on, without which its number means nothing.
        let machine = rustix::system::uname();
        connection
            .change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_CLIENT_MACHINE,
                AtomEnum::STRING,
                machine.nodename().to_bytes(),
            )
            .map_err(failed)?;
        connection
            .change_property32(
                PropMode::REPLACE,
                window,
                atoms._NET_WM_PID,
                AtomEnum::CARDINAL,
                &[std::process::id()],
            )
            .map_err(failed)?;
        self.set_size_hints().map_err(failed)?;
        // The window manager gives the window the focus, which it takes as given, as ICCCM's
        // passive model asks.
        let mut hints = WmHints::new();
        hints.input = Some(true);
        hints.set(connection, window).map_err(failed)?;
        // Before the map, which a window manager reads it at, for its focus.
        if !startup.is_empty() {
            connection
                .change_property8(
                    PropMode::REPLACE,
                    window,
                    atoms._NET_STARTUP_ID,
                    atoms.UTF8_STRING,
                    startup.as_bytes(),
                )
                .map_err(failed)?;
        }
        if opening.fullscreen {
            // Before the window maps, the state is the property itself.
            self.connection
                .change_property32(
                    PropMode::REPLACE,
                    window,
                    atoms._NET_WM_STATE,
                    AtomEnum::ATOM,
                    &[atoms._NET_WM_STATE_FULLSCREEN],
                )
                .map_err(failed)?;
        }
        Ok(())
    }

    /// Selects the extensions' events: XInput2's pointer, Present's notices, RandR's changes.
    fn select_input(&mut self) -> Result<(), Error> {
        let connection = &self.connection;
        // XInput 2.1 gives the scroll valuators, 2.2 touch; the server answers the highest that
        // both know.
        let version = if connection
            .extension_information(xinput::X11_EXTENSION_NAME)
            .map_err(|cause| Error::new("the X server's extensions", cause))?
            .is_some()
        {
            connection
                .xinput_xi_query_version(2, 2)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map(|version| (version.major_version, version.minor_version))
        } else {
            None
        };
        if version.is_none_or(|version| version < (2, 1)) {
            return Err(Error(
                "the X server gives no XInput 2.1, which the window's pointer needs".to_owned(),
            ));
        }
        let mut mask = xinput::XIEventMask::MOTION
            | xinput::XIEventMask::BUTTON_PRESS
            | xinput::XIEventMask::BUTTON_RELEASE
            | xinput::XIEventMask::ENTER
            | xinput::XIEventMask::LEAVE
            | xinput::XIEventMask::DEVICE_CHANGED
            | xinput::XIEventMask::KEY_PRESS
            | xinput::XIEventMask::KEY_RELEASE;
        if version.is_some_and(|version| version >= (2, 2)) {
            mask |= xinput::XIEventMask::TOUCH_BEGIN
                | xinput::XIEventMask::TOUCH_UPDATE
                | xinput::XIEventMask::TOUCH_END;
        } else {
            tracing::info!("the X server gives no XInput 2.2: the window takes no touch");
        }
        connection
            .xinput_xi_select_events(
                self.window,
                &[xinput::EventMask {
                    deviceid: ALL_MASTER_DEVICES,
                    mask: vec![mask],
                }],
            )
            .map_err(|cause| Error::new("the pointer's and keyboard's events", cause))?;
        // Each master pointer's scroll valuators, which change as it switches devices.
        match connection
            .xinput_xi_query_device(ALL_MASTER_DEVICES)
            .map_err(|cause| Error::new("the pointer's devices", cause))?
            .reply()
        {
            Ok(devices) => {
                for device in devices.infos {
                    self.pointers.classes(device.deviceid, &device.classes);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "the pointers' devices: the wheel's valuators are not read")
            }
        }
        let present = has_extension(connection, present::X11_EXTENSION_NAME)
            && connection
                .present_query_version(1, 0)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .is_some();
        if present {
            let id = connection
                .generate_id()
                .map_err(|cause| Error::new("Present's events", cause))?;
            connection
                .present_select_input(id, self.window, present::EventMask::COMPLETE_NOTIFY)
                .map_err(|cause| Error::new("Present's events", cause))?;
        } else {
            tracing::info!(
                "the X server gives no Present: the window's frames follow a timer at the \
                 display's rate"
            );
        }
        self.frames = Frames::new(present);
        // RandR 1.3 gives the current resources without probing the outputs.
        self.randr = has_extension(connection, randr::X11_EXTENSION_NAME)
            && connection
                .randr_query_version(1, 3)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .is_some_and(|version| (version.major_version, version.minor_version) >= (1, 3));
        if self.randr {
            connection
                .randr_select_input(
                    self.window,
                    randr::NotifyMask::SCREEN_CHANGE | randr::NotifyMask::CRTC_CHANGE,
                )
                .map_err(|cause| Error::new("RandR's events", cause))?;
        } else {
            tracing::info!("the X server gives no RandR 1.3: the display's period is not read");
        }
        Ok(())
    }

    /// Tells the launcher that started the program, through the startup notification's message,
    /// that its window opened, if the environment names the start, `id`. The variable stays set,
    /// as on Wayland.
    fn started(&mut self, id: &str) {
        if id.is_empty() {
            return;
        }
        let message = startup_removal(id);
        for (index, chunk) in message.chunks(20).enumerate() {
            let mut data = [0_u8; 20];
            data[..chunk.len()].copy_from_slice(chunk);
            let kind = if index == 0 {
                self.atoms._NET_STARTUP_INFO_BEGIN
            } else {
                self.atoms._NET_STARTUP_INFO
            };
            let event = ClientMessageEvent::new(8, self.window, kind, data);
            let _ = self
                .connection
                .send_event(false, self.root, EventMask::PROPERTY_CHANGE, event);
        }
    }

    fn set_title_properties(&self, title: &str) -> Result<(), x11rb::errors::ConnectionError> {
        for name in [self.atoms._NET_WM_NAME, AtomEnum::WM_NAME.into()] {
            self.connection.change_property8(
                PropMode::REPLACE,
                self.window,
                name,
                self.atoms.UTF8_STRING,
                title.as_bytes(),
            )?;
        }
        Ok(())
    }

    /// Tells the window manager the smallest size of the window, in pixels at its scale.
    fn set_size_hints(&self) -> Result<(), x11rb::errors::ConnectionError> {
        let mut hints = WmSizeHints::new();
        hints.min_size = self.min_size.map(|logical| {
            let [width, height] = logical.map(|side| pixels_of(side, self.scale));
            (side32(width), side32(height))
        });
        hints.set_normal_hints(&self.connection, self.window)?;
        Ok(())
    }

    /// Sends the root window a message of the extended window manager hints about the window.
    fn tell_window_manager(&self, kind: u32, data: [u32; 5]) {
        let event = ClientMessageEvent::new(32, self.window, kind, data);
        let _ = self.connection.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
    }

    /// Reads every event that the server sent, without waiting: whether any tells the host
    /// something, an input, a change of the window, or the frames' signal. Another client's
    /// property of the root window, a modifier held in another window, or a window manager's
    /// ping tell it nothing.
    fn read_events(&mut self) -> Result<bool, Error> {
        let mut told = false;
        while let Some(event) = self
            .connection
            .poll_for_event()
            .map_err(|cause| Error::new("the connection to the X server", cause))?
        {
            let frames = touches_frames(&event);
            let before = self.gathered.count();
            self.event(event, clock::now());
            told |= frames || self.gathered.count() != before;
        }
        Ok(told)
    }

    /// What `event`, received at `received`, tells.
    fn event(&mut self, event: Event, received: Instant) {
        match event {
            Event::XinputMotion(motion) => {
                let emulated = emulated(motion.flags);
                let time = self.time(motion.time, received);
                let moved = pointer::Event::Moved {
                    device: motion.deviceid,
                    x: pointer::pixels(motion.event_x),
                    y: pointer::pixels(motion.event_y),
                    mask: &motion.valuator_mask,
                    values: &motion.axisvalues,
                };
                for input in self.pointers.input(moved, emulated) {
                    self.gathered.inputs.push((time, input));
                }
            }
            Event::XinputButtonPress(press) => {
                self.last_press = press.time;
                self.button(&press, ButtonState::Pressed, received);
            }
            Event::XinputButtonRelease(release) => {
                self.button(&release, ButtonState::Released, received);
            }
            Event::XinputEnter(enter) => {
                let entered = pointer::Event::Entered {
                    x: pointer::pixels(enter.event_x),
                    y: pointer::pixels(enter.event_y),
                };
                for input in self.pointers.input(entered, false) {
                    self.gathered
                        .inputs
                        .push((EventTime::received(received), input));
                }
                // Another client's grab of the pointer moves neither the pointer's place for
                // the keys nor their focus.
                if !grab_crossing(enter.mode) {
                    self.focus(keyboard::Focus::Entered { focus: enter.focus }, received);
                }
            }
            Event::XinputLeave(leave) => {
                for input in self.pointers.input(pointer::Event::Left, false) {
                    self.gathered
                        .inputs
                        .push((EventTime::received(received), input));
                }
                if !grab_crossing(leave.mode) {
                    self.focus(keyboard::Focus::Left, received);
                }
            }
            Event::XinputDeviceChanged(changed) => {
                self.pointers.classes(changed.deviceid, &changed.classes);
            }
            Event::XinputKeyPress(press) => {
                self.last_press = press.time;
                let repeat = press.flags.contains(xinput::KeyEventFlags::KEY_REPEAT);
                self.key(&press, ButtonState::Pressed, repeat, received);
            }
            Event::XinputKeyRelease(release) => {
                self.key(&release, ButtonState::Released, false, received);
            }
            Event::FocusIn(focus) if focus.event == self.window => match focus.mode {
                xproto::NotifyMode::NORMAL | xproto::NotifyMode::UNGRAB => {
                    self.server_focus(received);
                }
                _ => {}
            },
            Event::FocusOut(focus) if focus.event == self.window => match focus.mode {
                xproto::NotifyMode::NORMAL => self.server_focus(received),
                xproto::NotifyMode::GRAB => self.focus(keyboard::Focus::Grabbed, received),
                _ => {}
            },
            Event::XkbStateNotify(state) if self.keys.follows(state.device_id) => {
                let (modifiers, layouts) = keyboard::state_of(&state);
                let input = self.keys.state(modifiers, layouts);
                self.push_received(received, input);
            }
            Event::XkbNewKeyboardNotify(new)
                if self.keys.follows(new.device_id)
                    && new.changed.contains(xkb::NKNDetail::KEYCODES) =>
            {
                let input = self.keys.keymap_changed(&self.connection);
                self.push_received(received, input);
            }
            Event::XkbMapNotify(map) if self.keys.follows(map.device_id) => {
                let input = self.keys.keymap_changed(&self.connection);
                self.push_received(received, input);
            }
            Event::XinputTouchBegin(event) => {
                self.last_press = event.time;
                self.touch(touch::Phase::Began, &event, received);
            }
            Event::XinputTouchUpdate(event) => self.touch(touch::Phase::Moved, &event, received),
            Event::XinputTouchEnd(event) => self.touch(touch::Phase::Ended, &event, received),
            Event::PresentCompleteNotify(complete) if complete.window == self.window => {
                let placed = self
                    .present_clock
                    .map(|clock| clock.on_host(complete.ust.saturating_mul(1000)));
                let at = present_time(placed, received);
                self.frames.completed(
                    complete.kind == present::CompleteKind::NOTIFY_MSC,
                    complete.mode == present::CompleteMode::SKIP,
                    at,
                );
            }
            Event::ConfigureNotify(configure) if configure.window == self.window => {
                let size = [u32::from(configure.width), u32::from(configure.height)]
                    .map(|side| side.max(1));
                if size != self.size {
                    self.size = size;
                    self.configured();
                    self.gathered.drawn = Some(self.drawn());
                }
                self.period_stale = true;
            }
            Event::PropertyNotify(property)
                if property.window == self.window && property.atom == self.atoms._NET_WM_STATE =>
            {
                // The layer writes the state itself before the window maps: only a window manager
                // makes it the window's.
                let fullscreen = self.window_manager_runs() && self.read_fullscreen();
                if fullscreen != self.fullscreen {
                    self.fullscreen = fullscreen;
                    self.configured();
                }
            }
            Event::MapNotify(map) if map.window == self.window => {
                self.mapped = true;
                self.shown_changed();
                self.period_stale = true;
            }
            Event::UnmapNotify(unmap) if unmap.window == self.window => {
                self.mapped = false;
                self.shown_changed();
            }
            Event::VisibilityNotify(visibility) if visibility.window == self.window => {
                self.covered = visibility.state == Visibility::FULLY_OBSCURED;
                self.shown_changed();
            }
            Event::Expose(expose) if expose.window == self.window => {
                self.gathered.exposed = true;
            }
            Event::ClientMessage(message)
                if message.window == self.window && message.type_ == self.atoms.WM_PROTOCOLS =>
            {
                let data = message.data.as_data32();
                if data[0] == self.atoms.WM_DELETE_WINDOW {
                    self.gathered.close = true;
                } else if data[0] == self.atoms._NET_WM_PING {
                    // Answered by sending it back to the root window.
                    let mut answer = message;
                    answer.window = self.root;
                    let _ = self.connection.send_event(
                        false,
                        self.root,
                        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                        answer,
                    );
                }
            }
            Event::RandrScreenChangeNotify(_) | Event::RandrNotify(_) => {
                self.period_stale = true;
            }
            Event::Error(error) => {
                tracing::warn!(?error, "the X server refused a request of the window's");
            }
            _ => {}
        }
    }

    fn button(&mut self, event: &xinput::ButtonPressEvent, state: ButtonState, received: Instant) {
        let time = self.time(event.time, received);
        let button = pointer::Event::Button {
            button: event.detail,
            state,
        };
        for input in self.pointers.input(button, emulated(event.flags)) {
            self.gathered.inputs.push((time, input));
        }
    }

    /// The time of an event that the server stamped `millis`, received at `received`.
    fn time(&mut self, millis: u32, received: Instant) -> EventTime {
        EventTime::anchored(self.anchor.time(millis, received))
    }

    /// What a touch's `event` at `phase`, received at `received`, gives.
    fn touch(&mut self, phase: touch::Phase, event: &xinput::TouchBeginEvent, received: Instant) {
        let touched = touch::touch(
            &mut self.fingers,
            phase,
            event.sourceid,
            event.detail,
            [event.event_x, event.event_y],
        );
        self.push_received(received, touched.received);
        if !touched.timed.is_empty() {
            let time = self.time(event.time, received);
            for input in touched.timed {
                self.gathered.inputs.push((time, input));
            }
        }
    }

    /// The key of `event` going down or up, received at `received`: the focus given, if it was not,
    /// since the server sends a key only to the window that holds the keyboard, another client's
    /// grab included as ended; the key translated in its own event's state.
    fn key(
        &mut self,
        event: &xinput::KeyPressEvent,
        state: ButtonState,
        repeat: bool,
        received: Instant,
    ) {
        if !self.keys.focused() {
            self.focus(keyboard::Focus::Key, received);
        }
        let own = keyboard::key_state(&event.mods, &event.group);
        let time = self.time(event.time, received);
        for input in self.keys.key_in(event.detail, state, repeat, own) {
            // The key at its time; the modifiers told around it at their reception's.
            let at = if matches!(input, Input::Key(_)) {
                time
            } else {
                EventTime::received(received)
            };
            self.gathered.inputs.push((at, input));
        }
    }

    /// What `event`, received at `received`, makes of the keyboard's focus; XKB's state followed
    /// as the window takes it or loses it, the X resources read again as it takes it, and the
    /// modifiers told released as it loses it.
    fn focus(&mut self, event: keyboard::Focus, received: Instant) {
        let before = self.keys.focused();
        let told = self.keys.focus(event);
        self.push_received(received, told);
        if self.keys.focused() != before {
            let held = self.keys.follow_state(&self.connection);
            self.push_received(received, held);
            if self.keys.focused() {
                self.read_resources_again();
            } else {
                let released = self.keys.released();
                self.push_received(received, released);
            }
        }
    }

    /// Reads where the server's focus now is, at a focus event of the window's, received at
    /// `received`: one round trip, and one more for each ancestor walked through.
    fn server_focus(&mut self, received: Instant) {
        let focus = self
            .connection
            .get_input_focus()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.focus);
        let place = focus_place(focus, self.window, |focus| self.is_ancestor(focus));
        self.focus(place, received);
    }

    /// Whether `window` is an ancestor of the layer's window: its parent, the window manager's
    /// frame, up to the root.
    fn is_ancestor(&self, window: xproto::Window) -> bool {
        let mut at = self.window;
        // A window's ancestors are few; the bound keeps a broken tree from looping.
        for _ in 0..64 {
            let Some(tree) = self
                .connection
                .query_tree(at)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
            else {
                return false;
            };
            if tree.parent == window {
                return true;
            }
            if tree.parent == x11rb::NONE || tree.parent == tree.root {
                return false;
            }
            at = tree.parent;
        }
        false
    }

    /// Gives `input`, if any, at its reception's time.
    fn push_received(&mut self, received: Instant, input: Option<Input>) {
        if let Some(input) = input {
            self.gathered
                .inputs
                .push((EventTime::received(received), input));
        }
    }

    /// The window's size or state changed: the next image shows it.
    fn configured(&mut self) {
        self.gathered.configured = Some(Configured {
            size: self.size.map(|side| logical_of(side, self.scale)),
            fullscreen: self.fullscreen,
        });
    }

    /// How the window draws now: its pixels, at its scale.
    fn drawn(&self) -> Drawn {
        Drawn {
            pixels: self.size,
            scale: self.scale,
        }
    }

    /// Reads the X resources again; where they changed, the scale that their `Xft.dpi` gives, and
    /// the cursor's theme and size, follow them. The window keeps its pixels; its logical size
    /// follows its scale.
    fn read_resources_again(&mut self) {
        let resources = read_resources(&self.connection);
        if resources == self.resources {
            return;
        }
        self.resources = resources;
        let database = Database::new_from_data(&self.resources);
        let screen = usize::try_from(self.screen).unwrap_or(0);
        self.cursors
            .resources_changed(&self.connection, screen, &database);
        let scale = scale_of(&database);
        if scale != self.scale {
            self.scale = scale;
            let _ = self.set_size_hints();
            self.configured();
            self.gathered.drawn = Some(self.drawn());
        }
    }

    fn shown_changed(&mut self) {
        self.frames.set_shown(self.mapped && !self.covered);
    }

    /// Whether a window manager of the extended hints runs: the window that the root's
    /// `_NET_SUPPORTING_WM_CHECK` names names itself the same way, as the hints ask.
    fn window_manager_runs(&self) -> bool {
        let checked = |window: xproto::Window| {
            self.connection
                .get_property(
                    false,
                    window,
                    self.atoms._NET_SUPPORTING_WM_CHECK,
                    AtomEnum::WINDOW,
                    0,
                    1,
                )
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .and_then(|reply| reply.value32()?.next())
        };
        checked(self.root).is_some_and(|manager| checked(manager) == Some(manager))
    }

    /// Whether the window's `_NET_WM_STATE` says that it fills the screen.
    fn read_fullscreen(&self) -> bool {
        let reply = self
            .connection
            .get_property(
                false,
                self.window,
                self.atoms._NET_WM_STATE,
                AtomEnum::ATOM,
                0,
                64,
            )
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        reply
            .and_then(|reply| {
                reply
                    .value32()
                    .map(|mut atoms| atoms.any(|atom| atom == self.atoms._NET_WM_STATE_FULLSCREEN))
            })
            .unwrap_or(false)
    }

    /// Reads the display's period again, from the CRTC that shows most of the window.
    fn read_period(&mut self) {
        self.period_stale = false;
        if !self.randr {
            return;
        }
        let period = self.window_period().unwrap_or_else(|error| {
            tracing::warn!(%error, "the display's period could not be read");
            None
        });
        self.frames.set_period(period);
    }

    fn window_period(&self) -> Result<Option<Duration>, x11rb::errors::ReplyError> {
        let connection = &self.connection;
        let at = connection
            .translate_coordinates(self.window, self.root, 0, 0)?
            .reply()?;
        let window = (
            i32::from(at.dst_x),
            i32::from(at.dst_y),
            self.size[0],
            self.size[1],
        );
        let resources = connection
            .randr_get_screen_resources_current(self.root)?
            .reply()?;
        let mut crtcs = Vec::with_capacity(resources.crtcs.len());
        for crtc in &resources.crtcs {
            let info = connection
                .randr_get_crtc_info(*crtc, resources.config_timestamp)?
                .reply()?;
            if info.mode == 0 {
                continue;
            }
            let period = resources
                .modes
                .iter()
                .find(|mode| mode.id == info.mode)
                .and_then(|mode| {
                    frames::period(
                        mode.dot_clock,
                        mode.htotal,
                        mode.vtotal,
                        mode.mode_flags.contains(randr::ModeFlag::DOUBLE_SCAN),
                        mode.mode_flags.contains(randr::ModeFlag::INTERLACE),
                    )
                });
            crtcs.push((
                (
                    i32::from(info.x),
                    i32::from(info.y),
                    u32::from(info.width),
                    u32::from(info.height),
                ),
                period,
            ));
        }
        Ok(frames::window_period(window, &crtcs))
    }
}

impl running::Platform for Platform {
    fn happened(&mut self) -> Result<Happened, Error> {
        // Before the events read from here on are timed: their anchor is made again after a
        // suspend. Those that ended the wait were timed as it read them, which the anchor of before
        // the suspend serves for events that came before it, the likely ones.
        let resumed = self.suspends.resumed();
        if resumed.is_some() {
            self.anchor.reset();
        }
        self.read_events()?;
        if self.period_stale {
            self.read_period();
        }
        let (signal, presentations) = self.frames.take(clock::now());
        let gathered = std::mem::take(&mut self.gathered);
        Ok(Happened {
            inputs: gathered.inputs,
            configured: gathered.configured,
            drawn: gathered.drawn,
            close_asked: gathered.close,
            terminated: self.asks.take_terminated(),
            woken: self.asks.take_woken(),
            signal,
            presentations,
            exposed: gathered.exposed,
            resumed,
        })
    }

    fn wait(&mut self, deadline: Option<Instant>) -> Result<(), Error> {
        let deadline = match (deadline, self.frames.deadline()) {
            (Some(deadline), Some(signal)) => Some(deadline.min(signal)),
            (deadline, signal) => deadline.or(signal),
        };
        let flush = |connection: &XCBConnection| {
            connection
                .flush()
                .map_err(|cause| Error::new("the connection to the X server", cause))
        };
        flush(&self.connection)?;
        // Events that a reply's wait read are in libxcb's queue, not on the socket: read first.
        if self.read_events()? {
            return Ok(());
        }
        // Events that tell the host nothing, as another client's property of the root window,
        // leave the loop waiting, without a turn of the host: only a wake, the deadline, the
        // kernel's notice of a resume or a clock set, or an event that tells it something end the
        // wait.
        loop {
            // What the events read asked, as a ping's answer, goes before each wait, the first
            // included.
            flush(&self.connection)?;
            let readable = self
                .waits
                .wait(self.connection.as_fd(), deadline, false)
                .map_err(|cause| Error::new("the loop's wait", cause))?;
            let told = readable && self.read_events()?;
            if told || !readable || self.waits.interrupted() {
                return Ok(());
            }
        }
    }

    fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle) {
        let connection = NonNull::new(self.driver.get_raw_xcb_connection().cast::<c_void>());
        let window = NonZeroU32::new(self.window).expect("the server never names a window 0");
        (
            RawDisplayHandle::Xcb(XcbDisplayHandle::new(connection, self.screen)),
            RawWindowHandle::Xcb(XcbWindowHandle::new(window)),
        )
    }

    fn now(&self) -> Instant {
        clock::now()
    }

    fn waker(&self) -> Waker {
        self.asks.waker()
    }

    fn set_title(&mut self, title: &str) {
        let _ = self.set_title_properties(title);
    }

    /// The hints apply at once: no image needs to follow.
    fn set_min_size(&mut self, size: Option<[u32; 2]>) -> bool {
        self.min_size = size;
        let _ = self.set_size_hints();
        false
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        let action = if fullscreen { STATE_ADD } else { STATE_REMOVE };
        self.tell_window_manager(
            self.atoms._NET_WM_STATE,
            [
                action,
                self.atoms._NET_WM_STATE_FULLSCREEN,
                0,
                FROM_APPLICATION,
                0,
            ],
        );
    }

    fn activate(&mut self, activation: Activation) {
        match activation {
            Activation::Front => self.tell_window_manager(
                self.atoms._NET_ACTIVE_WINDOW,
                [FROM_APPLICATION, self.last_press, 0, 0, 0],
            ),
            Activation::Attention => self.tell_window_manager(
                self.atoms._NET_WM_STATE,
                [
                    STATE_ADD,
                    self.atoms._NET_WM_STATE_DEMANDS_ATTENTION,
                    0,
                    FROM_APPLICATION,
                    0,
                ],
            ),
        }
    }

    fn set_cursor(&mut self, shape: CursorShape) {
        self.cursors.show(&self.connection, shape);
    }

    fn ask_signal(&mut self) -> u64 {
        let image = self.next_image;
        self.next_image += 1;
        if self.frames.asked(image, clock::now()) {
            // The next refresh of the window's CRTC: divisor 1 and remainder 0 name the one after
            // the current.
            let _ = self.connection.present_notify_msc(
                self.window,
                u32::try_from(image % u64::from(u32::MAX)).unwrap_or(0),
                0,
                1,
                0,
            );
        }
        image
    }
}

/// A time of Present received at `received`, `placed` on the host's clock from Linux's monotonic
/// one: the reception's where it lies after it, or more than [`PRESENT_TIME_RANGE`] before, off
/// this machine's clock, as a server elsewhere gives.
fn present_time(placed: Option<Instant>, received: Instant) -> Instant {
    match placed {
        Some(at) if at <= received && received - at <= PRESENT_TIME_RANGE => at,
        _ => received,
    }
}

/// Whether `event` bears on the frames' signal, which the host reads at its next turn: a present
/// told, the window shown or hidden, moved, or the displays changed.
fn touches_frames(event: &Event) -> bool {
    matches!(
        event,
        Event::PresentCompleteNotify(_)
            | Event::MapNotify(_)
            | Event::UnmapNotify(_)
            | Event::VisibilityNotify(_)
            | Event::ConfigureNotify(_)
            | Event::RandrScreenChangeNotify(_)
            | Event::RandrNotify(_)
    )
}

/// Where the server's focus `focus` lies for the window, as `GetInputFocus` names it: on the
/// window itself, or above it, following the pointer (`PointerRoot`) or on one of its ancestors,
/// which `is_ancestor` tells; `None` and a focus not read lie elsewhere.
fn focus_place(
    focus: Option<xproto::Window>,
    window: xproto::Window,
    is_ancestor: impl FnOnce(xproto::Window) -> bool,
) -> keyboard::Focus {
    let (window, above) = match focus {
        Some(focus) if focus == window => (true, false),
        Some(POINTER_ROOT) => (false, true),
        Some(focus) if focus != x11rb::NONE => (false, is_ancestor(focus)),
        _ => (false, false),
    };
    keyboard::Focus::Server { window, above }
}

/// Whether a crossing of the window's border comes from another client's grab of the pointer, or
/// its end, rather than from the pointer's move.
fn grab_crossing(mode: xinput::NotifyMode) -> bool {
    matches!(
        mode,
        xinput::NotifyMode::GRAB
            | xinput::NotifyMode::UNGRAB
            | xinput::NotifyMode::PASSIVE_GRAB
            | xinput::NotifyMode::PASSIVE_UNGRAB
    )
}

/// Whether the server gives the extension `name`.
fn has_extension(connection: &XCBConnection, name: &'static str) -> bool {
    connection
        .extension_information(name)
        .ok()
        .flatten()
        .is_some()
}

/// Whether XInput2 marks an event as the server's copy for older clients.
fn emulated(flags: xinput::PointerEventFlags) -> bool {
    flags.contains(xinput::PointerEventFlags::POINTER_EMULATED)
}

/// The startup notification's message that ends the start `id`, ended by a zero: its value
/// quoted, its quotes and backslashes escaped, as the specification asks.
fn startup_removal(id: &str) -> Vec<u8> {
    let mut quoted = String::with_capacity(id.len() + 2);
    quoted.push('"');
    for character in id.chars() {
        if character == '"' || character == '\\' {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    let mut message = format!("remove: ID={quoted}").into_bytes();
    message.push(0);
    message
}

/// The X resources' text that the root window's `RESOURCE_MANAGER` holds, as `xrdb` loads them
/// at a session's start; empty where it holds none. The person's files are not read in its place:
/// the session's tools load them into the property.
fn read_resources(connection: &XCBConnection) -> Vec<u8> {
    let root = connection.setup().roots[0].root;
    let read = connection
        .get_property(
            false,
            root,
            AtomEnum::RESOURCE_MANAGER,
            AtomEnum::STRING,
            0,
            // The property's whole length, in 4-byte units, as large as X11 lets it be.
            u32::MAX / 4,
        )
        .map_err(x11rb::errors::ReplyError::from)
        .and_then(|cookie| cookie.reply());
    match read {
        Ok(reply) => reply.value,
        Err(error) => {
            tracing::warn!(%error, "the X resources are not read: the scale is 1");
            Vec::new()
        }
    }
}

/// The scale that `resources`' `Xft.dpi` gives, over 96 dots per inch, held within [`SCALES`]; 1
/// where it gives none, or no positive number.
fn scale_of(resources: &Database) -> f64 {
    let dpi = resources
        .get_string("Xft.dpi", "Xft.Dpi")
        .and_then(|dpi| dpi.trim().parse::<f64>().ok())
        .filter(|dpi| dpi.is_finite() && *dpi > 0.0);
    dpi.map_or(1.0, |dpi| {
        (dpi / DPI_AT_SCALE_ONE).clamp(SCALES.0, SCALES.1)
    })
}

/// The pixels of a side of `logical` pixels at `scale`, one at least.
fn pixels_of(logical: u32, scale: f64) -> u32 {
    float_side(f64::from(logical) * scale)
}

/// The logical pixels of a side of `pixels` at `scale`, one at least.
fn logical_of(pixels: u32, scale: f64) -> u32 {
    float_side(f64::from(pixels) / scale)
}

/// A side measured as `side`, rounded, within one and `u32::MAX`.
fn float_side(side: f64) -> u32 {
    // A float's cast saturates, and a NaN gives 0, which the floor of one takes.
    (side.round() as u32).max(1)
}

/// A side for a window's geometry, which X11 counts in `u16`.
fn side16(pixels: u32) -> u16 {
    u16::try_from(pixels).unwrap_or(u16::MAX)
}

/// A side for the size hints, which count in `i32`.
fn side32(pixels: u32) -> i32 {
    i32::try_from(pixels).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_startup_message_quotes_its_id() {
        assert_eq!(
            startup_removal("app-1_TIME42"),
            b"remove: ID=\"app-1_TIME42\"\0".to_vec()
        );
        assert_eq!(
            startup_removal("a \"b\" \\c"),
            b"remove: ID=\"a \\\"b\\\" \\\\c\"\0".to_vec()
        );
    }

    #[test]
    fn a_present_time_counts_only_on_this_machines_clock() {
        let received = Instant::from_nanos(10_000_000_000);
        let before = Instant::from_nanos(9_990_000_000);
        assert_eq!(present_time(Some(before), received), before);
        // After its reception, or more than a second before: not this machine's clock.
        let after = Instant::from_nanos(10_000_000_001);
        assert_eq!(present_time(Some(after), received), received);
        let long_before = Instant::from_nanos(8_999_999_999);
        assert_eq!(present_time(Some(long_before), received), received);
        let a_second_before = Instant::from_nanos(9_000_000_000);
        assert_eq!(
            present_time(Some(a_second_before), received),
            a_second_before
        );
        assert_eq!(present_time(None, received), received);
    }

    #[test]
    fn the_servers_focus_lies_on_the_window_above_it_or_elsewhere() {
        let place = |focus, ancestor: bool| focus_place(focus, 42, |_| ancestor);
        let at = |window, above| keyboard::Focus::Server { window, above };
        assert_eq!(place(Some(42), false), at(true, false));
        assert_eq!(place(Some(POINTER_ROOT), false), at(false, true));
        // The root window, or a window manager's frame: above when an ancestor.
        assert_eq!(place(Some(7), true), at(false, true));
        assert_eq!(place(Some(7), false), at(false, false));
        // None, or the focus not read.
        assert_eq!(place(Some(x11rb::NONE), true), at(false, false));
        assert_eq!(place(None, true), at(false, false));
    }

    #[test]
    fn xft_dpi_gives_the_scale_over_96() {
        let scale = |resources: &[u8]| scale_of(&Database::new_from_data(resources));
        assert_eq!(scale(b"Xft.dpi:\t144\n"), 1.5);
        assert_eq!(scale(b"Xft.dpi: 192.0\n"), 2.0);
        assert_eq!(scale(b"Xft.antialias: 1\n"), 1.0);
        assert_eq!(scale(b""), 1.0);
        // Not a positive number: 1. Out of reach: held.
        assert_eq!(scale(b"Xft.dpi: high\n"), 1.0);
        assert_eq!(scale(b"Xft.dpi: -96\n"), 1.0);
        assert_eq!(scale(b"Xft.dpi: 0\n"), 1.0);
        assert_eq!(scale(b"Xft.dpi: 9600\n"), SCALES.1);
        assert_eq!(scale(b"Xft.dpi: 1\n"), SCALES.0);
    }

    #[test]
    fn logical_and_physical_sides_follow_the_scale() {
        assert_eq!(pixels_of(641, 1.5), 962);
        assert_eq!(logical_of(962, 1.5), 641);
        assert_eq!(pixels_of(0, 2.0), 1);
        assert_eq!(logical_of(1, 8.0), 1);
        assert_eq!(pixels_of(u32::MAX, 8.0), u32::MAX);
    }

    #[test]
    fn sides_fit_x11s_counts() {
        assert_eq!(side16(70_000), u16::MAX);
        assert_eq!(side16(640), 640);
        assert_eq!(side32(u32::MAX), i32::MAX);
    }
}
