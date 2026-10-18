//! Scene files read and resolved: their layout, the prefabs that they place, and the overrides that
//! they write for them (`docs/adr/0089-scenes-and-prefabs.md`).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

use slime_assets::AssetId;
use slime_reflect::{Problem, Reflect, Registry, Value};
use slime_serialize::{Document, Mode, ReadError, References};

use crate::components::Name;
use crate::document::{ADDED, CHANGED, REMOVED, SetAside};
use crate::library::{
    AssetName, MAX_ENTITIES, NO_RECORD, Overrides, Placement, Record, Resolved, ResolvedScene,
    SceneError, SceneFile, SceneLibrary, fingerprint,
};
use crate::transform::Transform;

/// The version of a document's layout: of a scene file's, past 1 only, and of a save's.
pub(crate) const LAYOUT: &str = "layout";
/// The assets that the file names, by their short names.
pub(crate) const ASSETS: &str = "assets";
/// The scene's root.
pub(crate) const ROOT: &str = "root";
/// The entities that the file makes, and its placements, by their IDs.
pub(crate) const ENTITIES: &str = "entities";
/// An entity's parent.
pub(crate) const PARENT: &str = "parent";
/// The prefab that a placement places, by the short name of its asset.
pub(crate) const PREFAB: &str = "prefab";
/// An entity's components, by their types' names.
pub(crate) const COMPONENTS: &str = "components";
/// An asset's ID.
pub(crate) const ID: &str = "id";
/// Where an asset's source was when the file was written.
pub(crate) const PATH: &str = "path";
/// The entities removed, in [`REMOVED`].
pub(crate) const ENTITIES_REMOVED: &str = "entities";
/// The components removed, in [`REMOVED`].
pub(crate) const COMPONENTS_REMOVED: &str = "components";

/// The layout of scene files that this program reads.
const LAYOUT_VERSION: u64 = 1;
/// What a reference names the scene's root.
pub(crate) const ROOT_NAME: &str = ".";

/// An entity that a scene file makes, or a placement, as its record says.
struct Entry {
    id: String,
    parent: Option<String>,
    prefab: Option<AssetId>,
}

/// The layout of a scene file: its assets, by their short names and in the order of its table, and
/// its entities' records.
struct Layout {
    assets: HashMap<String, AssetId>,
    table: Vec<AssetName>,
    entries: Vec<Entry>,
}

/// Names entities by their paths from the scene's root, and assets by the short names of the file's
/// table, as a scene file names them; reading only, as nothing writes a scene file through it.
struct SceneNames<'a> {
    /// The index of each entity's record, by its path, `.` for the root.
    entities: &'a HashMap<String, u32>,
    assets: &'a HashMap<String, AssetId>,
    /// The IDs of the file's placements.
    placements: HashSet<&'a str>,
    /// The paths read since they were last taken that lead into a placement whose prefab has no
    /// entity there: references to no entity.
    gone: RefCell<Vec<String>>,
}

impl SceneNames<'_> {
    /// The paths read since this was last called that lead into a placement whose prefab has no
    /// entity there.
    fn take_gone(&self) -> Vec<String> {
        self.gone.take()
    }
}

impl References for SceneNames<'_> {
    fn name(&self, _: u64) -> Option<String> {
        None
    }

    /// `""` is no entity, and so is a path into a placement whose prefab has no entity there, as a
    /// save reads a reference to an entity that a scene no longer holds.
    fn entity(&self, name: &str) -> Option<u64> {
        if name.is_empty() {
            return Some(NO_RECORD);
        }
        if let Some(&index) = self.entities.get(name) {
            return Some(u64::from(index));
        }
        let (first, _) = name.split_once('/')?;
        if self.placements.contains(first) {
            self.gone.borrow_mut().push(name.to_owned());
            return Some(NO_RECORD);
        }
        None
    }

    fn asset(&self, name: &str) -> Option<u128> {
        self.assets.get(name).map(|id| id.as_u128())
    }
}

