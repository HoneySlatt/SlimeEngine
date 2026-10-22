//! Deletions recorded, so that they can be undone: what [`Command::Delete`](crate::Command) takes out
//! of the world, and how [`Command::Restore`](crate::Command) creates it again
//! (`docs/adr/0109-stable-references-and-undo.md`).

use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::Arc;

use slime_reflect::{Kind, Registry, Value};

use crate::change::Tick;
use crate::entity::Entity;
use crate::error::WorldError;
use crate::hierarchy::children_of;
use crate::identity::PersistentId;
use crate::library::{Placement, value_size};
use crate::reflection::storage_in_registry;
use crate::scene_file::for_each_entity;
use crate::transform::Transform;
use crate::world::{Identity, World};

/// What deleting an entity took out of the world ([`Command::Delete`](crate::Command)): the entity
/// and its descendants, each with its persistent identity, its components as the values that
/// reflection reads, and its place among its siblings; what the world remembers of each, the
/// placement whose root it is among them; and the components of other entities that referred to
/// one of them. [`Command::Restore`](crate::Command) creates them again under the same
/// identities, at the same places, and points those references at them.
///
/// Its values refer to entities by their persistent identities, which hold where handles do not:
/// the entities restored take new handles, and so do the entities that a command undone and done
/// again creates. A clone costs a copy of every value.
#[derive(Clone, Debug)]
pub struct Deleted {
    /// The persistent identity of the entity deleted, the subtree's root.
    root: PersistentId,
    /// The subtree, each entity before its children, those in their order.
    entities: Vec<Record>,
    /// The parent of the subtree's root, if it had one, and the root's rank among its children.
    parent: Option<(PersistentId, usize)>,
    /// The components of entities out of the subtree that referred to an entity of it: their
    /// entities, their types' names, and their values, whole.
    referrers: Vec<(PersistentId, String, Value)>,
    /// The identities that the values refer to: a reference holds the index of its identity here,
    /// in the bits of a handle of generation 0, which no world gives out.
    references: Vec<PersistentId>,
}

/// An entity that a deletion took out of the world.
#[derive(Clone, Debug)]
struct Record {
    /// Its persistent identity; `None` for an entity that has none.
    identity: Option<PersistentId>,
    /// The index of its parent's record; `None` for the subtree's root.
    parent: Option<usize>,
    /// Its components, by their types' names.
    components: Vec<(String, Value)>,
    /// What the scene that spawned it placed here, if it is a placement's root.
    placement: Option<Arc<Placement>>,
    /// Whether the world had settled it, which makes its values the base that saves compare
    /// them with.
    settled: bool,
    /// The ID that a scene file written from the world gave it, and the identity that it takes
    /// there, for an entity created outside a scene.
    written_id: Option<Box<str>>,
    written_as: Option<PersistentId>,
}

impl PartialEq for Deleted {
    /// The same entities, values and places, and the same placements, as the same memory.
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.parent == other.parent
            && self.referrers == other.referrers
            && self.references == other.references
            && self.entities.len() == other.entities.len()
            && self.entities.iter().zip(&other.entities).all(|(a, b)| {
                a.identity == b.identity
                    && a.parent == b.parent
                    && a.components == b.components
                    && match (&a.placement, &b.placement) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        (a, b) => a.is_none() && b.is_none(),
                    }
                    && a.settled == b.settled
                    && a.written_id == b.written_id
                    && a.written_as == b.written_as
            })
    }
}

impl Deleted {
    /// The persistent identity of the entity deleted, the root of the subtree.
    pub fn root(&self) -> &PersistentId {
        &self.root
    }

    /// How many entities the deletion took out, the root and its descendants.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Whether it took out no entity: never, since a deletion takes its root at least.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// About the bytes of memory that it takes, its values and its identities counted, the
    /// placements that it shares with the world not: what an editor's history bounds.
    pub fn size(&self) -> usize {
        let id = |id: &PersistentId| {
            mem::size_of::<PersistentId>()
                + id.parts()
                    .iter()
                    .map(|part| mem::size_of::<String>() + part.len())
                    .sum::<usize>()
        };
        let records: usize = self
            .entities
            .iter()
            .map(|record| {
                mem::size_of::<Record>()
                    + record.identity.as_ref().map_or(0, id)
                    + record
                        .components
                        .iter()
                        .map(|(name, value)| {
                            mem::size_of::<String>() + name.len() + value_size(value)
                        })
                        .sum::<usize>()
            })
            .sum();
        let referrers: usize = self
            .referrers
            .iter()
            .map(|(holder, name, value)| id(holder) + name.len() + value_size(value))
            .sum();
        mem::size_of::<Self>() + records + referrers + self.references.iter().map(id).sum::<usize>()
    }
}

