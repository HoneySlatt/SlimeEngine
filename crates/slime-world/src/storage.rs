//! The storage of one component type: a sparse set, whose values sit side by side in a dense
//! array, with an index from each slot to its value, and the ticks of each value
//! (`crate::change`).

use std::any::Any;
use std::mem;

use slime_reflect::{Reflect, ReflectError, Value};

use crate::change::{Log, Mut, Removals, Tick, Ticks, TransformMarks};
use crate::entity::Entity;
use crate::world::Component;

/// A sparse entry that holds no value.
const EMPTY: u32 = u32::MAX;

/// How many values, side by side in the dense array, share a summary of their changes: a reader
/// of what changed skips the blocks whose summary is not after its last read. Set by measurement
/// (`docs/adr/0119-the-worlds-ticks-blocks-and-log.md`): the size that finds fastest the changes
/// spread among many values, the common ones. A summary costs half a byte per value.
pub(crate) const BLOCK: usize = 16;

/// The values of one component type, by slot, and their ticks. Public in a private module: the
/// queries' traits name it, and nothing outside the crate can.
#[derive(Debug)]
pub struct Storage<T> {
    /// For each slot, the index of its value in `values`, or `EMPTY`.
    sparse: Vec<u32>,
    /// The slot of each value, in the order of `values`.
    slots: Vec<u32>,
    values: Vec<T>,
    /// The ticks of each value, in the order of `values`.
    ticks: Vec<Ticks>,
    /// For each block of [`BLOCK`] values, a tick at or after the latest change of every value it
    /// holds, one moved into it as another was removed included: no value of a block changed after
    /// its summary.
    blocks: Vec<Tick>,
    /// The latest tick at which a value was written, added or removed.
    changed: Tick,
    /// The entities that a component left, by removal or destruction.
    removed: Log,
}

impl<T> Storage<T> {
    pub(crate) fn new() -> Self {
        Self {
            sparse: Vec::new(),
            slots: Vec::new(),
            values: Vec::new(),
            ticks: Vec::new(),
            blocks: Vec::new(),
            changed: Tick::default(),
            removed: Log::EMPTY,
        }
    }

    fn dense(&self, slot: u32) -> Option<usize> {
        match self.sparse.get(slot as usize) {
            Some(&index) if index != EMPTY => Some(index as usize),
            _ => None,
        }
    }

    pub(crate) fn get(&self, slot: u32) -> Option<&T> {
        self.dense(slot).map(|index| &self.values[index])
    }

