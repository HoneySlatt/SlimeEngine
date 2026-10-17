//! Scenes ready to spawn: what a scene spawns, its placements and their overrides applied, and the
//! library that resolves scene files and the glTF scenes that they place
//! (`docs/adr/0089-scenes-and-prefabs.md`).

use std::collections::HashMap;
use std::sync::Arc;

use glam::{DAffine3, Mat4};
use slime_assets::{AssetId, CookedScene};
use slime_reflect::{Reflect, Registry, Value};
use slime_serialize::ReadError;

use crate::components::{Extras, Light, Meshes, Name};
use crate::document::SetAside;
use crate::scene_file;
use crate::transform::{GlobalTransform, Transform, split};

/// How deep placements nest, prefabs placed in prefabs placed in a scene: 16 scenes from the one
/// resolved, itself included.
pub const MAX_NESTING: usize = 16;

/// What a template's reference to an entity holds for no entity: `""` in a scene file.
pub(crate) const NO_RECORD: u64 = u64::MAX;

/// The scenes that a game has, by their asset IDs: glTF scenes as the cooker makes them, and scene
/// files, each resolved once into what it spawns ([`SceneLibrary::resolve`]), the prefabs it places
/// resolved with it (`docs/adr/0089-scenes-and-prefabs.md`).
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
    resolved: HashMap<AssetId, Arc<ResolvedScene>>,
}

/// A scene as a game gives it to a library.
enum Source {
    /// A glTF scene, as the cooker makes it.
    Cooked(CookedScene),
    /// A scene file's text, and the scene's name, for messages.
    File { name: String, text: String },
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
                text: text.into(),
            },
        );
    }

    /// The scene `id` resolved into what it spawns, with the types of `registry`, a world's
    /// ([`World::registry`](crate::World::registry)): a glTF scene as
    /// [`ResolvedScene::from_cooked`] makes it; a scene file read strictly, the prefabs that it
    /// places resolved, at most [`MAX_NESTING`] deep, and its overrides applied. A scene is resolved
    /// once, then shared, until a scene is inserted again.
    ///
    /// # Errors
    ///
    /// [`SceneError::Missing`] for a scene that the library does not hold; [`SceneError::File`]
    /// for a scene file that says something wrong, where; [`SceneError::Cycle`] for prefabs that
    /// place each other; [`SceneError::TooDeep`] for placements nested deeper than
    /// [`MAX_NESTING`].
    pub fn resolve(
        &mut self,
        registry: &Registry,
        id: AssetId,
    ) -> Result<Arc<ResolvedScene>, SceneError> {
        self.resolve_within(registry, id, &mut Vec::new())
    }

    /// [`SceneLibrary::resolve`], within the scenes being resolved, `chain`, from the outermost.
    pub(crate) fn resolve_within(
        &mut self,
        registry: &Registry,
        id: AssetId,
        chain: &mut Vec<(AssetId, String)>,
    ) -> Result<Arc<ResolvedScene>, SceneError> {
        if let Some(resolved) = self.resolved.get(&id) {
            // A scene resolved alone first may nest too deep where another places it.
            if chain.len() + resolved.depth > MAX_NESTING {
                let mut names: Vec<String> = chain.iter().map(|(_, name)| name.clone()).collect();
                names.push(resolved.name.clone());
                return Err(SceneError::TooDeep(names));
            }
            return Ok(Arc::clone(resolved));
        }
        let (name, text) = match self.sources.get(&id) {
            None => {
                return Err(SceneError::Missing {
                    id,
                    placed_by: chain.last().map(|(_, name)| name.clone()),
                });
            }
            Some(Source::Cooked(scene)) => {
                let resolved = Arc::new(ResolvedScene::from_cooked(id, scene));
                self.resolved.insert(id, Arc::clone(&resolved));
                return Ok(resolved);
            }
            Some(Source::File { name, text }) => (name.clone(), text.clone()),
        };
        let names = |chain: &[(AssetId, String)], last: &str| {
            chain
                .iter()
                .map(|(_, name)| name.clone())
                .chain(std::iter::once(last.to_owned()))
                .collect()
        };
        if let Some(start) = chain.iter().position(|&(held, _)| held == id) {
            return Err(SceneError::Cycle(names(&chain[start..], &name)));
        }
        if chain.len() == MAX_NESTING {
            return Err(SceneError::TooDeep(names(chain, &name)));
        }
        chain.push((id, name.clone()));
        let resolved = scene_file::resolve(self, registry, id, &name, &text, chain);
        chain.pop();
        let resolved = Arc::new(resolved?);
        self.resolved.insert(id, Arc::clone(&resolved));
        Ok(resolved)
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
/// ([`World::spawn_scene`](crate::World::spawn_scene)), as many times as it needs.
#[derive(Debug)]
pub struct ResolvedScene {
    pub(crate) id: AssetId,
    pub(crate) name: String,
    /// The FNV-1a hash of what the scene was resolved from: its cooked bytes, or its text and the
    /// fingerprints of the prefabs that it places, in their order.
    pub(crate) fingerprint: u64,
    /// How many scenes deep its placements nest, itself included: 1 for a scene that places none.
    pub(crate) depth: usize,
    /// The root first; each entity's parent may come after it, as a scene file's may.
    pub(crate) records: Vec<Record>,
    pub(crate) set_aside: Vec<SetAside>,
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
    pub(crate) prefab: Arc<ResolvedScene>,
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
    pub fn overrides(&self) -> usize {
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
    /// # Panics
    ///
    /// If a node's parent does not come before it, which a scene read from cooked data never has
    /// (`CookedScene::from_bytes` refuses it).
    pub fn from_cooked(id: AssetId, scene: &CookedScene) -> Self {
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
            records.push(Record {
                path: vec![locals[index].clone()],
                parent: Some(node.parent.map_or(0, |parent| parent + 1)),
                components,
                placement: None,
            });
        }
        Self {
            id,
            name: scene.name.clone(),
            fingerprint: fingerprint(0xcbf2_9ce4_8422_2325, &scene.to_bytes()),
            depth: 1,
            records,
            set_aside: Vec::new(),
        }
    }

    /// The scene's asset ID.
    pub fn id(&self) -> AssetId {
        self.id
    }

    /// The scene's name, for messages.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How many entities the scene spawns, its root included.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the scene spawns no entity: never, as it spawns its root at least.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The overrides of the scene and of the prefabs that it places, at any depth, that could not
    /// be applied, and why: overrides whose entity or component the prefab no longer has, or that
    /// no longer fit it. Their entities are named by their paths from the scene's root, such as
    /// `lamp 1/glass`.
    pub fn set_aside(&self) -> &[SetAside] {
        &self.set_aside
    }
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
    /// have, a value that its type refuses; where, by its line and column, and the path of the part
    /// at fault.
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
    /// Placements nested deeper than [`MAX_NESTING`]: the names of the scenes, from the outermost.
    #[error("placements nested deeper than {MAX_NESTING} scenes: {}", .0.join(" → "))]
    TooDeep(Vec<String>),
}
