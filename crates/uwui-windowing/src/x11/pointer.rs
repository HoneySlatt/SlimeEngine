//! The pointer on X11, through XInput2's events of the master pointers, which the server sends
//! with each device's own times and positions to the sixteenth of a pixel: what the layer reads of
//! them, knowing neither the connection nor the clock.
//!
//! - *Moves*: the position in the window's pixels; entering moves it there, leaving lets it go.
//! - *Buttons*: X11's first three, then 8 and 9 as back and forward.
//! - *The wheel*: buttons 4 to 7 pressed, a notch each, from a mouse's wheel; the scroll valuators
//!   that move by fractions of a notch, from a touchpad, their increment a notch. The server
//!   emulates each from the other for older clients, marking the copies, which are skipped: each
//!   motion counts once. Its signs are turned: uwui's wheel scrolls up for a positive motion,
//!   X11's down.

use std::collections::HashMap;

use slime_input::{ButtonState, MouseButton, Pointer, ScrollDelta};
use x11rb::protocol::xinput::{DeviceClass, DeviceClassData, Fp3232, ScrollType};

use crate::Input;

/// A scroll valuator of a device: its number among the device's valuators, its direction, the
/// motion of a notch, and its last value, none until an event gives one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Axis {
    number: u16,
    vertical: bool,
    increment: f64,
    last: Option<f64>,
}

/// The scroll valuators of each master pointer, by its device's id.
#[derive(Debug, Default)]
pub(super) struct Pointers {
    axes: HashMap<u16, Vec<Axis>>,
}

/// One of XInput2's events of a pointer, as the layer reads it.
#[derive(Clone, Debug)]
pub(super) enum Event<'a> {
    /// It entered the window at `x`, `y`.
    Entered { x: f64, y: f64 },
    /// It left the window.
    Left,
    /// It moved to `x`, `y`; its valuators that `mask` names, in order.
    Moved {
        device: u16,
        x: f64,
        y: f64,
        mask: &'a [u32],
        values: &'a [Fp3232],
    },
    /// X11's button `button` went down or up.
    Button { button: u32, state: ButtonState },
}

/// A position in sixteenths of a pixel, as XInput2 gives it, in pixels.
pub(super) fn pixels(fixed: i32) -> f64 {
    f64::from(fixed) / 65536.0
}

/// A valuator's value in pixels or its own unit, as XInput2 gives it in 32.32 fixed point.
fn value(fixed: Fp3232) -> f64 {
    f64::from(fixed.integral) + f64::from(fixed.frac) / 4_294_967_296.0
}

impl Pointers {
    /// The device `device`'s classes, as XInput2 tells them at the start or when its master
    /// switched to another device: its scroll valuators, whose values start again.
    pub(super) fn classes(&mut self, device: u16, classes: &[DeviceClass]) {
        let axes = classes
            .iter()
            .filter_map(|class| match &class.data {
                DeviceClassData::Scroll(scroll) => {
                    let increment = value(scroll.increment);
                    (increment != 0.0 && increment.is_finite()).then_some(Axis {
                        number: scroll.number,
                        vertical: scroll.scroll_type == ScrollType::VERTICAL,
                        increment,
                        last: None,
                    })
                }
                _ => None,
            })
            .collect();
        self.axes.insert(device, axes);
    }

    /// What `event` gives as a person's input, the server's copies emulated for older clients
    /// skipped (`emulated`).
    pub(super) fn input(&mut self, event: Event<'_>, emulated: bool) -> Vec<Input> {
        if emulated {
            return Vec::new();
        }
        match event {
            Event::Entered { x, y } => {
                // The valuators' values went on outside the window: the next starts again.
                for axes in self.axes.values_mut() {
                    for axis in axes {
                        axis.last = None;
                    }
                }
                vec![moved(x, y)]
            }
            Event::Left => vec![Input::PointerLeft {
                pointer: Pointer::MOUSE,
            }],
            Event::Moved {
                device,
                x,
                y,
                mask,
                values,
            } => {
                let mut inputs = vec![moved(x, y)];
                if let Some(notches) = self.scrolled(device, mask, values) {
                    inputs.push(Input::Wheel(ScrollDelta::Lines {
                        x: -notches[0] as f32,
                        y: -notches[1] as f32,
                    }));
                }
                inputs
            }
            Event::Button { button, state } => match button {
                4..=7 => {
                    // A notch at each press; its release says nothing more.
                    if state == ButtonState::Released {
                        return Vec::new();
                    }
                    let (x, y) = match button {
                        4 => (0.0, 1.0),
                        5 => (0.0, -1.0),
                        6 => (1.0, 0.0),
                        _ => (-1.0, 0.0),
                    };
                    vec![Input::Wheel(ScrollDelta::Lines { x, y })]
                }
                _ => vec![Input::PointerButton {
                    pointer: Pointer::MOUSE,
                    button: mouse_button(button),
                    state,
                }],
            },
        }
    }

