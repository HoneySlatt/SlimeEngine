//! A world, or a subtree of it, written back as a scene file, its placements kept as placements
//! (`docs/adr/0092-a-world-written-back-as-a-scene.md`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, RandomState};

use slime_assets::AssetId;
use slime_reflect::{Reflect, Value};
use slime_serialize::{Composer, References};

use crate::components::Name;
use crate::document::{ADDED, CHANGED, DocumentError, REMOVED, SetAside};
use crate::entity::Entity;
use crate::identity::PersistentId;
use crate::library::{NO_RECORD, Placement, Record, ResolvedScene};
use crate::scene_file::{
    ASSETS, COMPONENTS, COMPONENTS_REMOVED, ENTITIES, ENTITIES_REMOVED, ID, PARENT, PATH, PREFAB,
    ROOT, ROOT_NAME, for_each_entity,
};
use crate::transform::Transform;
use crate::world::{Identity, World};

/// A scene file written from a world ([`World::write_scene`]), and what it could not hold.
#[derive(Debug)]
pub struct Written {
    /// The scene file's text.
    pub text: String,
    /// What the file leaves out, and why: an entity of the subtree that belongs to another scene, an
    /// entity of a placement moved out of its place, which a scene file cannot say, and an entity
    /// out of the subtree that a value refers to, which the file names `""`, no entity.
    pub left_out: Vec<SetAside>,
}

/// An entity that a scene file written from a world holds a record of.
struct Entry {
    entity: Entity,
    /// Its ID in the file.
    id: String,
    /// What it places, if it is a placement's root.
    placement: Option<std::sync::Arc<Placement>>,
}

/// The overrides of a scene file being written, by the paths of their entities from its root, in
/// the order of those paths and of the components' names.
#[derive(Default)]
struct Sections {
    changed: BTreeMap<(String, String), Value>,
    added: BTreeMap<(String, String), Value>,
    removed: BTreeSet<String>,
    removed_components: BTreeMap<String, BTreeSet<String>>,
}

/// Names the entities of a scene file being written by their paths from its root, and its assets
/// by the short names of its table; writing only.
struct WriteNames {
    /// The path of each entity that the file holds, by the bits of its handle.
    paths: HashMap<u64, String>,
    /// The short name of each asset of the file's table, by its ID.
    assets: HashMap<u128, String>,
    /// The entities that values refer to and that the file does not hold, by their bits.
    outside: RefCell<BTreeSet<u64>>,
}

impl References for WriteNames {
    /// An entity that the file does not hold is no entity, `""`.
    fn name(&self, entity: u64) -> Option<String> {
        if entity == handle_bits(Entity::NONE) {
            return Some(String::new());
        }
        Some(match self.paths.get(&entity) {
            Some(path) => path.clone(),
            None => {
                self.outside.borrow_mut().insert(entity);
                String::new()
            }
        })
    }

    fn entity(&self, _: &str) -> Option<u64> {
        None
    }

    fn asset_name(&self, id: u128) -> Option<String> {
        self.assets.get(&id).cloned()
    }

    fn asset(&self, _: &str) -> Option<u128> {
        None
    }
}

