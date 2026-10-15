//! Cooked scenes on the GPU: a scene that the asset pipeline cooked, its meshes, materials and
//! textures uploaded on the renderer's threads, its nodes flattened into instances and its lights
//! placed in world space.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use glam::{Mat4, Vec3};
use slime_assets::{
    AssetId, CookedAssets, CookedMaterial, CookedScene, CookedTexture, LightKind, LoadError,
    SceneLight, TextureEncoding, TextureFilter, TextureSampler, TextureWrap,
};
use slime_gfx::{
    AddressMode, Filter, Gpu, LinearColor, Sampler, SamplerDescriptor, Texture, TextureFormat,
};

use crate::error::RenderError;
use crate::light::{DirectionalLight, Light, PointLight, SpotLight};
use crate::material::{AlphaMode, Material, MaterialData, MaterialParts, SampledTexture};
use crate::mesh::{Mesh, MeshData};
use crate::pool;
use crate::renderer::{Instance, Renderer};

/// Why a cooked scene cannot be loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SceneError {
    /// An asset of the scene cannot be read from the cooked data.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// The renderer cannot create one of its objects: the GPU's memory is full, or the device was
    /// lost.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// A node places a mesh or a light where nothing can be drawn: at a point that is not finite,
    /// or a light without a direction.
    #[error("scene `{scene}`, node {node}: {reason}")]
    Placement {
        /// The scene's name.
        scene: String,
        /// The node's index among the scene's nodes.
        node: usize,
        /// What is wrong.
        reason: String,
    },
}

/// A placed mesh: indices into the scene's meshes and materials, its transform, and the node that
/// places it.
struct Placement {
    mesh: usize,
    material: usize,
    transform: Mat4,
    node: usize,
}

/// A cooked scene on the GPU ([`Renderer::load_scene`]): its meshes and materials, which keep
/// their textures and samplers, where its nodes place them, and its lights in world space.
pub struct LoadedScene {
    meshes: Vec<Mesh>,
    materials: Vec<Material>,
    placements: Vec<Placement>,
    /// The scene as it was cooked: its nodes, their names, transforms and `extras`, which a game
    /// reads its own data from.
    pub scene: CookedScene,
    /// The lights its nodes place, in world space, in the order of the nodes.
    pub lights: Vec<Light>,
    /// The smallest box around every mesh placed, its lowest corner then its highest, in world
    /// space; `None` for a scene that places no mesh.
    pub bounds: Option<(Vec3, Vec3)>,
}

impl LoadedScene {
    /// The instances to draw: one per mesh that a node places, casting shadows.
    pub fn instances(&self) -> Vec<Instance<'_>> {
        self.placements
            .iter()
            .map(|placement| {
                Instance::new(
                    &self.meshes[placement.mesh],
                    &self.materials[placement.material],
                    placement.transform,
                )
            })
            .collect()
    }

    /// The node that places each of [`LoadedScene::instances`], in the same order: an index into
    /// `scene.nodes`, so that a game can leave out or move what some nodes place, such as a door
    /// that opens.
    pub fn instance_nodes(&self) -> Vec<usize> {
        self.placements
            .iter()
            .map(|placement| placement.node)
            .collect()
    }

    /// The material named `name` among those its meshes are drawn with, its name being the cooked
    /// material's (`slime_assets::CookedMaterial::name`): when several have it, the first that a mesh of
    /// the scene is drawn with, in the order of the scene's meshes; `None` when none has it. A game
    /// changes how the scene looks through it ([`Material::set_values`]), and keeps a clone of it
    /// to do so at each frame.
    pub fn material(&self, name: &str) -> Option<&Material> {
        self.materials
            .iter()
            .find(|material| &*material.label == name)
    }
}

