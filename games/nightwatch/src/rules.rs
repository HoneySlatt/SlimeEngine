//! The rules of the game, without the GPU: where the player is and how it moves through the
//! courtyard. The window, the input and the drawing are elsewhere, so that the rules can be tested
//! alone and played in scripted games.

use slime_render::glam::Vec2;

use crate::courtyard::{Block, Courtyard};

/// The player's radius, in metres: it moves as a circle on the ground.
pub const PLAYER_RADIUS: f32 = 0.35;
/// How fast the player moves, in metres per second.
pub const PLAYER_SPEED: f32 = 3.0;

/// A game in progress.
pub struct Game {
    courtyard: Courtyard,
    /// The player's place on the ground: x and z, in metres.
    player: Vec2,
}

impl Game {
    /// A new game in `courtyard`, the player at its entrance.
    pub fn new(courtyard: Courtyard) -> Self {
        let player = courtyard.entrance;
        Self { courtyard, player }
    }

    pub fn courtyard(&self) -> &Courtyard {
        &self.courtyard
    }

    /// The player's place on the ground: x and z, in metres.
    pub fn player(&self) -> Vec2 {
        self.player
    }

    /// Moves the player for `step` seconds towards `direction`, on the ground, at the share of its
    /// speed that the direction's length gives, at most 1. A block in the way stops the player,
    /// which slides along it.
    pub fn step(&mut self, direction: Vec2, step: f32) {
        let travel = direction.clamp_length_max(1.0) * PLAYER_SPEED * step;
        // In moves of at most half the radius, so that no move carries the player past a corner.
        let parts = (travel.length() / (PLAYER_RADIUS / 2.0)).ceil().max(1.0);
        let part = travel / parts;
        for _ in 0..parts as u32 {
            self.player = push_out(self.player + part, PLAYER_RADIUS, &self.courtyard.blocks);
        }
    }
}

