//! Scenes unloaded and entities adopted (`docs/adr/0125-saves-per-scene.md`): what a world keeps of
//! a scene that it unloads, which it applies again when the scene is spawned again under its key,
//! and the entities that outlive their scene under keys of their own.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};

use slime_assets::AssetId;
use slime_reflect::{Reflect, Value};
use slime_serialize::References;

use crate::change::Tick;
use crate::components::Persistence;
use crate::document::created_root;
use crate::document::{Differences, Loaded, Numbers, Save};
use crate::entity::Entity;
use crate::error::WorldError;
use crate::identity::PersistentId;
use crate::library::ResolvedScene;
use crate::scene::{record_ids, valid_key};
use crate::scene_file::for_each_entity;
use crate::world::{Former, Identity, SpawnedScene, World};

/// What a world keeps of a scene that it unloaded: what the run changed of it since the world
/// settled it, but what saves leave out ([`Unsaved`](crate::Unsaved)) and what resets
/// ([`Persistence::Reset`]). Its values refer to entities by persistent identities: a reference is
/// the index of its identity in `references`, held in the bits of a handle of generation 0, which
/// no world gives out.
#[derive(Clone, Debug, Default)]
pub(crate) struct Unloaded {
    /// The records whose entities the run destroyed.
    pub(crate) removed: Vec<PersistentId>,
    /// The entities given another parent, and their parents, if they have one.
    pub(crate) parents: BTreeMap<PersistentId, Option<PersistentId>>,
    /// The components removed, added and changed.
    pub(crate) differences: Differences,
    /// The identities that the values refer to.
    pub(crate) references: Vec<PersistentId>,
}

/// The identities that what a world keeps of a scene unloaded refers to, each given an index once
/// ([`Unloaded::references`]): as it unloads the scene, and as a save that holds it is read, its
/// names of entities given indices, as this world names them ([`Numbers`]).
#[derive(Default)]
pub(crate) struct Table {
    pub(crate) references: RefCell<Vec<PersistentId>>,
    indices: RefCell<HashMap<PersistentId, u64>>,
    numbers: Numbers,
}

impl Table {
    /// A table that reads the names of a save that names entities created outside scenes as
    /// `numbers` says.
    pub(crate) fn reading(numbers: Numbers) -> Self {
        Self {
            numbers,
            ..Self::default()
        }
    }

    /// The index of `id`, given now if it has none yet.
    pub(crate) fn index(&self, id: PersistentId) -> u64 {
        let mut references = self.references.borrow_mut();
        *self
            .indices
            .borrow_mut()
            .entry(id.clone())
            .or_insert_with(|| {
                references.push(id);
                references.len() as u64 - 1
            })
    }
}

impl References for Table {
    /// What a world keeps names entities by indices, which it writes through a save's names.
    fn name(&self, _: u64) -> Option<String> {
        None
    }

    /// No entity, or a name that is not a persistent identity, is a handle that no world gives out.
    fn entity(&self, name: &str) -> Option<u64> {
        Some(
            self.numbers
                .identity(name)
                .map_or(Entity::NONE.to_bits(), |id| self.index(id)),
        )
    }
}

impl World {
    /// Unloads the scene spawned under `key`: destroys its entities and forgets it, keeping what
    /// the run changed of it since the world settled it ([`World::settle`]), which the world
    /// applies again when the game spawns the same scene under `key` and settles it, and which
    /// saves hold meanwhile (`docs/adr/0125-saves-per-scene.md`). What is under its entities and
    /// not of it, another scene's entity, a placement, an entity created outside scenes or adopted
    /// ([`World::adopt`]), is taken out of them first, and lives on at the world's root, but what
    /// saves leave out ([`Unsaved`](crate::Unsaved)), which goes with them.
    ///
    /// What it keeps of each entity, its components removed, added and changed, its parent, and
    /// whether the run destroyed it, its [`Persistence`] decides, as the entity holds it, or held
    /// it when the run destroyed it, else as the scene's root does: nothing of what resets, and
    /// nothing of what saves leave out. A scene spawned again and not settled since keeps what the
    /// world kept of it before.
    ///
    /// # Errors
    ///
    /// [`WorldError::NoScene`] if no scene is spawned under `key`.
    pub fn unload_scene(&mut self, key: &str) -> Result<(), WorldError> {
        let Some(held) = self.scenes.get(key) else {
            return Err(WorldError::NoScene(key.to_owned()));
        };
        let at = (key.to_owned(), held.scene.id());
        if held.settled || !self.unloaded.contains_key(&at) {
            let kept = self.kept_of(key, held);
            self.unloaded.insert(at, kept);
        }
        let scene = held.scene.clone();
        self.despawn_scene(key, &scene, &mut HashSet::new());
        Ok(())
    }