/// Resolves the scene file `source`, its text or its binary encoding, the scene `id` named `name`,
/// the prefabs that it places through `library`, within the scenes being resolved, `chain`.
pub(crate) fn resolve(
    library: &mut SceneLibrary,
    registry: &Registry,
    id: AssetId,
    name: &str,
    source: &SceneFile,
    chain: &mut Vec<(AssetId, String)>,
) -> Result<ResolvedScene, SceneError> {
    let file = |error: ReadError| SceneError::File {
        scene: name.to_owned(),
        error,
    };
    let (mut document, bytes) = match source {
        SceneFile::Text(text) => (
            Document::parse(registry, text, Mode::Strict).map_err(file)?,
            text.as_bytes(),
        ),
        SceneFile::Binary(bytes) => (
            Document::parse_binary(registry, bytes, Mode::Strict).map_err(file)?,
            bytes.as_slice(),
        ),
    };
    let Layout {
        assets,
        table,
        entries,
    } = layout(&mut document).map_err(file)?;
    // Resolved first: the entities of a prefab are the scene's too.
    let mut prefabs = Vec::with_capacity(entries.len());
    for entry in &entries {
        prefabs.push(match entry.prefab {
            Some(prefab) => Some(library.resolve_within(registry, prefab, chain)?),
            None => None,
        });
    }
    let mut scene = Resolution {
        registry,
        entries: &entries,
        prefabs: &prefabs,
        records: Vec::new(),
        paths: HashMap::new(),
        at: Vec::with_capacity(entries.len()),
        overrides: (0..entries.len()).map(|_| Overrides::default()).collect(),
        set_aside: vec![Vec::new(); entries.len()],
    };
    scene.place(name)?;
    scene.parents(&document).map_err(file)?;
    let paths = std::mem::take(&mut scene.paths);
    let names = SceneNames {
        entities: &paths,
        assets: &assets,
        placements: entries
            .iter()
            .zip(&prefabs)
            .filter(|(_, prefab)| prefab.is_some())
            .map(|(entry, _)| entry.id.as_str())
            .collect(),
        gone: RefCell::new(Vec::new()),
    };
    scene.components(&mut document, &names).map_err(file)?;
    scene.overrides(&mut document, &names).map_err(file)?;
    let mut hash = fingerprint(0xcbf2_9ce4_8422_2325, bytes);
    for prefab in prefabs.iter().flatten() {
        hash = fingerprint(hash, &prefab.0.fingerprint.to_le_bytes());
    }
    Ok(scene.finish(id, name, hash, table))
}

/// What a cooker checks of a scene file before it cooks it, read without the types of its values
/// (`docs/adr/0095-cooked-scene-files.md`): its table of assets and its entities, each with the
/// line of the text where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneOutline {
    /// The assets of its table, in their order.
    pub assets: Vec<OutlineAsset>,
    /// The entities that it makes and its placements, in their order.
    pub entities: Vec<OutlineEntity>,
}

/// An asset of a scene file's table, as [`outline`] reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineAsset {
    /// Its short name, which the file's values use.
    pub name: String,
    /// Its ID, which decides.
    pub id: AssetId,
    /// Where its source was, relative to the scene file's directory, if the table says.
    pub path: Option<String>,
    /// The line of its entry, from 1.
    pub line: usize,
}

/// An entity of a scene file, or a placement, as [`outline`] reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineEntity {
    /// Its ID in the file.
    pub id: String,
    /// What it places, if it is a placement.
    pub prefab: Option<AssetId>,
    /// The line of its record, from 1.
    pub line: usize,
}

/// Reads the scene file `text`, the scene named `name`, without the types of its values, as a
/// cooker does before it cooks it: its version, its table of assets, its entities, and what each
/// places. It refuses what resolving it refuses of those, without the prefabs that it places: a
/// text that is not TOML or not a scene file's, a key that a scene file does not have, an ID that
/// an entity cannot have, an asset of the table without an ID or named twice, a prefab that the
/// table does not name, a placement that holds components, a parent that names no entity of the
/// file, and entities that would be each other's ancestors. A parent inside a placement, and the
/// values of components and overrides, are checked when the scene is resolved, with its prefabs
/// and the game's types ([`SceneLibrary::resolve`]).
///
/// # Errors
///
/// [`SceneError::File`], where, by line and column.
pub fn outline(name: &str, text: &str) -> Result<SceneOutline, SceneError> {
    let file = |error: ReadError| SceneError::File {
        scene: name.to_owned(),
        error,
    };
    let mut registry = Registry::new();
    registry.register::<String>();
    registry.register::<u64>();
    // Lenient, so that the versions of the game's types, which the cooker does not know, are left
    // alone: the layout refuses a key that a scene file does not have whatever the mode.
    let mut document = Document::parse(&registry, text, Mode::Lenient).map_err(file)?;
    let Layout { table, entries, .. } = layout(&mut document).map_err(file)?;
    let line = |document: &Document<'_>, place: &[&str]| {
        document
            .error_at(place, Problem::Invalid(String::new()))
            .line_and_column()
            .map_or(1, |(line, _)| line)
    };
    let ids: HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.id.as_str(), index))
        .collect();
    // Each entity's parent in the file: the root, or the entity that its parent's path starts with.
    let mut parents = Vec::with_capacity(entries.len());
    for entry in &entries {
        let parent = match entry.parent.as_deref() {
            None | Some(ROOT_NAME) => None,
            Some(parent) => {
                let first = parent.split('/').next().unwrap_or(parent);
                match ids.get(first) {
                    Some(&index) => Some(index),
                    None => {
                        return Err(file(document.error_at(
                            &[ENTITIES, entry.id.as_str(), PARENT],
                            Problem::Invalid(format!("no entity `{parent}` in this scene")),
                        )));
                    }
                }
            }
        };
        parents.push(parent);
    }
    for (index, entry) in entries.iter().enumerate() {
        let mut next = parents[index];
        for _ in 0..entries.len() {
            let Some(at) = next else {
                break;
            };
            if at == index {
                return Err(file(document.error_at(
                    &[ENTITIES, entry.id.as_str(), PARENT],
                    Problem::Invalid(format!("`{}` would be its own ancestor", entry.id)),
                )));
            }
            next = parents[at];
        }
    }
    Ok(SceneOutline {
        assets: table
            .into_iter()
            .map(|asset| OutlineAsset {
                line: line(&document, &[ASSETS, asset.name.as_str()]),
                name: asset.name,
                id: asset.id,
                path: asset.path,
            })
            .collect(),
        entities: entries
            .into_iter()
            .map(|entry| OutlineEntity {
                line: line(&document, &[ENTITIES, entry.id.as_str()]),
                id: entry.id,
                prefab: entry.prefab,
            })
            .collect(),
    })
}

