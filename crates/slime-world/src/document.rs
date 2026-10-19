//! The world's documents: a save, what differs between a world and the world that a game makes at
//! the start of a run, and that difference applied again to a world made so
//! (`docs/adr/0087-the-worlds-documents.md`,
//! `docs/adr/0093-saves-and-hot-reload-with-placements.md`).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::mem;

use slime_assets::AssetId;
use slime_reflect::{Problem, Reflect, ReflectError, Registry, Value};
use slime_serialize::{Composer, Document, Mode, ReadError, References};

use crate::components::{Name, Unsaved};
use crate::entity::Entity;
use crate::identity::{CREATED as CREATED_KEY, PersistentId};
use crate::library::{MAX_ENTITIES, ResolvedScene, SceneLibrary, fingerprint};
use crate::scene::{record_ids, storages_of};
use crate::scene_file::{LAYOUT, for_each_entity, table};
use crate::world::{Identity, World};
use crate::write_back::{expected, handle_bits};

/// What the game says of the save: its keys and texts.
const ABOUT: &str = "about";
/// The count of the entities created outside scenes.
const NEXT_CREATED: &str = "next_created";
/// What the start that a save was made against created outside scenes: the count it reached, and
/// the fingerprint of the entities it created ([`World::created_fingerprint`]).
const START: &str = "start";
/// The layout of saves that this program writes, and the latest that it reads: 1 for the saves of
/// 0.3, which hold nothing spawned while the world ran, 2 for those of 0.4, which do not say what
/// their start created.
const LAYOUT_VERSION: u64 = 3;
/// The scenes that the start spawned, by their keys: their names and contents.
const SCENES: &str = "scenes";
/// What the world spawned while it ran, by the identities of their roots: their scenes' asset
/// IDs, names and contents.
const SPAWNED: &str = "spawned";
/// The entities and the components removed.
pub(crate) const REMOVED: &str = "removed";
/// The entities given another parent, or none.
const PARENTS: &str = "parents";
/// The entities created outside scenes, whole.
const CREATED: &str = "created";
/// The components added, whole.
pub(crate) const ADDED: &str = "added";
/// What changed of the components, in parts.
pub(crate) const CHANGED: &str = "changed";

/// Why a world cannot be saved, a save loaded, or a world written as a scene file.
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
    /// The save creates an entity, or places a placement made while the world ran, under an identity
    /// that is not one of an entity created outside a scene, `created/<number>`, the number written
    /// without a sign or a leading zero and at most `i64::MAX`, or a key
    /// ([`World::spawn_keyed`]), which places nothing.
    #[error(
        "the save creates or places `{0}`, which is not the identity of an entity created outside \
         a scene"
    )]
    NotCreated(String),
    /// The save creates an entity, or places a placement made while the world ran, under an identity
    /// that an entity of the world has, or that it gives twice: a key that the world's start gives
    /// already, or, in a save of layout 2 or earlier, a number, when the game made more entities at
    /// the start of its run than when the save was made.
    #[error(
        "the save creates or places `{0}`, an identity that an entity of the world, or of the save, \
         has already"
    )]
    IdentityInUse(PersistentId),
    /// The entity cannot be the root of a scene file written from the world: it is not alive, or it
    /// is a placement or an entity of one, which the scene that places it writes.
    #[error("{entity} cannot be written as a scene's root: {reason}")]
    NotARoot {
        /// The entity, by its persistent identity, or its handle if it has none.
        entity: String,
        /// Why.
        reason: String,
    },
}

/// What loading a save left aside, and why: what the save holds of entities or components that
/// the world, made from the scenes as they are now, no longer has.
#[derive(Debug, Default)]
pub struct Loaded {
    /// The keys of the scenes that the save was made on whose content changed since, prefabs
    /// included, or that the world no longer spawns; and the roots of what the world spawned while
    /// it ran whose scene changed since: a key, or `created/<number>` for a placement.
    pub scenes_changed: Vec<String>,
    /// The differences that could not be applied, or only in part: a reference to an entity that
    /// the world does not have refers to no entity.
    pub set_aside: Vec<SetAside>,
    /// What the save holds that this program does not know, left out: fields, types.
    pub warnings: Vec<ReadError>,
}

/// What could not be applied or held, and why: a difference of a save that a world could not apply
/// ([`Loaded::set_aside`]), what a scene file says that its prefabs no longer fit
/// ([`ResolvedScene::set_aside`](crate::ResolvedScene::set_aside)), or what a scene file written
/// from a world cannot hold ([`Written::left_out`](crate::Written::left_out)).
#[derive(Clone, Debug, PartialEq)]
pub struct SetAside {
    /// The entity it is about: by its persistent identity in a save and in a file written from a
    /// world, by its path from the scene's root in a scene file.
    pub entity: String,
    /// The component it is about, if it is about one.
    pub component: Option<String>,
    /// Why it was set aside.
    pub reason: String,
}

/// Checks the sections of the save `document` and their entries: one that is not a table, where a
/// save holds one, refuses the save, and a key that a save does not hold, misspelled or of a later
/// program, is a warning, as a field that its type does not have is.
fn sections(document: &Document<'_>, loaded: &mut Loaded) -> Result<(), DocumentError> {
    let mut section = |place: &[&str], known: Option<&[&str]>| {
        let keys = table(document, place).map_err(DocumentError::Read)?;
        for key in keys
            .iter()
            .filter(|key| known.is_some_and(|known| !known.contains(&key.as_str())))
        {
            let mut at = place.to_vec();
            at.push(key);
            loaded.warnings.push(
                document.error_at(
                    &at,
                    Problem::UnknownField {
                        field: key.clone(),
                        known: known
                            .into_iter()
                            .flatten()
                            .map(|&key| key.to_owned())
                            .collect(),
                    },
                ),
            );
        }
        Ok::<_, DocumentError>(keys)
    };
    let root = [
        ABOUT,
        LAYOUT,
        NEXT_CREATED,
        START,
        SCENES,
        SPAWNED,
        REMOVED,
        PARENTS,
        CREATED,
        ADDED,
        CHANGED,
    ];
    section(&[], Some(&root))?;
    section(&[ABOUT], None)?;
    section(&[START], Some(&["created", "content"]))?;
    section(&[PARENTS], None)?;
    section(&[REMOVED], Some(&["entities", "components"]))?;
    section(&[REMOVED, "components"], None)?;
    for (name, known) in [
        (SCENES, Some(&["name", "content"][..])),
        (SPAWNED, Some(&["scene", "name", "content"][..])),
        (CREATED, Some(&["parent", "components"][..])),
        (ADDED, None),
        (CHANGED, None),
    ] {
        for key in section(&[name], None)? {
            section(&[name, &key], known)?;
            if name == CREATED {
                section(&[name, &key, "components"], None)?;
            }
        }
    }
    Ok(())
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
    /// How the save read names the entities that a world created outside scenes.
    numbers: Numbers,
    /// The names read since they were last taken that no entity of the world has.
    unknown: RefCell<Vec<String>>,
}

