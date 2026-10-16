//! The hierarchy: each entity's parent and children, and where each entity is in the world.

use crate::components::Hidden;
use crate::entity::Entity;
use crate::error::WorldError;
use crate::transform::{GlobalTransform, Transform};
use crate::world::{Node, World, mark_stale, storage_in};

impl World {
    /// Makes `parent` the parent of `child`, its last child, or puts `child` at the root for
    /// `None`. The child keeps its [`Transform`], now relative to its new parent, so that where it
    /// is in the world changes with it.
    ///
    /// # Errors
    ///
    /// [`WorldError::NotAlive`] for an entity that is not alive, and [`WorldError::Cycle`] if
    /// `parent` is `child` or one of its descendants.
    pub fn set_parent(&mut self, child: Entity, parent: Option<Entity>) -> Result<(), WorldError> {
        for entity in [Some(child), parent].into_iter().flatten() {
            if !self.contains(entity) {
                return Err(WorldError::NotAlive(entity));
            }
        }
        if let Some(parent) = parent {
            let mut ancestor = Some(parent);
            while let Some(entity) = ancestor {
                if entity == child {
                    return Err(WorldError::Cycle { child, parent });
                }
                ancestor = self.nodes[entity.index() as usize].parent;
            }
        }
        if let Some(old) = self.nodes[child.index() as usize].parent.take() {
            self.nodes[old.index() as usize]
                .children
                .retain(|&sibling| sibling != child);
        }
        if let Some(parent) = parent {
            self.nodes[parent.index() as usize].children.push(child);
            self.nodes[child.index() as usize].parent = Some(parent);
        }
        if self.nodes[child.index() as usize].stale {
            // Stale already, perhaps through its old parent, whose subtree no longer holds it.
            self.changes.stale.push(child.index());
        } else {
            mark_stale(&mut self.nodes, &mut self.changes, child.index());
        }
        Ok(())
    }

    /// The entity's parent, if it is alive and has one.
    pub fn parent(&self, entity: Entity) -> Option<Entity> {
        if !self.contains(entity) {
            return None;
        }
        self.nodes[entity.index() as usize].parent
    }

    /// The entity's children, in the order they became its children; none if it is not alive.
    pub fn children(&self, entity: Entity) -> &[Entity] {
        if !self.contains(entity) {
            return &[];
        }
        &self.nodes[entity.index() as usize].children
    }

    /// Whether the entity, or one of its ancestors, holds [`Hidden`]: then it is not drawn, nor
    /// is the light it places (`docs/adr/0084-what-the-world-draws.md`). `false` if it is not
    /// alive.
    pub fn is_hidden(&self, entity: Entity) -> bool {
        let mut next = self.contains(entity).then_some(entity);
        while let Some(entity) = next {
            if self.get::<Hidden>(entity).is_some() {
                return true;
            }
            next = self.nodes[entity.index() as usize].parent;
        }
        false
    }

    /// Where the entity is in the world: its [`Transform`] composed with its ancestors', an
    /// ancestor without one counting as none. `None` if the entity is not alive or has no
    /// `Transform`.
    ///
    /// Always current: when a transform changed since [`World::update_transforms`] last ran, it is
    /// computed from the nearest ancestor that is current, a step per stale ancestor.
    pub fn global_transform(&self, entity: Entity) -> Option<GlobalTransform> {
        self.get::<Transform>(entity)?;
        Some(self.placement(entity))
    }

    /// Where the entity's slot is in the world, an absent transform counting as none.
    pub(crate) fn placement(&self, entity: Entity) -> GlobalTransform {
        let mut stale = Vec::new();
        let mut current = Some(entity);
        let mut base = GlobalTransform::IDENTITY;
        while let Some(entity) = current {
            let node = &self.nodes[entity.index() as usize];
            if !node.stale {
                base = node.global;
                break;
            }
            stale.push(entity);
            current = node.parent;
        }
        let transforms = self.storage::<Transform>();
        stale.iter().rev().fold(base, |parent, entity| {
            match transforms.and_then(|transforms| transforms.get(entity.index())) {
                Some(transform) => parent.mul_transform(transform),
                None => parent,
            }
        })
    }