/// Reads the layout of a scene file: its version, its table of assets, and its entities' records,
/// their components and its overrides left to read once the prefabs are resolved.
fn layout(document: &mut Document<'_>) -> Result<Layout, ReadError> {
    known(
        document,
        &[],
        &[LAYOUT, ASSETS, ROOT, ENTITIES, CHANGED, ADDED, REMOVED],
    )?;
    if let Some(Value::UInt(version)) = document.read(&[LAYOUT], "u64", None)?
        && !(1..=LAYOUT_VERSION).contains(&version)
    {
        return Err(document.error_at(
            &[LAYOUT],
            Problem::Invalid(format!(
                "a scene file of layout {version}, where this program reads layouts 1 to \
                 {LAYOUT_VERSION}"
            )),
        ));
    }
    let mut assets = HashMap::new();
    let mut written = Vec::new();
    let mut named: HashMap<AssetId, String> = HashMap::new();
    for name in table(document, &[ASSETS])? {
        let place = [ASSETS, name.as_str()];
        known(document, &place, &[ID, PATH])?;
        let id_place = [ASSETS, name.as_str(), ID];
        let id = match document.read(&id_place, "String", None)? {
            Some(Value::Text(text)) => AssetId::from_str(&text).map_err(|_| {
                document.error_at(
                    &id_place,
                    Problem::Invalid(format!("`{text}` is not an asset ID")),
                )
            })?,
            _ => {
                return Err(document.error_at(
                    &place,
                    Problem::Invalid("an asset of the table gives its `id`".to_owned()),
                ));
            }
        };
        let path = match document.read(&[ASSETS, name.as_str(), PATH], "String", None)? {
            Some(Value::Text(path)) => Some(path),
            _ => None,
        };
        if let Some(other) = named.insert(id, name.clone()) {
            return Err(document.error_at(
                &id_place,
                Problem::Invalid(format!("the asset {id} is named `{other}` already")),
            ));
        }
        written.push(AssetName {
            name: name.clone(),
            id,
            path,
        });
        assets.insert(name, id);
    }
    if holds(document, &[ROOT]) {
        known(document, &[ROOT], &[COMPONENTS])?;
        table(document, &[ROOT, COMPONENTS])?;
    }
    let mut entries = Vec::new();
    for id in table(document, &[ENTITIES])? {
        let place = [ENTITIES, id.as_str()];
        if let Err(reason) = valid_id(&id) {
            return Err(document.error_at(&place, Problem::Invalid(reason)));
        }
        let keys = known(document, &place, &[PARENT, PREFAB, COMPONENTS])?;
        let text = |document: &mut Document<'_>, key| match document.read(
            &[ENTITIES, id.as_str(), key],
            "String",
            None,
        ) {
            Ok(Some(Value::Text(text))) => Ok(Some(text)),
            Ok(_) => Ok(None),
            Err(error) => Err(error),
        };
        let parent = text(document, PARENT)?;
        let prefab = match text(document, PREFAB)? {
            Some(prefab) => Some(*assets.get(&prefab).ok_or_else(|| {
                document.error_at(
                    &[ENTITIES, id.as_str(), PREFAB],
                    Problem::Invalid(format!("no asset `{prefab}` in the table of assets")),
                )
            })?),
            None => None,
        };
        if keys.iter().any(|key| key == COMPONENTS) {
            if prefab.is_some() {
                return Err(document.error_at(
                    &[ENTITIES, id.as_str(), COMPONENTS],
                    Problem::Invalid(
                        "a placement's components are its prefab's: what it changes of them goes \
                         in `changed`, `added` and `removed`"
                            .to_owned(),
                    ),
                ));
            }
            table(document, &[ENTITIES, id.as_str(), COMPONENTS])?;
        }
        entries.push(Entry { id, parent, prefab });
    }
    Ok(Layout {
        assets,
        table: written,
        entries,
    })
}

