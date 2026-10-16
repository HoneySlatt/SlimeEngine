//! The world: its entities, the storages of their components, and their hierarchy.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::mem;

use slime_reflect::{Reflect, Registry};

use crate::components::{Extras, Hidden, Light, Meshes, Name};
use crate::entity::{Entities, Entity};
use crate::identity::PersistentId;
use crate::query::{Query, QueryIter, QueryMut};
use crate::storage::{AnyStorage, Storage};
use crate::transform::{GlobalTransform, Transform};

/// A value that an entity can hold, of a type that threads can share and that reflection
/// describes ([`Reflect`]), so that every component can be listed, edited and saved by name
/// (`docs/adr/0085-reflection.md`). An entity holds at most one component of each type.
///
/// A type becomes a component by saying so, `impl Component for Position {}`, so that a tuple or a
/// function given to [`World::insert`] by mistake does not compile.
pub trait Component: Reflect {}

/// The entities of a game, their components and their hierarchy
/// (`docs/adr/0079-the-engines-own-world.md`).
///
/// A world gives out [`Entity`] handles, keeps the components attached to them, one of each type
/// at most, and finds the entities that hold the components a [query](crate::Query) asks for. It
/// keeps each entity's parent and children, and where each entity with a [`Transform`] is in the
/// world. It holds no GPU object and needs no window. Worlds are values: a program holds as many as
/// it needs, and threads can share them.
///
/// ```
/// use slime_reflect::Reflect;
/// use slime_world::{Component, World};
///
/// #[derive(Debug, PartialEq, Reflect)]
/// #[reflect(name = "demo::Position")]
/// struct Position(f64);
/// impl Component for Position {}
/// #[derive(Reflect)]
/// #[reflect(name = "demo::Velocity")]
/// struct Velocity(f64);
/// impl Component for Velocity {}
///
/// let mut world = World::new();
/// let hero = world.spawn();
/// world.insert(hero, Position(0.0));
/// world.insert(hero, Velocity(3.0));
/// let rock = world.spawn();
/// world.insert(rock, Position(10.0));
///
/// // What moves moves: the rock has no velocity, so the query does not visit it.
/// world.query_mut::<(&mut Position, &Velocity)>(|_, (position, velocity)| {
///     position.0 += velocity.0 / 60.0;
/// });
/// assert_eq!(world.get::<Position>(hero), Some(&Position(0.05)));
/// assert_eq!(world.get::<Position>(rock), Some(&Position(10.0)));
/// assert_eq!(world.query::<&Position>().count(), 2);
/// ```
///
/// A world keeps the registry of the types its components are ([`World::registry`]), which
/// reaches every component by its name, without its type: [`World::component_value`],
/// [`World::set_property`].
///
/// # Costs
///
/// Reading, changing, attaching or detaching an entity's component looks up its type, then its
/// slot. Destroying an entity takes a step for each component type that the world has held, for it
/// and each of its descendants, and up to a step per child of its parent. A query walks the
/// components of the type that the fewest entities hold, and looks the others up for each of them.
/// A type that no entity has held costs nothing; one that entities have held keeps its storage,
/// whose index from slots to values grows with the highest slot that held one. Changing a
/// transform marks the entity's subtree, which [`World::update_transforms`] computes again, and
/// which reading a transform in the world computes meanwhile; during a fixed step, it also keeps
/// the subtree's transforms from before the step, once per entity and step.
/// `docs/adr/0082-the-worlds-storage-and-handles.md` gives what 100 000 entities cost, and
/// `docs/adr/0083-transforms-and-the-hierarchy.md` what the hierarchy changed of it and what their
/// transforms cost.
pub struct World {
    pub(crate) entities: Entities,
    /// One storage per component type that an entity has held, at the index of its component ID.
    pub(crate) storages: Vec<Box<dyn AnyStorage>>,
    /// The component ID of each type: the index of its storage. IDs are the world's own, given in
    /// the order types are first inserted.
    pub(crate) component_ids: HashMap<TypeId, usize>,
    /// The type of each storage, by component ID.
    pub(crate) component_types: Vec<TypeId>,
    /// The types of the components, those the world has stored and those registered ahead, and
    /// the types their values hold.
    pub(crate) registry: Registry,
    /// Each slot's place in the hierarchy, identity and transform in the world.
    pub(crate) nodes: Vec<Node>,
    /// What changed of the transforms in the world.
    pub(crate) changes: Changes,
    /// The entity of each persistent identity.
    pub(crate) identities: HashMap<PersistentId, Entity>,
    /// How many transforms in the world were computed again, for the tests.
    #[cfg(test)]
    pub(crate) computed: usize,
}

