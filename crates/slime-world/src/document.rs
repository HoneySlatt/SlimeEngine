//! The world's documents: a save, what differs between a world and the world that a game makes at
//! the start of a run, and that difference applied again to a world made so
//! (`docs/adr/0087-the-worlds-documents.md`).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::mem;

use slime_reflect::{Reflect, ReflectError, Registry, Value};
use slime_serialize::{Composer, Document, Mode, ReadError, References};

use crate::components::Unsaved;
use crate::entity::Entity;
use crate::identity::PersistentId;
use crate::world::{Identity, World};

/// What the game says of the save: its keys and texts.
const ABOUT: &str = "about";
/// The count of the entities created outside scenes.
const NEXT_CREATED: &str = "next_created";
/// The scenes spawned, by their keys: their names and contents.
const SCENES: &str = "scenes";
/// The entities and the components removed.
const REMOVED: &str = "removed";
/// The entities given another parent, or none.
const PARENTS: &str = "parents";
/// The entities created outside scenes, whole.
const CREATED: &str = "created";
/// The components added, whole.
const ADDED: &str = "added";
/// What changed of the components, in parts.
const CHANGED: &str = "changed";

/// Why a world cannot be saved, or a save loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DocumentError {
    /// A value that the world holds cannot be written, as `slime-serialize` says: a number out of
    /// its field's range, a document larger or deeper than a reader reads.
    #[error("cannot be saved: {0}")]
    Write(#[source] ReflectError),
    /// The save cannot be read: it is not TOML, is of a later version, or is not a save.
    #[error("the save cannot be read")]
    Read(#[source] ReadError),
    /// The save creates an entity under an identity that is not one of an entity created outside a
    /// scene, `created/<number>`, the number written without a sign or a leading zero and at most
    /// `i64::MAX`.
    #[error(
        "the save creates `{0}`, which is not the identity of an entity created outside a scene"
    )]
    NotCreated(String),
    /// The save creates an entity under an identity that an entity of the world has: the game made
    /// more entities at the start of its run than when the save was made.
    #[error("the save creates `{0}`, an identity that an entity of the world has already")]
    IdentityInUse(PersistentId),
}

/// What loading a save left aside, and why: what the save holds of entities or components that
/// the world, made from the scenes as they are now, no longer has.
#[derive(Debug, Default)]
pub struct Loaded {
    /// The keys of the scenes that the save was made on whose content changed since, or that the
    /// world no longer spawns.
    pub scenes_changed: Vec<String>,
    /// The differences that could not be applied, or only in part: a reference to an entity that
    /// the world does not have refers to no entity.
    pub set_aside: Vec<SetAside>,
    /// What the save holds that this program does not know, left out: fields, types.
    pub warnings: Vec<ReadError>,
}

/// A difference of a save that could not be applied, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct SetAside {
    /// The entity it is about, by its persistent identity.
    pub entity: String,
    /// The component it is about, if it is about one.
    pub component: Option<String>,
    /// Why it was set aside.
    pub reason: String,
}

/// What a save writes for a reference to no entity: one destroyed, or one that it leaves out.
const NO_ENTITY: &str = "";

/// The largest number of an entity created outside a scene that a save gives, and of the count
/// it keeps: `i64::MAX`, so that a world's count, which goes on from there, never runs out.
const LARGEST_CREATED: u64 = i64::MAX as u64;

/// Names entities by their persistent identities, in a world's documents, and keeps the names
/// read that the world has no entity of.
struct Names<'w> {
    world: &'w World,
    /// The names read since they were last taken that no entity of the world has.
    unknown: RefCell<Vec<String>>,
}

impl<'w> Names<'w> {
    fn new(world: &'w World) -> Self {
        Self {
            world,
            unknown: RefCell::new(Vec::new()),
        }
    }
}

impl References for Names<'_> {
    /// An entity destroyed, or that the save leaves out, is no entity, [`NO_ENTITY`].
    fn name(&self, entity: u64) -> Option<String> {
        let world = self.world;
        let id = Entity::from_value(&Value::Entity(entity))
            .ok()
            .filter(|&entity| !world.unsaved(entity))
            .and_then(|entity| world.persistent_id(entity));
        Some(id.map_or_else(|| NO_ENTITY.to_owned(), |id| id.to_string()))
    }

    /// No entity, or one that the world does not have, is a handle that no world gives out.
    fn entity(&self, name: &str) -> Option<u64> {
        let found = name.parse().ok().and_then(|id| self.world.find(&id));
        if found.is_none() && name != NO_ENTITY {
            self.unknown.borrow_mut().push(name.to_owned());
        }
        match found.unwrap_or(Entity::NONE).to_value() {
            Value::Entity(bits) => Some(bits),
            _ => None,
        }
    }
}

