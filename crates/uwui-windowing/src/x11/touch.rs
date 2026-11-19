//! Touch on X11, through XInput 2.2's touches of the master devices: what the layer reads of them,
//! knowing neither the connection nor the clock. Each touch is named by its device, the one that
//! the finger touches, and the server's number of the touch, which is unique on that device while
//! the finger is down; each finger is a pointer of its own (`crate::fingers`). The pointer's moves
//! and buttons that the server emulates from a touch for older clients are skipped with the
//! pointer's (`super::pointer`).

use crate::Input;
use crate::fingers::Fingers;

use super::pointer;

/// Where a touch is: begun, moved, or ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Began,
    Moved,
    Ended,
}

/// What a touch's event gives the host.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Touched {
    /// Told at its reception: a finger still down under the same number, which leaves.
    pub(super) received: Option<Input>,
    /// Told at the event's time.
    pub(super) timed: Vec<Input>,
}

/// The touch `id` of the device `source` at `phase`, at `x` and `y` in the window's 65,536ths of a
/// pixel, as XInput2 gives them.
pub(super) fn touch(
    fingers: &mut Fingers<(u16, u32)>,
    phase: Phase,
    source: u16,
    id: u32,
    [x, y]: [i32; 2],
) -> Touched {
    let key = (source, id);
    let at = [x, y].map(|side| pointer::pixels(side) as f32);
    match phase {
        Phase::Began => {
            let touched = fingers.down(key, at);
            Touched {
                received: touched.replaced,
                timed: touched.inputs.into(),
            }
        }
        Phase::Moved => Touched {
            received: None,
            timed: fingers.moved(key, at).into_iter().collect(),
        },
        Phase::Ended => Touched {
            received: None,
            timed: fingers.up(key).map(Vec::from).unwrap_or_default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use slime_input::{ButtonState, MouseButton, Pointer, PointerId};

    use super::*;

    fn finger(id: u32) -> Pointer {
        Pointer::finger(PointerId(id))
    }

    /// A position of XInput2's, `pixels` plus a half.
    fn half(pixels: i32) -> i32 {
        pixels * 65536 + 32768
    }

    #[test]
    fn a_touch_begins_moves_and_ends_at_its_pixels() {
        let mut fingers = Fingers::default();
        let began = touch(&mut fingers, Phase::Began, 12, 1, [half(10), half(20)]);
        assert_eq!(began.received, None);
        assert_eq!(
            began.timed,
            [
                Input::PointerMoved {
                    pointer: finger(1),
                    x: 10.5,
                    y: 20.5
                },
                Input::PointerButton {
                    pointer: finger(1),
                    button: MouseButton::Left,
                    state: ButtonState::Pressed
                },
            ]
        );
        assert_eq!(
            touch(&mut fingers, Phase::Moved, 12, 1, [half(11), 0]).timed,
            [Input::PointerMoved {
                pointer: finger(1),
                x: 11.5,
                y: 0.0
            }]
        );
        assert_eq!(
            touch(&mut fingers, Phase::Ended, 12, 1, [0, 0]).timed,
            [
                Input::PointerButton {
                    pointer: finger(1),
                    button: MouseButton::Left,
                    state: ButtonState::Released
                },
                Input::PointerLeft { pointer: finger(1) },
            ]
        );
        // Ended: its number touches nothing any more.
        assert_eq!(
            touch(&mut fingers, Phase::Moved, 12, 1, [0, 0]),
            Touched::default()
        );
    }

    #[test]
    fn the_same_number_on_two_devices_is_two_fingers() {
        let mut fingers = Fingers::default();
        touch(&mut fingers, Phase::Began, 12, 1, [0, 0]);
        let other = touch(&mut fingers, Phase::Began, 13, 1, [0, 0]);
        assert_eq!(other.received, None, "the first finger stays down");
        assert_eq!(
            other.timed[1],
            Input::PointerButton {
                pointer: finger(2),
                button: MouseButton::Left,
                state: ButtonState::Pressed
            }
        );
        // A number given again on the same device while down: the finger before leaves.
        let again = touch(&mut fingers, Phase::Began, 12, 1, [0, 0]);
        assert_eq!(
            again.received,
            Some(Input::PointerLeft { pointer: finger(1) })
        );
    }
}
