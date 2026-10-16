//! Fixed steps: what moved during the last one is drawn between its two transforms
//! (`docs/adr/0084-what-the-world-draws.md`).

use std::mem;

use crate::entity::Entity;
use crate::transform::GlobalTransform;
use crate::world::World;

impl World {
    /// Starts a fixed step: the transforms in the world that change from now until
    /// [`World::end_fixed_step`] are drawn between where they were before the step and where it
    /// leaves them ([`World::interpolated_transform`]). It computes the transforms in the world
    /// first, so that what changed since the last step is drawn at once, and forgets the transforms
    /// kept from the step before. `slime-app` calls it before each `App::fixed_update`.
    pub fn begin_fixed_step(&mut self) {
        self.update_transforms();
        for &(slot, _) in &self.changes.moved {
            self.nodes[slot as usize].moved = 0;
        }
        self.changes.moved.clear();
        self.changes.in_step = true;
    }

    /// Ends the fixed step that [`World::begin_fixed_step`] started, and computes the transforms in
    /// the world: what changed during the step is drawn between its two transforms, but what was
    /// teleported. `slime-app` calls it after each `App::fixed_update`.
    pub fn end_fixed_step(&mut self) {
        self.changes.in_step = false;
        self.update_transforms();
        for entity in mem::take(&mut self.changes.teleported) {
            self.drawn_where_it_is(entity);
        }
    }

    /// Makes `entity` and its descendants be drawn where they are at the end of the fixed step going
    /// on, rather than between where they were and where they went: a hero that starts again
    /// elsewhere does not slide there. Outside a fixed step, at once. Nothing for an entity that is
    /// not alive.
    pub fn teleport(&mut self, entity: Entity) {
        if self.changes.in_step {
            self.changes.teleported.push(entity);
        } else {
            self.drawn_where_it_is(entity);
        }
    }

    /// Forgets the transforms before the last fixed step of `entity` and its descendants.
    fn drawn_where_it_is(&mut self, entity: Entity) {
        if !self.contains(entity) {
            return;
        }
        let mut subtree = vec![entity.index()];
        while let Some(slot) = subtree.pop() {
            let node = &mut self.nodes[slot as usize];
            node.moved = 0;
            subtree.extend(node.children.iter().map(|child| child.index()));
        }
    }

    /// Where `entity` is drawn at `fraction` of the time from the last fixed step to the next, from
    /// 0 to 1, as `slime-app`'s `Context::fixed_step_alpha` gives it: between its transform in the
    /// world before the last step and its transform now, if that changed during the step
    /// ([`GlobalTransform::lerp`]); its transform now otherwise, and for a world that runs no step.
    /// A frame without a step keeps the last step's two transforms; a frame with several, the last
    /// one's. `None` if the entity is not alive or has no [`Transform`](crate::Transform).
    pub fn interpolated_transform(&self, entity: Entity, fraction: f64) -> Option<GlobalTransform> {
        let now = self.global_transform(entity)?;
        let moved = self.nodes[entity.index() as usize].moved;
        if moved == 0 {
            return Some(now);
        }
        let (_, before) = self.changes.moved[moved as usize - 1];
        Some(before.lerp(&now, fraction))
    }
}

#[cfg(test)]
mod tests {
    use glam::{DVec3, Quat};

    use crate::{Transform, World};

    fn at(x: f64) -> Transform {
        Transform::from_translation(DVec3::new(x, 0.0, 0.0))
    }

    /// Where `entity` is drawn along x at `fraction` of the step.
    fn drawn(world: &World, entity: crate::Entity, fraction: f64) -> f64 {
        world
            .interpolated_transform(entity, fraction)
            .expect("placed")
            .translation
            .x
    }

    /// Moves `entity` to `x` during a fixed step.
    fn step_to(world: &mut World, entity: crate::Entity, x: f64) {
        world.begin_fixed_step();
        world
            .get_mut::<Transform>(entity)
            .expect("placed")
            .translation
            .x = x;
        world.end_fixed_step();
    }

    #[test]
    fn what_moved_during_a_step_is_drawn_between_its_two_transforms() {
        let mut world = World::new();
        let hero = world.spawn();
        world.insert(hero, at(0.0));
        let rock = world.spawn();
        world.insert(rock, at(5.0));
        step_to(&mut world, hero, 10.0);
        for (fraction, x) in [(0.0, 0.0), (0.25, 2.5), (1.0, 10.0)] {
            assert!(
                (drawn(&world, hero, fraction) - x).abs() < 1e-12,
                "{fraction}"
            );
        }
        assert_eq!(drawn(&world, rock, 0.5), 5.0, "the rock did not move");
    }

