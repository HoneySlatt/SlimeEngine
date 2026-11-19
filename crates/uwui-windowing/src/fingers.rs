//! The fingers of a touch screen, each a pointer of its own, named anew at each touch: what every
//! layer gives of a touch, knowing neither the platform nor the clock. The platform numbers each
//! finger while it touches, by a key of its own type; uwui sees a pointer that moves there and
//! presses the primary button, moves, then lifts it and leaves.

use std::collections::HashMap;
use std::hash::Hash;

use slime_input::{ButtonState, MouseButton, Pointer, PointerId};

use crate::Input;

/// The fingers on the window, by the platform's key, and the next name of a finger.
#[derive(Debug)]
pub(crate) struct Fingers<K> {
    named: HashMap<K, PointerId>,
    next: u32,
}

impl<K> Default for Fingers<K> {
    fn default() -> Self {
        Self {
            named: HashMap::new(),
            next: 1,
        }
    }
}

/// What a finger's touch gives.
#[derive(Debug, PartialEq)]
pub(crate) struct Touched {
    /// A finger still down under the same key, against the platform's promise, which leaves so
    /// that the interface does not follow it for ever; told at its reception.
    pub(crate) replaced: Option<Input>,
    /// The new finger moved where it touched, then pressing; told at the touch's time.
    pub(crate) inputs: [Input; 2],
}

impl<K: Eq + Hash + Copy> Fingers<K> {
    /// A finger touched the window at `x` and `y`, in its physical pixels, under the platform's
    /// key `key`: a pointer named anew.
    pub(crate) fn down(&mut self, key: K, [x, y]: [f32; 2]) -> Touched {
        let finger = Pointer::finger(PointerId(self.next));
        // Never the mouse's 0, as the count turns over.
        self.next = self.next.checked_add(1).unwrap_or(1);
        let replaced = self
            .named
            .insert(key, finger.id)
            .map(|before| Input::PointerLeft {
                pointer: Pointer::finger(before),
            });
        Touched {
            replaced,
            inputs: [
                Input::PointerMoved {
                    pointer: finger,
                    x,
                    y,
                },
                Input::PointerButton {
                    pointer: finger,
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                },
            ],
        }
    }

    /// The finger of `key` moved to `x` and `y`; nothing for a key that touches nothing.
    pub(crate) fn moved(&self, key: K, [x, y]: [f32; 2]) -> Option<Input> {
        let &finger = self.named.get(&key)?;
        Some(Input::PointerMoved {
            pointer: Pointer::finger(finger),
            x,
            y,
        })
    }

    /// The finger of `key` lifted: it releases the button, then leaves; nothing for a key that
    /// touches nothing.
    pub(crate) fn up(&mut self, key: K) -> Option<[Input; 2]> {
        let pointer = Pointer::finger(self.named.remove(&key)?);
        Some([
            Input::PointerButton {
                pointer,
                button: MouseButton::Left,
                state: ButtonState::Released,
            },
            Input::PointerLeft { pointer },
        ])
    }

    /// Every finger cancelled, as when the platform takes the touch for a gesture of its own: each
    /// leaves without lifting, so that it presses nothing, in the order they touched.
    pub(crate) fn cancel(&mut self) -> Vec<Input> {
        let mut fingers: Vec<PointerId> = self.named.drain().map(|(_, finger)| finger).collect();
        fingers.sort_unstable();
        fingers
            .into_iter()
            .map(|finger| Input::PointerLeft {
                pointer: Pointer::finger(finger),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finger(id: u32) -> Pointer {
        Pointer::finger(PointerId(id))
    }

    #[test]
    fn each_touch_names_a_finger_anew() {
        let mut fingers = Fingers::default();
        let first = fingers.down(7, [1.0, 2.0]);
        assert_eq!(first.replaced, None);
        assert_eq!(
            first.inputs,
            [
                Input::PointerMoved {
                    pointer: finger(1),
                    x: 1.0,
                    y: 2.0
                },
                Input::PointerButton {
                    pointer: finger(1),
                    button: MouseButton::Left,
                    state: ButtonState::Pressed
                },
            ]
        );
        assert_eq!(
            fingers.moved(7, [3.0, 4.0]),
            Some(Input::PointerMoved {
                pointer: finger(1),
                x: 3.0,
                y: 4.0
            })
        );
        assert_eq!(
            fingers.up(7),
            Some([
                Input::PointerButton {
                    pointer: finger(1),
                    button: MouseButton::Left,
                    state: ButtonState::Released
                },
                Input::PointerLeft { pointer: finger(1) },
            ])
        );
        // The same key touching again is another finger.
        let again = fingers.down(7, [0.0, 0.0]);
        assert_eq!(
            again.inputs[1],
            Input::PointerButton {
                pointer: finger(2),
                button: MouseButton::Left,
                state: ButtonState::Pressed
            }
        );
    }

    #[test]
    fn a_key_unknown_gives_nothing_and_a_key_given_twice_lets_the_first_leave() {
        let mut fingers = Fingers::default();
        assert_eq!(fingers.moved(1, [0.0, 0.0]), None);
        assert_eq!(fingers.up(1), None);
        fingers.down(1, [0.0, 0.0]);
        let twice = fingers.down(1, [0.0, 0.0]);
        assert_eq!(
            twice.replaced,
            Some(Input::PointerLeft { pointer: finger(1) })
        );
    }

    #[test]
    fn a_cancel_lets_every_finger_leave_in_order() {
        let mut fingers = Fingers::default();
        fingers.down(9, [0.0, 0.0]);
        fingers.down(3, [0.0, 0.0]);
        assert_eq!(
            fingers.cancel(),
            [
                Input::PointerLeft { pointer: finger(1) },
                Input::PointerLeft { pointer: finger(2) },
            ]
        );
        assert!(fingers.cancel().is_empty());
    }

    #[test]
    fn the_count_turns_over_without_naming_the_mouse() {
        let mut fingers = Fingers {
            named: HashMap::new(),
            next: u32::MAX,
        };
        fingers.down(1, [0.0, 0.0]);
        let next = fingers.down(2, [0.0, 0.0]);
        assert_eq!(
            next.inputs[1],
            Input::PointerButton {
                pointer: finger(1),
                button: MouseButton::Left,
                state: ButtonState::Pressed
            }
        );
    }
}