/// A slot's place in the hierarchy, its identity, and its transform in the world.
#[derive(Debug, Default)]
pub(crate) struct Node {
    pub(crate) parent: Option<Entity>,
    /// In the order they became its children.
    pub(crate) children: Vec<Entity>,
    pub(crate) identity: Option<PersistentId>,
    /// Where the entity is in the world: its ancestors' transforms and its own composed, an absent
    /// transform counting as none. Current unless `stale`.
    pub(crate) global: GlobalTransform,
    /// Whether `global` must be computed again. A stale node's descendants are stale too.
    pub(crate) stale: bool,
    /// Its index in [`Changes::moved`] plus one, if its transform in the world changed during the
    /// last fixed step; 0 if it did not, or changed outside a step since.
    pub(crate) moved: u32,
}

/// What changed of the transforms in the world: what [`World::update_transforms`] computes again,
/// and what moved during the last fixed step, to draw between its two transforms.
#[derive(Default)]
pub(crate) struct Changes {
    /// Slots whose transform in the world went stale since [`World::update_transforms`] last ran,
    /// each the top of a stale subtree when it was marked, or a stale entity given a new parent.
    pub(crate) stale: Vec<u32>,
    /// Whether a fixed step goes on ([`World::begin_fixed_step`]).
    pub(crate) in_step: bool,
    /// The transform in the world, before the last fixed step, of each slot whose transform went
    /// stale during it.
    pub(crate) moved: Vec<(u32, GlobalTransform)>,
    /// The entities teleported during the fixed step going on.
    pub(crate) teleported: Vec<Entity>,
}

impl Changes {
    /// Marks a current node stale. During a fixed step, its transform in the world before the step
    /// is kept, the first time it goes stale in the step; outside one, the transform kept is
    /// forgotten, so that the node is drawn at once where it goes.
    fn went_stale(&mut self, node: &mut Node, slot: u32) {
        node.stale = true;
        if !self.in_step {
            node.moved = 0;
        } else if node.moved == 0 {
            self.moved.push((slot, node.global));
            node.moved = u32::try_from(self.moved.len()).expect("fewer moves than slots");
        }
    }
}

/// Marks a slot's transform in the world stale, with its subtree's, unless it is already.
pub(crate) fn mark_stale(nodes: &mut [Node], changes: &mut Changes, slot: u32) {
    let node = &mut nodes[slot as usize];
    if node.stale {
        return;
    }
    changes.went_stale(node, slot);
    changes.stale.push(slot);
    // Empty for an entity without children, as most are: nothing to allocate then.
    let mut subtree: Vec<u32> = node.children.iter().map(|child| child.index()).collect();
    while let Some(slot) = subtree.pop() {
        let node = &mut nodes[slot as usize];
        if !node.stale {
            changes.went_stale(node, slot);
            subtree.extend(node.children.iter().map(|child| child.index()));
        }
    }
}

/// The storage of `T` among a world's, if an entity has ever held a `T`: [`World::storage`], for
/// code that borrows the world's other fields apart.
pub(crate) fn storage_in<'s, T: Component>(
    component_ids: &HashMap<TypeId, usize>,
    storages: &'s [Box<dyn AnyStorage>],
) -> Option<&'s Storage<T>> {
    let id = *component_ids.get(&TypeId::of::<T>())?;
    let storage: &dyn Any = &*storages[id];
    Some(
        storage
            .downcast_ref::<Storage<T>>()
            .expect("a storage holds the type it is registered under"),
    )
}