    /// Computes again where each entity whose transform, or an ancestor's, changed is in the world,
    /// each once, from the top of each stale subtree down. Reading a transform in the world is
    /// current without it; with it, reading costs no more than a lookup, until the next change.
    ///
    /// It also empties the world's list of the subtrees that changed, which grows with each change
    /// until then: a world whose transforms change calls it once in a while, even if it only reads
    /// them one at a time.
    pub fn update_transforms(&mut self) {
        let Self {
            storages,
            component_ids,
            nodes,
            changes,
            ..
        } = self;
        let transforms = storage_in::<Transform>(component_ids, storages);
        let mut computed = 0;
        let mut next = Vec::new();
        for slot in changes.stale.drain(..) {
            if !nodes[slot as usize].stale {
                continue;
            }
            // The top of the stale subtree that holds it: a stale node's descendants are stale.
            let mut top = slot;
            while let Some(parent) = nodes[top as usize].parent {
                if !nodes[parent.index() as usize].stale {
                    break;
                }
                top = parent.index();
            }
            let base = match nodes[top as usize].parent {
                Some(parent) => nodes[parent.index() as usize].global,
                None => GlobalTransform::IDENTITY,
            };
            next.push((top, base));
            while let Some((slot, parent)) = next.pop() {
                let global = match transforms.and_then(|transforms| transforms.get(slot)) {
                    Some(transform) => parent.mul_transform(transform),
                    None => parent,
                };
                let node: &mut Node = &mut nodes[slot as usize];
                node.global = global;
                node.stale = false;
                computed += 1;
                next.extend(node.children.iter().map(|child| (child.index(), global)));
            }
        }
        #[cfg(test)]
        {
            self.computed += computed;
        }
        let _ = computed;
    }
}

#[cfg(test)]
mod tests {
    use glam::{DVec3, Quat};

    use super::*;

    fn at(x: f64) -> Transform {
        Transform::from_translation(DVec3::new(x, 0.0, 0.0))
    }

    /// A root at x = 1 with two children, the first with a child of its own, each 1 m further.
    fn family(world: &mut World) -> [Entity; 4] {
        let root = world.spawn();
        let first = world.spawn();
        let second = world.spawn();
        let grandchild = world.spawn();
        for (entity, x) in [(root, 1.0), (first, 1.0), (second, 2.0), (grandchild, 1.0)] {
            world.insert(entity, at(x));
        }
        world.set_parent(first, Some(root)).expect("alive");
        world.set_parent(second, Some(root)).expect("alive");
        world.set_parent(grandchild, Some(first)).expect("alive");
        [root, first, second, grandchild]
    }

    fn x_of(world: &World, entity: Entity) -> f64 {
        world
            .global_transform(entity)
            .expect("placed")
            .translation
            .x
    }

    #[test]
    fn children_keep_their_order_and_follow_their_parent() {
        let mut world = World::new();
        let [root, first, second, grandchild] = family(&mut world);
        assert_eq!(world.children(root), [first, second]);
        assert_eq!(world.parent(grandchild), Some(first));
        assert_eq!(x_of(&world, grandchild), 3.0);
        world.set_parent(grandchild, Some(second)).expect("alive");
        assert_eq!(world.children(first), []);
        assert_eq!(x_of(&world, grandchild), 4.0);
        world.set_parent(grandchild, None).expect("alive");
        assert_eq!(world.parent(grandchild), None);
        assert_eq!(x_of(&world, grandchild), 1.0);
    }

    #[test]
    fn a_hidden_ancestor_hides_its_descendants() {
        let mut world = World::new();
        let [root, first, _, grandchild] = family(&mut world);
        assert!(!world.is_hidden(grandchild));
        world.insert(first, Hidden);
        assert!(world.is_hidden(first) && world.is_hidden(grandchild));
        assert!(!world.is_hidden(root));
        world.remove::<Hidden>(first);
        assert!(!world.is_hidden(grandchild));
        world.insert(first, Hidden);
        world.despawn(root);
        assert!(!world.is_hidden(grandchild), "a dead entity is not hidden");
    }

