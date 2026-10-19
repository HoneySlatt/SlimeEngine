//! Scenes ready to spawn: what a scene spawns, its placements and their overrides applied, and the
//! library that resolves scene files and the glTF scenes that they place
//! (`docs/adr/0089-scenes-and-prefabs.md`).

use std::collections::HashMap;
use std::sync::Arc;

use glam::{DAffine3, Mat4};
use slime_assets::{AssetId, AssetKind, CookedAssets, CookedScene, LoadError};
use slime_reflect::{Problem, Reflect, Registry, Value};
use slime_serialize::{Document, Mode, ReadError};

use crate::components::{Extras, Light, Meshes, Name};
use crate::document::SetAside;
use crate::scene_file;
use crate::transform::{GlobalTransform, Transform, split};

/// How deep placements nest, prefabs placed in prefabs placed in a scene: 16 scenes from the one
/// resolved, itself included.
pub const MAX_NESTING: usize = 16;

/// The most entities that a resolved scene holds, its root and its placements' included: 2²⁰, a
/// little over a million, so that a few small files whose placements nest cannot make a scene that
/// fills the memory.
pub const MAX_ENTITIES: usize = 1 << 20;

/// The most bytes that a resolved scene takes in memory, about, its placements' copies of their
/// prefabs counted, and the ID of each placement, which every entity copied repeats: 256 MiB, so
/// that a few small files that place a large prefab many times, or under a long ID, cannot fill the
/// memory, as [`MAX_ENTITIES`] does for a prefab of many entities. The scenes that a save spawns
/// while it is loaded take at most as much, together (`World::load`). Counted for each record and
/// each report of what was set aside, their texts' and names' bytes and, for each value, the size
/// of a [`Value`], which leaves out what the allocator and the vectors' spare room add.
pub const MAX_SCENE_SIZE: usize = 256 << 20;

/// What a template's reference to an entity holds for no entity: `""` in a scene file.
pub(crate) const NO_RECORD: u64 = u64::MAX;

/// The scenes that a game has, by their asset IDs: glTF scenes as the cooker makes them, and scene
/// files, each resolved once into what it spawns ([`SceneLibrary::resolve`]), the prefabs it places
/// resolved with it (`docs/adr/0089-scenes-and-prefabs.md`).
///
/// # The layout of a scene file
///
/// A scene file is a TOML document, which `slime-serialize` reads as its conventions say
/// ([`slime_serialize::format`]), the versions of the types past version 1 under `$versions` at its
/// top (`docs/adr/0091-scene-documents-and-placements.md`). Its keys:
///
/// - `layout`: the version of its layout, 1, which the file may leave out; a later one is refused.
/// - `[assets]`: the assets that its values and its placements name, each under a short name, `lamp
///   = { id = "<UUID>", path = "lamp.scene" }`. The ID decides; the path, relative to the file's
///   directory and written with `/`, is a hint, which the cooker checks.
/// - `[root.components]`: the components of the scene's root, by their types' names.
/// - `[entities]`: the entities that the file makes and its placements, each under an ID of the
///   file, which is not empty, holds no `/`, does not start with `#` and is not `.`; their order is
///   that of siblings. An entity's record holds its `parent`, another entity's ID, a path into a
///   placement, `porch/post`, or `.`, the root, which a record without a parent has; and its
///   `components`, by their types' names. A placement's record holds `prefab`, the short name of a
///   scene of the table, and may hold a parent, but no components: its entities are known through
///   it, `porch/light`.
/// - The overrides of the placements, in the sections of a save ([`World::save`](crate::World)):
///   `[changed."<path>".<type>]` holds the parts of a component that the file changes,
///   `[added."<path>"]` the components that it adds, whole, and `[removed]` what it removes,
///   `entities = ["<path>"]` and `components = { "<path>" = ["<type>"] }`.
///
/// A value names an entity by its path from the file's root, `.` for the root and `""` for no
/// entity, and an asset by its short name. The cooker cooks a scene file into the binary encoding
/// of the same document (`docs/adr/0095-cooked-scene-files.md`), which a library reads as it reads
/// the text.
///
/// ```
/// use slime_assets::AssetId;
/// use slime_world::{PersistentId, SceneLibrary, World};
///
/// let (yard, lamp) = (AssetId::from_u128(1), AssetId::from_u128(2));
/// let mut scenes = SceneLibrary::new();
/// scenes.insert_file(lamp, "lamp", r#"
/// [entities.light.components]
/// Light = { Point = { color = { r = 1.0, g = 0.8, b = 0.6, a = 1.0 }, intensity = 40.0, range = 8.0 } }
/// "#);
/// scenes.insert_file(yard, "yard", r#"
/// [assets]
/// lamp = { id = "00000000-0000-0000-0000-000000000002", path = "lamp.scene" }
///
/// [entities.porch]
/// prefab = "lamp"
///
/// [entities.gate]
/// prefab = "lamp"
///
/// [changed."gate/light".Light.Point]
/// intensity = 10.0
/// "#);
/// let mut world = World::new();
/// let scene = scenes.resolve(world.registry(), yard).expect("a scene");
/// world.spawn_scene("yard", &scene).expect("spawned");
/// assert!(world.find(&PersistentId::new(["yard", "gate", "light"])).is_some());
/// ```
#[derive(Default)]
pub struct SceneLibrary {
    sources: HashMap<AssetId, Source>,
    resolved: HashMap<AssetId, ResolvedScene>,
    /// The cooked data whose scenes it reads when they are first resolved, if it reads some.
    assets: Option<CookedAssets>,
}