impl std::fmt::Debug for World {
    /// How many entities are alive: what a world holds is read through its queries.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("World")
            .field("entities", &self.len())
            .finish_non_exhaustive()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    /// An empty world, whose registry holds the engine's components: [`Transform`], [`Name`],
    /// [`Meshes`], [`Hidden`], [`Light`] and [`Extras`].
    pub fn new() -> Self {
        let mut world = Self {
            entities: Entities::default(),
            storages: Vec::new(),
            component_ids: HashMap::new(),
            component_types: Vec::new(),
            registry: Registry::new(),
            nodes: Vec::new(),
            changes: Changes::default(),
            identities: HashMap::new(),
            #[cfg(test)]
            computed: 0,
        };
        world.register::<Transform>();
        world.register::<Name>();
        world.register::<Meshes>();
        world.register::<Hidden>();
        world.register::<Light>();
        world.register::<Extras>();
        world
    }

    /// A new entity, which holds no component and has no parent yet.
    ///
    /// # Panics
    ///
    /// If the world already holds `u32::MAX` slots, live or retired.
    pub fn spawn(&mut self) -> Entity {
        let entity = self.entities.spawn();
        let slot = entity.index() as usize;
        if self.nodes.len() <= slot {
            self.nodes.resize_with(slot + 1, Node::default);
        } else {
            self.nodes[slot] = Node::default();
        }
        if self.changes.in_step {
            // Stale from the start, it keeps no transform from before the step: it is drawn at once
            // where the step leaves it.
            self.nodes[slot].stale = true;
            self.changes.stale.push(entity.index());
        }
        entity
    }