    /// The slot's value, to change through a guard that stamps it with `tick` once written, and
    /// marks its transform in the world stale through `marks`, if given.
    pub(crate) fn get_mut<'a>(
        &'a mut self,
        slot: u32,
        tick: Tick,
        marks: Option<TransformMarks<'a>>,
    ) -> Option<Mut<'a, T>> {
        let index = self.dense(slot)?;
        Some(Mut::new(
            &mut self.values[index],
            &mut self.ticks[index].changed,
            &mut self.blocks[index / BLOCK],
            &mut self.changed,
            tick,
            marks,
        ))
    }

    /// The ticks of the slot's value, if it holds one.
    pub(crate) fn ticks(&self, slot: u32) -> Option<Ticks> {
        self.dense(slot).map(|index| self.ticks[index])
    }

    /// Stores the slot's value, stamped with `tick`, and gives back the one it replaced, whose
    /// tick of addition the new value keeps.
    pub(crate) fn insert(&mut self, slot: u32, value: T, tick: Tick) -> Option<T> {
        self.changed = tick;
        if let Some(index) = self.dense(slot) {
            self.ticks[index].changed = tick;
            self.blocks[index / BLOCK] = tick;
            return Some(mem::replace(&mut self.values[index], value));
        }
        let at = slot as usize;
        if self.sparse.len() <= at {
            self.sparse.resize(at + 1, EMPTY);
        }
        // Fewer values than slots, and at most 2^32 - 1 slots (`Entities::spawn`): the index fits,
        // and is never `EMPTY`.
        let index = self.values.len();
        self.sparse[at] = u32::try_from(index).expect("fewer values than slots");
        self.slots.push(slot);
        self.values.push(value);
        self.ticks.push(Ticks {
            added: tick,
            changed: tick,
        });
        if index.is_multiple_of(BLOCK) {
            self.blocks.push(tick);
        } else {
            self.blocks[index / BLOCK] = tick;
        }
        None
    }

    /// Takes the slot's value out, moving the last value into its place, and logs that it left
    /// `entity`, the slot's, at `tick`.
    pub(crate) fn remove(&mut self, slot: u32, entity: Entity, tick: Tick) -> Option<T> {
        let index = self.dense(slot)?;
        self.sparse[slot as usize] = EMPTY;
        self.slots.swap_remove(index);
        let value = self.values.swap_remove(index);
        self.ticks.swap_remove(index);
        if let Some(&moved) = self.slots.get(index) {
            self.sparse[moved as usize] = u32::try_from(index).expect("fewer values than slots");
            // The moved value keeps its ticks: its new block tells that it may hold a change.
            let block = &mut self.blocks[index / BLOCK];
            *block = (*block).max(self.ticks[index].changed);
        }
        self.blocks.truncate(self.values.len().div_ceil(BLOCK));
        self.changed = tick;
        self.removed.push(tick, entity);
        Some(value)
    }

    /// The slots that hold a value, in the storage's order, which removals change.
    pub(crate) fn slots(&self) -> &[u32] {
        &self.slots
    }

    /// The latest tick at which a value was written, added or removed.
    pub(crate) fn changed(&self) -> Tick {
        self.changed
    }

    /// The slots whose value changed or was added after `since`, with their values and ticks, in
    /// the storage's order: the blocks whose summary is not after `since` are skipped unread.
    pub(crate) fn changed_since(&self, since: Tick) -> impl Iterator<Item = (u32, &T, Ticks)> {
        self.changed_blocks(since).flat_map(move |block| {
            let range = block * BLOCK..((block + 1) * BLOCK).min(self.values.len());
            range
                .filter(move |&index| self.ticks[index].changed > since)
                .map(|index| (self.slots[index], &self.values[index], self.ticks[index]))
        })
    }

    /// The blocks whose summary is after `since`.
    pub(crate) fn changed_blocks(&self, since: Tick) -> impl Iterator<Item = usize> {
        let all = if self.changed > since {
            self.blocks.len()
        } else {
            0
        };
        (0..all).filter(move |&block| self.blocks[block] > since)
    }

    /// The entities that a component of the storage left after `since`, with the ticks at which
    /// they did, oldest first; `None` if the log no longer holds all of them.
    pub(crate) fn removed_since(&self, since: Tick) -> Option<Removals<'_>> {
        self.removed.since(since)
    }
}

/// A storage whose component type is known only to the code that made it: the world keeps them
/// all side by side, and downcasts each to its `Storage<T>`.
pub trait AnyStorage: Any + Send + Sync {
    /// Drops the slot's value, if it has one, logging that it left `entity` at `tick`: its entity
    /// was destroyed.
    fn remove_slot(&mut self, slot: u32, entity: Entity, tick: Tick);

    /// Whether the slot holds a component.
    fn holds(&self, slot: u32) -> bool;

    /// The value of the slot's component, through reflection, if it holds one.
    fn value(&self, slot: u32) -> Option<Value>;

    /// Gives the slot the component that `value` describes, stamped with `tick`, in place of the
    /// one it holds, if any.
    ///
    /// # Errors
    ///
    /// As the component type's [`Reflect::from_value`]: the slot stays as it was then.
    fn insert_value(&mut self, slot: u32, value: &Value, tick: Tick) -> Result<(), ReflectError>;