/// A scene as a library holds it.
enum Source {
    /// A glTF scene, as the cooker makes it.
    Cooked(CookedScene),
    /// A scene file, and the scene's name, for messages.
    File { name: String, file: SceneFile },
}

/// A scene file's document: its text, or its binary encoding, as the cooker writes it.
#[derive(Clone)]
pub(crate) enum SceneFile {
    Text(String),
    Binary(Vec<u8>),
}

impl std::fmt::Debug for SceneLibrary {
    /// How many scenes it holds.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneLibrary")
            .field("scenes", &self.sources.len())
            .finish_non_exhaustive()
    }
}

impl SceneLibrary {
    /// A library that holds no scene.
    pub fn new() -> Self {
        Self::default()
    }

    /// A library of the scenes of `assets`, the cooked data of a game: glTF scenes and scene files,
    /// each read from its cooked file when it is first resolved
    /// (`docs/adr/0095-cooked-scene-files.md`). A scene inserted by hand stands for one of the same
    /// ID in `assets`, until a reload changes it.
    pub fn of(assets: CookedAssets) -> Self {
        Self {
            assets: Some(assets),
            ..Self::default()
        }
    }

    /// Takes `assets` in place of the cooked data that the library reads, as a game does when the
    /// cooked data changes on disk (`docs/adr/0076-hot-reload-of-cooked-data.md`), and returns the
    /// IDs of the scenes whose cooked files or sources changed, or that the new data no longer has,
    /// in the order of the IDs. Those are read again when next resolved, in place of a scene of the
    /// same ID inserted by hand, and every scene is resolved again, so that a scene that places a
    /// prefab that changed follows it. A game that carries its run into a world spawned anew learns
    /// which of the scenes that it spawned changed ([`World::carry`](crate::World::carry)).
    pub fn reload(&mut self, assets: CookedAssets) -> Vec<AssetId> {
        let file = |assets: &CookedAssets, id| {
            assets
                .manifest()
                .get(id)
                .filter(|entry| matches!(entry.kind, AssetKind::Scene | AssetKind::SceneFile))
                .map(|entry| (entry.file.clone(), entry.source.clone()))
        };
        let changed: Vec<AssetId> = match &self.assets {
            Some(old) => {
                let scenes = |assets: &CookedAssets| {
                    assets
                        .manifest()
                        .iter()
                        .filter(|(_, entry)| {
                            matches!(entry.kind, AssetKind::Scene | AssetKind::SceneFile)
                        })
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>()
                };
                let mut ids = scenes(old);
                ids.extend(scenes(&assets));
                ids.sort();
                ids.dedup();
                ids.retain(|&id| file(old, id) != file(&assets, id));
                ids
            }
            None => Vec::new(),
        };
        for id in &changed {
            self.sources.remove(id);
        }
        if !changed.is_empty() {
            self.resolved.clear();
        }
        self.assets = Some(assets);
        changed
    }