/// A cooked scene being loaded in the background, on threads of the renderer's own
/// ([`Renderer::load_scene_in_background`]). A game polls it at each frame with
/// [`SceneLoad::take`], which gives the whole scene once it is loaded, never a part of it; or waits
/// for it with [`SceneLoad::wait`].
///
/// Dropping it cancels the load. The drop waits for the load's tasks that are running, a mesh or a
/// texture each, and drops what they made; the tasks that have not started then do nothing when
/// their turn comes, and never use the GPU. Once the drop returns, no thread of the load holds the
/// `Gpu`.
pub struct SceneLoad {
    /// The thread that runs the load; `None` once its result is taken.
    thread: Option<JoinHandle<Option<Result<LoadedScene, SceneError>>>>,
    /// Tells the load's thread, which waits for the reports of its tasks, that the load is dropped.
    reports: Sender<Report>,
}

impl SceneLoad {
    /// The scene once it is loaded, or why it could not be: `None` while the load goes on, and
    /// once its result was taken.
    ///
    /// # Errors
    ///
    /// As [`Renderer::load_scene`].
    ///
    /// # Panics
    ///
    /// With the panic of a loading thread, if one panicked, even when an asset also failed: a bug,
    /// as when a texture is larger than the device allows ([`Renderer::create_texture`]). Also if
    /// the operating system could not start the renderer's threads.
    pub fn take(&mut self) -> Option<Result<LoadedScene, SceneError>> {
        if !self.thread.as_ref()?.is_finished() {
            return None;
        }
        self.thread.take().map(finish)
    }

    /// Waits for the load to finish, and returns its scene, or why it could not be loaded.
    ///
    /// # Errors
    ///
    /// As [`Renderer::load_scene`].
    ///
    /// # Panics
    ///
    /// As [`SceneLoad::take`]; and if `take` already returned the load's result.
    pub fn wait(mut self) -> Result<LoadedScene, SceneError> {
        finish(self.thread.take().expect("the load's result is taken once"))
    }
}

impl Drop for SceneLoad {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            // The load's thread is gone if it has finished: it has nothing to cancel then.
            let _ = self.reports.send(Report::Dropped);
            // What the load gives, its panic included, is dropped with it.
            let _ = thread.join();
        }
    }
}

/// The result of the load that `thread` ran, or its panic, raised again.
fn finish(
    thread: JoinHandle<Option<Result<LoadedScene, SceneError>>>,
) -> Result<LoadedScene, SceneError> {
    match thread.join() {
        Ok(result) => result.expect("only a dropped load gives no result"),
        Err(panic) => panic::resume_unwind(panic),
    }
}

impl Renderer {
    /// Loads the cooked scene `id` from `assets` onto the GPU, as
    /// [`Renderer::load_scene_in_background`] does, and waits for it.
    ///
    /// # Errors
    ///
    /// [`SceneError::Load`] if an asset cannot be read; [`SceneError::Render`] if the GPU's memory
    /// cannot hold an object, the device is lost, the GPU cannot sample the cooked textures
    /// (`GfxError::UnsupportedTexture`), or a material has values out of their ranges
    /// ([`RenderError::InvalidMaterial`]); [`SceneError::Placement`] if a node places a mesh at a
    /// point that is not finite, which a transform that overflows gives, or a light at such a point
    /// or with no direction, as a transform that flattens its −Z axis gives. When several assets
    /// fail, the error is the first that a loading thread reports, which can change from one run to
    /// the next.
    ///
    /// # Panics
    ///
    /// As [`Renderer::load_scene_in_background`] and [`SceneLoad::take`].
    pub fn load_scene(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        id: AssetId,
    ) -> Result<LoadedScene, SceneError> {
        self.load_scene_in_background(gpu, assets, id).wait()
    }