impl World {
    /// What differs between this world and `start`, the world that the game makes at the start of a
    /// run, from the same scenes and as it prepares them, as the text of a save
    /// (`docs/adr/0087-the-worlds-documents.md`): the entities removed; those created while the
    /// world ran, whole; the components added, whole, and removed; the parents changed; and what
    /// changed of the other components, in parts. Entities are matched by their persistent
    /// identities: one is created since the start if `start` has no entity of its identity. Those
    /// that hold [`Unsaved`], or whose ancestor does, are left out, as are the fields marked
    /// transient: an entity of `start` that the world leaves out is not removed, and loading the save
    /// keeps it as `start` has it, out of its parent if the save removes that. A reference to an
    /// entity is its persistent identity, and a reference to an entity destroyed or left out is
    /// `""`, no entity. The save also holds the scenes that the world spawned, so that loading it
    /// says which changed since, and the count of the entities created outside scenes. Its first
    /// section, after the versions of the types past version 1 if it holds some, is `about`, what
    /// the game says of it, keys and texts that the engine does not read, such as what the run was
    /// made on, which [`World::about`] reads without loading the save; none for an empty `about`.
    ///
    /// The same worlds give the same bytes: entities in the order of their identities, components
    /// in the order of their names. Its cost grows in proportion with the entities of the two worlds
    /// and with what differs: measured in a release build on the owner's machine, 10 000 entities
    /// created, each with a name and a transform, save in 0.22 s into 2.9 MB of text, and 40 000 in
    /// four times as long. A save holds at most [`slime_serialize::MAX_SIZE`], about 57 000 such
    /// entities.
    ///
    /// # Errors
    ///
    /// [`DocumentError::Write`] if a value cannot be written, a number out of its field's range, or
    /// the save would be larger or deeper than a reader reads ([`slime_serialize::MAX_SIZE`],
    /// [`slime_serialize::MAX_DEPTH`]).
    pub fn save(
        &self,
        start: &World,
        about: &BTreeMap<String, String>,
    ) -> Result<String, DocumentError> {
        let names = Names::new(self);
        let mut save = Composer::new(&self.registry)
            .naming(&names)
            .without_transient();
        let mut put = |place: &[&str], ty: &str, value: Value| {
            save.put(place, ty, &value).map_err(DocumentError::Write)
        };
        for (key, text) in about {
            put(&[ABOUT, key], "String", Value::Text(text.clone()))?;
        }
        put(&[NEXT_CREATED], "u64", Value::UInt(self.next_created))?;
        for (key, scene) in &self.scenes {
            put(
                &[SCENES, key, "name"],
                "String",
                Value::Text(scene.name.clone()),
            )?;
            let content = Value::Text(format!("{:016x}", scene.content));
            put(&[SCENES, key, "content"], "String", content)?;
        }
        // An entity is created since the start if the start has no entity of its identity, saved or
        // not; removed if the start saves one and the world has none.
        let now = self.saved_entities();
        let named_now = self.named_entities();
        let before = start.saved_entities();
        let named_before = start.named_entities();
        let removed: Vec<Value> = before
            .keys()
            .filter(|id| !named_now.contains_key(*id))
            .map(|id| Value::Text(id.to_string()))
            .collect();
        if !removed.is_empty() {
            put(&[REMOVED, "entities"], "Vec<String>", Value::List(removed))?;
        }
        let parent_of = |world: &World, entity| {
            world
                .parent(entity)
                .and_then(|parent| world.persistent_id(parent))
        };
        for (id, &entity) in &now {
            let key = id.to_string();
            let Some(&old) = named_before.get(id) else {
                continue;
            };
            let gone: Vec<Value> = start
                .component_names_sorted(old)
                .into_iter()
                .filter(|name| self.component_value(entity, name).is_none())
                .map(|name| Value::Text(name.to_owned()))
                .collect();
            if !gone.is_empty() {
                put(
                    &[REMOVED, "components", &key],
                    "Vec<String>",
                    Value::List(gone),
                )?;
            }
            let parent = parent_of(self, entity);
            if parent != parent_of(start, old) {
                put(&[PARENTS, &key], "Option<String>", text_option(parent))?;
            }
        }
        drop(put);
        for (id, &entity) in &now {
            let key = id.to_string();
            match named_before.get(id) {
                None => {
                    let parent = text_option(parent_of(self, entity));
                    save.put(&[CREATED, &key, "parent"], "Option<String>", &parent)
                        .map_err(DocumentError::Write)?;
                    for name in self.component_names_sorted(entity) {
                        let Some(value) = self.component_value(entity, name) else {
                            continue;
                        };
                        save.put(&[CREATED, &key, "components", name], name, &value)
                            .map_err(DocumentError::Write)?;
                    }
                }
                Some(&old) => {
                    for name in self.component_names_sorted(entity) {
                        let Some(value) = self.component_value(entity, name) else {
                            continue;
                        };
                        match start.component_value(old, name) {
                            None => save.put(&[ADDED, &key, name], name, &value),
                            Some(was) => {
                                let part = self
                                    .registry
                                    .difference(name, &was, &value, false)
                                    .map_err(DocumentError::Write)?;
                                match part {
                                    Some(part) => {
                                        save.put_part(&[CHANGED, &key, name], name, &part)
                                    }
                                    None => Ok(()),
                                }
                            }
                        }
                        .map_err(DocumentError::Write)?;
                    }
                }
            }
        }
        save.finish().map_err(DocumentError::Write)
    }

    /// Applies the save `text` to this world, which the game has made as at the start of a run,
    /// from the scenes as they are now ([`World::save`]): the entities that it creates are spawned
    /// under their identities, the parents set, an entity whose parent the world does not have out
    /// of its own, the entities removed despawned, the entities saved under them that the save does
    /// not remove taken out of them first, the components removed, added and changed, and the count
    /// of the entities created outside scenes kept. A save is read leniently: what this program does
    /// not know is left out, with a warning. A difference whose entity or component the world no
    /// longer has, or that the component's type refuses, is set aside, and the rest applied; so is a
    /// reference to an entity that the world no longer has, which refers to no entity then, and a
    /// parent that the world does not have. The scenes that changed since the save are named. Loading
    /// costs about two and a half times as much as saving, half of it parsing the text: 0.54 s for
    /// the 10 000 entities of [`World::save`]'s measure.
    ///
    /// # Errors
    ///
    /// [`DocumentError::Read`] if the save is not one: not TOML, of a later version of the format or
    /// of a type, too large; [`DocumentError::NotCreated`] if it creates an entity under an identity
    /// that a world does not give to the entities it creates; [`DocumentError::IdentityInUse`] if it
    /// creates one under an identity that an entity of the world has. The world is unchanged then.
    pub fn load(&mut self, text: &str) -> Result<Loaded, DocumentError> {
        // Set apart while the save is read, so that the entities that it creates are spawned while
        // the document, which reads through the registry, is open: the save is parsed once.
        let read = {
            let registry = mem::take(&mut self.registry);
            let apart = RegistryApart {
                world: self,
                registry,
            };
            Document::parse(&apart.registry, text, Mode::Lenient)
                .map_err(DocumentError::Read)
                .and_then(|mut document| apart.world.read(&mut document))
        };
        let (save, mut loaded) = read?;
        self.apply(save, &mut loaded);
        Ok(loaded)
    }