    /// Holds `scene`, a glTF scene as the cooker makes it, under its asset ID, in place of the scene
    /// that the ID named, if one: the scenes resolved before are resolved again when next asked.
    ///
    /// # Panics
    ///
    /// If a node's parent does not come after it, which a scene read from cooked data never has
    /// (`CookedScene::from_bytes` refuses it).
    pub fn insert_cooked(&mut self, id: AssetId, scene: CookedScene) {
        check_order(&scene);
        self.resolved.clear();
        self.sources.insert(id, Source::Cooked(scene));
    }

    /// Holds the text of a scene file, under its asset ID, `name` naming it in messages, in place of
    /// the scene that the ID named, if one: the scenes resolved before are resolved again when next
    /// asked. The text is read when the scene is resolved.
    pub fn insert_file(&mut self, id: AssetId, name: impl Into<String>, text: impl Into<String>) {
        self.resolved.clear();
        self.sources.insert(
            id,
            Source::File {
                name: name.into(),
                file: SceneFile::Text(text.into()),
            },
        );
    }

    /// The scene `id` resolved into what it spawns, with the types of `registry`, a world's
    /// ([`World::registry`](crate::World::registry)): a glTF scene as
    /// [`ResolvedScene::from_cooked`] makes it; a scene file read strictly, the prefabs that it
    /// places resolved, at most [`MAX_NESTING`] deep, and its overrides applied. A scene is resolved
    /// once, then shared, until a scene is inserted again or a reload changes one
    /// ([`SceneLibrary::reload`]), with the types of the registry given the first time it is asked
    /// for: a library serves the worlds of one game, whose types are the same.
    ///
    /// # Errors
    ///
    /// [`SceneError::Missing`] for a scene that the library does not hold; [`SceneError::File`] for
    /// a scene file that says something wrong, where; [`SceneError::Cycle`] for prefabs that place
    /// each other; [`SceneError::TooDeep`] for placements nested deeper than [`MAX_NESTING`];
    /// [`SceneError::TooLarge`] for a scene of more than [`MAX_ENTITIES`] entities, or that takes
    /// more than [`MAX_SCENE_SIZE`] bytes; [`SceneError::Node`] for a glTF scene's typed component
    /// that `registry` refuses; [`SceneError::Load`] for a scene whose cooked file cannot be read.
    pub fn resolve(
        &mut self,
        registry: &Registry,
        id: AssetId,
    ) -> Result<ResolvedScene, SceneError> {
        self.resolve_within(registry, id, &mut Vec::new())
    }

