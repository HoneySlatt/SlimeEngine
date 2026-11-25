//! The mouse's messages of Win32, turned into the host's input: its moves in the window's physical
//! pixels, its buttons and its wheel, in lines.
//!
//! While a button is held, the window holds the mouse's capture, so that a drag that leaves the
//! window keeps its moves and its release, as Wayland's and X11's implicit grabs give them; a
//! capture taken by another window releases what the window held.

use slime_input::{ButtonState, MouseButton, Pointer, ScrollDelta};
use windows_sys::Win32::Foundation::{LPARAM, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WHEEL_DELTA, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
    XBUTTON1, XBUTTON2,
};

use crate::Input;

/// The low word of `value`, signed, as a point's coordinate or a wheel's turn.
fn signed_low(value: usize) -> i16 {
    (value & 0xFFFF) as u16 as i16
}

/// The high word of `value`, signed.
fn signed_high(value: usize) -> i16 {
    ((value >> 16) & 0xFFFF) as u16 as i16
}

/// The mouse's message `message`, with its parameters, as the host's input; none for another
/// message, or a side button that Windows names neither first nor second.
pub(super) fn translate(message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<Input> {
    let button = |button, state| {
        Some(Input::PointerButton {
            pointer: Pointer::MOUSE,
            button,
            state,
        })
    };
    let side = || match signed_high(wparam) as u16 {
        XBUTTON1 => Some(MouseButton::Back),
        XBUTTON2 => Some(MouseButton::Forward),
        _ => None,
    };
    let turned = || f32::from(signed_high(wparam)) / WHEEL_DELTA as f32;
    match message {
        WM_MOUSEMOVE if false => Some(Input::PointerMoved {
            pointer: Pointer::MOUSE,
            x: f32::from(signed_low(lparam as usize)),
            y: f32::from(signed_high(lparam as usize)),
        }),
        WM_LBUTTONDOWN => button(MouseButton::Left, ButtonState::Pressed),
        WM_LBUTTONUP => button(MouseButton::Left, ButtonState::Released),
        WM_RBUTTONDOWN => button(MouseButton::Right, ButtonState::Pressed),
        WM_RBUTTONUP => button(MouseButton::Right, ButtonState::Released),
        WM_MBUTTONDOWN => button(MouseButton::Middle, ButtonState::Pressed),
        WM_MBUTTONUP => button(MouseButton::Middle, ButtonState::Released),
        WM_XBUTTONDOWN => button(side()?, ButtonState::Pressed),
        WM_XBUTTONUP => button(side()?, ButtonState::Released),
        // Turned away from the person: up, as the host counts.
        WM_MOUSEWHEEL => Some(Input::Wheel(ScrollDelta::Lines {
            x: 0.0,
            y: turned(),
        })),
        // Tilted right: the host counts left as positive.
        WM_MOUSEHWHEEL => Some(Input::Wheel(ScrollDelta::Lines {
            x: -turned(),
            y: 0.0,
        })),
        _ => None,
    }
}

/// The buttons held, which keep the mouse's capture.
#[derive(Debug, Default)]
pub(super) struct Held(Vec<MouseButton>);

impl Held {
    /// Follows `input`: whether the window must take the capture, the first button pressed, or
    /// give it back, the last released.
    pub(super) fn follow(&mut self, input: &Input) -> Option<bool> {
        let Input::PointerButton { button, state, .. } = input else {
            return None;
        };
        let before = self.0.is_empty();
        match state {
            ButtonState::Pressed if !self.0.contains(button) => self.0.push(*button),
            ButtonState::Released => self.0.retain(|held| held != button),
            _ => {}
        }
        match (before, self.0.is_empty()) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }

    /// Whether a button is held.
    pub(super) fn any(&self) -> bool {
        !self.0.is_empty()
    }

    /// The releases of every button held, which the capture lost to another window takes.
    pub(super) fn release_all(&mut self) -> Vec<Input> {
        self.0
            .drain(..)
            .map(|button| Input::PointerButton {
                pointer: Pointer::MOUSE,
                button,
                state: ButtonState::Released,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A point's parameter, as Win32 packs one.
    fn point(x: i16, y: i16) -> LPARAM {
        (usize::from(x as u16) | usize::from(y as u16) << 16) as LPARAM
    }

    /// A wheel's parameter, its turn in the high word.
    fn turn(delta: i16) -> WPARAM {
        usize::from(delta as u16) << 16
    }

    #[test]
    fn a_move_gives_its_place_in_the_windows_pixels_left_and_above_included() {
        assert_eq!(
            translate(WM_MOUSEMOVE, 0, point(640, 360)),
            Some(Input::PointerMoved {
                pointer: Pointer::MOUSE,
                x: 640.0,
                y: 360.0
            })
        );
        // A drag held past the window's top left, under the capture.
        assert_eq!(
            translate(WM_MOUSEMOVE, 0, point(-5, -12)),
            Some(Input::PointerMoved {
                pointer: Pointer::MOUSE,
                x: -5.0,
                y: -12.0
            })
        );
    }

    #[test]
    fn the_buttons_and_the_side_buttons_are_named() {
        let pressed = |button| {
            Some(Input::PointerButton {
                pointer: Pointer::MOUSE,
                button,
                state: ButtonState::Pressed,
            })
        };
        assert_eq!(translate(WM_LBUTTONDOWN, 0, 0), pressed(MouseButton::Left));
        assert_eq!(translate(WM_RBUTTONDOWN, 0, 0), pressed(MouseButton::Right));
        assert_eq!(
            translate(WM_MBUTTONDOWN, 0, 0),
            pressed(MouseButton::Middle)
        );
        assert_eq!(
            translate(WM_XBUTTONDOWN, (XBUTTON1 as usize) << 16, 0),
            pressed(MouseButton::Back)
        );
        assert_eq!(
            translate(WM_XBUTTONUP, (XBUTTON2 as usize) << 16, 0),
            Some(Input::PointerButton {
                pointer: Pointer::MOUSE,
                button: MouseButton::Forward,
                state: ButtonState::Released,
            })
        );
        assert_eq!(translate(WM_XBUTTONDOWN, 3 << 16, 0), None);
    }

    #[test]
    fn the_wheel_scrolls_up_away_from_the_person_and_left_as_positive() {
        assert_eq!(
            translate(WM_MOUSEWHEEL, turn(120), 0),
            Some(Input::Wheel(ScrollDelta::Lines { x: 0.0, y: 1.0 }))
        );
        // A wheel of a fine resolution turns in parts of a notch.
        assert_eq!(
            translate(WM_MOUSEWHEEL, turn(-60), 0),
            Some(Input::Wheel(ScrollDelta::Lines { x: 0.0, y: -0.5 }))
        );
        assert_eq!(
            translate(WM_MOUSEHWHEEL, turn(120), 0),
            Some(Input::Wheel(ScrollDelta::Lines { x: -1.0, y: 0.0 }))
        );
    }

    #[test]
    fn the_capture_is_taken_at_the_first_button_and_given_back_at_the_last() {
        let mut held = Held::default();
        let input = |button, state| Input::PointerButton {
            pointer: Pointer::MOUSE,
            button,
            state,
        };
        assert_eq!(
            held.follow(&input(MouseButton::Left, ButtonState::Pressed)),
            Some(true)
        );
        assert_eq!(
            held.follow(&input(MouseButton::Right, ButtonState::Pressed)),
            None
        );
        assert_eq!(
            held.follow(&input(MouseButton::Left, ButtonState::Released)),
            None
        );
        assert!(held.any());
        assert_eq!(
            held.follow(&input(MouseButton::Right, ButtonState::Released)),
            Some(false)
        );
        // A release without its press, as after the capture was lost: nothing to give back.
        assert_eq!(
            held.follow(&input(MouseButton::Left, ButtonState::Released)),
            None
        );
        held.follow(&input(MouseButton::Middle, ButtonState::Pressed));
        assert_eq!(
            held.release_all(),
            [input(MouseButton::Middle, ButtonState::Released)]
        );
        assert!(!held.any());
    }
}