    /// What the save `text` says of itself, as the game gave it to [`World::save`]: its keys and
    /// texts, read without a world and without loading the save, so that a game tells whether a
    /// save is one it can load, or lists its saves. The text is parsed whole, as loading parses it.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    ///
    /// use slime_world::World;
    ///
    /// let world = World::new();
    /// let about = BTreeMap::from([("made on".to_owned(), "the first zone".to_owned())]);
    /// let save = world.save(&World::new(), &about).expect("saved");
    /// assert_eq!(World::about(&save).expect("a save"), about);
    /// ```
    ///
    /// # Errors
    ///
    /// [`DocumentError::Read`] if the text is not a document: larger than a reader reads, not TOML,
    /// of a later version of the format; or if what it says of itself is not text. It reads nothing
    /// else, and so accepts a save that [`World::load`] would refuse, of a later version of a type.
    pub fn about(text: &str) -> Result<BTreeMap<String, String>, DocumentError> {
        let mut registry = Registry::new();
        registry.register::<String>();
        let mut document =
            Document::parse(&registry, text, Mode::Lenient).map_err(DocumentError::Read)?;
        let mut about = BTreeMap::new();
        for key in document.keys(&[ABOUT]) {
            let read = document
                .read(&[ABOUT, &key], "String", None)
                .map_err(DocumentError::Read)?;
            if let Some(Value::Text(text)) = read {
                about.insert(key, text);
            }
        }
        Ok(about)
    }

    /// The first difference between what this world and `other` would save, said in a sentence, or
    /// `None` if there is none: an entity that one of them saves and the other does not have, by its
    /// persistent identity; and of the entities that both save, another parent, other components,
    /// or a component's value, floats compared by their bits and references to entities by the
    /// identities they refer to. The entities that hold [`Unsaved`], or whose ancestor does, and the
    /// fields marked transient are left out, as a save leaves them out. It tells whether a world
    /// loaded from a save is the world saved, or whether two runs went the same way.
    ///
    /// ```
    /// use slime_world::{Name, World};
    ///
    /// let (mut a, mut b) = (World::new(), World::new());
    /// let (door, gate) = (a.spawn(), b.spawn());
    /// a.insert(door, Name("door".to_owned()));
    /// b.insert(gate, Name("door".to_owned()));
    /// assert_eq!(a.first_difference(&b), None);
    /// b.insert(gate, Name("gate".to_owned()));
    /// assert!(a.first_difference(&b).is_some());
    /// ```
    pub fn first_difference(&self, other: &World) -> Option<String> {
        let (ours, theirs) = (self.saved_entities(), other.saved_entities());
        let (our_names, their_names) = (self.named_entities(), other.named_entities());
        if let Some(id) = ours.keys().find(|id| !their_names.contains_key(id)) {
            return Some(format!("`{id}` is only in the first world"));
        }
        if let Some(id) = theirs.keys().find(|id| !our_names.contains_key(id)) {
            return Some(format!("`{id}` is only in the second world"));
        }
        let parent = |world: &World, entity| {
            world
                .parent(entity)
                .and_then(|parent| world.persistent_id(parent))
        };
        let (our_references, their_references) = (Names::new(self), Names::new(other));
        for (id, &entity) in ours.iter() {
            let Some(&other_entity) = theirs.get(id) else {
                continue;
            };
            let parents = (parent(self, entity), parent(other, other_entity));
            if parents.0 != parents.1 {
                return Some(format!(
                    "`{id}` has the parent {:?} in the first world, and {:?} in the second",
                    parents.0.map(|id| id.to_string()),
                    parents.1.map(|id| id.to_string())
                ));
            }
            let names = self.component_names_sorted(entity);
            let other_names = other.component_names_sorted(other_entity);
            if names != other_names {
                return Some(format!(
                    "`{id}` holds {names:?} in the first world, and {other_names:?} in the second"
                ));
            }
            for name in names {
                let values = (
                    self.component_value(entity, name)
                        .map(|value| named(value, &our_references)),
                    other
                        .component_value(other_entity, name)
                        .map(|value| named(value, &their_references)),
                );
                let same = match &values {
                    (Some(a), Some(b)) => {
                        matches!(self.registry.difference(name, a, b, false), Ok(None))
                    }
                    _ => false,
                };
                if !same {
                    return Some(format!(
                        "`{id}`: its `{name}` is {:?} in the first world, and {:?} in the second",
                        values.0, values.1
                    ));
                }
            }
        }
        None
    }

    /// Spawns the entities that the save `document` creates, then reads the rest of it; the world as
    /// it was if the save cannot be read.
    fn read(&mut self, document: &mut Document<'_>) -> Result<(Save, Loaded), DocumentError> {
        let mut loaded = Loaded::default();
        let next_created = self.next_created;
        // Created first, so that the values read next can refer to them.
        let created = self.create(document)?;
        match self.read_save(document, &mut loaded) {
            Ok(save) => Ok((save, loaded)),
            Err(error) => {
                for entity in created {
                    self.despawn(entity);
                }
                // Numbers from the count's again, larger than every number held.
                self.created.forget_destroyed(&self.entities);
                self.next_created = next_created;
                Err(error)
            }
        }
    }

    /// Spawns the entities that the save `document` creates, under their identities, and returns
    /// them.
    fn create(&mut self, document: &Document<'_>) -> Result<Vec<Entity>, DocumentError> {
        let keys = document.keys(&[CREATED]);
        let mut numbers = Vec::new();
        for key in keys {
            let number = key
                .parse::<PersistentId>()
                .ok()
                .and_then(|id| id.created_number())
                .filter(|&number| number <= LARGEST_CREATED)
                .ok_or_else(|| DocumentError::NotCreated(key.clone()))?;
            if self.created.find(number, &self.entities).is_some() {
                return Err(DocumentError::IdentityInUse(PersistentId::created(number)));
            }
            numbers.push(number);
        }
        let created: Vec<(u64, Entity)> = numbers
            .into_iter()
            .map(|number| {
                let entity = self.spawn_bare();
                self.nodes[entity.index() as usize].identity = Identity::Created(number);
                // The count stays larger than every number given, whatever the save's says.
                self.next_created = self.next_created.max(number + 1);
                (number, entity)
            })
            .collect();
        self.created.extend(created.iter().copied());
        Ok(created.into_iter().map(|(_, entity)| entity).collect())
    }