    /// [`SceneLibrary::resolve`], within the scenes being resolved, `chain`, from the outermost.
    pub(crate) fn resolve_within(
        &mut self,
        registry: &Registry,
        id: AssetId,
        chain: &mut Vec<(AssetId, String)>,
    ) -> Result<ResolvedScene, SceneError> {
        let names = |chain: &[(AssetId, String)], last: &str| {
            chain
                .iter()
                .map(|(_, name)| name.clone())
                .chain(std::iter::once(last.to_owned()))
                .collect()
        };
        if let Some(resolved) = self.resolved.get(&id) {
            // A scene resolved alone first may nest too deep where another places it.
            if chain.len() + resolved.0.depth > MAX_NESTING {
                return Err(SceneError::TooDeep(names(chain, &resolved.0.name)));
            }
            return Ok(resolved.clone());
        }
        if !self.sources.contains_key(&id)
            && let Some(source) = self.read(id)?
        {
            self.sources.insert(id, source);
        }
        let source = self.sources.get(&id).ok_or_else(|| SceneError::Missing {
            id,
            placed_by: chain.last().map(|(_, name)| name.clone()),
        })?;
        let name = match source {
            Source::Cooked(scene) => scene.name.clone(),
            Source::File { name, .. } => name.clone(),
        };
        if let Some(start) = chain.iter().position(|&(held, _)| held == id) {
            return Err(SceneError::Cycle(names(&chain[start..], &name)));
        }
        if chain.len() == MAX_NESTING {
            return Err(SceneError::TooDeep(names(chain, &name)));
        }
        let file = match source {
            Source::Cooked(scene) => {
                let resolved = ResolvedScene::from_cooked(id, scene, registry)?;
                self.resolved.insert(id, resolved.clone());
                return Ok(resolved);
            }
            Source::File { file, .. } => file.clone(),
        };
        chain.push((id, name.clone()));
        let resolved = scene_file::resolve(self, registry, id, &name, &file, chain);
        chain.pop();
        let resolved = resolved?;
        self.resolved.insert(id, resolved.clone());
        Ok(resolved)
    }
}

impl SceneLibrary {
    /// The scene `id` of the cooked data that the library reads, if it lists one by that ID.
    ///
    /// # Errors
    ///
    /// [`SceneError::Load`] if its cooked file cannot be read, or is not a scene's.
    fn read(&self, id: AssetId) -> Result<Option<Source>, SceneError> {
        let Some(assets) = &self.assets else {
            return Ok(None);
        };
        let Some(entry) = assets.manifest().get(id) else {
            return Ok(None);
        };
        let load = |error| SceneError::Load { id, error };
        match entry.kind {
            AssetKind::SceneFile => Ok(Some(Source::File {
                name: std::path::Path::new(&entry.source).file_stem().map_or_else(
                    || entry.source.clone(),
                    |stem| stem.to_string_lossy().into_owned(),
                ),
                file: SceneFile::Binary(assets.scene_file(id).map_err(load)?),
            })),
            // Another kind than a scene's is refused as reading it as a glTF scene refuses it.
            _ => Ok(Some(Source::Cooked(assets.scene(id).map_err(load)?))),
        }
    }
}

/// Panics if a node's parent does not come before it.
fn check_order(scene: &CookedScene) {
    for (index, node) in scene.nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            assert!(
                (parent as usize) < index,
                "scene `{}`: node {index}'s parent comes after it",
                scene.name
            );
        }
    }
}

/// A scene resolved into what it spawns: its entities, each with its parent, its components and
/// the path of IDs that names it from the scene's root, its placements and their overrides applied
/// ([`SceneLibrary::resolve`], [`ResolvedScene::from_cooked`]). A world spawns it under a key
/// ([`World::spawn_scene`](crate::World::spawn_scene)), as many times as it needs. It is shared:
/// a clone costs a count.
#[derive(Clone, Debug)]
pub struct ResolvedScene(pub(crate) Arc<Resolved>);

/// What a resolved scene holds.
#[derive(Debug)]
pub(crate) struct Resolved {
    pub(crate) id: AssetId,
    pub(crate) name: String,
    /// The FNV-1a hash of what the scene was resolved from: a glTF scene's cooked bytes, or a scene
    /// file's binary encoding, cooked or made from its text, and the fingerprints of the prefabs that
    /// it places, in their order.
    pub(crate) fingerprint: u64,
    /// How many scenes deep its placements nest, itself included: 1 for a scene that places none.
    pub(crate) depth: usize,
    /// About the bytes that its records and its reports take ([`MAX_SCENE_SIZE`], [`size`]).
    pub(crate) size: usize,
    /// The root first; each entity's parent may come after it, as a scene file's may.
    pub(crate) records: Vec<Record>,
    pub(crate) set_aside: Vec<SetAside>,
    /// The scene file's table of assets, in its order; none for a glTF scene.
    pub(crate) assets: Vec<AssetName>,
}

