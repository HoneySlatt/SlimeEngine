#![doc = include_str!("../README.md")]

mod entity;
mod query;
mod storage;
mod world;

pub use entity::Entity;
pub use query::{Query, QueryIter, QueryMut};
pub use world::{Component, World};

#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    #[test]
    fn a_world_and_its_handles_can_be_shared_between_threads() {
        send_and_sync::<World>();
        send_and_sync::<Entity>();
    }
}
