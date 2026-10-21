//! The input as data: the keys and mouse buttons held, and those pressed and released since the
//! last frame (`docs/adr/0107-the-execution-model.md`).

use crate::event::Event;
use crate::input::{ButtonState, KeyCode, MouseButton};

/// The keys and mouse buttons held, and those pressed and released since the last frame: what a
/// game's simulation reads of the player's hands, as data, whether they come from the keyboard and
/// the mouse or from a test's script (`docs/adr/0107-the-execution-model.md`).
///
/// `slime-app`'s host keeps it from the window's events ([`Input::handle`]) and ends each frame
/// with [`Input::end_frame`], after the simulation has read it; a host without a window, such as a
/// test, presses and releases its keys itself.
///
/// ```
/// use slime_platform::{Input, KeyCode};
///
/// let mut input = Input::default();
/// input.keys_mut().press(KeyCode::KeyW);
/// assert!(input.keys().held(KeyCode::KeyW) && input.keys().pressed(KeyCode::KeyW));
/// input.end_frame();
/// // Still held, no longer pressed in this frame.
/// assert!(input.keys().held(KeyCode::KeyW) && !input.keys().pressed(KeyCode::KeyW));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Input {
    keys: Buttons<KeyCode>,
    mouse: Buttons<MouseButton>,
}

impl Input {
    /// The keys, by their position on the keyboard ([`KeyCode`]).
    pub fn keys(&self) -> &Buttons<KeyCode> {
        &self.keys
    }

    /// The keys, to press and release them: what a host that has no keyboard, such as a test, does.
    pub fn keys_mut(&mut self) -> &mut Buttons<KeyCode> {
        &mut self.keys
    }

    /// The mouse buttons.
    pub fn mouse(&self) -> &Buttons<MouseButton> {
        &self.mouse
    }

    /// The mouse buttons, to press and release them.
    pub fn mouse_mut(&mut self) -> &mut Buttons<MouseButton> {
        &mut self.mouse
    }

    /// Follows `event`: a key or a mouse button going down or up; the presses that the keyboard's
    /// auto-repeat sends, and every other event, change nothing. When `taken`, as when a
    /// presentation took the event for itself, a key or a button put down is not followed, and one
    /// let go is, so that none stays held. The window's keys and buttons pair up (see
    /// [`Event::Key`]): one still down when the window loses the focus is released then.
    pub fn handle(&mut self, event: &Event, taken: bool) {
        match event {
            Event::Key(key) if !key.repeat && !(taken && key.state == ButtonState::Pressed) => {
                self.keys.follow(key.physical, key.state);
            }
            Event::MouseButton { button, state } if !(taken && *state == ButtonState::Pressed) => {
                self.mouse.follow(*button, *state);
            }
            _ => {}
        }
    }

    /// Ends a frame: the keys and buttons pressed and released in it are no longer, and those held
    /// stay held.
    pub fn end_frame(&mut self) {
        self.keys.end_frame();
        self.mouse.end_frame();
    }
}

/// Which buttons of one kind, keys or mouse buttons, are held, and which were pressed and released
/// since the last frame ([`Input::end_frame`]). A button pressed and released within one frame
/// reads as pressed and as released in it, and not as held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Buttons<B> {
    /// In the order they went down.
    held: Vec<B>,
    pressed: Vec<B>,
    released: Vec<B>,
}

impl<B> Default for Buttons<B> {
    fn default() -> Self {
        Self {
            held: Vec::new(),
            pressed: Vec::new(),
            released: Vec::new(),
        }
    }
}

impl<B: Copy + PartialEq> Buttons<B> {
    /// Whether `button` is down.
    pub fn held(&self, button: B) -> bool {
        self.held.contains(&button)
    }

    /// Whether `button` went down in this frame.
    pub fn pressed(&self, button: B) -> bool {
        self.pressed.contains(&button)
    }

    /// Whether `button` went up in this frame.
    pub fn released(&self, button: B) -> bool {
        self.released.contains(&button)
    }

    /// The buttons down, in the order they went down.
    pub fn all_held(&self) -> &[B] {
        &self.held
    }