    /// Starts loading the cooked scene `id` from `assets` onto the GPU, and returns at once: the
    /// returned [`SceneLoad`] gives the scene once it is loaded. The load reads every mesh its
    /// nodes place, every material those meshes are drawn with and every texture those materials
    /// read, each once, on the renderer's threads, which create their GPU objects through clones of
    /// `gpu` (ADR-0073); and it places its lights by their nodes in world space, at the node's
    /// origin, along its −Z axis whatever its scale. A glTF file's default scene has the file's ID.
    ///
    /// The renderer's threads are as many as the machine's cores but one, at least 1 and at most
    /// 4, shared by every load of the process; each load has a thread of its own besides, which
    /// reads the scene and its materials and puts the loaded scene together. Their uploads do not
    /// hold up the thread that submits frames (ADR-0074); a frame that draws the first shadows, or
    /// changes the shape of the frame otherwise, still costs more during a load than after it.
    ///
    /// # Panics
    ///
    /// If the operating system cannot start the load's thread.
    pub fn load_scene_in_background(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        id: AssetId,
    ) -> SceneLoad {
        let (sender, reports) = mpsc::channel();
        let loader = Loader {
            gpu: gpu.clone(),
            assets: Arc::new(assets.clone()),
            parts: self.material_parts.clone(),
            sender: sender.clone(),
        };
        let thread = thread::Builder::new()
            .name("slime-render scene load".to_owned())
            .spawn(move || loader.load(id, &reports))
            .expect("the operating system starts a thread");
        SceneLoad {
            thread: Some(thread),
            reports: sender,
        }
    }
}

/// What the thread of a load works with.
struct Loader {
    gpu: Gpu,
    assets: Arc<CookedAssets>,
    parts: MaterialParts,
    /// Where the load's tasks report.
    sender: Sender<Report>,
}

/// A mesh that a task of a load made.
struct MadeMesh {
    mesh: Mesh,
    /// The material it is drawn with.
    material: AssetId,
    /// Its lowest corner, then its highest.
    bounds: [[f32; 3]; 2],
}

/// What a task of a load made.
enum Made {
    /// The mesh at this index of the scene's meshes.
    Mesh(usize, MadeMesh),
    /// The texture of this ID.
    Texture(AssetId, Texture),
}

/// What the thread of a load hears from its tasks and from its [`SceneLoad`].
enum Report {
    /// A task ran: what it made, or the error that stopped it, or its panic.
    Ran(thread::Result<Result<Made, SceneError>>),
    /// A task's turn came after the load stopped: it did nothing.
    Skipped,
    /// The `SceneLoad` was dropped.
    Dropped,
}

/// What the tasks of a load work with, shared by them: emptied when the load stops, so that the
/// tasks that start after that do nothing, and never use the GPU.
struct Work {
    /// The GPU and the cooked data; `None` once the load has stopped.
    shared: Option<(Gpu, Arc<CookedAssets>)>,
    /// How many tasks took them.
    started: usize,
}

/// The tasks of a load on the pool.
struct Tasks {
    work: Arc<Mutex<Work>>,
    sender: Sender<Report>,
    /// How many tasks have not reported.
    pending: usize,
    /// How many tasks that ran have reported.
    ran: usize,
}

impl Tasks {
    /// Runs `task` on the pool with the load's GPU and cooked data, unless the load has stopped
    /// when its turn comes; it reports to the load's thread. The task drops its clones of the GPU
    /// and of the cooked data before it reports, so that none outlives the load.
    fn spawn(
        &mut self,
        task: impl FnOnce(&Gpu, &CookedAssets) -> Result<Made, SceneError> + Send + 'static,
    ) {
        self.pending += 1;
        let work = Arc::clone(&self.work);
        let sender = self.sender.clone();
        pool::run(move || {
            let shared = {
                let mut work = work
                    .lock()
                    .expect("no thread panics while it holds a load's work");
                let shared = work.shared.clone();
                work.started += usize::from(shared.is_some());
                shared
            };
            let report = match shared {
                Some((gpu, assets)) => {
                    Report::Ran(panic::catch_unwind(AssertUnwindSafe(move || {
                        task(&gpu, &assets)
                    })))
                }
                None => Report::Skipped,
            };
            // The load's thread waits for the report of every task that ran; a task skipped after
            // the load ended finds it gone.
            let _ = sender.send(report);
        });
    }

    /// Stops the load: the tasks that start from now on do nothing. Returns how many started
    /// before, whose reports the load's thread waits for.
    fn stop(&self) -> usize {
        let mut work = self
            .work
            .lock()
            .expect("no thread panics while it holds a load's work");
        work.shared = None;
        work.started
    }
}