    /// Destroys an entity, with its descendants, and drops their components. `false` if no entity
    /// with this handle is alive in this world.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.contains(entity) {
            return false;
        }
        if self.nodes[entity.index() as usize].children.is_empty() {
            self.destroy(entity);
            return true;
        }
        // Each entity before its children, its first child first: destroyed in the opposite
        // order, each entity goes after its descendants, and as the last child left to its parent.
        let mut subtree = Vec::new();
        let mut next = vec![entity];
        while let Some(entity) = next.pop() {
            subtree.push(entity);
            next.extend(self.nodes[entity.index() as usize].children.iter().rev());
        }
        for &entity in subtree.iter().rev() {
            self.destroy(entity);
        }
        true
    }

    /// Destroys a live entity whose children are destroyed already.
    fn destroy(&mut self, entity: Entity) {
        let slot = entity.index();
        // Components first: if one panics as it is dropped, the entity stays alive with the
        // others, still its parent's child, rather than leaving them to the next entity of its
        // slot.
        for storage in &mut self.storages {
            storage.remove_slot(slot);
        }
        let node = mem::take(&mut self.nodes[slot as usize]);
        if let Some(parent) = node.parent {
            let siblings = &mut self.nodes[parent.index() as usize].children;
            if siblings.last() == Some(&entity) {
                siblings.pop();
            } else {
                siblings.retain(|&child| child != entity);
            }
        }
        if let Some(identity) = node.identity {
            self.identities.remove(&identity);
        }
        self.entities.despawn(entity);
    }

    /// Whether an entity with this handle is alive in this world.
    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.contains(entity)
    }

    /// How many entities are alive.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Whether no entity is alive.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Attaches a component to an entity, and gives back the component of the same type that it
    /// replaced, if the entity held one. The first component of a type that the world stores
    /// registers the type ([`World::register`]).
    ///
    /// # Panics
    ///
    /// If the entity is not alive: a component attached to a destroyed entity would be lost, which
    /// only a bug does. As [`World::register`], the first time a component of `T` is stored.
    pub fn insert<T: Component>(&mut self, entity: Entity, component: T) -> Option<T> {
        assert!(
            self.entities.contains(entity),
            "a component attached to {entity:?}, which is not alive"
        );
        self.transform_changes::<T>(entity.index());
        let id = match self.component_ids.get(&TypeId::of::<T>()) {
            Some(&id) => id,
            None => {
                self.registry.register::<T>();
                self.storages.push(Box::new(Storage::<T>::new()));
                self.component_types.push(TypeId::of::<T>());
                let id = self.storages.len() - 1;
                self.component_ids.insert(TypeId::of::<T>(), id);
                id
            }
        };
        let storage: &mut dyn Any = &mut *self.storages[id];
        let replaced = storage
            .downcast_mut::<Storage<T>>()
            .expect("a storage holds the type it is registered under")
            .insert(entity.index(), component);
        // An entity placed for the first time during a fixed step had no place before it: it is
        // drawn at once where it is, rather than from where its parent was.
        if replaced.is_none() && TypeId::of::<T>() == TypeId::of::<Transform>() {
            self.teleport(entity);
        }
        replaced
    }

    /// Detaches the entity's component of type `T`, if it holds one.
    pub fn remove<T: Component>(&mut self, entity: Entity) -> Option<T> {
        if !self.entities.contains(entity) {
            return None;
        }
        let removed = self.storage_mut::<T>()?.remove(entity.index());
        if removed.is_some() {
            self.transform_changes::<T>(entity.index());
        }
        removed
    }

    /// The entity's component of type `T`, if it is alive and holds one.
    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        if !self.entities.contains(entity) {
            return None;
        }
        self.storage::<T>()?.get(entity.index())
    }

    /// The entity's component of type `T`, to change, if it is alive and holds one. A
    /// [`Transform`] reached this way counts as changed.
    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.entities.contains(entity) {
            return None;
        }
        self.storage::<T>()?.get(entity.index())?;
        self.transform_changes::<T>(entity.index());
        self.storage_mut::<T>()?.get_mut(entity.index())
    }

    /// The entities that hold every component that the query reads, with those components, in no
    /// particular order.
    ///
    /// ```
    /// use slime_reflect::Reflect;
    /// use slime_world::{Component, Name, World};
    ///
    /// #[derive(Reflect)]
    /// #[reflect(name = "demo::Lantern")]
    /// struct Lantern;
    /// impl Component for Lantern {}
    ///
    /// let mut world = World::new();
    /// for (name, lit) in [("north", true), ("south", false)] {
    ///     let entity = world.spawn();
    ///     world.insert(entity, Name(name.to_owned()));
    ///     if lit {
    ///         world.insert(entity, Lantern);
    ///     }
    /// }
    /// let lit: Vec<_> = world.query::<(&Name, &Lantern)>().map(|(_, (name, _))| &name.0).collect();
    /// assert_eq!(lit, ["north"]);
    /// ```
    pub fn query<Q: Query>(&self) -> QueryIter<'_, Q> {
        QueryIter::new(self)
    }

    /// Calls `visit` for each entity that holds every component that the query names, with those
    /// components, in no particular order: `&mut T` to change one, `&T` to read it. A
    /// [`Transform`] reached as `&mut Transform` counts as changed.
    ///
    /// # Panics
    ///
    /// If the query names a component type twice, as `(&mut Position, &Position)`: it would read a
    /// component while it changes it.
    pub fn query_mut<Q: QueryMut>(&mut self, mut visit: impl FnMut(Entity, Q::Item<'_>)) {
        let mut types = Vec::new();
        Q::component_types(&mut types);
        for (index, (type_id, name, _)) in types.iter().enumerate() {
            assert!(
                !types[..index].iter().any(|(other, _, _)| other == type_id),
                "a query names `{name}` twice"
            );
        }
        let changes_transforms = types
            .iter()
            .any(|&(type_id, _, mutable)| mutable && type_id == TypeId::of::<Transform>());
        let Self {
            entities,
            storages,
            component_ids,
            nodes,
            changes,
            ..
        } = self;
        // Each storage borrowed apart, so that the query's parts borrow theirs together.
        let mut borrowed: Vec<Option<&mut dyn AnyStorage>> = storages
            .iter_mut()
            .map(|storage| Some(&mut **storage))
            .collect();
        let Some(mut borrow) = Q::borrow(component_ids, &mut borrowed) else {
            return;
        };
        // `visit` changes components, never which entities hold them: the slots stay as they are.
        for index in 0..Q::driver(&borrow).len() {
            let slot = Q::driver(&borrow)[index];
            if let Some(item) = Q::get(&mut borrow, slot) {
                // Before `visit`, so that one that panics leaves no change unmarked.
                if changes_transforms {
                    mark_stale(nodes, changes, slot);
                }
                visit(entities.at(slot), item);
            }
        }
    }

    /// Marks the slot's transform in the world stale if `T` is [`Transform`].
    fn transform_changes<T: Component>(&mut self, slot: u32) {
        if TypeId::of::<T>() == TypeId::of::<Transform>() {
            mark_stale(&mut self.nodes, &mut self.changes, slot);
        }
    }

    /// The storage of `T`, if an entity has ever held a `T`.
    pub(crate) fn storage<T: Component>(&self) -> Option<&Storage<T>> {
        storage_in(&self.component_ids, &self.storages)
    }

    fn storage_mut<T: Component>(&mut self) -> Option<&mut Storage<T>> {
        let id = *self.component_ids.get(&TypeId::of::<T>())?;
        let storage: &mut dyn Any = &mut *self.storages[id];
        Some(
            storage
                .downcast_mut::<Storage<T>>()
                .expect("a storage holds the type it is registered under"),
        )
    }

    /// The handle of the entity alive in the slot, which a storage holds a value for.
    pub(crate) fn entity_at(&self, slot: u32) -> Entity {
        self.entities.at(slot)
    }
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;

    #[derive(Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Position")]
    struct Position(f64);
    impl Component for Position {}

    #[derive(Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Velocity")]
    struct Velocity(f64);
    impl Component for Velocity {}

    #[derive(Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Name")]
    struct Name(String);
    impl Component for Name {}

    /// A component type that no entity of the tests holds.
    #[derive(Reflect)]
    #[reflect(name = "test::Unused")]
    struct Unused;
    impl Component for Unused {}

    #[test]
    fn components_are_attached_read_changed_and_detached() {
        let mut world = World::new();
        let entity = world.spawn();
        assert_eq!(world.get::<Position>(entity), None);
        assert_eq!(world.insert(entity, Position(1.0)), None);
        assert_eq!(world.insert(entity, Position(2.0)), Some(Position(1.0)));
        world.get_mut::<Position>(entity).expect("held").0 += 1.0;
        assert_eq!(world.get::<Position>(entity), Some(&Position(3.0)));
        assert_eq!(world.remove::<Position>(entity), Some(Position(3.0)));
        assert_eq!(world.remove::<Position>(entity), None);
        assert_eq!(world.get::<Velocity>(entity), None, "a type never inserted");
    }

    #[test]
    fn a_destroyed_entity_drops_its_components_and_its_handle_finds_nothing() {
        /// Counts how many of its values were dropped.
        #[derive(Reflect)]
        #[reflect(name = "test::Counted")]
        struct Counted(#[reflect(skip)] Arc<AtomicUsize>);
        impl Component for Counted {}
        impl Drop for Counted {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let first = world.spawn();
        world.insert(first, Counted(Arc::clone(&dropped)));
        world.insert(first, Name("first".to_owned()));
        assert!(world.despawn(first));
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
        assert!(!world.despawn(first));
        let second = world.spawn();
        assert_eq!(second.index(), first.index(), "the slot is reused");
        assert!(!world.contains(first));
        assert_eq!(world.get::<Name>(first), None);
        assert_eq!(
            world.get::<Name>(second),
            None,
            "the new entity holds nothing yet"
        );
        assert_eq!(world.remove::<Name>(first), None);
        assert_eq!(world.len(), 1);
    }

    #[test]
    #[should_panic(expected = "which is not alive")]
    fn a_component_attached_to_a_destroyed_entity_panics() {
        let mut world = World::new();
        let entity = world.spawn();
        world.despawn(entity);
        world.insert(entity, Name("lost".to_owned()));
    }

    #[test]
    fn queries_give_the_handles_of_the_entities_now_in_reused_slots() {
        let mut world = World::new();
        let old = world.spawn();
        world.insert(old, Name("old".to_owned()));
        world.despawn(old);
        let new = world.spawn();
        world.insert(new, Name("new".to_owned()));
        let visited: Vec<Entity> = world.query::<&Name>().map(|(entity, _)| entity).collect();
        assert_eq!(visited, [new]);
        world.query_mut::<&mut Name>(|entity, _| assert_eq!(entity, new));
        assert!(!world.contains(old));
    }

    #[test]
    fn a_component_replaced_is_still_held_once() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Name("first".to_owned()));
        world.insert(entity, Name("second".to_owned()));
        let names: Vec<&str> = world
            .query::<&Name>()
            .map(|(_, name)| name.0.as_str())
            .collect();
        assert_eq!(names, ["second"]);
    }

    #[test]
    fn a_retired_slot_drops_its_components_and_is_never_given_again() {
        let mut world = World::new();
        let entity = world.spawn();
        world
            .entities
            .set_generation(entity.index(), std::num::NonZeroU32::MAX);
        let last = world.entity_at(entity.index());
        world.insert(last, Name("last".to_owned()));
        assert!(world.despawn(last));
        assert_eq!(world.query::<&Name>().count(), 0);
        assert!(world.is_empty());
        let next = world.spawn();
        assert_ne!(next.index(), last.index());
        assert_eq!(world.len(), 1);
    }

    #[test]
    fn a_component_that_panics_as_it_is_dropped_leaves_its_entity_alive() {
        #[derive(Reflect)]
        #[reflect(name = "test::Fragile")]
        struct Fragile;
        impl Component for Fragile {}
        impl Drop for Fragile {
            fn drop(&mut self) {
                panic!("dropped");
            }
        }
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Fragile);
        world.insert(entity, Name("kept".to_owned()));
        let despawned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.despawn(entity);
        }));
        assert!(despawned.is_err());
        assert!(
            world.contains(entity),
            "its slot is not given to another entity"
        );
        let other = world.spawn();
        assert_ne!(other.index(), entity.index());
        assert_eq!(world.get::<Name>(other), None);
    }

    #[test]
    fn a_handle_means_something_only_in_the_world_that_gave_it_out() {
        let mut first = World::new();
        let mut second = World::new();
        let entity = first.spawn();
        first.insert(entity, Name("here".to_owned()));
        assert!(!second.contains(entity));
        assert_eq!(second.get::<Name>(entity), None);
        let _ = second.spawn();
        // The same slot and generation in another world: a handle means something only where it
        // came from, which a world cannot tell, so it finds that world's entity.
        assert!(second.contains(entity));
    }

    /// A world of entities that hold a position; every second one a velocity; every third one a
    /// name.
    fn world_of(count: usize) -> (World, Vec<Entity>) {
        let mut world = World::new();
        let entities = (0..count)
            .map(|index| {
                let entity = world.spawn();
                world.insert(entity, Position(index as f64));
                if index % 2 == 0 {
                    world.insert(entity, Velocity(1.0));
                }
                if index % 3 == 0 {
                    world.insert(entity, Name("named".to_owned()));
                }
                entity
            })
            .collect();
        (world, entities)
    }

    #[test]
    fn a_query_visits_exactly_the_entities_that_hold_what_it_asks_for() {
        let (mut world, entities) = world_of(12);
        world.despawn(entities[6]);
        let mut visited: Vec<Entity> = world
            .query::<(&Position, &Velocity, &Name)>()
            .map(|(entity, _)| entity)
            .collect();
        visited.sort_unstable();
        assert_eq!(
            visited,
            [entities[0]],
            "0 and 6 hold all three, and 6 is destroyed"
        );
        assert_eq!(world.query::<&Position>().count(), 11);
        assert_eq!(world.query::<(&Velocity, &Position)>().count(), 5);
        for (entity, position) in world.query::<&Position>() {
            assert_eq!(world.get::<Position>(entity), Some(position));
        }
        assert_eq!(
            world.query::<&Unused>().count(),
            0,
            "a type no entity has held"
        );
    }

    #[test]
    fn changes_made_through_a_query_are_in_the_world() {
        let (mut world, entities) = world_of(10);
        let mut visits = 0;
        world.query_mut::<(&mut Position, &Velocity)>(|_, (position, velocity)| {
            position.0 += velocity.0;
            visits += 1;
        });
        assert_eq!(visits, 5);
        for (index, &entity) in entities.iter().enumerate() {
            let moved = if index % 2 == 0 { 1.0 } else { 0.0 };
            assert_eq!(
                world.get::<Position>(entity),
                Some(&Position(index as f64 + moved))
            );
        }
        world.query_mut::<&mut Name>(|_, name| name.0 = "renamed".to_owned());
        assert!(world.query::<&Name>().all(|(_, name)| name.0 == "renamed"));
        world.query_mut::<(&mut Unused, &Position)>(|_, _| unreachable!("no entity holds one"));
    }

    #[test]
    #[should_panic(expected = "twice")]
    fn a_query_that_names_a_type_twice_panics() {
        let (mut world, _) = world_of(2);
        world.query_mut::<(&mut Position, &Position)>(|_, _| {});
    }

    #[test]
    fn a_world_can_be_sent_to_another_thread() {
        let (world, entities) = world_of(4);
        let world = thread::spawn(move || {
            assert_eq!(world.query::<&Position>().count(), 4);
            world
        })
        .join()
        .expect("the thread finished");
        assert!(world.contains(entities[3]));
    }

    /// Prints what spawning, querying, changing and destroying 100 000 entities cost. Run it in a
    /// release build:
    /// `cargo test --release -p slime-world --lib -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, to run in a release build"]
    fn what_100_000_entities_cost() {
        const COUNT: usize = 100_000;
        let time = |what: &str, work: &mut dyn FnMut()| {
            // The best of five runs, to leave out the machine's noise.
            let best = (0..5)
                .map(|_| {
                    let started = Instant::now();
                    work();
                    started.elapsed()
                })
                .min()
                .unwrap_or(Duration::ZERO);
            eprintln!("{what}: {best:?}");
        };
        // Each world is dropped after it is timed.
        let mut worlds = Vec::new();
        time(
            "spawn 100 000 entities with a position, half with a velocity, a third with a name",
            &mut || worlds.push(world_of(COUNT)),
        );
        drop(worlds);
        let (mut world, entities) = world_of(COUNT);
        time("read the positions of 100 000 entities", &mut || {
            let sum: f64 = world.query::<&Position>().map(|(_, p)| p.0).sum();
            std::hint::black_box(sum);
        });
        time(
            "read the 16 667 entities with a position, a velocity and a name",
            &mut || {
                let count = world.query::<(&Position, &Velocity, &Name)>().count();
                std::hint::black_box(count);
            },
        );
        time("move the 50 000 entities with a velocity", &mut || {
            world.query_mut::<(&mut Position, &Velocity)>(|_, (position, velocity)| {
                position.0 += velocity.0;
            });
        });
        time(
            "read the positions of 100 000 entities one by one",
            &mut || {
                let sum: f64 = entities
                    .iter()
                    .filter_map(|&entity| world.get::<Position>(entity))
                    .map(|p| p.0)
                    .sum();
                std::hint::black_box(sum);
            },
        );
        // Each run destroys a world of its own, made before it is timed.
        let best = (0..5)
            .map(|_| {
                let (mut world, entities) = world_of(COUNT);
                let started = Instant::now();
                for entity in entities {
                    world.despawn(entity);
                }
                let elapsed = started.elapsed();
                std::hint::black_box(world);
                elapsed
            })
            .min()
            .unwrap_or(Duration::ZERO);
        eprintln!("destroy 100 000 entities: {best:?}");
    }
}