/// Names the entities that values refer to by their persistent identities, each given an index
/// once ([`Deleted::references`]).
#[derive(Default)]
struct Naming {
    references: Vec<PersistentId>,
    indices: HashMap<PersistentId, u64>,
}

impl Naming {
    /// `value` with each reference to an entity of `world` replaced by the index of its identity;
    /// a reference to no entity, or to one without an identity, by no entity.
    fn named(&mut self, world: &World, mut value: Value) -> Value {
        for_each_entity(&mut value, &mut |bits| {
            let id = Entity::from_bits(*bits)
                .filter(|&entity| world.contains(entity))
                .and_then(|entity| world.persistent_id(entity));
            *bits = match id {
                Some(id) => {
                    let references = &mut self.references;
                    *self.indices.entry(id.clone()).or_insert_with(|| {
                        references.push(id);
                        references.len() as u64 - 1
                    })
                }
                None => Entity::NONE.to_bits(),
            };
        });
        value
    }
}

/// `value`, named by [`Naming`], its references given the handles of the entities of `world` whose
/// identities `references` holds: no entity for one that the world does not have.
fn resolved(world: &World, references: &[PersistentId], value: &Value) -> Value {
    let mut value = value.clone();
    for_each_entity(&mut value, &mut |bits| {
        if *bits >> 32 == 0 {
            *bits = references
                .get(*bits as usize)
                .and_then(|id| world.find(id))
                .unwrap_or(Entity::NONE)
                .to_bits();
        }
    });
    value
}

/// Whether a value of the type named `ty` in `registry` may hold a reference to an entity, those
/// of `seen` left out, so that a type that holds itself ends.
fn may_refer(registry: &Registry, ty: &str, seen: &mut HashSet<String>) -> bool {
    if !seen.insert(ty.to_owned()) {
        return false;
    }
    let Some(registration) = registry.get(ty) else {
        return false;
    };
    match &registration.info().kind {
        Kind::Entity => true,
        Kind::Struct(fields) => fields
            .iter()
            .any(|field| may_refer(registry, &field.ty, seen)),
        Kind::Enum(variants) => variants
            .iter()
            .flat_map(|variant| &variant.fields)
            .any(|field| may_refer(registry, &field.ty, seen)),
        Kind::Option(inner) | Kind::List { element: inner, .. } => may_refer(registry, inner, seen),
        _ => false,
    }
}

impl World {
    /// What deleting `root`, the entity whose persistent identity is `id`, takes out of the world,
    /// before it is deleted: see [`Deleted`]. It costs a reading of each component of the subtree,
    /// and of each component outside it of a type that may refer to an entity.
    pub(crate) fn record_deletion(&self, id: &PersistentId, root: Entity) -> Deleted {
        let mut naming = Naming::default();
        // Each entity before its children, its first child first.
        let mut subtree: Vec<(Entity, Option<usize>)> = Vec::new();
        let mut next = vec![(root, None)];
        while let Some((entity, parent)) = next.pop() {
            let index = subtree.len();
            subtree.push((entity, parent));
            next.extend(
                children_of(&self.nodes, entity.index())
                    .rev()
                    .map(|child| (child, Some(index))),
            );
        }
        let entities = subtree
            .iter()
            .map(|&(entity, parent)| {
                let node = &self.nodes[entity.index() as usize];
                let components = self
                    .storages
                    .iter()
                    .zip(&self.component_types)
                    .filter_map(|(storage, &type_id)| {
                        let value = storage.value(entity.index())?;
                        let name = self.registry.of_type(type_id)?.info().name.clone();
                        Some((name, naming.named(self, value)))
                    })
                    .collect();
                Record {
                    identity: self.persistent_id(entity),
                    parent,
                    components,
                    placement: node.placement.clone(),
                    settled: node.base > Tick::default(),
                    written_id: node.written_id.clone(),
                    written_as: node.written_as.clone(),
                }
            })
            .collect();
        let parent = self.parent(root).map(|parent| {
            let id = self
                .persistent_id(parent)
                .expect("an entity alive that has children has an identity");
            (id, self.rank(root).expect("a child has a rank"))
        });
        let referrers = self.referrers(&subtree, &mut naming);
        Deleted {
            root: id.clone(),
            entities,
            parent,
            referrers,
            references: naming.references,
        }
    }