/// Accepts an ID that an entity of a scene file may have, refuses it with the reason otherwise.
pub(crate) fn valid_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        Err("an entity's ID is not empty".to_owned())
    } else if id.contains('/') {
        Err(format!(
            "`{id}` holds `/`, which joins the IDs of an identity's parts"
        ))
    } else if id.starts_with('#') {
        Err(format!(
            "`{id}` starts with `#`, as the IDs that glTF nodes without names take"
        ))
    } else if id == ROOT_NAME {
        Err(format!("`{ROOT_NAME}` names the scene's root"))
    } else {
        Ok(())
    }
}

/// Whether the document holds something at `place`, which is not empty.
fn holds(document: &Document<'_>, place: &[&str]) -> bool {
    let (last, parent) = place.split_last().expect("a place within the document");
    document.keys(parent).iter().any(|key| key == last)
}

/// The keys of the table at `place`; none if the document holds nothing there.
///
/// # Errors
///
/// When it holds something other than a table there.
fn table(document: &Document<'_>, place: &[&str]) -> Result<Vec<String>, ReadError> {
    if document.is_table(place) {
        Ok(document.keys(place))
    } else if holds(document, place) {
        Err(document.error_at(place, Problem::Invalid("a table".to_owned())))
    } else {
        Ok(Vec::new())
    }
}

/// The keys of the table at `place`, each one of `known`.
///
/// # Errors
///
/// When it holds something other than a table, or a key that is not one of `known`.
fn known(
    document: &Document<'_>,
    place: &[&str],
    known: &[&str],
) -> Result<Vec<String>, ReadError> {
    let keys = table(document, place)?;
    if let Some(key) = keys.iter().find(|key| !known.contains(&key.as_str())) {
        let mut at = place.to_vec();
        at.push(key);
        return Err(document.error_at(
            &at,
            Problem::UnknownField {
                field: key.clone(),
                known: known.iter().map(|&key| key.to_owned()).collect(),
            },
        ));
    }
    Ok(keys)
}

/// A scene file being resolved: its records, as its entries and its prefabs give them, then its
/// values and its overrides.
struct Resolution<'a> {
    registry: &'a Registry,
    entries: &'a [Entry],
    /// Each entry's prefab, resolved, if it is a placement.
    prefabs: &'a [Option<ResolvedScene>],
    records: Vec<Record>,
    /// The index of each record, by its path, `.` for the root, until its values are read.
    paths: HashMap<String, u32>,
    /// The index of each entry's record, the root's of its placement for a placement.
    at: Vec<u32>,
    /// Each entry's overrides, as the file writes them, for a placement.
    overrides: Vec<Overrides>,
    /// Each entry's overrides set aside, for a placement.
    set_aside: Vec<Vec<SetAside>>,
}