    /// Replaces the slot's component with the one that `value` describes, stamped with `tick`, if
    /// the slot holds one.
    ///
    /// # Errors
    ///
    /// As the component type's [`Reflect::from_value`]: the slot's component stays then.
    fn set_value(&mut self, slot: u32, value: &Value, tick: Tick) -> Result<(), ReflectError>;
}

impl<T: Component> AnyStorage for Storage<T> {
    fn remove_slot(&mut self, slot: u32, entity: Entity, tick: Tick) {
        self.remove(slot, entity, tick);
    }

    fn holds(&self, slot: u32) -> bool {
        self.get(slot).is_some()
    }

    fn value(&self, slot: u32) -> Option<Value> {
        self.get(slot).map(Reflect::to_value)
    }

    fn insert_value(&mut self, slot: u32, value: &Value, tick: Tick) -> Result<(), ReflectError> {
        let component = T::from_value(value)?;
        self.insert(slot, component, tick);
        Ok(())
    }

    fn set_value(&mut self, slot: u32, value: &Value, tick: Tick) -> Result<(), ReflectError> {
        let component = T::from_value(value)?;
        if let Some(mut held) = self.get_mut(slot, tick, None) {
            *held = component;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entities;

    #[test]
    fn a_removed_value_leaves_the_others_where_their_slots_find_them() {
        let mut entities = Entities::default();
        let entity = entities.spawn();
        let mut storage = Storage::new();
        let tick = Tick::default();
        for slot in [4, 0, 9, 2] {
            assert_eq!(storage.insert(slot, slot * 10, tick), None);
        }
        assert_eq!(storage.insert(9, 91, tick), Some(90), "replaced");
        assert_eq!(storage.remove(0, entity, tick), Some(0));
        assert_eq!(storage.remove(0, entity, tick), None, "already removed");
        assert_eq!(storage.remove(5, entity, tick), None, "never held");
        assert_eq!(storage.get(4), Some(&40));
        assert_eq!(storage.get(9), Some(&91));
        assert_eq!(storage.get(2), Some(&20));
        assert_eq!(storage.get(100), None);
        let mut slots = storage.slots().to_vec();
        slots.sort_unstable();
        assert_eq!(slots, [2, 4, 9]);
        *storage.get_mut(2, tick, None).expect("held") += 1;
        assert_eq!(storage.get(2), Some(&21));
    }

    #[test]
    fn a_reader_of_what_changed_skips_the_blocks_that_did_not_change() {
        let mut entities = Entities::default();
        let mut storage = Storage::new();
        let slots = 0..(BLOCK * 4) as u32;
        for slot in slots.clone() {
            storage.insert(slot, slot, Tick(1));
        }
        let read = Tick(1);
        *storage
            .get_mut(BLOCK as u32 + 3, Tick(2), None)
            .expect("held") = 0;
        assert_eq!(storage.changed_blocks(read).collect::<Vec<_>>(), [1]);
        let changed: Vec<_> = storage
            .changed_since(read)
            .map(|(slot, _, _)| slot)
            .collect();
        assert_eq!(changed, [BLOCK as u32 + 3]);

        // The last value moves into the place of one removed: its new block holds its change, and
        // the block it left keeps its summary, which a reader reads again to find nothing there.
        let entity = entities.spawn();
        *storage.get_mut(slots.end - 1, Tick(3), None).expect("held") = 7;
        storage.remove(0, entity, Tick(3));
        let read = Tick(2);
        assert_eq!(storage.changed_blocks(read).collect::<Vec<_>>(), [0, 3]);
        let changed: Vec<_> = storage
            .changed_since(read)
            .map(|(slot, _, _)| slot)
            .collect();
        assert_eq!(changed, [slots.end - 1]);
        assert_eq!(
            storage.removed_since(read).map(Iterator::collect::<Vec<_>>),
            Some(vec![(entity, Tick(3))])
        );
        assert_eq!(storage.changed_blocks(Tick(3)).count(), 0, "nothing after");
    }
}
