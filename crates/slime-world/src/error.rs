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
    /// No scene is spawned under the key ([`World::unload_scene`](crate::World::unload_scene)).
    #[error("no scene is spawned under `{0}`")]
    NoScene(String),
    /// The entity is not one that a scene spawned under a key, or a placement, gave, but its root:
    /// what adopting it would take out of its scene
    /// ([`World::adopt`](crate::World::adopt)).
    #[error("{0:?} is not an entity of a scene below its root, which alone can be adopted")]
    NotAdoptable(Entity),
    /// The entity is not alive, or holds no component of the type named.
    #[error("{entity:?} holds no component `{component}`")]
    NoComponent {
        /// The entity.
        entity: Entity,
        /// The component's type, by the name it was given.
        component: String,
    },
    /// An identity that is not one of an entity of a scene that the world creates
    /// ([`World::spawn_as`](crate::World::spawn_as)): two parts, a key under which a scene is
    /// spawned and an ID that a scene file can hold.
    #[error(
        "`{0}` cannot name an entity of a scene: it is not a key under which a scene is spawned, \
         then an ID that a scene file can hold"
    )]
    NotInScene(PersistentId),
    /// An entity that has no persistent identity, by which a command would name it.
    #[error("{0:?} has no persistent identity, by which a command would name it")]
    NoIdentity(Entity),
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