    /// What the world keeps of `held`, the scene spawned under `key`, as it unloads it.
    fn kept_of(&self, key: &str, held: &SpawnedScene) -> Unloaded {
        let root = PersistentId::new([key]);
        // The scene's mark, which its root holds, or held when the run destroyed it.
        let scene = match self.identities.get(&root) {
            Some(&root) => self.get::<Persistence>(root).copied(),
            None => self.gone.get(&root).copied().flatten(),
        }
        .unwrap_or_default();
        let resets = |mark: Option<Persistence>| mark.unwrap_or(scene) == Persistence::Reset;
        let mut saved = BTreeMap::new();
        for id in record_ids(&root, &held.scene) {
            if let Some(&entity) = self.identities.get(&id)
                && !self.unsaved(entity)
                && !resets(self.get::<Persistence>(entity).copied())
            {
                saved.insert(id, entity);
            }
        }
        let removed = self
            .gone
            .iter()
            .filter(|(id, mark)| id.parts().starts_with(root.parts()) && !resets(**mark))
            .map(|(id, _)| id.clone())
            .collect();
        // The identity that a save gives an entity, if it holds it.
        let id_of = |entity: Entity| {
            Some(entity)
                .filter(|&entity| !self.unsaved(entity))
                .and_then(|entity| self.persistent_id(entity))
        };
        let mut parents = BTreeMap::new();
        for (id, &entity) in &saved {
            if let Some(before) = &self.nodes[entity.index() as usize].parent_kept {
                let parent = self
                    .parent(entity)
                    .and_then(|parent| self.persistent_id(parent));
                if parent != *before {
                    parents.insert(id.clone(), parent);
                }
            }
        }
        let in_save: HashSet<Entity> = saved.values().copied().collect();
        let mut differences = self.changes_since_settled(&|entity| in_save.contains(&entity));
        let table = Table::default();
        let keep = |value: &mut Value| {
            for_each_entity(value, &mut |bits| {
                let id = Entity::from_value(&Value::Entity(*bits))
                    .ok()
                    .and_then(id_of);
                *bits = id.map_or(Entity::NONE.to_bits(), |id| table.index(id));
            });
        };
        for value in differences
            .added
            .values_mut()
            .chain(differences.changed.values_mut())
        {
            keep(value);
        }
        Unloaded {
            removed,
            parents,
            differences,
            references: table.references.into_inner(),
        }
    }

    /// Applies what the world kept of the scene `asset` spawned under `key` when it unloaded it,
    /// `kept`, as a save applies; what cannot be, as when the scene changed since, is logged.
    pub(crate) fn apply_unloaded(&mut self, key: &str, asset: AssetId, kept: Unloaded) {
        // The records of what the run adopted out of the scene, which spawning it again gave anew:
        // what of the scene refers to one, or is under it, goes to the entity adopted, before the
        // record is removed.
        let of_scene: Vec<Entity> = self.scenes.get(key).map_or_else(Vec::new, |held| {
            record_ids(&PersistentId::new([key]), &held.scene)
                .iter()
                .filter_map(|id| self.identities.get(id).copied())
                .collect()
        });
        for id in &kept.removed {
            let adopted = self.adopted_as(id, key, asset);
            if adopted != *id
                && let Some(&twin) = self.identities.get(id)
                && let Some(entity) = self.find(&adopted)
            {
                self.redirect(twin, entity, &of_scene);
            }
        }
        let handles: Vec<u64> = kept
            .references
            .iter()
            .map(|id| {
                let entity = self.find(&self.adopted_as(id, key, asset));
                entity.unwrap_or(Entity::NONE).to_bits()
            })
            .collect();
        let resolve = |(id, name): (PersistentId, String), mut value: Value| {
            for_each_entity(&mut value, &mut |bits| {
                if *bits >> 32 == 0 {
                    *bits = handles
                        .get(*bits as usize)
                        .copied()
                        .unwrap_or(Entity::NONE.to_bits());
                }
            });
            (id.to_string(), name, value)
        };
        let mut save = Save::default();
        save.removed = kept.removed.iter().map(ToString::to_string).collect();
        save.removed_components = kept
            .differences
            .removed
            .into_iter()
            .map(|(id, names)| (id.to_string(), names))
            .collect();
        save.parents = kept
            .parents
            .into_iter()
            .map(|(id, parent)| {
                let parent = parent.map(|parent| self.adopted_as(&parent, key, asset).to_string());
                (id.to_string(), parent)
            })
            .collect();
        save.added = kept
            .differences
            .added
            .into_iter()
            .map(|(at, value)| resolve(at, value))
            .collect();
        save.changed = kept
            .differences
            .changed
            .into_iter()
            .map(|(at, value)| resolve(at, value))
            .collect();
        let mut loaded = Loaded::default();
        self.apply_save(save, &mut loaded);
        for aside in loaded.set_aside {
            tracing::warn!(
                "the scene spawned again under `{key}` does not take back what the world kept of \
                 `{}`{}: {}",
                aside.entity,
                aside
                    .component
                    .map_or_else(String::new, |component| format!(
                        ", component `{component}`"
                    )),
                aside.reason
            );
        }
    }

