//! The camera, behind the hero and a little above, which the arrow keys turn around it.

use slime_render::Camera;
use slime_render::glam::{Vec2, Vec3};

use crate::zone::Collider;

/// How far the camera is from the point it looks at, in metres, when nothing stands in between.
const DISTANCE: f32 = 5.0;
/// How steeply it looks down, in radians above the ground.
const PITCH: f32 = 0.3;
/// How high above the ground the point it looks at is, in metres: about the hero's shoulders.
const LOOK_HEIGHT: f32 = 1.5;
/// How far it keeps from a collider, in metres.
const MARGIN: f32 = 0.3;
/// The steps at which the line from the hero to the camera is checked, in metres.
const SAMPLE: f32 = 0.05;
/// How fast it turns, in radians per second.
const TURN_SPEED: f32 = 1.8;

/// A camera that follows the hero, turned by the player.
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

    /// The camera looking at `hero`, on the ground, from behind it: when a collider stands between
    /// them, it comes nearer, in front of the collider, as near as it must, down to the hero's
    /// shoulders, so that it never stands inside a collider.
    pub fn camera(&self, hero: Vec2, colliders: &[Collider]) -> Camera {
        let target = Vec3::new(hero.x, LOOK_HEIGHT, hero.y);
        let (forward, _) = self.ground_axes();
        let (sin, cos) = PITCH.sin_cos();
        let toward = Vec3::new(-forward.x * cos, sin, -forward.y * cos);
        let distance = blocked(target, toward, DISTANCE, colliders)
            .map_or(DISTANCE, |clear| clear.max(SAMPLE));
        Camera {
            eye: target + toward * distance,
            target,
            ..Camera::default()
        }
    }
}

/// How far from `from`, along `direction`, of length 1, a point can stand within `distance` with
/// no collider nearer to it than the margin: `None` if the whole line keeps that clear.
fn blocked(from: Vec3, direction: Vec3, distance: f32, colliders: &[Collider]) -> Option<f32> {
    (1..=(distance / SAMPLE) as u32)
        .map(|sample| sample as f32 * SAMPLE)
        .find(|&along| {
            let point = from + direction * along;
            colliders.iter().any(|collider| {
                point.y < collider.height + MARGIN
                    && collider.covers(Vec2::new(point.x, point.z), MARGIN)
            })
        })
        .map(|along| along - SAMPLE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::Shape;

    #[test]
    fn forward_moves_away_from_the_camera_and_right_to_its_right() {
        for direction in [Vec2::X, -Vec2::Y, Vec2::new(0.6, 0.8)] {
            let camera = FollowCamera::looking_along(direction);
            let (forward, right) = camera.ground_axes();
            assert!(
                forward.abs_diff_eq(direction, 1e-5),
                "{forward} for {direction}"
            );
            let view = camera.camera(Vec2::ZERO, &[]);
            let looking = (view.target - view.eye).normalize();
            // The camera looks down along forward, and its right on the ground is forward turned a
            // quarter turn clockwise seen from above.
            assert!(
                Vec2::new(looking.x, looking.z)
                    .normalize()
                    .abs_diff_eq(forward, 1e-5)
            );
            assert!(looking.y < 0.0);
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

    /// Whether any collider stands within the margin of the line from the camera to what it
    /// looks at.
    fn hidden(view: &Camera, colliders: &[Collider]) -> bool {
        let direction = (view.eye - view.target).normalize();
        let length = view.eye.distance(view.target);
        blocked(view.target, direction, length, colliders).is_some()
    }

    #[test]
    fn a_collider_behind_the_hero_never_stands_between_the_camera_and_it() {
        // Looking along −z, the camera stands at +z of the hero; a tall trunk and a high turned
        // wall stand there.
        let camera = FollowCamera::looking_along(-Vec2::Y);
        let colliders = [
            Collider {
                shape: Shape::Cylinder { radius: 0.6 },
                center: Vec2::new(0.0, 2.5),
                height: 8.0,
            },
            Collider {
                shape: Shape::Box {
                    half_size: Vec2::new(3.0, 0.3),
                    yaw: 0.3,
                },
                center: Vec2::new(4.0, 1.0),
                height: 4.0,
            },
        ];
        for hero in [
            Vec2::ZERO,
            Vec2::new(0.0, 1.5),
            Vec2::new(4.0, -0.5),
            Vec2::new(4.0, -0.1),
        ] {
            let view = camera.camera(hero, &colliders);
            let length = view.eye.distance(view.target);
            assert!(length < DISTANCE, "{hero}: the camera came nearer");
            assert!(!hidden(&view, &colliders), "{hero}: {view:?}");
        }
    }

    #[test]
    fn a_low_collider_below_the_line_of_sight_leaves_the_camera_where_it_is() {
        // A fallen block 0.9 m high, 2 m behind the hero: the line of sight passes above it.
        let camera = FollowCamera::looking_along(-Vec2::Y);
        let block = Collider {
            shape: Shape::Box {
                half_size: Vec2::new(1.0, 0.5),
                yaw: 0.8,
            },
            center: Vec2::new(0.0, 2.0),
            height: 0.9,
        };
        for colliders in [&[][..], &[block][..]] {
            let view = camera.camera(Vec2::ZERO, colliders);
            assert!((view.eye.distance(view.target) - DISTANCE).abs() < 1e-4);
        }
    }
}
