//! The courtyard the game takes place in: its blocks (walls, crates, the gate, poles, a switch),
//! the guards' rounds, the floodlights, where the player enters and where it leaves. Positions are
//! x and z in metres, on the ground, at y = 0, +y up; north is −z and east +x.

use slime_render::glam::Vec2;

/// What a block is: the game draws each kind its own way, and all of them stop the player and
/// hide it from the lights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// A wall of the courtyard or inside it.
    Wall,
    /// A crate, 1 m high: the player, lower, hides behind it.
    Crate,
    /// The gate in the east wall, which the player leaves through.
    Gate,
    /// The pole a floodlight stands on.
    Pole,
    /// The post of a switch.
    Switch,
}

/// A box standing on the ground, its sides along x and z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub kind: BlockKind,
    /// The center of its footprint.
    pub center: Vec2,
    /// Half its footprint along x and z.
    pub half_size: Vec2,
    /// How high it stands, in metres.
    pub height: f32,
}

impl Block {
    fn new(kind: BlockKind, center: [f32; 2], size: [f32; 2], height: f32) -> Self {
        Self {
            kind,
            center: Vec2::from(center),
            half_size: Vec2::from(size) / 2.0,
            height,
        }
    }
}

/// A guard's round: the points it walks to, in order, and from the last back to the first.
#[derive(Clone, Debug, PartialEq)]
pub struct Patrol {
    pub waypoints: Vec<Vec2>,
}

/// A floodlight on a pole: where it stands, and where it points, of length 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Floodlight {
    pub position: Vec2,
    pub direction: Vec2,
}

/// A switch that turns a floodlight off: where it stands, and the index of its floodlight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Switch {
    pub position: Vec2,
    pub floodlight: usize,
}

/// A courtyard: its blocks, its guards' rounds, its floodlights and switches, where the player
/// starts and where it leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Courtyard {
    /// Half the size of the ground inside the outer walls, along x and z.
    pub half_size: Vec2,
    pub blocks: Vec<Block>,
    pub patrols: Vec<Patrol>,
    pub floodlights: Vec<Floodlight>,
    pub switches: Vec<Switch>,
    /// Where the player starts.
    pub entrance: Vec2,
    /// Where the player leaves: in front of the gate.
    pub exit: Vec2,
}

/// The height of the outer walls, and their thickness.
const WALL_HEIGHT: f32 = 2.5;
const WALL_THICKNESS: f32 = 0.4;
/// The width of the gate in the east wall.
const GATE_WIDTH: f32 = 2.0;
/// The side of a floodlight's pole, and its height.
const POLE_SIDE: f32 = 0.3;
pub const POLE_HEIGHT: f32 = 3.5;
/// The side of a switch's post, and its height.
const SWITCH_SIDE: f32 = 0.3;
pub const SWITCH_HEIGHT: f32 = 1.1;