    /// Adopts `entity`, an entity of a scene below its root, or of a placement, under `key`: it,
    /// and the descendants whose identities are under its own, as the entities of a placement are
    /// under its root, take identities under `key`, `companion`, `companion/light`, as an entity
    /// created under a key does ([`World::spawn_keyed`]), and no longer end with their scene
    /// (`docs/adr/0125-saves-per-scene.md`). A child whose identity is not under its own, as a
    /// scene file's entity whose parent it names, stays of its scene, which it ends with: the game
    /// adopts it too to keep it, as what the game moved under it from elsewhere.
    ///
    /// The identity that it had is its *former* identity, which the world keeps with the scene that
    /// it was of, and saves too: references written before, such as those that the world keeps of a
    /// scene unloaded, find it by it. Its scene, saved or unloaded, says that the records of what
    /// it adopted are gone, which spawning the scene again does not bring back: what of the scene
    /// refers to such a record, or is under it, is given the entity adopted. A save holds what it
    /// adopted whole, as what the run created, and loading the save into a world that holds the
    /// former entities, of the same scene, adopts them again, so that what refers to them follows.
    ///
    /// ```
    /// use slime_assets::AssetId;
    /// use slime_world::{SceneLibrary, World};
    ///
    /// let mut scenes = SceneLibrary::new();
    /// let level = AssetId::from_u128(1);
    /// scenes.insert_file(level, "level", "[entities.guide.components]\nName = \"guide\"\n");
    /// let mut world = World::new();
    /// let scene = scenes.resolve(world.registry(), level).expect("resolved");
    /// world.spawn_scene("level", &scene).expect("spawned");
    /// let guide = world.find(&"level/guide".parse().unwrap()).expect("spawned");
    /// world.adopt(guide, "companion").expect("adopted");
    /// world.unload_scene("level").expect("unloaded");
    /// assert_eq!(world.find(&"companion".parse().unwrap()), Some(guide));
    /// ```
    ///
    /// # Errors
    ///
    /// [`WorldError::NotAlive`] if `entity` is not alive; [`WorldError::InvalidKey`] if `key` is
    /// empty, holds `/`, or is `created`; [`WorldError::IdentityInUse`] if an entity has the
    /// identity `key`, or a scene is spawned under it, already, or an entity was adopted under it
    /// before, whose key stays its own; [`WorldError::NotAdoptable`] if `entity` is not an entity
    /// of a scene or a placement below its root. Nothing changes then.
    pub fn adopt(&mut self, entity: Entity, key: &str) -> Result<(), WorldError> {
        let id = valid_key(key)?;
        if !self.contains(entity) {
            return Err(WorldError::NotAlive(entity));
        }
        let former = match &self.nodes[entity.index() as usize].identity {
            Identity::Scene(former) if former.parts().len() > 1 => former.clone(),
            _ => return Err(WorldError::NotAdoptable(entity)),
        };
        let Some(scene) = self.scene_of(&former) else {
            return Err(WorldError::NotAdoptable(entity));
        };
        if self.key_taken(key) {
            return Err(WorldError::IdentityInUse(id));
        }
        let mut next = vec![entity];
        while let Some(at) = next.pop() {
            let children: Vec<Entity> = self.children(at).collect();
            next.extend(children.iter().copied());
            let Identity::Scene(old) = &self.nodes[at.index() as usize].identity else {
                continue;
            };
            let Some(rest) = old.parts().strip_prefix(former.parts()) else {
                continue;
            };
            let new = PersistentId::new(id.parts().iter().chain(rest).map(String::as_str));
            let old = old.clone();
            // Gone from its scene for good, whatever the scene's mark, if saves hold it.
            if self.nodes[at.index() as usize].base != Tick::default() && !self.unsaved(at) {
                self.gone.insert(old.clone(), Some(Persistence::Persistent));
            }
            // A child that stays of its scene, as a scene file's entity whose parent it names, has
            // another parent's identity, which a save says.
            for child in children {
                let node = &self.nodes[child.index() as usize];
                let adopted = matches!(&node.identity, Identity::Scene(id)
                    if id.parts().starts_with(former.parts()));
                if !adopted && node.base != Tick::default() && node.parent_kept.is_none() {
                    self.nodes[child.index() as usize].parent_kept = Some(Some(old.clone()));
                }
            }
            self.identities.remove(&old);
            self.identify(at, new);
            // Unsettled, as what the run creates: a save holds it whole.
            let node = &mut self.nodes[at.index() as usize];
            node.base = Tick::default();
            node.parent_kept = None;
            for storage in &mut self.storages {
                storage.forget_kept(at.index());
            }
        }
        self.formers
            .insert(key.to_owned(), Former { id: former, scene });
        self.structure = self.next_tick();
        Ok(())
    }