/// `position` moved out of every block that the circle of `radius` around it overlaps: along the
/// line from the block's nearest point, which leaves the part of a move along the block's side, so
/// that the player slides.
fn push_out(mut position: Vec2, radius: f32, blocks: &[Block]) -> Vec2 {
    // Pushed out of one block, the circle can enter another, in a corner between two: a few rounds
    // settle it.
    for _ in 0..4 {
        let mut pushed = false;
        for block in blocks {
            let offset = position - block.center;
            let away = offset - offset.clamp(-block.half_size, block.half_size);
            let distance = away.length();
            if distance >= radius {
                continue;
            }
            pushed = true;
            if distance > 1e-6 {
                position += away * ((radius - distance) / distance);
            } else {
                // The center is inside the block: out through its nearest side.
                let depth = block.half_size - offset.abs();
                if depth.x < depth.y {
                    position.x = block.center.x + offset.x.signum() * (block.half_size.x + radius);
                } else {
                    position.y = block.center.y + offset.y.signum() * (block.half_size.y + radius);
                }
            }
        }
        if !pushed {
            break;
        }
    }
    position
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::courtyard::BlockKind;

    /// The fixed step the game runs at: 1/60 s.
    const STEP: f32 = 1.0 / 60.0;

    /// A courtyard of the given blocks, with the player starting at `entrance`.
    fn courtyard(entrance: [f32; 2], blocks: &[([f32; 2], [f32; 2])]) -> Courtyard {
        Courtyard {
            half_size: Vec2::splat(50.0),
            blocks: blocks
                .iter()
                .map(|&(center, size)| Block {
                    kind: BlockKind::Wall,
                    center: Vec2::from(center),
                    half_size: Vec2::from(size) / 2.0,
                    height: 2.0,
                })
                .collect(),
            entrance: Vec2::from(entrance),
            exit: Vec2::ZERO,
        }
    }

    /// Plays `seconds` of holding `direction`.
    fn hold(game: &mut Game, direction: Vec2, seconds: f32) {
        for _ in 0..(seconds / STEP).round() as u32 {
            game.step(direction, STEP);
        }
    }

    #[test]
    fn the_player_moves_at_its_speed_and_a_short_direction_slows_it() {
        let mut game = Game::new(courtyard([0.0, 0.0], &[]));
        hold(&mut game, Vec2::X, 1.0);
        assert!(
            (game.player().x - PLAYER_SPEED).abs() < 1e-4,
            "{}",
            game.player()
        );
        // A diagonal of length √2 moves at the same speed, a direction of length 0.5 at half.
        let mut game = Game::new(courtyard([0.0, 0.0], &[]));
        hold(&mut game, Vec2::ONE, 1.0);
        assert!((game.player().length() - PLAYER_SPEED).abs() < 1e-4);
        let mut game = Game::new(courtyard([0.0, 0.0], &[]));
        hold(&mut game, Vec2::new(0.0, 0.5), 1.0);
        assert!((game.player().y - PLAYER_SPEED / 2.0).abs() < 1e-4);
    }

    #[test]
    fn a_wall_stops_the_player_at_its_radius() {
        // A wall whose west side is at x = 2.
        let mut game = Game::new(courtyard([0.0, 0.0], &[([2.5, 0.0], [1.0, 10.0])]));
        hold(&mut game, Vec2::X, 3.0);
        assert!(
            (game.player().x - (2.0 - PLAYER_RADIUS)).abs() < 1e-4,
            "{}",
            game.player()
        );
    }

    #[test]
    fn the_player_slides_along_a_wall_it_moves_into_at_an_angle() {
        let mut game = Game::new(courtyard([0.0, 0.0], &[([2.5, 0.0], [1.0, 100.0])]));
        hold(&mut game, Vec2::new(1.0, 1.0).normalize(), 2.0);
        let player = game.player();
        assert!((player.x - (2.0 - PLAYER_RADIUS)).abs() < 1e-4, "{player}");
        // Along the wall at the share of the speed that the direction gives along it.
        let along = PLAYER_SPEED * 2.0 * std::f32::consts::FRAC_1_SQRT_2;
        assert!((player.y - along).abs() < 0.05, "{player}");
    }

    #[test]
    fn the_player_goes_round_a_corner_without_crossing_it() {
        // A block from (1, 1) to (3, 3); the player starts south-west of it and moves north-east,
        // straight at its corner, then along its sides.
        let block = ([2.0, 2.0], [2.0, 2.0]);
        for direction in [Vec2::ONE, Vec2::new(1.0, 0.9), Vec2::new(0.9, 1.0)] {
            let mut game = Game::new(courtyard([0.0, 0.0], &[block]));
            for _ in 0..240 {
                game.step(direction, STEP);
                let offset = (game.player() - Vec2::splat(2.0)).abs() - Vec2::ONE;
                let distance = offset.max(Vec2::ZERO).length();
                assert!(
                    distance > PLAYER_RADIUS - 1e-3,
                    "{direction}: {} overlaps the block",
                    game.player()
                );
            }
        }
    }

    #[test]
    fn the_player_does_not_pass_a_gap_narrower_than_itself() {
        // Two blocks whose facing sides are 0.5 apart, less than the player's diameter of 0.7.
        let blocks = [([-1.25, 2.0], [2.0, 1.0]), ([1.25, 2.0], [2.0, 1.0])];
        for start in [0.0, 0.1, -0.2] {
            let mut game = Game::new(courtyard([start, 0.0], &blocks));
            hold(&mut game, Vec2::Y, 3.0);
            assert!(game.player().y < 1.5, "from x = {start}: {}", game.player());
        }
        // A gap of 1 m lets it through.
        let blocks = [([-1.5, 2.0], [2.0, 1.0]), ([1.5, 2.0], [2.0, 1.0])];
        let mut game = Game::new(courtyard([0.0, 0.0], &blocks));
        hold(&mut game, Vec2::Y, 3.0);
        assert!(game.player().y > 3.0, "{}", game.player());
    }

    #[test]
    fn a_fast_move_does_not_cross_a_thin_wall() {
        // A step of a tenth of a second at full speed is 0.3 m, a wall 0.1 m thick.
        let mut game = Game::new(courtyard([0.0, 0.0], &[([1.0, 0.0], [0.1, 10.0])]));
        for _ in 0..30 {
            game.step(Vec2::X, 0.1);
        }
        assert!(game.player().x < 1.0, "{}", game.player());
    }

    #[test]
    fn the_player_stays_inside_the_nightwatch_courtyard() {
        let mut game = Game::new(Courtyard::nightwatch());
        let half_size = game.courtyard().half_size;
        for direction in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y, Vec2::ONE, -Vec2::ONE] {
            hold(&mut game, direction, 15.0);
            let inside = half_size - Vec2::splat(PLAYER_RADIUS - 1e-3);
            assert!(
                game.player().abs().cmple(inside).all(),
                "{direction}: {}",
                game.player()
            );
        }
    }
}
