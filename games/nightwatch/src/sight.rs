//! What a light shows: whether a point of the ground lies inside a cone of light, within its range,
//! with nothing standing in between. The rules decide with this alone; the engine's spot lights
//! draw the cones, and the shadows of what stands in between (ADR-0058).

use slime_render::glam::Vec2;

use crate::courtyard::Block;

/// A cone of light on the ground, seen from above.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cone {
    /// Where the light stands.
    pub origin: Vec2,
    /// Where it points, of length 1.
    pub direction: Vec2,
    /// The angle from its axis to its edge, in radians.
    pub half_angle: f32,
    /// How far it reaches, in metres.
    pub range: f32,
}

impl Cone {
    /// Whether `point` lies inside the cone, and no block stands between the light and it.
    pub fn sees(&self, point: Vec2, blocks: &[Block]) -> bool {
        let offset = point - self.origin;
        let distance = offset.length();
        if distance > self.range {
            return false;
        }
        // Right at the light, the point is seen whichever way it points.
        let inside =
            distance < 1e-3 || offset.dot(self.direction) >= distance * self.half_angle.cos();
        inside && clear_line(self.origin, point, blocks)
    }
}

/// Whether the segment from `from` to `to` crosses no block, the blocks that hold `from` aside:
/// a light on a pole sees out of it.
pub fn clear_line(from: Vec2, to: Vec2, blocks: &[Block]) -> bool {
    let delta = to - from;
    blocks.iter().all(|block| {
        let low = block.center - block.half_size;
        let high = block.center + block.half_size;
        if from.cmpge(low).all() && from.cmple(high).all() {
            return true;
        }
        // The part of the segment inside the block's slab along each axis (Kay and Kajiya's slabs).
        let (mut enter, mut leave) = (0.0_f32, 1.0_f32);
        for axis in 0..2 {
            if delta[axis].abs() < 1e-9 {
                if from[axis] < low[axis] || from[axis] > high[axis] {
                    return true;
                }
                continue;
            }
            let (a, b) = (
                (low[axis] - from[axis]) / delta[axis],
                (high[axis] - from[axis]) / delta[axis],
            );
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
        }
        enter > leave
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::courtyard::BlockKind;

    fn crate_at(center: [f32; 2]) -> Block {
        Block {
            kind: BlockKind::Crate,
            center: Vec2::from(center),
            half_size: Vec2::splat(0.5),
            height: 1.0,
        }
    }

    /// A cone from the origin along +x, 30° to each side, reaching 5 m.
    const CONE: Cone = Cone {
        origin: Vec2::ZERO,
        direction: Vec2::X,
        half_angle: std::f32::consts::FRAC_PI_6,
        range: 5.0,
    };

    #[test]
    fn a_cone_sees_inside_its_angle_and_range_only() {
        assert!(CONE.sees(Vec2::new(3.0, 0.0), &[]));
        assert!(CONE.sees(Vec2::new(3.0, 1.6), &[]), "within 30°");
        assert!(!CONE.sees(Vec2::new(3.0, 1.8), &[]), "beyond 30°");
        assert!(!CONE.sees(Vec2::new(5.1, 0.0), &[]), "beyond its range");
        assert!(!CONE.sees(Vec2::new(-1.0, 0.0), &[]), "behind it");
    }

    #[test]
    fn a_block_between_hides_and_one_beside_does_not() {
        assert!(!CONE.sees(Vec2::new(4.0, 0.0), &[crate_at([2.0, 0.0])]));
        assert!(CONE.sees(Vec2::new(4.0, 0.0), &[crate_at([2.0, 1.5])]));
        // Just past a corner of the block, the line clears it.
        assert!(CONE.sees(Vec2::new(4.0, 1.6), &[crate_at([2.0, 0.0])]));
        // A block behind the point does not hide it.
        assert!(CONE.sees(Vec2::new(2.0, 0.0), &[crate_at([4.0, 0.0])]));
    }

    #[test]
    fn a_light_sees_out_of_the_block_it_stands_in() {
        let pole = Block {
            half_size: Vec2::splat(0.2),
            ..crate_at([0.0, 0.0])
        };
        assert!(CONE.sees(Vec2::new(3.0, 0.0), &[pole]));
    }
}