impl World {
    /// The subtree of `root` written as a scene file (`docs/adr/0089-scenes-and-prefabs.md`): a file
    /// that spawned under `root`'s key gives the same world, its placements kept as placements, and
    /// that the same world writes in the same bytes.
    ///
    /// - The root's components are the file's root's: its `Name` left out when it is the last ID of
    ///   its identity, which spawning gives it, and its `Transform` when it is the identity.
    /// - Each entity of the subtree that the scene of `root` holds, `zone/door` under `zone`, is a
    ///   record of the file under its ID, with its parent and its components, whole; each one
    ///   created outside a scene is a record under an ID that the world gives it now, its `Name` and
    ///   a few random letters, `torch-k3f9`, and keeps for the next time it writes it. Records come
    ///   in the order of a walk of the hierarchy, children in their order.
    /// - Each placement is a record that names its prefab, with what differs from it: values as
    ///   parts, components added and removed, entities removed. The overrides that the scene that
    ///   placed it wrote are kept: one that holds the prefab's value still, and one set aside, whose
    ///   entity or component the prefab no longer has.
    /// - Entities that hold [`Unsaved`](crate::Unsaved), with their descendants, fields marked
    ///   transient, and fields that hold their defaults are left out.
    /// - References to entities are paths from the root, `.` the root itself; one to an entity that
    ///   the file does not hold is `""`, no entity, and reported. Assets are named by the short names
    ///   of a table: those of the file that the scene was spawned from, else a prefab's name, else
    ///   `asset 1`, `asset 2`...
    ///
    /// What the file cannot hold is left out and reported ([`Written::left_out`]): an entity of the
    /// subtree that belongs to another scene, and the place of a placement's entity moved out of the
    /// place that its prefab gives it. The comments of the file that the scene was read from are not
    /// kept. Entities created outside scenes keep their identities, `created/<number>`: the file
    /// gives them their IDs when it is spawned.
    ///
    /// # Errors
    ///
    /// [`DocumentError::NotARoot`] if `root` is not alive, is a placement, or is an entity of one,
    /// which the scene that places it writes; [`DocumentError::Write`] if a value cannot be written,
    /// or the file would be larger or deeper than a reader reads.
    pub fn write_scene(&mut self, root: Entity) -> Result<Written, DocumentError> {
        let namespace = self.namespace_of(root)?;
        let mut left_out = Vec::new();
        let walked = self.walk(root, namespace.as_deref(), &mut left_out);
        self.give_ids(&walked, namespace.as_deref());
        let entries: Vec<Entry> = walked
            .into_iter()
            .map(|entity| Entry {
                entity,
                id: self.file_id(entity),
                placement: self.nodes[entity.index() as usize].placement.clone(),
            })
            .collect();
        self.compose(root, namespace.as_deref(), &entries, left_out)
    }

    /// The root of the outermost placement that `entity` belongs to, if it belongs to one: the
    /// placement whose prefab a scene file written from the world compares it with
    /// ([`World::write_scene`]). A placement's root belongs to itself, unless a placement holds
    /// it.
    pub fn placement_root(&self, entity: Entity) -> Option<Entity> {
        let id = self.persistent_id(entity)?;
        let parts = id.parts();
        (2..=parts.len()).find_map(|length| {
            let root = self.find(&PersistentId::new(
                parts[..length].iter().map(String::as_str),
            ))?;
            self.nodes[root.index() as usize]
                .placement
                .is_some()
                .then_some(root)
        })
    }

    /// Whether the value of `entity`'s component named `component` differs from the one that the
    /// prefab of its outermost placement gives it ([`World::placement_root`]), floats by their
    /// bits: what an inspector marks as overridden. `true` if the prefab gives it no such component;
    /// `None` if the entity belongs to no placement, or holds no such component.
    pub fn differs_from_prefab(&self, entity: Entity, component: &str) -> Option<bool> {
        let root = self.placement_root(entity)?;
        let placement = self.nodes[root.index() as usize].placement.as_deref()?;
        let value = self.component_value(entity, component)?;
        let root_id = self.persistent_id(root)?;
        let id = self.persistent_id(entity)?;
        let inner = &id.parts()[root_id.parts().len()..];
        let placed = self.placed(root, placement);
        let entities: Vec<Option<Entity>> = placed.iter().map(|(_, entity)| *entity).collect();
        let (record, _) = placed.iter().find(|(record, _)| record.path == inner)?;
        let name = self.registry.get(component)?.info().name.clone();
        let expected = match record.components.iter().find(|(held, _)| *held == name) {
            Some((_, value)) => with_handles(value, &entities),
            None if inner.is_empty() && name == "Name" => Name(self.file_id(root)).to_value(),
            None => return Some(true),
        };
        Some(matches!(
            self.registry.difference(&name, &expected, &value, false),
            Ok(Some(_))
        ))
    }

    /// The key of the scene whose entities a file written from `root` holds, or none for an entity
    /// created outside a scene.
    fn namespace_of(&self, root: Entity) -> Result<Option<String>, DocumentError> {
        let refused = |reason: &str| DocumentError::NotARoot {
            entity: self
                .persistent_id(root)
                .map_or_else(|| format!("{root:?}"), |id| format!("`{id}`")),
            reason: reason.to_owned(),
        };
        if !self.contains(root) {
            return Err(refused("it is not alive"));
        }
        let node = &self.nodes[root.index() as usize];
        if node.placement.is_some() {
            return Err(refused(
                "it is a placement, which the scene that places it writes",
            ));
        }
        match &node.identity {
            Identity::Created(_) => Ok(None),
            Identity::Scene(id) if id.parts().len() <= 2 => Ok(Some(id.parts()[0].clone())),
            Identity::Scene(_) => Err(refused(
                "it is an entity of a placement, which the scene that places it writes",
            )),
            Identity::None => Err(refused("it has no identity")),
        }
    }

