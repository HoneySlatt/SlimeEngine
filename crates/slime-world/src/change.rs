//! What the world knows of its changes (`docs/adr/0108-what-the-world-knows-of-its-changes.md`):
//! the world's ticks, the ticks of each value of a component, the guard through which a value is
//! written, and the log of what left.

use std::collections::{HashMap, VecDeque, vec_deque};
use std::ops::{Deref, DerefMut};

use crate::components::Name;
use crate::entity::Entity;
use crate::world::{Changes, Node, mark_stale};
use crate::{Component, World};

/// A moment of a world's changes: a number that grows by one at each operation that may change
/// the world (`World::tick`). Every value of a component holds the tick at which it was added and
/// the one at which it last changed; a reader keeps the world's tick when it reads, and asks what
/// changed after it the next time. The default tick, zero, is before every change: a reader that
/// starts from it sees everything.
///
/// A tick is 64 bits wide: a world that changes a billion times a second takes centuries to run
/// out of them, so that no tick is ever given twice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub(crate) u64);

impl Tick {
    /// The tick after this one.
    pub(crate) fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// The ticks of a value of a component: when it was added to its entity, and when it last
/// changed, its addition included. A component that replaces another of its type keeps the tick
/// at which the first was added: the entity held one all along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticks {
    /// The tick at which the entity began to hold a component of this type.
    pub added: Tick,
    /// The tick at which the value was last written, or added.
    pub changed: Tick,
}

/// What a guard of a [`Transform`](crate::Transform) marks once written: the world's hierarchy,
/// in which the entity's subtree goes stale. Public in a private module: the queries' traits name
/// it, and nothing outside the crate can.
pub struct TransformMarks<'a> {
    pub(crate) nodes: &'a mut [Node],
    pub(crate) changes: &'a mut Changes,
    pub(crate) slot: u32,
}

/// A component reached to be changed: it reads as the value, and the first write through it
/// stamps the value with the tick at which it was reached, so that the world's readers learn that
/// it changed (`World::changed`). Reaching a value without writing it changes nothing.
///
/// Most code writes through it as through `&mut T`: `light.intensity = 2.0`, `*transform = at`,
/// and `&mut guard` gives a function that takes `&mut T` what it asks for. A match that binds
/// parts of the value to change them matches `&mut *guard`. Each of these stamps the value, even
/// if nothing in it changes: code that changes only some values reads first, through the guard,
/// then writes. [`Mut::set_if_differs`] writes only a value that differs from the one held, and
/// stamps it only then.
pub struct Mut<'a, T> {
    value: &'a mut T,
    /// The value's tick of change, its block's summary and its storage's.
    changed: &'a mut Tick,
    block: &'a mut Tick,
    storage: &'a mut Tick,
    tick: Tick,
    /// For a transform, what its first write marks; `None` once marked, or for another type.
    marks: Option<TransformMarks<'a>>,
    /// Where the value's base is kept before its first write after it; `None` once kept, or for
    /// a value of no base.
    keep: Option<Keep<'a, T>>,
}

/// Where a guard keeps the base of its value, its value when its entity was last settled
/// (`World::settle`), before the first write after it: what a save compares the value with
/// (`docs/adr/0125-saves-per-scene.md`).
pub(crate) struct Keep<'a, T> {
    /// The bases kept of the values of a storage, by slot: `None` for a value that the entity did
    /// not hold.
    pub(crate) kept: &'a mut HashMap<u32, Option<T>>,
    pub(crate) slot: u32,
    /// The tick at which the entity was settled: a value not changed since is its base.
    pub(crate) base: Tick,
}

impl<'a, T: Clone> Mut<'a, T> {
    pub(crate) fn new(
        value: &'a mut T,
        changed: &'a mut Tick,
        block: &'a mut Tick,
        storage: &'a mut Tick,
        tick: Tick,
        marks: Option<TransformMarks<'a>>,
        keep: Option<Keep<'a, T>>,
    ) -> Self {
        Self {
            value,
            changed,
            block,
            storage,
            tick,
            marks,
            keep,
        }
    }

    /// Stamps the value as changed, once, its base kept first if it is still its base.
    fn stamp(&mut self) {
        if *self.changed == self.tick {
            return;
        }
        if let Some(keep) = self.keep.take()
            && *self.changed <= keep.base
        {
            keep.kept
                .entry(keep.slot)
                .or_insert_with(|| Some(self.value.clone()));
        }
        *self.changed = self.tick;
        *self.block = self.tick;
        *self.storage = self.tick;
        if let Some(marks) = self.marks.take() {
            mark_stale(marks.nodes, marks.changes, marks.slot);
        }
    }