    #[test]
    fn an_entity_cannot_become_its_own_ancestor() {
        let mut world = World::new();
        let [root, first, _, grandchild] = family(&mut world);
        assert_eq!(
            world.set_parent(root, Some(grandchild)),
            Err(WorldError::Cycle {
                child: root,
                parent: grandchild
            })
        );
        assert_eq!(
            world.set_parent(first, Some(first)),
            Err(WorldError::Cycle {
                child: first,
                parent: first
            })
        );
        assert_eq!(world.children(root).len(), 2, "unchanged");
    }

    #[test]
    fn a_destroyed_entity_takes_its_subtree_with_it() {
        let mut world = World::new();
        let [root, first, second, grandchild] = family(&mut world);
        assert!(world.despawn(first));
        assert!(!world.contains(grandchild));
        assert_eq!(world.children(root), [second]);
        assert_eq!(world.len(), 2);
        assert_eq!(
            world.set_parent(second, Some(first)),
            Err(WorldError::NotAlive(first))
        );
    }

    #[test]
    fn transforms_in_the_world_are_current_before_and_after_an_update() {
        let mut world = World::new();
        let [root, _, second, grandchild] = family(&mut world);
        world.update_transforms();
        world
            .get_mut::<Transform>(root)
            .expect("placed")
            .translation
            .x = 10.0;
        assert_eq!(x_of(&world, grandchild), 12.0, "current before the update");
        world.update_transforms();
        assert_eq!(x_of(&world, grandchild), 12.0);
        world.query_mut::<&mut Transform>(|_, transform| transform.rotation = Quat::IDENTITY);
        world.insert(second, at(5.0));
        assert_eq!(x_of(&world, second), 15.0);
        world.remove::<Transform>(root);
        assert_eq!(
            x_of(&world, second),
            5.0,
            "an ancestor without a transform counts as none"
        );
        assert_eq!(world.global_transform(root), None);
    }

    #[test]
    fn only_the_subtrees_that_changed_are_computed_again() {
        let mut world = World::new();
        let [root, first, second, _] = family(&mut world);
        world.update_transforms();
        world.computed = 0;
        world
            .get_mut::<Transform>(first)
            .expect("placed")
            .translation
            .x = 2.0;
        world.update_transforms();
        assert_eq!(world.computed, 2, "the first child and its child");
        world.computed = 0;
        world.update_transforms();
        assert_eq!(world.computed, 0, "nothing changed");
        world.get_mut::<Transform>(second).expect("placed");
        world.get_mut::<Transform>(root).expect("placed");
        world.update_transforms();
        assert_eq!(world.computed, 4, "the whole family, each once");
    }

    #[test]
    fn a_stale_entity_given_a_new_parent_is_computed_again() {
        let mut world = World::new();
        let [root, _, _, grandchild] = family(&mut world);
        world.update_transforms();
        // The root's family goes stale through the root, which no longer holds the grandchild.
        world
            .get_mut::<Transform>(root)
            .expect("placed")
            .translation
            .x = 10.0;
        world.set_parent(grandchild, None).expect("alive");
        world.computed = 0;
        world.update_transforms();
        assert_eq!(world.computed, 4, "the root's three, and the grandchild");
        assert!(world.nodes.iter().all(|node| !node.stale));
        assert_eq!(x_of(&world, grandchild), 1.0);
    }

    #[test]
    fn reading_transforms_through_a_query_marks_nothing() {
        let mut world = World::new();
        family(&mut world);
        world.update_transforms();
        world.computed = 0;
        world.query_mut::<&Transform>(|_, _| {});
        world.update_transforms();
        assert_eq!(world.computed, 0);
    }

