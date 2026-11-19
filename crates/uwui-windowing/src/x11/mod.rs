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
//!   its process and machine named. Its size is the window manager's, its scale 1; without a
//!   window manager, as under Xvfb, it opens at the size asked and its requests do nothing.
//! - *Activation*: `_NET_ACTIVE_WINDOW`, named by the person's last press, and
//!   `_NET_WM_STATE_DEMANDS_ATTENTION`; the start's `DESKTOP_STARTUP_ID` told ended, as the
//!   startup notification's specification asks.
//! - *Its frames' signal*: Present's notices, or the layer's own timer where the server gives no
//!   Present; when each image showed, where the driver presents through Present; the display's
//!   period from RandR (`frames`). A window unmapped, or covered whole where no compositor keeps
//!   it, holds its signal until it shows again; an exposed one asks to be drawn again.
//! - *The pointer*: XInput2's moves, buttons and wheel (`pointer`), each with the server's time
//!   anchored on the host's clock (`crate::anchor`); entering and leaving with their reception's.
//!
//! The keyboard, the cursor's shapes, the scale and touch come at M6d.7b2: until then the window
//! takes no key, and shows the server's cursor.

mod frames;
mod pointer;

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
use x11rb::properties::WmSizeHints;
use x11rb::protocol::Event;
use x11rb::protocol::present::{self, ConnectionExt as _};
use x11rb::protocol::randr::{self, ConnectionExt as _};
use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    self, AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
    Visibility, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

use crate::anchor::Anchor;
use crate::clock::{self, Clock, Suspends};
use crate::wait::Waits;
use crate::wake::{self, Asks};
use crate::{Activation, Configured, Drawn, Error, Handler, Happened, Input, Opening, running};

use frames::{Frames, Shown};
use pointer::Pointers;

/// The shortest time suspended after which the events' anchor is made again: what parts the two
/// clocks while the machine is awake is far below it.
const RESUMED: Duration = Duration::from_secs(1);

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
    }
}

/// `_NET_WM_STATE`'s actions, from the extended window manager hints.
const STATE_REMOVE: u32 = 0;
const STATE_ADD: u32 = 1;
/// A request's source in the extended window manager hints: an application.
const FROM_APPLICATION: u32 = 1;
/// XInput2's device id for every master device.
const ALL_MASTER_DEVICES: u16 = 1;

/// Runs the loop of a window opened as `opening` says, calling `handler` back, until it ends.
pub(crate) fn run<H: Handler>(opening: Opening, handler: H) -> (H, Result<(), Error>) {
    running::run(Platform::open(opening), handler)
}