    /// Reads all that the save `document` holds but the entities it creates, which are spawned
    /// already.
    fn read_save(
        &self,
        document: &mut Document<'_>,
        loaded: &mut Loaded,
    ) -> Result<Save, DocumentError> {
        let names = Names::new(self);
        let mut save = Save {
            next_created: match document.read(&[NEXT_CREATED], "u64", None) {
                // Larger than any a world saves, so that the count never runs out.
                Ok(Some(Value::UInt(next))) => next.min(LARGEST_CREATED),
                Ok(_) => 1,
                Err(error) => return Err(DocumentError::Read(error)),
            },
            ..Save::default()
        };
        for key in document.keys(&[SCENES]) {
            let content = document
                .read(&[SCENES, &key, "content"], "String", None)
                .map_err(DocumentError::Read)?;
            let same = match (self.scenes.get(&key), content) {
                (Some(scene), Some(Value::Text(content))) => {
                    format!("{:016x}", scene.content) == content
                }
                _ => false,
            };
            if !same {
                loaded.scenes_changed.push(key);
            }
        }
        let mut aside = |entity: &str, component: Option<&str>, reason: String| {
            loaded.set_aside.push(SetAside {
                entity: entity.to_owned(),
                component: component.map(str::to_owned),
                reason,
            });
        };
        if let Some(Value::List(entities)) = document
            .read(&[REMOVED, "entities"], "Vec<String>", None)
            .map_err(DocumentError::Read)?
        {
            save.removed = texts(entities);
        }
        for key in document.keys(&[REMOVED, "components"]) {
            if let Some(Value::List(names)) = document
                .read(&[REMOVED, "components", &key], "Vec<String>", None)
                .map_err(DocumentError::Read)?
            {
                save.removed_components.push((key, texts(names)));
            }
        }
        for key in document.keys(&[PARENTS]) {
            if let Some(parent) = document
                .read(&[PARENTS, &key], "Option<String>", None)
                .map_err(DocumentError::Read)?
            {
                save.parents.push((key, option_text(&parent)));
            }
        }
        // A component read, set aside if it cannot be, its references to entities that the world
        // does not have reported.
        let mut take = |key: &str, name: &str, read: Result<Option<Value>, ReadError>| {
            let value = match read {
                Ok(value) => value,
                Err(error) => {
                    aside(key, Some(name), error.to_string());
                    None
                }
            };
            for unknown in names.unknown.take() {
                aside(
                    key,
                    Some(name),
                    format!(
                        "it refers to `{unknown}`, which the world does not have: to no entity now"
                    ),
                );
            }
            value
        };
        for key in document.keys(&[CREATED]) {
            if let Some(parent) = document
                .read(&[CREATED, &key, "parent"], "Option<String>", None)
                .map_err(DocumentError::Read)?
            {
                save.parents.push((key.clone(), option_text(&parent)));
            }
            for name in document.keys(&[CREATED, &key, "components"]) {
                let read =
                    document.read(&[CREATED, &key, "components", &name], &name, Some(&names));
                if let Some(value) = take(&key, &name, read) {
                    save.added.push((key.clone(), name, value));
                }
            }
        }
        for key in document.keys(&[ADDED]) {
            for name in document.keys(&[ADDED, &key]) {
                let read = document.read(&[ADDED, &key, &name], &name, Some(&names));
                if let Some(value) = take(&key, &name, read) {
                    save.added.push((key.clone(), name, value));
                }
            }
        }
        for key in document.keys(&[CHANGED]) {
            for name in document.keys(&[CHANGED, &key]) {
                let read = document.read_part(&[CHANGED, &key, &name], &name, Some(&names));
                if let Some(part) = take(&key, &name, read) {
                    save.changed.push((key.clone(), name, part));
                }
            }
        }
        loaded.warnings.extend(document.warnings());
        Ok(save)
    }

    /// Applies what a save holds, setting aside what cannot be.
    fn apply(&mut self, save: Save, loaded: &mut Loaded) {
        let mut aside = |entity: &str, component: Option<&str>, reason: &str| {
            loaded.set_aside.push(SetAside {
                entity: entity.to_owned(),
                component: component.map(str::to_owned),
                reason: reason.to_owned(),
            });
        };
        const GONE: &str = "the world has no entity of this identity";
        // Found before any is despawned: removing an entity removes its descendants, which the
        // save names too.
        let mut removed = Vec::new();
        for key in &save.removed {
            match self.entity_named(key) {
                Some(entity) => removed.push(entity),
                None => aside(key, None, GONE),
            }
        }
        // Every entity given another parent is taken out first, then put where the save says: one
        // at a time, a child could not take its parent's place, as its parent would be its
        // descendant still.
        let mut moved = Vec::new();
        for (key, parent) in &save.parents {
            let Some(entity) = self.entity_named(key) else {
                aside(key, None, GONE);
                continue;
            };
            let parent = match parent {
                Some(parent) => match self.entity_named(parent) {
                    Some(parent) => Some(parent),
                    // Out of its parent all the same, so that it outlives it if the save removes it.
                    None => {
                        aside(
                            key,
                            None,
                            &format!("the world has no parent `{parent}`: it has none now"),
                        );
                        None
                    }
                },
                None => None,
            };
            moved.push((key, entity, parent));
        }
        for &(_, entity, _) in &moved {
            // Alive, so taking it out of its parent cannot fail.
            let _ = self.set_parent(entity, None);
        }
        for (key, entity, parent) in moved {
            if parent.is_some()
                && let Err(error) = self.set_parent(entity, parent)
            {
                aside(key, None, &error.to_string());
            }
        }
        // After the parents, so that an entity moved out of one removed stays. An entity that the
        // save neither removes nor moves, but that the world left out since the start under
        // another, stays too, out of the one removed: as the start has it, as far as it can be.
        let removing: HashSet<Entity> = removed.iter().copied().collect();
        for &entity in &removed {
            let kept: Vec<Entity> = self
                .children(entity)
                .iter()
                .copied()
                .filter(|child| !removing.contains(child) && !self.unsaved(*child))
                .collect();
            for child in kept {
                // Alive, so taking it out of its parent cannot fail.
                let _ = self.set_parent(child, None);
            }
        }
        for entity in removed {
            self.despawn(entity);
        }
        for (key, names) in &save.removed_components {
            for name in names {
                let removed = self
                    .entity_named(key)
                    .and_then(|entity| self.remove_component_value(entity, name));
                if removed.is_none() {
                    aside(key, Some(name), "the entity holds no such component");
                }
            }
        }
        for (key, name, value) in save.added {
            let Some(entity) = self.entity_named(&key) else {
                aside(&key, Some(&name), GONE);
                continue;
            };
            if let Err(error) = self.insert_component_value(entity, &name, value) {
                aside(&key, Some(&name), &error.to_string());
            }
        }
        for (key, name, part) in save.changed {
            let Some(entity) = self.entity_named(&key) else {
                aside(&key, Some(&name), GONE);
                continue;
            };
            let Some(mut value) = self.component_value(entity, &name) else {
                aside(&key, Some(&name), "the entity holds no such component");
                continue;
            };
            let changed = self
                .registry
                .overlay(&name, &mut value, &part)
                .map_err(|error| error.to_string())
                .and_then(|()| {
                    self.set_component_value(entity, &name, value)
                        .map_err(|error| error.to_string())
                });
            if let Err(reason) = changed {
                aside(&key, Some(&name), &reason);
            }
        }
        self.next_created = self.next_created.max(save.next_created);
    }