impl Drop for Tasks {
    /// Stops the load when its thread ends, a panic of its own included, so that no task left in
    /// the pool's queue uses the GPU after it.
    fn drop(&mut self) {
        self.stop();
    }
}

impl Loader {
    /// Loads the scene `id`, hearing from its tasks and its [`SceneLoad`] through `reports`:
    /// `None` if the `SceneLoad` was dropped.
    fn load(
        &self,
        id: AssetId,
        reports: &Receiver<Report>,
    ) -> Option<Result<LoadedScene, SceneError>> {
        let scene = match self.assets.scene(id) {
            Ok(scene) => scene,
            Err(error) => return Some(Err(error.into())),
        };
        // No task has started: the only report there can be is the drop of the `SceneLoad`.
        if reports.try_recv().is_ok() {
            return None;
        }
        // The meshes, each once, in the order in which the nodes first place them.
        let mut mesh_index: HashMap<AssetId, usize> = HashMap::new();
        let mut mesh_ids = Vec::new();
        for &mesh in scene.nodes.iter().flat_map(|node| &node.meshes) {
            if let Entry::Vacant(entry) = mesh_index.entry(mesh) {
                entry.insert(mesh_ids.len());
                mesh_ids.push(mesh);
            }
        }
        let mut tasks = Tasks {
            work: Arc::new(Mutex::new(Work {
                shared: Some((self.gpu.clone(), Arc::clone(&self.assets))),
                started: 0,
            })),
            sender: self.sender.clone(),
            pending: 0,
            ran: 0,
        };
        for (index, &id) in mesh_ids.iter().enumerate() {
            tasks.spawn(move |gpu, assets| {
                let cooked = assets.mesh(id)?;
                let data = MeshData {
                    positions: cooked.positions,
                    normals: cooked.normals,
                    uvs: cooked.uvs,
                    tangents: cooked.tangents,
                    indices: cooked.indices,
                };
                let mesh = Mesh::new(gpu, &cooked.name, &data)?;
                let material = cooked.material;
                let bounds = cooked.bounds;
                Ok(Made::Mesh(
                    index,
                    MadeMesh {
                        mesh,
                        material,
                        bounds,
                    },
                ))
            });
        }
        // A mesh names its material, which is read here, small, and names its textures, which
        // tasks upload: they start as soon as the first mesh that needs them is made.
        let mut meshes: Vec<Option<MadeMesh>> = mesh_ids.iter().map(|_| None).collect();
        let mut materials: HashMap<AssetId, CookedMaterial> = HashMap::new();
        let mut textures: HashMap<AssetId, Option<Texture>> = HashMap::new();
        let mut failure = None;
        let mut panicked = None;
        let mut dropped = false;
        // Once the load stops, how many tasks had started: their reports are waited for.
        let mut stopped: Option<usize> = None;
        while match stopped {
            Some(started) => tasks.ran < started,
            None => tasks.pending > 0,
        } {
            let ran = match reports
                .recv()
                .expect("the load's thread keeps a sender of its reports")
            {
                Report::Dropped => {
                    dropped = true;
                    stopped.get_or_insert_with(|| tasks.stop());
                    continue;
                }
                Report::Skipped => {
                    tasks.pending -= 1;
                    continue;
                }
                Report::Ran(ran) => {
                    tasks.pending -= 1;
                    tasks.ran += 1;
                    ran
                }
            };
            let made = match ran {
                Err(panic) => {
                    panicked.get_or_insert(panic);
                    stopped.get_or_insert_with(|| tasks.stop());
                    continue;
                }
                Ok(Err(error)) => {
                    failure.get_or_insert(error);
                    stopped.get_or_insert_with(|| tasks.stop());
                    continue;
                }
                // What a task made after the load stopped is dropped with it.
                Ok(Ok(_)) if stopped.is_some() => continue,
                Ok(Ok(made)) => made,
            };
            let (index, mesh) = match made {
                Made::Texture(id, texture) => {
                    textures.insert(id, Some(texture));
                    continue;
                }
                Made::Mesh(index, mesh) => (index, mesh),
            };
            let material = mesh.material;
            meshes[index] = Some(mesh);
            if materials.contains_key(&material) {
                continue;
            }
            match self.assets.material(material) {
                Ok(cooked) => {
                    for slot in cooked.textures().into_iter().flatten() {
                        let id = slot.texture;
                        if let Entry::Vacant(entry) = textures.entry(id) {
                            entry.insert(None);
                            // Named after its source, whichever material asks for it first.
                            let label = match self.assets.manifest().get(id) {
                                Some(entry) => format!("texture {id} from {}", entry.source),
                                None => format!("texture {id}"),
                            };
                            tasks.spawn(move |gpu, assets| {
                                let texture = upload_texture(gpu, &label, &assets.texture(id)?)?;
                                Ok(Made::Texture(id, texture))
                            });
                        }
                    }
                    materials.insert(material, cooked);
                }
                Err(error) => {
                    failure.get_or_insert(error.into());
                    stopped.get_or_insert_with(|| tasks.stop());
                }
            }
        }
        if let Some(panic) = panicked {
            panic::resume_unwind(panic);
        }
        if dropped {
            return None;
        }
        if let Some(error) = failure {
            return Some(Err(error));
        }
        let meshes = meshes
            .into_iter()
            .map(|mesh| mesh.expect("every mesh is made when nothing failed"))
            .collect();
        let textures = textures
            .into_iter()
            .map(|(id, texture)| {
                (
                    id,
                    texture.expect("every texture is made when nothing failed"),
                )
            })
            .collect();
        Some(self.assemble(scene, &mesh_index, meshes, &materials, &textures))
    }