/// What came since the last turn, gathered from the events as they are read.
#[derive(Debug, Default)]
struct Gathered {
    inputs: Vec<(EventTime, Input)>,
    configured: Option<Configured>,
    drawn: Option<Drawn>,
    close: bool,
    exposed: bool,
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
    frames: Frames,
    gathered: Gathered,
    /// Whether RandR tells the displays' modes and their changes.
    randr: bool,
    /// The display's period must be read again: the window moved, or the displays changed.
    period_stale: bool,
    /// The window's size in pixels, and whether it fills the screen.
    size: [u32; 2],
    fullscreen: bool,
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
    fn open(opening: Opening) -> Result<(Self, Drawn), Error> {
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
        let size = opening.size.map(|side| side.clamp(1, u32::from(u16::MAX)));
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
                            | EventMask::EXPOSURE,
                    ),
            )
            .map_err(|cause| Error::new("the window", cause))?;
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
            frames: Frames::new(false),
            gathered: Gathered::default(),
            randr: false,
            period_stale: true,
            size,
            fullscreen: false,
            mapped: false,
            covered: false,
            last_press: 0,
            next_image: 0,
            present_clock: Clock::from_raw(1),
        };
        platform.describe(&opening)?;
        platform.select_input()?;
        platform
            .connection
            .map_window(window)
            .map_err(|cause| Error::new("the window", cause))?;
        platform.started();
        platform
            .connection
            .flush()
            .map_err(|cause| Error::new("the connection to the X server", cause))?;
        platform.frames.set_shown(false);
        // The first turn tells the window's first size, as a Wayland compositor's first configure.
        platform.gathered.configured = Some(Configured {
            size,
            fullscreen: false,
        });
        wake::on_termination(&platform.asks);
        let drawn = Drawn {
            pixels: size,
            scale: 1.0,
        };
        Ok((platform, drawn))
    }

    /// Gives the window its properties: its title, class, process, protocols, minimum size, and
    /// whether it opens filling the screen.
    fn describe(&mut self, opening: &Opening) -> Result<(), Error> {
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
        self.set_size_hints(opening.min_size).map_err(failed)?;
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
        // XInput 2.1 gives the scroll valuators.
        let xinput = connection
            .extension_information(xinput::X11_EXTENSION_NAME)
            .map_err(|cause| Error::new("the X server's extensions", cause))?
            .is_some()
            && connection
                .xinput_xi_query_version(2, 1)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .is_some_and(|version| (version.major_version, version.minor_version) >= (2, 1));
        if !xinput {
            return Err(Error(
                "the X server gives no XInput 2.1, which the window's pointer needs".to_owned(),
            ));
        }
        connection
            .xinput_xi_select_events(
                self.window,
                &[xinput::EventMask {
                    deviceid: ALL_MASTER_DEVICES,
                    mask: vec![
                        xinput::XIEventMask::MOTION
                            | xinput::XIEventMask::BUTTON_PRESS
                            | xinput::XIEventMask::BUTTON_RELEASE
                            | xinput::XIEventMask::ENTER
                            | xinput::XIEventMask::LEAVE
                            | xinput::XIEventMask::DEVICE_CHANGED,
                    ],
                }],
            )
            .map_err(|cause| Error::new("the pointer's events", cause))?;
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
    /// that its window opened, if the environment names the start. The variable stays set, as on
    /// Wayland.
    fn started(&mut self) {
        let id = std::env::var("DESKTOP_STARTUP_ID").unwrap_or_default();
        if id.is_empty() {
            return;
        }
        let _ = self.connection.change_property8(
            PropMode::REPLACE,
            self.window,
            self.atoms._NET_STARTUP_ID,
            self.atoms.UTF8_STRING,
            id.as_bytes(),
        );
        let message = startup_removal(&id);
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

    fn set_size_hints(&self, min: Option<[u32; 2]>) -> Result<(), x11rb::errors::ConnectionError> {
        let mut hints = WmSizeHints::new();
        hints.min_size = min.map(|[width, height]| (side32(width), side32(height)));
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

    /// Reads every event that the server sent, without waiting: whether any came.
    fn read_events(&mut self) -> Result<bool, Error> {
        let mut any = false;
        while let Some(event) = self
            .connection
            .poll_for_event()
            .map_err(|cause| Error::new("the connection to the X server", cause))?
        {
            any = true;
            self.event(event, clock::now());
        }
        Ok(any)
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
            }
            Event::XinputLeave(_) => {
                for input in self.pointers.input(pointer::Event::Left, false) {
                    self.gathered
                        .inputs
                        .push((EventTime::received(received), input));
                }
            }
            Event::XinputDeviceChanged(changed) => {
                self.pointers.classes(changed.deviceid, &changed.classes);
            }
            Event::PresentCompleteNotify(complete) if complete.window == self.window => {
                let at = self.present_time(complete.ust, received);
                if complete.kind == present::CompleteKind::NOTIFY_MSC {
                    self.frames.signalled(at);
                } else if complete.mode == present::CompleteMode::SKIP {
                    self.frames.presented(Shown::Skipped);
                } else {
                    self.frames.presented(Shown::At(at));
                }
            }
            Event::ConfigureNotify(configure) if configure.window == self.window => {
                let size = [u32::from(configure.width), u32::from(configure.height)]
                    .map(|side| side.max(1));
                if size != self.size {
                    self.size = size;
                    self.configured();
                    self.gathered.drawn = Some(Drawn {
                        pixels: size,
                        scale: 1.0,
                    });
                }
                self.period_stale = true;
            }
            Event::PropertyNotify(property)
                if property.window == self.window && property.atom == self.atoms._NET_WM_STATE =>
            {
                let fullscreen = self.read_fullscreen();
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

    /// A time of Present, in microseconds of Linux's monotonic clock, received at `received`, on
    /// the host's clock: the reception's where it lies off this machine's clock.
    fn present_time(&self, micros: u64, received: Instant) -> Instant {
        let Some(clock) = self.present_clock else {
            return received;
        };
        let at = clock.on_host(micros.saturating_mul(1000));
        if at <= received && received - at <= PRESENT_TIME_RANGE {
            at
        } else {
            received
        }
    }

    /// The window's size or state changed: the next image shows it.
    fn configured(&mut self) {
        self.gathered.configured = Some(Configured {
            size: self.size,
            fullscreen: self.fullscreen,
        });
    }

    fn shown_changed(&mut self) {
        self.frames.set_shown(self.mapped && !self.covered);
    }

    /// Whether the window manager says that the window fills the screen.
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
        // Before the events are timed: their anchor is made again after a suspend.
        if self.suspends.take() >= RESUMED {
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
        })
    }

    fn wait(&mut self, deadline: Option<Instant>) -> Result<(), Error> {
        let deadline = match (deadline, self.frames.deadline()) {
            (Some(deadline), Some(signal)) => Some(deadline.min(signal)),
            (deadline, signal) => deadline.or(signal),
        };
        self.connection
            .flush()
            .map_err(|cause| Error::new("the connection to the X server", cause))?;
        // Events that a reply's wait read are in libxcb's queue, not on the socket: read first.
        if self.read_events()? {
            return Ok(());
        }
        let readable = self
            .waits
            .wait(self.connection.as_fd(), deadline, false)
            .map_err(|cause| Error::new("the loop's wait", cause))?;
        if readable {
            self.read_events()?;
        }
        Ok(())
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
        let _ = self.set_size_hints(size);
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

    fn set_cursor(&mut self, _: CursorShape) {
        // Told once: the next requests do nothing as well.
        static TOLD: std::sync::Once = std::sync::Once::new();
        TOLD.call_once(|| {
            tracing::info!("the cursor's shapes come to X11 at M6d.7b2: the server's shows");
        });
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
    fn sides_fit_x11s_counts() {
        assert_eq!(side16(70_000), u16::MAX);
        assert_eq!(side16(640), 640);
        assert_eq!(side32(u32::MAX), i32::MAX);
    }
}