/// About the bytes that `records` and `set_aside` take in memory, as [`MAX_SCENE_SIZE`] counts
/// them.
pub(crate) fn size(records: &[Record], set_aside: &[SetAside]) -> usize {
    let text = |text: &String| size_of::<String>() + text.len();
    let records: usize = records
        .iter()
        .map(|record| {
            size_of::<Record>()
                + record.path.iter().map(text).sum::<usize>()
                + record
                    .components
                    .iter()
                    .map(|(name, value)| text(name) + value_size(value))
                    .sum::<usize>()
        })
        .sum();
    let set_aside: usize = set_aside
        .iter()
        .map(|aside| {
            size_of::<SetAside>()
                + aside.entity.len()
                + aside.component.as_ref().map_or(0, String::len)
                + aside.reason.len()
        })
        .sum();
    records + set_aside
}

/// About the bytes that placing `scene` under the identity `under` takes, as [`MAX_SCENE_SIZE`]
/// counts them: the scene's records, and `under` again for each of them, which its path and the
/// key that names it repeat.
pub(crate) fn placed_size(scene: &ResolvedScene, under: &str) -> usize {
    let repeated = 2 * (size_of::<String>() + under.len());
    scene
        .0
        .size
        .saturating_add(scene.0.records.len().saturating_mul(repeated))
}

/// About the bytes that `value` takes in memory, what it holds included.
fn value_size(value: &Value) -> usize {
    let fields = |fields: &[(String, Value)]| -> usize {
        fields
            .iter()
            .map(|(name, value)| size_of::<String>() + name.len() + value_size(value))
            .sum()
    };
    size_of::<Value>()
        + match value {
            Value::Text(text) => text.len(),
            Value::Struct(inner) => fields(inner),
            Value::Enum(variant, inner) => variant.len() + fields(inner),
            Value::Option(Some(inner)) => value_size(inner),
            Value::List(elements) => elements.iter().map(value_size).sum(),
            _ => 0,
        }
}

/// An asset of a scene file's table: its short name, its ID, and where its source was.
#[derive(Clone, Debug)]
pub(crate) struct AssetName {
    pub(crate) name: String,
    pub(crate) id: AssetId,
    pub(crate) path: Option<String>,
}

/// An entity of a resolved scene.
#[derive(Clone, Debug)]
pub(crate) struct Record {
    /// The IDs that name it from the scene's root, the root's empty.
    pub(crate) path: Vec<String>,
    /// The index of its parent's record; `None` for the root's.
    pub(crate) parent: Option<u32>,
    /// Its components, by their types' names, each value checked; a reference to an entity holds the
    /// index of its record, or [`NO_RECORD`].
    pub(crate) components: Vec<(String, Value)>,
    /// What the scene placed here, if it is a placement's root.
    pub(crate) placement: Option<Arc<Placement>>,
}

/// What a world remembers of a placement, a prefab placed in a scene
/// (`docs/adr/0089-scenes-and-prefabs.md`): the prefab, the overrides that the scene that placed
/// it writes for it, and those of them that were set aside
/// ([`World::placement`](crate::World::placement)).
#[derive(Debug)]
pub struct Placement {
    pub(crate) prefab: ResolvedScene,
    pub(crate) overrides: Overrides,
    pub(crate) set_aside: Vec<SetAside>,
}

/// The overrides of a placement, as the scene that placed it writes them, those set aside
/// included: paths from the placement's root, the root's empty, and values with references to
/// entities as indices of the placing scene's records.
#[derive(Debug, Default)]
pub(crate) struct Overrides {
    pub(crate) removed: Vec<Vec<String>>,
    pub(crate) removed_components: Vec<(Vec<String>, Vec<String>)>,
    pub(crate) added: Vec<(Vec<String>, String, Value)>,
    pub(crate) changed: Vec<(Vec<String>, String, Value)>,
}

impl Placement {
    /// The prefab placed.
    pub fn prefab(&self) -> &ResolvedScene {
        &self.prefab
    }