    /// Puts the loaded scene together from what the tasks made: its materials, with the samplers
    /// they need, and where its nodes place its meshes and lights.
    fn assemble(
        &self,
        scene: CookedScene,
        mesh_index: &HashMap<AssetId, usize>,
        meshes: Vec<MadeMesh>,
        materials: &HashMap<AssetId, CookedMaterial>,
        textures: &HashMap<AssetId, Texture>,
    ) -> Result<LoadedScene, SceneError> {
        let mut samplers: HashMap<TextureSampler, Sampler> = HashMap::new();
        let mut material_of: HashMap<AssetId, usize> = HashMap::new();
        let mut loaded = LoadedScene {
            meshes: Vec::with_capacity(meshes.len()),
            materials: Vec::new(),
            placements: Vec::new(),
            scene: CookedScene::default(),
            lights: Vec::new(),
            bounds: None,
        };
        // The material and bounds of each mesh, in the order of `loaded.meshes`.
        let mut drawn = Vec::with_capacity(meshes.len());
        for MadeMesh {
            mesh,
            material,
            bounds,
        } in meshes
        {
            let index = match material_of.entry(material) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let made =
                        self.create_material(&materials[&material], textures, &mut samplers)?;
                    loaded.materials.push(made);
                    *entry.insert(loaded.materials.len() - 1)
                }
            };
            loaded.meshes.push(mesh);
            drawn.push((index, bounds));
        }
        let world = scene.world_transforms();
        let placement = |node: usize, reason: String| SceneError::Placement {
            scene: scene.name.clone(),
            node,
            reason,
        };
        for (index, (node, transform)) in scene.nodes.iter().zip(&world).enumerate() {
            let transform = Mat4::from_cols_array(transform);
            for mesh in &node.meshes {
                let mesh = mesh_index[mesh];
                let (material, bounds) = drawn[mesh];
                loaded.bounds = grow(loaded.bounds, transform, bounds)
                    .map_err(|reason| placement(index, reason))?;
                loaded.placements.push(Placement {
                    mesh,
                    material,
                    transform,
                    node: index,
                });
            }
        }
        loaded.lights = scene_lights(&scene)?;
        tracing::info!(
            scene = %scene.name,
            meshes = loaded.meshes.len(),
            materials = loaded.materials.len(),
            textures = textures.len(),
            instances = loaded.placements.len(),
            lights = loaded.lights.len(),
            "cooked scene loaded"
        );
        loaded.scene = scene;
        Ok(loaded)
    }

    /// Creates the material that `cooked` describes, reading the textures it names from `textures`
    /// and creating the samplers that are not in `samplers`.
    fn create_material(
        &self,
        cooked: &CookedMaterial,
        textures: &HashMap<AssetId, Texture>,
        samplers: &mut HashMap<TextureSampler, Sampler>,
    ) -> Result<Material, SceneError> {
        for slot in cooked.textures().into_iter().flatten() {
            if let Entry::Vacant(entry) = samplers.entry(slot.sampler) {
                let sampler = self
                    .gpu
                    .create_sampler(&cooked.name, &sampler_descriptor(slot.sampler))
                    .map_err(RenderError::from)?;
                entry.insert(sampler);
            }
        }
        let sampled = |slot: Option<slime_assets::MaterialTexture>| {
            slot.map(|slot| SampledTexture {
                texture: &textures[&slot.texture],
                sampler: &samplers[&slot.sampler],
            })
        };
        let [r, g, b, a] = cooked.base_color;
        let [er, eg, eb] = cooked.emissive;
        let data = MaterialData {
            base_color: LinearColor::rgba(r, g, b, a),
            base_color_texture: sampled(cooked.base_color_texture),
            metallic: cooked.metallic,
            roughness: cooked.roughness,
            metallic_roughness_texture: sampled(cooked.metallic_roughness_texture),
            normal_texture: sampled(cooked.normal_texture),
            normal_scale: cooked.normal_scale,
            occlusion_texture: sampled(cooked.occlusion_texture),
            occlusion_strength: cooked.occlusion_strength,
            emissive: LinearColor::rgb(er, eg, eb),
            emissive_strength: cooked.emissive_strength,
            emissive_texture: sampled(cooked.emissive_texture),
            alpha_mode: match cooked.alpha_cutoff {
                Some(cutoff) => AlphaMode::Mask { cutoff },
                None => AlphaMode::Opaque,
            },
            double_sided: cooked.double_sided,
        };
        Ok(self.parts.create(&self.gpu, &cooked.name, &data)?)
    }
}