    /// Makes what refers to `from` among the components of `among` refer to `to`, and the children
    /// of `from` children of `to`.
    fn redirect(&mut self, from: Entity, to: Entity, among: &[Entity]) {
        let (from_bits, to_bits) = (from.to_bits(), to.to_bits());
        for &entity in among {
            let names: Vec<String> = self
                .component_names(entity)
                .into_iter()
                .map(str::to_owned)
                .collect();
            for name in names {
                let Some(mut value) = self.component_value(entity, &name) else {
                    continue;
                };
                let mut refers = false;
                for_each_entity(&mut value, &mut |bits| {
                    if *bits == from_bits {
                        *bits = to_bits;
                        refers = true;
                    }
                });
                if refers {
                    // The value that it held, but a reference, which its type took already.
                    let _ = self.set_component_value(entity, &name, value);
                }
            }
        }
        let children: Vec<Entity> = self.children(from).collect();
        for child in children {
            // Both alive, and `to` is no descendant of the record spawned anew.
            let _ = self.set_parent(child, Some(to));
        }
    }

    /// Whether `key` names an entity, a scene spawned under it, or an entity adopted under it,
    /// alive or not: the key of an entity adopted stays its own, which its former identity names.
    pub(crate) fn key_taken(&self, key: &str) -> bool {
        self.identities.contains_key(&PersistentId::new([key]))
            || self.scenes.contains_key(key)
            || self.formers.contains_key(key)
    }

    /// The asset of the scene, spawned under a key, or of the placement's prefab, that the entity
    /// of `id` is of, if the world holds it.
    pub(crate) fn scene_of(&self, id: &PersistentId) -> Option<AssetId> {
        match created_root(id) {
            Some(number) => self.placements.get(&number).map(ResolvedScene::id),
            None => self.scenes.get(&id.parts()[0]).map(|held| held.scene.id()),
        }
    }

    /// The identity that the entity of `id` had in its scene before it was adopted, if it was: `id`
    /// under the key that adopting gave it.
    pub(crate) fn former_of(&self, id: &PersistentId) -> Option<PersistentId> {
        let (key, rest) = id.parts().split_first()?;
        let former = self.formers.get(key)?;
        Some(PersistentId::new(
            former.id.parts().iter().chain(rest).map(String::as_str),
        ))
    }

    /// `id`, or the identity that adopting gave the entity that had it, if one did. An identity
    /// under `key`, where the scene `asset` is spawned, names an entity of that scene, not one
    /// adopted out of another scene that was spawned under the key before.
    fn adopted_as(&self, id: &PersistentId, key: &str, asset: AssetId) -> PersistentId {
        self.formers
            .iter()
            .find_map(|(adopted, former)| {
                if id.parts()[0] == key && former.scene != asset {
                    return None;
                }
                let rest = id.parts().strip_prefix(former.id.parts())?;
                Some(PersistentId::new(
                    std::iter::once(adopted.as_str()).chain(rest.iter().map(String::as_str)),
                ))
            })
            .unwrap_or_else(|| id.clone())
    }
}
