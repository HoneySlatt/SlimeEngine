#![doc = include_str!("../README.md")]

mod components;
mod entity;
mod error;
mod hierarchy;
mod identity;
mod query;
mod scene;
mod storage;
mod transform;
mod world;

pub use components::{Extras, Light, Meshes, Name};
pub use entity::Entity;
pub use error::WorldError;
pub use identity::PersistentId;
pub use query::{Query, QueryIter, QueryMut};
pub use transform::{GlobalTransform, Transform};
pub use world::{Component, World};

/// The math crate the API uses, re-exported so that callers use the same version.
pub use glam;

#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    #[test]
    fn a_world_its_handles_and_its_errors_can_be_shared_between_threads() {
        send_and_sync::<World>();
        send_and_sync::<Entity>();
        send_and_sync::<PersistentId>();
        send_and_sync::<WorldError>();
    }
}
