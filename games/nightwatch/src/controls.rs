//! The keys the player holds, and what they ask for. The engine reports each key's presses and
//! releases, not which keys are down: the game keeps that itself, until Stage 6 or 7
//! (ADR-0057). Keys are read by their position on the keyboard, so that W, A, S and D are Z, Q, S
//! and D on a French layout.

use slime_platform::{ButtonState, KeyCode, KeyEvent};
use slime_render::glam::Vec2;

/// Which of the game's keys are down.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    turn_left: bool,
    turn_right: bool,
}

impl Controls {
    /// Follows `key`, if it is one of the game's.
    pub fn handle(&mut self, key: &KeyEvent) {
        let down = key.state == ButtonState::Pressed;
        let held = match key.physical {
            KeyCode::KeyW => &mut self.forward,
            KeyCode::KeyS => &mut self.back,
            KeyCode::KeyA => &mut self.left,
            KeyCode::KeyD => &mut self.right,
            KeyCode::ArrowLeft => &mut self.turn_left,
            KeyCode::ArrowRight => &mut self.turn_right,
            _ => return,
        };
        *held = down;
    }

    /// Where the player asks to move: x to the right, y forward, each from −1 to 1. Opposite keys
    /// cancel each other.
    pub fn movement(&self) -> Vec2 {
        let axis = |positive: bool, negative: bool| f32::from(positive) - f32::from(negative);
        Vec2::new(axis(self.right, self.left), axis(self.forward, self.back))
    }

    /// Which way the player asks the camera to turn: 1 to the left, −1 to the right, 0 neither.
    pub fn turn(&self) -> f32 {
        f32::from(self.turn_left) - f32::from(self.turn_right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slime_platform::Key;

    fn key(physical: KeyCode, state: ButtonState) -> KeyEvent {
        KeyEvent {
            physical,
            logical: Key::Unidentified,
            text: None,
            state,
            repeat: false,
        }
    }

    #[test]
    fn held_keys_move_and_released_ones_stop() {
        let mut controls = Controls::default();
        controls.handle(&key(KeyCode::KeyW, ButtonState::Pressed));
        controls.handle(&key(KeyCode::KeyD, ButtonState::Pressed));
        assert_eq!(controls.movement(), Vec2::new(1.0, 1.0));
        controls.handle(&key(KeyCode::KeyA, ButtonState::Pressed));
        assert_eq!(
            controls.movement(),
            Vec2::new(0.0, 1.0),
            "left and right cancel"
        );
        controls.handle(&key(KeyCode::KeyW, ButtonState::Released));
        controls.handle(&key(KeyCode::KeyD, ButtonState::Released));
        controls.handle(&key(KeyCode::KeyA, ButtonState::Released));
        assert_eq!(controls.movement(), Vec2::ZERO);
        controls.handle(&key(KeyCode::ArrowLeft, ButtonState::Pressed));
        assert_eq!(controls.turn(), 1.0);
    }
}