    /// The notches that `device`'s scroll valuators moved by, across then down, from the values
    /// that `mask` names; none when none moved.
    fn scrolled(&mut self, device: u16, mask: &[u32], values: &[Fp3232]) -> Option<[f64; 2]> {
        let axes = self.axes.get_mut(&device)?;
        let mut notches = [0.0, 0.0];
        let mut moved = false;
        // The values come in the order of the mask's bits.
        let mut given = values.iter();
        for (word, bits) in mask.iter().enumerate() {
            for bit in 0..32 {
                if bits & (1 << bit) == 0 {
                    continue;
                }
                let Some(&fixed) = given.next() else {
                    return moved.then_some(notches);
                };
                let number = word * 32 + bit;
                let Some(axis) = axes
                    .iter_mut()
                    .find(|axis| usize::from(axis.number) == number)
                else {
                    continue;
                };
                let now = value(fixed);
                if let Some(last) = axis.last.replace(now) {
                    notches[usize::from(axis.vertical)] += (now - last) / axis.increment;
                    moved = true;
                }
            }
        }
        (moved && notches != [0.0, 0.0]).then_some(notches)
    }
}

/// The mouse moved to `x`, `y`, in the window's pixels.
fn moved(x: f64, y: f64) -> Input {
    Input::PointerMoved {
        pointer: Pointer::MOUSE,
        x: x as f32,
        y: y as f32,
    }
}

/// X11's button `button`, 4 to 7 being the wheel's.
fn mouse_button(button: u32) -> MouseButton {
    match button {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        8 => MouseButton::Back,
        9 => MouseButton::Forward,
        other => MouseButton::Other(u16::try_from(other).unwrap_or(u16::MAX)),
    }
}

#[cfg(test)]
mod tests {
    use x11rb::protocol::xinput::{DeviceClassDataScroll, ScrollFlags};

    use super::*;

    fn fixed(value: f64) -> Fp3232 {
        let integral = value.floor();
        Fp3232 {
            integral: integral as i32,
            frac: ((value - integral) * 4_294_967_296.0) as u32,
        }
    }

    fn scroll(number: u16, scroll_type: ScrollType, increment: f64) -> DeviceClass {
        DeviceClass {
            len: 0,
            sourceid: 0,
            data: DeviceClassData::Scroll(DeviceClassDataScroll {
                number,
                scroll_type,
                flags: ScrollFlags::default(),
                increment: fixed(increment),
            }),
        }
    }

    /// A touchpad's pointer, device 2: across on valuator 2, down on 3, 15 a notch.
    fn touchpad() -> Pointers {
        let mut pointers = Pointers::default();
        pointers.classes(
            2,
            &[
                scroll(2, ScrollType::HORIZONTAL, 15.0),
                scroll(3, ScrollType::VERTICAL, 15.0),
            ],
        );
        pointers
    }

    fn moved_by(pointers: &mut Pointers, across: f64, down: f64) -> Vec<Input> {
        // Valuators 0 and 1, the position, then 2 and 3.
        let values = [fixed(10.0), fixed(20.0), fixed(across), fixed(down)];
        pointers.input(
            Event::Moved {
                device: 2,
                x: 10.0,
                y: 20.0,
                mask: &[0b1111],
                values: &values,
            },
            false,
        )
    }

