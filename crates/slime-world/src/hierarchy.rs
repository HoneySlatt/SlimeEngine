//! The hierarchy: each entity's parent and children, and where each entity is in the world.

use std::iter::FusedIterator;

use crate::components::Hidden;
use crate::entity::Entity;
use crate::error::WorldError;
use crate::transform::{GlobalTransform, Transform};
use crate::world::{Node, World, mark_stale, storage_in};

/// The children of an entity, in the order they became its children: what [`World::children`]
/// gives. It reads them from either end, and knows how many are left.
#[derive(Clone, Debug)]
pub struct Children<'w> {
    nodes: &'w [Node],
    front: Option<Entity>,
    back: Option<Entity>,
    left: usize,
}

impl Iterator for Children<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Entity> {
        if self.left == 0 {
            return None;
        }
        let child = self.front?;
        self.left -= 1;
        self.front = self.nodes[child.index() as usize].next_sibling;
        Some(child)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl DoubleEndedIterator for Children<'_> {
    fn next_back(&mut self) -> Option<Entity> {
        if self.left == 0 {
            return None;
        }
        let child = self.back?;
        self.left -= 1;
        self.back = self.nodes[child.index() as usize].previous_sibling;
        Some(child)
    }
}

impl ExactSizeIterator for Children<'_> {}

impl FusedIterator for Children<'_> {}

/// The children of the slot's entity.
pub(crate) fn children_of(nodes: &[Node], slot: u32) -> Children<'_> {
    let node = &nodes[slot as usize];
    Children {
        nodes,
        front: node.first_child,
        back: node.last_child,
        left: node.child_count as usize,
    }
}

/// Makes `child`, which has no parent, the last child of `parent`.
fn join_parent(nodes: &mut [Node], child: Entity, parent: Entity) {
    let last = nodes[parent.index() as usize].last_child;
    let node = &mut nodes[child.index() as usize];
    node.parent = Some(parent);
    node.previous_sibling = last;
    node.next_sibling = None;
    match last {
        Some(last) => nodes[last.index() as usize].next_sibling = Some(child),
        None => nodes[parent.index() as usize].first_child = Some(child),
    }
    let parent = &mut nodes[parent.index() as usize];
    parent.last_child = Some(child);
    parent.child_count += 1;
}

/// Takes `child` out of its parent's children, its siblings linked to each other in its place,
/// and puts it at the root; nothing if it has no parent.
pub(crate) fn leave_parent(nodes: &mut [Node], child: Entity) {
    let node = &mut nodes[child.index() as usize];
    let Some(parent) = node.parent.take() else {
        return;
    };
    let (previous, next) = (node.previous_sibling.take(), node.next_sibling.take());
    match previous {
        Some(previous) => nodes[previous.index() as usize].next_sibling = next,
        None => nodes[parent.index() as usize].first_child = next,
    }
    match next {
        Some(next) => nodes[next.index() as usize].previous_sibling = previous,
        None => nodes[parent.index() as usize].last_child = previous,
    }
    nodes[parent.index() as usize].child_count -= 1;
}

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
        leave_parent(&mut self.nodes, child);
        if let Some(parent) = parent {
            join_parent(&mut self.nodes, child, parent);
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
    pub fn children(&self, entity: Entity) -> Children<'_> {
        if !self.contains(entity) {
            return Children {
                nodes: &self.nodes,
                front: None,
                back: None,
                left: 0,
            };
        }
        children_of(&self.nodes, entity.index())
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
        Some(self.slot_global(entity))
    }

    /// Where the entity's slot is in the world, an absent transform counting as none.
    pub(crate) fn slot_global(&self, entity: Entity) -> GlobalTransform {
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
            // Each node of the subtree is read once: it gives its first child, and its next
            // sibling but for the top's, which share its parent but not its subtree.
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
                if let Some(sibling) = node.next_sibling
                    && slot != top
                {
                    next.push((sibling.index(), parent));
                }
                if let Some(child) = node.first_child {
                    next.push((child.index(), global));
                }
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
        assert_eq!(world.children(root).collect::<Vec<_>>(), [first, second]);
        assert_eq!(world.parent(grandchild), Some(first));
        assert_eq!(x_of(&world, grandchild), 3.0);
        world.set_parent(grandchild, Some(second)).expect("alive");
        assert_eq!(world.children(first).len(), 0);
        assert_eq!(x_of(&world, grandchild), 4.0);
        world.set_parent(grandchild, None).expect("alive");
        assert_eq!(world.parent(grandchild), None);
        assert_eq!(x_of(&world, grandchild), 1.0);
    }

    /// Children that leave their parent from its first, its last and its middle, by a new parent,
    /// by none or destroyed, leave the others in their order, read from either end.
    #[test]
    fn children_leave_from_anywhere_and_the_others_keep_their_order() {
        let mut world = World::new();
        let parent = world.spawn();
        let other = world.spawn();
        let children: Vec<Entity> = (0..6)
            .map(|_| {
                let child = world.spawn();
                world.set_parent(child, Some(parent)).expect("alive");
                child
            })
            .collect();
        world.set_parent(children[0], None).expect("alive");
        world.set_parent(children[5], Some(other)).expect("alive");
        assert!(world.despawn(children[2]));
        let left = [children[1], children[3], children[4]];
        assert_eq!(world.children(parent).collect::<Vec<_>>(), left);
        assert_eq!(
            world.children(parent).rev().collect::<Vec<_>>(),
            [children[4], children[3], children[1]]
        );
        assert_eq!(world.children(parent).len(), 3);
        let mut both_ends = world.children(parent);
        assert_eq!(
            (both_ends.next(), both_ends.next_back(), both_ends.next()),
            (Some(children[1]), Some(children[4]), Some(children[3]))
        );
        assert_eq!((both_ends.next(), both_ends.next_back()), (None, None));
        world.set_parent(children[0], Some(parent)).expect("alive");
        assert_eq!(
            world.children(parent).collect::<Vec<_>>(),
            [children[1], children[3], children[4], children[0]],
            "a child that comes back is the last"
        );
        assert_eq!(world.children(other).collect::<Vec<_>>(), [children[5]]);
        for child in world.children(parent).collect::<Vec<_>>() {
            world.set_parent(child, None).expect("alive");
        }
        assert_eq!(world.children(parent).len(), 0);
        assert_eq!(world.children(parent).next_back(), None);
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
        assert_eq!(world.children(root).collect::<Vec<_>>(), [second]);
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
        #[derive(slime_reflect::Reflect)]
        #[reflect(name = "test::Fragile")]
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
        assert_eq!(world.children(root).collect::<Vec<_>>(), [first]);
        assert_eq!(world.children(first).len(), 0);
        let other = world.spawn();
        assert_eq!(world.parent(other), None);
        assert_eq!(
            world.children(root).collect::<Vec<_>>(),
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
            .flat_map(|&root| world.children(root).collect::<Vec<_>>())
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
