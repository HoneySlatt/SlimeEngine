//! Fixed steps: what moved during the last one is drawn between its two transforms
//! (`docs/adr/0084-what-the-world-draws.md`).

use std::mem;

use crate::entity::Entity;
use crate::hierarchy::children_of;
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
    /// elsewhere does not slide there. Carried, if one of their ancestors moved during the step, by
    /// that ancestor ([`World::interpolated_transform`]). Outside a fixed step, at once. Nothing for
    /// an entity that is not alive.
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
            self.nodes[slot as usize].moved = 0;
            subtree.extend(children_of(&self.nodes, slot).map(|child| child.index()));
        }
    }

    /// Where `entity` is drawn at `fraction` of the time from the last fixed step to the next, from
    /// 0 to 1, as `slime-app`'s `Context::fixed_step_alpha` gives it
    /// (`docs/adr/0084-what-the-world-draws.md`):
    ///
    /// - If its transform in the world changed during the last step, between where it was before
    ///   the step and where it is now ([`GlobalTransform::lerp`]).
    /// - If it is drawn where it is now, as an entity changed outside a step, placed during one or
    ///   teleported is, but an ancestor's transform changed during the step: carried by the nearest
    ///   such ancestor, where it is now relative to it, so that it stays on it.
    /// - Otherwise, and for a world that runs no step, where it is now.
    ///
    /// A frame without a step keeps the last step's two transforms; a frame with several, the last
    /// one's. Writing a [`Transform`](crate::Transform) outside a step, even with the value it held,
    /// counts as a change; reaching one without writing it does not. An entity given another parent
    /// during a step is drawn from where it was to where it goes. A stretch, which `GlobalTransform`
    /// does not hold, is drawn as it is now. `None` if the entity is not alive or has no `Transform`.
    pub fn interpolated_transform(&self, entity: Entity, fraction: f64) -> Option<GlobalTransform> {
        let now = self.global_transform(entity)?;
        if self.changes.moved.is_empty() {
            // Nothing moved during the last step: no ancestor to look for.
            return Some(now);
        }
        let node = &self.nodes[entity.index() as usize];
        if node.moved != 0 {
            let (_, before) = self.changes.moved[node.moved as usize - 1];
            return Some(before.lerp(&now, fraction));
        }
        let mut ancestor = node.parent;
        while let Some(parent) = ancestor {
            let node = &self.nodes[parent.index() as usize];
            if node.moved != 0 {
                let (_, before) = self.changes.moved[node.moved as usize - 1];
                let parent_now = self.slot_global(parent);
                let Some(relative) = now.relative_to(&parent_now) else {
                    return Some(now);
                };
                return Some(before.lerp(&parent_now, fraction).compose(&relative));
            }
            ancestor = node.parent;
        }
        Some(now)
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

    /// An entity set where it already is during each step, as a game that sets its sun from the
    /// time of day does, is drawn exactly where it is at every fraction of a step: a sun that did
    /// not move keeps the sky it made.
    #[test]
    fn an_entity_set_where_it_is_during_a_step_is_drawn_exactly_there() {
        let mut world = World::new();
        let sun = world.spawn();
        let at = Transform {
            translation: DVec3::new(0.3, 7.1, -2.9),
            rotation: Quat::from_rotation_x(-0.9) * Quat::from_rotation_y(0.4),
            ..Transform::IDENTITY
        };
        world.insert(sun, at);
        world.update_transforms();
        let here = world.global_transform(sun).expect("placed");
        for frame in 0..24 {
            world.begin_fixed_step();
            *world.get_mut::<Transform>(sun).expect("placed") = at;
            world.end_fixed_step();
            world.update_transforms();
            let fraction = (f64::from(frame) * 60.0 / 144.0).fract();
            assert_eq!(
                world.interpolated_transform(sun, fraction),
                Some(here),
                "{fraction}"
            );
        }
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
    fn an_entity_placed_for_the_first_time_during_a_step_is_drawn_at_once_where_it_is() {
        let mut world = World::new();
        let cart = world.spawn();
        world.insert(cart, at(50.0));
        let arrow = world.spawn();
        world.set_parent(arrow, Some(cart)).expect("alive");
        let loose = world.spawn();
        world.update_transforms();
        world.begin_fixed_step();
        world.insert(arrow, at(100.0));
        world.insert(loose, at(7.0));
        world.end_fixed_step();
        assert_eq!(drawn(&world, arrow, 0.0), 150.0);
        assert_eq!(drawn(&world, loose, 0.0), 7.0);
        // Placed again during a step, after it lost its place: at once too.
        world.begin_fixed_step();
        world.remove::<Transform>(loose);
        world.insert(loose, at(9.0));
        world.end_fixed_step();
        assert_eq!(drawn(&world, loose, 0.0), 9.0);
    }

    #[test]
    fn what_is_drawn_at_once_stays_on_an_ancestor_that_moved() {
        let mut world = World::new();
        let cart = world.spawn();
        world.insert(cart, at(0.0));
        let lantern = world.spawn();
        world.insert(lantern, at(1.0));
        world.set_parent(lantern, Some(cart)).expect("alive");
        world.update_transforms();
        world.begin_fixed_step();
        world
            .get_mut::<Transform>(cart)
            .expect("placed")
            .translation
            .x = 10.0;
        // A sword drawn from the step on, in the cart.
        let sword = world.spawn();
        world.insert(sword, at(2.0));
        world.set_parent(sword, Some(cart)).expect("alive");
        world.end_fixed_step();
        // The lantern swings outside the step, as an animation at each frame does.
        world
            .get_mut::<Transform>(lantern)
            .expect("placed")
            .translation
            .y = 0.5;
        let cart_x = drawn(&world, cart, 0.5);
        assert!((cart_x - 5.0).abs() < 1e-12);
        assert!((drawn(&world, lantern, 0.5) - 6.0).abs() < 1e-12);
        let lantern_y = world
            .interpolated_transform(lantern, 0.5)
            .expect("placed")
            .translation
            .y;
        assert!((lantern_y - 0.5).abs() < 1e-12);
        assert!((drawn(&world, sword, 0.5) - 7.0).abs() < 1e-12);
        // Turned with it too.
        world.begin_fixed_step();
        world.get_mut::<Transform>(cart).expect("placed").rotation =
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        world.end_fixed_step();
        world
            .get_mut::<Transform>(lantern)
            .expect("placed")
            .translation
            .y = 0.0;
        let half = world
            .interpolated_transform(lantern, 0.5)
            .expect("placed")
            .translation;
        let expected = DVec3::new(10.0 + 0.5f64.sqrt(), 0.0, -(0.5f64.sqrt()));
        assert!((half - expected).length() < 1e-6, "{half}");
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
