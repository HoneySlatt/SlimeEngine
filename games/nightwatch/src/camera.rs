//! The camera, above and behind the player, which the arrow keys turn around it.

use slime_render::Camera;
use slime_render::glam::{Vec2, Vec3};

/// How far the camera is from the point it looks at, in metres.
const DISTANCE: f32 = 10.0;
/// How steeply it looks down, in radians above the ground: steep enough to see over a wall 1.5 m
/// behind the player.
const PITCH: f32 = 1.05;
/// How high above the ground the point it looks at is, in metres: about the player's middle.
const LOOK_HEIGHT: f32 = 0.4;
/// How fast it turns, in radians per second.
const TURN_SPEED: f32 = 1.8;

/// A camera that follows the player, turned by the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowCamera {
    /// Which way it looks, in radians: 0 looks along −z, and it grows turning to the left.
    yaw: f32,
}

impl FollowCamera {
    /// A camera that looks along `direction` on the ground, x and z.
    pub fn looking_along(direction: Vec2) -> Self {
        Self {
            yaw: (-direction.x).atan2(-direction.y),
        }
    }

    /// Turns for `seconds` at its speed: `turn` 1 to the left, −1 to the right.
    pub fn turn(&mut self, turn: f32, seconds: f32) {
        self.yaw += turn * TURN_SPEED * seconds;
    }

    /// The directions on the ground, x and z, of the camera's forward and of its right.
    pub fn ground_axes(&self) -> (Vec2, Vec2) {
        let (sin, cos) = self.yaw.sin_cos();
        (Vec2::new(-sin, -cos), Vec2::new(cos, -sin))
    }

    /// The direction on the ground that `movement` asks for, x to the camera's right and y
    /// forward, so that forward moves away from the camera whichever way it looks.
    pub fn on_ground(&self, movement: Vec2) -> Vec2 {
        let (forward, right) = self.ground_axes();
        right * movement.x + forward * movement.y
    }

    /// The camera looking at `player`, on the ground.
    pub fn camera(&self, player: Vec2) -> Camera {
        let target = Vec3::new(player.x, LOOK_HEIGHT, player.y);
        let (forward, _) = self.ground_axes();
        let (sin, cos) = PITCH.sin_cos();
        let back = Vec3::new(-forward.x, 0.0, -forward.y) * cos + Vec3::Y * sin;
        Camera {
            eye: target + back * DISTANCE,
            target,
            ..Camera::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_moves_away_from_the_camera_and_right_to_its_right() {
        for direction in [Vec2::X, -Vec2::Y, Vec2::new(0.6, 0.8)] {
            let camera = FollowCamera::looking_along(direction);
            let (forward, right) = camera.ground_axes();
            assert!(
                forward.abs_diff_eq(direction, 1e-5),
                "{forward} for {direction}"
            );
            let view = camera.camera(Vec2::ZERO);
            let looking = (view.target - view.eye).normalize();
            // The camera looks down along forward, and its right on the ground is forward turned a
            // quarter turn clockwise seen from above.
            assert!(
                Vec2::new(looking.x, looking.z)
                    .normalize()
                    .abs_diff_eq(forward, 1e-5)
            );
            assert!(right.abs_diff_eq(Vec2::new(-forward.y, forward.x), 1e-5));
            assert!(
                camera
                    .on_ground(Vec2::new(0.0, 1.0))
                    .abs_diff_eq(forward, 1e-5)
            );
        }
    }

    #[test]
    fn turning_left_turns_the_view_to_the_left() {
        let mut camera = FollowCamera::looking_along(-Vec2::Y);
        camera.turn(1.0, std::f32::consts::FRAC_PI_2 / TURN_SPEED);
        let (forward, _) = camera.ground_axes();
        // Looking along −z, the left is −x.
        assert!(forward.abs_diff_eq(-Vec2::X, 1e-5), "{forward}");
    }
}