    /// The overrides that the scene that placed it writes and that could not be applied, and why:
    /// their entities, by their paths from the scene that placed it, such as `lamp 1/glass`.
    pub fn set_aside(&self) -> &[SetAside] {
        &self.set_aside
    }

    /// How many overrides the scene that placed it writes for it, those set aside included.
    pub fn override_count(&self) -> usize {
        let Overrides {
            removed,
            removed_components,
            added,
            changed,
        } = &self.overrides;
        removed.len()
            + removed_components
                .iter()
                .map(|(_, names)| names.len())
                .sum::<usize>()
            + added.len()
            + changed.len()
    }
}

impl ResolvedScene {
    /// The scene of `scene`, a glTF scene as the cooker makes it, whose asset ID is `id`.
    ///
    /// Each node becomes an entity, child of its parent's, the scene's root nodes children of the
    /// root, in the scene's order. Each holds a [`Transform`] from its node's matrix, its [`Name`]
    /// if it has one, the [`Meshes`] and the [`Light`] it places, and its [`Extras`]; the root
    /// holds [`Transform::IDENTITY`]. Its ID in the scene is the node's name, or its index, `#12`,
    /// with a warning, when the name is missing, starts with `#` as indices do, holds `/`, which
    /// joins the parts of an identity, or another node has it too: an export that reorders the nodes
    /// keeps them, a node renamed loses its own. No two entities of a scene get the same ID.
    ///
    /// A node's matrix gives its transform as `docs/adr/0079-the-engines-own-world.md` says: three
    /// scales within 10⁻⁵ of the largest become its uniform scale; otherwise its stretch, which its
    /// children do not inherit. A node with children scaled unevenly or mirrored is logged with a
    /// warning, and its children are placed from where the file puts them in the world. A shear,
    /// which a node turned under a node scaled unevenly gets, is dropped: its rotation keeps the
    /// direction of its first axis that is not flattened, and the others are made square to it.
    ///
    /// A node's typed components, which its `slime:<type name>` properties give
    /// (`docs/adr/0089-scenes-and-prefabs.md`), are read through `registry`, a world's, strictly, as
    /// a scene file's components are.
    ///
    /// # Errors
    ///
    /// [`SceneError::Node`] if a typed component is of a type that `registry` does not hold, has a
    /// value that its type refuses, or is one that the node gives already, its transform, name,
    /// meshes, light or extras: the node, and the type and the field in the error's path.
    ///
    /// # Panics
    ///
    /// If a node's parent does not come before it, which a scene read from cooked data never has
    /// (`CookedScene::from_bytes` refuses it).
    pub fn from_cooked(
        id: AssetId,
        scene: &CookedScene,
        registry: &Registry,
    ) -> Result<Self, SceneError> {
        check_order(scene);
        let mut has_children = vec![false; scene.nodes.len()];
        for node in &scene.nodes {
            if let Some(parent) = node.parent {
                has_children[parent as usize] = true;
            }
        }
        let locals = local_ids(scene);
        let mut records = Vec::with_capacity(scene.nodes.len() + 1);
        records.push(Record {
            path: Vec::new(),
            parent: None,
            components: vec![("Transform".to_owned(), Transform::IDENTITY.to_value())],
            placement: None,
        });
        // Where the file puts each node in the scene, and where its entity is, which differ below
        // a node with children scaled unevenly.
        let mut exact: Vec<DAffine3> = Vec::with_capacity(scene.nodes.len());
        let mut placed: Vec<GlobalTransform> = Vec::with_capacity(scene.nodes.len());
        let mut flattened: Vec<bool> = Vec::with_capacity(scene.nodes.len());
        for (index, node) in scene.nodes.iter().enumerate() {
            let parent = node.parent.map(|parent| parent as usize);
            let local = DAffine3::from_mat4(Mat4::from_cols_array(&node.transform).as_dmat4());
            let parent_exact = parent.map_or(DAffine3::IDENTITY, |parent| exact[parent]);
            let parent_placed = parent.map_or(GlobalTransform::IDENTITY, |parent| placed[parent]);
            let below_flattened = parent.is_some_and(|parent| flattened[parent]);
            let world = parent_exact * local;
            let matrix = if below_flattened {
                parent_placed.to_affine(glam::Vec3::ONE).inverse() * world
            } else {
                local
            };
            let (transform, flattens) = split(matrix, has_children[index]);
            if flattens {
                tracing::warn!(
                    scene = %scene.name,
                    node = %node.name,
                    "a node with children is scaled unevenly or mirrored: its children are placed apart from it"
                );
            }
            exact.push(world);
            placed.push(parent_placed.mul_transform(&transform));
            flattened.push(flattens || below_flattened);

            let mut components = vec![("Transform".to_owned(), transform.to_value())];
            if !node.name.is_empty() {
                components.push(("Name".to_owned(), Name(node.name.clone()).to_value()));
            }
            if !node.meshes.is_empty() {
                let meshes = Meshes {
                    ids: node.meshes.clone(),
                    casts_shadows: true,
                };
                components.push(("Meshes".to_owned(), meshes.to_value()));
            }
            if let Some(light) = node.light {
                components.push(("Light".to_owned(), Light::from_cooked(&light).to_value()));
            }
            if !node.extras.is_empty() {
                components.push(("Extras".to_owned(), Extras(node.extras.clone()).to_value()));
            }
            if !node.components.is_empty() {
                typed(registry, &node.components, &mut components).map_err(|error| {
                    SceneError::Node {
                        scene: scene.name.clone(),
                        node: locals[index].clone(),
                        error,
                    }
                })?;
            }
            records.push(Record {
                path: vec![locals[index].clone()],
                parent: Some(node.parent.map_or(0, |parent| parent + 1)),
                components,
                placement: None,
            });
        }
        Ok(Self(Arc::new(Resolved {
            id,
            name: scene.name.clone(),
            fingerprint: fingerprint(0xcbf2_9ce4_8422_2325, &scene.to_bytes()),
            depth: 1,
            size: size(&records, &[]),
            records,
            set_aside: Vec::new(),
            assets: Vec::new(),
        })))
    }

