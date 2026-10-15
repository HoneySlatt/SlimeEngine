//! The place the game is played in: its ground, the colliders that stop the hero, and where the
//! hero starts. Positions are x and z in metres, on the ground, at y = 0, +y up; north is −z and
//! east +x; an angle about the vertical is in radians from +x towards +z.
//!
//! Until the zone is made from its sources and cooked (Stage 4a, M4a.6), the game is played on a
//! test ground built here, with the two kinds of collider that the zone will have.

use slime_render::glam::Vec2;

/// The footprint of a collider on the ground, around its center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle of half the size `half_size` along its own axes, turned by `yaw` about the
    /// vertical: its first axis lies along `Vec2::from_angle(yaw)`.
    Box { half_size: Vec2, yaw: f32 },
    /// A circle.
    Cylinder { radius: f32 },
}

/// What stops the hero: a box turned about the vertical or a cylinder, standing on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Collider {
    pub shape: Shape,
    /// The center of its footprint.
    pub center: Vec2,
    /// How high it stands, in metres.
    pub height: f32,
}

impl Collider {
    fn turned_box(center: [f32; 2], size: [f32; 2], yaw: f32, height: f32) -> Self {
        Self {
            shape: Shape::Box {
                half_size: Vec2::from(size) / 2.0,
                yaw,
            },
            center: Vec2::from(center),
            height,
        }
    }

    fn cylinder(center: [f32; 2], radius: f32, height: f32) -> Self {
        Self {
            shape: Shape::Cylinder { radius },
            center: Vec2::from(center),
            height,
        }
    }

    /// Where a circle of `radius` around `position` must move to overlap the footprint no more,
    /// if it overlaps it: away from the footprint's nearest point, which keeps the part of a move
    /// along the footprint's side, so that what moves slides along it. A center inside a box leaves
    /// through the nearest side.
    pub fn push_out(&self, position: Vec2, radius: f32) -> Option<Vec2> {
        match self.shape {
            Shape::Box { half_size, yaw } => {
                // In the box's own axes, where its sides lie along x and y.
                let local = Vec2::from_angle(-yaw).rotate(position - self.center);
                let away = local - local.clamp(-half_size, half_size);
                let distance = away.length();
                if distance >= radius {
                    return None;
                }
                let out = if distance > 1e-6 {
                    local + away * ((radius - distance) / distance)
                } else {
                    let depth = half_size - local.abs();
                    if depth.x < depth.y {
                        Vec2::new(local.x.signum() * (half_size.x + radius), local.y)
                    } else {
                        Vec2::new(local.x, local.y.signum() * (half_size.y + radius))
                    }
                };
                Some(self.center + Vec2::from_angle(yaw).rotate(out))
            }
            Shape::Cylinder { radius: own_radius } => {
                let offset = position - self.center;
                let distance = offset.length();
                let reach = own_radius + radius;
                if distance >= reach {
                    None
                } else if distance > 1e-6 {
                    Some(self.center + offset * (reach / distance))
                } else {
                    Some(self.center + Vec2::X * reach)
                }
            }
        }
    }

    /// Whether `point`, on the ground, lies within `margin` of the footprint, or inside it.
    pub fn covers(&self, point: Vec2, margin: f32) -> bool {
        match self.shape {
            Shape::Box { half_size, yaw } => {
                let local = Vec2::from_angle(-yaw).rotate(point - self.center);
                local.abs().cmple(half_size + margin).all()
            }
            Shape::Cylinder { radius } => point.distance(self.center) <= radius + margin,
        }
    }
}

/// A zone: its ground, its colliders, and where the hero starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    /// Half the size of the ground along x and z, around the origin; the hero does not leave it.
    pub half_size: Vec2,
    pub colliders: Vec<Collider>,
    /// Where the hero starts.
    pub start: Vec2,
    /// Which way the hero and the camera look at the start, of length 1.
    pub facing: Vec2,
}

impl Zone {
    /// The test ground: 40 m square, with the pieces of a ruined wall and a fallen block turned at
    /// angles, a colonnade of four pillars, and the trunks of a few trees. The hero starts in the
    /// south, looking north, at the ruins.
    pub fn test_ground() -> Self {
        let wall = Collider::turned_box;
        let colliders = vec![
            // The ruined wall: three pieces of different heights, turned off its line.
            wall([-6.0, -4.0], [4.0, 0.6], 0.15, 2.2),
            wall([-1.5, -5.0], [3.0, 0.6], -0.3, 1.2),
            wall([5.0, -3.5], [5.0, 0.7], 0.5, 3.0),
            // A block fallen from it, lower than the camera.
            wall([3.0, 4.0], [1.6, 1.2], 0.8, 0.9),
            // The colonnade, beyond the wall.
            Collider::cylinder([-3.0, -9.0], 0.45, 4.0),
            Collider::cylinder([3.0, -9.0], 0.45, 4.0),
            Collider::cylinder([-3.0, -14.0], 0.45, 4.0),
            Collider::cylinder([3.0, -14.0], 0.45, 4.0),
            // Trees.
            Collider::cylinder([-9.0, 6.0], 0.6, 8.0),
            Collider::cylinder([8.0, 9.0], 0.7, 9.0),
            Collider::cylinder([-12.0, -8.0], 0.8, 10.0),
            Collider::cylinder([11.0, -12.0], 0.6, 8.0),
            Collider::cylinder([-5.0, 14.0], 0.5, 7.0),
        ];
        Self {
            half_size: Vec2::splat(20.0),
            colliders,
            start: Vec2::new(0.0, 12.0),
            facing: Vec2::NEG_Y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_is_on_the_ground_and_clear_of_every_collider() {
        let zone = Zone::test_ground();
        assert!(zone.start.abs().cmplt(zone.half_size - 1.0).all());
        for collider in &zone.colliders {
            assert!(!collider.covers(zone.start, 1.0), "{collider:?}");
        }
        assert!((zone.facing.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn every_collider_stands_on_the_ground() {
        let zone = Zone::test_ground();
        for collider in &zone.colliders {
            assert!(
                collider.center.abs().cmplt(zone.half_size - 2.0).all(),
                "{collider:?}"
            );
            assert!(collider.height > 0.0);
        }
    }

    #[test]
    fn a_turned_box_covers_its_turned_footprint() {
        // 4 m long along its first axis, turned a quarter turn: it lies along z.
        let collider =
            Collider::turned_box([0.0, 0.0], [4.0, 1.0], std::f32::consts::FRAC_PI_2, 1.0);
        assert!(collider.covers(Vec2::new(0.0, 1.9), 0.0));
        assert!(!collider.covers(Vec2::new(1.9, 0.0), 0.0));
        assert!(collider.covers(Vec2::new(0.7, 0.0), 0.25));
        assert!(!collider.covers(Vec2::new(0.8, 0.0), 0.25));
    }
}