    /// The entities of the subtree of `root` that the file holds records of, in the order of a walk
    /// of the hierarchy, children in their order: those of the scene `namespace` that its file
    /// makes or places, and those created outside a scene. The entities of a placement are its
    /// prefab's; those of another scene are left out, with their descendants, and reported.
    fn walk(
        &self,
        root: Entity,
        namespace: Option<&str>,
        left_out: &mut Vec<SetAside>,
    ) -> Vec<Entity> {
        let mut entries = Vec::new();
        let mut placements: HashSet<Vec<String>> = HashSet::new();
        let mut next: Vec<Entity> = self.children(root).iter().rev().copied().collect();
        while let Some(entity) = next.pop() {
            if self.get::<crate::Unsaved>(entity).is_some() {
                continue;
            }
            let node = &self.nodes[entity.index() as usize];
            let inside = match &node.identity {
                Identity::Scene(id) => {
                    (2..id.parts().len()).any(|length| placements.contains(&id.parts()[..length]))
                }
                _ => false,
            };
            if !inside {
                let ours = match &node.identity {
                    Identity::Created(_) => true,
                    Identity::Scene(id) => {
                        id.parts().len() == 2 && Some(id.parts()[0].as_str()) == namespace
                    }
                    Identity::None => false,
                };
                if !ours {
                    left_out.push(SetAside {
                        entity: self
                            .persistent_id(entity)
                            .map_or_else(|| format!("{entity:?}"), |id| id.to_string()),
                        component: None,
                        reason: "it belongs to another scene than the one written, or to a \
                                 placement out of it: left out, with its descendants"
                            .to_owned(),
                    });
                    continue;
                }
                if node.placement.is_some()
                    && let Some(id) = self.persistent_id(entity)
                {
                    placements.insert(id.parts().to_vec());
                }
                entries.push(entity);
            }
            next.extend(self.children(entity).iter().rev());
        }
        entries
    }

    /// Gives an ID of the file to each entity of `entries` created outside a scene that has none:
    /// its name, or `entity`, and four random letters or digits, unique in the file and in the scene
    /// `namespace`.
    fn give_ids(&mut self, entries: &[Entity], namespace: Option<&str>) {
        let mut taken: HashSet<String> = entries
            .iter()
            .filter(|&&entity| {
                matches!(
                    self.nodes[entity.index() as usize].identity,
                    Identity::Scene(_)
                ) || self.nodes[entity.index() as usize].written_id.is_some()
            })
            .map(|&entity| self.file_id(entity))
            .collect();
        let random = RandomState::new();
        for &entity in entries {
            let node = &self.nodes[entity.index() as usize];
            if !matches!(node.identity, Identity::Created(_)) || node.written_id.is_some() {
                continue;
            }
            let base = match self.get::<Name>(entity) {
                Some(Name(name)) => {
                    let name = name.replace('/', "-");
                    let name = name.trim();
                    if name.is_empty() || name.starts_with('#') || name == ROOT_NAME {
                        "entity".to_owned()
                    } else {
                        name.to_owned()
                    }
                }
                None => "entity".to_owned(),
            };
            let mut attempt = 0_u64;
            let id = loop {
                let mut bits = random.hash_one((entity.index(), attempt));
                let letters: String = (0..4)
                    .map(|_| {
                        let digit = (bits % 36) as u32;
                        bits /= 36;
                        char::from_digit(digit, 36).expect("a digit below 36")
                    })
                    .collect();
                let id = format!("{base}-{letters}");
                let in_scene = namespace.is_some_and(|key| {
                    self.identities
                        .contains_key(&PersistentId::new([key, id.as_str()]))
                });
                if !taken.contains(&id) && !in_scene {
                    break id;
                }
                attempt += 1;
            };
            taken.insert(id.clone());
            self.nodes[entity.index() as usize].written_id = Some(id.into_boxed_str());
        }
    }

    /// The ID of an entity that a scene file written from the world holds a record of: the last of
    /// its identity, or the ID that the world gave it, created outside a scene.
    fn file_id(&self, entity: Entity) -> String {
        let node = &self.nodes[entity.index() as usize];
        match (&node.written_id, &node.identity) {
            (Some(id), _) => id.to_string(),
            (None, Identity::Scene(id)) => id.parts().last().cloned().unwrap_or_default(),
            (None, _) => String::new(),
        }
    }