    /// Puts `button` down, pressed in this frame; nothing changes if it is down already.
    pub fn press(&mut self, button: B) {
        if !self.held(button) {
            self.held.push(button);
            self.pressed.push(button);
        }
    }

    /// Lets `button` go, released in this frame; nothing changes if it is not down.
    pub fn release(&mut self, button: B) {
        if let Some(index) = self.held.iter().position(|&held| held == button) {
            self.held.remove(index);
            self.released.push(button);
        }
    }

    fn follow(&mut self, button: B, state: ButtonState) {
        match state {
            ButtonState::Pressed => self.press(button),
            ButtonState::Released => self.release(button),
        }
    }

    fn end_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Key, KeyEvent};

    fn key(physical: KeyCode, state: ButtonState, repeat: bool) -> Event {
        Event::Key(KeyEvent {
            physical,
            logical: Key::Unidentified,
            text: None,
            state,
            repeat,
        })
    }

    #[test]
    fn a_key_held_stays_held_from_frame_to_frame_until_released() {
        let mut input = Input::default();
        input.handle(&key(KeyCode::KeyW, ButtonState::Pressed, false), false);
        assert!(input.keys().held(KeyCode::KeyW));
        assert!(input.keys().pressed(KeyCode::KeyW));
        input.end_frame();
        // The keyboard's auto-repeat is no new press.
        input.handle(&key(KeyCode::KeyW, ButtonState::Pressed, true), false);
        assert!(input.keys().held(KeyCode::KeyW));
        assert!(!input.keys().pressed(KeyCode::KeyW));
        input.end_frame();
        input.handle(&key(KeyCode::KeyW, ButtonState::Released, false), false);
        assert!(!input.keys().held(KeyCode::KeyW));
        assert!(input.keys().released(KeyCode::KeyW));
        input.end_frame();
        assert_eq!(input, Input::default());
    }

    #[test]
    fn a_key_pressed_and_released_within_a_frame_reads_as_both_and_not_as_held() {
        let mut input = Input::default();
        input.handle(&key(KeyCode::KeyE, ButtonState::Pressed, false), false);
        input.handle(&key(KeyCode::KeyE, ButtonState::Released, false), false);
        assert!(input.keys().pressed(KeyCode::KeyE));
        assert!(input.keys().released(KeyCode::KeyE));
        assert!(!input.keys().held(KeyCode::KeyE));
    }

    #[test]
    fn what_was_taken_is_not_put_down_and_always_let_go() {
        let mut input = Input::default();
        // A key typed into a field of an overlay: neither held nor pressed.
        input.handle(&key(KeyCode::KeyR, ButtonState::Pressed, false), true);
        assert!(!input.keys().held(KeyCode::KeyR) && !input.keys().pressed(KeyCode::KeyR));
        // A key put down for the game, let go while the overlay takes the keyboard: let go.
        input.handle(&key(KeyCode::KeyW, ButtonState::Pressed, false), false);
        input.handle(&key(KeyCode::KeyW, ButtonState::Released, false), true);
        assert!(!input.keys().held(KeyCode::KeyW) && input.keys().released(KeyCode::KeyW));
        // The same for a click on a panel, and a button let go over it.
        let button = |state| Event::MouseButton {
            button: MouseButton::Left,
            state,
        };
        input.handle(&button(ButtonState::Pressed), true);
        assert!(!input.mouse().held(MouseButton::Left));
        input.handle(&button(ButtonState::Pressed), false);
        input.handle(&button(ButtonState::Released), true);
        assert!(!input.mouse().held(MouseButton::Left));
    }

    #[test]
    fn mouse_buttons_are_followed_apart_from_keys_and_in_the_order_they_went_down() {
        let mut input = Input::default();
        for button in [MouseButton::Right, MouseButton::Left] {
            let press = Event::MouseButton {
                button,
                state: ButtonState::Pressed,
            };
            input.handle(&press, false);
        }
        assert_eq!(
            input.mouse().all_held(),
            [MouseButton::Right, MouseButton::Left]
        );
        assert!(input.keys().all_held().is_empty());
        // A release of what is not down, as a script may send, changes nothing.
        input.keys_mut().release(KeyCode::KeyA);
        assert!(!input.keys().released(KeyCode::KeyA));
    }
}