    /// The entity of the persistent identity that `text` writes, if one is alive.
    fn entity_named(&self, text: &str) -> Option<Entity> {
        self.find(&text.parse().ok()?)
    }

    /// The entities that have a persistent identity, by their identities.
    fn named_entities(&self) -> BTreeMap<PersistentId, Entity> {
        self.entities()
            .filter_map(|entity| Some((self.persistent_id(entity)?, entity)))
            .collect()
    }

    /// The entities that a save holds, by their persistent identities: all but those that hold
    /// [`Unsaved`], or whose ancestor does.
    fn saved_entities(&self) -> BTreeMap<PersistentId, Entity> {
        let mut entities = self.named_entities();
        entities.retain(|_, &mut entity| !self.unsaved(entity));
        entities
    }

    /// Whether the entity, or one of its ancestors, holds [`Unsaved`].
    fn unsaved(&self, entity: Entity) -> bool {
        let mut next = Some(entity);
        while let Some(entity) = next {
            if self.get::<Unsaved>(entity).is_some() {
                return true;
            }
            next = self.parent(entity);
        }
        false
    }

    /// The names of the types of the components that `entity` holds, in their order.
    fn component_names_sorted(&self, entity: Entity) -> Vec<&str> {
        let mut names = self.component_names(entity);
        names.sort_unstable();
        names
    }
}

/// `value` with its references to entities as the persistent identities that `names` gives them, as
/// text: what a save holds of them.
fn named(value: Value, names: &Names) -> Value {
    match value {
        Value::Entity(bits) => Value::Text(names.name(bits).unwrap_or_default()),
        Value::Struct(fields) => Value::Struct(
            fields
                .into_iter()
                .map(|(name, value)| (name, named(value, names)))
                .collect(),
        ),
        Value::Enum(variant, fields) => Value::Enum(
            variant,
            fields
                .into_iter()
                .map(|(name, value)| (name, named(value, names)))
                .collect(),
        ),
        Value::Option(held) => Value::Option(held.map(|held| Box::new(named(*held, names)))),
        Value::List(elements) => Value::List(
            elements
                .into_iter()
                .map(|element| named(element, names))
                .collect(),
        ),
        other => other,
    }
}

/// A world's registry, set apart while a save is read, and given back to the world however reading
/// ends, a panic of a type's upgrade or check included.
struct RegistryApart<'w> {
    world: &'w mut World,
    registry: Registry,
}

impl Drop for RegistryApart<'_> {
    fn drop(&mut self) {
        self.world.registry = mem::take(&mut self.registry);
    }
}

/// What a save holds, read, but the entities it creates.
#[derive(Default)]
struct Save {
    next_created: u64,
    removed: Vec<String>,
    removed_components: Vec<(String, Vec<String>)>,
    /// Each entity given another parent, the created ones included, and its parent, if it has one.
    parents: Vec<(String, Option<String>)>,
    /// The components added, of the entities the world had and of those it created: the entity,
    /// the component and its value.
    added: Vec<(String, String, Value)>,
    /// What changed of the components: the entity, the component and the part.
    changed: Vec<(String, String, Value)>,
}

/// The persistent identity `id`, if there is one, as an option of text.
fn text_option(id: Option<PersistentId>) -> Value {
    Value::Option(id.map(|id| Box::new(Value::Text(id.to_string()))))
}