impl Resolution<'_> {
    /// Makes the root's record, and those of the entries, each placement's its prefab's, under its
    /// ID.
    ///
    /// # Errors
    ///
    /// [`SceneError::TooLarge`] for a scene of more than [`MAX_ENTITIES`] entities: the scene `name`.
    fn place(&mut self, name: &str) -> Result<(), SceneError> {
        let too_large = || SceneError::TooLarge {
            scene: name.to_owned(),
        };
        // Counted before anything is made, so that a scene too large costs nothing.
        let entities: usize = 1 + self
            .prefabs
            .iter()
            .map(|prefab| prefab.as_ref().map_or(1, |prefab| prefab.0.records.len()))
            .sum::<usize>();
        if entities > MAX_ENTITIES {
            return Err(too_large());
        }
        self.records.reserve(entities);
        self.records.push(Record {
            path: Vec::new(),
            parent: None,
            components: Vec::new(),
            placement: None,
        });
        self.paths.insert(ROOT_NAME.to_owned(), 0);
        for (entry, prefab) in self.entries.iter().zip(self.prefabs) {
            let base = u32::try_from(self.records.len()).map_err(|_| too_large())?;
            self.at.push(base);
            let Some(prefab) = prefab else {
                self.paths.insert(entry.id.clone(), base);
                self.records.push(Record {
                    path: vec![entry.id.clone()],
                    parent: None,
                    components: Vec::new(),
                    placement: None,
                });
                continue;
            };
            for record in &prefab.0.records {
                let mut path = Vec::with_capacity(record.path.len() + 1);
                path.push(entry.id.clone());
                path.extend(record.path.iter().cloned());
                let index = u32::try_from(self.records.len()).map_err(|_| too_large())?;
                self.paths.insert(path.join("/"), index);
                self.records.push(Record {
                    path,
                    parent: record.parent.map(|parent| parent + base),
                    components: record
                        .components
                        .iter()
                        .map(|(name, value)| (name.clone(), shifted(value.clone(), base)))
                        .collect(),
                    placement: record.placement.clone(),
                });
            }
            let root = &mut self.records[base as usize];
            if !root.components.iter().any(|(name, _)| name == "Name") {
                root.components
                    .push(("Name".to_owned(), Name(entry.id.clone()).to_value()));
            }
        }
        Ok(())
    }

    /// Gives each entry's record its parent, the root's if it names none. One whose parent is in a
    /// placement whose prefab no longer has it is the root's child, and set aside, as an entity of a
    /// save whose parent the world does not have is left out of its own.
    ///
    /// # Errors
    ///
    /// A parent that the scene does not have, or an entity that would be its own ancestor.
    fn parents(&mut self, document: &Document<'_>) -> Result<(), ReadError> {
        let entries = self.entries;
        for (index, entry) in entries.iter().enumerate() {
            let parent = match &entry.parent {
                None => 0,
                Some(parent) => match (self.paths.get(parent), self.placement_of(parent)) {
                    (Some(&found), _) => found,
                    (None, Some(placement)) => {
                        let reason = format!(
                            "its parent `{parent}` is not in the prefab: the scene's root is its \
                             parent"
                        );
                        self.aside(placement, &entry.id, None, reason);
                        0
                    }
                    (None, None) => {
                        return Err(document.error_at(
                            &[ENTITIES, entry.id.as_str(), PARENT],
                            Problem::Invalid(format!("no entity `{parent}` in this scene")),
                        ));
                    }
                },
            };
            self.records[self.at[index] as usize].parent = Some(parent);
        }
        for (entry, &at) in self.entries.iter().zip(&self.at) {
            let mut next = self.records[at as usize].parent;
            let mut steps = 0;
            while let Some(parent) = next {
                steps += 1;
                if parent == at || steps > self.records.len() {
                    return Err(document.error_at(
                        &[ENTITIES, entry.id.as_str(), PARENT],
                        Problem::Invalid(format!("`{}` would be its own ancestor", entry.id)),
                    ));
                }
                next = self.records[parent as usize].parent;
            }
        }
        Ok(())
    }

    /// Reads the components of the root and of the entities that the file makes; the root holds
    /// [`Transform::IDENTITY`] unless the file gives it another.
    fn components(
        &mut self,
        document: &mut Document<'_>,
        names: &SceneNames<'_>,
    ) -> Result<(), ReadError> {
        let (components, gone) =
            read_components(self.registry, document, &[ROOT, COMPONENTS], names)?;
        self.records[0].components = components;
        self.report_gone(ROOT_NAME, gone);
        if !self.records[0]
            .components
            .iter()
            .any(|(name, _)| name == "Transform")
        {
            self.records[0]
                .components
                .push(("Transform".to_owned(), Transform::IDENTITY.to_value()));
        }
        let entries = self.entries;
        for (index, entry) in entries.iter().enumerate() {
            if self.prefabs[index].is_none() {
                let place = [ENTITIES, entry.id.as_str(), COMPONENTS];
                let (components, gone) = read_components(self.registry, document, &place, names)?;
                self.records[self.at[index] as usize].components = components;
                self.report_gone(&entry.id, gone);
            }
        }
        Ok(())
    }

    /// Reports the references of the components of the file's entity `id`, each with its component,
    /// to entities that the prefab of the placement they lead into no longer has.
    fn report_gone(&mut self, id: &str, gone: Vec<(String, String)>) {
        for (component, path) in gone {
            if let Some(placement) = self.placement_of(&path) {
                self.aside(placement, id, Some(&component), gone_reason(&path));
            }
        }
    }

    /// The entry of the placement that `path` leads into, past its root, if it leads into one.
    fn placement_of(&self, path: &str) -> Option<usize> {
        let (first, _) = path.split_once('/')?;
        self.entries
            .iter()
            .position(|entry| entry.id == first)
            .filter(|&entry| self.prefabs[entry].is_some())
    }

    /// Reads the file's overrides and applies them, in the order that saves apply theirs: the
    /// entities removed, the components removed, added, then changed. One whose entity or component
    /// the prefab no longer has, or that no longer fits it, is set aside; so is a reference to an
    /// entity that a prefab no longer has, which refers to no entity. The entities removed are all
    /// found before any is removed; an entity of the file under one removed, or a placement, is the
    /// root's child then, and set aside, as a save keeps an entity that it does not remove.
    ///
    /// # Errors
    ///
    /// An override of something other than a placement's entity, a component given twice for one
    /// entity, or a value that its type refuses.
    fn overrides(
        &mut self,
        document: &mut Document<'_>,
        names: &SceneNames<'_>,
    ) -> Result<(), ReadError> {
        let mut removed = vec![false; self.records.len()];
        if holds(document, &[REMOVED]) {
            known(
                document,
                &[REMOVED],
                &[ENTITIES_REMOVED, COMPONENTS_REMOVED],
            )?;
            let place = [REMOVED, ENTITIES_REMOVED];
            let mut found = Vec::new();
            for path in texts(document.read(&place, "Vec<String>", None)?) {
                let (entry, inner) = self.target(document, &place, &path)?;
                if inner.is_empty() {
                    return Err(document.error_at(
                        &place,
                        Problem::Invalid(format!(
                            "`{path}` is a placement: remove its record to remove it"
                        )),
                    ));
                }
                match Self::find(names.entities, &path, &removed) {
                    Ok(index) => found.push(index),
                    Err(reason) => self.aside(entry, &path, None, reason),
                }
                self.overrides[entry].removed.push(inner);
            }
            self.remove(&found, &mut removed);
            for path in table(document, &[REMOVED, COMPONENTS_REMOVED])? {
                let place = [REMOVED, COMPONENTS_REMOVED, path.as_str()];
                let (entry, inner) = self.target(document, &place, &path)?;
                let mut components: Vec<String> = Vec::new();
                for component in texts(document.read(&place, "Vec<String>", None)?) {
                    let name = self.component_name(document, &place, &component)?;
                    if components.contains(&name) {
                        return Err(twice(document, &place, &name));
                    }
                    let gone = Self::find(names.entities, &path, &removed).and_then(|index| {
                        let held = &mut self.records[index as usize].components;
                        let at = held
                            .iter()
                            .position(|(held, _)| *held == name)
                            .ok_or_else(|| "the entity holds no such component".to_owned())?;
                        held.remove(at);
                        Ok(())
                    });
                    if let Err(reason) = gone {
                        self.aside(entry, &path, Some(&name), reason);
                    }
                    components.push(name);
                }
                self.overrides[entry]
                    .removed_components
                    .push((inner, components));
            }
        }
        for path in table(document, &[ADDED])? {
            let (entry, inner) = self.target(document, &[ADDED, path.as_str()], &path)?;
            let mut given: Vec<String> = Vec::new();
            for component in table(document, &[ADDED, path.as_str()])? {
                let place = [ADDED, path.as_str(), component.as_str()];
                let name = self.component_name(document, &place, &component)?;
                if given.contains(&name) {
                    return Err(twice(document, &place, &name));
                }
                let value = document
                    .read(&place, &component, Some(names))?
                    .expect("a key of the table");
                for gone in names.take_gone() {
                    self.aside(entry, &path, Some(&name), gone_reason(&gone));
                }
                match Self::find(names.entities, &path, &removed) {
                    Ok(index) => {
                        let held = &mut self.records[index as usize].components;
                        match held.iter_mut().find(|(held, _)| *held == name) {
                            Some((_, old)) => *old = value.clone(),
                            None => held.push((name.clone(), value.clone())),
                        }
                    }
                    Err(reason) => self.aside(entry, &path, Some(&name), reason),
                }
                given.push(name.clone());
                self.overrides[entry]
                    .added
                    .push((inner.clone(), name, value));
            }
        }
        for path in table(document, &[CHANGED])? {
            let (entry, inner) = self.target(document, &[CHANGED, path.as_str()], &path)?;
            let mut given: Vec<String> = Vec::new();
            for component in table(document, &[CHANGED, path.as_str()])? {
                let place = [CHANGED, path.as_str(), component.as_str()];
                let name = self.component_name(document, &place, &component)?;
                if given.contains(&name) {
                    return Err(twice(document, &place, &name));
                }
                let part = document
                    .read_part(&place, &component, Some(names))?
                    .expect("a key of the table");
                for gone in names.take_gone() {
                    self.aside(entry, &path, Some(&name), gone_reason(&gone));
                }
                let registry = self.registry;
                let changed = Self::find(names.entities, &path, &removed).and_then(|index| {
                    let held = &mut self.records[index as usize].components;
                    let (_, value) = held
                        .iter_mut()
                        .find(|(held, _)| *held == name)
                        .ok_or_else(|| "the entity holds no such component".to_owned())?;
                    let mut overlaid = value.clone();
                    registry
                        .overlay(&name, &mut overlaid, &part)
                        .map_err(|error| error.to_string())?;
                    *value = overlaid;
                    Ok(())
                });
                if let Err(reason) = changed {
                    self.aside(entry, &path, Some(&name), reason);
                }
                given.push(name.clone());
                self.overrides[entry]
                    .changed
                    .push((inner.clone(), name, part));
            }
        }
        self.compact(&removed);
        Ok(())
    }

    /// The entry of the placement that the override of `path`, at `place`, is for, and the IDs of
    /// its entity from the placement's root.
    ///
    /// # Errors
    ///
    /// When the path does not lead into one of the file's placements.
    fn target(
        &self,
        document: &Document<'_>,
        place: &[&str],
        path: &str,
    ) -> Result<(usize, Vec<String>), ReadError> {
        let mut parts = path.split('/');
        let first = parts.next().unwrap_or_default();
        let inner: Vec<String> = parts.map(str::to_owned).collect();
        let invalid = |reason: String| document.error_at(place, Problem::Invalid(reason));
        if inner.iter().any(String::is_empty) {
            return Err(invalid(format!("`{path}` is not a path of IDs")));
        }
        let entry = self
            .entries
            .iter()
            .position(|entry| entry.id == first)
            .ok_or_else(|| invalid(format!("no placement `{first}` in this scene")))?;
        if self.prefabs[entry].is_none() {
            return Err(invalid(format!(
                "`{first}` is an entity that this scene makes, not a placement: its record holds \
                 its components"
            )));
        }
        Ok((entry, inner))
    }

    /// The name of the component type `component`, which an override names at `place`: its name, if
    /// `component` is one of its aliases.
    ///
    /// # Errors
    ///
    /// When the registry holds no type of that name.
    fn component_name(
        &self,
        document: &Document<'_>,
        place: &[&str],
        component: &str,
    ) -> Result<String, ReadError> {
        match self.registry.get(component) {
            Some(registration) => Ok(registration.info().name.clone()),
            None => Err(document.error_at(place, Problem::NotRegistered(component.to_owned()))),
        }
    }

    /// The index of the record of `path`, unless the scene no longer has it, or removed it.
    fn find(paths: &HashMap<String, u32>, path: &str, removed: &[bool]) -> Result<u32, String> {
        match paths.get(path) {
            Some(&index) if !removed[index as usize] => Ok(index),
            Some(_) => Err("the scene removes this entity".to_owned()),
            None => Err("the prefab has no entity of this ID".to_owned()),
        }
    }

    /// Marks the records `found` removed, with their descendants but the file's entries, its
    /// entities and its placements, which become the root's children, set aside.
    fn remove(&mut self, found: &[u32], removed: &mut [bool]) {
        let children = self.children();
        let mut entry_at = vec![None; self.records.len()];
        for (entry, &at) in self.at.iter().enumerate() {
            entry_at[at as usize] = Some(entry);
        }
        let mut kept = Vec::new();
        let mut next = found.to_vec();
        while let Some(index) = next.pop() {
            if removed[index as usize] {
                continue;
            }
            removed[index as usize] = true;
            for &child in &children[index as usize] {
                match entry_at[child as usize] {
                    Some(entry) => kept.push((entry, child, index)),
                    None => next.push(child),
                }
            }
        }
        for (entry, child, parent) in kept {
            self.records[child as usize].parent = Some(0);
            let parent = self.records[parent as usize].path.join("/");
            if let Some(placement) = self.placement_of(&parent) {
                let reason = format!(
                    "its parent `{parent}` is removed by this scene: the scene's root is its parent"
                );
                let id = self.entries[entry].id.clone();
                self.aside(placement, &id, None, reason);
            }
        }
    }

    /// The indices of each record's children.
    fn children(&self) -> Vec<Vec<u32>> {
        let mut children = vec![Vec::new(); self.records.len()];
        for (index, record) in self.records.iter().enumerate() {
            if let Some(parent) = record.parent {
                children[parent as usize]
                    .push(u32::try_from(index).expect("fewer records than u32::MAX"));
            }
        }
        children
    }

    /// Sets aside an override of the placement `entry`, at `path`.
    fn aside(&mut self, entry: usize, path: &str, component: Option<&str>, reason: String) {
        self.set_aside[entry].push(SetAside {
            entity: path.to_owned(),
            component: component.map(str::to_owned),
            reason,
        });
    }

    /// Leaves out the records removed, and makes the references to them references to no entity.
    fn compact(&mut self, removed: &[bool]) {
        let mut index = vec![NO_RECORD; removed.len()];
        let mut kept = 0;
        for (at, &gone) in removed.iter().enumerate() {
            if !gone {
                index[at] = kept;
                kept += 1;
            }
        }
        let records = std::mem::take(&mut self.records);
        self.records = records
            .into_iter()
            .zip(removed)
            .filter(|(_, gone)| !**gone)
            .map(|(mut record, _)| {
                record.parent = record.parent.map(|parent| {
                    u32::try_from(index[parent as usize]).expect("a parent is kept with its child")
                });
                for (_, value) in &mut record.components {
                    renumbered(value, &index);
                }
                record
            })
            .collect();
        for overrides in &mut self.overrides {
            for (_, _, value) in overrides.added.iter_mut().chain(&mut overrides.changed) {
                renumbered(value, &index);
            }
        }
        for at in &mut self.at {
            *at = u32::try_from(index[*at as usize]).unwrap_or(u32::MAX);
        }
    }

    /// The scene resolved, its placements remembered.
    fn finish(
        mut self,
        id: AssetId,
        name: &str,
        fingerprint: u64,
        assets: Vec<AssetName>,
    ) -> ResolvedScene {
        let mut set_aside = Vec::new();
        let depth = 1 + self
            .prefabs
            .iter()
            .flatten()
            .map(|prefab| prefab.0.depth)
            .max()
            .unwrap_or(0);
        for (index, entry) in self.entries.iter().enumerate() {
            let Some(prefab) = &self.prefabs[index] else {
                continue;
            };
            set_aside.extend(prefab.0.set_aside.iter().map(|aside| SetAside {
                entity: format!("{}/{}", entry.id, aside.entity),
                ..aside.clone()
            }));
            set_aside.extend(self.set_aside[index].iter().cloned());
            // A placement removed with the entity it was a child of is remembered no more.
            let Some(record) = self.records.get_mut(self.at[index] as usize) else {
                continue;
            };
            record.placement = Some(Arc::new(Placement {
                prefab: prefab.clone(),
                overrides: std::mem::take(&mut self.overrides[index]),
                set_aside: std::mem::take(&mut self.set_aside[index]),
            }));
        }
        ResolvedScene(Arc::new(Resolved {
            id,
            name: name.to_owned(),
            fingerprint,
            depth,
            records: self.records,
            set_aside,
            assets,
        }))
    }
}

