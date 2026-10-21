//! Queries: the entities that hold every component a game asks for, with those components.
//!
//! A query names component types: `&T` to read the component of type `T`, `&mut T` to change it,
//! and tuples of up to four of these. [`World::query`] reads, through an iterator;
//! [`World::query_mut`] may change, through a closure that it calls for each entity. Both visit
//! the entities in no particular order, which adding and removing components changes, walking the
//! storage of the component that the fewest entities hold.

use std::any::{Any, TypeId, type_name};

use crate::change::{Mut, Tick, TransformMarks};
use crate::entity::Entity;
use crate::storage::{AnyStorage, Storage};
use crate::world::ComponentIds;
use crate::{Component, Transform, World};

mod sealed {
    /// Keeps the query traits to the types this crate implements them for.
    pub trait Sealed {}
}

impl<T> sealed::Sealed for &T {}
impl<T> sealed::Sealed for &mut T {}

/// A query that reads: `&T`, or a tuple of up to four queries, such as `(&Position, &Velocity)`.
///
/// [`World::query`] runs it. Its other items are the crate's own machinery.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a query that reads",
    note = "a query reads `&T`, where `T` is a `Component`, or a tuple of such queries; \
            `World::query_mut` changes components, with `&mut T`"
)]
pub trait Query: sealed::Sealed {
    /// What the query gives for one entity: `&T`, or a tuple of what its parts give.
    type Item<'w>;

    #[doc(hidden)]
    type Storages<'w>: Copy;

    #[doc(hidden)]
    fn storages(world: &World) -> Option<Self::Storages<'_>>;

    #[doc(hidden)]
    fn driver<'w>(storages: Self::Storages<'w>) -> &'w [u32];

    #[doc(hidden)]
    fn fetch<'w>(storages: Self::Storages<'w>, slot: u32) -> Option<Self::Item<'w>>;
}

impl<T: Component> Query for &T {
    type Item<'w> = &'w T;
    type Storages<'w> = &'w Storage<T>;

    fn storages(world: &World) -> Option<&Storage<T>> {
        world.storage::<T>()
    }

    fn driver<'w>(storage: Self::Storages<'w>) -> &'w [u32] {
        storage.slots()
    }

    fn fetch<'w>(storage: Self::Storages<'w>, slot: u32) -> Option<Self::Item<'w>> {
        storage.get(slot)
    }
}

/// A query that may change components: `&T` to read, `&mut T` to change, through a guard that
/// stamps the value once written ([`Mut`]), or a tuple of up to four of these, such as
/// `(&mut Position, &Velocity)`. It names each component type once.
///
/// [`World::query_mut`] runs it. Its other items are the crate's own machinery.
pub trait QueryMut: sealed::Sealed {
    /// What the query gives for one entity: `&T`, [`Mut<T>`](Mut) for `&mut T`, or a tuple of what
    /// its parts give.
    type Item<'a>;

    #[doc(hidden)]
    type Borrow<'w>;

    #[doc(hidden)]
    fn component_types(types: &mut Vec<(TypeId, &'static str, bool)>);

    #[doc(hidden)]
    fn borrow<'w>(
        ids: &ComponentIds,
        storages: &mut [Option<&'w mut dyn AnyStorage>],
    ) -> Option<Self::Borrow<'w>>;

    #[doc(hidden)]
    fn driver<'b>(borrow: &'b Self::Borrow<'_>) -> &'b [u32];

    /// The slot's item, its guards stamping with `tick` and keeping the bases of an entity settled
    /// at `base`; the part that changes a transform takes `marks`, which its first write marks.
    #[doc(hidden)]
    fn get<'a>(
        borrow: &'a mut Self::Borrow<'_>,
        slot: u32,
        tick: Tick,
        base: Tick,
        marks: &mut Option<TransformMarks<'a>>,
    ) -> Option<Self::Item<'a>>;
}

/// The storage of `T` among those a query borrows, taken so that no other part of the query can
/// borrow it too; `None` when no entity has ever held a `T`.
fn take<'w, T: Component>(
    ids: &ComponentIds,
    storages: &mut [Option<&'w mut dyn AnyStorage>],
) -> Option<&'w mut Storage<T>> {
    let id = *ids.get(&TypeId::of::<T>())?;
    let storage: &'w mut dyn Any = storages[id]
        .take()
        .expect("`World::query_mut` refuses a component type named twice");
    Some(
        storage
            .downcast_mut::<Storage<T>>()
            .expect("a storage holds the type it is registered under"),
    )
}

impl<T: Component> QueryMut for &T {
    type Item<'a> = &'a T;
    type Borrow<'w> = &'w Storage<T>;

    fn component_types(types: &mut Vec<(TypeId, &'static str, bool)>) {
        types.push((TypeId::of::<T>(), type_name::<T>(), false));
    }

    fn borrow<'w>(
        ids: &ComponentIds,
        storages: &mut [Option<&'w mut dyn AnyStorage>],
    ) -> Option<&'w Storage<T>> {
        take::<T>(ids, storages).map(|storage| &*storage)
    }