/// The text that `value`, an option of text, holds, if it holds one.
fn option_text(value: &Value) -> Option<String> {
    match value {
        Value::Option(Some(text)) => match &**text {
            Value::Text(text) => Some(text.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The texts of a list of text.
fn texts(values: Vec<Value>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| match value {
            Value::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use glam::{DVec3, Mat4, Vec3};
    use slime_assets::{AssetId, CookedScene, SceneNode};

    use super::*;
    use crate::{Hidden, Meshes, Name, Transform};

    fn node(name: &str, parent: Option<u32>, at: [f32; 3]) -> SceneNode {
        SceneNode {
            name: name.to_owned(),
            parent,
            transform: Mat4::from_translation(Vec3::from(at)).to_cols_array(),
            meshes: vec![AssetId::from_u128(7)],
            light: None,
            extras: Vec::new(),
        }
    }

    fn scene() -> CookedScene {
        CookedScene {
            name: "yard".to_owned(),
            nodes: vec![
                node("gate", None, [0.0, 0.0, 0.0]),
                node("post", Some(0), [1.0, 0.0, 0.0]),
                node("well", None, [5.0, 0.0, 2.0]),
                node("tree", None, [-3.0, 0.0, 4.0]),
            ],
        }
    }

    /// A world as a game makes it at the start of a run.
    fn start() -> World {
        let mut world = World::new();
        world.spawn_scene("yard", &scene()).expect("spawned");
        world
    }

    fn entity(world: &World, id: &str) -> Entity {
        world
            .find(&id.parse().expect("an identity"))
            .expect("alive")
    }

    /// A run: a value changed, a component added and one removed, an entity removed, one moved to
    /// another parent, two created, one of them for a moment.
    fn played() -> World {
        let mut world = start();
        let well = entity(&world, "yard/well");
        world.insert(well, Transform::from_translation(DVec3::new(6.5, 0.0, 2.0)));
        world.insert(entity(&world, "yard/gate"), Hidden);
        world.remove::<Meshes>(entity(&world, "yard/post"));
        world.despawn(entity(&world, "yard/tree"));
        let post = entity(&world, "yard/post");
        world.set_parent(post, Some(well)).expect("a parent");
        let torch = world.spawn();
        world.insert(torch, Name("torch".to_owned()));
        world.insert(
            torch,
            Transform::from_translation(DVec3::new(0.5, 1.0, 0.0)),
        );
        world.set_parent(torch, Some(well)).expect("a parent");
        let spark = world.spawn();
        world.insert(spark, Unsaved);
        world
    }

    #[test]
    fn a_world_saved_and_loaded_into_its_start_is_the_world_saved() {
        let played = played();
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
        assert!(report.scenes_changed.is_empty() && report.warnings.is_empty());
        assert_eq!(played.first_difference(&loaded), None, "{save}");
        assert_eq!(
            loaded
                .save(&start(), &BTreeMap::new())
                .expect("saved again"),
            save,
            "the same bytes"
        );
        // The entity created for a moment is left out, and its number not given again.
        assert!(
            loaded
                .find(&"created/2".parse().expect("an identity"))
                .is_none()
        );
        assert_eq!(loaded.spawn(), entity(&loaded, "created/3"));
        assert_eq!(
            save,
            r#"next_created = 3

[scenes]
yard = { name = "yard", content = "SCENE" }

[removed]
entities = ["yard/tree"]

[removed.components]
"yard/post" = ["Meshes"]

[parents]
"yard/post" = ["yard/well"]

[created."created/1"]
parent = ["yard/well"]

[created."created/1".components]
Name = "torch"

[created."created/1".components.Transform]
translation = { x = 0.5, y = 1.0, z = 0.0 }
rotation = { x = 0.0, y = 0.0, z = 0.0, w = 1.0 }
scale = 1.0
stretch = { x = 1.0, y = 1.0, z = 1.0 }

[added."yard/gate"]
Hidden = {}

[changed."yard/well".Transform]
translation = { x = 6.5 }
"#
            .replace("SCENE", &format!("{:016x}", played.scenes["yard"].content))
        );
    }

    /// The scene of [`scene`] as a new export makes it: the well moved, the tree gone, the gate
    /// renamed, the post moved and a bench added.
    fn scene_changed() -> CookedScene {
        CookedScene {
            name: "yard".to_owned(),
            nodes: vec![
                node("door", None, [0.0, 0.0, 0.0]),
                node("post", Some(0), [1.5, 0.0, 0.0]),
                node("well", None, [9.0, 0.0, 2.0]),
                node("bench", None, [2.0, 0.0, -2.0]),
            ],
        }
    }

    #[test]
    fn a_save_loaded_on_a_scene_changed_since_applies_what_it_can_and_says_what_it_cannot() {
        let save = played().save(&start(), &BTreeMap::new()).expect("saved");
        let mut world = World::new();
        world
            .spawn_scene("yard", &scene_changed())
            .expect("spawned");
        let report = world.load(&save).expect("loaded");
        assert_eq!(report.scenes_changed, ["yard"]);
        let reported: Vec<(&str, Option<&str>)> = report
            .set_aside
            .iter()
            .map(|aside| (aside.entity.as_str(), aside.component.as_deref()))
            .collect();
        assert_eq!(
            reported,
            [("yard/tree", None), ("yard/gate", Some("Hidden"))],
            "the tree is gone already, the gate renamed"
        );
        // The well keeps what the save changed of it, the post was taken out and moved under it,
        // the torch was created under it, and the bench, new, is there.
        let well = entity(&world, "yard/well");
        assert_eq!(
            world.get::<Transform>(well).map(|at| at.translation),
            Some(DVec3::new(6.5, 0.0, 2.0))
        );
        let post = entity(&world, "yard/post");
        assert_eq!(world.parent(post), Some(well));
        assert!(world.get::<Meshes>(post).is_none());
        assert_eq!(
            world.get::<Transform>(post).map(|at| at.translation),
            Some(DVec3::new(1.5, 0.0, 0.0)),
            "where the scene now puts it, which the save did not change"
        );
        assert_eq!(world.parent(entity(&world, "created/1")), Some(well));
        entity(&world, "yard/bench");
    }

    #[test]
    fn a_save_that_is_not_one_is_refused_and_the_world_left_as_it_was() {
        let save = played().save(&start(), &BTreeMap::new()).expect("saved");
        for (text, problem) in [
            ("next_created = [", "not TOML"),
            ("\"$format\" = 2\n", "newer"),
            ("\"$versions\" = { Transform = 9 }\n", "newer"),
            ("[created.\"yard/gate\"]\nparent = []\n", "not the identity"),
            // Its entity created, then taken back when the count cannot be read.
            (
                "next_created = true\n[created.\"created/7\"]\n",
                "cannot be read",
            ),
        ] {
            let mut world = start();
            let error = world.load(text).expect_err(text);
            assert!(
                format!(
                    "{error}: {}",
                    std::error::Error::source(&error).map_or(String::new(), ToString::to_string)
                )
                .contains(problem),
                "{text}: {error:?}"
            );
            assert_eq!(
                world.first_difference(&start()),
                None,
                "{text}: the world as it was"
            );
            let next = world.spawn();
            assert_eq!(
                world.persistent_id(next),
                Some(PersistentId::created(1)),
                "{text}: the count as it was"
            );
        }
        // A save whose count is lower than a number it gives: the world's stays larger.
        let mut world = start();
        world
            .load("next_created = 2\n[created.\"created/5\"]\n")
            .expect("loaded");
        let next = world.spawn();
        assert_eq!(world.persistent_id(next), Some(PersistentId::created(6)));
        // A second load creates its entities under identities that the first gave already.
        let mut world = start();
        world.load(&save).expect("loaded");
        let error = world.load(&save).expect_err("twice");
        assert!(matches!(error, DocumentError::IdentityInUse(id) if id.to_string() == "created/1"));
        // Truncated anywhere, a save gives an error or loads what it holds, and never panics.
        for end in (0..save.len()).filter(|&end| save.is_char_boundary(end)) {
            let _ = start().load(&save[..end]);
        }
    }

    /// A component that refers to another entity, and holds what saves leave out.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Rope")]
    struct Rope {
        to: Entity,
        #[reflect(transient, default)]
        swing: f32,
    }
    impl crate::Component for Rope {}

    #[test]
    fn references_to_entities_are_saved_as_identities_and_transient_fields_left_out() {
        let start = || {
            let mut world = super::tests::start();
            world.register::<Rope>();
            world
        };
        let mut played = start();
        let (gate, well) = (entity(&played, "yard/gate"), entity(&played, "yard/well"));
        played.insert(
            gate,
            Rope {
                to: well,
                swing: 0.5,
            },
        );
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        assert!(
            save.contains("[added.\"yard/gate\"]\n\"test::Rope\" = { to = \"yard/well\" }\n"),
            "{save}"
        );
        assert!(!save.contains("swing"), "{save}");
        let mut loaded = start();
        loaded.load(&save).expect("loaded");
        let rope = loaded
            .get::<Rope>(entity(&loaded, "yard/gate"))
            .expect("a rope");
        assert_eq!(rope.to, entity(&loaded, "yard/well"));
        assert_eq!(rope.swing, 0.0, "its default");
        // A reference to an entity destroyed, or that the save leaves out, is no entity; one to an
        // entity created since is its identity, whatever the slots of the two worlds.
        let spark = played.spawn();
        played.insert(spark, Unsaved);
        let ash = played.spawn();
        played.despawn(ash);
        let torch = played.spawn();
        played.insert(
            gate,
            Rope {
                to: spark,
                swing: 0.0,
            },
        );
        played.insert(
            well,
            Rope {
                to: ash,
                swing: 0.0,
            },
        );
        let post = entity(&played, "yard/post");
        played.insert(
            post,
            Rope {
                to: torch,
                swing: 0.0,
            },
        );
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        assert!(save.contains("\"test::Rope\" = { to = \"\" }"), "{save}");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
        for at in ["yard/gate", "yard/well"] {
            let rope = loaded.get::<Rope>(entity(&loaded, at)).expect("a rope");
            assert!(!loaded.contains(rope.to), "{at}: no entity");
        }
        assert_ne!(
            played.get::<Rope>(post).map(|rope| rope.to.to_value()),
            loaded
                .get::<Rope>(entity(&loaded, "yard/post"))
                .map(|rope| rope.to.to_value()),
            "the torch is in another slot"
        );
        assert_eq!(played.first_difference(&loaded), None, "{save}");
        // A reference to an entity that the world does not have refers to no entity, reported.
        let mut loaded = start();
        let report = loaded
            .load("[added.\"yard/gate\"]\n\"test::Rope\" = { to = \"yard/nothing\" }\n")
            .expect("loaded");
        let rope = loaded
            .get::<Rope>(entity(&loaded, "yard/gate"))
            .expect("the rope is added");
        assert!(!loaded.contains(rope.to));
        assert_eq!(report.set_aside.len(), 1, "{:?}", report.set_aside);
        assert!(
            report.set_aside[0].reason.contains("`yard/nothing`"),
            "{:?}",
            report.set_aside
        );
    }

    #[test]
    fn a_child_takes_its_parents_place_and_one_moved_out_of_an_entity_removed_stays() {
        let mut played = start();
        let root = entity(&played, "yard");
        let (gate, post, well) = (
            entity(&played, "yard/gate"),
            entity(&played, "yard/post"),
            entity(&played, "yard/well"),
        );
        // The post, the gate's child, becomes its parent.
        played.set_parent(post, Some(root)).expect("a parent");
        played.set_parent(gate, Some(post)).expect("a parent");
        // The tree gets a child, and is removed with it after the child moves out.
        let tree = entity(&played, "yard/tree");
        played.set_parent(well, Some(tree)).expect("a parent");
        played.set_parent(well, Some(root)).expect("a parent");
        let sapling = played.spawn();
        played.set_parent(sapling, Some(tree)).expect("a parent");
        let seed = played.spawn();
        played.set_parent(seed, Some(sapling)).expect("a parent");
        played.set_parent(seed, None).expect("no parent");
        played.despawn(tree);
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(
            report.set_aside.is_empty(),
            "{:?}\n{save}",
            report.set_aside
        );
        assert_eq!(played.first_difference(&loaded), None, "{save}");
        // An entity removed with its parent is not reported as gone.
        let mut played = start();
        played.despawn(entity(&played, "yard/gate"));
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(
            report.set_aside.is_empty(),
            "{:?}\n{save}",
            report.set_aside
        );
        assert_eq!(played.first_difference(&loaded), None, "{save}");
        // A scene whose root is destroyed, one of its entities moved out first, is still saved.
        let mut played = start();
        let well = entity(&played, "yard/well");
        played.set_parent(well, None).expect("no parent");
        played.despawn(entity(&played, "yard"));
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        assert!(save.contains("[scenes]\nyard = "), "{save}");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(report.set_aside.is_empty() && report.scenes_changed.is_empty());
        assert_eq!(played.first_difference(&loaded), None, "{save}");
    }

    /// A mood, whose variant that holds data has a field that saves leave out.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Mood")]
    enum Mood {
        Calm,
        Angry {
            level: u8,
            #[reflect(transient, default)]
            shake: f32,
        },
    }

    /// A knot of a rope, with a field that saves leave out.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Knot")]
    struct Knot {
        at: f32,
        #[reflect(transient, default)]
        seen: bool,
    }

    /// What saves hold whole within what changed of a component: a variant, a list, an option.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Temper")]
    struct Temper {
        mood: Mood,
        knots: Vec<Knot>,
        last: Option<Knot>,
    }
    impl crate::Component for Temper {}

    #[test]
    fn what_a_change_holds_whole_takes_the_defaults_of_its_transient_fields() {
        let start = || {
            let mut world = super::tests::start();
            let gate = entity(&world, "yard/gate");
            world.insert(
                gate,
                Temper {
                    mood: Mood::Calm,
                    knots: Vec::new(),
                    last: None,
                },
            );
            world
        };
        let mut played = start();
        let knot = Knot {
            at: 1.0,
            seen: true,
        };
        played.insert(
            entity(&played, "yard/gate"),
            Temper {
                mood: Mood::Angry {
                    level: 3,
                    shake: 0.7,
                },
                knots: vec![knot.clone()],
                last: Some(knot),
            },
        );
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        assert!(!save.contains("shake") && !save.contains("seen"), "{save}");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(
            report.set_aside.is_empty(),
            "{:?}\n{save}",
            report.set_aside
        );
        let unseen = Knot {
            at: 1.0,
            seen: false,
        };
        assert_eq!(
            loaded.get::<Temper>(entity(&loaded, "yard/gate")),
            Some(&Temper {
                mood: Mood::Angry {
                    level: 3,
                    shake: 0.0
                },
                knots: vec![unseen.clone()],
                last: Some(unseen),
            })
        );
        assert_eq!(played.first_difference(&loaded), None, "{save}");
    }

    #[test]
    fn numbers_of_entities_created_are_written_one_way_and_never_run_out() {
        for text in [
            "[created.\"created/18446744073709551615\"]\n",
            "[created.\"created/9223372036854775808\"]\n",
            "[created.\"created/01\"]\n",
            "[created.\"created/+1\"]\n",
        ] {
            let error = start().load(text).expect_err(text);
            assert!(
                matches!(error, DocumentError::NotCreated(_)),
                "{text}: {error}"
            );
        }
        let largest = i64::MAX as u64;
        for (text, next) in [
            (
                "next_created = \"18446744073709551615\"\n".to_owned(),
                largest,
            ),
            (format!("[created.\"created/{largest}\"]\n"), largest + 1),
        ] {
            let mut world = start();
            world.load(&text).expect(&text);
            let spawned = world.spawn();
            assert_eq!(
                world.persistent_id(spawned),
                Some(PersistentId::created(next)),
                "{text}"
            );
        }
    }

    #[test]
    fn what_the_start_leaves_out_and_the_world_saves_is_not_created_again() {
        let start = || {
            let mut world = super::tests::start();
            let tree = entity(&world, "yard/tree");
            world.insert(tree, Unsaved);
            let ember = world.spawn();
            world.set_parent(ember, Some(tree)).expect("a parent");
            world
        };
        let mut played = start();
        let tree = entity(&played, "yard/tree");
        played.remove::<Unsaved>(tree);
        let ember = entity(&played, "created/1");
        played.set_parent(ember, None).expect("no parent");
        // The well, which the start saves, is left out since: the save does not remove it.
        let well = entity(&played, "yard/well");
        played.insert(well, Unsaved);
        played.insert(well, Transform::from_translation(DVec3::new(7.0, 0.0, 0.0)));
        // An entity left out is left out with its descendants.
        let spark = played.spawn();
        played.insert(spark, Unsaved);
        let flame = played.spawn();
        played.set_parent(flame, Some(spark)).expect("a parent");
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        assert!(!save.contains("[created"), "{save}");
        assert!(!save.contains("yard/well"), "{save}");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(
            report.set_aside.is_empty(),
            "{:?}\n{save}",
            report.set_aside
        );
        assert_eq!(played.first_difference(&loaded), None, "{save}");
        let well = entity(&loaded, "yard/well");
        assert_eq!(
            loaded.get::<Transform>(well).map(|at| at.translation),
            Some(DVec3::new(5.0, 0.0, 2.0)),
            "as the start has it"
        );
    }

    #[test]
    fn what_the_save_does_not_remove_outlives_an_entity_removed() {
        // The post, the gate's child at the start, is left out since, under an unsaved entity.
        let mut played = start();
        let spark = played.spawn();
        played.insert(spark, Unsaved);
        let post = entity(&played, "yard/post");
        played.set_parent(post, Some(spark)).expect("a parent");
        played.despawn(entity(&played, "yard/gate"));
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(
            report.set_aside.is_empty(),
            "{:?}\n{save}",
            report.set_aside
        );
        let post = entity(&loaded, "yard/post");
        assert_eq!(loaded.parent(post), None, "out of the gate removed");
        // An entity whose parent the world does not have is out of its own all the same.
        let mut loaded = start();
        let report = loaded
            .load("[removed]\nentities = [\"yard/gate\"]\n\n[parents]\n\"yard/post\" = [\"yard/shed\"]\n")
            .expect("loaded");
        let post = entity(&loaded, "yard/post");
        assert_eq!(loaded.parent(post), None);
        assert_eq!(report.set_aside.len(), 1, "{:?}", report.set_aside);
        assert!(report.set_aside[0].reason.contains("`yard/shed`"));
    }

    /// A component of a version that no upgrade brings forward: reading one panics.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Brittle", version = 2, upgrade = shatter)]
    struct Brittle {
        hardness: f32,
    }
    impl crate::Component for Brittle {}

    fn shatter(_: u32, _: Value, _: bool) -> Result<Value, String> {
        panic!("shattered");
    }

    #[test]
    fn a_world_keeps_its_registry_when_reading_a_save_panics() {
        let mut world = start();
        world.register::<Brittle>();
        let text = "\"$versions\" = { \"test::Brittle\" = 1 }\n\n\
                    [added.\"yard/gate\".\"test::Brittle\"]\nhardness = 2.0\n";
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| world.load(text)));
        assert!(panicked.is_err());
        assert!(world.registry().get("test::Brittle").is_some());
        assert!(world.registry().get("Transform").is_some());
    }

    #[test]
    fn what_a_save_says_of_itself_comes_first_and_is_read_without_loading_it() {
        let about = BTreeMap::from([
            ("layout".to_owned(), "8fe31823c2a1615f".to_owned()),
            ("made with".to_owned(), "version 0.3".to_owned()),
        ]);
        let save = played().save(&start(), &about).expect("saved");
        assert!(
            save.starts_with(
                "about = { layout = \"8fe31823c2a1615f\", \"made with\" = \"version 0.3\" }\n"
            ),
            "{save}"
        );
        assert_eq!(World::about(&save).expect("a save"), about);
        let mut loaded = start();
        let report = loaded.load(&save).expect("loaded");
        assert!(report.set_aside.is_empty() && report.warnings.is_empty());
        assert_eq!(played().first_difference(&loaded), None);
        // A save without it says nothing; one that is not a save, or says something else than
        // text, is refused.
        let quiet = played().save(&start(), &BTreeMap::new()).expect("saved");
        assert!(!quiet.contains("about"), "{quiet}");
        assert!(World::about(&quiet).expect("a save").is_empty());
        for text in ["about = [", "about = { layout = 3 }\n", "\"$format\" = 2\n"] {
            assert!(
                matches!(World::about(text), Err(DocumentError::Read(_))),
                "{text}"
            );
        }
    }

    #[test]
    #[ignore = "a measurement, to run in a release build"]
    fn what_saving_and_loading_10_000_entities_created_costs() {
        let mut played = start();
        for index in 0..10_000 {
            let entity = played.spawn();
            played.insert(entity, Name(format!("stone {index}")));
            played.insert(
                entity,
                Transform::from_translation(DVec3::new(f64::from(index), 0.0, 0.0)),
            );
        }
        let started = std::time::Instant::now();
        let save = played.save(&start(), &BTreeMap::new()).expect("saved");
        eprintln!("save {} bytes: {:?}", save.len(), started.elapsed());
        let mut loaded = start();
        let started = std::time::Instant::now();
        loaded.load(&save).expect("loaded");
        eprintln!("load: {:?}", started.elapsed());
        assert_eq!(played.first_difference(&loaded), None);
    }
}