    /// The scene file of `root` and of `entries`, the scene `namespace`'s, what it leaves out added
    /// to `left_out`.
    fn compose(
        &self,
        root: Entity,
        namespace: Option<&str>,
        entries: &[Entry],
        mut left_out: Vec<SetAside>,
    ) -> Result<Written, DocumentError> {
        let scene = namespace.and_then(|key| Some((key, &self.scenes.get(key)?.scene)));
        let mut paths: HashMap<u64, String> = HashMap::new();
        paths.insert(handle_bits(root), ROOT_NAME.to_owned());
        for entry in entries {
            paths.insert(handle_bits(entry.entity), entry.id.clone());
            if let Some(placement) = &entry.placement {
                for (record, entity) in self.placed(entry.entity, placement) {
                    if let Some(entity) = entity
                        && !record.path.is_empty()
                    {
                        paths.insert(
                            handle_bits(entity),
                            format!("{}/{}", entry.id, record.path.join("/")),
                        );
                    }
                }
            }
        }
        let mut sections = Sections::default();
        for entry in entries {
            if let Some(placement) = &entry.placement {
                self.overrides_of(entry, placement, scene, &mut sections, &mut left_out)?;
            }
        }
        // The assets that the file names: its prefabs, and those that its values hold.
        let mut used: Vec<AssetId> = Vec::new();
        let mut use_asset = |id: AssetId| {
            if !used.contains(&id) {
                used.push(id);
            }
        };
        let mut values: Vec<&Value> = sections
            .changed
            .values()
            .chain(sections.added.values())
            .collect();
        let held: Vec<(Entity, Vec<(String, Value)>)> = std::iter::once(root)
            .chain(
                entries
                    .iter()
                    .filter(|entry| entry.placement.is_none())
                    .map(|entry| entry.entity),
            )
            .map(|entity| (entity, self.written_components(entity, entity == root)))
            .collect();
        values.extend(
            held.iter()
                .flat_map(|(_, components)| components.iter().map(|(_, v)| v)),
        );
        for entry in entries {
            if let Some(placement) = &entry.placement {
                use_asset(placement.prefab.id());
            }
        }
        for value in values {
            for_each_asset(value, &mut |id| use_asset(AssetId::from_u128(id)));
        }
        let table = asset_names(&used, scene, entries);
        let names = WriteNames {
            paths,
            assets: table
                .iter()
                .map(|(name, id, _)| (id.as_u128(), name.clone()))
                .collect(),
            outside: RefCell::new(BTreeSet::new()),
        };
        let mut file = Composer::new(&self.registry)
            .naming(&names)
            .without_transient()
            .without_defaults();
        let write = DocumentError::Write;
        let mut sorted = table.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, id, path) in &sorted {
            file.put(&[ASSETS, name, ID], "String", &Value::Text(id.to_string()))
                .map_err(write)?;
            if let Some(path) = path {
                file.put(&[ASSETS, name, PATH], "String", &Value::Text(path.clone()))
                    .map_err(write)?;
            }
        }
        let components_of = |entity: Entity| {
            held.iter()
                .find(|(held, _)| *held == entity)
                .map(|(_, components)| components)
        };
        for (name, value) in components_of(root).into_iter().flatten() {
            file.put(&[ROOT, COMPONENTS, name], name, value)
                .map_err(write)?;
        }
        for entry in entries {
            let id = entry.id.as_str();
            file.table(&[ENTITIES, id]).map_err(write)?;
            if let Some(parent) = self.parent(entry.entity)
                && parent != root
            {
                let path = names.name(handle_bits(parent)).unwrap_or_default();
                file.put(&[ENTITIES, id, PARENT], "String", &Value::Text(path))
                    .map_err(write)?;
            }
            match &entry.placement {
                Some(placement) => {
                    let prefab = names
                        .asset_name(placement.prefab.id().as_u128())
                        .unwrap_or_default();
                    file.put(&[ENTITIES, id, PREFAB], "String", &Value::Text(prefab))
                        .map_err(write)?;
                }
                None => {
                    for (name, value) in components_of(entry.entity).into_iter().flatten() {
                        file.put(&[ENTITIES, id, COMPONENTS, name], name, value)
                            .map_err(write)?;
                    }
                }
            }
        }
        for ((path, name), part) in &sections.changed {
            file.put_part(&[CHANGED, path, name], name, part)
                .map_err(write)?;
        }
        for ((path, name), value) in &sections.added {
            file.put(&[ADDED, path, name], name, value).map_err(write)?;
        }
        if !sections.removed.is_empty() {
            let removed = sections.removed.iter().cloned().map(Value::Text).collect();
            file.put(
                &[REMOVED, ENTITIES_REMOVED],
                "Vec<String>",
                &Value::List(removed),
            )
            .map_err(write)?;
        }
        for (path, components) in &sections.removed_components {
            let components = components.iter().cloned().map(Value::Text).collect();
            file.put(
                &[REMOVED, COMPONENTS_REMOVED, path],
                "Vec<String>",
                &Value::List(components),
            )
            .map_err(write)?;
        }
        let text = file.finish().map_err(write)?;
        for bits in names.outside.take() {
            let entity = match Entity::from_value(&Value::Entity(bits)) {
                Ok(entity) => entity,
                Err(_) => continue,
            };
            left_out.push(SetAside {
                entity: self
                    .persistent_id(entity)
                    .map_or_else(|| format!("{entity:?}"), |id| id.to_string()),
                component: None,
                reason: "a value refers to it, out of the scene written: `\"\"`, no entity, there"
                    .to_owned(),
            });
        }
        Ok(Written { text, left_out })
    }

    /// The components that a scene file writes of `entity`, by their names, in their order: the
    /// root's without the `Name` that spawning gives it and without the identity as its transform.
    fn written_components(&self, entity: Entity, root: bool) -> Vec<(String, Value)> {
        let spawned_name = match &self.nodes[entity.index() as usize].identity {
            Identity::Scene(id) => id.parts().last().cloned(),
            _ => None,
        };
        self.component_names_sorted(entity)
            .into_iter()
            .filter_map(|name| {
                let value = self.component_value(entity, name)?;
                if root
                    && name == "Name"
                    && spawned_name
                        .as_ref()
                        .is_some_and(|spawned| value.same(&Name(spawned.clone()).to_value()))
                {
                    return None;
                }
                if root && name == "Transform" && value.same(&Transform::IDENTITY.to_value()) {
                    return None;
                }
                Some((name.to_owned(), value))
            })
            .collect()
    }

    /// The records of `placement`'s prefab, each with the entity of the world that has its identity
    /// under the placement's root, `root`, if one is alive.
    fn placed<'p>(
        &self,
        root: Entity,
        placement: &'p Placement,
    ) -> Vec<(&'p Record, Option<Entity>)> {
        let base = self
            .persistent_id(root)
            .map(|id| id.parts().to_vec())
            .unwrap_or_default();
        placement
            .prefab
            .0
            .records
            .iter()
            .map(|record| {
                let id = PersistentId::new(base.iter().chain(&record.path).map(String::as_str));
                (record, self.find(&id))
            })
            .collect()
    }

    /// Adds to `sections` what differs between the placement of `entry` and its prefab, and the
    /// overrides that the scene that placed it wrote, `scene`, that still say something.
    fn overrides_of(
        &self,
        entry: &Entry,
        placement: &Placement,
        scene: Option<(&str, &ResolvedScene)>,
        sections: &mut Sections,
        left_out: &mut Vec<SetAside>,
    ) -> Result<(), DocumentError> {
        let placed = self.placed(entry.entity, placement);
        let entities: Vec<Option<Entity>> = placed.iter().map(|(_, entity)| *entity).collect();
        let path_of = |inner: &[String]| {
            if inner.is_empty() {
                entry.id.clone()
            } else {
                format!("{}/{}", entry.id, inner.join("/"))
            }
        };
        let write = DocumentError::Write;
        // What differs from the prefab now.
        let mut differences: BTreeMap<(String, String), Value> = BTreeMap::new();
        for (index, (record, entity)) in placed.iter().enumerate() {
            let path = path_of(&record.path);
            let Some(entity) = *entity else {
                let parent_alive = record
                    .parent
                    .is_some_and(|parent| entities[parent as usize].is_some());
                if parent_alive {
                    sections.removed.insert(path);
                }
                continue;
            };
            if let Some(parent) = record.parent
                && self.parent(entity) != entities[parent as usize]
            {
                left_out.push(SetAside {
                    entity: self
                        .persistent_id(entity)
                        .map_or_else(String::new, |id| id.to_string()),
                    component: None,
                    reason: "an entity of a placement moved out of its place in the prefab, which \
                             a scene file cannot say: written in its place"
                        .to_owned(),
                });
            }
            let mut expected: Vec<(String, Value)> = record
                .components
                .iter()
                .map(|(name, value)| (name.clone(), with_handles(value, &entities)))
                .collect();
            if index == 0 && !expected.iter().any(|(name, _)| name == "Name") {
                expected.push(("Name".to_owned(), Name(entry.id.clone()).to_value()));
            }
            for (name, prefab_value) in &expected {
                match self.component_value(entity, name) {
                    None => {
                        sections
                            .removed_components
                            .entry(path.clone())
                            .or_default()
                            .insert(name.clone());
                    }
                    Some(value) => {
                        if let Some(part) = self
                            .registry
                            .difference(name, prefab_value, &value, false)
                            .map_err(write)?
                        {
                            differences.insert((path.clone(), name.clone()), part);
                        }
                    }
                }
            }
            for name in self.component_names_sorted(entity) {
                if !expected.iter().any(|(held, _)| held == name)
                    && let Some(value) = self.component_value(entity, name)
                {
                    sections
                        .added
                        .insert((path.clone(), name.to_owned()), value);
                }
            }
        }
        // The overrides that the scene wrote, kept where they still say something.
        let at = |inner: &[String]| {
            placed
                .iter()
                .find(|(record, _)| record.path == inner)
                .map(|(record, entity)| (*record, *entity))
        };
        let authored = &placement.overrides;
        for inner in &authored.removed {
            if at(inner).is_none() {
                sections.removed.insert(path_of(inner));
            }
        }
        for (inner, names) in &authored.removed_components {
            let held = at(inner);
            for name in names {
                let kept = match held {
                    None => true,
                    Some((record, Some(entity))) => {
                        self.component_value(entity, name).is_none()
                            && !record.components.iter().any(|(held, _)| held == name)
                    }
                    Some((_, None)) => false,
                };
                if kept {
                    sections
                        .removed_components
                        .entry(path_of(inner))
                        .or_default()
                        .insert(name.clone());
                }
            }
        }
        for (inner, name, value) in &authored.added {
            let key = (path_of(inner), name.clone());
            match at(inner) {
                None => {
                    sections.added.insert(key, self.authored(value, scene));
                }
                Some((_, Some(entity))) => {
                    if let Some(now) = self.component_value(entity, name) {
                        differences.remove(&key);
                        sections.added.insert(key, now);
                    }
                }
                Some((_, None)) => {}
            }
        }
        for (inner, name, part) in &authored.changed {
            let key = (path_of(inner), name.clone());
            if sections.added.contains_key(&key) {
                continue;
            }
            let now = match at(inner) {
                Some((_, Some(entity))) => self.component_value(entity, name),
                _ => None,
            };
            match now {
                Some(now) => {
                    let shape = match differences.get(&key) {
                        Some(difference) => shape_union(part, difference),
                        None => part.clone(),
                    };
                    differences.insert(key, part_like(&now, &shape));
                }
                None => {
                    differences.insert(key, self.authored(part, scene));
                }
            }
        }
        sections.changed.extend(differences);
        Ok(())
    }

    /// `value`, an override as a scene file wrote it, its references to the records of the scene
    /// that placed it, `scene`, made references to the entities of the world that have their
    /// identities now, or to no entity.
    fn authored(&self, value: &Value, scene: Option<(&str, &ResolvedScene)>) -> Value {
        let mut value = value.clone();
        for_each_entity(&mut value, &mut |bits| {
            let found = scene.and_then(|(key, scene)| {
                let record = scene.0.records.get(usize::try_from(*bits).ok()?)?;
                let id = PersistentId::new(
                    std::iter::once(key).chain(record.path.iter().map(String::as_str)),
                );
                self.find(&id)
            });
            *bits = handle_bits(found.unwrap_or(Entity::NONE));
        });
        value
    }
}