    fn driver<'b>(storage: &'b &Storage<T>) -> &'b [u32] {
        storage.slots()
    }

    fn get<'a>(
        storage: &'a mut &Storage<T>,
        slot: u32,
        _: Tick,
        _: Tick,
        _: &mut Option<TransformMarks<'a>>,
    ) -> Option<&'a T> {
        storage.get(slot)
    }
}

impl<T: Component> QueryMut for &mut T {
    type Item<'a> = Mut<'a, T>;
    type Borrow<'w> = &'w mut Storage<T>;

    fn component_types(types: &mut Vec<(TypeId, &'static str, bool)>) {
        types.push((TypeId::of::<T>(), type_name::<T>(), true));
    }

    fn borrow<'w>(
        ids: &ComponentIds,
        storages: &mut [Option<&'w mut dyn AnyStorage>],
    ) -> Option<&'w mut Storage<T>> {
        take::<T>(ids, storages)
    }

    fn driver<'b>(storage: &'b &mut Storage<T>) -> &'b [u32] {
        storage.slots()
    }

    fn get<'a>(
        storage: &'a mut &mut Storage<T>,
        slot: u32,
        tick: Tick,
        base: Tick,
        marks: &mut Option<TransformMarks<'a>>,
    ) -> Option<Mut<'a, T>> {
        let marks = if TypeId::of::<T>() == TypeId::of::<Transform>() {
            marks.take()
        } else {
            None
        };
        storage.get_mut(slot, tick, base, marks)
    }
}

/// Implements both traits for a tuple whose parts are named by type and index.
macro_rules! tuples {
    ($($part:ident $index:tt),+) => {
        impl<$($part),+> sealed::Sealed for ($($part,)+) {}

        impl<$($part: Query),+> Query for ($($part,)+) {
            type Item<'w> = ($($part::Item<'w>,)+);
            type Storages<'w> = ($($part::Storages<'w>,)+);

            fn storages(world: &World) -> Option<Self::Storages<'_>> {
                Some(($($part::storages(world)?,)+))
            }

            fn driver<'w>(storages: Self::Storages<'w>) -> &'w [u32] {
                let mut driver: Option<&'w [u32]> = None;
                $(
                    let slots = $part::driver(storages.$index);
                    if driver.is_none_or(|fewest| slots.len() < fewest.len()) {
                        driver = Some(slots);
                    }
                )+
                driver.expect("a tuple has parts")
            }

            fn fetch<'w>(storages: Self::Storages<'w>, slot: u32) -> Option<Self::Item<'w>> {
                Some(($($part::fetch(storages.$index, slot)?,)+))
            }
        }

        impl<$($part: QueryMut),+> QueryMut for ($($part,)+) {
            type Item<'a> = ($($part::Item<'a>,)+);
            type Borrow<'w> = ($($part::Borrow<'w>,)+);

            fn component_types(types: &mut Vec<(TypeId, &'static str, bool)>) {
                $($part::component_types(types);)+
            }

            fn borrow<'w>(
                ids: &ComponentIds,
                storages: &mut [Option<&'w mut dyn AnyStorage>],
            ) -> Option<Self::Borrow<'w>> {
                Some(($($part::borrow(ids, storages)?,)+))
            }

            fn driver<'b>(borrow: &'b Self::Borrow<'_>) -> &'b [u32] {
                let mut driver: Option<&'b [u32]> = None;
                $(
                    let slots = $part::driver(&borrow.$index);
                    if driver.is_none_or(|fewest| slots.len() < fewest.len()) {
                        driver = Some(slots);
                    }
                )+
                driver.expect("a tuple has parts")
            }

            fn get<'a>(
                borrow: &'a mut Self::Borrow<'_>,
                slot: u32,
                tick: Tick,
                base: Tick,
                marks: &mut Option<TransformMarks<'a>>,
            ) -> Option<Self::Item<'a>> {
                Some(($($part::get(&mut borrow.$index, slot, tick, base, marks)?,)+))
            }
        }
    };
}

tuples!(A 0);
tuples!(A 0, B 1);
tuples!(A 0, B 1, C 2);
tuples!(A 0, B 1, C 2, D 3);

/// The entities that hold every component of a [`Query`], with those components: what
/// [`World::query`] returns.
pub struct QueryIter<'w, Q: Query> {
    world: &'w World,
    storages: Option<Q::Storages<'w>>,
    driver: &'w [u32],
    next: usize,
}

impl<'w, Q: Query> QueryIter<'w, Q> {
    pub(crate) fn new(world: &'w World) -> Self {
        let storages = Q::storages(world);
        Self {
            world,
            storages,
            driver: storages.map_or(&[], Q::driver),
            next: 0,
        }
    }
}

impl<'w, Q: Query> Iterator for QueryIter<'w, Q> {
    type Item = (Entity, Q::Item<'w>);

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = if self.storages.is_some() {
            self.driver.len() - self.next
        } else {
            0
        };
        (0, Some(left))
    }

    fn next(&mut self) -> Option<Self::Item> {
        let storages = self.storages?;
        while let Some(&slot) = self.driver.get(self.next) {
            self.next += 1;
            if let Some(item) = Q::fetch(storages, slot) {
                return Some((self.world.entity_at(slot), item));
            }
        }
        None
    }
}