    /// The scene's asset ID.
    pub fn id(&self) -> AssetId {
        self.0.id
    }

    /// The scene's name, for messages.
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// How many entities the scene spawns, its root included.
    pub fn len(&self) -> usize {
        self.0.records.len()
    }

    /// Whether the scene spawns no entity: never, as it spawns its root at least.
    pub fn is_empty(&self) -> bool {
        self.0.records.is_empty()
    }

    /// The overrides of the scene and of the prefabs that it places, at any depth, that could not
    /// be applied, and why: overrides whose entity or component the prefab no longer has, or that
    /// no longer fit it. Their entities are named by their paths from the scene's root, such as
    /// `lamp 1/glass`.
    pub fn set_aside(&self) -> &[SetAside] {
        &self.0.set_aside
    }
}

/// Reads `bytes`, a node's typed components in the binary encoding of a document whose keys are
/// their types' names, through `registry`, into `components`, by their types' names.
///
/// # Errors
///
/// A document that is not one, a type that `registry` does not hold, a value that its type refuses,
/// or a component that `components` holds already.
fn typed(
    registry: &Registry,
    bytes: &[u8],
    components: &mut Vec<(String, Value)>,
) -> Result<(), ReadError> {
    let mut document = Document::parse_binary(registry, bytes, Mode::Strict)?;
    for key in document.keys(&[]) {
        let Some(value) = document.read(&[&key], &key, None)? else {
            continue;
        };
        let name = registry.get(&key).map_or_else(
            || key.clone(),
            |registration| registration.info().name.clone(),
        );
        if components.iter().any(|(held, _)| *held == name) {
            return Err(document.error_at(
                &[&key],
                Problem::Invalid(format!("the node gives its `{name}` already")),
            ));
        }
        components.push((name, value));
    }
    Ok(())
}

