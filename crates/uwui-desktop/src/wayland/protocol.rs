//! What the compositor tells the window, gathered for the host's loop: xdg-shell's configures and
//! closing, xdg-decoration's choice, the frame callbacks, and the pointer's events with their
//! times.

use slime_input::{ButtonState, MouseButton, ScrollDelta};
use uwui::host::Input;
use uwui::time::{EventTime, Instant};
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::{
    wl_callback, wl_compositor, wl_pointer, wl_registry, wl_seat, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::anchor::Anchor;
use crate::clock;

/// Linux's codes of the mouse's buttons (`linux/input-event-codes.h`), which Wayland gives.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
const BTN_SIDE: u32 = 0x113;
const BTN_EXTRA: u32 = 0x114;

/// A configure of the window that the compositor made, which the host acknowledges as it draws
/// the next frame.
#[derive(Clone, Copy, Debug)]
pub(super) struct Configure {
    pub(super) serial: u32,
    /// The size that the compositor asks, in logical pixels; none where it leaves it to the
    /// application.
    pub(super) size: Option<[u32; 2]>,
    pub(super) fullscreen: bool,
}

/// The wheel's motion within one of the pointer's frames.
#[derive(Clone, Copy, Debug, Default)]
struct Axis {
    time: Option<u32>,
    /// Surface-local pixels, across then down, as Wayland signs them: down and right positive.
    pixels: [f64; 2],
    /// The wheel's notches, signed as the pixels.
    notches: [i32; 2],
}

/// What the compositor told since the loop last took it.
pub(super) struct Told {
    pub(super) configure: Option<Configure>,
    /// The window's size and states of the toplevel's last configure, until its surface's.
    pending: (Option<[u32; 2]>, bool),
    /// The person asked the window to close.
    pub(super) close: bool,
    /// When the last frame callback came, if one did.
    pub(super) signal: Option<Instant>,
    /// The person's input, in order, with its times.
    pub(super) inputs: Vec<(EventTime, Input<'static>)>,
    axis: Axis,
    anchor: Anchor,
    /// Whether the compositor draws the window's bar, once it said.
    pub(super) decorated: Option<bool>,
    pub(super) pointer: Option<wl_pointer::WlPointer>,
}

impl Told {
    pub(super) fn new() -> Self {
        Self {
            configure: None,
            pending: (None, false),
            close: false,
            signal: None,
            inputs: Vec::new(),
            axis: Axis::default(),
            anchor: Anchor::default(),
            decorated: None,
            pointer: None,
        }
    }

    /// Forgets the anchor of the events' times: the system resumed.
    pub(super) fn resumed(&mut self) {
        self.anchor.reset();
    }

    /// The time of an event that the compositor stamped `millis`, received at `received`.
    fn time(&mut self, millis: u32, received: Instant) -> EventTime {
        EventTime::anchored(self.anchor.time(millis, received))
    }

    /// What the pointer did, but entering and leaving, which name a surface: its moves, its buttons
    /// and its wheel, gathered by the pointer's frames, received at `received`.
    fn pointer(&mut self, event: wl_pointer::Event, received: Instant) {
        match event {
            wl_pointer::Event::Motion {
                time,
                surface_x,
                surface_y,
            } => {
                let time = self.time(time, received);
                self.push(
                    time,
                    Input::PointerMoved {
                        x: surface_x as f32,
                        y: surface_y as f32,
                    },
                );
            }
            wl_pointer::Event::Button {
                time,
                button,
                state,
                ..
            } => {
                let state = match state {
                    WEnum::Value(wl_pointer::ButtonState::Pressed) => ButtonState::Pressed,
                    _ => ButtonState::Released,
                };
                let time = self.time(time, received);
                self.push(
                    time,
                    Input::PointerButton {
                        button: mouse_button(button),
                        state,
                    },
                );
            }
            wl_pointer::Event::Axis { time, axis, value } => {
                self.axis.time = Some(time);
                if let Some(at) = axis_index(axis) {
                    self.axis.pixels[at] += value;
                }
            }
            wl_pointer::Event::AxisDiscrete { axis, discrete } => {
                if let Some(at) = axis_index(axis) {
                    self.axis.notches[at] += discrete;
                }
            }
            wl_pointer::Event::Frame => {
                let axis = std::mem::take(&mut self.axis);
                let Some(millis) = axis.time else {
                    return;
                };
                // uwui's wheel scrolls up for a positive motion, Wayland's down: signs turned.
                let delta = if axis.notches != [0, 0] {
                    ScrollDelta::Lines {
                        x: -axis.notches[0] as f32,
                        y: -axis.notches[1] as f32,
                    }
                } else {
                    ScrollDelta::Pixels {
                        x: -axis.pixels[0],
                        y: -axis.pixels[1],
                    }
                };
                let time = self.time(millis, received);
                self.push(time, Input::Wheel(delta));
            }
            _ => {}
        }
    }

    fn push(&mut self, time: EventTime, input: Input<'static>) {
        self.inputs.push((time, input));
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Told {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Globals that come or go during the run are not followed: a seat added later gives no
        // pointer.
    }
}

delegate_noop!(Told: ignore wl_compositor::WlCompositor);
delegate_noop!(Told: ignore wl_surface::WlSurface);
delegate_noop!(Told: ignore zxdg_decoration_manager_v1::ZxdgDecorationManagerV1);

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Told {
    fn event(
        _: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for Told {
    fn event(
        told: &mut Self,
        _: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            let (size, fullscreen) = told.pending;
            told.configure = Some(Configure {
                serial,
                size,
                fullscreen,
            });
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for Told {
    fn event(
        told: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                // Zero, or a side that is not a size, leaves the size to the application.
                let size = match (u32::try_from(width), u32::try_from(height)) {
                    (Ok(width), Ok(height)) if width > 0 && height > 0 => Some([width, height]),
                    _ => None,
                };
                let fullscreen = states.as_chunks::<4>().0.iter().any(|&state| {
                    u32::from_ne_bytes(state) == xdg_toplevel::State::Fullscreen as u32
                });
                told.pending = (size, fullscreen);
            }
            xdg_toplevel::Event::Close => told.close = true,
            _ => {}
        }
    }
}

impl Dispatch<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1, ()> for Told {
    fn event(
        told: &mut Self,
        _: &zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event {
            let server = mode == WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            if told.decorated != Some(server) {
                if server {
                    tracing::info!("the compositor draws the window's bar");
                } else {
                    tracing::info!("the window has no bar: the compositor leaves it to the window");
                }
            }
            told.decorated = Some(server);
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for Told {
    fn event(
        told: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The callback's own time has an undefined base: its reception anchors the grid.
        if let wl_callback::Event::Done { .. } = event {
            told.signal = Some(clock::now());
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Told {
    fn event(
        told: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            let has = capabilities.contains(wl_seat::Capability::Pointer);
            match (&told.pointer, has) {
                (None, true) => told.pointer = Some(seat.get_pointer(queue, ())),
                (Some(_), false) => {
                    if let Some(pointer) = told.pointer.take() {
                        pointer.release();
                    }
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for Told {
    fn event(
        told: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            // Entering and leaving give no time: their reception is theirs.
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                ..
            } => told.push(
                EventTime::received(clock::now()),
                Input::PointerMoved {
                    x: surface_x as f32,
                    y: surface_y as f32,
                },
            ),
            wl_pointer::Event::Leave { .. } => {
                told.push(EventTime::received(clock::now()), Input::PointerLeft);
            }
            event => told.pointer(event, clock::now()),
        }
    }
}

/// The index of `axis` in an [`Axis`]'s pairs, across then down.
fn axis_index(axis: WEnum<wl_pointer::Axis>) -> Option<usize> {
    match axis {
        WEnum::Value(wl_pointer::Axis::HorizontalScroll) => Some(0),
        WEnum::Value(wl_pointer::Axis::VerticalScroll) => Some(1),
        _ => None,
    }
}

/// The button of Linux's code `code`.
fn mouse_button(code: u32) -> MouseButton {
    match code {
        BTN_LEFT => MouseButton::Left,
        BTN_RIGHT => MouseButton::Right,
        BTN_MIDDLE => MouseButton::Middle,
        BTN_SIDE => MouseButton::Back,
        BTN_EXTRA => MouseButton::Forward,
        other => MouseButton::Other(u16::try_from(other).unwrap_or(u16::MAX)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_codes_name_the_mouse_buttons() {
        assert_eq!(mouse_button(0x110), MouseButton::Left);
        assert_eq!(mouse_button(0x111), MouseButton::Right);
        assert_eq!(mouse_button(0x112), MouseButton::Middle);
        assert_eq!(mouse_button(0x113), MouseButton::Back);
        assert_eq!(mouse_button(0x114), MouseButton::Forward);
        assert_eq!(mouse_button(0x115), MouseButton::Other(0x115));
    }

    fn at(millis: u64) -> Instant {
        Instant::from_nanos(millis * 1_000_000)
    }

    fn inputs(told: &mut Told) -> Vec<String> {
        told.inputs
            .drain(..)
            .map(|(time, input)| {
                format!(
                    "{} {:?} {input:?}",
                    time.at.as_nanos() / 1_000_000,
                    time.source
                )
            })
            .collect()
    }

    #[test]
    fn the_pointers_moves_and_buttons_carry_their_anchored_times() {
        let mut told = Told::new();
        // The compositor's clock 5 s behind the host's; the first event came 2 ms late.
        let motion = |time, x, y| wl_pointer::Event::Motion {
            time,
            surface_x: x,
            surface_y: y,
        };
        told.pointer(motion(1_000, 10.5, 20.0), at(6_002));
        told.pointer(motion(1_008, 11.0, 21.0), at(6_009));
        told.pointer(
            wl_pointer::Event::Button {
                serial: 1,
                time: 1_010,
                button: 0x110,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
            },
            at(6_013),
        );
        assert_eq!(
            inputs(&mut told),
            [
                "6002 Anchored PointerMoved { x: 10.5, y: 20.0 }",
                "6009 Anchored PointerMoved { x: 11.0, y: 21.0 }",
                "6011 Anchored PointerButton { button: Left, state: Pressed }",
            ]
        );
    }

    #[test]
    fn the_wheel_is_gathered_by_the_pointers_frame_its_signs_turned() {
        let mut told = Told::new();
        let vertical = || WEnum::Value(wl_pointer::Axis::VerticalScroll);
        // A notch down: lines, scrolling down, as uwui signs it.
        told.pointer(
            wl_pointer::Event::Axis {
                time: 100,
                axis: vertical(),
                value: 15.0,
            },
            at(5_100),
        );
        told.pointer(
            wl_pointer::Event::AxisDiscrete {
                axis: vertical(),
                discrete: 1,
            },
            at(5_100),
        );
        assert!(
            told.inputs.is_empty(),
            "the wheel waits for the pointer's frame"
        );
        told.pointer(wl_pointer::Event::Frame, at(5_100));
        // A touchpad's motion across, without notches: pixels.
        told.pointer(
            wl_pointer::Event::Axis {
                time: 120,
                axis: WEnum::Value(wl_pointer::Axis::HorizontalScroll),
                value: -3.5,
            },
            at(5_120),
        );
        told.pointer(wl_pointer::Event::Frame, at(5_120));
        // A frame without the wheel gives nothing.
        told.pointer(wl_pointer::Event::Frame, at(5_130));
        assert_eq!(
            inputs(&mut told),
            [
                "5100 Anchored Wheel(Lines { x: 0.0, y: -1.0 })",
                "5120 Anchored Wheel(Pixels { x: 3.5, y: -0.0 })",
            ]
        );
    }
}
