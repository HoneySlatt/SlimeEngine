//! A world, or a subtree of it, written back as a scene file, its placements kept as placements
//! (`docs/adr/0092-a-world-written-back-as-a-scene.md`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, RandomState};

use slime_assets::AssetId;
use slime_reflect::{Reflect, Value};
use slime_serialize::{Composer, References};

use crate::components::{Name, Unsaved};
use crate::document::{ADDED, CHANGED, DocumentError, REMOVED, SetAside};
use crate::entity::Entity;
use crate::identity::PersistentId;
use crate::library::{NO_RECORD, Placement, Record, ResolvedScene};
use crate::scene_file::{
    ASSETS, COMPONENTS, COMPONENTS_REMOVED, ENTITIES, ENTITIES_REMOVED, ID, PARENT, PATH, PREFAB,
    ROOT, ROOT_NAME, for_each_entity, valid_id,
};
use crate::transform::Transform;
use crate::world::{Identity, World};

/// A scene file written from a world ([`World::write_scene`]), and what it could not hold.
#[derive(Debug)]
pub struct Written {
    /// The scene file's text.
    pub text: String,
    /// What the file could not hold as the world has it, each with its entity and why: an entity of
    /// another scene, or one that a key names ([`World::spawn_keyed`]), left out, with its
    /// descendants; an entity created outside a scene that holds [`Unsaved`], or is under
    /// an entity that does, left out; the place, or the order among its siblings, of an entity of a
    /// placement, which a scene file cannot say; an entity whose place in its prefab is removed; an
    /// override that no longer fits its prefab and whose value changed since; an entity whose ID a
    /// file cannot hold, written under another; a root without a `Name` or a `Transform`, which
    /// spawning gives it; and an entity out of the subtree that a value refers to, written `""`.
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

/// The entity of a [`SetAside`] report: its persistent identity, or its handle if it has none.
fn reported(world: &World, entity: Entity) -> String {
    world
        .persistent_id(entity)
        .map_or_else(|| format!("{entity:?}"), |id| id.to_string())
}

impl World {
    /// The subtree of `root` written as a scene file (`docs/adr/0089-scenes-and-prefabs.md`): a file
    /// that spawned under `root`'s key gives the same world, its placements kept as placements, and
    /// that the same world writes in the same bytes.
    ///
    /// - The root's components are the file's root's: its `Name` left out when it is the last ID of
    ///   its identity, which spawning gives it back, and its `Transform` when it is the identity. A
    ///   root created outside a scene writes its `Name`: the file spawned under that name gives it
    ///   back, and the world spawned so writes the file without it.
    /// - Each entity of the subtree that the scene of `root` holds, `zone/door` under `zone`, is a
    ///   record of the file under its ID, with its parent and its components, whole; an entity of
    ///   another scene, or one that a key names ([`World::spawn_keyed`]), which a file cannot name,
    ///   is left out, with its descendants, and reported; each one
    ///   created outside a scene, and each whose ID a file cannot hold, such as a glTF node without
    ///   a name of its own, `#3`, is a record under an ID that the world gives it: its `Name`, or its
    ///   prefab's name for a placement, or `entity`, and four random letters or digits,
    ///   `torch-k3f9`, unique in the file and in the scene, which the world keeps and gives again
    ///   while it is free. Records come in the order of a walk of the hierarchy, children in their
    ///   order.
    /// - Each placement is a record that names its prefab, with what differs from it: values as
    ///   parts, components added and removed, entities removed. The overrides that the scene that
    ///   placed it wrote are kept: one that holds the prefab's value still, and one set aside, whose
    ///   entity or component the prefab no longer has, or that no longer fits it, while the value
    ///   is the prefab's; one whose entity or component the world removed since is not.
    /// - A component that its entity marks driven ([`Driven`](crate::Driven)) is written as its
    ///   author wrote it, never its value of the moment: as the scene's record of the entity holds
    ///   it, or for a placement as the prefab gives it, with the override that the scene wrote for
    ///   it, if it wrote one. One that no author wrote, on an entity created outside scenes or added
    ///   to one, is written as it is.
    /// - [`Unsaved`], which leaves an entity out of saves, is written as any other component, for
    ///   the root, the scene's entities and its placements' alike. Entities created outside scenes
    ///   that hold it, or that are under an entity other than the root that holds it, are left
    ///   out, with their descendants, and reported: what a run creates and does not save. Fields
    ///   marked transient, and fields that hold their defaults, are left out.
    /// - References to entities are paths from the root, `.` the root itself; one to an entity that
    ///   the file does not hold is `""`, no entity, and reported if it is alive. Assets are named by
    ///   the short names of a table: those of the file that the scene was spawned from, else a
    ///   prefab's name, `lamp`, or `lamp 2` when another asset has it, else `asset 1`, `asset 2`...
    ///
    /// What the file cannot hold as the world has it is reported ([`Written::left_out`]). Entities
    /// created outside scenes keep their identities, `created/<number>`: the file gives them their
    /// IDs when it is spawned under the scene's key, which the world remembers and its saves say,
    /// so that a run carried into the file cooked again finds each there, once ([`World::carry`]).
    /// The comments of the file that the scene was read from are not kept.
    ///
    /// The cost is linear in the entities of the subtree, but that each entity created outside
    /// scenes looks for [`Unsaved`] among its ancestors below the root. Measured in a release build
    /// on the owner's machine: a world of a thousand placements of a prefab of 10 entities, 11 001
    /// entities, is written in 0.12 s; of a prefab of 100 entities, 101 001 entities, in 1.06 s,
    /// both files 136 kB, the prefab's entities placed, not copied; 10 000 entities created outside
    /// scenes, in 0.08 s, 548 kB; 80 000, in 1.0 s, 4.5 MB.
    ///
    /// ```
    /// use slime_assets::AssetId;
    /// use slime_world::{Name, SceneLibrary, World};
    ///
    /// let mut scenes = SceneLibrary::new();
    /// scenes.insert_file(AssetId::from_u128(1), "yard", "[entities.well.components]\nName = \"well\"\n");
    /// let mut world = World::new();
    /// let scene = scenes.resolve(world.registry(), AssetId::from_u128(1)).expect("a scene");
    /// let yard = world.spawn_scene("yard", &scene).expect("spawned");
    /// let bucket = world.spawn();
    /// world.insert(bucket, Name("bucket".to_owned()));
    /// world.set_parent(bucket, Some(yard)).expect("both are alive");
    /// let written = world.write_scene(yard).expect("written");
    /// assert!(written.text.contains("Name = \"well\""));
    /// assert!(written.text.contains("Name = \"bucket\""));
    /// assert_eq!(world.write_scene(yard).expect("written").text, written.text);
    /// ```
    ///
    /// # Errors
    ///
    /// [`DocumentError::NotARoot`] if `root` is not alive, has no identity, is a placement, or is an
    /// entity of one, which the scene that places it writes; [`DocumentError::Write`] if a value
    /// cannot be written, or the file would be larger or deeper than a reader reads.
    pub fn write_scene(&mut self, root: Entity) -> Result<Written, DocumentError> {
        let namespace = self.namespace_of(root)?;
        let mut left_out = Vec::new();
        let walked = self.walk(root, namespace.as_deref(), &mut left_out);
        self.give_ids(&walked, namespace.as_deref(), &mut left_out);
        if let Some(key) = namespace.as_deref() {
            for &entity in &walked {
                if matches!(
                    self.nodes[entity.index() as usize].identity,
                    Identity::Created(_)
                ) {
                    let id = PersistentId::new([key, &self.file_id(entity)]);
                    self.nodes[entity.index() as usize].written_as = Some(id);
                }
            }
        }
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
        let index = placed.iter().position(|(record, _)| record.path == inner)?;
        let name = self.registry.get(component)?.info().name.clone();
        let Some(expected) = expected(
            placed[index].0,
            index,
            Some(&self.file_id(root)),
            &entities,
            &name,
        ) else {
            return Some(true);
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
    /// makes or places, and those created outside a scene. The entities of a placement of the
    /// subtree are its prefab's, wherever they are; those of another scene, and those created outside
    /// a scene that a save of the subtree would leave out ([`World::left_unsaved`]), are left out,
    /// with their descendants, and reported.
    fn walk(
        &self,
        root: Entity,
        namespace: Option<&str>,
        left_out: &mut Vec<SetAside>,
    ) -> Vec<Entity> {
        // The placements of the subtree first, so that an entity of one moved before it is known.
        let mut placements: HashSet<Vec<String>> = HashSet::new();
        let mut next: Vec<Entity> = self.children(root).collect();
        while let Some(entity) = next.pop() {
            if self.nodes[entity.index() as usize].placement.is_some()
                && let Some(id) = self.persistent_id(entity)
            {
                placements.insert(id.parts().to_vec());
            }
            next.extend(self.children(entity));
        }
        let mut entries = Vec::new();
        let mut next: Vec<Entity> = self.children(root).rev().collect();
        while let Some(entity) = next.pop() {
            if self.left_unsaved(root, entity) {
                left_out.push(SetAside {
                    entity: reported(self, entity),
                    component: None,
                    reason: "created outside a scene, and Unsaved holds it or an entity above it: \
                             left out, with its descendants"
                        .to_owned(),
                });
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
                    let reason = match &node.identity {
                        // One part, as the root of a scene spawned under a key has too.
                        Identity::Scene(id)
                            if id.parts().len() == 1
                                && !self.scenes.contains_key(&id.parts()[0]) =>
                        {
                            "a key names it (`World::spawn_keyed`), which a scene file does not \
                             hold: left out, with its descendants"
                        }
                        _ => {
                            "it belongs to another scene than the one written, or to a placement \
                             out of it: left out, with its descendants"
                        }
                    };
                    left_out.push(SetAside {
                        entity: reported(self, entity),
                        component: None,
                        reason: reason.to_owned(),
                    });
                    continue;
                }
                entries.push(entity);
            }
            next.extend(self.children(entity).rev());
        }
        entries
    }

    /// Gives an ID of the file to each entity of `entries` that has none it can keep: one created
    /// outside a scene, and one of the scene whose ID a file cannot hold, which is reported. The ID
    /// is its name, or its prefab's name for a placement, or `entity`, and four random letters or
    /// digits, unique in the file and in the scene `namespace`; one that the world gave before is
    /// kept while it is free.
    fn give_ids(
        &mut self,
        entries: &[Entity],
        namespace: Option<&str>,
        left_out: &mut Vec<SetAside>,
    ) {
        // The IDs of the scene's entities come first: their identities hold them.
        let mut taken: HashSet<String> = HashSet::new();
        let mut needing = Vec::new();
        for &entity in entries {
            let node = &self.nodes[entity.index() as usize];
            if let (None, Identity::Scene(id)) = (&node.written_id, &node.identity) {
                let own = id.parts().last().cloned().unwrap_or_default();
                if valid_id(&own).is_ok() && taken.insert(own) {
                    continue;
                }
            }
            needing.push(entity);
        }
        let random = RandomState::new();
        for entity in needing {
            let free = |world: &World, taken: &HashSet<String>, id: &str| {
                !taken.contains(id)
                    && namespace.is_none_or(|key| {
                        world
                            .identities
                            .get(&PersistentId::new([key, id]))
                            .is_none_or(|&other| other == entity)
                    })
            };
            if let Some(id) = &self.nodes[entity.index() as usize].written_id
                && free(self, &taken, id)
            {
                taken.insert(id.to_string());
                continue;
            }
            let node = &self.nodes[entity.index() as usize];
            let base = self
                .get::<Name>(entity)
                .map(|Name(name)| name.clone())
                .or_else(|| {
                    node.placement
                        .as_ref()
                        .map(|placement| placement.prefab.name().to_owned())
                })
                .map(|name| name.replace('/', "-").trim().to_owned())
                .filter(|name| valid_id(name).is_ok())
                .unwrap_or_else(|| "entity".to_owned());
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
                if free(self, &taken, &id) {
                    break id;
                }
                attempt += 1;
            };
            if let Identity::Scene(own) = &node.identity {
                left_out.push(SetAside {
                    entity: own.to_string(),
                    component: None,
                    reason: format!(
                        "a scene file cannot hold its ID: written as `{id}`, which names it when \
                         the file is spawned"
                    ),
                });
            }
            taken.insert(id.clone());
            self.nodes[entity.index() as usize].written_id = Some(id.into_boxed_str());
        }
    }

