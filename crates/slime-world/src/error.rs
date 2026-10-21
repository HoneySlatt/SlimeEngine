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
    /// No entity alive has this persistent identity, which a command names
    /// ([`World::apply`](crate::World::apply)).
    #[error("no entity alive has the persistent identity `{0}`")]
    NotFound(PersistentId),
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
    /// The world spawned another scene under the key, of which an entity lives still, moved out
    /// of it: its identity is under the key, which a scene spawned under it would share
    /// ([`World::spawn_scene`](crate::World::spawn_scene)).
    #[error(
        "the scene spawned under `{key}` before has `{entity}` alive still, whose identity a scene \
         spawned under the key would share"
    )]
    KeyInUse {
        /// The key.
        key: String,
        /// The entity of the scene spawned under it before, by its persistent identity: the first
        /// of its entities alive.
        entity: PersistentId,
    },
    /// The entity is not alive, or holds no component of the type named.
    #[error("{entity:?} holds no component `{component}`")]
    NoComponent {
        /// The entity.
        entity: Entity,
        /// The component's type, by the name it was given.
        component: String,
    },
    /// A key that no scene or entity is spawned under: empty, holding `/`, or `created`, under
    /// which the world names the entities it creates outside scenes.
    #[error("`{0}` cannot be a key: it is empty, holds `/`, or is `created`")]
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
    /// A value of a scene that its component's type refuses as the scene spawns, by a rule of its
    /// type's check, which reflection leaves to the type
    /// ([`World::spawn_scene`](crate::World::spawn_scene)).
    #[error("scene `{scene}`, `{entity}`, component `{component}`: {error}")]
    RefusedInScene {
        /// The scene spawned, by its name: the file that it was resolved from, or that placed the
        /// prefab that the entity's path names.
        scene: String,
        /// The entity, by the persistent identity it would have.
        entity: PersistentId,
        /// The component's type, by its name.
        component: String,
        /// Why, and where in the component.
        error: Box<ReflectError>,
    },
}