impl Courtyard {
    /// Nightwatch's courtyard: 24 m from west to east and 16 m from north to south, closed by
    /// walls. Two inner walls split it into three parts, joined by an opening at the south end of
    /// the first and at the north end of the second, so that the player zigzags from the entrance,
    /// at the west end, to the gate, in the middle of the east wall. A guard walks a round in each
    /// part; a floodlight lights the middle part from the north wall, and another the way to the
    /// gate, which a switch on the north wall of the east part turns off.
    pub fn nightwatch() -> Self {
        use BlockKind::{Crate, Gate, Pole, Switch as Post, Wall};
        let half_size = Vec2::new(12.0, 8.0);
        let (x, z) = (
            half_size.x + WALL_THICKNESS / 2.0,
            half_size.y + WALL_THICKNESS / 2.0,
        );
        let length = 2.0 * x + WALL_THICKNESS;
        // The east wall, on each side of the gate.
        let side = half_size.y - GATE_WIDTH / 2.0;
        let side_center = GATE_WIDTH / 2.0 + side / 2.0;
        let floodlights = vec![
            // On the north wall of the middle part, lighting it to the south.
            Floodlight {
                position: Vec2::new(0.0, -7.6),
                direction: Vec2::Y,
            },
            // On the east wall, north of the gate, lighting the way to it.
            Floodlight {
                position: Vec2::new(11.6, -3.5),
                direction: Vec2::new(-1.0, 0.8).normalize(),
            },
        ];
        let switches = vec![Switch {
            position: Vec2::new(8.5, -7.3),
            floodlight: 1,
        }];
        let mut blocks = vec![
            Block::new(Wall, [0.0, -z], [length, WALL_THICKNESS], WALL_HEIGHT),
            Block::new(Wall, [0.0, z], [length, WALL_THICKNESS], WALL_HEIGHT),
            Block::new(
                Wall,
                [-x, 0.0],
                [WALL_THICKNESS, 2.0 * half_size.y],
                WALL_HEIGHT,
            ),
            Block::new(Wall, [x, -side_center], [WALL_THICKNESS, side], WALL_HEIGHT),
            Block::new(Wall, [x, side_center], [WALL_THICKNESS, side], WALL_HEIGHT),
            Block::new(Gate, [x, 0.0], [WALL_THICKNESS, GATE_WIDTH], WALL_HEIGHT),
            // Inner walls: open at the south end of the first, the north end of the second.
            Block::new(Wall, [-4.0, -3.5], [0.4, 9.0], 2.0),
            Block::new(Wall, [4.0, 3.5], [0.4, 9.0], 2.0),
            // Crates.
            Block::new(Crate, [-8.5, -4.0], [1.0, 1.0], 1.0),
            Block::new(Crate, [-7.0, 3.5], [2.0, 1.0], 1.0),
            Block::new(Crate, [-1.5, 1.5], [1.0, 1.0], 1.0),
            Block::new(Crate, [0.5, -5.0], [1.0, 2.0], 1.0),
            Block::new(Crate, [1.0, 5.5], [1.0, 1.0], 1.0),
            Block::new(Crate, [7.0, -2.0], [2.0, 1.0], 1.0),
            Block::new(Crate, [8.5, 4.5], [1.0, 1.0], 1.0),
        ];
        for floodlight in &floodlights {
            blocks.push(Block::new(
                Pole,
                floodlight.position.into(),
                [POLE_SIDE; 2],
                POLE_HEIGHT,
            ));
        }
        for switch in &switches {
            blocks.push(Block::new(
                Post,
                switch.position.into(),
                [SWITCH_SIDE; 2],
                SWITCH_HEIGHT,
            ));
        }
        let round = |points: &[[f32; 2]]| Patrol {
            waypoints: points.iter().copied().map(Vec2::from).collect(),
        };
        Self {
            half_size,
            blocks,
            patrols: vec![
                // Along the west part, between the inner wall and the crates.
                round(&[[-5.5, -6.0], [-5.5, 6.5]]),
                // Round the middle part.
                round(&[[-2.5, 4.0], [2.5, 4.0], [2.5, -2.5], [-2.5, -2.5]]),
                // Round the east part.
                round(&[[5.5, -6.0], [10.0, -6.0], [10.0, 2.5], [5.5, 2.5]]),
            ],
            floodlights,
            switches,
            entrance: Vec2::new(-10.5, 0.0),
            exit: Vec2::new(half_size.x - 0.8, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How far `point` is outside `block`'s footprint, 0 inside.
    fn gap(point: Vec2, block: &Block) -> f32 {
        ((point - block.center).abs() - block.half_size)
            .max(Vec2::ZERO)
            .length()
    }

    #[test]
    fn the_entrance_and_the_exit_are_inside_the_courtyard_and_clear_of_every_block() {
        let courtyard = Courtyard::nightwatch();
        for place in [courtyard.entrance, courtyard.exit] {
            assert!(
                place.abs().cmplt(courtyard.half_size).all(),
                "{place} is inside"
            );
            for block in &courtyard.blocks {
                assert!(gap(place, block) > 0.5, "{place} is clear of {block:?}");
            }
        }
    }

    #[test]
    fn the_guards_rounds_keep_clear_of_every_block() {
        let courtyard = Courtyard::nightwatch();
        for patrol in &courtyard.patrols {
            let points = &patrol.waypoints;
            for (index, &from) in points.iter().enumerate() {
                let to = points[(index + 1) % points.len()];
                for step in 0..=100 {
                    let point = from.lerp(to, step as f32 / 100.0);
                    for block in &courtyard.blocks {
                        assert!(
                            gap(point, block) > 0.4,
                            "{point} of {patrol:?} is clear of {block:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn floodlights_stand_on_their_poles_and_switches_name_one() {
        let courtyard = Courtyard::nightwatch();
        for floodlight in &courtyard.floodlights {
            assert!((floodlight.direction.length() - 1.0).abs() < 1e-6);
            assert!(
                courtyard
                    .blocks
                    .iter()
                    .any(|block| block.kind == BlockKind::Pole
                        && block.center == floodlight.position)
            );
        }
        for switch in &courtyard.switches {
            assert!(switch.floodlight < courtyard.floodlights.len());
        }
    }
}