    /// Writes `value` in place of the value held if they differ, and only then stamps it as
    /// changed; returns whether it wrote it. What a game that sets a value at each frame, or a
    /// network that sends only what changed, writes through.
    pub fn set_if_differs(&mut self, value: T) -> bool
    where
        T: PartialEq,
    {
        if *self.value == value {
            return false;
        }
        self.stamp();
        *self.value = value;
        true
    }
}

impl<T> Deref for Mut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: Clone> DerefMut for Mut<'_, T> {
    /// The value, to write: stamps it as changed.
    fn deref_mut(&mut self) -> &mut T {
        self.stamp();
        self.value
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Mut<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

/// How many entries a log of what left holds before it forgets its older half: a reader that
/// reads less often than that many removals finds that the log no longer holds all of them, and
/// reads the whole world again. 16 bytes each, 1 MiB per log at most.
const LOGGED: usize = 65_536;

/// The entities that left, a storage's component or the world, with the ticks at which they
/// left, the oldest first.
#[derive(Debug)]
pub(crate) struct Log {
    entries: VecDeque<(Tick, Entity)>,
    /// The latest tick of an entry that the log forgot: a reader whose last read is before it may
    /// have missed some.
    forgot: Tick,
}

impl Log {
    /// A log that holds nothing, and forgot nothing.
    pub(crate) const EMPTY: Self = Self {
        entries: VecDeque::new(),
        forgot: Tick(0),
    };

    pub(crate) fn push(&mut self, tick: Tick, entity: Entity) {
        if self.entries.len() == LOGGED {
            let (newest_forgotten, _) = self.entries[LOGGED / 2 - 1];
            self.forgot = newest_forgotten;
            self.entries.drain(..LOGGED / 2);
        }
        self.entries.push_back((tick, entity));
    }

    /// The entries after `since`, oldest first, as entities and their ticks; `None` if the log
    /// forgot some of them.
    pub(crate) fn since(&self, since: Tick) -> Option<Removals<'_>> {
        if since < self.forgot {
            return None;
        }
        let first = self.entries.partition_point(|&(tick, _)| tick <= since);
        Some(Removals {
            entries: self.entries.range(first..),
        })
    }
}

/// What left after a tick, as entities and the ticks at which they left, oldest first
/// ([`World::removed`], [`World::destroyed`]).
pub struct Removals<'w> {
    entries: vec_deque::Iter<'w, (Tick, Entity)>,
}

impl Iterator for Removals<'_> {
    type Item = (Entity, Tick);

    fn next(&mut self) -> Option<(Entity, Tick)> {
        self.entries.next().map(|&(tick, entity)| (entity, tick))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

impl World {
    /// The world's tick now: what changes from now on is stamped with a later tick. A reader keeps
    /// it when it reads, and asks the next time for what changed after it ([`World::changed`],
    /// [`World::removed`], [`World::destroyed`], [`World::structure`]); each reader reads at its
    /// own pace, sharing nothing with the others.
    pub fn tick(&self) -> Tick {
        self.tick
    }

    /// The ticks of the entity's component of type `T`, if it is alive and holds one.
    pub fn ticks<T: Component>(&self, entity: Entity) -> Option<Ticks> {
        if !self.contains(entity) {
            return None;
        }
        self.storage::<T>()?.ticks(entity.index())
    }

    /// The entities whose component of type `T` was written or added after `since`, with the
    /// component and its ticks, in no particular order: what a reader that last read at `since`
    /// has not seen. A component added after `since` has its tick of addition after it.
    ///
    /// It costs a look at a summary per block and a read of the blocks that changed, not a read of
    /// every component: the components of a type are kept in blocks of 16, each with the latest
    /// tick at which one of its components changed, and the blocks that did not change after
    /// `since` are skipped unread. A component written through a
    /// guard counts as changed even if it was written with the value it held
    /// ([`Mut::set_if_differs`]).
    pub fn changed<T: Component>(
        &self,
        since: Tick,
    ) -> impl Iterator<Item = (Entity, &T, Ticks)> + '_ {
        self.storage::<T>()
            .into_iter()
            .flat_map(move |storage| storage.changed_since(since))
            .map(|(slot, value, ticks)| (self.entity_at(slot), value, ticks))
    }

    /// The entities that a component of type `T` left after `since`, by removal or because the
    /// entity was destroyed, with the ticks at which it did, oldest first. An entity listed may
    /// hold a `T` again, given since; a destroyed one is no longer alive.
    ///
    /// `None` if the world no longer holds all of them: its log of removals keeps the latest
    /// 65 536 of each type, then forgets the older half, and a reader that read before what it
    /// forgot reads the whole world again.
    pub fn removed<T: Component>(&self, since: Tick) -> Option<Removals<'_>> {
        match self.storage::<T>() {
            Some(storage) => storage.removed_since(since),
            None => {
                static NOTHING: Log = Log::EMPTY;
                NOTHING.since(since)
            }
        }
    }

    /// The entities destroyed after `since`, with the ticks at which they were, oldest first; as
    /// [`World::removed`], `None` if the world no longer holds all of them.
    pub fn destroyed(&self, since: Tick) -> Option<Removals<'_>> {
        self.destroyed.since(since)
    }

    /// The latest tick at which an entity was spawned or destroyed, or changed parent: a list of
    /// the world's entities, as an editor shows it, is made again only when it is after the tick of
    /// the list. A rename does not move it, but [`World::names`]
    /// (`docs/adr/0167-the-hierarchy.md`).
    pub fn structure(&self) -> Tick {
        self.structure
    }

    /// The latest tick at which an entity was given, changed or lost its [`Name`]: a list of the
    /// world's entities reads again the names of those whose `Name` changed since
    /// ([`World::changed`], [`World::removed`]) only when it is after the tick of its names
    /// (`docs/adr/0167-the-hierarchy.md`).
    pub fn names(&self) -> Tick {
        self.storage::<Name>()
            .map_or(Tick::default(), |names| names.changed())
    }
}