/// The bits of `entity`'s handle, as a value holds them.
fn handle_bits(entity: Entity) -> u64 {
    match entity.to_value() {
        Value::Entity(bits) => bits,
        _ => 0,
    }
}

/// `value`, a resolved scene's, its references to the scene's records made references to the
/// entities that `entities` gives them, in the records' order, or to no entity.
fn with_handles(value: &Value, entities: &[Option<Entity>]) -> Value {
    let mut value = value.clone();
    for_each_entity(&mut value, &mut |bits| {
        let entity = if *bits == NO_RECORD {
            None
        } else {
            entities.get(*bits as usize).copied().flatten()
        };
        *bits = handle_bits(entity.unwrap_or(Entity::NONE));
    });
    value
}

/// Calls `visit` on the bits of every asset ID that `value` holds.
fn for_each_asset(value: &Value, visit: &mut impl FnMut(u128)) {
    match value {
        Value::Asset(id) => visit(*id),
        Value::Struct(fields) | Value::Enum(_, fields) => {
            for (_, field) in fields {
                for_each_asset(field, visit);
            }
        }
        Value::Option(Some(held)) => for_each_asset(held, visit),
        Value::List(elements) => {
            for element in elements {
                for_each_asset(element, visit);
            }
        }
        _ => {}
    }
}