/// The lights that the nodes of the cooked scene `scene` place, in world space, in the order of the
/// nodes: each at its node's origin, along its node's −Z axis whatever its scale, as
/// [`LoadedScene::lights`] holds them. It reads no file and creates nothing on the GPU, so that a
/// game knows a scene's lights before the scene is loaded: to compute a sky around its sun, for
/// example.
///
/// # Errors
///
/// [`SceneError::Placement`] for a light at a point that is not finite, which a transform that
/// overflows gives, or with no direction, as a transform that flattens its −Z axis gives.
pub fn scene_lights(scene: &CookedScene) -> Result<Vec<Light>, SceneError> {
    scene
        .nodes
        .iter()
        .zip(scene.world_transforms())
        .enumerate()
        .filter_map(|(index, (node, transform))| Some((index, node.light?, transform)))
        .map(|(index, light, transform)| {
            place_light(light, Mat4::from_cols_array(&transform)).map_err(|reason| {
                SceneError::Placement {
                    scene: scene.name.clone(),
                    node: index,
                    reason,
                }
            })
        })
        .collect()
}

/// The light that a node placed by `transform` holds, in world space: at the node's origin, along
/// its −Z axis whatever its scale; refused, with the reason, at a point that is not finite or
/// without a direction.
fn place_light(light: SceneLight, transform: Mat4) -> Result<Light, String> {
    let position = transform.transform_point3(Vec3::ZERO);
    if !position.is_finite() {
        return Err(format!(
            "its light is placed at {position}, which is not finite"
        ));
    }
    // Scale changes the length of the axis, not the direction it gives.
    let direction = transform
        .transform_vector3(Vec3::NEG_Z)
        .try_normalize()
        .ok_or_else(|| {
            "its light has no direction: the node's transform flattens its −Z axis or is not \
             finite"
                .to_owned()
        })?;
    let [r, g, b] = light.color;
    let color = LinearColor::rgb(r, g, b);
    Ok(match light.kind {
        LightKind::Directional => Light::Directional(DirectionalLight {
            direction,
            color,
            illuminance: light.intensity,
            ..DirectionalLight::default()
        }),
        LightKind::Point => Light::Point(PointLight {
            position,
            color,
            intensity: light.intensity,
            range: light.range,
        }),
        LightKind::Spot {
            inner_cone_angle,
            outer_cone_angle,
        } => Light::Spot(SpotLight {
            position,
            direction,
            color,
            intensity: light.intensity,
            range: light.range,
            inner_cone_angle,
            outer_cone_angle,
            casts_shadows: true,
        }),
    })
}