    #[test]
    fn a_frame_without_a_step_keeps_the_last_two_and_one_with_two_takes_the_last() {
        let mut world = World::new();
        let hero = world.spawn();
        world.insert(hero, at(0.0));
        step_to(&mut world, hero, 10.0);
        // A frame without a step: further along the same two.
        assert!((drawn(&world, hero, 0.75) - 7.5).abs() < 1e-12);
        // A frame with two steps.
        step_to(&mut world, hero, 20.0);
        step_to(&mut world, hero, 30.0);
        assert!((drawn(&world, hero, 0.0) - 20.0).abs() < 1e-12);
        assert!((drawn(&world, hero, 0.5) - 25.0).abs() < 1e-12);
        // A step that moves nothing: drawn where it is.
        world.begin_fixed_step();
        world.end_fixed_step();
        assert_eq!(drawn(&world, hero, 0.0), 30.0);
    }

    #[test]
    fn a_change_made_outside_a_step_is_drawn_at_once() {
        let mut world = World::new();
        let hero = world.spawn();
        world.insert(hero, at(0.0));
        step_to(&mut world, hero, 10.0);
        world
            .get_mut::<Transform>(hero)
            .expect("placed")
            .translation
            .x = 50.0;
        assert_eq!(drawn(&world, hero, 0.0), 50.0);
        world.update_transforms();
        assert_eq!(drawn(&world, hero, 0.0), 50.0);
    }

    #[test]
    fn a_teleport_does_not_slide_and_takes_the_entity_s_children_with_it() {
        let mut world = World::new();
        let hero = world.spawn();
        world.insert(hero, at(0.0));
        let sword = world.spawn();
        world.insert(sword, at(1.0));
        world.set_parent(sword, Some(hero)).expect("alive");
        world.begin_fixed_step();
        world
            .get_mut::<Transform>(hero)
            .expect("placed")
            .translation
            .x = 100.0;
        world.teleport(hero);
        world.end_fixed_step();
        assert_eq!(drawn(&world, hero, 0.0), 100.0);
        assert_eq!(drawn(&world, sword, 0.0), 101.0);
        // Outside a step, at once.
        step_to(&mut world, hero, 200.0);
        world.teleport(hero);
        assert_eq!(drawn(&world, sword, 0.0), 201.0);
    }

    #[test]
    fn the_children_of_what_moved_are_drawn_between_their_two_transforms() {
        let mut world = World::new();
        let cart = world.spawn();
        world.insert(cart, at(0.0));
        let lantern = world.spawn();
        world.insert(lantern, at(1.0));
        world.set_parent(lantern, Some(cart)).expect("alive");
        step_to(&mut world, cart, 10.0);
        assert!((drawn(&world, lantern, 0.5) - 6.0).abs() < 1e-12);
    }

    #[test]
    fn an_entity_spawned_during_a_step_is_drawn_at_once_where_it_is() {
        let mut world = World::new();
        world.begin_fixed_step();
        let arrow = world.spawn();
        world.insert(arrow, at(7.0));
        world.end_fixed_step();
        assert_eq!(drawn(&world, arrow, 0.0), 7.0);
    }

    #[test]
    fn rotations_turn_along_the_shortest_arc() {
        let mut world = World::new();
        let door = world.spawn();
        world.insert(door, Transform::IDENTITY);
        world.begin_fixed_step();
        world.get_mut::<Transform>(door).expect("placed").rotation =
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        world.end_fixed_step();
        let half = world
            .interpolated_transform(door, 0.5)
            .expect("placed")
            .rotation;
        assert!(half.angle_between(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)) < 1e-6);
    }

    #[test]
    fn a_world_that_runs_no_step_draws_its_transforms_as_they_are() {
        let mut world = World::new();
        let hero = world.spawn();
        world.insert(hero, at(0.0));
        world
            .get_mut::<Transform>(hero)
            .expect("placed")
            .translation
            .x = 3.0;
        assert_eq!(drawn(&world, hero, 0.5), 3.0);
        let unplaced = world.spawn();
        assert_eq!(world.interpolated_transform(unplaced, 0.5), None);
    }
}