/// Reads the components of the table at `place`, by their types' names, aliases given their names,
/// and the references that they hold, each with its component, to entities that the prefab of the
/// placement they lead into no longer has.
///
/// # Errors
///
/// A type that the registry does not hold, a value that its type refuses, or a type given twice,
/// by its name and an alias.
fn read_components(
    registry: &Registry,
    document: &mut Document<'_>,
    place: &[&str],
    names: &SceneNames<'_>,
) -> Result<(Components, Vec<(String, String)>), ReadError> {
    let mut components: Vec<(String, Value)> = Vec::new();
    let mut gone = Vec::new();
    for component in document.keys(place) {
        let mut at = place.to_vec();
        at.push(&component);
        let value = document
            .read(&at, &component, Some(names))?
            .expect("a key of the table");
        let name = registry.get(&component).map_or_else(
            || component.clone(),
            |registration| registration.info().name.clone(),
        );
        if components.iter().any(|(held, _)| *held == name) {
            return Err(document.error_at(
                &at,
                Problem::Invalid(format!("the entity holds `{name}` already")),
            ));
        }
        gone.extend(
            names
                .take_gone()
                .into_iter()
                .map(|path| (name.clone(), path)),
        );
        components.push((name, value));
    }
    Ok((components, gone))
}

/// The error of a component that an override gives twice for one entity, at `place`, by its name and
/// an alias.
fn twice(document: &Document<'_>, place: &[&str], name: &str) -> ReadError {
    document.error_at(
        place,
        Problem::Invalid(format!("the entity's `{name}` is given twice")),
    )
}