/// Folds `bytes` into the FNV-1a hash `hash`, from `0xcbf2_9ce4_8422_2325`: a fingerprint of a
/// scene that a save keeps, the same on every platform and with every version of Rust.
pub(crate) fn fingerprint(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Each node's ID within its scene: its name, or `#<index>` with a warning when the name is
/// missing, starts with `#`, which would let it be taken for another node's index, holds `/`,
/// which separates the parts of an identity, or is repeated. No two nodes get the same ID.
fn local_ids(scene: &CookedScene) -> Vec<String> {
    let mut uses: HashMap<&str, usize> = HashMap::new();
    for node in &scene.nodes {
        *uses.entry(node.name.as_str()).or_default() += 1;
    }
    scene
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            if !node.name.is_empty()
                && !node.name.starts_with('#')
                && !node.name.contains('/')
                && uses[node.name.as_str()] == 1
            {
                node.name.clone()
            } else {
                tracing::warn!(
                    scene = %scene.name,
                    index,
                    name = %node.name,
                    "a node without a name of its own is known by its index, which a new export can change"
                );
                format!("#{index}")
            }
        })
        .collect()
}

/// Why a scene cannot be resolved.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SceneError {
    /// The library holds no scene of this ID: the scene asked for, or a prefab that a scene places.
    #[error("{}", match placed_by {
        Some(scene) => format!("scene `{scene}` places the scene {id}, which the library does not hold"),
        None => format!("the library holds no scene {id}"),
    })]
    Missing {
        /// The scene's asset ID.
        id: AssetId,
        /// The scene that places it, by its name, if a scene does.
        placed_by: Option<String>,
    },
    /// A scene file that says something wrong: a text that is not TOML or not a scene file's, an ID
    /// that an entity cannot have, a parent, an asset, a placement or a component that it does not
    /// have, a value that its type refuses; where, by its line and column in a text, by its byte in
    /// a cooked file's binary encoding, and the path of the part at fault, which names the entity,
    /// the component and the field.
    #[error("scene `{scene}`: {error}")]
    File {
        /// The scene, by its name.
        scene: String,
        /// What is wrong, and where.
        #[source]
        error: ReadError,
    },
    /// Scenes that place each other: their names, the first again at the end.
    #[error("scenes that place each other: {}", .0.join(" → "))]
    Cycle(Vec<String>),
    /// Placements nested deeper than [`MAX_NESTING`]: the names of the scenes, from the outermost, to
    /// the one too deep, or to a scene resolved before whose placements nest too deep where it is
    /// placed.
    #[error("placements nested deeper than {MAX_NESTING} scenes: {}", .0.join(" → "))]
    TooDeep(Vec<String>),
    /// A scene of more than [`MAX_ENTITIES`] entities, or that takes more than [`MAX_SCENE_SIZE`]
    /// bytes, its placements' counted.
    #[error(
        "scene `{scene}` holds more than {MAX_ENTITIES} entities, or takes more than \
         {MAX_SCENE_SIZE} bytes, its placements' counted"
    )]
    TooLarge {
        /// The scene, by its name.
        scene: String,
    },
    /// A typed component of a glTF scene's node that the registry does not hold, or whose value its
    /// type refuses (`docs/adr/0089-scenes-and-prefabs.md`): the scene, the node, and why, the type
    /// and the field in the error's path.
    #[error("scene `{scene}`, node `{node}`: {error}")]
    Node {
        /// The scene, by its name.
        scene: String,
        /// The node, by its name, or its index, `#12`, when it has none.
        node: String,
        /// What is wrong, and where.
        #[source]
        error: ReadError,
    },
    /// A scene that the cooked data that the library reads lists, but whose file cannot be read, or
    /// is not a scene's.
    #[error("the scene {id} cannot be read: {error}")]
    Load {
        /// The scene's asset ID.
        id: AssetId,
        /// Why.
        #[source]
        error: LoadError,
    },
}
