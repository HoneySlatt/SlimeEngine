//! Entities: the handles that a world gives out, and the slots behind them.

use std::fmt;
use std::num::NonZeroU32;

/// One thing in a [`World`](crate::World): a handle to which components are attached.
///
/// A handle is made of the index of a slot in its world and the slot's generation. Destroying an
/// entity frees its slot for a later entity under the next generation, so that the handles of
/// destroyed entities are refused: [`World::contains`](crate::World::contains) is `false` for them,
/// and [`World::get`](crate::World::get) finds nothing. A handle means something only in the world
/// that gave it out, and is never written to files. A world cannot tell a handle from another
/// world: one whose index and generation match an entity alive in it names that entity.
///
/// It takes 8 bytes, and so does an `Option<Entity>`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entity {
    index: u32,
    generation: NonZeroU32,
}

impl Entity {
    /// A handle that no world gives out, since no world holds a slot of its index: what a save's
    /// reference to no entity reads as.
    pub(crate) const NONE: Self = Self {
        index: u32::MAX,
        generation: NonZeroU32::MIN,
    };

    /// The index of the entity's slot in its world.
    pub(crate) fn index(self) -> u32 {
        self.index
    }
}

/// A reference to an entity, a kind of value of its own: [`Value::Entity`](slime_reflect::Value),
/// the bits of its handle, which mean something only in the world that gave it out, while it runs.
impl slime_reflect::Reflect for Entity {
    fn type_name() -> String {
        "Entity".to_owned()
    }

    fn type_info() -> slime_reflect::TypeInfo {
        slime_reflect::TypeInfo {
            name: Self::type_name(),
            aliases: Vec::new(),
            version: 1,
            doc: "One thing in a world: a handle to which components are attached.".to_owned(),
            default: None,
            kind: slime_reflect::Kind::Entity,
        }
    }

    fn register_dependencies(_: &mut slime_reflect::Registry) {}

    fn to_value(&self) -> slime_reflect::Value {
        slime_reflect::Value::Entity(u64::from(self.generation.get()) << 32 | u64::from(self.index))
    }

    fn from_value(value: &slime_reflect::Value) -> Result<Self, slime_reflect::ReflectError> {
        let slime_reflect::Value::Entity(bits) = *value else {
            return Err(slime_reflect::support::wrong("Entity", value));
        };
        let generation = u32::try_from(bits >> 32).ok().and_then(NonZeroU32::new);
        let Some(generation) = generation else {
            return Err(slime_reflect::ReflectError::new(
                slime_reflect::Problem::Invalid("an entity handle has a generation".to_owned()),
            ));
        };
        Ok(Self {
            index: bits as u32,
            generation,
        })
    }
}

/// `3v1`: the slot's index, then the generation.
impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}v{}", self.index, self.generation)
    }
}

/// A slot of a world, which holds one entity at a time.
#[derive(Clone, Copy, Debug)]
struct Slot {
    /// The generation of the entity the slot holds, or of the next one if it holds none.
    generation: NonZeroU32,
    alive: bool,
}

/// The slots of a world: which entities are alive, and which slots are free.
#[derive(Debug, Default)]
pub(crate) struct Entities {
    slots: Vec<Slot>,
    /// Free slots, the most recently freed last.
    free: Vec<u32>,
    alive: usize,
}

impl Entities {
    /// A new entity, in a free slot if there is one.
    ///
    /// # Panics
    ///
    /// If the world already holds `u32::MAX` slots.
    pub(crate) fn spawn(&mut self) -> Entity {
        if let Some(index) = self.free.pop() {
            self.alive += 1;
            let slot = &mut self.slots[index as usize];
            slot.alive = true;
            return Entity {
                index,
                generation: slot.generation,
            };
        }
        let index = u32::try_from(self.slots.len())
            .ok()
            .filter(|&index| index < u32::MAX)
            .expect("a world holds fewer than 2^32 - 1 slots, live or retired");
        self.alive += 1;
        self.slots.push(Slot {
            generation: NonZeroU32::MIN,
            alive: true,
        });
        Entity {
            index,
            generation: NonZeroU32::MIN,
        }
    }

    /// Frees the entity's slot under the next generation, or retires it for good if its generation
    /// would wrap, so that no later entity can share a handle with this one. `false` if the entity
    /// was not alive.
    pub(crate) fn despawn(&mut self, entity: Entity) -> bool {
        if !self.contains(entity) {
            return false;
        }
        self.alive -= 1;
        let slot = &mut self.slots[entity.index as usize];
        slot.alive = false;
        if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            self.free.push(entity.index);
        }
        true
    }

    /// Whether the entity is alive.
    pub(crate) fn contains(&self, entity: Entity) -> bool {
        self.slots
            .get(entity.index as usize)
            .is_some_and(|slot| slot.alive && slot.generation == entity.generation)
    }

    /// The handle of the entity alive in the slot. The slot must hold one.
    pub(crate) fn at(&self, index: u32) -> Entity {
        let slot = self.slots[index as usize];
        debug_assert!(slot.alive, "slot {index} holds no entity");
        Entity {
            index,
            generation: slot.generation,
        }
    }

    /// How many entities are alive.
    pub(crate) fn len(&self) -> usize {
        self.alive
    }

    /// The entities alive, in the order of their slots.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        (0..)
            .zip(&self.slots)
            .filter(|(_, slot)| slot.alive)
            .map(|(index, slot)| Entity {
                index,
                generation: slot.generation,
            })
    }

    /// Sets a slot's generation, for the tests of a generation that would wrap.
    #[cfg(test)]
    pub(crate) fn set_generation(&mut self, index: u32, generation: NonZeroU32) {
        self.slots[index as usize].generation = generation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_destroyed_entity_is_refused_and_its_slot_reused_under_the_next_generation() {
        let mut entities = Entities::default();
        let first = entities.spawn();
        assert!(entities.contains(first));
        assert!(entities.despawn(first));
        assert!(!entities.contains(first));
        assert!(!entities.despawn(first), "already destroyed");
        let second = entities.spawn();
        assert_eq!(second.index, first.index);
        assert_eq!(second.generation.get(), 2);
        assert!(!entities.contains(first));
        assert!(entities.contains(second));
        assert_eq!(entities.len(), 1);
    }

    #[test]
    fn a_slot_whose_generation_would_wrap_is_retired() {
        let mut entities = Entities::default();
        let entity = entities.spawn();
        entities.set_generation(entity.index, NonZeroU32::MAX);
        let last = entities.at(entity.index);
        assert!(entities.despawn(last));
        let next = entities.spawn();
        assert_ne!(next.index, last.index, "the slot is not reused");
        assert!(!entities.contains(last));
    }

    #[test]
    fn handles_take_eight_bytes() {
        assert_eq!(size_of::<Entity>(), 8);
        assert_eq!(size_of::<Option<Entity>>(), 8);
    }

    #[test]
    fn a_handle_prints_its_index_and_generation() {
        let mut entities = Entities::default();
        let entity = entities.spawn();
        assert_eq!(format!("{entity:?}"), "0v1");
    }
}
