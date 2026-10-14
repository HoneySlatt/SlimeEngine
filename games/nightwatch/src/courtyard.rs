//! The courtyard the game takes place in: its blocks (walls, crates, the gate), where the player
//! enters and where the exit is, on the ground plane. Positions are x and z in metres, the ground
//! at y = 0, +y up.

use slime_render::glam::Vec2;

/// What a block is: the game draws each kind its own way, and all of them stop the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// A wall of the courtyard or inside it.
    Wall,
    /// A crate, low enough to see over.
    Crate,
    /// The gate in the east wall, which the player leaves through.
    Gate,
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

/// A courtyard: its blocks, where the player starts and where it leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Courtyard {
    /// Half the size of the ground inside the outer walls, along x and z.
    pub half_size: Vec2,
    pub blocks: Vec<Block>,
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

impl Courtyard {
    /// Nightwatch's courtyard: 24 m from west to east and 16 m from north to south, closed by
    /// walls, with inner walls and crates to hide behind. The player enters at the west end and
    /// leaves through the gate in the middle of the east wall.
    pub fn nightwatch() -> Self {
        use BlockKind::{Crate, Gate, Wall};
        let half_size = Vec2::new(12.0, 8.0);
        let (x, z) = (
            half_size.x + WALL_THICKNESS / 2.0,
            half_size.y + WALL_THICKNESS / 2.0,
        );
        let length = 2.0 * x + WALL_THICKNESS;
        // The east wall, on each side of the gate.
        let side = half_size.y - GATE_WIDTH / 2.0;
        let side_center = GATE_WIDTH / 2.0 + side / 2.0;
        let blocks = vec![
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
            // Inner walls, which split the courtyard into three parts.
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
        Self {
            half_size,
            blocks,
            entrance: Vec2::new(-10.5, 0.0),
            exit: Vec2::new(half_size.x - 0.8, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entrance_and_the_exit_are_inside_the_courtyard_and_clear_of_every_block() {
        let courtyard = Courtyard::nightwatch();
        for place in [courtyard.entrance, courtyard.exit] {
            assert!(
                place.abs().cmplt(courtyard.half_size).all(),
                "{place} is inside"
            );
            for block in &courtyard.blocks {
                let gap = ((place - block.center).abs() - block.half_size).max_element();
                assert!(gap > 0.5, "{place} is clear of {block:?}");
            }
        }
    }
}