    /// The components of the entities out of `subtree` that refer to an entity of it, named by
    /// `naming`.
    fn referrers(
        &self,
        subtree: &[(Entity, Option<usize>)],
        naming: &mut Naming,
    ) -> Vec<(PersistentId, String, Value)> {
        let inside: HashSet<u64> = subtree.iter().map(|(entity, _)| entity.to_bits()).collect();
        let referring: Vec<(usize, String)> = self
            .component_types
            .iter()
            .enumerate()
            .filter_map(|(id, &type_id)| {
                let name = &self.registry.of_type(type_id)?.info().name;
                may_refer(&self.registry, name, &mut HashSet::new()).then(|| (id, name.clone()))
            })
            .collect();
        if referring.is_empty() {
            return Vec::new();
        }
        let mut referrers = Vec::new();
        for entity in self.entities() {
            if inside.contains(&entity.to_bits()) {
                continue;
            }
            for (id, name) in &referring {
                let Some(mut value) = self.storages[*id].value(entity.index()) else {
                    continue;
                };
                let mut refers = false;
                for_each_entity(&mut value, &mut |bits| refers |= inside.contains(bits));
                if refers && let Some(holder) = self.persistent_id(entity) {
                    referrers.push((holder, name.clone(), naming.named(self, value)));
                }
            }
        }
        referrers
    }

    /// Creates again what a deletion took out of the world, as [`Command::Restore`](crate::Command)
    /// says, and returns the root.
    ///
    /// # Errors
    ///
    /// [`WorldError::IdentityInUse`] if an entity alive has one of the identities;
    /// [`WorldError::NotFound`] if the root's parent is not alive; [`WorldError::UnknownComponent`]
    /// and [`WorldError::Refused`] if a component's type has no storage in the world, or refuses
    /// its value. Nothing is created then.
    pub(crate) fn restore(&mut self, deleted: &Deleted) -> Result<Entity, WorldError> {
        for id in deleted
            .entities
            .iter()
            .filter_map(|record| record.identity.as_ref())
        {
            if self.find(id).is_some() {
                return Err(WorldError::IdentityInUse(id.clone()));
            }
        }
        let parent = match &deleted.parent {
            Some((id, rank)) => Some((
                self.find(id)
                    .ok_or_else(|| WorldError::NotFound(id.clone()))?,
                *rank,
            )),
            None => None,
        };
        let mut storages = HashMap::new();
        for (name, _) in deleted
            .entities
            .iter()
            .flat_map(|record| &record.components)
        {
            if !storages.contains_key(name) {
                let (id, _) = storage_in_registry(&self.registry, &self.component_ids, name)?;
                storages.insert(name.clone(), id);
            }
        }
        let entities: Vec<Entity> = deleted.entities.iter().map(|_| self.spawn_bare()).collect();
        let mut created = Vec::new();
        for (record, &entity) in deleted.entities.iter().zip(&entities) {
            let Some(id) = &record.identity else {
                continue;
            };
            match id.created_number() {
                Some(number) => {
                    self.nodes[entity.index() as usize].identity = Identity::Created(number);
                    created.push((number, entity));
                }
                None => self.identify(entity, id.clone()),
            }
        }
        self.created.extend(created);
        let tick = self.next_tick();
        for (record, &entity) in deleted.entities.iter().zip(&entities) {
            for (name, value) in &record.components {
                let id = storages[name];
                let value = resolved(self, &deleted.references, value);
                if let Err(error) =
                    self.storages[id].insert_value(entity.index(), &value, tick, Tick::default())
                {
                    for &entity in &entities {
                        self.despawn(entity);
                    }
                    return Err(WorldError::Refused {
                        component: name.clone(),
                        error,
                    });
                }
                self.held_changed(self.component_types[id], entity.index());
                if self.component_types[id] == TypeId::of::<Transform>() {
                    self.teleport(entity);
                }
            }
        }
        for (index, record) in deleted.entities.iter().enumerate() {
            if let Some(parent) = record.parent {
                self.set_parent(entities[index], Some(entities[parent]))
                    .expect("both are alive, and a record's parent comes before it");
            }
        }
        if let Some((parent, rank)) = parent {
            self.set_parent_at(entities[0], parent, rank)
                .expect("both are alive, and the root is new");
        }
        let settled = self.next_tick();
        for (record, &entity) in deleted.entities.iter().zip(&entities) {
            let node = &mut self.nodes[entity.index() as usize];
            node.placement.clone_from(&record.placement);
            node.written_id.clone_from(&record.written_id);
            node.written_as.clone_from(&record.written_as);
            if record.settled {
                node.base = settled;
            }
            if let Some(id) = &record.identity {
                self.gone.remove(id);
                if let (Some(number), Some(placement)) = (id.created_number(), &record.placement) {
                    self.placements.insert(number, placement.prefab.clone());
                }
            }
        }
        for (holder, name, value) in &deleted.referrers {
            if let Some(holder) = self.find(holder) {
                let value = resolved(self, &deleted.references, value);
                // The holder no longer holds the component, or refuses the value, only if the world
                // changed since the deletion otherwise than by undoing: the reference stays as it is.
                let _ = self.set_component_value(holder, name, value);
            }
        }
        Ok(entities[0])
    }
}