/// Uploads a cooked texture, as [`Renderer::create_texture`] says.
pub(crate) fn upload_texture(
    gpu: &Gpu,
    label: &str,
    data: &CookedTexture,
) -> Result<Texture, RenderError> {
    let format = match data.encoding {
        TextureEncoding::Bc7Srgb => TextureFormat::Bc7RgbaUnormSrgb,
        TextureEncoding::Bc7 => TextureFormat::Bc7RgbaUnorm,
        TextureEncoding::Bc5 => TextureFormat::Bc5RgUnorm,
        TextureEncoding::Rgba8Srgb => TextureFormat::Rgba8UnormSrgb,
        TextureEncoding::Rgba8 => TextureFormat::Rgba8Unorm,
    };
    let levels: Vec<&[u8]> = data.levels.iter().map(Vec::as_slice).collect();
    Ok(gpu.create_texture_with_levels(label, data.width, data.height, format, &levels)?)
}

/// The GPU layer's sampler for a cooked texture slot's.
fn sampler_descriptor(sampler: TextureSampler) -> SamplerDescriptor {
    let filter = |filter| match filter {
        TextureFilter::Nearest => Filter::Nearest,
        TextureFilter::Linear => Filter::Linear,
    };
    let address = |wrap| match wrap {
        TextureWrap::Repeat => AddressMode::Repeat,
        TextureWrap::MirroredRepeat => AddressMode::MirrorRepeat,
        TextureWrap::ClampToEdge => AddressMode::ClampToEdge,
    };
    SamplerDescriptor {
        mag_filter: filter(sampler.mag_filter),
        min_filter: filter(sampler.min_filter),
        address_u: address(sampler.wrap_u),
        address_v: address(sampler.wrap_v),
    }
}