    /// The ID of an entity that a scene file written from the world holds a record of: the ID that
    /// the world gave it, else the last of its identity.
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
                self.overrides_of(root, entry, placement, scene, &mut sections, &mut left_out)?;
            }
        }
        for (component, missing) in [
            ("Name", self.get::<Name>(root).is_none()),
            ("Transform", self.get::<Transform>(root).is_none()),
        ] {
            if missing {
                left_out.push(SetAside {
                    entity: reported(self, root),
                    component: Some(component.to_owned()),
                    reason: "the root holds none, and spawning gives it one".to_owned(),
                });
            }
        }
        // The records of the scene of `root`, by their paths, which give the driven components as
        // their author wrote them.
        let records: HashMap<&[String], &Record> = scene
            .map(|(_, resolved)| {
                resolved
                    .0
                    .records
                    .iter()
                    .map(|record| (record.path.as_slice(), record))
                    .collect()
            })
            .unwrap_or_default();
        let held: HashMap<Entity, Vec<(String, Value)>> = std::iter::once(root)
            .chain(
                entries
                    .iter()
                    .filter(|entry| entry.placement.is_none())
                    .map(|entry| entry.entity),
            )
            .map(|entity| {
                let components = self.written_components(entity, entity == root, scene, &records);
                (entity, components)
            })
            .collect();
        // The assets that the file names: its prefabs, and those that its values hold.
        let mut used: Vec<AssetId> = Vec::new();
        let mut use_asset = |id: AssetId| {
            if !used.contains(&id) {
                used.push(id);
            }
        };
        for entry in entries {
            if let Some(placement) = &entry.placement {
                use_asset(placement.prefab.id());
            }
        }
        let values = sections
            .changed
            .values()
            .chain(sections.added.values())
            .chain(
                std::iter::once(root)
                    .chain(entries.iter().map(|entry| entry.entity))
                    .flat_map(|entity| {
                        held.get(&entity)
                            .into_iter()
                            .flatten()
                            .map(|(_, value)| value)
                    }),
            );
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
        for (name, value) in held.get(&root).into_iter().flatten() {
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
                    for (name, value) in held.get(&entry.entity).into_iter().flatten() {
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
            // A destroyed entity is no entity, as a save writes it: nothing to report.
            let Ok(entity) = Entity::from_value(&Value::Entity(bits)) else {
                continue;
            };
            if !self.contains(entity) {
                continue;
            }
            let reason = if self.left_unsaved(root, entity) {
                "a value refers to it, which is left out with Unsaved: `\"\"`, no entity, there"
            } else {
                "a value refers to it, out of the subtree written: `\"\"`, no entity, there"
            };
            left_out.push(SetAside {
                entity: reported(self, entity),
                component: None,
                reason: reason.to_owned(),
            });
        }
        Ok(Written { text, left_out })
    }

    /// The components that a scene file writes of `entity`, by their names, in their order: the
    /// root's without the `Name` that spawning gives it and without the identity as its transform;
    /// a component that `entity` marks driven as its record in `scene`, the scene of the file's
    /// root, whose `records` are given by their paths, holds it, as its author wrote it, if the
    /// record holds it.
    fn written_components(
        &self,
        entity: Entity,
        root: bool,
        scene: Option<(&str, &ResolvedScene)>,
        records: &HashMap<&[String], &Record>,
    ) -> Vec<(String, Value)> {
        let spawned_name = match &self.nodes[entity.index() as usize].identity {
            Identity::Scene(id) => id.parts().last().cloned(),
            _ => None,
        };
        self.component_names_sorted(entity)
            .into_iter()
            .filter_map(|name| {
                let authored = if self.drives(entity, name) {
                    self.authored_component(entity, name, scene, records)
                } else {
                    None
                };
                let value = authored.or_else(|| self.component_value(entity, name))?;
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

    /// Whether a scene file written from `root` leaves `entity` out: an entity created outside a
    /// scene that holds [`Unsaved`], or whose ancestor below `root` does, what a run creates and does
    /// not save. The scene's own entities and its placements are written whatever they hold.
    fn left_unsaved(&self, root: Entity, entity: Entity) -> bool {
        if !matches!(
            self.nodes[entity.index() as usize].identity,
            Identity::Created(_)
        ) {
            return false;
        }
        let mut next = Some(entity);
        while let Some(entity) = next.filter(|&entity| entity != root) {
            if self.get::<Unsaved>(entity).is_some() {
                return true;
            }
            next = self.parent(entity);
        }
        false
    }

    /// Sets the entities of the placement of `root` back to what its prefab gives them: each
    /// component of a record as the record holds it, the root named by its ID, and the components
    /// that the record does not hold removed. Loading a save that describes the placement from its
    /// prefab does it first, when the world has the placement from a scene file written since, with
    /// its overrides (`World::load`).
    pub(crate) fn reset_to_prefab(&mut self, root: Entity) {
        let Some(placement) = self.nodes[root.index() as usize].placement.clone() else {
            return;
        };
        let root_name = self.file_id(root);
        let placed = self.placed(root, &placement);
        let entities: Vec<Option<Entity>> = placed.iter().map(|(_, entity)| *entity).collect();
        for (index, (record, entity)) in placed.iter().enumerate() {
            let Some(entity) = *entity else {
                continue;
            };
            let held: Vec<String> = self
                .component_names(entity)
                .into_iter()
                .map(str::to_owned)
                .collect();
            for name in &held {
                match expected(record, index, Some(&root_name), &entities, name) {
                    Some(value) => {
                        // A value that the prefab gives is one that its types accept.
                        let _ = self.set_component_value(entity, name, value);
                    }
                    None => {
                        self.remove_component_value(entity, name);
                    }
                }
            }
            for (name, _) in &record.components {
                if !held.contains(name)
                    && let Some(value) = expected(record, index, Some(&root_name), &entities, name)
                {
                    let _ = self.insert_component_value(entity, name, value);
                }
            }
        }
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

    /// Adds to `sections` what differs between the placement of `entry`, in a file written from
    /// `root`, and its prefab, and the overrides that the scene that placed it wrote, `scene`, that
    /// still say something; and to `left_out` what the file cannot say of it.
    fn overrides_of(
        &self,
        root: Entity,
        entry: &Entry,
        placement: &Placement,
        scene: Option<(&str, &ResolvedScene)>,
        sections: &mut Sections,
        left_out: &mut Vec<SetAside>,
    ) -> Result<(), DocumentError> {
        let placed = self.placed(entry.entity, placement);
        let entities: Vec<Option<Entity>> = placed.iter().map(|(_, entity)| *entity).collect();
        let index_of: HashMap<Entity, usize> = entities
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| Some(((*entity)?, index)))
            .collect();
        let path_of = |inner: &[String]| {
            if inner.is_empty() {
                entry.id.clone()
            } else {
                format!("{}/{}", entry.id, inner.join("/"))
            }
        };
        let report = |left_out: &mut Vec<SetAside>, entity: Entity, reason: &str| {
            left_out.push(SetAside {
                entity: reported(self, entity),
                component: None,
                reason: reason.to_owned(),
            });
        };
        let write = DocumentError::Write;
        // Each record whose entity is lost with its place in the prefab, or held out of the file.
        let mut lost = vec![false; placed.len()];
        let mut differences: BTreeMap<(String, String), Value> = BTreeMap::new();
        for (index, (record, entity)) in placed.iter().enumerate() {
            let path = path_of(&record.path);
            let parent = record.parent.map(|parent| parent as usize);
            let place_gone =
                parent.is_some_and(|parent| entities[parent].is_none() || lost[parent]);
            let Some(entity) = *entity else {
                lost[index] = place_gone;
                if !place_gone {
                    sections.removed.insert(path);
                }
                continue;
            };
            if place_gone {
                lost[index] = true;
                report(
                    left_out,
                    entity,
                    "its place in the prefab is removed: it is lost when the file is spawned",
                );
                continue;
            }
            if let Some(parent) = parent
                && self.parent(entity) != entities[parent]
            {
                report(
                    left_out,
                    entity,
                    "it moved out of its place in the prefab, which a scene file cannot say: \
                     written in its place",
                );
            }
            // A placement's entities keep its prefab's order, before the file's.
            let mut last = None;
            let mut file_child_seen = false;
            let ordered = self.children(entity).all(|child| {
                if self.left_unsaved(root, child) {
                    return true;
                }
                match index_of.get(&child) {
                    Some(&at) if placed[at].0.parent == Some(index as u32) => {
                        let in_order = !file_child_seen && last.is_none_or(|last| last < at);
                        last = Some(at);
                        in_order
                    }
                    _ => {
                        file_child_seen = true;
                        true
                    }
                }
            });
            if !ordered {
                report(
                    left_out,
                    entity,
                    "the order of its children, which a scene file cannot say: written in the \
                     order of its prefab, then of the file",
                );
            }
            let names: Vec<String> = record
                .components
                .iter()
                .map(|(name, _)| name.clone())
                .chain(
                    (index == 0 && !record.components.iter().any(|(name, _)| name == "Name"))
                        .then(|| "Name".to_owned()),
                )
                .collect();
            for name in &names {
                let base = expected(record, index, Some(&entry.id), &entities, name)
                    .expect("a component of the record");
                match self.component_value(entity, name) {
                    None => {
                        sections
                            .removed_components
                            .entry(path.clone())
                            .or_default()
                            .insert(name.clone());
                    }
                    // As its author wrote it: the prefab's, or the scene's override, below.
                    Some(_) if self.drives(entity, name) => {}
                    Some(value) => {
                        if let Some(part) = self
                            .registry
                            .difference(name, &base, &value, false)
                            .map_err(write)?
                        {
                            differences.insert((path.clone(), name.clone()), part);
                        }
                    }
                }
            }
            for name in self.component_names_sorted(entity) {
                if !names.iter().any(|held| held == name)
                    && let Some(value) = self.component_value(entity, name)
                {
                    sections
                        .added
                        .insert((path.clone(), name.to_owned()), value);
                }
            }
        }
        // The overrides that the scene wrote, kept where they still say something.
        let at = |inner: &[String]| placed.iter().position(|(record, _)| record.path == inner);
        let authored = &placement.overrides;
        for inner in &authored.removed {
            match at(inner) {
                // Set aside, or removed since, the topmost or not.
                None => {
                    sections.removed.insert(path_of(inner));
                }
                Some(index) if entities[index].is_none() && !lost[index] => {
                    sections.removed.insert(path_of(inner));
                }
                Some(_) => {}
            }
        }
        for (inner, names) in &authored.removed_components {
            let held = at(inner);
            for name in names {
                let kept = match held {
                    None => true,
                    Some(index) => match entities[index] {
                        Some(entity) => {
                            self.component_value(entity, name).is_none()
                                && !placed[index]
                                    .0
                                    .components
                                    .iter()
                                    .any(|(held, _)| held == name)
                        }
                        None => false,
                    },
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
                Some(index) => {
                    if let Some(entity) = entities[index]
                        && !lost[index]
                        && let Some(now) = self.component_value(entity, name)
                    {
                        differences.remove(&key);
                        let value = if self.drives(entity, name) {
                            self.authored(value, scene)
                        } else {
                            now
                        };
                        sections.added.insert(key, value);
                    }
                }
            }
        }
        for (inner, name, part) in &authored.changed {
            let key = (path_of(inner), name.clone());
            if sections.added.contains_key(&key) {
                continue;
            }
            let written = self.authored(part, scene);
            let Some(index) = at(inner) else {
                // The prefab no longer has its entity: set aside, as written.
                differences.insert(key, written);
                continue;
            };
            let Some(entity) = entities[index].filter(|_| !lost[index]) else {
                // Removed since: the removal says it.
                continue;
            };
            let base = expected(placed[index].0, index, Some(&entry.id), &entities, name);
            match (base, self.component_value(entity, name)) {
                // The prefab no longer has its component: set aside, as written.
                (None, None) => {
                    differences.insert(key, written);
                }
                // Removed since, or added since whole: the removal or the addition says it.
                (Some(_), None) | (None, Some(_)) => {}
                // As its author wrote it.
                (Some(_), Some(_)) if self.drives(entity, name) => {
                    differences.insert(key, written);
                }
                (Some(base), Some(now)) => {
                    let mut overlaid = base.clone();
                    if self.registry.overlay(name, &mut overlaid, &written).is_ok() {
                        let shape = match differences.get(&key) {
                            Some(difference) => shape_union(part, difference),
                            None => part.clone(),
                        };
                        differences.insert(key, part_like(&now, &shape));
                    } else if self
                        .registry
                        .difference(name, &base, &now, false)
                        .map_err(write)?
                        .is_none()
                    {
                        // It no longer fits the prefab, which the value still holds: set aside, as
                        // written.
                        differences.insert(key, written);
                    } else {
                        left_out.push(SetAside {
                            entity: reported(self, entity),
                            component: Some(name.clone()),
                            reason: "the override that its scene wrote no longer fits the \
                                     prefab, and the value changed since: written as it is now"
                                .to_owned(),
                        });
                    }
                }
            }
        }
        sections.changed.extend(differences);
        Ok(())
    }

    /// The value of `entity`'s component named `name` as its record in `scene`, the scene whose key
    /// begins its identity, holds it, its references made references to the world's entities;
    /// `None` if no record of it holds one: its author wrote none. `records` are the scene's, by
    /// their paths.
    fn authored_component(
        &self,
        entity: Entity,
        name: &str,
        scene: Option<(&str, &ResolvedScene)>,
        records: &HashMap<&[String], &Record>,
    ) -> Option<Value> {
        let (key, _) = scene?;
        let id = self.persistent_id(entity)?;
        let (first, path) = id.parts().split_first()?;
        if first != key {
            return None;
        }
        let record = records.get(path)?;
        let (_, value) = record.components.iter().find(|(held, _)| held == name)?;
        Some(self.authored(value, scene))
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

/// The value of the component named `name` that `record`, a scene's record at `index`, gives the
/// entity spawned from it, its references made references to `entities`; the root is named
/// `root_name` unless the scene names it: a placement's ID in a file, a scene's key, none for a
/// placement made while the world runs. `None` if the record gives no such component.
pub(crate) fn expected(
    record: &Record,
    index: usize,
    root_name: Option<&str>,
    entities: &[Option<Entity>],
    name: &str,
) -> Option<Value> {
    match record.components.iter().find(|(held, _)| held == name) {
        Some((_, value)) => Some(with_handles(value, entities)),
        None if index == 0 && name == "Name" => root_name.map(|id| Name(id.to_owned()).to_value()),
        None => None,
    }
}

/// The bits of `entity`'s handle, as a value holds them.
pub(crate) fn handle_bits(entity: Entity) -> u64 {
    match entity.to_value() {
        Value::Entity(bits) => bits,
        _ => 0,
    }
}

/// `value`, a resolved scene's, its references to the scene's records made references to the
/// entities that `entities` gives them, in the records' order, or to no entity.
pub(crate) fn with_handles(value: &Value, entities: &[Option<Entity>]) -> Value {
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
                let prefab = prefab.filter(|name| !name.is_empty());
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