/// Why a reference to `path`, which leads into a placement whose prefab has no entity there, refers
/// to no entity.
fn gone_reason(path: &str) -> String {
    format!("it refers to `{path}`, which the prefab does not have: to no entity")
}

/// Components, by their types' names.
type Components = Vec<(String, Value)>;

/// The texts of a list of text, as `Vec<String>` reads it.
fn texts(value: Option<Value>) -> Vec<String> {
    match value {
        Some(Value::List(values)) => values
            .into_iter()
            .filter_map(|value| match value {
                Value::Text(text) => Some(text),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `value`, a prefab's, its references to the prefab's records moved by `base`, where the prefab's
/// records are among a scene's.
fn shifted(mut value: Value, base: u32) -> Value {
    for_each_entity(&mut value, &mut |bits| {
        if *bits != NO_RECORD {
            *bits += u64::from(base);
        }
    });
    value
}

/// Renumbers the references of `value` to records as `index` does, which holds each record's new
/// index, or [`NO_RECORD`] for one left out.
fn renumbered(value: &mut Value, index: &[u64]) {
    for_each_entity(value, &mut |bits| {
        if *bits != NO_RECORD {
            *bits = index[*bits as usize];
        }
    });
}

/// Calls `visit` on the bits of every reference to an entity that `value` holds.
pub(crate) fn for_each_entity(value: &mut Value, visit: &mut impl FnMut(&mut u64)) {
    match value {
        Value::Entity(bits) => visit(bits),
        Value::Struct(fields) | Value::Enum(_, fields) => {
            for (_, field) in fields {
                for_each_entity(field, visit);
            }
        }
        Value::Option(Some(held)) => for_each_entity(held, visit),
        Value::List(elements) => {
            for element in elements {
                for_each_entity(element, visit);
            }
        }
        _ => {}
    }
}