impl<'w> Names<'w> {
    fn new(world: &'w World, numbers: Numbers) -> Self {
        Self {
            world,
            numbers,
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
        let found = self
            .numbers
            .identity(name)
            .and_then(|id| self.world.find(&id));
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
    /// (`docs/adr/0087-the-worlds-documents.md`,
    /// `docs/adr/0093-saves-and-hot-reload-with-placements.md`): the entities removed; those
    /// created while the world ran, whole; the components added, whole, and removed; the parents
    /// changed; and what changed of the other components, in parts. Entities are matched by their
    /// persistent identities: one is created since the start if `start` has no entity of its
    /// identity. Those that hold [`Unsaved`], or whose ancestor does, are left out, as are the
    /// fields marked transient: an entity of `start` that the world leaves out is not removed, and
    /// loading the save keeps it as `start` has it, out of its parent if the save removes that. A
    /// reference to an entity is its persistent identity, compared with `start`'s by the identity
    /// that each names, whatever order the two worlds were made in, and a reference to an entity
    /// destroyed or left out is `""`, no entity. The save also holds the scenes that `start`
    /// spawned, so that loading it says which changed since, the count of the entities created
    /// outside scenes, and what `start` created outside scenes: its count, and a fingerprint of
    /// those entities, their numbers, the names of their components' types and their [`Name`]s. Its
    /// first section, after the versions of the types past version 1 if it holds some, is `about`,
    /// what the game says of it, keys and texts that the engine does not read, such as what the run
    /// was made on, which [`World::about`] reads without loading the save; none for an empty
    /// `about`.
    ///
    /// What the world spawned while it ran, a scene under a key under which `start` spawned no
    /// scene or another one ([`World::spawn_scene`]), or a placement made while it ran
    /// ([`World::spawn_placement`]), is written as its scene, by its asset ID and fingerprint,
    /// while an entity of it is saved: its entities are compared with what the scene spawns, as
    /// those of `start` are with `start`, and loading spawns the scene as it is then, under the
    /// same identity, before it applies the rest.
    ///
    /// The same worlds give the same bytes: entities in the order of their identities, components
    /// in the order of their names. Its cost grows in proportion with the entities of the two
    /// worlds and with what differs: measured in a release build on the owner's machine, 10 000
    /// entities created, each with a name and a transform, save in 0.22 s into 2.9 MB of text, and
    /// 40 000 in four times as long. A save holds at most [`slime_serialize::MAX_SIZE`], about 57
    /// 000 such entities. A thousand placements made while the world runs, each moved and a value
    /// of it changed, save in 0.08 s into 238 kB, and load in 0.03 s, for a prefab of 10 entities;
    /// in 0.85 s and 0.16 s for a prefab of 100.
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
        let names = Names::new(self, Numbers::SAME);
        let mut save = Composer::new(&self.registry)
            .naming(&names)
            .without_transient();
        let mut put = |place: &[&str], ty: &str, value: Value| {
            save.put(place, ty, &value).map_err(DocumentError::Write)
        };
        for (key, text) in about {
            put(&[ABOUT, key], "String", Value::Text(text.clone()))?;
        }
        put(&[LAYOUT], "u64", Value::UInt(LAYOUT_VERSION))?;
        put(&[NEXT_CREATED], "u64", Value::UInt(self.next_created))?;
        put(&[START, "created"], "u64", Value::UInt(start.next_created))?;
        let content = Value::Text(content_text(start.created_fingerprint()));
        put(&[START, "content"], "String", content)?;
        let now = self.saved_entities();
        let named_now = self.named_entities();
        for (key, scene) in self
            .scenes
            .iter()
            .filter(|(key, _)| self.spawned_as_at_start(start, key))
        {
            put(
                &[SCENES, key, "name"],
                "String",
                Value::Text(scene.name.clone()),
            )?;
            let content = Value::Text(content_text(scene.content));
            put(&[SCENES, key, "content"], "String", content)?;
        }
        let spawned = self.spawned_while_running(start, &now);
        for spawned in &spawned {
            let key = spawned.root.to_string();
            let scene = &spawned.scene.0;
            put(
                &[SPAWNED, &key, "scene"],
                "String",
                Value::Text(scene.id.to_string()),
            )?;
            put(
                &[SPAWNED, &key, "name"],
                "String",
                Value::Text(scene.name.clone()),
            )?;
            let content = Value::Text(content_text(scene.fingerprint));
            put(&[SPAWNED, &key, "content"], "String", content)?;
        }
        // What each entity is compared with: the start's entity of its identity, saved or not, or
        // the record of what the world spawned while it ran.
        let mut before: BTreeMap<PersistentId, Before<'_>> = start
            .named_entities()
            .into_iter()
            .map(|(id, entity)| (id, Before::Start(entity)))
            .collect();
        let mut saved_before: Vec<PersistentId> = start.saved_entities().into_keys().collect();
        for spawned in &spawned {
            for (index, id) in spawned.ids.iter().enumerate() {
                before.insert(id.clone(), Before::Spawned(spawned, index));
                if !spawned.unsaved[index] {
                    saved_before.push(id.clone());
                }
            }
        }
        saved_before.sort();
        // An entity is created since the start if nothing is compared with it; removed if what it
        // is compared with is saved and the world has no entity of its identity.
        let removed: Vec<Value> = saved_before
            .iter()
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
            let Some(&old) = before.get(id) else {
                continue;
            };
            let gone: Vec<Value> = old
                .component_names(start)
                .into_iter()
                .filter(|name| self.component_value(entity, name).is_none())
                .map(Value::Text)
                .collect();
            if !gone.is_empty() {
                put(
                    &[REMOVED, "components", &key],
                    "Vec<String>",
                    Value::List(gone),
                )?;
            }
            let parent = parent_of(self, entity);
            if parent != old.parent(start) {
                put(&[PARENTS, &key], "Option<String>", text_option(parent))?;
            }
        }
        drop(put);
        for (id, &entity) in &now {
            let key = id.to_string();
            match before.get(id) {
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
                Some(old) => {
                    for name in self.component_names_sorted(entity) {
                        let Some(value) = self.component_value(entity, name) else {
                            continue;
                        };
                        // A reference to an entity that the save writes as no entity, destroyed or
                        // left out, compared as one, on both sides: a record gives no entity for
                        // one of its scene destroyed since.
                        let value = to_no_entity(value, &names);
                        let was = old
                            .value(start, self, name)
                            .map(|was| to_no_entity(was, &names));
                        match was {
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
    /// from the scenes as they are now ([`World::save`]): what the world spawned while it ran is
    /// spawned again, each scene resolved through `scenes` as it is now, under the identity that it
    /// had; the entities that the save creates are spawned under their identities, the parents set,
    /// an entity whose parent the world does not have out of its own, the entities removed
    /// despawned, the entities saved under them that the save does not remove taken out of them
    /// first, the components removed, added and changed, and the count of the entities created
    /// outside scenes kept. What the run created outside scenes, its entities and its placements,
    /// takes the numbers that follow this world's count, whatever the start creates now; what the
    /// start of the run created keeps its numbers, and what the save holds of it is set aside if
    /// this world's start created other entities outside scenes, by the fingerprint that the save
    /// holds: a key names an entity whatever the start creates ([`World::spawn_keyed`]). A save is
    /// read leniently: what this program does not know is left out, with a warning, a section of
    /// the save included. A difference whose entity or component the world no longer has, or that
    /// the component's type refuses, is set aside, and the rest applied; so is a reference to an
    /// entity that the world no longer has, which refers to no entity then, a parent that the world
    /// does not have, and a scene spawned while the world ran that `scenes` cannot give, that the
    /// world spawned already, whose values its types refuse, or that would bring the entities that
    /// the save spawns so past [`MAX_ENTITIES`]. A scene spawned under a key under which the world
    /// spawned another destroys that one first, with what is under it, as when a run changed level.
    /// The scenes that changed since the save, those spawned while the world ran included, are
    /// named. The saves of 0.3, of layout 1, which hold nothing spawned while the world ran, are
    /// read as they were. Loading entities created costs about two and a half times as much as
    /// saving them, half of it parsing the text: 0.54 s for the 10 000 entities of
    /// [`World::save`]'s measure, which gives what placements cost too.
    ///
    /// # Errors
    ///
    /// [`DocumentError::Read`] if the save is not one: not TOML, of a later version of the format,
    /// of its layout or of a type, too large, or with a section that is not what a save holds;
    /// [`DocumentError::NotCreated`] if it creates an entity, or places a placement, under an
    /// identity that a world does not give to the entities it creates;
    /// [`DocumentError::IdentityInUse`] if it creates or places one under an identity that an
    /// entity of the world has, or twice. All of it is read before the world changes, which is
    /// unchanged then.
    ///
    /// # Panics
    ///
    /// If the world runs out of slots ([`World::spawn`]).
    pub fn load(&mut self, text: &str, scenes: &mut SceneLibrary) -> Result<Loaded, DocumentError> {
        // Taken while the world holds its registry, which names the components' types.
        let start = self.created_fingerprint();
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
                .and_then(|mut document| {
                    apart
                        .world
                        .read(&apart.registry, &mut document, start, scenes)
                })
        };
        let (save, mut loaded) = read?;
        self.apply(save, &mut loaded);
        Ok(loaded)
    }

    /// The run of this world carried into `start`, a world that the game made as at the start of a
    /// run from its scenes as they are now, when `old_start` is the one that it made before they
    /// changed: what saving the run against `old_start` and loading the save into `start` give
    /// (`docs/adr/0089-scenes-and-prefabs.md`), what loading set aside with it. A scene or a prefab
    /// changed reaches every entity that it spawned but what the run changed of it, its entities
    /// added appear, and what the run changed stays. Handles to entities change, as when a save is
    /// loaded. It costs a save and a load.
    ///
    /// # Errors
    ///
    /// As [`World::save`] and [`World::load`]: nothing is carried then.
    ///
    /// # Panics
    ///
    /// As [`World::load`].
    pub fn carry(
        &self,
        old_start: &World,
        mut start: World,
        scenes: &mut SceneLibrary,
    ) -> Result<(World, Loaded), DocumentError> {
        let save = self.save(old_start, &BTreeMap::new())?;
        let loaded = start.load(&save, scenes)?;
        Ok((start, loaded))
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
        let (our_references, their_references) = (
            Names::new(self, Numbers::SAME),
            Names::new(other, Numbers::SAME),
        );
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

    /// Reads the save `document`: first all that can refuse it, the world untouched; then spawns
    /// what the world that made it spawned while it ran, and the entities that it creates, so that
    /// the components' values read last, which can only be set aside, can refer to them. `start` is
    /// the world's [`World::created_fingerprint`], taken before its registry was set apart.
    fn read(
        &mut self,
        registry: &Registry,
        document: &mut Document<'_>,
        start: u64,
        scenes: &mut SceneLibrary,
    ) -> Result<(Save, Loaded), DocumentError> {
        let mut loaded = Loaded::default();
        let (mut save, plan) = self.read_layout(document, start, &mut loaded)?;
        // Nothing refuses the save from here on: the world changes.
        self.spawn_what_ran(registry, scenes, plan.spawned, &mut save, &mut loaded);
        self.create(plan.created);
        self.read_values(document, &mut save, &mut loaded);
        Ok((save, loaded))
    }

    /// Reads all that refuses the save `document` if it is wrong, without changing the world: its
    /// layout, what the world that made it spawned while it ran, the numbers of the entities that it
    /// creates, its count, its scenes, the entities and components that it removes, and the parents.
    /// `start` is the world's [`World::created_fingerprint`].
    fn read_layout(
        &self,
        document: &mut Document<'_>,
        start: u64,
        loaded: &mut Loaded,
    ) -> Result<(Save, Plan), DocumentError> {
        if let Some(Value::UInt(layout)) = document
            .read(&[LAYOUT], "u64", None)
            .map_err(DocumentError::Read)?
            && !(1..=LAYOUT_VERSION).contains(&layout)
        {
            return Err(DocumentError::Read(document.error_at(
                &[LAYOUT],
                Problem::Invalid(format!(
                    "a save of layout {layout}, where this program reads layouts 1 to \
                     {LAYOUT_VERSION}"
                )),
            )));
        }
        sections(document, loaded)?;
        let start_created = document
            .read(&[START, "created"], "u64", None)
            .map_err(DocumentError::Read)?;
        let start_content = document
            .read(&[START, "content"], "String", None)
            .map_err(DocumentError::Read)?;
        let renumbering = match start_created {
            Some(Value::UInt(from)) => Numbers {
                from,
                to: self.next_created,
                start_changed: start_content != Some(Value::Text(content_text(start))),
            },
            _ => Numbers::SAME,
        };
        let mut plan = Plan::default();
        // The numbers that the save gives, as this world numbers them, each once, none that the
        // world gives already.
        let mut numbers = HashSet::new();
        let mut number = |key: &str, id: Option<PersistentId>| {
            let number = id
                .and_then(|id| id.created_number())
                .filter(|&number| number <= LARGEST_CREATED)
                .and_then(|number| renumbering.number(number))
                .ok_or_else(|| DocumentError::NotCreated(key.to_owned()))?;
            if self.created.find(number, &self.entities).is_some() || !numbers.insert(number) {
                return Err(DocumentError::IdentityInUse(PersistentId::created(number)));
            }
            Ok(number)
        };
        for key in document.keys(&[SPAWNED]) {
            let id = key.parse::<PersistentId>().ok();
            let identity = match id {
                Some(id) if id.parts().first().map(String::as_str) == Some(CREATED_KEY) => {
                    SpawningAs::Placement(number(&key, Some(id))?)
                }
                _ => SpawningAs::Key(key.clone()),
            };
            let mut text = |field: &str| match document
                .read(&[SPAWNED, &key, field], "String", None)
                .map_err(DocumentError::Read)?
            {
                Some(Value::Text(text)) => Ok(text),
                _ => Ok(String::new()),
            };
            plan.spawned.push(ToSpawn {
                asset: text("scene")?,
                name: text("name")?,
                content: text("content")?,
                key,
                identity,
            });
        }
        let mut keys = HashSet::new();
        for key in document.keys(&[CREATED]) {
            let id = key.parse::<PersistentId>().ok();
            let creation = match id {
                // A key that the run gave an entity that it created.
                Some(id) if id.parts().len() == 1 && id.parts()[0] != CREATED_KEY => {
                    if self.find(&id).is_some() || !keys.insert(id.clone()) {
                        return Err(DocumentError::IdentityInUse(id));
                    }
                    Creation::Key(id)
                }
                _ => Creation::Number(number(&key, id)?),
            };
            plan.created.push(creation);
        }
        let mut save = Save {
            next_created: match document.read(&[NEXT_CREATED], "u64", None) {
                // Larger than any a world saves, so that the count never runs out.
                Ok(Some(Value::UInt(next))) => renumbering
                    .number(next.min(LARGEST_CREATED))
                    .unwrap_or(LARGEST_CREATED),
                Ok(_) => 1,
                Err(error) => return Err(DocumentError::Read(error)),
            },
            numbers: renumbering,
            ..Save::default()
        };
        for key in document.keys(&[SCENES]) {
            let content = document
                .read(&[SCENES, &key, "content"], "String", None)
                .map_err(DocumentError::Read)?;
            let same = match (self.scenes.get(&key), content) {
                (Some(scene), Some(Value::Text(content))) => content_text(scene.content) == content,
                _ => false,
            };
            if !same {
                loaded.scenes_changed.push(key);
            }
        }
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
        for (section, parent) in [(PARENTS, None), (CREATED, Some("parent"))] {
            for key in document.keys(&[section]) {
                let place: Vec<&str> = [section, key.as_str()].into_iter().chain(parent).collect();
                if let Some(parent) = document
                    .read(&place, "Option<String>", None)
                    .map_err(DocumentError::Read)?
                {
                    save.parents.push((key, option_text(&parent)));
                }
            }
        }
        Ok((save, plan))
    }

    /// Spawns what the world that made a save spawned while it ran, `spawned`, each scene as
    /// `scenes` resolves it now with the types of `registry`, the world's, under the identity that it
    /// had; sets aside those that cannot be spawned, and names those whose scene changed since.
    fn spawn_what_ran(
        &mut self,
        registry: &Registry,
        scenes: &mut SceneLibrary,
        spawned: Vec<ToSpawn>,
        save: &mut Save,
        loaded: &mut Loaded,
    ) {
        let mut numbered = Vec::new();
        let mut entities = 0;
        for ToSpawn {
            key,
            identity,
            asset,
            name,
            content,
        } in spawned
        {
            let mut aside = |reason: String| {
                loaded.set_aside.push(SetAside {
                    entity: key.clone(),
                    component: None,
                    reason: format!(
                        "the scene `{name}` is not spawned, with what was under it: {reason}"
                    ),
                });
            };
            let Ok(asset_id) = asset.parse::<AssetId>() else {
                aside(format!("`{asset}` is not an asset ID"));
                continue;
            };
            let scene = match scenes.resolve(registry, asset_id) {
                Ok(scene) => scene,
                Err(error) => {
                    aside(error.to_string());
                    continue;
                }
            };
            entities += scene.len();
            if entities > MAX_ENTITIES {
                aside(format!(
                    "the save spawns more than {MAX_ENTITIES} entities while it is loaded"
                ));
                continue;
            }
            let storages = match storages_of(registry, &self.component_ids, &scene) {
                Ok(storages) => storages,
                Err(error) => {
                    aside(error.to_string());
                    continue;
                }
            };
            let root = match identity {
                SpawningAs::Key(key) => {
                    if let Some(held) = self.scenes.get(&key) {
                        if held.scene.id() == asset_id {
                            aside(
                                "the world spawned it already, whose entities take the save's \
                                 differences"
                                    .to_owned(),
                            );
                            continue;
                        }
                        // Another scene under the key, as when a run changed level: the world's
                        // goes first, with what was under it.
                        let held = held.scene.clone();
                        self.despawn_scene(&key, &held, &mut save.gone);
                    }
                    self.spawn_scene_with(&key, &scene, &storages)
                }
                SpawningAs::Placement(number) => {
                    self.place(&scene, number, &storages).inspect(|&root| {
                        numbered.push((number, root));
                        self.next_created = self.next_created.max(number + 1);
                    })
                }
            };
            match root {
                Ok(_) if content_text(scene.0.fingerprint) != content => {
                    loaded.scenes_changed.push(key);
                }
                Ok(_) => {}
                Err(error) => aside(error.to_string()),
            }
        }
        self.created.extend(numbered);
    }

    /// Destroys the entities of `scene`, spawned under `key`, with their descendants, and forgets
    /// the scene; adds their identities to `gone`.
    fn despawn_scene(&mut self, key: &str, scene: &ResolvedScene, gone: &mut HashSet<String>) {
        for id in record_ids(&PersistentId::new([key]), scene) {
            let Some(entity) = self.find(&id) else {
                continue;
            };
            let mut next = vec![entity];
            while let Some(entity) = next.pop() {
                if let Some(id) = self.persistent_id(entity) {
                    gone.insert(id.to_string());
                }
                next.extend(self.children(entity));
            }
            self.despawn(entity);
        }
        self.scenes.remove(key);
    }

    /// Spawns the entities that a save creates, under their numbers or their keys.
    fn create(&mut self, creations: Vec<Creation>) {
        let mut created = Vec::new();
        for creation in creations {
            let entity = self.spawn_bare();
            match creation {
                Creation::Number(number) => {
                    self.nodes[entity.index() as usize].identity = Identity::Created(number);
                    // The count stays larger than every number given, whatever the save's says.
                    self.next_created = self.next_created.max(number + 1);
                    created.push((number, entity));
                }
                Creation::Key(id) => self.identify(entity, id),
            }
        }
        self.created.extend(created);
    }

    /// Reads the components' values that the save `document` holds, into `save`, the entities they
    /// belong to and refer to spawned already: those that cannot be read, and the references to
    /// entities that the world does not have, are set aside.
    fn read_values(&self, document: &mut Document<'_>, save: &mut Save, loaded: &mut Loaded) {
        let names = Names::new(self, save.numbers);
        let mut take = |key: &str, name: &str, read: Result<Option<Value>, ReadError>| {
            let mut aside = |reason: String| {
                loaded.set_aside.push(SetAside {
                    entity: key.to_owned(),
                    component: Some(name.to_owned()),
                    reason,
                });
            };
            let value = match read {
                Ok(value) => value,
                Err(error) => {
                    aside(error.to_string());
                    None
                }
            };
            for unknown in names.unknown.take() {
                aside(format!(
                    "it refers to `{unknown}`, which the world does not have: to no entity now"
                ));
            }
            value
        };
        for key in document.keys(&[CREATED]) {
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
        let numbers = save.numbers;
        // Why a difference is set aside whose entity, `key`, the world does not have.
        let gone = |key: &str| {
            if numbers.doubtful(key) {
                START_CHANGED
            } else {
                "the world has no entity of this identity"
            }
        };
        // Found before any is despawned: removing an entity removes its descendants, which the
        // save names too.
        let mut removed = Vec::new();
        for key in &save.removed {
            match self.entity_named(numbers, key) {
                Some(entity) => removed.push(entity),
                // Destroyed already, with a scene that another under its key replaced.
                None if save.gone.contains(key) => {}
                None => aside(key, None, gone(key)),
            }
        }
        // Every entity given another parent is taken out first, then put where the save says: one
        // at a time, a child could not take its parent's place, as its parent would be its
        // descendant still.
        let mut moved = Vec::new();
        for (key, parent) in &save.parents {
            let Some(entity) = self.entity_named(numbers, key) else {
                aside(key, None, gone(key));
                continue;
            };
            let parent = match parent {
                Some(parent) => match self.entity_named(numbers, parent) {
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
            let Some(entity) = self.entity_named(numbers, key) else {
                for name in names {
                    aside(key, Some(name), gone(key));
                }
                continue;
            };
            for name in names {
                if self.remove_component_value(entity, name).is_none() {
                    aside(key, Some(name), "the entity holds no such component");
                }
            }
        }
        for (key, name, value) in save.added {
            let Some(entity) = self.entity_named(numbers, &key) else {
                aside(&key, Some(&name), gone(&key));
                continue;
            };
            if let Err(error) = self.insert_component_value(entity, &name, value) {
                aside(&key, Some(&name), &error.to_string());
            }
        }
        for (key, name, part) in save.changed {
            let Some(entity) = self.entity_named(numbers, &key) else {
                aside(&key, Some(&name), gone(&key));
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
    fn entity_named(&self, numbers: Numbers, text: &str) -> Option<Entity> {
        self.find(&numbers.identity(text)?)
    }

    /// The entities that have a persistent identity, by their identities.
    fn named_entities(&self) -> BTreeMap<PersistentId, Entity> {
        self.entities()
            .filter_map(|entity| Some((self.persistent_id(entity)?, entity)))
            .collect()
    }

    /// The fingerprint of the entities that this world created outside scenes and holds: their
    /// numbers, the names of their components' types, and their [`Name`]s. A save holds its
    /// start's, so that loading it tells whether the start it is loaded into created the same
    /// entities, which its differences name by their numbers.
    fn created_fingerprint(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325;
        for (number, entity) in self.created.alive(&self.entities) {
            hash = fingerprint(hash, &number.to_le_bytes());
            for name in self.component_names_sorted(entity) {
                hash = fingerprint(fingerprint(hash, name.as_bytes()), &[0]);
            }
            if let Some(Name(name)) = self.get::<Name>(entity) {
                hash = fingerprint(hash, name.as_bytes());
            }
            hash = fingerprint(hash, &[0xff]);
        }
        hash
    }

    /// The entities that a save holds, by their persistent identities: all but those that hold
    /// [`Unsaved`], or whose ancestor does.
    fn saved_entities(&self) -> BTreeMap<PersistentId, Entity> {
        let mut entities = self.named_entities();
        entities.retain(|_, &mut entity| !self.unsaved(entity));
        entities
    }

    /// Whether the entity, or one of its ancestors, holds [`Unsaved`].
    pub(crate) fn unsaved(&self, entity: Entity) -> bool {
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
    pub(crate) fn component_names_sorted(&self, entity: Entity) -> Vec<&str> {
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
    /// How it names the entities created outside scenes.
    numbers: Numbers,
    removed: Vec<String>,
    removed_components: Vec<(String, Vec<String>)>,
    /// Each entity given another parent, the created ones included, and its parent, if it has one.
    parents: Vec<(String, Option<String>)>,
    /// The components added, of the entities the world had and of those it created: the entity,
    /// the component and its value.
    added: Vec<(String, String, Value)>,
    /// What changed of the components: the entity, the component and the part.
    changed: Vec<(String, String, Value)>,
    /// The identities of the entities that loading destroyed when it spawned a scene under a key in
    /// place of another: the save removes them, and nothing is reported of them.
    gone: HashSet<String>,
}

/// What a save holds that refuses it if it is wrong, read before the world changes.
#[derive(Default)]
struct Plan {
    /// What the world that made the save spawned while it ran.
    spawned: Vec<ToSpawn>,
    /// The entities that the save creates: their numbers in this world, or their keys.
    created: Vec<Creation>,
}

/// An entity that a save creates: under a number of the world's count, or a key
/// ([`World::spawn_keyed`]).
enum Creation {
    Number(u64),
    Key(PersistentId),
}

/// How a save names the entities that a world created outside scenes, and what this world names
/// them as it loads the save ([`World::load`]): what the run created, from the count that the start
/// it was saved against reached up, takes numbers from this world's count up, whatever the game's
/// start creates now; what that start created keeps its numbers, unless this world's start created
/// other entities, by the fingerprint that the save holds of them, when it may be another entity.
#[derive(Clone, Copy)]
struct Numbers {
    /// The count that the save's start reached.
    from: u64,
    /// This world's count before the save is loaded.
    to: u64,
    /// Whether this world's start created other entities outside scenes than the save's.
    start_changed: bool,
}

impl Default for Numbers {
    fn default() -> Self {
        Self::SAME
    }
}

impl Numbers {
    /// The identities as the save gives them: for a save of layout 2 or earlier, which does not say
    /// what its start created, and for a world's documents other than saves loaded.
    const SAME: Self = Self {
        from: u64::MAX,
        to: u64::MAX,
        start_changed: false,
    };

    /// The number in this world of what the save numbers `number`, if a world can give it.
    fn number(self, number: u64) -> Option<u64> {
        if number < self.from {
            return Some(number);
        }
        (number - self.from)
            .checked_add(self.to)
            .filter(|&number| number <= LARGEST_CREATED)
    }

    /// The identity in this world of the entity that the save names `text`; `None` if the text
    /// names no entity, or an entity that the save's start created when this world's start
    /// created others.
    fn identity(self, text: &str) -> Option<PersistentId> {
        let id: PersistentId = text.parse().ok()?;
        let Some(number) = created_root(&id) else {
            return Some(id);
        };
        if number < self.from {
            return (!self.start_changed).then_some(id);
        }
        let number = self.number(number)?.to_string();
        let parts = [CREATED_KEY, number.as_str()].into_iter();
        Some(PersistentId::new(
            parts.chain(id.parts()[2..].iter().map(String::as_str)),
        ))
    }

    /// Whether `text` names an entity that the save's start created, which this world's start may
    /// have given to another entity.
    fn doubtful(self, text: &str) -> bool {
        self.start_changed
            && text
                .parse()
                .ok()
                .and_then(|id| created_root(&id))
                .is_some_and(|number| number < self.from)
    }
}

/// The number of the entity created outside scenes that `id` names, or names a placement's entity
/// under: `created/7`, `created/7/light`.
fn created_root(id: &PersistentId) -> Option<u64> {
    let parts = id.parts();
    if parts.len() < 2 {
        return None;
    }
    PersistentId::new(parts[..2].iter().map(String::as_str)).created_number()
}

/// Why a save's difference is set aside when its entity is an entity that the save's start created,
/// and this world's start created others.
const START_CHANGED: &str = "the game's start creates other entities outside scenes than when the \
                             save was made, so that this one may be another: a key names it \
                             whatever the start creates (`World::spawn_keyed`)";

/// A scene that the world that made a save spawned while it ran, as the save gives it.
struct ToSpawn {
    /// The persistent identity of its root, as the save writes it.
    key: String,
    identity: SpawningAs,
    /// Its asset ID, its name and its fingerprint, as the save writes them.
    asset: String,
    name: String,
    content: String,
}

/// `value` with its references to the entities that `names` writes as no entity, destroyed or left
/// out, made references to no entity.
fn to_no_entity(mut value: Value, names: &Names) -> Value {
    for_each_entity(&mut value, &mut |bits| {
        if names.name(*bits).as_deref() == Some(NO_ENTITY) {
            *bits = handle_bits(Entity::NONE);
        }
    });
    value
}

/// A scene's fingerprint as a save writes it: sixteen hexadecimal digits.
fn content_text(fingerprint: u64) -> String {
    format!("{fingerprint:016x}")
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

/// A scene that a world spawned while it ran, as a save writes it: under a key that the start did
/// not spawn, or as a placement made while the world ran.
struct Spawned<'w> {
    /// The persistent identity of its root: the key, or `created/<number>`.
    root: PersistentId,
    scene: &'w ResolvedScene,
    /// The name that spawning gives the root unless the scene gives one: the key; none for a
    /// placement.
    key: Option<&'w str>,
    /// The persistent identity of each record's entity, in the records' order.
    ids: Vec<PersistentId>,
    /// Each record's entity in the world, if one is alive.
    entities: Vec<Option<Entity>>,
    /// Whether each record, or an ancestor of it, holds [`Unsaved`], which a save leaves out.
    unsaved: Vec<bool>,
}

/// What a save compares an entity of the world with.
#[derive(Clone, Copy)]
enum Before<'a> {
    /// The start's entity of the same identity.
    Start(Entity),
    /// The record at this index of a scene spawned while the world ran.
    Spawned(&'a Spawned<'a>, usize),
}

impl Before<'_> {
    /// The persistent identity of the parent, if one.
    fn parent(self, start: &World) -> Option<PersistentId> {
        match self {
            Before::Start(entity) => start
                .parent(entity)
                .and_then(|parent| start.persistent_id(parent)),
            Before::Spawned(spawned, index) => spawned.scene.0.records[index]
                .parent
                .map(|parent| spawned.ids[parent as usize].clone()),
        }
    }

    /// The names of the components' types, in their order.
    fn component_names(self, start: &World) -> Vec<String> {
        match self {
            Before::Start(entity) => start
                .component_names_sorted(entity)
                .into_iter()
                .map(str::to_owned)
                .collect(),
            Before::Spawned(spawned, index) => {
                let record = &spawned.scene.0.records[index];
                let mut names: Vec<String> = record
                    .components
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect();
                if index == 0 && spawned.key.is_some() && !names.iter().any(|name| name == "Name") {
                    names.push("Name".to_owned());
                }
                names.sort_unstable();
                names
            }
        }
    }

    /// The value of the component named `name`, if it holds one, its references made references to
    /// the entities of `world` of the identities that they name, as the world saved and `start`
    /// give the same entity other handles when they were not made in the same order.
    fn value(self, start: &World, world: &World, name: &str) -> Option<Value> {
        match self {
            Before::Start(entity) => start.component_value(entity, name).map(|mut value| {
                for_each_entity(&mut value, &mut |bits| {
                    let found = Entity::from_value(&Value::Entity(*bits))
                        .ok()
                        .and_then(|entity| start.persistent_id(entity))
                        .and_then(|id| world.find(&id));
                    *bits = handle_bits(found.unwrap_or(Entity::NONE));
                });
                value
            }),
            Before::Spawned(spawned, index) => expected(
                &spawned.scene.0.records[index],
                index,
                spawned.key,
                &spawned.entities,
                name,
            ),
        }
    }
}

/// What a scene spawned while a world ran was spawned as.
enum SpawningAs {
    /// A scene under this key.
    Key(String),
    /// A placement made while the world ran, under this number.
    Placement(u64),
}

impl World {
    /// Whether `start` spawned the scene that this world spawned under `key`: the same asset under
    /// the same key.
    fn spawned_as_at_start(&self, start: &World, key: &str) -> bool {
        match (self.scenes.get(key), start.scenes.get(key)) {
            (Some(ours), Some(theirs)) => ours.scene.id() == theirs.scene.id(),
            _ => false,
        }
    }

    /// What this world spawned while it ran, against `start`, of which an entity is saved, `saved`:
    /// the scenes spawned under keys that `start` did not spawn, then the placements made while the
    /// world ran that `start` did not make, in the order of their roots' identities.
    fn spawned_while_running<'w>(
        &'w self,
        start: &World,
        saved: &BTreeMap<PersistentId, Entity>,
    ) -> Vec<Spawned<'w>> {
        let keys = self
            .scenes
            .iter()
            .filter(|(key, _)| !self.spawned_as_at_start(start, key))
            .map(|(key, spawned)| {
                (
                    PersistentId::new([key.as_str()]),
                    &spawned.scene,
                    Some(key.as_str()),
                )
            });
        let placements = self
            .placements
            .iter()
            .filter(|(number, _)| !start.placements.contains_key(*number))
            .map(|(&number, scene)| (PersistentId::created(number), scene, None));
        let mut spawned: Vec<Spawned<'w>> = keys
            .chain(placements)
            .filter_map(|(root, scene, key)| {
                let records = &scene.0.records;
                let ids = record_ids(&root, scene);
                if !ids.iter().any(|id| saved.contains_key(id)) {
                    return None;
                }
                // Followed up through the parents, which a scene file may give after their children.
                let unsaved = (0..records.len())
                    .map(|index| {
                        let mut next = Some(index);
                        while let Some(at) = next {
                            let record = &records[at];
                            if record.components.iter().any(|(name, _)| name == "Unsaved") {
                                return true;
                            }
                            next = record.parent.map(|parent| parent as usize);
                        }
                        false
                    })
                    .collect();
                Some(Spawned {
                    entities: ids.iter().map(|id| self.find(id)).collect(),
                    root,
                    scene,
                    key,
                    ids,
                    unsaved,
                })
            })
            .collect();
        spawned.sort_by(|a, b| a.root.cmp(&b.root));
        spawned
    }
}

#[cfg(test)]
mod tests {
    use glam::{DVec3, Mat4, Vec3};
    use slime_assets::{AssetId, CookedScene, SceneNode};

    use super::*;
    use crate::{Hidden, Meshes, Name, ResolvedScene, Transform};

    /// `scene` resolved, as a game resolves a glTF scene that it spawns alone.
    fn resolved(scene: &CookedScene) -> ResolvedScene {
        ResolvedScene::from_cooked(AssetId::from_u128(1), scene, World::new().registry())
            .expect("a scene of the engine's components")
    }

    fn node(name: &str, parent: Option<u32>, at: [f32; 3]) -> SceneNode {
        SceneNode {
            name: name.to_owned(),
            parent,
            transform: Mat4::from_translation(Vec3::from(at)).to_cols_array(),
            meshes: vec![AssetId::from_u128(7)],
            light: None,
            extras: Vec::new(),
            components: Vec::new(),
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
        world
            .spawn_scene("yard", &resolved(&scene()))
            .expect("spawned");
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
            r#"layout = 3
next_created = 3
start = { created = 1, content = "cbf29ce484222325" }

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
            .spawn_scene("yard", &resolved(&scene_changed()))
            .expect("spawned");
        let report = world.load(&save, &mut SceneLibrary::new()).expect("loaded");
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
            ("layout = 4\n", "a save of layout 4"),
            (
                "[spawned.\"created/x\"]\nscene = \"\"\n",
                "not the identity",
            ),
            ("[created.\"yard/gate\"]\nparent = []\n", "not the identity"),
            // Its entity created, then taken back when the count cannot be read.
            (
                "next_created = true\n[created.\"created/7\"]\n",
                "cannot be read",
            ),
        ] {
            let mut world = start();
            let error = world.load(text, &mut SceneLibrary::new()).expect_err(text);
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
            .load(
                "next_created = 2\n[created.\"created/5\"]\n",
                &mut SceneLibrary::new(),
            )
            .expect("loaded");
        let next = world.spawn();
        assert_eq!(world.persistent_id(next), Some(PersistentId::created(6)));
        // A second load, into a world whose start created more: what the run created takes the
        // numbers that follow the world's.
        let mut world = start();
        world.load(&save, &mut SceneLibrary::new()).expect("loaded");
        world.load(&save, &mut SceneLibrary::new()).expect("twice");
        assert_eq!(
            world.get::<Name>(entity(&world, "created/3")),
            Some(&Name("torch".to_owned()))
        );
        // A save of 0.4, of layout 2, which does not say what its start created, gives the numbers
        // that it holds: they are in use.
        let mut world = start();
        world.load(&save, &mut SceneLibrary::new()).expect("loaded");
        let old = save.replace("layout = 3", "layout = 2");
        let old = old.replace(
            "start = { created = 1, content = \"cbf29ce484222325\" }\n",
            "",
        );
        let error = world
            .load(&old, &mut SceneLibrary::new())
            .expect_err("in use");
        assert!(matches!(error, DocumentError::IdentityInUse(id) if id.to_string() == "created/1"));
        // Truncated anywhere, a save gives an error or loads what it holds, and never panics.
        for end in (0..save.len()).filter(|&end| save.is_char_boundary(end)) {
            let _ = start().load(&save[..end], &mut SceneLibrary::new());
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
    fn a_save_of_0_3_loads_as_it_did() {
        // As 0.3 wrote it: no layout, and a rotation changed by the coordinates that changed.
        let mut loaded = start();
        let save =
            "next_created = 2\n\n[scenes]\nyard = { name = \"yard\", content = \"SCENE\" }\n\n\
                    [created.\"created/1\"]\nparent = [\"yard/well\"]\n\n\
                    [created.\"created/1\".components]\nName = \"torch\"\n\n\
                    [changed.\"yard/well\".Transform]\nrotation = { y = 0.6, w = 0.8 }\n"
                .replace("SCENE", &format!("{:016x}", loaded.scenes["yard"].content));
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
        assert!(
            report.set_aside.is_empty()
                && report.scenes_changed.is_empty()
                && report.warnings.is_empty(),
            "{report:?}"
        );
        let well = entity(&loaded, "yard/well");
        assert_eq!(
            loaded.get::<Transform>(well).expect("held").rotation,
            glam::Quat::from_xyzw(0.0, 0.6, 0.0, 0.8)
        );
        assert_eq!(loaded.parent(entity(&loaded, "created/1")), Some(well));
        assert!(
            loaded
                .save(&start(), &BTreeMap::new())
                .expect("saved")
                .starts_with("layout = 3\n")
        );
    }

    #[test]
    fn a_save_refused_leaves_the_world_as_it_was_whatever_it_spawns_first() {
        let mut scenes = SceneLibrary::new();
        scenes.insert_cooked(AssetId::from_u128(1), scene());
        let yard = "{ scene = \"00000000-0000-0000-0000-000000000001\" }";
        for (text, refused) in [
            (
                format!("[spawned]\ncave = {yard}\n\"created/01\" = {yard}\n"),
                "not the identity",
            ),
            (
                format!("[spawned]\ncave = {yard}\nbad = {{ scene = 5 }}\n"),
                "cannot be read",
            ),
            (
                format!("[spawned]\n\"created/5\" = {yard}\n\"created/1\" = {yard}\n"),
                "has already",
            ),
            (
                format!("[spawned]\ncave = {yard}\n\n[created.\"created/1\"]\n"),
                "has already",
            ),
        ] {
            let made = || {
                let mut world = start();
                world.spawn();
                world
            };
            let mut world = made();
            let error = world.load(&text, &mut scenes).expect_err(&text);
            assert!(
                format!(
                    "{error}: {}",
                    std::error::Error::source(&error).map_or(String::new(), ToString::to_string)
                )
                .contains(refused),
                "{text}: {error:?}"
            );
            assert_eq!(world.first_difference(&made()), None, "{text}");
            assert!(
                !world.scenes.contains_key("cave") && world.placements.is_empty(),
                "{text}"
            );
            assert!(world.find(&PersistentId::created(5)).is_none(), "{text}");
            assert_eq!(world.next_created, 2, "{text}");
        }
        let save = "[spawned]\n\"created/4\" = { scene = \"00000000-0000-0000-0000-000000000001\" }\n\
                    cave = { scene = \"00000000-0000-0000-0000-000000000001\" }\n\n\
                    [created.\"yard/gate\"]\nparent = []\n";
        let mut world = start();
        assert!(matches!(
            world.load(save, &mut scenes),
            Err(DocumentError::NotCreated(_))
        ));
        assert_eq!(world.first_difference(&start()), None);
        assert!(!world.scenes.contains_key("cave") && world.placements.is_empty());
        let next = world.spawn();
        assert_eq!(world.persistent_id(next), Some(PersistentId::created(1)));
        // Without what failed, it spawns both.
        let save = save.replace("[created.\"yard/gate\"]\nparent = []\n", "");
        let report = world.load(&save, &mut scenes).expect("loaded");
        assert_eq!(
            report.scenes_changed,
            ["created/4", "cave"],
            "no content said"
        );
        assert!(
            world
                .find(&"created/4/well".parse().expect("an identity"))
                .is_some()
        );
        assert!(
            world
                .find(&"cave/well".parse().expect("an identity"))
                .is_some()
        );
        assert_eq!(world.next_created, 5);
    }

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
        loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
            .load(
                "[added.\"yard/gate\"]\n\"test::Rope\" = { to = \"yard/nothing\" }\n",
                &mut SceneLibrary::new(),
            )
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
            let error = start()
                .load(text, &mut SceneLibrary::new())
                .expect_err(text);
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
            world.load(&text, &mut SceneLibrary::new()).expect(&text);
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
            .load("[removed]\nentities = [\"yard/gate\"]\n\n[parents]\n\"yard/post\" = [\"yard/shed\"]\n", &mut SceneLibrary::new())
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
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.load(text, &mut SceneLibrary::new())
        }));
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
        let report = loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
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
        loaded
            .load(&save, &mut SceneLibrary::new())
            .expect("loaded");
        eprintln!("load: {:?}", started.elapsed());
        assert_eq!(played.first_difference(&loaded), None);
    }
}
