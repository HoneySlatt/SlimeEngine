//! The eye and the lock (`docs/adr/0169-the-eye-and-the-lock.md`): what the user hid and locked,
//! by persistent identity, the user's and never the level's, and the entities that they cover.

use std::collections::HashSet;

use slime_world::{Entity, PersistentId, World};

/// What the user hid and what the user locked, each entity by its persistent identity, so that an
/// entity deleted and restored keeps them; never written, journaled nor undone.
#[derive(Clone, Debug, Default)]
pub(crate) struct Eyes {
    hidden: HashSet<PersistentId>,
    locked: HashSet<PersistentId>,
    /// Counted up at each change, which the window follows.
    revision: u64,
}

impl Eyes {
    /// Whether the user hid `id`'s entity itself.
    pub(crate) fn is_hidden(&self, id: &PersistentId) -> bool {
        self.hidden.contains(id)
    }

    /// Whether the user locked `id`'s entity itself.
    pub(crate) fn is_locked(&self, id: &PersistentId) -> bool {
        self.locked.contains(id)
    }

    /// Hides `id`'s entity, or shows it again.
    pub(crate) fn set_hidden(&mut self, id: PersistentId, hidden: bool) {
        if set(&mut self.hidden, id, hidden) {
            self.revision += 1;
        }
    }

    /// Locks `id`'s entity, or unlocks it.
    pub(crate) fn set_locked(&mut self, id: PersistentId, locked: bool) {
        if set(&mut self.locked, id, locked) {
            self.revision += 1;
        }
    }

    /// Counted up at each change.
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// The entities of `world` hidden: each that the user hid, and those below it.
    pub(crate) fn hidden_entities(&self, world: &World) -> HashSet<Entity> {
        covered(world, &self.hidden)
    }

    /// The entities of `world` locked: each that the user locked, and those below it.
    pub(crate) fn locked_entities(&self, world: &World) -> HashSet<Entity> {
        covered(world, &self.locked)
    }
}

/// Puts `id` in `ids` or takes it out; whether that changed them.
fn set(ids: &mut HashSet<PersistentId>, id: PersistentId, held: bool) -> bool {
    if held {
        ids.insert(id)
    } else {
        ids.remove(&id)
    }
}

/// The entities of `world` whose identities `ids` holds, with their descendants.
fn covered(world: &World, ids: &HashSet<PersistentId>) -> HashSet<Entity> {
    let mut entities = HashSet::new();
    let mut next: Vec<Entity> = ids.iter().filter_map(|id| world.find(id)).collect();
    while let Some(entity) = next.pop() {
        if entities.insert(entity) {
            next.extend(world.children(entity));
        }
    }
    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_hidden_covers_the_entities_below_it_and_follows_their_identities() {
        let mut world = World::new();
        let [yard, lamp, light, well] = std::array::from_fn(|_| world.spawn());
        world.set_parent(lamp, Some(yard)).expect("alive");
        world.set_parent(light, Some(lamp)).expect("alive");
        world.set_parent(well, Some(yard)).expect("alive");
        let id = |entity| world.persistent_id(entity).expect("an identity");
        let mut eyes = Eyes::default();
        eyes.set_hidden(id(lamp), true);
        eyes.set_locked(id(well), true);
        assert_eq!(eyes.hidden_entities(&world), HashSet::from([lamp, light]));
        assert_eq!(eyes.locked_entities(&world), HashSet::from([well]));
        assert!(eyes.is_hidden(&id(lamp)) && !eyes.is_hidden(&id(light)));
        let revision = eyes.revision();
        eyes.set_hidden(id(lamp), true);
        assert_eq!(eyes.revision(), revision, "nothing changed");
        eyes.set_hidden(id(lamp), false);
        assert!(eyes.hidden_entities(&world).is_empty());
        assert!(eyes.revision() > revision);
    }
}
