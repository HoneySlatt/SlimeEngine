//! Errors of the world.

use slime_reflect::ReflectError;

use crate::{Entity, PersistentId};

/// A change that the world refuses.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
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
    /// The entity is not alive, or holds no component of the type named.
    #[error("{entity:?} holds no component `{component}`")]
    NoComponent {
        /// The entity.
        entity: Entity,
        /// The component's type, by the name it was given.
        component: String,
    },
    /// A key that no scene is spawned under: empty, holding `/`, or `created`, under which the world
    /// names the entities it creates outside scenes.
    #[error("`{0}` cannot be a scene's key: it is empty, holds `/`, or is `created`")]
    InvalidKey(String),
    /// No component type of this name is registered in the world
    /// ([`World::register`](crate::World::register)).
    #[error("no component type `{0}` is registered")]
    UnknownComponent(String),
    /// A value that the component's type refuses, as reflection says
    /// ([`Registry::set`](slime_reflect::Registry::set)): with the path of the part at fault.
    #[error("component `{component}`: {error}")]
    Refused {
        /// The component's type, by its name.
        component: String,
        /// Why, and where in the component.
        error: ReflectError,
    },
}