#[cfg(test)]
mod tests {
    use slime_color::LinearColor;

    use super::*;
    use crate::entity::Entities;
    use crate::{Hidden, Light};

    #[test]
    fn a_log_forgets_its_older_half_and_says_so_to_the_readers_that_needed_it() {
        let mut entities = Entities::default();
        let entity = entities.spawn();
        let mut log = Log::EMPTY;
        for tick in 1..=LOGGED as u64 + 1 {
            log.push(Tick(tick), entity);
        }
        assert!(log.since(Tick(0)).is_none(), "forgot the first half");
        let half = LOGGED as u64 / 2;
        assert!(log.since(Tick(half - 1)).is_none());
        let kept: Vec<_> = log.since(Tick(half)).expect("all kept").collect();
        assert_eq!(kept.len(), LOGGED / 2 + 1);
        assert_eq!(kept[0], (entity, Tick(half + 1)));
        assert_eq!(log.since(Tick(LOGGED as u64 + 1)).expect("kept").count(), 0);
    }

    /// A world of `count` entities that place a point light each, of intensity their index.
    fn lights(count: usize) -> (World, Vec<Entity>) {
        let mut world = World::new();
        let entities = (0..count)
            .map(|index| {
                let entity = world.spawn();
                world.insert(entity, point(index as f32));
                entity
            })
            .collect();
        (world, entities)
    }

    fn point(intensity: f32) -> Light {
        Light::Point {
            color: LinearColor::WHITE,
            intensity,
            range: None,
        }
    }

    /// The entities whose light changed after `since`, in their order.
    fn changed(world: &World, since: Tick) -> Vec<Entity> {
        let mut changed: Vec<_> = world
            .changed::<Light>(since)
            .map(|(entity, _, _)| entity)
            .collect();
        changed.sort_unstable();
        changed
    }

    #[test]
    fn each_of_two_readers_sees_a_value_written_through_a_guard_once_at_its_own_pace() {
        let (mut world, entities) = lights(200);
        let (mut fast, mut slow) = (world.tick(), world.tick());
        *world.get_mut::<Light>(entities[3]).expect("held") = point(-1.0);
        assert_eq!(changed(&world, fast), [entities[3]]);
        fast = world.tick();
        world.query_mut::<&mut Light>(|entity, mut light| {
            if entity == entities[150] {
                *light = point(-2.0);
            }
        });
        // Reached through the query, the other lights were not written: they did not change.
        assert_eq!(changed(&world, fast), [entities[150]]);
        fast = world.tick();
        assert_eq!(changed(&world, fast), [], "seen once");
        assert_eq!(changed(&world, slow), [entities[3], entities[150]]);
        slow = world.tick();
        assert_eq!(changed(&world, slow), []);
        let ticks = world.ticks::<Light>(entities[3]).expect("held");
        assert!(ticks.added < ticks.changed, "{ticks:?}");
    }

    #[test]
    fn a_value_set_equal_through_the_variant_is_not_stamped() {
        let (mut world, entities) = lights(2);
        let read = world.tick();
        let mut light = world.get_mut::<Light>(entities[1]).expect("held");
        assert!(!light.set_if_differs(point(1.0)), "the same");
        assert_eq!(changed(&world, read), []);
        let mut light = world.get_mut::<Light>(entities[1]).expect("held");
        assert!(light.set_if_differs(point(5.0)));
        assert_eq!(changed(&world, read), [entities[1]]);
    }

