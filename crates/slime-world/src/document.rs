//! The world's documents: a save, what a run changed of the world as the game made it and settled
//! it, and that applied again to a world made so (`docs/adr/0087-the-worlds-documents.md`,
//! `docs/adr/0093-saves-and-hot-reload-with-placements.md`, `docs/adr/0125-saves-per-scene.md`).

use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::mem;

use slime_assets::AssetId;
use slime_reflect::{Problem, Reflect, ReflectError, Registry, Value};
use slime_serialize::{Composer, Document, Mode, ReadError, ReadProblem, References};

use crate::change::Tick;
use crate::components::{Driven, Name, Unsaved};
use crate::entity::Entity;
use crate::identity::{CREATED as CREATED_KEY, PersistentId};
use crate::library::{MAX_ENTITIES, MAX_SCENE_SIZE, ResolvedScene, SceneLibrary, placed_size};
use crate::scene::{record_ids, storages_of, valid_key};
use crate::scene_file::{LAYOUT, for_each_entity, table};
use crate::unload::{Table, Unloaded};
use crate::world::{Former, Identity, World};
use crate::write_back::handle_bits;

/// What the game says of the save: its keys and texts.
const ABOUT: &str = "about";
/// The count of the entities created outside scenes.
const NEXT_CREATED: &str = "next_created";
/// The count of the entities created outside scenes that the start of the run reached, after which
/// the run's are numbered.
const START: &str = "start";
/// The layout of saves that this program writes, and the only one that it reads
/// (`docs/adr/0125-saves-per-scene.md`): 5, the differences of each entity from its base. 1 was
/// that of the saves of 0.3, 2 of 0.4, 3 of those written since P1, 4 of those of M6a.7a: each
/// held what differs from a start world that the game made again.
const LAYOUT_VERSION: u64 = 5;
/// The scenes spawned under keys, the last under each, by their keys: their asset IDs, names and
/// contents.
const SCENES: &str = "scenes";
/// The placements made while the world ran of which an entity is alive, by the identities of their
/// roots, `created/<number>`: their prefabs' asset IDs, names and contents, and the identities that
/// a scene file written from the world gives them.
const SPAWNED: &str = "spawned";
/// The entities and the components removed.
pub(crate) const REMOVED: &str = "removed";
/// The entities given another parent, or none.
const PARENTS: &str = "parents";
/// The entities created outside scenes, whole: each with its parent, its components and, for one
/// that a scene file written from the world holds, the identity that it has there, `written`.
const CREATED: &str = "created";
/// The identity of an entity created outside scenes in the scene file written from the world
/// ([`World::write_scene`]).
const WRITTEN: &str = "written";
/// The components added, whole.
pub(crate) const ADDED: &str = "added";
/// What changed of the components, in parts.
pub(crate) const CHANGED: &str = "changed";
/// What the world keeps of the scenes that it unloaded, by their keys and their asset IDs: their
/// entities and components removed, their parents changed, and their components added and changed,
/// in sections laid out as the save's own.
const UNLOADED: &str = "unloaded";
/// The identities that the entities adopted had in their scenes, by the keys that adopting gave
/// them.
const ADOPTED: &str = "adopted";

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
    /// already, or that the save gives both to a scene that it spawns and to an entity that it
    /// creates.
    #[error(
        "the save creates or places `{0}`, an identity that an entity of the world, or of the save, \
         has already"
    )]
    IdentityInUse(PersistentId),
    /// The start of the world's run created, outside scenes, entities that saves hold
    /// and that have no key, each named by its identity and its [`Name`] if it has one: a later
    /// version of the game that creates them in another order would apply the save's changes to
    /// others. A game creates what its start needs under a key, an entity with
    /// [`World::spawn_keyed`] and a prefab with [`World::spawn_scene`] rather than
    /// [`World::spawn_placement`], or leaves it out of saves ([`Unsaved`])
    /// (`docs/adr/0109-stable-references-and-undo.md`).
    #[error(
        "the start created, without a key, {}: a save cannot name them for good; create them with \
         World::spawn_keyed, a prefab with World::spawn_scene, or mark them Unsaved",
        .0.join(", ")
    )]
    UnkeyedAtStart(Vec<String>),
    /// The save is of another layout than the one that this program reads: one that an earlier
    /// version of the engine wrote, which it no longer reads, or a later version
    /// (`docs/adr/0125-saves-per-scene.md`).
    #[error("{}", layout_refused(*found, *read))]
    Layout {
        /// The layout that the save says: 1 for one that says none, as those of 0.3.
        found: u64,
        /// The layout that this program reads.
        read: u64,
    },
    /// The world was never settled ([`World::settle`]): a save cannot tell what the game made at
    /// the start of the run from what the run changed.
    #[error("the world was never settled: a game settles it once it has made the start of a run")]
    NotSettled,
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

/// Why a save of the layout `found` is refused by a program that reads the layout `read`.
fn layout_refused(found: u64, read: u64) -> String {
    let wrote = if found < read {
        "an earlier version of the engine wrote, and which this program no longer reads"
    } else {
        "a later version of the engine wrote"
    };
    format!("a save of layout {found}, which {wrote}: this program reads saves of layout {read}")
}

