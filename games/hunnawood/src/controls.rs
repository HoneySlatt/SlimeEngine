//! What the keys the player holds ask for, read from the input that the engine gives as data
//! (`slime_platform::Input`, `docs/adr/0107-the-execution-model.md`). Keys are read by their
//! position on the keyboard, so that W, A, S and D are Z, Q, S and D on a French layout.

use slime_platform::{Input, KeyCode};
use slime_render::glam::Vec2;

/// Where the player asks to move: x to the right, y forward, each from −1 to 1. Opposite keys
/// cancel each other.
pub fn movement(input: &Input) -> Vec2 {
    let keys = input.keys();
    let axis = |positive, negative| f32::from(keys.held(positive)) - f32::from(keys.held(negative));
    Vec2::new(
        axis(KeyCode::KeyD, KeyCode::KeyA),
        axis(KeyCode::KeyW, KeyCode::KeyS),
    )
}

/// Which way the player asks to turn where it looks: 1 to the left, −1 to the right, 0 neither.
pub fn turn(input: &Input) -> f32 {
    let keys = input.keys();
    f32::from(keys.held(KeyCode::ArrowLeft)) - f32::from(keys.held(KeyCode::ArrowRight))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_keys_move_and_released_ones_stop() {
        let mut input = Input::default();
        input.keys_mut().press(KeyCode::KeyW);
        input.keys_mut().press(KeyCode::KeyD);
        assert_eq!(movement(&input), Vec2::new(1.0, 1.0));
        input.keys_mut().press(KeyCode::KeyA);
        assert_eq!(
            movement(&input),
            Vec2::new(0.0, 1.0),
            "left and right cancel"
        );
        for key in [KeyCode::KeyW, KeyCode::KeyD, KeyCode::KeyA] {
            input.keys_mut().release(key);
        }
        assert_eq!(movement(&input), Vec2::ZERO);
        input.keys_mut().press(KeyCode::ArrowLeft);
        assert_eq!(turn(&input), 1.0);
    }
}
