//! Errors of the world.

use crate::{Entity, PersistentId};

/// A change that the world refuses.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WorldError {
    /// The entity is not alive in this world.
    #[error("{0:?} is not alive")]
    NotAlive(Entity),
    /// The entity would become its own ancestor.
    #[error("{child:?} cannot become a child of {parent:?}, which is itself or its descendant")]
    Cycle {
        /// The entity to place.
        child: Entity,
        /// The parent asked for.
        parent: Entity,
    },
    /// Another entity has this persistent identity already, as a scene spawned twice under one key
    /// would give.
    #[error("an entity has the persistent identity `{0}` already")]
    IdentityInUse(PersistentId),
}