/// Checks that `document` is a save of the layout that this program reads.
fn check_layout(document: &mut Document<'_>) -> Result<(), DocumentError> {
    // A save that says no layout is one of 0.3, of layout 1; a document that holds nothing is none.
    let found = match document
        .read(&[LAYOUT], "u64", None)
        .map_err(DocumentError::Read)?
    {
        Some(Value::UInt(layout)) => layout,
        _ if document.keys(&[]).is_empty() => {
            let problem =
                Problem::Invalid("the document holds nothing: it is not a save".to_owned());
            return Err(DocumentError::Read(document.error_at(&[], problem)));
        }
        _ => 1,
    };
    if found != LAYOUT_VERSION {
        return Err(DocumentError::Layout {
            found,
            read: LAYOUT_VERSION,
        });
    }
    Ok(())
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
    /// the world does not have refers to no entity. Each names its entity as the save does: the
    /// number of an entity that the run created is the save's, which the world loaded may give
    /// another entity.
    pub set_aside: Vec<SetAside>,
    /// What the save holds that this program does not know, left out: fields, types, variants, and
    /// sections of the save; the first [`slime_serialize::MAX_WARNINGS`], then one that counts the
    /// rest. A component of a type, or whose value names a variant, that this
    /// program does not know is left out whole, as a later version of the game writes them: a game
    /// that writes the save back without them loses them.
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
fn sections(document: &mut Document<'_>) -> Result<(), DocumentError> {
    let mut section = |place: &[&str], known: Option<&[&str]>| {
        let keys = table(document, place).map_err(DocumentError::Read)?;
        for key in keys
            .iter()
            .filter(|key| known.is_some_and(|known| !known.contains(&key.as_str())))
        {
            let mut at = place.to_vec();
            at.push(key);
            let unknown = document.error_at(
                &at,
                Problem::UnknownField {
                    field: key.clone(),
                    known: known
                        .into_iter()
                        .flatten()
                        .map(|&key| key.to_owned())
                        .collect(),
                },
            );
            document.warn(unknown);
        }
        Ok::<_, DocumentError>(keys)
    };
    let root = [
        ABOUT,
        LAYOUT,
        NEXT_CREATED,
        START,
        ADOPTED,
        SCENES,
        SPAWNED,
        REMOVED,
        PARENTS,
        ADDED,
        CHANGED,
        CREATED,
        UNLOADED,
    ];
    section(&[], Some(&root))?;
    section(&[ABOUT], None)?;
    section(&[START], Some(&["created"]))?;
    for (name, known) in [
        (ADOPTED, Some(&["former", "scene"][..])),
        (SCENES, Some(&["scene", "name", "content"][..])),
        (SPAWNED, Some(&["scene", "name", "content", WRITTEN][..])),
        (CREATED, Some(&["parent", "components", WRITTEN][..])),
    ] {
        for key in section(&[name], None)? {
            section(&[name, &key], known)?;
            if name == CREATED {
                section(&[name, &key, "components"], None)?;
            }
        }
    }
    // The differences of the world, and of each scene that it keeps unloaded, laid out alike.
    differences(&mut section, &[])?;
    for key in section(&[UNLOADED], None)? {
        for asset in section(&[UNLOADED, &key], None)? {
            differences(&mut section, &[UNLOADED, &key, &asset])?;
        }
    }
    Ok(())
}

/// What checks the table of a save at a place against the keys that it may hold, if it says them,
/// and gives its keys.
type Section<'a> = dyn FnMut(&[&str], Option<&[&str]>) -> Result<Vec<String>, DocumentError> + 'a;

/// Checks the differences that a save holds at `place`, of the world or of a scene that it keeps
/// unloaded, through `section`.
fn differences(section: &mut Section<'_>, place: &[&str]) -> Result<(), DocumentError> {
    let mut check = |rest: &[&str], known: Option<&[&str]>| {
        let at: Vec<&str> = place.iter().chain(rest).copied().collect();
        section(&at, known)
    };
    check(&[PARENTS], None)?;
    check(&[REMOVED], Some(&["entities", "components"]))?;
    check(&[REMOVED, "components"], None)?;
    for name in [ADDED, CHANGED] {
        for key in check(&[name], None)? {
            check(&[name, &key], None)?;
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
    /// What a save names by the numbers of its creations and that the world has under the
    /// identities that a scene file gave it ([`Save::aliases`]).
    aliases: HashMap<String, PersistentId>,
    /// While it writes what the world keeps of a scene unloaded, the identities that its values
    /// refer to, by the bits of handles of generation 0 that stand for them ([`Unloaded`]).
    table: Cell<Option<&'w [PersistentId]>>,
}

impl<'w> Names<'w> {
    fn new(world: &'w World, numbers: Numbers) -> Self {
        Self {
            world,
            numbers,
            unknown: RefCell::new(Vec::new()),
            aliases: HashMap::new(),
            table: Cell::new(None),
        }
    }
}

impl References for Names<'_> {
    /// An entity destroyed, or that the save leaves out, is no entity, [`NO_ENTITY`].
    fn name(&self, entity: u64) -> Option<String> {
        if let Some(table) = self.table.get()
            && entity >> 32 == 0
        {
            let id = table.get(entity as usize);
            return Some(id.map_or_else(|| NO_ENTITY.to_owned(), ToString::to_string));
        }
        let world = self.world;
        let id = Entity::from_value(&Value::Entity(entity))
            .ok()
            .filter(|&entity| !world.unsaved(entity))
            .and_then(|entity| world.persistent_id(entity));
        Some(id.map_or_else(|| NO_ENTITY.to_owned(), |id| id.to_string()))
    }

    /// No entity, or one that the world does not have, is a handle that no world gives out.
    fn entity(&self, name: &str) -> Option<u64> {
        let found = aliased(&self.aliases, name)
            .or_else(|| self.numbers.identity(name))
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
    /// The run that the world holds, as the text of a save (`docs/adr/0125-saves-per-scene.md`):
    /// what changed since the world was settled ([`World::settle`]), as the game made the start of
    /// the run and each scene that it spawned since, and what the run created since.
    ///
    /// - The scenes spawned under keys, by key: their asset IDs, names and fingerprints, so that
    ///   loading spawns those that the world it is loaded into lacks, or holds others in place of,
    ///   and says which changed since; but a scene of which entities live that the save leaves out
    ///   every one. The placements made while the world ran of which the save holds an entity, as
    ///   their prefabs.
    /// - The entities that the world settled and that the run destroyed, whose identities the world
    ///   kept as they went: the entities of a placement that the save does not name are not.
    /// - For each entity settled: the components removed, added, whole, and changed, in parts, and
    ///   its parent if it changed. Each is compared with its base, which the world kept at its
    ///   first change: a save costs what changed. A component that its entity marks driven
    ///   ([`Driven`]), at its base and now, is not saved: loading leaves it as the scene gives it,
    ///   its author's, and its system computes it again; a mark that the run added or took off
    ///   hides nothing.
    /// - The entities created since the start, outside scenes or under keys, and those adopted,
    ///   whole: their parents and components, and the identity that a scene file written from the
    ///   world gives one ([`World::write_scene`]); the former identities of those adopted
    ///   ([`World::adopt`]).
    /// - What the world keeps of the scenes that it unloaded, by key and asset, laid out as what it
    ///   saves of the scenes that it holds ([`World::unload_scene`]).
    ///
    /// Entities are named by their persistent identities. Those that hold [`Unsaved`], or whose
    /// ancestor does, are left out, as are the fields marked transient: an entity settled that the
    /// world leaves out is not removed, and loading keeps it as the start has it. A reference to an
    /// entity is its persistent identity, and one to an entity destroyed or left out is `""`, no
    /// entity. The save also holds the count of the entities created outside scenes, and the count
    /// that the start reached, after which the run's are numbered. Its first section, after the
    /// versions of the types past version 1 if it holds some, is `about`, what the game says of it,
    /// keys and texts that the engine does not read, such as what the run was made on, which
    /// [`World::about`] reads without loading the save; none for an empty `about`.
    ///
    /// The same world gives the same bytes: entities in the order of their identities, components
    /// in the order of their names. It costs what changed, and what the run created, written
    /// whole: it reads the values changed, the entities destroyed and the scenes, and walks the
    /// world's entities once without their components, to find those created and those moved. The
    /// figures of `world.save.100k` are in `docs/measurements.md`. A save holds at most
    /// [`slime_serialize::MAX_SIZE`] bytes and [`slime_serialize::MAX_VALUES`] values, as a reader
    /// counts them.
    ///
    /// # Errors
    ///
    /// [`DocumentError::NotSettled`] if the world was never settled;
    /// [`DocumentError::UnkeyedAtStart`] if the start created, outside scenes, entities without
    /// keys that the save would hold; [`DocumentError::Write`] if a value cannot be written, a
    /// number out of its field's range, or the save would be larger, of more values or deeper than
    /// a reader reads ([`slime_serialize::MAX_SIZE`], [`slime_serialize::MAX_VALUES`],
    /// [`slime_serialize::MAX_DEPTH`]).
    pub fn save(&self, about: &BTreeMap<String, String>) -> Result<String, DocumentError> {
        if self.settled.is_none() {
            return Err(DocumentError::NotSettled);
        }
        let unkeyed: Vec<String> = self
            .unkeyed_at_start
            .iter()
            .filter_map(|&number| {
                let entity = self.created.find(number, &self.entities)?;
                (!self.unsaved(entity)).then(|| {
                    let id = PersistentId::created(number);
                    match self.get::<Name>(entity) {
                        Some(Name(name)) => format!("`{id}` ({name})"),
                        None => format!("`{id}`"),
                    }
                })
            })
            .collect();
        if !unkeyed.is_empty() {
            return Err(DocumentError::UnkeyedAtStart(unkeyed));
        }
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
        put(&[START, "created"], "u64", Value::UInt(self.start_created))?;
        for (key, former) in &self.formers {
            put(
                &[ADOPTED, key, "former"],
                "String",
                Value::Text(former.id.to_string()),
            )?;
            let scene = Value::Text(former.scene.to_string());
            put(&[ADOPTED, key, "scene"], "String", scene)?;
        }
        let spawned = self.spawned_in_save();
        for spawned in &spawned {
            let scene = &spawned.scene.0;
            let (section, key) = match spawned.key {
                Some(key) => (SCENES, key.to_owned()),
                None => (SPAWNED, spawned.root.to_string()),
            };
            put(
                &[section, &key, "scene"],
                "String",
                Value::Text(scene.id.to_string()),
            )?;
            put(
                &[section, &key, "name"],
                "String",
                Value::Text(scene.name.clone()),
            )?;
            let content = Value::Text(content_text(scene.fingerprint));
            put(&[section, &key, "content"], "String", content)?;
            if spawned.key.is_none()
                && let Some(written) = self.written_as(&spawned.root)
            {
                put(&[section, &key, WRITTEN], "String", Value::Text(written))?;
            }
        }
        drop(put);
        // What the world settled and the run destroyed, but the entities of the placements that the
        // save does not name, of which nothing that it holds lives.
        let named: HashSet<&PersistentId> = spawned.iter().map(|spawned| &spawned.root).collect();
        let removed: Vec<PersistentId> = self
            .gone
            .keys()
            .filter(|id| {
                created_root(id).is_none_or(|number| named.contains(&PersistentId::created(number)))
            })
            .cloned()
            .collect();
        // The entities that a save holds: those that have identities, but those that it leaves out.
        let saved = |entity: Entity| self.persistent_id(entity).filter(|_| !self.unsaved(entity));
        // What changed of each entity settled, by its components' bases, kept at their first
        // change; and the parents changed. What the run created since the start, unsettled, whole.
        let mut parents = BTreeMap::new();
        let mut created = BTreeMap::new();
        for entity in self.entities() {
            let node = &self.nodes[entity.index() as usize];
            if node.base == Tick::default() {
                if let Some(id) = saved(entity) {
                    created.insert(id, entity);
                }
            } else if let Some(before) = &node.parent_kept
                && let Some(id) = saved(entity)
            {
                let parent = self
                    .parent(entity)
                    .and_then(|parent| self.persistent_id(parent));
                if parent != *before {
                    parents.insert(id, parent);
                }
            }
        }
        let changes = self.changes_since_settled(&|entity| saved(entity).is_some());
        put_differences(&mut save, &[], &removed, &parents, &changes)?;
        for (id, entity) in created {
            let key = id.to_string();
            let parent = text_option(
                self.parent(entity)
                    .and_then(|parent| self.persistent_id(parent)),
            );
            save.put(&[CREATED, &key, "parent"], "Option<String>", &parent)
                .map_err(DocumentError::Write)?;
            if let Some(written) = self.written_as(&id) {
                save.put(&[CREATED, &key, WRITTEN], "String", &Value::Text(written))
                    .map_err(DocumentError::Write)?;
            }
            for name in self.component_names_sorted(entity) {
                let Some(value) = self.component_value(entity, name) else {
                    continue;
                };
                save.put(&[CREATED, &key, "components", name], name, &value)
                    .map_err(DocumentError::Write)?;
            }
        }
        // What the world keeps of the scenes that it unloaded, its references named by the
        // identities that it keeps.
        for ((key, asset), kept) in &self.unloaded {
            let asset = asset.to_string();
            names.table.set(Some(&kept.references));
            let parents = &kept.parents;
            let place = [UNLOADED, key.as_str(), asset.as_str()];
            put_differences(&mut save, &place, &kept.removed, parents, &kept.differences)?;
            names.table.set(None);
        }
        save.finish().map_err(DocumentError::Write)
    }

    /// Applies the save `text` to this world, which the game has made as at the start of a run,
    /// from the scenes as they are now, and settled ([`World::settle`]); a world never settled is
    /// settled first (`docs/adr/0125-saves-per-scene.md`).
    ///
    /// - The scenes that the save names under keys: one that this world holds is kept, its content
    ///   compared; one that it lacks, or holds another scene in place of, is spawned, resolved
    ///   through `scenes` as it is now, in place of this world's, as when a run changed level: what
    ///   is under that one's entities without being of it stays, and the save removes it, or places
    ///   it where the run left it, as a hero that the start placed in the level and that walked out
    ///   of it. `prepare` is called with the key of each scene spawned so, so that the game
    ///   prepares it as it prepares the scenes that it spawns, and the world is settled then: what
    ///   the save changed of them, it changes from there. The entities that the save creates exist
    ///   while they are prepared, without their components or their parents yet, and a preparation
    ///   may change the scene's entities only ([`World::settle`]). A scene that this world holds
    ///   and that the save does not name stays, as one that a later version of the game spawns at
    ///   its start; one that the save says the run unloaded is unloaded, and what the world keeps
    ///   of the scenes unloaded is the save's ([`World::unload_scene`]), but what the run kept of a
    ///   scene that it spawned again and had not settled, which is applied once the save is, as the
    ///   run's next settling would.
    /// - The placements that the save names are spawned again under the identities that they had,
    ///   but one that a scene file written from the world holds and that this world has under the
    ///   identity that the file gave it, which is set back to its prefab and takes the save's
    ///   differences ([`World::carry`]).
    /// - The entities that the save creates are spawned under their identities, but one that this
    ///   world has so, which takes what the save holds of it, whole; those adopted under their
    ///   keys, their former identities kept ([`World::adopt`]): this world adopts again the
    ///   entities that it holds under those identities, of the same scene, which take what the save
    ///   holds of them, so that what refers to them in their scene follows them, but what the run
    ///   destroyed of them.
    /// - The parents are set, an entity whose parent the world does not have out of its own; the
    ///   entities removed despawned, the entities saved under them that the save does not remove
    ///   taken out of them first; the components removed, added and changed; and the count of the
    ///   entities created outside scenes kept.
    ///
    /// What the run created outside scenes, its entities and its placements, takes the numbers that
    /// follow this world's count, whatever the start creates now; what the start of the run created
    /// keeps its numbers: a key names an entity whatever the start creates
    /// ([`World::spawn_keyed`]). What is set aside names its entity as the save does, a number of
    /// the run as the save numbers it ([`Loaded::set_aside`]). A save is read leniently: what this
    /// program does not know is left out, with a warning ([`Loaded::warnings`]), a section of the
    /// save, a field, and a component of a type or with a variant that it does not know included. A
    /// difference whose entity or component the world no longer has, or that the component's type
    /// refuses, is set aside, and the rest applied; so is a reference to an entity that the world
    /// no longer has, which refers to no entity then, a parent that the world does not have, and a
    /// scene that `scenes` cannot give, whose values its types refuse, or that would bring the
    /// entities that the save spawns so past [`MAX_ENTITIES`], or what they take past
    /// [`MAX_SCENE_SIZE`](crate::MAX_SCENE_SIZE), as a resolved scene's placements are bounded. The
    /// scenes that changed since the save, the placements included, are named. Loading entities
    /// created costs about three times as much as saving them: in a release build on the owner's
    /// machine, 10 000 entities created, each with a name and a transform, 2.9 MB of text, are
    /// saved in 0.15 s and loaded in 0.42 s, which gives what placements cost too.
    ///
    /// # Errors
    ///
    /// [`DocumentError::Layout`] if the save is of another layout than the one that this program
    /// reads, as the saves that the engine wrote before 0.5 are; [`DocumentError::Read`] if the
    /// save is not one: not TOML, of a later version of the format or of a type, too large, or with
    /// a section that is not what a save holds; [`DocumentError::NotCreated`] if it creates an
    /// entity, or places a placement, under an identity that a world does not give to the entities
    /// it creates; [`DocumentError::IdentityInUse`] if it creates or places one under an identity
    /// that an entity of the world has, or twice. All of it is read before the world changes, which
    /// is unchanged then, but settled if it never was.
    ///
    /// # Panics
    ///
    /// If the world runs out of slots ([`World::spawn`]).
    pub fn load(
        &mut self,
        text: &str,
        scenes: &mut SceneLibrary,
        prepare: &mut dyn FnMut(&mut World, &str),
    ) -> Result<Loaded, DocumentError> {
        // What the game made is the base of what the save changes.
        if self.settled.is_none() {
            self.settle();
        }
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
                .and_then(|mut document| apart.world.read(&apart.registry, &mut document, scenes))
        };
        let (mut save, mut loaded) = read?;
        // The scenes that the save spawned, prepared as the game prepares those it spawns, then
        // settled: what the save changes of them, it changes from there.
        for key in &save.spawned_keys {
            prepare(self, key);
        }
        self.settle();
        let pending = mem::take(&mut save.pending);
        self.apply_save(save, &mut loaded);
        for (key, asset, kept) in pending {
            self.apply_unloaded(&key, asset, kept);
        }
        Ok(loaded)
    }

    /// The run of this world carried into `start`, a world that the game made as at the start of a
    /// run from its scenes as they are now, and settled: what saving the run and loading the save
    /// into `start` give, `prepare` preparing the scenes that loading spawns ([`World::load`],
    /// `docs/adr/0125-saves-per-scene.md`), what loading set aside with it. A scene or a prefab
    /// changed reaches every entity that it spawned but what the run changed of it, its entities
    /// added appear, and what the run changed stays. An entity that the run created and that a scene
    /// file written from the world holds ([`World::write_scene`]) is the file's entity once the file
    /// is cooked again and spawned in `start`, given what the run holds of it, whole, rather than
    /// created a second time. Handles to entities change, as when a save is loaded. It costs a save
    /// and a load.
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
        mut start: World,
        scenes: &mut SceneLibrary,
        prepare: &mut dyn FnMut(&mut World, &str),
    ) -> Result<(World, Loaded), DocumentError> {
        let save = self.save(&BTreeMap::new())?;
        let loaded = start.load(&save, scenes, prepare)?;
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
    /// let mut world = World::new();
    /// world.settle();
    /// let about = BTreeMap::from([("made on".to_owned(), "the first zone".to_owned())]);
    /// let save = world.save(&about).expect("saved");
    /// assert_eq!(World::about(&save).expect("a save"), about);
    /// ```
    ///
    /// # Errors
    ///
    /// [`DocumentError::Read`] if the text is not a document: larger than a reader reads, not TOML,
    /// of a later version of the format; or if what it says of itself is not text;
    /// [`DocumentError::Layout`] if it is a save of another layout than the one that this program
    /// reads. It reads nothing else, and so accepts a save that [`World::load`] would refuse, of a
    /// later version of a type.
    pub fn about(text: &str) -> Result<BTreeMap<String, String>, DocumentError> {
        let mut registry = Registry::new();
        registry.register::<String>();
        registry.register::<u64>();
        let mut document =
            Document::parse(&registry, text, Mode::Lenient).map_err(DocumentError::Read)?;
        check_layout(&mut document)?;
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
    /// identities they refer to. The entities that hold [`Unsaved`], or whose ancestor does, the
    /// fields marked transient are left out, as a save leaves them out, and so are the values of the
    /// components that an entity of this world marks driven ([`Driven`]), which a system computes
    /// again, whatever a save holds of them. It tells whether a world loaded from a save is the
    /// world saved, or whether two runs went the same way. It compares the entities that the worlds
    /// hold, not what they keep of the scenes that they unloaded, which their saves compare.
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
            for name in names.into_iter().filter(|name| !self.drives(entity, name)) {
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
    /// the scenes that the run spawned, and the entities that it creates, so that the components'
    /// values read last, which can only be set aside, can refer to them.
    fn read(
        &mut self,
        registry: &Registry,
        document: &mut Document<'_>,
        scenes: &mut SceneLibrary,
    ) -> Result<(Save, Loaded), DocumentError> {
        let mut loaded = Loaded::default();
        let (mut save, plan) = self.read_layout(document, &mut loaded)?;
        // Nothing refuses the save from here on: the world changes. The scenes that the run
        // unloaded and that this world holds, unless the save names them under their keys, are
        // unloaded as the run left them: what the run kept of them is the save's. What it kept of a
        // scene that it spawned again, and had not settled, is applied once the save is, as the
        // run's next settling would apply it.
        let mut unloaded = plan.unloaded;
        for (key, asset) in unloaded.keys() {
            if let Some(held) = self.scenes.get(key)
                && held.scene.id() == *asset
                && !plan.listed.contains_key(key)
            {
                let held = held.scene.clone();
                self.despawn_scene(key, &held, &mut save.gone);
            }
        }
        let pending: Vec<(String, AssetId)> = unloaded
            .keys()
            .filter(|(key, asset)| plan.listed.get(key) == Some(&asset.to_string()))
            .cloned()
            .collect();
        for at in pending {
            if let Some(kept) = unloaded.remove(&at) {
                save.pending.push((at.0, at.1, kept));
            }
        }
        self.unloaded = unloaded;
        self.spawn_what_ran(registry, scenes, plan.spawned, &mut save, &mut loaded);
        // What the run adopted and holds, adopted again where this world holds it, of the same
        // scene, so that what refers to it in its scene, by its handle, refers to it still: the
        // save creates it whole; what the run destroyed of it is destroyed, and the records that
        // the save removes of it are not reported.
        let created: HashSet<&PersistentId> = plan
            .created
            .iter()
            .filter_map(|creation| match creation {
                Creation::Key(id, _) => Some(id),
                Creation::Number(..) => None,
            })
            .collect();
        for (key, former) in &plan.adopted {
            let root = PersistentId::new([key.as_str()]);
            if self.formers.get(key) == Some(former) || !created.contains(&root) {
                continue;
            }
            let Some(entity) = self.find(&former.id) else {
                continue;
            };
            if self.scene_of(&former.id) != Some(former.scene) || self.adopt(entity, key).is_err() {
                continue;
            }
            // What it destroys, what is under it taken out first, which the save places.
            let mut next = vec![entity];
            while let Some(at) = next.pop() {
                let children: Vec<Entity> = self.children(at).collect();
                next.extend(children.iter().copied());
                let doomed = self.persistent_id(at).is_some_and(|id| {
                    id.parts().starts_with(root.parts()) && !created.contains(&id)
                });
                if doomed {
                    for child in children {
                        // Both alive, so taking it out of its parent cannot fail.
                        let _ = self.set_parent(child, None);
                    }
                    self.despawn(at);
                }
            }
            let gone = save.removed.iter().filter(|removed| {
                removed
                    .parse::<PersistentId>()
                    .is_ok_and(|id| id.parts().starts_with(former.id.parts()))
            });
            save.gone.extend(gone.cloned());
        }
        self.formers = plan.adopted;
        self.create(registry, plan.created, &mut save);
        self.read_values(document, &mut save, &mut loaded);
        Ok((save, loaded))
    }

    /// Reads all that refuses the save `document` if it is wrong, without changing the world: its
    /// layout, its scenes, what the world that made it spawned while it ran, the numbers of the
    /// entities that it creates, its count, the entities and components that it removes, and the
    /// parents.
    fn read_layout(
        &self,
        document: &mut Document<'_>,
        loaded: &mut Loaded,
    ) -> Result<(Save, Plan), DocumentError> {
        check_layout(document)?;
        sections(document)?;
        let renumbering = match document
            .read(&[START, "created"], "u64", None)
            .map_err(DocumentError::Read)?
        {
            Some(Value::UInt(from)) => Numbers {
                from,
                to: self.next_created,
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
        // The identity that a scene file written from the world gives what the save creates or
        // places under `key` in `section`, if the save says one: an identity of a scene's entity.
        let written = |document: &mut Document<'_>, section: &str, key: &str| match document
            .read(&[section, key, WRITTEN], "String", None)
            .map_err(DocumentError::Read)?
        {
            Some(Value::Text(text)) => Ok(text
                .parse::<PersistentId>()
                .ok()
                .filter(|id| id.created_number().is_none() && id.parts().len() > 1)),
            _ => Ok(None),
        };
        // The keys that the save gives, to the scenes that it spawns and to the entities that it
        // creates, each once.
        let mut keys = HashSet::new();
        for key in document.keys(&[SPAWNED]) {
            let identity = SpawningAs::Placement(number(&key, key.parse().ok())?);
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
                written: written(document, SPAWNED, &key)?,
                key,
                identity,
            });
        }
        // The scenes that the run had under keys: those of this world kept, their content compared;
        // the others spawned, in place of this world's under the same key.
        for key in document.keys(&[SCENES]) {
            let key = &key;
            let mut text = |field: &str| match document
                .read(&[SCENES, key, field], "String", None)
                .map_err(DocumentError::Read)?
            {
                Some(Value::Text(text)) => Ok(text),
                _ => Ok(String::new()),
            };
            let (asset, name, content) = (text("scene")?, text("name")?, text("content")?);
            plan.listed.insert(key.clone(), asset.clone());
            let held = self.scenes.get(key).filter(|held| {
                held.scene.id().to_string() == asset
                    && self.find(&PersistentId::new([key.as_str()])).is_some()
            });
            match held {
                Some(held) => {
                    if content_text(held.content) != content {
                        loaded.scenes_changed.push(key.clone());
                    }
                }
                None => {
                    keys.extend(
                        key.parse::<PersistentId>()
                            .ok()
                            .filter(|id| id.parts().len() == 1),
                    );
                    plan.spawned.push(ToSpawn {
                        asset,
                        name,
                        content,
                        written: None,
                        key: key.clone(),
                        identity: SpawningAs::Key(key.clone()),
                    });
                }
            }
        }
        // The keys given to entities adopted, each with the identity that its entity had in its
        // scene.
        for key in document.keys(&[ADOPTED]) {
            let mut text = |field: &str| match document
                .read(&[ADOPTED, &key, field], "String", None)
                .map_err(DocumentError::Read)?
            {
                Some(Value::Text(text)) => Ok(Some(text)),
                _ => Ok(None),
            };
            let id = text("former")?.and_then(|text| renumbering.identity(&text));
            let scene = text("scene")?.and_then(|text| text.parse::<AssetId>().ok());
            let (Some(id), Some(scene)) = (id, scene) else {
                let problem = Problem::Invalid(
                    "an entity adopted names the identity that it had in its scene and the scene's \
                     asset ID"
                        .to_owned(),
                );
                return Err(DocumentError::Read(
                    document.error_at(&[ADOPTED, &key], problem),
                ));
            };
            if id.parts().len() < 2 || id.created_number().is_some() || valid_key(&key).is_err() {
                let problem = Problem::Invalid(
                    "a key adopted is a key, given to an entity of a scene below its root"
                        .to_owned(),
                );
                return Err(DocumentError::Read(
                    document.error_at(&[ADOPTED, &key], problem),
                ));
            }
            if plan.listed.contains_key(&key) {
                return Err(DocumentError::IdentityInUse(PersistentId::new([
                    key.as_str()
                ])));
            }
            plan.adopted.insert(key, Former { id, scene });
        }
        for key in document.keys(&[CREATED]) {
            let in_file = written(document, CREATED, &key)?.map(|id| Written {
                id,
                key: key.clone(),
                components: document.keys(&[CREATED, &key, "components"]),
            });
            let id = key.parse::<PersistentId>().ok();
            let creation = match id {
                // A key that the run gave an entity that it created, or what of its scene an entity
                // adopted took under its key.
                Some(id)
                    if id.parts()[0] != CREATED_KEY
                        && (id.parts().len() == 1 || plan.adopted.contains_key(&id.parts()[0])) =>
                {
                    if self.find(&id).is_some() || !keys.insert(id.clone()) {
                        return Err(DocumentError::IdentityInUse(id));
                    }
                    Creation::Key(id, document.keys(&[CREATED, &key, "components"]))
                }
                _ => Creation::Number(number(&key, id)?, in_file),
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
        read_removals_at(document, &[], &mut save)?;
        for key in document.keys(&[CREATED]) {
            if let Some(parent) = document
                .read(&[CREATED, &key, "parent"], "Option<String>", None)
                .map_err(DocumentError::Read)?
            {
                save.parents.push((key, option_text(&parent)));
            }
        }
        // What the world that made the save kept of the scenes that it unloaded.
        for key in document.keys(&[UNLOADED]) {
            for asset in document.keys(&[UNLOADED, &key]) {
                let place = [UNLOADED, key.as_str(), asset.as_str()];
                let Ok(id) = asset.parse::<AssetId>() else {
                    let problem = Problem::Invalid(format!("`{asset}` is not an asset ID"));
                    return Err(DocumentError::Read(document.error_at(&place, problem)));
                };
                let mut kept = Save::default();
                read_removals_at(document, &place, &mut kept)?;
                let table = Table::reading(renumbering);
                read_values_at(
                    document,
                    &place,
                    &table,
                    &mut kept,
                    &mut |document, entity, name, read| match read {
                        Ok(value) => value,
                        Err(error) if unknown(&error) => {
                            document.warn(error);
                            None
                        }
                        Err(error) => {
                            loaded.set_aside.push(SetAside {
                                entity: entity.to_owned(),
                                component: Some(name.to_owned()),
                                reason: error.to_string(),
                            });
                            None
                        }
                    },
                );
                let references = table.references.into_inner();
                let kept = unloaded(kept, references, renumbering, &mut loaded.set_aside);
                plan.unloaded.insert((key.clone(), id), kept);
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
        let mut size: usize = 0;
        // A placement that a scene file written from the world holds comes after the scenes under
        // keys, so that the file's level is spawned when its placement is looked for.
        let mut spawned = spawned;
        spawned.sort_by_key(|spawning| spawning.written.is_some());
        for ToSpawn {
            key,
            identity,
            asset,
            name,
            content,
            written,
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
            size = size.saturating_add(placed_size(&scene, &key));
            if entities > MAX_ENTITIES || size > MAX_SCENE_SIZE {
                aside(format!(
                    "the save spawns more than {MAX_ENTITIES} entities, or more than \
                     {MAX_SCENE_SIZE} bytes of scenes, while it is loaded"
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
                        // Another scene under the key, as when a run changed level, or one whose
                        // root no longer lives: the world's goes first, with what was under it.
                        let held = held.scene.clone();
                        self.despawn_scene(&key, &held, &mut save.gone);
                    }
                    self.spawn_scene_with(&key, &scene, &storages)
                        .inspect(|_| save.spawned_keys.push(key.clone()))
                }
                SpawningAs::Placement(number) => {
                    // Written into a scene file that the world spawned: the file's placement of
                    // the same prefab, once.
                    let in_file = written.as_ref().and_then(|id| {
                        let root = self.find(id)?;
                        let placement = self.nodes[root.index() as usize].placement.as_ref()?;
                        (placement.prefab.id() == asset_id && save.claimed.insert(root))
                            .then_some(root)
                    });
                    if let (Some(root), Some(id)) = (in_file, &written) {
                        save.aliases.insert(key.clone(), id.clone());
                        save.reset.push(root);
                        Ok(root)
                    } else {
                        self.place(&scene, number, &storages).inspect(|&root| {
                            numbered.push((number, root));
                            self.next_created = self.next_created.max(number + 1);
                            self.remember_written(root, written.clone());
                        })
                    }
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

    /// Destroys the entities of `scene`, spawned under `key`, and forgets the scene; adds their
    /// identities to `gone`. What is under them and not of the scene, another scene's entity, a
    /// placement or an entity created outside scenes, is taken out of them first, but what a save
    /// leaves out ([`Unsaved`]): the save removes it, or says where it went when the run left the
    /// level, as a hero that walked out of it.
    pub(crate) fn despawn_scene(
        &mut self,
        key: &str,
        scene: &ResolvedScene,
        gone: &mut HashSet<String>,
    ) {
        let mut entities = Vec::new();
        for id in record_ids(&PersistentId::new([key]), scene) {
            if let Some(entity) = self.find(&id) {
                gone.insert(id.to_string());
                entities.push(entity);
            }
        }
        let of_scene: HashSet<Entity> = entities.iter().copied().collect();
        for &entity in &entities {
            let kept: Vec<Entity> = self
                .children(entity)
                .filter(|child| !of_scene.contains(child) && !self.unsaved(*child))
                .collect();
            for child in kept {
                // Alive, so taking it out of its parent cannot fail.
                let _ = self.set_parent(child, None);
            }
        }
        for entity in entities {
            // Gone already when an ancestor of the scene went first.
            self.despawn(entity);
        }
        self.scenes.remove(key);
        self.forget_gone_under(&PersistentId::new([key]));
    }

    /// Spawns the entities that a save creates, under their numbers or their keys.
    fn create(&mut self, registry: &Registry, creations: Vec<Creation>, save: &mut Save) {
        let mut created = Vec::new();
        for creation in creations {
            match creation {
                Creation::Number(number, written) => {
                    // Written into a scene file that the world spawned: that entity, once, which
                    // keeps only the components that the save gives it, by their types' names.
                    if let Some(written) = &written
                        && let Some(entity) = self.find(&written.id)
                        && matches!(
                            self.nodes[entity.index() as usize].identity,
                            Identity::Scene(_)
                        )
                        && save.claimed.insert(entity)
                    {
                        save.aliases.insert(written.key.clone(), written.id.clone());
                        save.whole
                            .push((entity, canonical(registry, &written.components)));
                        continue;
                    }
                    let entity = self.spawn_bare();
                    self.nodes[entity.index() as usize].identity = Identity::Created(number);
                    self.remember_written(entity, written.map(|written| written.id));
                    // The count stays larger than every number given, whatever the save's says.
                    self.next_created = self.next_created.max(number + 1);
                    created.push((number, entity));
                }
                Creation::Key(id, components) => match self.find(&id) {
                    // An entity of a scene that the save adopted as it loaded: it keeps only the
                    // components that the save gives it.
                    Some(entity) if save.claimed.insert(entity) => {
                        let components = canonical(registry, &components);
                        save.whole.push((entity, components));
                    }
                    _ => {
                        let entity = self.spawn_bare();
                        self.identify(entity, id);
                    }
                },
            }
        }
        self.created.extend(created);
    }

    /// Remembers that a scene file written from the world gives `entity` the identity `written`, if
    /// it gives it one, so that the next save says it, and the next file written gives it the same
    /// ID.
    fn remember_written(&mut self, entity: Entity, written: Option<PersistentId>) {
        let node = &mut self.nodes[entity.index() as usize];
        node.written_id = written
            .as_ref()
            .and_then(|id| id.parts().last())
            .map(|id| id.clone().into_boxed_str());
        node.written_as = written;
    }

    /// Reads the components' values that the save `document` holds, into `save`, the entities they
    /// belong to and refer to spawned already: a value of a type or with a variant that this program
    /// does not know is left out with a warning, as a field that its type does not have is; those
    /// that cannot be read otherwise, and the references to entities that the world does not have,
    /// are set aside.
    fn read_values(&self, document: &mut Document<'_>, save: &mut Save, loaded: &mut Loaded) {
        let names = Names {
            aliases: save.aliases.clone(),
            ..Names::new(self, save.numbers)
        };
        let mut take = |document: &mut Document<'_>,
                        key: &str,
                        name: &str,
                        read: Result<Option<Value>, ReadError>| {
            let aside = |loaded: &mut Loaded, reason: String| {
                loaded.set_aside.push(SetAside {
                    entity: key.to_owned(),
                    component: Some(name.to_owned()),
                    reason,
                });
            };
            let value = match read {
                Ok(value) => value,
                Err(error) if unknown(&error) => {
                    document.warn(error);
                    None
                }
                Err(error) => {
                    aside(loaded, error.to_string());
                    None
                }
            };
            for unknown in names.unknown.take() {
                aside(
                    loaded,
                    format!(
                        "it refers to `{unknown}`, to no entity now: the world does not have it"
                    ),
                );
            }
            value
        };
        for key in document.keys(&[CREATED]) {
            for name in document.keys(&[CREATED, &key, "components"]) {
                let read =
                    document.read(&[CREATED, &key, "components", &name], &name, Some(&names));
                if let Some(value) = take(document, &key, &name, read) {
                    save.added.push((key.clone(), name, value));
                }
            }
        }
        read_values_at(document, &[], &names, save, &mut take);
        loaded.warnings.extend(document.warnings());
    }

    /// Applies what a save holds, setting aside what cannot be.
    pub(crate) fn apply_save(&mut self, save: Save, loaded: &mut Loaded) {
        let mut aside = |entity: &str, component: Option<&str>, reason: &str| {
            loaded.set_aside.push(SetAside {
                entity: entity.to_owned(),
                component: component.map(str::to_owned),
                reason: reason.to_owned(),
            });
        };
        let numbers = save.numbers;
        // The entity that the save names `key`: the one that a scene file holds for what the save
        // creates and the world has, else the one of its identity.
        let named = |world: &World, key: &str| match aliased(&save.aliases, key) {
            Some(id) => world.find(&id),
            None => world.entity_named(numbers, key),
        };
        // Why a difference is set aside whose entity, `key`, the world does not have.
        let gone = |_: &str| "the world has no entity of this identity";
        for &root in &save.reset {
            self.reset_to_prefab(root);
        }
        // Found before any is despawned: removing an entity removes its descendants, which the
        // save names too.
        let mut removed = Vec::new();
        for key in &save.removed {
            match named(self, key) {
                Some(entity) => removed.push(entity),
                // Destroyed already, with a scene that another under its key replaced, or removed
                // from a placement before a scene file was written from it.
                None if save.gone.contains(key) || aliased(&save.aliases, key).is_some() => {}
                None => aside(key, None, gone(key)),
            }
        }
        // Every entity given another parent is taken out first, then put where the save says: one
        // at a time, a child could not take its parent's place, as its parent would be its
        // descendant still.
        let mut moved = Vec::new();
        for (key, parent) in &save.parents {
            let Some(entity) = named(self, key) else {
                aside(key, None, gone(key));
                continue;
            };
            let parent = match parent {
                Some(parent) => match named(self, parent) {
                    Some(parent) => Some(parent),
                    // Out of its parent all the same, so that it outlives it if the save removes it.
                    None => {
                        aside(
                            key,
                            None,
                            &format!(
                                "it has no parent now, where it had `{parent}`: {}",
                                gone(parent)
                            ),
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
            let Some(entity) = named(self, key) else {
                // Removed from a placement before a scene file was written from it.
                if aliased(&save.aliases, key).is_none() {
                    for name in names {
                        aside(key, Some(name), gone(key));
                    }
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
            let Some(entity) = named(self, &key) else {
                aside(&key, Some(&name), gone(&key));
                continue;
            };
            if let Err(error) = self.insert_component_value(entity, &name, value) {
                aside(&key, Some(&name), &error.to_string());
            }
        }
        for (key, name, part) in save.changed {
            let Some(entity) = named(self, &key) else {
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
        // What the save creates whole, the file's entity holds as the save gives it: no more.
        for (entity, components) in &save.whole {
            let held: Vec<String> = self
                .component_names(*entity)
                .into_iter()
                .map(str::to_owned)
                .collect();
            for name in held.iter().filter(|name| !components.contains(name)) {
                self.remove_component_value(*entity, name);
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

    /// The identity that the entity of `id`, created outside scenes, takes in the scene file written
    /// from the world that holds it ([`World::write_scene`]), as text, if one does.
    fn written_as(&self, id: &PersistentId) -> Option<String> {
        let entity = self.find(id)?;
        let written = self.nodes[entity.index() as usize].written_as.as_ref()?;
        Some(written.to_string())
    }

    /// Whether `entity` marks its component of the type named `name` driven ([`Driven`]).
    pub(crate) fn drives(&self, entity: Entity, name: &str) -> bool {
        self.get::<Driven>(entity)
            .is_some_and(|driven| driven.holds(name))
    }
}

/// The identity in the world of the entity that a save names `name`, if `aliases` takes it there
/// ([`Save::aliases`]): the identity that a scene file gave what the save created or placed as
/// `created/<number>`, followed by the rest of `name`, for an entity under such a placement.
fn aliased(aliases: &HashMap<String, PersistentId>, name: &str) -> Option<PersistentId> {
    if aliases.is_empty() {
        return None;
    }
    let id = name.parse::<PersistentId>().ok()?;
    let parts = id.parts();
    let alias = aliases.get(&parts.get(..2)?.join("/"))?;
    let rest = parts[2..].iter().map(String::as_str);
    Some(PersistentId::new(
        alias.parts().iter().map(String::as_str).chain(rest),
    ))
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
pub(crate) struct Save {
    next_created: u64,
    /// How it names the entities created outside scenes.
    pub(crate) numbers: Numbers,
    pub(crate) removed: Vec<String>,
    pub(crate) removed_components: Vec<(String, Vec<String>)>,
    /// Each entity given another parent, the created ones included, and its parent, if it has one.
    pub(crate) parents: Vec<(String, Option<String>)>,
    /// The components added, of the entities the world had and of those it created: the entity,
    /// the component and its value.
    pub(crate) added: Vec<(String, String, Value)>,
    /// What changed of the components: the entity, the component and the part.
    pub(crate) changed: Vec<(String, String, Value)>,
    /// The identities of the entities that loading destroyed when it spawned a scene under a key in
    /// place of another: the save removes them, and nothing is reported of them.
    gone: HashSet<String>,
    /// What the save creates or places and that a scene file written from the world holds, which
    /// the world has under the identity that the file gave it: by the save's name of it,
    /// `created/<number>`, that identity, which the save's names of it and of what is under it reach
    /// in place of a new entity.
    aliases: HashMap<String, PersistentId>,
    /// The entities that the save creates whole and that the world had so: each with the names of
    /// the components that the save gives it, which it keeps alone.
    whole: Vec<(Entity, Vec<String>)>,
    /// The entities of the world that what the save creates or places has become, each once.
    claimed: HashSet<Entity>,
    /// The keys of the scenes that the save spawned, which the game prepares before they are
    /// settled.
    spawned_keys: Vec<String>,
    /// The roots of the placements that the save describes from their prefabs and that the world
    /// has from a scene file, overrides included: set back to their prefabs before the save applies.
    reset: Vec<Entity>,
    /// What the world that made the save kept of the scenes that it spawned again and had not
    /// settled, by their keys and assets: applied once the save is.
    pub(crate) pending: Vec<(String, AssetId, Unloaded)>,
}

/// What a save holds that refuses it if it is wrong, read before the world changes.
#[derive(Default)]
struct Plan {
    /// What the save spawns: the scenes that it names and that the world lacks, or holds others in
    /// place of, and the placements made while the world that made it ran.
    spawned: Vec<ToSpawn>,
    /// The entities that the save creates: their numbers in this world, or their keys.
    created: Vec<Creation>,
    /// The scenes that the save names, by their keys: their asset IDs, as the save writes them.
    listed: BTreeMap<String, String>,
    /// What the world that made the save kept of the scenes that it unloaded.
    unloaded: BTreeMap<(String, AssetId), Unloaded>,
    /// The identities that the entities that it adopted had, and their scenes, by the keys that it
    /// gave them.
    adopted: BTreeMap<String, Former>,
}

/// An entity that a save creates: under a number of the world's count, with what the save says of
/// it if a scene file written from the world holds it, or a key ([`World::spawn_keyed`]).
enum Creation {
    Number(u64, Option<Written>),
    /// Under a key, with the names of the components that the save gives it.
    Key(PersistentId, Vec<String>),
}

/// An entity that a save creates and that a scene file written from the world holds
/// ([`World::write_scene`]).
struct Written {
    /// The identity that the file gives it.
    id: PersistentId,
    /// The save's name of it, `created/<number>`.
    key: String,
    /// The names of the components that the save gives it.
    components: Vec<String>,
}

/// How a save names the entities that a world created outside scenes, and what this world names
/// them as it loads the save ([`World::load`]): what the run created, from the count that the start
/// of its run reached up, takes numbers from this world's count up, whatever the game's start
/// creates now; what that start created keeps its numbers.
#[derive(Clone, Copy)]
pub(crate) struct Numbers {
    /// The count that the save's start reached.
    from: u64,
    /// This world's count before the save is loaded.
    to: u64,
}

impl Default for Numbers {
    fn default() -> Self {
        Self::SAME
    }
}

impl Numbers {
    /// The identities as the save gives them: for a world's documents other than saves loaded.
    pub(crate) const SAME: Self = Self {
        from: u64::MAX,
        to: u64::MAX,
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
    /// names no entity. What the save's start created keeps its number: saving refuses a start
    /// that created what a save would hold, so that only what saves leave out has one.
    pub(crate) fn identity(self, text: &str) -> Option<PersistentId> {
        let id: PersistentId = text.parse().ok()?;
        let Some(number) = created_root(&id) else {
            return Some(id);
        };
        if number < self.from {
            return Some(id);
        }
        let number = self.number(number)?.to_string();
        let parts = [CREATED_KEY, number.as_str()].into_iter();
        Some(PersistentId::new(
            parts.chain(id.parts()[2..].iter().map(String::as_str)),
        ))
    }
}

/// The number of the entity created outside scenes that `id` names, or names a placement's entity
/// under: `created/7`, `created/7/light`.
pub(crate) fn created_root(id: &PersistentId) -> Option<u64> {
    let parts = id.parts();
    if parts.len() < 2 {
        return None;
    }
    PersistentId::new(parts[..2].iter().map(String::as_str)).created_number()
}

/// A scene that the world that made a save spawned while it ran, as the save gives it.
struct ToSpawn {
    /// The persistent identity of its root, as the save writes it.
    key: String,
    identity: SpawningAs,
    /// Its asset ID, its name and its fingerprint, as the save writes them.
    asset: String,
    name: String,
    content: String,
    /// For a placement, the identity that a scene file written from the world gives its root, if
    /// one holds it.
    written: Option<PersistentId>,
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

/// Whether `error` says that a value names a type or a variant that this program does not know, as
/// a later version of a game writes them: what a lenient reading leaves out with a warning.
fn unknown(error: &ReadError) -> bool {
    matches!(
        error.problem(),
        ReadProblem::Value(Problem::NotRegistered(_) | Problem::UnknownVariant { .. })
    )
}

/// A scene's fingerprint as a save writes it: sixteen hexadecimal digits.
fn content_text(fingerprint: u64) -> String {
    format!("{fingerprint:016x}")
}

/// Puts in `save`, at `place`, the top of the save or the table of a scene that the world keeps
/// unloaded, what changed of entities: those `removed`, in the order of their identities, the
/// `parents` changed, and the components removed, added whole and changed in parts.
fn put_differences(
    save: &mut Composer<'_>,
    place: &[&str],
    removed: &[PersistentId],
    parents: &BTreeMap<PersistentId, Option<PersistentId>>,
    differences: &Differences,
) -> Result<(), DocumentError> {
    let mut put = |rest: &[&str], ty: &str, value: &Value, part: bool| {
        let at: Vec<&str> = place.iter().chain(rest).copied().collect();
        match part {
            false => save.put(&at, ty, value),
            true => save.put_part(&at, ty, value),
        }
        .map_err(DocumentError::Write)
    };
    let mut removed: Vec<String> = removed.iter().map(ToString::to_string).collect();
    removed.sort_unstable();
    if !removed.is_empty() {
        let removed = Value::List(removed.into_iter().map(Value::Text).collect());
        put(&[REMOVED, "entities"], "Vec<String>", &removed, false)?;
    }
    for (id, gone) in &differences.removed {
        let gone = Value::List(gone.iter().cloned().map(Value::Text).collect());
        put(
            &[REMOVED, "components", &id.to_string()],
            "Vec<String>",
            &gone,
            false,
        )?;
    }
    for (id, parent) in parents {
        let parent = text_option(parent.clone());
        put(
            &[PARENTS, &id.to_string()],
            "Option<String>",
            &parent,
            false,
        )?;
    }
    for ((id, name), value) in &differences.added {
        put(&[ADDED, &id.to_string(), name], name, value, false)?;
    }
    for ((id, name), part) in &differences.changed {
        put(&[CHANGED, &id.to_string(), name], name, part, true)?;
    }
    Ok(())
}

/// Reads the entities and the components removed, and the entities given another parent, that a
/// save holds at `place`, the top of the save or the table of a scene that it keeps unloaded, into
/// `save`.
fn read_removals_at(
    document: &mut Document<'_>,
    place: &[&str],
    save: &mut Save,
) -> Result<(), DocumentError> {
    let at = |rest: &[&str]| -> Vec<String> {
        place
            .iter()
            .chain(rest)
            .map(|&key| key.to_owned())
            .collect()
    };
    let entities = at(&[REMOVED, "entities"]);
    if let Some(Value::List(entities)) = document
        .read(&place_of(&entities), "Vec<String>", None)
        .map_err(DocumentError::Read)?
    {
        save.removed = texts(entities);
    }
    for key in document.keys(&place_of(&at(&[REMOVED, "components"]))) {
        let names = at(&[REMOVED, "components", key.as_str()]);
        if let Some(Value::List(names)) = document
            .read(&place_of(&names), "Vec<String>", None)
            .map_err(DocumentError::Read)?
        {
            save.removed_components.push((key, texts(names)));
        }
    }
    for key in document.keys(&place_of(&at(&[PARENTS]))) {
        let parent = at(&[PARENTS, key.as_str()]);
        if let Some(parent) = document
            .read(&place_of(&parent), "Option<String>", None)
            .map_err(DocumentError::Read)?
        {
            save.parents.push((key, option_text(&parent)));
        }
    }
    Ok(())
}

/// The keys of a place held as text, as a document reads them.
fn place_of(keys: &[String]) -> Vec<&str> {
    keys.iter().map(String::as_str).collect()
}

/// What becomes of a value that a save holds, read for an entity and a component: the value, or
/// none, set aside or warned of.
type Take<'a> = dyn FnMut(&mut Document<'_>, &str, &str, Result<Option<Value>, ReadError>) -> Option<Value>
    + 'a;

/// Reads the components added, whole, and changed, in parts, that a save holds at `place`, the top
/// of the save or the table of a scene that it keeps unloaded, into `save`, naming entities
/// through `names`; `take` says what becomes of each value read.
fn read_values_at(
    document: &mut Document<'_>,
    place: &[&str],
    names: &dyn References,
    save: &mut Save,
    take: &mut Take<'_>,
) {
    for (section, part) in [(ADDED, false), (CHANGED, true)] {
        let table: Vec<String> = place
            .iter()
            .chain([&section])
            .map(|&key| key.to_owned())
            .collect();
        for key in document.keys(&place_of(&table)) {
            let mut entity = table.clone();
            entity.push(key.clone());
            for name in document.keys(&place_of(&entity)) {
                let mut at = entity.clone();
                at.push(name.clone());
                let at = place_of(&at);
                let read = match part {
                    false => document.read(&at, &name, Some(names)),
                    true => document.read_part(&at, &name, Some(names)),
                };
                if let Some(value) = take(document, &key, &name, read) {
                    match part {
                        false => save.added.push((key.clone(), name, value)),
                        true => save.changed.push((key.clone(), name, value)),
                    }
                }
            }
        }
    }
}

/// What a world keeps of a scene unloaded, read from a save as `kept`, its values naming entities
/// by the indices of `references`, its names of entities created outside scenes as `numbers`
/// gives them in this world; an entity named by what is not a persistent identity is set aside in
/// `set_aside`.
fn unloaded(
    kept: Save,
    references: Vec<PersistentId>,
    numbers: Numbers,
    set_aside: &mut Vec<SetAside>,
) -> Unloaded {
    let mut id = |text: &str, component: Option<&str>| {
        let id = numbers.identity(text);
        if id.is_none() {
            set_aside.push(SetAside {
                entity: text.to_owned(),
                component: component.map(str::to_owned),
                reason: "it is not a persistent identity".to_owned(),
            });
        }
        id
    };
    let mut unloaded = Unloaded {
        references,
        ..Unloaded::default()
    };
    unloaded.removed = kept
        .removed
        .iter()
        .filter_map(|text| id(text, None))
        .collect();
    for (text, names) in kept.removed_components {
        if let Some(entity) = id(&text, None) {
            unloaded.differences.removed.insert(entity, names);
        }
    }
    for (text, parent) in kept.parents {
        let parent = parent.and_then(|parent| id(&parent, None));
        if let Some(entity) = id(&text, None) {
            unloaded.parents.insert(entity, parent);
        }
    }
    for (text, name, value) in kept.added {
        if let Some(entity) = id(&text, Some(&name)) {
            unloaded.differences.added.insert((entity, name), value);
        }
    }
    for (text, name, part) in kept.changed {
        if let Some(entity) = id(&text, Some(&name)) {
            unloaded.differences.changed.insert((entity, name), part);
        }
    }
    unloaded
}

/// The names of the components `names`, as `registry` names their types now, rather than by their
/// aliases.
fn canonical(registry: &Registry, names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|name| {
            registry
                .get(name)
                .map_or_else(|| name.clone(), |held| held.info().name.clone())
        })
        .collect()
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

/// A scene that a world spawned under a key, or a placement, that a save names, so that loading
/// spawns it.
struct Spawned<'w> {
    /// The persistent identity of its root: the key, or `created/<number>`.
    root: PersistentId,
    scene: &'w ResolvedScene,
    /// The key it was spawned under; none for a placement.
    key: Option<&'w str>,
}

/// What changed of the entities that a world settled, since it settled them, by their identities
/// and their components' names, in their order: what a save writes of them.
#[derive(Clone, Debug, Default)]
pub(crate) struct Differences {
    /// The components removed.
    pub(crate) removed: BTreeMap<PersistentId, Vec<String>>,
    /// The components added, whole.
    pub(crate) added: BTreeMap<(PersistentId, String), Value>,
    /// What changed of the others, in parts.
    pub(crate) changed: BTreeMap<(PersistentId, String), Value>,
}

/// What a scene spawned while a world ran was spawned as.
enum SpawningAs {
    /// A scene under this key.
    Key(String),
    /// A placement made while the world ran, under this number.
    Placement(u64),
}

impl World {
    /// The scenes spawned under keys, the last under each, and the placements, that a save names,
    /// the scenes first, each in the order of its root's identity. It names a scene under a key,
    /// unless entities of it live of which it leaves out every one ([`Unsaved`]): one of which no
    /// entity lives is one that the run destroyed, whose entities it removes. It names a placement
    /// of which it holds an entity. Its root, which the save holds, is enough: the records are
    /// looked through only when it does not.
    fn spawned_in_save(&self) -> Vec<Spawned<'_>> {
        let keys = self.scenes.iter().map(|(key, spawned)| {
            (
                PersistentId::new([key.as_str()]),
                &spawned.scene,
                Some(key.as_str()),
            )
        });
        let placements = self
            .placements
            .iter()
            .map(|(&number, scene)| (PersistentId::created(number), scene, None));
        keys.chain(placements)
            .filter(|(root, scene, key)| {
                if self.find(root).is_some_and(|root| !self.unsaved(root)) {
                    return true;
                }
                let alive: Vec<Entity> = record_ids(root, scene)
                    .iter()
                    .filter_map(|id| self.find(id))
                    .collect();
                alive.iter().any(|&entity| !self.unsaved(entity))
                    || (key.is_some() && alive.is_empty())
            })
            .map(|(root, scene, key)| Spawned { root, scene, key })
            .collect()
    }

    /// What changed since the world settled them of the entities that a save holds, as `saved`
    /// says: each component changed, added or removed is compared with the base that the world
    /// kept of it at its first change, but one that its entity marks driven at its base and now.
    pub(crate) fn changes_since_settled(&self, saved: &dyn Fn(Entity) -> bool) -> Differences {
        let names = Names::new(self, Numbers::SAME);
        let driven_id = self.component_ids.get(&TypeId::of::<Driven>()).copied();
        // The mark that an entity holds, by its slot, at its base or now.
        let driven = |slot: u32, at_base: bool| -> Option<Driven> {
            let storage = &self.storages[driven_id?];
            let value = match (at_base, storage.kept(slot)) {
                (true, Some(kept)) => kept?,
                _ => storage.value(slot)?,
            };
            Driven::from_value(&value).ok()
        };
        let mut differences = Differences::default();
        for (id, storage) in self.storages.iter().enumerate() {
            let Some(name) = self
                .registry
                .of_type(self.component_types[id])
                .map(|registration| registration.info().name.clone())
            else {
                continue;
            };
            for slot in storage.kept_slots() {
                let entity = self.entity_at(slot);
                let Some(identity) = self.persistent_id(entity) else {
                    continue;
                };
                if !saved(entity) {
                    continue;
                }
                let Some(base) = storage.kept(slot) else {
                    continue;
                };
                let now = storage.value(slot);
                let marked = |at_base| driven(slot, at_base).is_some_and(|d| d.holds(&name));
                match (base, now) {
                    (Some(_), Some(_)) if marked(true) && marked(false) => {}
                    (Some(base), Some(now)) => {
                        let (base, now) = (to_no_entity(base, &names), to_no_entity(now, &names));
                        if let Ok(Some(part)) = self.registry.difference(&name, &base, &now, false)
                        {
                            differences.changed.insert((identity, name.clone()), part);
                        }
                    }
                    (Some(_), None) => differences
                        .removed
                        .entry(identity)
                        .or_default()
                        .push(name.clone()),
                    (None, Some(now)) => {
                        differences.added.insert((identity, name.clone()), now);
                    }
                    (None, None) => {}
                }
            }
        }
        for names in differences.removed.values_mut() {
            names.sort_unstable();
        }
        differences
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
        world.settle();
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("loaded");
        assert!(report.set_aside.is_empty(), "{:?}", report.set_aside);
        assert!(report.scenes_changed.is_empty() && report.warnings.is_empty());
        assert_eq!(played.first_difference(&loaded), None, "{save}");
        assert_eq!(
            loaded.save(&BTreeMap::new()).expect("saved again"),
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
            r#"layout = 5
next_created = 3
start = { created = 1 }

[scenes]
yard = { scene = "00000000-0000-0000-0000-000000000001", name = "yard", content = "SCENE" }

[removed]
entities = ["yard/tree"]

[removed.components]
"yard/post" = ["Meshes"]

[parents]
"yard/post" = ["yard/well"]

[added."yard/gate"]
Hidden = {}

[changed."yard/well".Transform]
translation = { x = 6.5 }

[created."created/1"]
parent = ["yard/well"]

[created."created/1".components]
Name = "torch"

[created."created/1".components.Transform]
translation = { x = 0.5, y = 1.0, z = 0.0 }
rotation = { x = 0.0, y = 0.0, z = 0.0, w = 1.0 }
scale = 1.0
stretch = { x = 1.0, y = 1.0, z = 1.0 }
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
        let save = played().save(&BTreeMap::new()).expect("saved");
        let mut world = World::new();
        world
            .spawn_scene("yard", &resolved(&scene_changed()))
            .expect("spawned");
        let report = world
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("loaded");
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
        let save = played().save(&BTreeMap::new()).expect("saved");
        for (text, problem) in [
            ("next_created = [", "not TOML"),
            ("\"$format\" = 2\n", "newer"),
            ("\"$versions\" = { Transform = 9 }\n", "newer"),
            ("layout = 6\n", "a save of layout 6"),
            (
                "layout = 5\n[spawned.\"created/x\"]\nscene = \"\"\n",
                "not the identity",
            ),
            (
                "layout = 5\n[created.\"yard/gate\"]\nparent = []\n",
                "not the identity",
            ),
            // Its entity created, then taken back when the count cannot be read.
            (
                "layout = 5\nnext_created = true\n[created.\"created/7\"]\n",
                "cannot be read",
            ),
        ] {
            let mut world = start();
            let error = world
                .load(text, &mut SceneLibrary::new(), &mut |_, _| {})
                .expect_err(text);
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
                "layout = 5\nnext_created = 2\n[created.\"created/5\"]\n",
                &mut SceneLibrary::new(),
                &mut |_, _| {},
            )
            .expect("loaded");
        let next = world.spawn();
        assert_eq!(world.persistent_id(next), Some(PersistentId::created(6)));
        // A second load, into a world whose start created more: what the run created takes the
        // numbers that follow the world's.
        let mut world = start();
        world
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("loaded");
        world
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("twice");
        assert_eq!(
            world.get::<Name>(entity(&world, "created/3")),
            Some(&Name("torch".to_owned()))
        );
        // A save edited so that it does not say what its start created gives the numbers that it
        // holds: they are in use.
        let mut world = start();
        world
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("loaded");
        let edited = save.replace("start = { created = 1 }\n", "");
        assert_ne!(edited, save);
        let error = world
            .load(&edited, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect_err("in use");
        assert!(matches!(error, DocumentError::IdentityInUse(id) if id.to_string() == "created/1"));
        // Truncated anywhere, a save gives an error or loads what it holds, and never panics.
        for end in (0..save.len()).filter(|&end| save.is_char_boundary(end)) {
            let _ = start().load(&save[..end], &mut SceneLibrary::new(), &mut |_, _| {});
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
    fn a_save_of_an_earlier_layout_is_refused_saying_so() {
        // As 0.3 wrote it, without a layout; as 0.4, P1 and M6a.7a wrote them, of layouts 2 to 4:
        // differences from a start world made again, which this program no longer reads.
        let old = "next_created = 2\n\n[scenes]\nyard = { name = \"yard\", content = \"0\" }\n\n\
                   [changed.\"yard/well\".Transform]\nrotation = { y = 0.6, w = 0.8 }\n";
        for layout in [None, Some(2), Some(3), Some(4)] {
            let text = match layout {
                Some(layout) => format!("layout = {layout}\n{old}"),
                None => old.to_owned(),
            };
            let mut world = start();
            let refused = world
                .load(&text, &mut SceneLibrary::new(), &mut |_, _| {})
                .expect_err("refused");
            let message = slime_error_chain(&refused);
            assert!(
                message.contains("an earlier version of the engine wrote"),
                "{layout:?}: {message}"
            );
            let well = entity(&world, "yard/well");
            assert_eq!(
                world.get::<Transform>(well).expect("held").rotation,
                glam::Quat::IDENTITY,
                "unchanged"
            );
        }
    }

    /// `error`'s message, followed by those of its causes.
    fn slime_error_chain(error: &dyn std::error::Error) -> String {
        let mut said = error.to_string();
        let mut cause = error.source();
        while let Some(error) = cause {
            said.push_str(": ");
            said.push_str(&error.to_string());
            cause = error.source();
        }
        said
    }

    #[test]
    fn a_save_refused_leaves_the_world_as_it_was_whatever_it_spawns_first() {
        let mut scenes = SceneLibrary::new();
        scenes.insert_cooked(AssetId::from_u128(1), scene());
        let yard = "{ scene = \"00000000-0000-0000-0000-000000000001\" }";
        for (text, refused) in [
            (
                format!(
                    "layout = 5\n[scenes]\ncave = {yard}\n[spawned]\n\"created/01\" = {yard}\n"
                ),
                "not the identity",
            ),
            (
                format!("layout = 5\n[scenes]\ncave = {yard}\nbad = {{ scene = 5 }}\n"),
                "cannot be read",
            ),
            (
                format!("layout = 5\n[spawned]\n\"created/5\" = {yard}\n\"created/1\" = {yard}\n"),
                "has already",
            ),
            (
                format!("layout = 5\n[scenes]\ncave = {yard}\n\n[created.\"created/1\"]\n"),
                "has already",
            ),
        ] {
            let made = || {
                let mut world = start();
                world.spawn();
                world.settle();
                world
            };
            let mut world = made();
            let error = world
                .load(&text, &mut scenes, &mut |_, _| {})
                .expect_err(&text);
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
        let save = "layout = 5\n[scenes]\ncave = { scene = \"00000000-0000-0000-0000-000000000001\" }\n\n\
                    [spawned]\n\"created/4\" = { scene = \"00000000-0000-0000-0000-000000000001\" }\n\n\
                    [created.\"yard/gate\"]\nparent = []\n";
        let mut world = start();
        assert!(matches!(
            world.load(save, &mut scenes, &mut |_, _| {}),
            Err(DocumentError::NotCreated(_))
        ));
        assert_eq!(world.first_difference(&start()), None);
        assert!(!world.scenes.contains_key("cave") && world.placements.is_empty());
        let next = world.spawn();
        assert_eq!(world.persistent_id(next), Some(PersistentId::created(1)));
        // Without what failed, it spawns both.
        let save = save.replace("[created.\"yard/gate\"]\nparent = []\n", "");
        let report = world
            .load(&save, &mut scenes, &mut |_, _| {})
            .expect("loaded");
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
            world.settle();
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        assert!(
            save.contains("[added.\"yard/gate\"]\n\"test::Rope\" = { to = \"yard/well\" }\n"),
            "{save}"
        );
        assert!(!save.contains("swing"), "{save}");
        let mut loaded = start();
        loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        assert!(save.contains("\"test::Rope\" = { to = \"\" }"), "{save}");
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
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
                "layout = 5\n[added.\"yard/gate\"]\n\"test::Rope\" = { to = \"yard/nothing\" }\n",
                &mut SceneLibrary::new(),
                &mut |_, _| {},
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        assert!(save.contains("[scenes]\nyard = "), "{save}");
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
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
            world.settle();
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        assert!(!save.contains("shake") && !save.contains("seen"), "{save}");
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
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
            "layout = 5\n[created.\"created/18446744073709551615\"]\n",
            "layout = 5\n[created.\"created/9223372036854775808\"]\n",
            "layout = 5\n[created.\"created/01\"]\n",
            "layout = 5\n[created.\"created/+1\"]\n",
        ] {
            let error = start()
                .load(text, &mut SceneLibrary::new(), &mut |_, _| {})
                .expect_err(text);
            assert!(
                matches!(error, DocumentError::NotCreated(_)),
                "{text}: {error}"
            );
        }
        let largest = i64::MAX as u64;
        for (text, next) in [
            (
                "layout = 5\nnext_created = \"18446744073709551615\"\n".to_owned(),
                largest,
            ),
            (
                format!("layout = 5\n[created.\"created/{largest}\"]\n"),
                largest + 1,
            ),
        ] {
            let mut world = start();
            world
                .load(&text, &mut SceneLibrary::new(), &mut |_, _| {})
                .expect(&text);
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
            let mut world = World::new();
            world
                .spawn_scene("yard", &resolved(&scene()))
                .expect("spawned");
            let tree = entity(&world, "yard/tree");
            world.insert(tree, Unsaved);
            let ember = world.spawn_keyed("ember").expect("a free key");
            world.set_parent(ember, Some(tree)).expect("a parent");
            world.settle();
            world
        };
        let mut played = start();
        let tree = entity(&played, "yard/tree");
        played.remove::<Unsaved>(tree);
        let ember = entity(&played, "ember");
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        assert!(!save.contains("[created"), "{save}");
        assert!(!save.contains("yard/well"), "{save}");
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
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
            .load("layout = 5\n[removed]\nentities = [\"yard/gate\"]\n\n[parents]\n\"yard/post\" = [\"yard/shed\"]\n", &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("loaded");
        let post = entity(&loaded, "yard/post");
        assert_eq!(loaded.parent(post), None);
        assert_eq!(report.set_aside.len(), 1, "{:?}", report.set_aside);
        assert!(report.set_aside[0].reason.contains("`yard/shed`"));
    }

    /// A component of a version that no upgrade brings forward: its upgrade panics, as a game's may
    /// on a value that it did not expect.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Brittle", version = 2, upgrade = shatter)]
    struct Brittle {
        hardness: f32,
    }
    impl crate::Component for Brittle {}

    fn shatter(_: u32, _: Value, _: bool) -> Result<Value, String> {
        panic!("shattered");
    }

    /// A component whose check panics on a weight that it did not expect.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Fragile", check = Fragile::check)]
    struct Fragile {
        weight: f32,
    }
    impl crate::Component for Fragile {}

    impl Fragile {
        fn check(&self) -> Result<(), String> {
            assert!(self.weight < 100.0, "too heavy");
            Ok(())
        }
    }

    #[test]
    fn a_panic_in_a_type_s_upgrade_or_check_sets_its_value_aside() {
        let mut world = start();
        world.register::<Brittle>();
        world.register::<Fragile>();
        let text = "layout = 5\n\"$versions\" = { \"test::Brittle\" = 1 }\n\n\
                    [removed]\nentities = [\"yard/tree\"]\n\n\
                    [added.\"yard/gate\".\"test::Brittle\"]\nhardness = 2.0\n\n\
                    [added.\"yard/well\".\"test::Fragile\"]\nweight = 500.0\n";
        let report = world
            .load(text, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("loaded, the values that panicked set aside");
        let aside: Vec<(&str, Option<&str>)> = report
            .set_aside
            .iter()
            .map(|aside| (aside.entity.as_str(), aside.component.as_deref()))
            .collect();
        assert_eq!(
            aside,
            [
                ("yard/gate", Some("test::Brittle")),
                ("yard/well", Some("test::Fragile"))
            ]
        );
        assert!(
            report
                .set_aside
                .iter()
                .all(|aside| aside.reason.contains("panicked")),
            "{:?}",
            report.set_aside
        );
        // The rest is applied, and the world keeps its registry.
        assert!(world.find(&"yard/tree".parse().unwrap()).is_none());
        let gate = entity(&world, "yard/gate");
        assert!(world.get::<Brittle>(gate).is_none());
        assert!(world.registry().get("test::Brittle").is_some());
        assert!(world.registry().get("Transform").is_some());
    }

    #[test]
    fn what_a_save_says_of_itself_comes_first_and_is_read_without_loading_it() {
        let about = BTreeMap::from([
            ("layout".to_owned(), "8fe31823c2a1615f".to_owned()),
            ("made with".to_owned(), "version 0.3".to_owned()),
        ]);
        let save = played().save(&about).expect("saved");
        assert!(
            save.starts_with(
                "about = { layout = \"8fe31823c2a1615f\", \"made with\" = \"version 0.3\" }\n"
            ),
            "{save}"
        );
        assert_eq!(World::about(&save).expect("a save"), about);
        let mut loaded = start();
        let report = loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("loaded");
        assert!(report.set_aside.is_empty() && report.warnings.is_empty());
        assert_eq!(played().first_difference(&loaded), None);
        // A save without it says nothing; one that is not a save, or says something else than
        // text, is refused.
        let quiet = played().save(&BTreeMap::new()).expect("saved");
        assert!(!quiet.contains("about"), "{quiet}");
        assert!(World::about(&quiet).expect("a save").is_empty());
        for text in [
            "about = [",
            "layout = 5\nabout = { layout = 3 }\n",
            "\"$format\" = 2\n",
        ] {
            assert!(
                matches!(World::about(text), Err(DocumentError::Read(_))),
                "{text}"
            );
        }
        // Nor is a save of another layout, which loading refuses.
        assert!(matches!(
            World::about("about = { made = \"long ago\" }\n"),
            Err(DocumentError::Layout { found: 1, read: 5 })
        ));
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
        let save = played.save(&BTreeMap::new()).expect("saved");
        eprintln!("save {} bytes: {:?}", save.len(), started.elapsed());
        let mut loaded = start();
        let started = std::time::Instant::now();
        loaded
            .load(&save, &mut SceneLibrary::new(), &mut |_, _| {})
            .expect("loaded");
        eprintln!("load: {:?}", started.elapsed());
        assert_eq!(played.first_difference(&loaded), None);
    }
}