/// `bounds` grown by the box `mesh`, its lowest and highest corners, placed by `transform`; refused
/// if a corner lands at a point that is not finite, which would spoil the box.
fn grow(
    bounds: Option<(Vec3, Vec3)>,
    transform: Mat4,
    mesh: [[f32; 3]; 2],
) -> Result<Option<(Vec3, Vec3)>, String> {
    let [low, high] = mesh.map(Vec3::from);
    let mut bounds = bounds;
    for corner in 0..8 {
        let pick = |bit: usize, low: f32, high: f32| if corner & bit == 0 { low } else { high };
        let point = transform.transform_point3(Vec3::new(
            pick(1, low.x, high.x),
            pick(2, low.y, high.y),
            pick(4, low.z, high.z),
        ));
        if !point.is_finite() {
            return Err(format!(
                "it places a mesh at {point}: its transform overflows"
            ));
        }
        bounds = Some(match bounds {
            Some((min, max)) => (min.min(point), max.max(point)),
            None => (point, point),
        });
    }
    Ok(bounds)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Barrier;
    use std::time::Duration;

    use slime_assets::{AssetKind, CookedMesh, MANIFEST_FILE, Manifest, ManifestEntry, SceneNode};
    use slime_gfx::TextureFormat;

    use super::*;

    /// A directory of cooked data, in the system's temporary directory, holding a scene of
    /// `count` squares, each a mesh of its own, all drawn with one material without textures.
    /// Returns the cooked data and the scene's ID.
    fn squares(name: &str, count: u128) -> (CookedAssets, AssetId) {
        let directory = std::env::temp_dir()
            .join(format!("slime-render-{}", std::process::id()))
            .join(name);
        if directory.exists() {
            fs::remove_dir_all(&directory).expect("the old directory can be removed");
        }
        fs::create_dir_all(&directory).expect("the directory can be created");
        let mut manifest = Manifest::new();
        let mut write = |id: AssetId, kind, file: String, bytes: Vec<u8>, needs| {
            fs::write(directory.join(&file), &bytes).expect("written");
            let size = bytes.len() as u64;
            let source = "squares.gltf".to_owned();
            manifest.insert(
                id,
                ManifestEntry {
                    kind,
                    file,
                    size,
                    needs,
                    source,
                },
            );
        };
        let material = AssetId::from_u128(1);
        let scene = AssetId::from_u128(2);
        write(
            material,
            AssetKind::Material,
            "square.material".to_owned(),
            CookedMaterial::default().to_bytes(),
            Vec::new(),
        );
        let square = MeshData::square();
        let mut nodes = Vec::new();
        for index in 0..count {
            let id = AssetId::from_u128(100 + index);
            let mesh = CookedMesh {
                name: format!("square {index}"),
                material,
                positions: square.positions.clone(),
                normals: square.normals.clone(),
                uvs: square.uvs.clone(),
                tangents: square.tangents.clone(),
                indices: square.indices.clone(),
                bounds: [[-0.5, -0.5, 0.0], [0.5, 0.5, 0.0]],
            };
            write(
                id,
                AssetKind::Mesh,
                format!("{index}.mesh"),
                mesh.to_bytes(),
                vec![material],
            );
            nodes.push(SceneNode {
                name: format!("square {index}"),
                parent: None,
                transform: Mat4::IDENTITY.to_cols_array(),
                meshes: vec![id],
                light: None,
                extras: Vec::new(),
            });
        }
        let cooked = CookedScene {
            name: "squares".to_owned(),
            nodes,
        };
        write(
            scene,
            AssetKind::Scene,
            "squares.scene".to_owned(),
            cooked.to_bytes(),
            Vec::new(),
        );
        fs::write(directory.join(MANIFEST_FILE), manifest.to_bytes()).expect("written");
        let assets = CookedAssets::open(&directory).expect("the manifest is read");
        (assets, scene)
    }

    #[test]
    fn a_load_dropped_while_its_tasks_wait_in_the_pool_returns_at_once() {
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        let (assets, scene) = squares("dropped", 8);
        // Every thread of the pool waits for the test, so that the load's tasks wait in its queue.
        let threads = pool::threads();
        let barrier = Arc::new(Barrier::new(threads + 1));
        let (busy, all_busy) = mpsc::channel();
        for _ in 0..threads {
            let barrier = Arc::clone(&barrier);
            let busy = busy.clone();
            pool::run(move || {
                busy.send(()).expect("the test waits");
                barrier.wait();
            });
        }
        for _ in 0..threads {
            all_busy.recv().expect("every thread of the pool is busy");
        }
        let load = renderer.load_scene_in_background(&gpu, &assets, scene);
        let (dropped, done) = mpsc::channel();
        thread::spawn(move || {
            drop(load);
            dropped.send(()).expect("the test waits");
        });
        let returned = done.recv_timeout(Duration::from_secs(10)).is_ok();
        barrier.wait();
        assert!(returned, "the drop waited for tasks that had not started");

        // The pool goes on: the tasks of the dropped load skip, and another load runs.
        let loaded = renderer
            .load_scene(&gpu, &assets, scene)
            .expect("the scene loads");
        assert_eq!(loaded.instances().len(), 8);
        drop(assets);
        let _ = fs::remove_dir_all(
            std::env::temp_dir().join(format!("slime-render-{}", std::process::id())),
        );
    }
}
