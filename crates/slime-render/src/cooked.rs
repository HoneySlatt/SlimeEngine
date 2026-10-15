//! Cooked scenes on the GPU: a scene that the asset pipeline cooked, its meshes, materials and
//! textures uploaded on the renderer's threads, its nodes flattened into instances and its lights
//! placed in world space.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
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
}

/// A cooked scene being loaded in the background, on threads of the renderer's own
/// ([`Renderer::load_scene_in_background`]). A game polls it at each frame with
/// [`SceneLoad::take`], which gives the whole scene once it is loaded, never a part of it; or waits
/// for it with [`SceneLoad::wait`].
///
/// Dropping it cancels the load: the drop skips the tasks that have not started, waits for those
/// running, a texture or a mesh each, and drops what they made.
pub struct SceneLoad {
    /// The thread that runs the load; `None` once its result is taken.
    thread: Option<JoinHandle<Option<Result<LoadedScene, SceneError>>>>,
    /// Set to cancel the load.
    cancel: Arc<AtomicBool>,
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
    /// With the panic of a loading thread, if one panicked: a bug, as when a texture is larger
    /// than the device allows ([`Renderer::create_texture`]).
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
            self.cancel.store(true, Ordering::Relaxed);
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
        Ok(result) => result.expect("only a dropped load is cancelled"),
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
    /// cannot hold an object, the device is lost, or the GPU cannot sample the cooked textures
    /// (`GfxError::UnsupportedTexture`); [`SceneError::Placement`] if a node places a mesh at a
    /// point that is not finite, which a transform that overflows gives, or a light at such a point
    /// or with no direction, as a transform that flattens its −Z axis gives. When several assets
    /// fail, the error is the first that a loading thread reports.
    ///
    /// # Panics
    ///
    /// As [`SceneLoad::take`].
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
    /// reads the scene and its materials and puts the loaded scene together. While the threads
    /// upload textures, the thread that submits frames can wait for them, by about the time each
    /// texture takes to copy: wgpu's queue takes the same lock for both.
    ///
    /// # Panics
    ///
    /// If the operating system cannot start a thread.
    pub fn load_scene_in_background(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        id: AssetId,
    ) -> SceneLoad {
        let cancel = Arc::new(AtomicBool::new(false));
        let loader = Loader {
            gpu: gpu.clone(),
            assets: Arc::new(assets.clone()),
            parts: self.material_parts.clone(),
            cancel: Arc::clone(&cancel),
        };
        let thread = thread::Builder::new()
            .name("slime-render scene load".to_owned())
            .spawn(move || loader.load(id))
            .expect("the operating system starts a thread");
        SceneLoad {
            thread: Some(thread),
            cancel,
        }
    }
}

/// What the thread of a load works with.
struct Loader {
    gpu: Gpu,
    assets: Arc<CookedAssets>,
    parts: MaterialParts,
    cancel: Arc<AtomicBool>,
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

/// What a task reports: `None` if the load was cancelled before it started; otherwise what it
/// made or the error that stopped it, or its panic.
type Report = Option<thread::Result<Result<Made, SceneError>>>;

/// The tasks of a load on the pool, and their reports.
struct Tasks {
    sender: Sender<Report>,
    reports: Receiver<Report>,
    /// How many tasks have not reported yet.
    pending: usize,
}

impl Loader {
    /// Loads the scene `id`: `None` if the load was cancelled from outside, by a drop.
    fn load(&self, id: AssetId) -> Option<Result<LoadedScene, SceneError>> {
        let scene = match self.assets.scene(id) {
            Ok(scene) => scene,
            Err(error) => return Some(Err(error.into())),
        };
        // The meshes, each once, in the order in which the nodes first place them.
        let mut mesh_index: HashMap<AssetId, usize> = HashMap::new();
        let mut mesh_ids = Vec::new();
        for &mesh in scene.nodes.iter().flat_map(|node| &node.meshes) {
            if let Entry::Vacant(entry) = mesh_index.entry(mesh) {
                entry.insert(mesh_ids.len());
                mesh_ids.push(mesh);
            }
        }
        let (sender, reports) = mpsc::channel();
        let mut tasks = Tasks {
            sender,
            reports,
            pending: 0,
        };
        for (index, &id) in mesh_ids.iter().enumerate() {
            self.spawn(&mut tasks, move |gpu, assets| {
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
        while tasks.pending > 0 {
            let report = tasks
                .reports
                .recv()
                .expect("the load keeps a sender while tasks are pending");
            tasks.pending -= 1;
            let made = match report {
                None => continue,
                Some(Err(panic)) => {
                    self.cancel.store(true, Ordering::Relaxed);
                    panicked.get_or_insert(panic);
                    continue;
                }
                Some(Ok(Err(error))) => {
                    self.cancel.store(true, Ordering::Relaxed);
                    failure.get_or_insert(error);
                    continue;
                }
                Some(Ok(Ok(made))) => made,
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
            if self.cancel.load(Ordering::Relaxed) || materials.contains_key(&material) {
                continue;
            }
            match self.assets.material(material) {
                Ok(cooked) => {
                    for slot in cooked.textures().into_iter().flatten() {
                        let id = slot.texture;
                        if let Entry::Vacant(entry) = textures.entry(id) {
                            entry.insert(None);
                            let label = format!("{} texture {id}", cooked.name);
                            self.spawn(&mut tasks, move |gpu, assets| {
                                let texture = upload_texture(gpu, &label, &assets.texture(id)?)?;
                                Ok(Made::Texture(id, texture))
                            });
                        }
                    }
                    materials.insert(material, cooked);
                }
                Err(error) => {
                    self.cancel.store(true, Ordering::Relaxed);
                    failure.get_or_insert(error.into());
                }
            }
        }
        if let Some(panic) = panicked {
            panic::resume_unwind(panic);
        }
        if let Some(error) = failure {
            return Some(Err(error));
        }
        if self.cancel.load(Ordering::Relaxed) {
            return None;
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

    /// Runs `task` on the pool, with this load's GPU and cooked data, unless the load is cancelled
    /// before it starts; it reports to `tasks`.
    fn spawn(
        &self,
        tasks: &mut Tasks,
        task: impl FnOnce(&Gpu, &CookedAssets) -> Result<Made, SceneError> + Send + 'static,
    ) {
        tasks.pending += 1;
        let gpu = self.gpu.clone();
        let assets = Arc::clone(&self.assets);
        let cancel = Arc::clone(&self.cancel);
        let sender = tasks.sender.clone();
        pool::run(move || {
            let report = (!cancel.load(Ordering::Relaxed))
                .then(|| panic::catch_unwind(AssertUnwindSafe(|| task(&gpu, &assets))));
            // The load waits for every report, so its receiver is there.
            let _ = sender.send(report);
        });
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
            if let Some(light) = node.light {
                loaded.lights.push(
                    place_light(light, transform).map_err(|reason| placement(index, reason))?,
                );
            }
        }
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
