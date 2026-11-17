//! What the compositor tells the window, gathered for the host's loop: xdg-shell's configures and
//! closing, xdg-decoration's choice, the frame callbacks, the scale that the compositor prefers,
//! the tokens of activation, and the person's input with its times: the pointer's events, and the
//! keyboard's through libxkbcommon (`crate::xkb`), with its repeat (`super::repeat`).

use std::collections::HashMap;
use std::os::fd::OwnedFd;

use slime_input::{
    ButtonState, CursorShape, KeyEvent, Modifiers, MouseButton, Pointer, ScrollDelta,
};
use uwui::host::Input;
use uwui::time::{EventTime, Instant};
use wayland_client::backend::ObjectId;
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::{
    wl_callback, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::activation::v1::client::{xdg_activation_token_v1, xdg_activation_v1};
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use super::repeat::Repeat;
use crate::anchor::Anchor;
use crate::clock;
use crate::xkb::{self, Keyboard};

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

/// A person's input as the host keeps it until it gives it to the interface, which takes a key by
/// reference.
#[derive(Debug)]
pub(super) enum Kept {
    /// A key went down, up or repeated.
    Key(KeyEvent),
    /// Any other input.
    Other(Input<'static>),
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

/// What the compositor told since the loop last took it, and the state of the person's devices.
pub(super) struct Told {
    pub(super) configure: Option<Configure>,
    /// The window's size and states of the toplevel's last configure, until its surface's.
    pending: (Option<[u32; 2]>, bool),
    /// The person asked the window to close.
    pub(super) close: bool,
    /// When the last frame callback came, if one did.
    pub(super) signal: Option<Instant>,
    /// The person's input, in order, with its times.
    pub(super) inputs: Vec<(EventTime, Kept)>,
    axis: Axis,
    anchor: Anchor,
    /// Whether the compositor draws the window's bar, once it said.
    pub(super) decorated: Option<bool>,
    pub(super) pointer: Option<wl_pointer::WlPointer>,
    /// The physical pixels per logical one that the window draws at, which the pointer's positions
    /// and the wheel's pixels are multiplied by.
    pub(super) scale: f64,
    /// The scale that `wp_fractional_scale_v1` prefers, in 120ths, once it said.
    pub(super) fractional_scale: Option<u32>,
    /// The integer scale that the surface prefers (`wl_surface` 6), once it said.
    preferred_buffer_scale: Option<i32>,
    /// Each output's integer scale, and the outputs that the window lies on, where the compositor
    /// prefers no scale for the surface itself, as Weston.
    outputs: HashMap<ObjectId, i32>,
    entered: Vec<ObjectId>,
    /// The tokens of activation that the compositor gave, to be used on the window.
    pub(super) tokens: Vec<String>,
    /// The serial of the person's last press of a key or a button, or of the keyboard's focus
    /// given, which a request to bring the window to the front names, as `xdg_activation_v1`
    /// allows.
    pub(super) serial: Option<u32>,
    pub(super) cursor: Cursor,
    pub(super) keys: Keys,
}

/// The pointer's shape over the window, set through `wp_cursor_shape_v1`.
#[derive(Default)]
pub(super) struct Cursor {
    /// The compositor's manager of shapes, if it gives one.
    pub(super) manager: Option<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1>,
    device: Option<wp_cursor_shape_device_v1::WpCursorShapeDeviceV1>,
    /// The shape that the interface asks.
    shape: CursorShape,
    /// The serial of the pointer's entry, while it lies in the window.
    entered: Option<u32>,
}

impl Cursor {
    /// Shows `shape` while the pointer lies in the window.
    pub(super) fn set(&mut self, shape: CursorShape) {
        self.shape = shape;
        self.show();
    }

    fn show(&self) {
        if let (Some(device), Some(serial)) = (&self.device, self.entered) {
            device.set_shape(serial, cursor_shape(self.shape));
        }
    }
}

/// The keyboard: its keymap and state, its repeat, and the modifiers last given.
pub(super) struct Keys {
    /// None where libxkbcommon does not load: the window takes no key.
    xkb: Option<Keyboard>,
    pub(super) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(super) repeat: Repeat,
    modifiers: Modifiers,
    /// Whether the window holds the keyboard's focus.
    focused: bool,
}

impl Told {
    pub(super) fn new() -> Self {
        let xkb = Keyboard::new(&xkb::locale())
            .inspect_err(|error| tracing::warn!(%error, "the window takes no key"))
            .ok();
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
            scale: 1.0,
            fractional_scale: None,
            preferred_buffer_scale: None,
            outputs: HashMap::new(),
            entered: Vec::new(),
            tokens: Vec::new(),
            serial: None,
            cursor: Cursor::default(),
            keys: Keys {
                xkb,
                keyboard: None,
                repeat: Repeat::default(),
                modifiers: Modifiers::default(),
                focused: false,
            },
        }
    }

    /// The integer scale for the window: the one that the surface prefers, else the largest of the
    /// outputs that it lies on; none before either is known.
    pub(super) fn integer_scale(&self) -> Option<i32> {
        self.preferred_buffer_scale.or_else(|| {
            self.entered
                .iter()
                .filter_map(|output| self.outputs.get(output))
                .copied()
                .max()
        })
    }

    /// Forgets the anchor of the events' times: the system resumed.
    pub(super) fn resumed(&mut self) {
        self.anchor.reset();
    }

    /// The time of an event that the compositor stamped `millis`, received at `received`.
    fn time(&mut self, millis: u32, received: Instant) -> EventTime {
        EventTime::anchored(self.anchor.time(millis, received))
    }

    /// The repeats of the key held that are due at `now`, given as presses that repeat.
    pub(super) fn repeat(&mut self, now: Instant) {
        let keys = &mut self.keys;
        let Some(xkb) = &mut keys.xkb else {
            return;
        };
        for (code, at) in keys.repeat.due(now) {
            let event = xkb.key(code, ButtonState::Pressed, true);
            self.inputs
                .push((EventTime::received(at), Kept::Key(event)));
        }
    }

    /// A position of the surface, in logical pixels, in the window's physical pixels.
    fn physical(&self, x: f64, y: f64) -> Input<'static> {
        Input::PointerMoved {
            pointer: Pointer::MOUSE,
            x: (x * self.scale) as f32,
            y: (y * self.scale) as f32,
        }
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
                let moved = self.physical(surface_x, surface_y);
                self.push(time, moved);
            }
            wl_pointer::Event::Button {
                serial,
                time,
                button,
                state,
            } => {
                let state = match state {
                    WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                        self.serial = Some(serial);
                        ButtonState::Pressed
                    }
                    _ => ButtonState::Released,
                };
                let time = self.time(time, received);
                self.push(
                    time,
                    Input::PointerButton {
                        pointer: Pointer::MOUSE,
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
                // uwui's wheel scrolls up for a positive motion, Wayland's down: signs turned; its
                // pixels are physical.
                let delta = if axis.notches != [0, 0] {
                    ScrollDelta::Lines {
                        x: -axis.notches[0] as f32,
                        y: -axis.notches[1] as f32,
                    }
                } else {
                    ScrollDelta::Pixels {
                        x: -axis.pixels[0] * self.scale,
                        y: -axis.pixels[1] * self.scale,
                    }
                };
                let time = self.time(millis, received);
                self.push(time, Input::Wheel(delta));
            }
            _ => {}
        }
    }

    /// What the keyboard did, received at `received`.
    fn keyboard(&mut self, event: wl_keyboard::Event, received: Instant) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                self.keys.repeat.stop();
                if format != WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                    tracing::warn!(
                        ?format,
                        "a keymap of a format unknown: the window takes no key"
                    );
                    return;
                }
                let Some(xkb) = &mut self.keys.xkb else {
                    return;
                };
                let set = read_keymap(&fd, size)
                    .map_err(|error| error.to_string())
                    .and_then(|text| xkb.set_keymap(&text).map_err(|error| error.to_string()));
                if let Err(error) = set {
                    tracing::warn!(%error, "the compositor's keymap is refused: keys keep the last");
                }
            }
            wl_keyboard::Event::Enter { serial, .. } => {
                self.serial = Some(serial);
                self.focus(true, received);
            }
            wl_keyboard::Event::Leave { .. } => self.focus(false, received),
            wl_keyboard::Event::Key {
                serial,
                time,
                key,
                state,
            } => {
                let Some(xkb) = &mut self.keys.xkb else {
                    return;
                };
                let state = match state {
                    WEnum::Value(wl_keyboard::KeyState::Pressed) => ButtonState::Pressed,
                    _ => ButtonState::Released,
                };
                let event = xkb.key(key, state, false);
                let repeats = xkb.repeats(key);
                let time = self.time(time, received);
                if state == ButtonState::Pressed {
                    self.serial = Some(serial);
                    self.keys.repeat.pressed(key, time.at, repeats);
                } else {
                    self.keys.repeat.released(key);
                }
                self.inputs.push((time, Kept::Key(event)));
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                let Some(xkb) = &mut self.keys.xkb else {
                    return;
                };
                let held = xkb.set_modifiers(mods_depressed, mods_latched, mods_locked, group);
                if held != self.keys.modifiers {
                    self.keys.modifiers = held;
                    self.push(EventTime::received(received), Input::Modifiers(held));
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                let rate = u32::try_from(rate).unwrap_or(0);
                let delay = u32::try_from(delay).unwrap_or(0);
                self.keys.repeat.set_info(rate, delay);
            }
            _ => {}
        }
    }

    /// The window took the keyboard's focus, or lost it, at `received`: told before what follows,
    /// as uwui asks; a key held repeats no more.
    fn focus(&mut self, given: bool, received: Instant) {
        self.keys.focused = given;
        if !given {
            self.keys.repeat.stop();
            if let Some(xkb) = &mut self.keys.xkb {
                xkb.forget_sequence();
            }
        }
        self.push(EventTime::received(received), Input::Focus(given));
    }

    /// The seat lost its keyboard, at `received`: a focus that it held is lost with it, though the
    /// compositor need not say so.
    fn keyboard_gone(&mut self, received: Instant) {
        if self.keys.focused {
            self.focus(false, received);
        }
        self.keys.repeat.stop();
        if let Some(keyboard) = self.keys.keyboard.take() {
            keyboard.release();
        }
    }

    fn push(&mut self, time: EventTime, input: Input<'static>) {
        self.inputs.push((time, Kept::Other(input)));
    }
}