    #[test]
    fn a_child_1_mm_from_a_turned_parent_10_000_km_away_stays_1_mm_from_it() {
        let mut world = World::new();
        let parent = world.spawn();
        world.insert(
            parent,
            Transform {
                translation: DVec3::new(1.0e7, 0.0, -1.0e7),
                rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                ..Transform::IDENTITY
            },
        );
        let child = world.spawn();
        world.insert(child, at(0.001));
        world.set_parent(child, Some(parent)).expect("alive");
        for update in [false, true] {
            if update {
                world.update_transforms();
            }
            let place = |entity| world.global_transform(entity).expect("placed").translation;
            // Turned a quarter about +Y, +X becomes −Z: 1 mm away, within 10 nm.
            let offset = place(child) - place(parent);
            assert!((offset - DVec3::new(0.0, 0.0, -0.001)).length() < 1e-8);
        }
    }

    #[test]
    fn a_component_that_panics_as_it_is_dropped_leaves_the_hierarchy_whole() {
        struct Fragile;
        impl crate::Component for Fragile {}
        impl Drop for Fragile {
            fn drop(&mut self) {
                panic!("dropped");
            }
        }
        let mut world = World::new();
        let [root, first, second, grandchild] = family(&mut world);
        world.insert(first, Fragile);
        let despawned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.despawn(root);
        }));
        assert!(despawned.is_err());
        // The second child and the grandchild went before the first child, which stays.
        assert!(!world.contains(second) && !world.contains(grandchild));
        assert!(world.contains(root) && world.contains(first));
        assert_eq!(world.children(root), [first]);
        assert_eq!(world.children(first), []);
        let other = world.spawn();
        assert_eq!(world.parent(other), None);
        assert_eq!(
            world.children(root),
            [first],
            "an entity in a freed slot is no one's child"
        );
    }
}

#[cfg(test)]
mod costs {
    use std::time::{Duration, Instant};

    use glam::DVec3;

    use super::*;

    /// Prints what computing the transforms in the world of 100 000 entities costs: 1 000 roots of
    /// 99 children each, then 1 % of them moved. Run it in a release build:
    /// `cargo test --release -p slime-world --lib -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, to run in a release build"]
    fn what_computing_100_000_transforms_costs() {
        let best = |work: &mut dyn FnMut() -> Duration| (0..5).map(|_| work()).min().unwrap();
        let family = || {
            let mut world = World::new();
            let roots: Vec<Entity> = (0..1_000)
                .map(|root| {
                    let entity = world.spawn();
                    world.insert(
                        entity,
                        Transform::from_translation(DVec3::new(root as f64, 0.0, 0.0)),
                    );
                    for child in 0..99 {
                        let child_entity = world.spawn();
                        world.insert(
                            child_entity,
                            Transform::from_translation(DVec3::new(0.0, child as f64, 0.0)),
                        );
                        world.set_parent(child_entity, Some(entity)).expect("alive");
                    }
                    entity
                })
                .collect();
            (world, roots)
        };
        let all = best(&mut || {
            let (mut world, _) = family();
            let started = Instant::now();
            world.update_transforms();
            started.elapsed()
        });
        eprintln!("compute the transforms of 100 000 entities: {all:?}");
        let (mut world, roots) = family();
        world.update_transforms();
        let moved = best(&mut || {
            for &root in &roots[..10] {
                world
                    .get_mut::<Transform>(root)
                    .expect("placed")
                    .translation
                    .z += 1.0;
            }
            let started = Instant::now();
            world.update_transforms();
            started.elapsed()
        });
        eprintln!("compute again the 1 000 whose root moved: {moved:?}");
        let entities: Vec<Entity> = roots
            .iter()
            .flat_map(|&root| world.children(root).to_vec())
            .collect();
        let read = best(&mut || {
            let started = Instant::now();
            let sum: f64 = entities
                .iter()
                .map(|&entity| {
                    world
                        .global_transform(entity)
                        .expect("placed")
                        .translation
                        .y
                })
                .sum();
            std::hint::black_box(sum);
            started.elapsed()
        });
        eprintln!("read the transforms in the world of 99 000 entities, current: {read:?}");
    }
}