    #[test]
    fn removed_components_and_destroyed_entities_are_in_the_log_with_their_ticks() {
        let (mut world, entities) = lights(3);
        let child = world.spawn();
        world.insert(child, point(9.0));
        world
            .set_parent(child, Some(entities[2]))
            .expect("both alive");
        let read = world.tick();
        world.remove::<Light>(entities[0]);
        let removed_at = world.tick();
        world.despawn(entities[2]);
        let destroyed_at = world.tick();
        let removed: Vec<_> = world.removed::<Light>(read).expect("logged").collect();
        assert_eq!(
            removed,
            [
                (entities[0], removed_at),
                (child, destroyed_at),
                (entities[2], destroyed_at)
            ]
        );
        let destroyed: Vec<_> = world.destroyed(read).expect("logged").collect();
        assert_eq!(
            destroyed,
            [(child, destroyed_at), (entities[2], destroyed_at)]
        );
        assert_eq!(world.destroyed(destroyed_at).expect("logged").count(), 0);
        assert_eq!(
            world.removed::<Name>(read).expect("logged").count(),
            0,
            "none held"
        );
    }

    #[test]
    fn the_structure_moves_when_entities_come_go_or_move_and_the_names_when_they_are_renamed() {
        let (mut world, entities) = lights(2);
        let (mut structure, mut names) = (world.structure(), world.names());
        // Which of the two ticks moved since the last call.
        let mut moved = |world: &World| {
            let now = (world.structure(), world.names());
            let moved = (now.0 > structure, now.1 > names);
            (structure, names) = now;
            moved
        };
        world.insert(entities[0], point(3.0));
        *world.get_mut::<Light>(entities[1]).expect("held") = point(4.0);
        world.remove::<Light>(entities[1]);
        assert_eq!(moved(&world), (false, false), "components other than names");
        let child = world.spawn();
        assert_eq!(moved(&world), (true, false), "spawned");
        world
            .set_parent(child, Some(entities[0]))
            .expect("both alive");
        assert_eq!(moved(&world), (true, false), "a new parent");
        world
            .set_parent(child, Some(entities[0]))
            .expect("both alive");
        assert_eq!(moved(&world), (false, false), "the parent it had");
        world.insert(child, Name("lamp".to_owned()));
        assert_eq!(moved(&world), (false, true), "named");
        world.get_mut::<Name>(child).expect("named").0.push('s');
        assert_eq!(moved(&world), (false, true), "renamed");
        assert!(
            !world
                .get_mut::<Name>(child)
                .expect("named")
                .set_if_differs(Name("lamps".to_owned()))
        );
        assert_eq!(moved(&world), (false, false), "renamed as it was");
        world.remove::<Name>(child);
        assert_eq!(moved(&world), (false, true), "a name lost");
        world.despawn(entities[0]);
        assert!(moved(&world).0, "destroyed");
    }

    #[test]
    fn the_hidden_flag_follows_hidden_and_changes_of_parent() {
        let mut world = World::new();
        let [root, child, grandchild, other] = std::array::from_fn(|_| world.spawn());
        world.set_parent(child, Some(root)).expect("alive");
        world.set_parent(grandchild, Some(child)).expect("alive");
        let hidden = |world: &World| [root, child, grandchild, other].map(|e| world.is_hidden(e));
        assert_eq!(hidden(&world), [false; 4]);
        world.insert(root, Hidden);
        assert_eq!(hidden(&world), [true, true, true, false]);
        world.insert(grandchild, Hidden);
        world.remove::<Hidden>(root);
        assert_eq!(
            hidden(&world),
            [false, false, true, false],
            "hidden by itself"
        );
        world.insert(other, Hidden);
        world.set_parent(child, Some(other)).expect("alive");
        assert_eq!(hidden(&world), [false, true, true, true]);
        world.set_parent(child, None).expect("alive");
        assert_eq!(hidden(&world), [false, false, true, true]);
        world.remove_component_value(grandchild, "Hidden");
        assert_eq!(hidden(&world), [false, false, false, true]);
        world
            .insert_component_value(child, "Hidden", slime_reflect::Value::Struct(Vec::new()))
            .expect("a unit");
        assert_eq!(hidden(&world), [false, true, true, true]);
        // The next entity may take the slot of a hidden one destroyed: it starts unhidden.
        world.despawn(other);
        let next = world.spawn();
        assert!(!world.is_hidden(next));
    }
}
