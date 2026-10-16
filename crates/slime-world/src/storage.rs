//! The storage of one component type: a sparse set, whose values sit side by side in a dense
//! array, with an index from each slot to its value.

use std::any::Any;
use std::mem;

/// A sparse entry that holds no value.
const EMPTY: u32 = u32::MAX;

/// The values of one component type, by slot. Public in a private module: the queries' traits name
/// it, and nothing outside the crate can.
#[derive(Debug)]
pub struct Storage<T> {
    /// For each slot, the index of its value in `values`, or `EMPTY`.
    sparse: Vec<u32>,
    /// The slot of each value, in the order of `values`.
    slots: Vec<u32>,
    values: Vec<T>,
}

impl<T> Storage<T> {
    pub(crate) fn new() -> Self {
        Self {
            sparse: Vec::new(),
            slots: Vec::new(),
            values: Vec::new(),
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

    pub(crate) fn get_mut(&mut self, slot: u32) -> Option<&mut T> {
        self.dense(slot).map(|index| &mut self.values[index])
    }

    /// Stores the slot's value, and gives back the one it replaced.
    pub(crate) fn insert(&mut self, slot: u32, value: T) -> Option<T> {
        if let Some(index) = self.dense(slot) {
            return Some(mem::replace(&mut self.values[index], value));
        }
        let at = slot as usize;
        if self.sparse.len() <= at {
            self.sparse.resize(at + 1, EMPTY);
        }
        // Fewer values than slots, and at most 2^32 - 1 slots (`Entities::spawn`): the index fits,
        // and is never `EMPTY`.
        self.sparse[at] = u32::try_from(self.values.len()).expect("fewer values than slots");
        self.slots.push(slot);
        self.values.push(value);
        None
    }

    /// Takes the slot's value out, moving the last value into its place.
    pub(crate) fn remove(&mut self, slot: u32) -> Option<T> {
        let index = self.dense(slot)?;
        self.sparse[slot as usize] = EMPTY;
        self.slots.swap_remove(index);
        let value = self.values.swap_remove(index);
        if let Some(&moved) = self.slots.get(index) {
            self.sparse[moved as usize] = u32::try_from(index).expect("fewer values than slots");
        }
        Some(value)
    }

    /// The slots that hold a value, in the storage's order, which removals change.
    pub(crate) fn slots(&self) -> &[u32] {
        &self.slots
    }
}

/// A storage whose component type is known only to the code that made it: the world keeps them
/// all side by side, and downcasts each to its `Storage<T>`.
pub trait AnyStorage: Any + Send + Sync {
    /// Drops the slot's value, if it has one: its entity was destroyed.
    fn remove_slot(&mut self, slot: u32);
}

impl<T: Send + Sync + 'static> AnyStorage for Storage<T> {
    fn remove_slot(&mut self, slot: u32) {
        self.remove(slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_removed_value_leaves_the_others_where_their_slots_find_them() {
        let mut storage = Storage::new();
        for slot in [4, 0, 9, 2] {
            assert_eq!(storage.insert(slot, slot * 10), None);
        }
        assert_eq!(storage.insert(9, 91), Some(90), "replaced");
        assert_eq!(storage.remove(0), Some(0));
        assert_eq!(storage.remove(0), None, "already removed");
        assert_eq!(storage.remove(5), None, "never held");
        assert_eq!(storage.get(4), Some(&40));
        assert_eq!(storage.get(9), Some(&91));
        assert_eq!(storage.get(2), Some(&20));
        assert_eq!(storage.get(100), None);
        let mut slots = storage.slots().to_vec();
        slots.sort_unstable();
        assert_eq!(slots, [2, 4, 9]);
        *storage.get_mut(2).expect("held") += 1;
        assert_eq!(storage.get(2), Some(&21));
    }
}
