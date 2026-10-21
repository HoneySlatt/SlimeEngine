//! The camera, behind the hero and a little above, looking where the player looks ([`Look`]),
//! which the arrow keys turn.

use slime_render::Camera;
use slime_render::glam::{Vec2, Vec3};

use crate::look::Look;
use crate::zone::Footprint;

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

/// The camera looking at `hero`, on the ground, from behind it, along `look`: when a collider
/// stands between them, it comes nearer, in front of the collider, as near as it must, down to the
/// hero's shoulders, so that it never stands inside a collider.
pub fn follow(look: Look, hero: Vec2, colliders: &[Footprint]) -> Camera {
    let target = Vec3::new(hero.x, LOOK_HEIGHT, hero.y);
    let (forward, _) = look.ground_axes();
    let (sin, cos) = PITCH.sin_cos();
    let toward = Vec3::new(-forward.x * cos, sin, -forward.y * cos);
    let distance =
        blocked(target, toward, DISTANCE, colliders).map_or(DISTANCE, |clear| clear.max(SAMPLE));
    Camera {
        eye: (target + toward * distance).as_dvec3(),
        target: target.as_dvec3(),
        ..Camera::default()
    }
}

/// How far from `from`, along `direction`, of length 1, a point can stand within `distance` with
/// no collider nearer to it than the margin: `None` if the whole line keeps that clear.
fn blocked(from: Vec3, direction: Vec3, distance: f32, colliders: &[Footprint]) -> Option<f32> {
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
    fn the_camera_looks_down_along_where_the_player_looks() {
        for direction in [Vec2::X, -Vec2::Y, Vec2::new(0.6, 0.8)] {
            let view = follow(Look::along(direction), Vec2::ZERO, &[]);
            let looking = (view.target - view.eye).as_vec3().normalize();
            assert!(
                Vec2::new(looking.x, looking.z)
                    .normalize()
                    .abs_diff_eq(direction, 1e-5)
            );
            assert!(looking.y < 0.0);
        }
    }

    /// Whether any collider stands within the margin of the line from the camera to what it
    /// looks at.
    fn hidden(view: &Camera, colliders: &[Footprint]) -> bool {
        let direction = (view.eye - view.target).as_vec3().normalize();
        let length = view.eye.distance(view.target) as f32;
        blocked(view.target.as_vec3(), direction, length, colliders).is_some()
    }

    #[test]
    fn a_collider_behind_the_hero_never_stands_between_the_camera_and_it() {
        // Looking along −z, the camera stands at +z of the hero; a tall trunk and a high turned
        // wall stand there.
        let look = Look::along(-Vec2::Y);
        let colliders = [
            Footprint {
                shape: Shape::Cylinder { radius: 0.6 },
                center: Vec2::new(0.0, 2.5),
                height: 8.0,
            },
            Footprint {
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
            let view = follow(look, hero, &colliders);
            let length = view.eye.distance(view.target) as f32;
            assert!(length < DISTANCE, "{hero}: the camera came nearer");
            assert!(!hidden(&view, &colliders), "{hero}: {view:?}");
        }
    }

    #[test]
    fn a_low_collider_below_the_line_of_sight_leaves_the_camera_where_it_is() {
        // A fallen block 0.9 m high, 2 m behind the hero: the line of sight passes above it.
        let look = Look::along(-Vec2::Y);
        let block = Footprint {
            shape: Shape::Box {
                half_size: Vec2::new(1.0, 0.5),
                yaw: 0.8,
            },
            center: Vec2::new(0.0, 2.0),
            height: 0.9,
        };
        for colliders in [&[][..], &[block][..]] {
            let view = follow(look, Vec2::ZERO, colliders);
            assert!((view.eye.distance(view.target) as f32 - DISTANCE).abs() < 1e-4);
        }
    }
}