/// The table of assets of a scene file that names the assets `used`, in their order: each the name
/// and the path that the file of the scene spawned, `scene`, gave it, else a prefab's name, else
/// `asset 1`, `asset 2`...; names made unique by a number.
fn asset_names(
    used: &[AssetId],
    scene: Option<(&str, &ResolvedScene)>,
    entries: &[Entry],
) -> Vec<(String, AssetId, Option<String>)> {
    let known = |id: AssetId| {
        scene.and_then(|(_, scene)| scene.0.assets.iter().find(|asset| asset.id == id))
    };
    let mut taken: HashSet<String> = used
        .iter()
        .filter_map(|&id| known(id).map(|asset| asset.name.clone()))
        .collect();
    let unique = |base: String, taken: &mut HashSet<String>| {
        let mut name = base.clone();
        let mut number = 2;
        while taken.contains(&name) {
            name = format!("{base} {number}");
            number += 1;
        }
        taken.insert(name.clone());
        name
    };
    let mut count = 0;
    used.iter()
        .map(|&id| match known(id) {
            Some(asset) => (asset.name.clone(), id, asset.path.clone()),
            None => {
                let prefab = entries.iter().find_map(|entry| {
                    let placement = entry.placement.as_ref()?;
                    (placement.prefab.id() == id).then(|| placement.prefab.name().to_owned())
                });
                let base = prefab.unwrap_or_else(|| {
                    count += 1;
                    format!("asset {count}")
                });
                (unique(base, &mut taken), id, None)
            }
        })
        .collect()
}