    #[test]
    fn the_buttons_are_x11s_and_the_wheels_notches_turned() {
        let mut pointers = Pointers::default();
        let pressed = |pointers: &mut Pointers, button| {
            pointers.input(
                Event::Button {
                    button,
                    state: ButtonState::Pressed,
                },
                false,
            )
        };
        assert_eq!(
            pressed(&mut pointers, 3),
            [Input::PointerButton {
                pointer: Pointer::MOUSE,
                button: MouseButton::Right,
                state: ButtonState::Pressed
            }]
        );
        assert_eq!(
            pressed(&mut pointers, 9),
            [Input::PointerButton {
                pointer: Pointer::MOUSE,
                button: MouseButton::Forward,
                state: ButtonState::Pressed
            }]
        );
        // Button 4 scrolls up, 7 right.
        assert_eq!(
            pressed(&mut pointers, 4),
            [Input::Wheel(ScrollDelta::Lines { x: 0.0, y: 1.0 })]
        );
        assert_eq!(
            pressed(&mut pointers, 7),
            [Input::Wheel(ScrollDelta::Lines { x: -1.0, y: 0.0 })]
        );
        let released = pointers.input(
            Event::Button {
                button: 5,
                state: ButtonState::Released,
            },
            false,
        );
        assert!(released.is_empty());
    }

    #[test]
    fn the_scroll_valuators_move_by_fractions_of_a_notch_from_their_last_value() {
        let mut pointers = touchpad();
        // The first values only set where the valuators stand.
        assert_eq!(moved_by(&mut pointers, 100.0, 300.0), [moved(10.0, 20.0)]);
        // Down 7.5, half a notch: scrolls up by half a line in uwui's sense, turned.
        assert_eq!(
            moved_by(&mut pointers, 100.0, 307.5),
            [
                moved(10.0, 20.0),
                Input::Wheel(ScrollDelta::Lines { x: -0.0, y: -0.5 })
            ]
        );
        // Left 30, two notches.
        assert_eq!(
            moved_by(&mut pointers, 70.0, 307.5),
            [
                moved(10.0, 20.0),
                Input::Wheel(ScrollDelta::Lines { x: 2.0, y: -0.0 })
            ]
        );
        // Entering again starts the values again.
        pointers.input(Event::Entered { x: 0.0, y: 0.0 }, false);
        assert_eq!(moved_by(&mut pointers, 0.0, 0.0), [moved(10.0, 20.0)]);
    }

    #[test]
    fn the_servers_copies_for_older_clients_are_skipped() {
        let mut pointers = touchpad();
        let copy = pointers.input(
            Event::Button {
                button: 4,
                state: ButtonState::Pressed,
            },
            true,
        );
        assert!(copy.is_empty());
    }

    #[test]
    fn valuators_of_a_device_without_scroll_classes_move_the_pointer_alone() {
        let mut pointers = touchpad();
        // Its master switched to a mouse without scroll valuators.
        pointers.classes(2, &[]);
        assert_eq!(moved_by(&mut pointers, 1.0, 2.0), [moved(10.0, 20.0)]);
        assert_eq!(moved_by(&mut pointers, 50.0, 90.0), [moved(10.0, 20.0)]);
    }

    #[test]
    fn a_valuator_beyond_the_first_word_of_the_mask_is_read() {
        let mut pointers = Pointers::default();
        pointers.classes(5, &[scroll(33, ScrollType::VERTICAL, 1.0)]);
        let event = |pointers: &mut Pointers, down: f64| {
            let values = [fixed(1.0), fixed(down)];
            pointers.input(
                Event::Moved {
                    device: 5,
                    x: 0.0,
                    y: 0.0,
                    mask: &[0b1, 0b10],
                    values: &values,
                },
                false,
            )
        };
        event(&mut pointers, 4.0);
        assert_eq!(
            event(&mut pointers, 5.0)[1],
            Input::Wheel(ScrollDelta::Lines { x: -0.0, y: -1.0 })
        );
    }

    #[test]
    fn positions_come_in_sixteenths_of_a_pixel() {
        assert_eq!(pixels(65536 * 3 + 32768), 3.5);
        assert_eq!(pixels(-65536), -1.0);
    }
}
