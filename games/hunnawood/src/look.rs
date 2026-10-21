//! Where the player looks, which the arrow keys turn: the direction that the movement keys move the
//! hero relative to, and that the camera looks along from behind the hero.

use slime_reflect::Reflect;
use slime_render::glam::Vec2;
use slime_world::Component;

/// How fast the player turns where it looks, in radians per second.
pub const TURN_SPEED: f32 = 1.8;

/// Where the player looks, a component of the game's on the hero's entity: the rules move the hero
/// relative to it, and the camera looks along it. The rules turn it at real time, whatever the
/// speed of game time, so that the player looks around a game that is paused or slowed down, as
/// the camera of `docs/adr/0107-the-execution-model.md` does.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "hunnawood::Look")]
pub struct Look {
    /// Which way the player looks, in radians: 0 looks along −z, and it grows turning to the left.
    #[reflect(unit = "rad")]
    pub yaw: f32,
}

impl Component for Look {}

impl Look {
    /// Looking along `direction` on the ground, x and z.
    pub fn along(direction: Vec2) -> Self {
        Self {
            yaw: (-direction.x).atan2(-direction.y),
        }
    }

    /// Turns for `seconds` at [`TURN_SPEED`]: `turn` 1 to the left, −1 to the right.
    pub fn turn(&mut self, turn: f32, seconds: f32) {
        self.yaw += turn * TURN_SPEED * seconds;
    }

    /// The directions on the ground, x and z, of where the player looks and of its right.
    pub fn ground_axes(&self) -> (Vec2, Vec2) {
        let (sin, cos) = self.yaw.sin_cos();
        (Vec2::new(-sin, -cos), Vec2::new(cos, -sin))
    }

    /// The direction on the ground that `movement` asks for, x to the right of where the player
    /// looks and y forward, so that forward moves away from the camera whichever way it looks.
    pub fn on_ground(&self, movement: Vec2) -> Vec2 {
        let (forward, right) = self.ground_axes();
        right * movement.x + forward * movement.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_moves_along_the_look_and_right_to_its_right() {
        for direction in [Vec2::X, -Vec2::Y, Vec2::new(0.6, 0.8)] {
            let look = Look::along(direction);
            let (forward, right) = look.ground_axes();
            assert!(
                forward.abs_diff_eq(direction, 1e-5),
                "{forward} for {direction}"
            );
            // Its right on the ground is forward turned a quarter turn clockwise seen from above.
            assert!(right.abs_diff_eq(Vec2::new(-forward.y, forward.x), 1e-5));
            assert!(
                look.on_ground(Vec2::new(0.0, 1.0))
                    .abs_diff_eq(forward, 1e-5)
            );
        }
    }

    #[test]
    fn turning_left_turns_the_look_to_the_left() {
        let mut look = Look::along(-Vec2::Y);
        look.turn(1.0, std::f32::consts::FRAC_PI_2 / TURN_SPEED);
        let (forward, _) = look.ground_axes();
        // Looking along −z, the left is −x.
        assert!(forward.abs_diff_eq(-Vec2::X, 1e-5), "{forward}");
    }
}