/// The places of `a` and `b`, two parts of values of one type: the fields that either gives, at any
/// depth.
fn shape_union(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Struct(a), Value::Struct(b)) => Value::Struct(union_fields(a, b)),
        (Value::Enum(a_name, a), Value::Enum(b_name, b)) if a_name == b_name => {
            Value::Enum(a_name.clone(), union_fields(a, b))
        }
        (_, b) => b.clone(),
    }
}

/// The fields of `a` and of `b`, each the union of both where both give it.
fn union_fields(a: &[(String, Value)], b: &[(String, Value)]) -> Vec<(String, Value)> {
    let mut fields = a.to_vec();
    for (name, value) in b {
        match fields.iter_mut().find(|(held, _)| held == name) {
            Some((_, held)) => *held = shape_union(held, value),
            None => fields.push((name.clone(), value.clone())),
        }
    }
    fields
}

/// The part of `value` that has the places of `shape`, a part of a value of the same type: each
/// field that `shape` gives, `value`'s, at any depth; `value` whole where `shape` holds another
/// variant than `value`, or a value that is not a struct's or a variant's fields.
fn part_like(value: &Value, shape: &Value) -> Value {
    let fields_like = |fields: &[(String, Value)], shape: &[(String, Value)]| {
        shape
            .iter()
            .filter_map(|(name, place)| {
                fields
                    .iter()
                    .find(|(held, _)| held == name)
                    .map(|(_, field)| (name.clone(), part_like(field, place)))
            })
            .collect()
    };
    match (value, shape) {
        (Value::Struct(fields), Value::Struct(shape)) => Value::Struct(fields_like(fields, shape)),
        (Value::Enum(name, fields), Value::Enum(shape_name, shape)) if name == shape_name => {
            Value::Enum(name.clone(), fields_like(fields, shape))
        }
        _ => value.clone(),
    }
}