/// The keymap's text that the compositor gives in `fd`, `size` bytes, read from its start without
/// moving the file's offset, which the compositor may share with other clients.
fn read_keymap(fd: &OwnedFd, size: u32) -> std::io::Result<Vec<u8>> {
    let size = usize::try_from(size).unwrap_or(usize::MAX);
    // A keymap past a few megabytes is no keymap.
    if size > 16 << 20 {
        return Err(std::io::Error::other("a keymap of more than 16 MiB"));
    }
    let mut text = vec![0; size];
    let mut read = 0;
    while read < size {
        let offset = u64::try_from(read).unwrap_or(u64::MAX);
        match rustix::io::pread(fd, &mut text[read..], offset) {
            Ok(0) => break,
            Ok(count) => read += count,
            Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
    }
    text.truncate(read);
    Ok(text)
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
delegate_noop!(Told: ignore zxdg_decoration_manager_v1::ZxdgDecorationManagerV1);
delegate_noop!(Told: ignore wp_viewporter::WpViewporter);
delegate_noop!(Told: ignore wp_viewport::WpViewport);
delegate_noop!(Told: ignore wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);
delegate_noop!(Told: ignore wp_cursor_shape_manager_v1::WpCursorShapeManagerV1);
delegate_noop!(Told: ignore wp_cursor_shape_device_v1::WpCursorShapeDeviceV1);
delegate_noop!(Told: ignore xdg_activation_v1::XdgActivationV1);

impl Dispatch<wl_surface::WlSurface, ()> for Told {
    fn event(
        told: &mut Self,
        _: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_surface::Event::PreferredBufferScale { factor } => {
                told.preferred_buffer_scale = Some(factor);
            }
            wl_surface::Event::Enter { output } => told.entered.push(output.id()),
            wl_surface::Event::Leave { output } => told.entered.retain(|id| *id != output.id()),
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Told {
    fn event(
        told: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Scale { factor } = event {
            told.outputs.insert(output.id(), factor);
        }
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for Told {
    fn event(
        told: &mut Self,
        _: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            told.fractional_scale = Some(scale);
        }
    }
}

impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, ()> for Told {
    fn event(
        told: &mut Self,
        token: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: xdg_activation_token_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token: given } = event {
            told.tokens.push(given);
            token.destroy();
        }
    }
}

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
        let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        else {
            return;
        };
        let has = capabilities.contains(wl_seat::Capability::Pointer);
        match (&told.pointer, has) {
            (None, true) => {
                let pointer = seat.get_pointer(queue, ());
                told.cursor.device = told
                    .cursor
                    .manager
                    .as_ref()
                    .map(|manager| manager.get_pointer(&pointer, queue, ()));
                told.pointer = Some(pointer);
            }
            (Some(_), false) => {
                if let Some(device) = told.cursor.device.take() {
                    device.destroy();
                }
                told.cursor.entered = None;
                if let Some(pointer) = told.pointer.take() {
                    pointer.release();
                }
            }
            _ => {}
        }
        let has = capabilities.contains(wl_seat::Capability::Keyboard);
        match (&told.keys.keyboard, has) {
            (None, true) => told.keys.keyboard = Some(seat.get_keyboard(queue, ())),
            (Some(_), false) => told.keyboard_gone(clock::now()),
            _ => {}
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
            // Entering and leaving give no time: their reception is theirs. The cursor's shape is
            // set at each entry, which the compositor asks.
            wl_pointer::Event::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => {
                told.cursor.entered = Some(serial);
                told.cursor.show();
                let moved = told.physical(surface_x, surface_y);
                told.push(EventTime::received(clock::now()), moved);
            }
            wl_pointer::Event::Leave { .. } => {
                told.cursor.entered = None;
                told.push(
                    EventTime::received(clock::now()),
                    Input::PointerLeft {
                        pointer: Pointer::MOUSE,
                    },
                );
            }
            event => told.pointer(event, clock::now()),
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Told {
    fn event(
        told: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        told.keyboard(event, clock::now());
    }
}

/// The shape of `wp_cursor_shape_v1` that shows `shape`.
fn cursor_shape(shape: CursorShape) -> wp_cursor_shape_device_v1::Shape {
    use wp_cursor_shape_device_v1::Shape;
    match shape {
        CursorShape::Text => Shape::Text,
        CursorShape::ResizeHorizontal => Shape::EwResize,
        CursorShape::ResizeVertical => Shape::NsResize,
        CursorShape::Default => Shape::Default,
        // A shape added to `slime-input` that the host does not map yet shows the default.
        _ => Shape::Default,
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
                let input = match input {
                    Kept::Other(input) => format!("{input:?}"),
                    Kept::Key(key) => format!(
                        "Key {:?} {:?} {:?} {:?}{}",
                        key.physical,
                        key.logical,
                        key.text,
                        key.state,
                        if key.repeat { " repeat" } else { "" }
                    ),
                };
                format!(
                    "{} {:?} {input}",
                    time.at.as_nanos() / 1_000_000,
                    time.source
                )
            })
            .collect()
    }

    /// The keymap of the US layout, as `scripts/compositor-input.py` gives it, in a file of the
    /// test's own.
    fn keymap() -> (OwnedFd, u32) {
        let text = b"xkb_keymap {
            xkb_keycodes { include \"evdev+aliases(qwerty)\" };
            xkb_types { include \"complete\" };
            xkb_compat { include \"complete\" };
            xkb_symbols { include \"pc+us+inet(evdev)\" };
        };\0";
        // A file of each call's own: the tests run in parallel, each removing its file once open.
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("uwui-desktop-keymap-{}-{call}", std::process::id()));
        std::fs::write(&path, text).expect("the keymap is written");
        let file = std::fs::File::open(&path).expect("the keymap opens");
        let _ = std::fs::remove_file(&path);
        (
            OwnedFd::from(file),
            u32::try_from(text.len()).expect("a small keymap"),
        )
    }

    fn key(time: u32, key: u32, pressed: bool) -> wl_keyboard::Event {
        wl_keyboard::Event::Key {
            serial: time,
            time,
            key,
            state: WEnum::Value(if pressed {
                wl_keyboard::KeyState::Pressed
            } else {
                wl_keyboard::KeyState::Released
            }),
        }
    }

    #[test]
    fn the_keyboard_gives_its_focus_modifiers_and_keys_in_order() {
        let mut told = Told::new();
        let (fd, size) = keymap();
        told.keyboard(
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            },
            at(5_000),
        );
        told.focus(true, at(5_000));
        // Shift held, then `a`: its character in capitals.
        let modifiers = |depressed| wl_keyboard::Event::Modifiers {
            serial: 2,
            mods_depressed: depressed,
            mods_latched: 0,
            mods_locked: 0,
            group: 0,
        };
        told.keyboard(modifiers(1), at(5_001));
        // The same modifiers told twice give one event.
        told.keyboard(modifiers(1), at(5_001));
        told.keyboard(key(1_002, 30, true), at(5_003));
        told.keyboard(key(1_010, 30, false), at(5_010));
        told.keyboard(modifiers(0), at(5_011));
        assert_eq!(told.serial, Some(1_002));
        let given = inputs(&mut told);
        assert_eq!(given.len(), 5, "{given:#?}");
        assert_eq!(given[0], "5000 Received Focus(true)");
        assert!(given[1].starts_with("5001 Received Modifiers(Modifiers { shift: true"));
        assert_eq!(
            given[2],
            "5003 Anchored Key KeyA Character(\"A\") Some(\"A\") Pressed"
        );
        assert_eq!(
            given[3],
            "5010 Anchored Key KeyA Character(\"A\") None Released"
        );
        assert!(given[4].starts_with("5011 Received Modifiers(Modifiers { shift: false"));
        // The focus leaves: told, and the key held no longer repeats.
        told.keyboard(key(1_020, 30, true), at(5_020));
        assert!(told.keys.repeat.deadline().is_some());
        told.focus(false, at(5_030));
        assert!(told.keys.repeat.deadline().is_none());
        assert_eq!(
            inputs(&mut told).last().map(String::as_str),
            Some("5030 Received Focus(false)")
        );
    }

    #[test]
    fn a_new_keymap_or_a_keyboard_gone_ends_the_repeat_and_the_focus() {
        let mut told = Told::new();
        let (fd, size) = keymap();
        told.keyboard(
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            },
            at(5_000),
        );
        told.focus(true, at(5_000));
        told.keyboard(key(1_000, 30, true), at(5_000));
        assert!(told.keys.repeat.deadline().is_some());
        // A new keymap: the key held no longer means what it did.
        let (fd, size) = keymap();
        told.keyboard(
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            },
            at(5_100),
        );
        assert!(told.keys.repeat.deadline().is_none());
        // The seat loses its keyboard while the window holds its focus: the focus goes with it.
        told.keyboard(key(1_200, 30, true), at(5_200));
        told.inputs.clear();
        told.keyboard_gone(at(5_300));
        assert!(told.keys.repeat.deadline().is_none());
        assert_eq!(inputs(&mut told), ["5300 Received Focus(false)"]);
        // Gone again without the focus: nothing to tell.
        told.keyboard_gone(at(5_400));
        assert!(inputs(&mut told).is_empty());
    }

    #[test]
    fn a_dead_key_before_the_focus_leaves_composes_with_nothing_after() {
        let mut told = Told::new();
        let mut french = Keyboard::new("en_US.UTF-8").expect("libxkbcommon loads");
        french.set_test_layout("fr").expect("the layout compiles");
        told.keys.xkb = Some(french);
        told.focus(true, at(5_000));
        // `^`, dead on a French keyboard, then the focus lost and given back, then `e`.
        told.keyboard(key(1_000, 26, true), at(5_000));
        told.focus(false, at(5_010));
        told.focus(true, at(5_020));
        told.keyboard(key(1_030, 18, true), at(5_030));
        let given = inputs(&mut told);
        assert_eq!(
            given.last().map(String::as_str),
            Some("5030 Anchored Key KeyE Character(\"e\") Some(\"e\") Pressed")
        );
    }

    #[test]
    fn a_key_held_repeats_at_the_compositors_pace() {
        let mut told = Told::new();
        let (fd, size) = keymap();
        let keymap = wl_keyboard::Event::Keymap {
            format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
            fd,
            size,
        };
        told.keyboard(keymap, at(5_000));
        told.keyboard(
            wl_keyboard::Event::RepeatInfo {
                rate: 50,
                delay: 200,
            },
            at(5_000),
        );
        told.keyboard(key(1_000, 30, true), at(5_000));
        told.inputs.clear();
        // The key pressed at 5 s on the host's clock: repeats at 5.2 s, then every 20 ms.
        assert_eq!(told.keys.repeat.deadline(), Some(at(5_200)));
        told.repeat(at(5_220));
        assert_eq!(
            inputs(&mut told),
            [
                "5200 Received Key KeyA Character(\"a\") Some(\"a\") Pressed repeat",
                "5220 Received Key KeyA Character(\"a\") Some(\"a\") Pressed repeat",
            ]
        );
        // Shift does not repeat, and leaves `a` repeating.
        told.keyboard(key(1_230, 42, true), at(5_230));
        assert_eq!(told.keys.repeat.deadline(), Some(at(5_240)));
        told.keyboard(key(1_235, 30, false), at(5_235));
        assert_eq!(told.keys.repeat.deadline(), None);
    }

    #[test]
    fn the_pointers_positions_and_pixels_follow_the_scale() {
        let mut told = Told::new();
        told.scale = 1.5;
        told.pointer(
            wl_pointer::Event::Motion {
                time: 1_000,
                surface_x: 10.0,
                surface_y: 20.0,
            },
            at(6_000),
        );
        told.pointer(
            wl_pointer::Event::Axis {
                time: 1_001,
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value: 4.0,
            },
            at(6_001),
        );
        told.pointer(wl_pointer::Event::Frame, at(6_001));
        assert_eq!(
            inputs(&mut told),
            [
                "6000 Anchored PointerMoved { pointer: Pointer { id: PointerId(0), device: Mouse }, x: 15.0, y: 30.0 }",
                "6001 Anchored Wheel(Pixels { x: -0.0, y: -6.0 })",
            ]
        );
    }

    #[test]
    fn a_keymap_read_leaves_its_files_offset() {
        let (fd, size) = keymap();
        let first = read_keymap(&fd, size).expect("it reads");
        let second = read_keymap(&fd, size).expect("it reads again");
        assert_eq!(first, second);
        assert!(first.starts_with(b"xkb_keymap {"));
        assert!(read_keymap(&fd, u32::MAX).is_err());
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
                "6002 Anchored PointerMoved { pointer: Pointer { id: PointerId(0), device: Mouse }, x: 10.5, y: 20.0 }",
                "6009 Anchored PointerMoved { pointer: Pointer { id: PointerId(0), device: Mouse }, x: 11.0, y: 21.0 }",
                "6011 Anchored PointerButton { pointer: Pointer { id: PointerId(0), device: Mouse }, button: Left, state: Pressed }",
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
