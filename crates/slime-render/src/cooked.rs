//! Cooked scenes on the GPU: a scene that the asset pipeline cooked, its meshes, materials and
//! textures uploaded by tasks of the engine's pool, its nodes flattened into instances and its
//! lights placed in world space.

use std::any::Any;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;

use glam::{DAffine3, DVec3, Mat4};
use slime_assets::{
    AssetId, CookedAssets, CookedMaterial, CookedScene, CookedTexture, LightKind, LoadError,
    SceneLight, TextureEncoding, TextureFilter, TextureSampler, TextureWrap,
};
use slime_color::LinearColor;
use slime_gfx::{
    AddressMode, Filter, GfxError, Gpu, Sampler, SamplerDescriptor, Texture, TextureFormat,
    Uploader,
};

use crate::error::RenderError;
use crate::light::{DirectionalLight, Light, PointLight, SpotLight};
use crate::loads;
use crate::material::{AlphaMode, Material, MaterialData, MaterialParts, SampledTexture};
use crate::mesh::{Mesh, MeshData, MeshPages};

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

/// A placed mesh: indices into the scene's meshes and materials, and its transform.
struct Placement {
    mesh: usize,
    material: usize,
    transform: DAffine3,
}

/// A cooked scene on the GPU ([`Renderer::load_scene`]): its meshes and materials, which keep
/// their textures and samplers, where its nodes place them, and its lights in world space.
pub struct LoadedScene {
    meshes: Vec<Mesh>,
    materials: Vec<Material>,
    placements: Vec<Placement>,
    /// Its meshes and textures, by ID, with the cooked file each was read from, which a reload
    /// keeps ([`Renderer::reload_scene_in_background`]).
    reusable: Reusable,
    /// The scene as it was cooked: its nodes, their names, transforms and `extras`, which a game
    /// reads its own data from.
    pub scene: CookedScene,
    /// The lights its nodes place, in world space, in the order of the nodes.
    pub lights: Vec<Light>,
    /// The smallest box around every mesh placed, its lowest corner then its highest, in world
    /// space, in metres; `None` for a scene that places no mesh.
    pub bounds: Option<(DVec3, DVec3)>,
}

impl LoadedScene {
    /// The instances to draw: one per mesh that a node places, casting shadows.
    pub fn instances(&self) -> Vec<Instance> {
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

    /// The material named `name` among those its meshes are drawn with, its name being the cooked
    /// material's (`slime_assets::CookedMaterial::name`): when several have it, the first that a
    /// mesh of the scene is drawn with, in the order of the scene's meshes; `None` when none has
    /// it. A game changes how the scene looks through it ([`Material::set_values`]), and keeps a
    /// clone of it to do so at each frame.
    pub fn material(&self, name: &str) -> Option<&Material> {
        self.materials
            .iter()
            .find(|material| &*material.label == name)
    }
}

/// A cooked scene being loaded in the background, by tasks of the engine's pool
/// ([`Renderer::load_scene_in_background`]). A game polls it at each frame with
/// [`SceneLoad::take`], which gives the whole scene once it is loaded, never a part of it; or waits
/// for it with [`SceneLoad::wait`].
///
/// Dropping it cancels the load. The drop waits for the load's tasks that are running, a mesh or a
/// texture each, and drops what they made; the tasks that have not started then do nothing when
/// their turn comes, and never use the GPU. Once the drop returns, no task of the load holds the
/// `Gpu`; the copies that it left to the renderer's uploader go to the GPU as the others do, those
/// still waiting when the renderer is dropped at once.
pub struct SceneLoad {
    load: Load<LoadedScene>,
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
    /// With the panic of a task of the load, if one panicked, even when an asset also failed: a
    /// bug, as when a scene is reloaded on another device than the one it was loaded on
    /// ([`Renderer::reload_scene_in_background`]).
    pub fn take(&mut self) -> Option<Result<LoadedScene, SceneError>> {
        self.load.take()
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
    pub fn wait(self) -> Result<LoadedScene, SceneError> {
        self.load.wait()
    }
}

/// A load that runs as tasks of the engine's pool, without a thread of its own: a scene's
/// ([`SceneLoad`]), or a batch of meshes for the asset cache. The last of its tasks and of its
/// uploads to end completes it (ADR-0107). Dropping it cancels the load, as [`SceneLoad`] says.
pub(crate) struct Load<T> {
    job: Arc<Job<T>>,
}

impl<T> Load<T> {
    /// Whether the load finished: its result, or its panic, waits to be taken, or was taken.
    pub(crate) fn is_finished(&self) -> bool {
        let progress = self.job.progress();
        progress.result.is_some() || progress.taken
    }

    /// What the load gave, once it finished: `None` while it goes on, and once its result was
    /// taken. A panic of one of its tasks is raised again here.
    pub(crate) fn take(&mut self) -> Option<Result<T, SceneError>> {
        let mut progress = self.job.progress();
        let result = progress.result.take()?;
        progress.taken = true;
        drop(progress);
        Some(finish(result))
    }

    /// Waits for the load to finish, and returns what it gave.
    fn wait(self) -> Result<T, SceneError> {
        let mut progress = self.job.progress();
        loop {
            if let Some(result) = progress.result.take() {
                progress.taken = true;
                drop(progress);
                return finish(result);
            }
            assert!(!progress.taken, "the load's result is taken once");
            progress = self
                .job
                .changed
                .wait(progress)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl<T> Drop for Load<T> {
    fn drop(&mut self) {
        let mut progress = self.job.progress();
        progress.dropped = true;
        progress.stopped = true;
        // What the load gave, its panic included, is dropped with it, and what its tasks share,
        // the GPU's objects among them.
        let result = progress.result.take();
        let work = self.job.work().take();
        while progress.running > 0 {
            progress = self
                .job
                .changed
                .wait(progress)
                .unwrap_or_else(PoisonError::into_inner);
        }
        drop(progress);
        drop((result, work));
    }
}

/// The result of a load, or the panic of one of its tasks, raised again.
fn finish<T>(result: thread::Result<Result<T, SceneError>>) -> Result<T, SceneError> {
    match result {
        Ok(result) => result,
        Err(panic) => panic::resume_unwind(panic),
    }
}

/// One load: what it and its tasks share.
struct Job<T> {
    /// Its number in the loads' lane, which runs the tasks of the loads started first first.
    number: u64,
    progress: Mutex<Progress<T>>,
    /// Told when the load finishes, and when a running task ends.
    changed: Condvar,
    /// What its tasks work with, the GPU's objects among them; `None` once the load stopped or
    /// finished, so that the tasks that start later hold no GPU object.
    work: Mutex<Option<Work>>,
    /// The tasks and the uploads that have not ended: the last to end completes the load.
    left: AtomicUsize,
    /// The first error of a task or an upload, which stops the load.
    failure: Mutex<Option<SceneError>>,
    /// The first panic of a task, which stops the load.
    panic: Mutex<Option<Box<dyn Any + Send>>>,
    /// Whether a panic dropped one of its uploads before its copies were submitted, which stops
    /// the load with a panic of its own unless a task's panic says more: the task that panicked in
    /// its upload's hand-off records its panic after the upload is dropped.
    upload_dropped: AtomicBool,
    /// Makes the load's result from its work, once everything is made.
    complete: Mutex<Option<Complete<T>>>,
}

/// What makes a load's result from its work.
type Complete<T> = Box<dyn FnOnce(Work) -> Result<T, SceneError> + Send>;

struct Progress<T> {
    /// What the load gave once it finished, or the panic of one of its tasks; `None` before, and
    /// once taken.
    result: Option<thread::Result<Result<T, SceneError>>>,
    taken: bool,
    /// Whether the `Load` was dropped: the load then gives nothing.
    dropped: bool,
    /// Whether the load stopped, dropped or failed: the tasks that start from then on do nothing.
    stopped: bool,
    /// How many of its tasks run with the GPU, its completion included.
    running: usize,
}

/// What a load's tasks work with, and what they made.
pub(crate) struct Work {
    gpu: Gpu,
    assets: Arc<CookedAssets>,
    parts: MaterialParts,
    /// The pages of the renderer's meshes, into which the load's meshes go.
    meshes: MeshPages,
    /// The renderer's uploader, which brings the load's meshes and textures to the GPU.
    uploader: Uploader,
    /// What the scene that the load reloads was made of, to keep; empty for a first load.
    reuse: Reusable,
    /// The meshes asked for, in their order.
    mesh_ids: Vec<AssetId>,
    /// The scene of a scene's load, once read.
    scene: Option<CookedScene>,
    made: Assembly,
}

/// What a task works with: clones of the parts of [`Work`] that it reads.
struct Context {
    gpu: Gpu,
    assets: Arc<CookedAssets>,
    meshes: MeshPages,
    uploader: Uploader,
}

/// A mesh that a task of a load made.
#[derive(Clone)]
pub(crate) struct MadeMesh {
    pub(crate) mesh: Mesh,
    /// The material it is drawn with.
    pub(crate) material: AssetId,
    /// Its lowest corner, then its highest.
    pub(crate) bounds: [[f32; 3]; 2],
}

/// What a loaded scene, or the asset cache, was made of: its meshes and its textures, by ID, each
/// with the name of the cooked file it was read from. A load keeps those whose file the manifest
/// still names: a cooked file's name says what made it, so the same name holds the same data.
#[derive(Clone, Default)]
pub(crate) struct Reusable {
    pub(crate) meshes: HashMap<AssetId, (String, MadeMesh)>,
    pub(crate) textures: HashMap<AssetId, (String, Texture)>,
}

/// The meshes that a load made or kept, in the order they were asked for, the materials they are
/// drawn with and the textures those read, by ID.
pub(crate) struct Batch {
    pub(crate) meshes: Vec<(AssetId, MadeMesh)>,
    pub(crate) materials: HashMap<AssetId, Material>,
    pub(crate) textures: HashMap<AssetId, Texture>,
    /// How many meshes were kept rather than read.
    pub(crate) kept_meshes: usize,
    /// How many textures were kept rather than read.
    pub(crate) kept_textures: usize,
}

/// What a load has made, or kept, so far.
#[derive(Default)]
struct Assembly {
    /// The meshes asked for, in their order.
    meshes: Vec<Option<MadeMesh>>,
    /// The materials that the meshes made so far are drawn with.
    materials: HashMap<AssetId, CookedMaterial>,
    /// The textures those materials read, `None` until made.
    textures: HashMap<AssetId, Option<Texture>>,
    /// How many meshes were kept.
    kept_meshes: usize,
    /// How many textures were kept.
    kept_textures: usize,
}

impl<T> Job<T> {
    fn progress(&self) -> MutexGuard<'_, Progress<T>> {
        // A task's panic is caught before it reaches a lock: a lock poisoned is as good as whole.
        self.progress.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn work(&self) -> MutexGuard<'_, Option<Work>> {
        self.work.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<T: Send + 'static> Job<T> {
    /// Runs `task` as a background task of the engine's pool, with clones of what the load works
    /// with, unless the load stopped when its turn comes; its error or its panic stops the load.
    fn spawn(
        self: &Arc<Self>,
        task: impl FnOnce(&Arc<Self>, &Context) -> Result<(), SceneError> + Send + 'static,
    ) {
        self.left.fetch_add(1, Ordering::AcqRel);
        let job = Arc::clone(self);
        loads::run(self.number, move || {
            if let Some(context) = job.start_running() {
                match panic::catch_unwind(AssertUnwindSafe(|| task(&job, &context))) {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => job.fail(error),
                    Err(panic) => job.panicked(panic),
                }
                drop(context);
                job.stop_running();
            }
            job.end_one();
        });
    }

    /// Clones of what the load works with, counted as running, unless the load stopped.
    fn start_running(&self) -> Option<Context> {
        {
            let mut progress = self.progress();
            if progress.stopped {
                return None;
            }
            progress.running += 1;
        }
        // Not under the lock of the load's progress, which a game polls at each frame.
        let context = self.work().as_ref().map(|work| Context {
            gpu: work.gpu.clone(),
            assets: Arc::clone(&work.assets),
            meshes: work.meshes.clone(),
            uploader: work.uploader.clone(),
        });
        if context.is_none() {
            self.stop_running();
        }
        context
    }

    fn stop_running(&self) {
        self.progress().running -= 1;
        self.changed.notify_all();
    }

    fn fail(&self, error: SceneError) {
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert(error);
        self.progress().stopped = true;
    }

    fn panicked(&self, panic: Box<dyn Any + Send>) {
        self.panic
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert(panic);
        self.progress().stopped = true;
    }

    /// The callback of an upload, which counts until it is called or dropped: an error stops the
    /// load, and the end of the last upload or task completes it. Dropped uncalled, as when its
    /// hand-off returned an error, it ends the upload all the same; dropped by a panic, of its
    /// hand-off or of the uploader's thread, it also stops the load, which then raises the task's
    /// panic, or one of its own, where it is taken, so that the load still ends.
    fn upload_done(self: &Arc<Self>) -> impl FnOnce(Result<(), GfxError>) + Send + 'static {
        self.left.fetch_add(1, Ordering::AcqRel);
        let end = UploadEnd {
            job: Some(Arc::clone(self)),
        };
        move |result| end.call(result)
    }

    /// Ends a task or an upload that is not the last; the last completes the load.
    fn end_one(self: &Arc<Self>) {
        if self.left.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.complete();
        }
    }

    /// Gives the load's result, once nothing of it is left to end: its first panic, its first
    /// error, or what its completion makes from its work. A dropped load gives nothing.
    fn complete(&self) {
        let (work, complete) = {
            let mut progress = self.progress();
            if progress.dropped {
                return;
            }
            // Counted as running, so that a drop waits for the completion, which uses the GPU.
            progress.running += 1;
            let complete = self
                .complete
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            (self.work().take(), complete)
        };
        let panic = self
            .panic
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .or_else(|| {
                self.upload_dropped.load(Ordering::Acquire).then(|| {
                    Box::new(
                        "an upload was dropped before its copies were submitted: a panic stopped \
                         its hand-off or the uploader's thread"
                            .to_owned(),
                    ) as Box<dyn Any + Send>
                })
            });
        let failure = self
            .failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let result = match (panic, failure, work, complete) {
            (Some(panic), ..) => Err(panic),
            (None, Some(error), ..) => Ok(Err(error)),
            (None, None, Some(work), Some(complete)) => {
                panic::catch_unwind(AssertUnwindSafe(|| complete(work)))
            }
            // The load is completed once, and its work taken by the completion alone.
            (None, None, ..) => unreachable!("a load is completed once"),
        };
        let mut progress = self.progress();
        if progress.dropped {
            // Dropped while it completed: what it made goes before the drop returns.
            drop(progress);
            drop(result);
            progress = self.progress();
        } else {
            progress.result = Some(result);
        }
        progress.running -= 1;
        drop(progress);
        self.changed.notify_all();
    }

    /// Asks for the meshes `mesh_ids`, each once: those that the load's `reuse` holds whose cooked
    /// file the manifest still names are kept, a task reads and uploads each other.
    fn ask(self: &Arc<Self>, mesh_ids: Vec<AssetId>) -> Result<(), SceneError> {
        let held: Vec<Option<MadeMesh>> = {
            let mut work = self.work();
            let Some(work) = work.as_mut() else {
                return Ok(());
            };
            work.made.meshes = mesh_ids.iter().map(|_| None).collect();
            let held: Vec<Option<MadeMesh>> = mesh_ids
                .iter()
                .map(|&id| kept(&work.assets, &work.reuse.meshes, id))
                .collect();
            work.made.kept_meshes = held.iter().flatten().count();
            work.mesh_ids.clone_from(&mesh_ids);
            held
        };
        for (index, (&id, held)) in mesh_ids.iter().zip(&held).enumerate() {
            if held.is_none() {
                self.spawn(move |job, context| job.read_mesh(context, index, id));
            }
        }
        for (index, mesh) in held.into_iter().enumerate() {
            if let Some(mesh) = mesh {
                self.take_mesh(index, mesh)?;
            }
        }
        Ok(())
    }

    /// Reads the mesh `id`, the one at `index` of those asked for, places it in a page of the
    /// renderer's meshes, and leaves its copy to the renderer's uploader.
    fn read_mesh(
        self: &Arc<Self>,
        context: &Context,
        index: usize,
        id: AssetId,
    ) -> Result<(), SceneError> {
        let cooked = context.assets.mesh(id)?;
        let data = MeshData {
            positions: cooked.positions,
            normals: cooked.normals,
            uvs: cooked.uvs,
            tangents: cooked.tangents,
            indices: cooked.indices,
        };
        // An upload that does not start ends with its callback, dropped uncalled.
        let done = self.upload_done();
        let mesh = Mesh::upload(
            &context.gpu,
            &context.meshes,
            &context.uploader,
            &cooked.name,
            &data,
            done,
        )?;
        self.take_mesh(
            index,
            MadeMesh {
                mesh,
                material: cooked.material,
                bounds: cooked.bounds,
            },
        )
    }

    /// Takes the mesh made or kept at `index` into what the load made: reads its material, unless
    /// a mesh read it before, and starts reading the textures that the material reads, but those
    /// that the load or the scene it reloads holds.
    fn take_mesh(self: &Arc<Self>, index: usize, mesh: MadeMesh) -> Result<(), SceneError> {
        let material = mesh.material;
        let assets = {
            let mut work = self.work();
            let Some(work) = work.as_mut() else {
                return Ok(());
            };
            work.made.meshes[index] = Some(mesh);
            if work.made.materials.contains_key(&material) {
                return Ok(());
            }
            Arc::clone(&work.assets)
        };
        // Read outside the lock, which the other tasks of the load take; two meshes of one
        // material may both read it, and the second keeps the first's.
        let cooked = assets.material(material)?;
        let wanted = {
            let mut work = self.work();
            let Some(work) = work.as_mut() else {
                return Ok(());
            };
            if work.made.materials.contains_key(&material) {
                return Ok(());
            }
            let mut wanted = Vec::new();
            for slot in cooked.textures().into_iter().flatten() {
                let id = slot.texture;
                let Entry::Vacant(entry) = work.made.textures.entry(id) else {
                    continue;
                };
                if let Some(texture) = kept(&work.assets, &work.reuse.textures, id) {
                    entry.insert(Some(texture));
                    work.made.kept_textures += 1;
                    continue;
                }
                entry.insert(None);
                // Named after its source, whichever material asks for it first.
                let label = match work.assets.manifest().get(id) {
                    Some(entry) => format!("texture {id} from {}", entry.source),
                    None => format!("texture {id}"),
                };
                wanted.push((id, label));
            }
            work.made.materials.insert(material, cooked);
            wanted
        };
        for (id, label) in wanted {
            self.spawn(move |job, context| job.read_texture(context, id, &label));
        }
        Ok(())
    }

    /// Reads the texture `id`, and leaves its upload to the renderer's uploader.
    fn read_texture(
        self: &Arc<Self>,
        context: &Context,
        id: AssetId,
        label: &str,
    ) -> Result<(), SceneError> {
        let cooked = context.assets.texture(id)?;
        // As in `read_mesh`.
        let done = self.upload_done();
        let texture = queue_texture(&context.uploader, label, &cooked, done)?;
        if let Some(work) = self.work().as_mut() {
            work.made.textures.insert(id, Some(texture));
        }
        Ok(())
    }
}

/// The end of one upload of a load's ([`Job::upload_done`]).
struct UploadEnd<T: Send + 'static> {
    /// `None` once called.
    job: Option<Arc<Job<T>>>,
}

impl<T: Send + 'static> UploadEnd<T> {
    /// Ends the upload: an error stops the load; the last upload or task to end has the load
    /// completed, before the tasks that wait in the loads' lane, so that a load is not held behind
    /// others. Called on the uploader's thread, it does little there.
    fn call(mut self, result: Result<(), GfxError>) {
        let job = self.job.take().expect("an upload ends once");
        if let Err(error) = result {
            job.fail(RenderError::from(error).into());
        }
        Self::end(job);
    }

    fn end(job: Arc<Job<T>>) {
        if job.left.fetch_sub(1, Ordering::AcqRel) == 1 {
            loads::run(0, move || job.complete());
        }
    }
}

impl<T: Send + 'static> Drop for UploadEnd<T> {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            // An upload whose hand-off returned an error has its task's error; one that a panic
            // dropped has none, unless the panic was its task's.
            if thread::panicking() {
                job.upload_dropped.store(true, Ordering::Release);
                job.progress().stopped = true;
            }
            Self::end(job);
        }
    }
}

/// What `kept` holds of the asset `id`, if the manifest of `assets` still names the cooked file it
/// was read from.
fn kept<T: Clone>(
    assets: &CookedAssets,
    kept: &HashMap<AssetId, (String, T)>,
    id: AssetId,
) -> Option<T> {
    let (file, made) = kept.get(&id)?;
    (assets.manifest().get(id)?.file == *file).then(|| made.clone())
}

impl Renderer {
    /// Loads the cooked scene `id` from `assets` onto the GPU, as
    /// [`Renderer::load_scene_in_background`] does, and waits for it.
    ///
    /// # Errors
    ///
    /// [`SceneError::Load`] if an asset cannot be read; [`SceneError::Render`] if the GPU's memory
    /// cannot hold an object, the device is lost, the GPU cannot sample the cooked textures
    /// (`GfxError::UnsupportedTexture`), or a material has values out of their ranges, or a texture
    /// of the wrong kind for its slot, data where it reads colors or the reverse
    /// ([`RenderError::InvalidMaterial`]); [`SceneError::Placement`] if a node places a mesh at a
    /// point that is not finite, which a transform that overflows gives, or a light at such a point
    /// or with no direction, as a transform that flattens its −Z axis gives. When several assets
    /// fail, the error is the first that a task of the load reports, which can change from one run
    /// to the next.
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
    /// read, each once, by background tasks of the engine's pool (`slime_tasks`), which create
    /// their GPU objects through clones of `gpu` and write their vertices and texels into staging
    /// buffers (ADR-0073, ADR-0107); and it places its lights by their nodes in world space, at the
    /// node's origin, along its −Z axis whatever its scale. A glTF file's default scene has the
    /// file's ID.
    ///
    /// The renderer's uploader submits the copies of the meshes and textures to the GPU, at most a
    /// budget of bytes per frame ([`Renderer::render`] says how much), so that a frame does not
    /// wait for all of a load's copies, and a load takes longer the more bytes it brings. The load
    /// ends when the last of its tasks and of its copies does, without a thread of its own: the
    /// pool's threads are shared by every load of the process, and keep one for the work that a
    /// frame waits for. A frame that draws the first shadows, or changes the shape of the frame
    /// otherwise, can cost more during a load than after it.
    ///
    /// # Panics
    ///
    /// If the operating system cannot start the threads of the engine's pool.
    pub fn load_scene_in_background(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        id: AssetId,
    ) -> SceneLoad {
        self.start_scene_load(gpu, assets, id, Reusable::default())
    }

    /// Starts loading the cooked scene `id` from `assets` again, as
    /// [`Renderer::load_scene_in_background`] does, once the cooked data has changed: the meshes
    /// and textures of `previous` whose cooked file the manifest of `assets` still names are kept,
    /// neither read nor uploaded again: the cooker names each cooked file after what made it
    /// (`slime_cook::cook` says how), so the same name holds the same data. The materials are made
    /// again, from the textures kept or uploaded, with the values the cooked data gives: the values
    /// that a game set on the materials of `previous` ([`Material::set_values`]) are not carried
    /// over, and a game that changes them finds them again in the new scene
    /// ([`LoadedScene::material`]). `previous` stays whole, to be drawn while the load goes on; the
    /// new scene and it share what was kept.
    ///
    /// # Panics
    ///
    /// As [`Renderer::load_scene_in_background`]; and when a material is made, if `previous` was
    /// loaded on another device than `gpu`'s: wgpu refuses its textures.
    pub fn reload_scene_in_background(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        id: AssetId,
        previous: &LoadedScene,
    ) -> SceneLoad {
        self.start_scene_load(gpu, assets, id, previous.reusable.clone())
    }

    /// Starts the load of the scene `id` from `assets`, which keeps what `reuse` holds whose cooked
    /// file the manifest still names.
    fn start_scene_load(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        id: AssetId,
        reuse: Reusable,
    ) -> SceneLoad {
        let load = self.loader().start_load(
            gpu,
            assets,
            reuse,
            move |job, context| {
                let scene = context.assets.scene(id)?;
                // The meshes, each once, in the order in which the nodes first place them.
                let mut mesh_ids = Vec::new();
                let mut seen = HashSet::new();
                for &mesh in scene.nodes.iter().flat_map(|node| &node.meshes) {
                    if seen.insert(mesh) {
                        mesh_ids.push(mesh);
                    }
                }
                if let Some(work) = job.work().as_mut() {
                    work.scene = Some(scene);
                }
                job.ask(mesh_ids)
            },
            |mut work| {
                let scene = work
                    .scene
                    .take()
                    .expect("a scene's load reads its scene first");
                let batch = work.made_batch()?;
                work.assemble(scene, batch)
            },
        );
        SceneLoad { load }
    }

    /// What its loads, and an asset cache's, take of it: the parts of its materials, the pages of
    /// its meshes and its uploader, which they share.
    pub(crate) fn loader(&self) -> Loader {
        Loader {
            parts: self.material_parts.clone(),
            meshes: self.meshes.clone(),
            uploader: self.uploader.clone(),
        }
    }
}

/// What a load takes of the renderer that draws what it brings: the parts of its materials, the
/// pages of its meshes and its uploader. An asset cache keeps one from its creation, so that it
/// loads without the renderer (ADR-0111).
#[derive(Clone)]
pub(crate) struct Loader {
    parts: MaterialParts,
    meshes: MeshPages,
    uploader: Uploader,
}

impl Loader {
    /// Starts loading the meshes `mesh_ids` from `assets`, each once, with the materials they are
    /// drawn with and the textures those read, as a scene's load does: a batch for the asset cache,
    /// which keeps what `reuse` holds whose cooked file the manifest still names.
    pub(crate) fn start_batch(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        reuse: Reusable,
        mesh_ids: Vec<AssetId>,
    ) -> Load<Batch> {
        self.start_load(
            gpu,
            assets,
            reuse,
            move |job, _| job.ask(mesh_ids),
            Work::batch,
        )
    }

    /// Starts a load whose first task, `begin`, asks for what it loads, and whose result
    /// `complete` makes from what it made.
    fn start_load<T: Send + 'static>(
        &self,
        gpu: &Gpu,
        assets: &CookedAssets,
        reuse: Reusable,
        begin: impl FnOnce(&Arc<Job<T>>, &Context) -> Result<(), SceneError> + Send + 'static,
        complete: impl FnOnce(Work) -> Result<T, SceneError> + Send + 'static,
    ) -> Load<T> {
        let job = Arc::new(Job {
            number: loads::number(),
            progress: Mutex::new(Progress {
                result: None,
                taken: false,
                dropped: false,
                stopped: false,
                running: 0,
            }),
            changed: Condvar::new(),
            work: Mutex::new(Some(Work {
                gpu: gpu.clone(),
                assets: Arc::new(assets.clone()),
                parts: self.parts.clone(),
                meshes: self.meshes.clone(),
                uploader: self.uploader.clone(),
                reuse,
                mesh_ids: Vec::new(),
                scene: None,
                made: Assembly::default(),
            })),
            left: AtomicUsize::new(0),
            failure: Mutex::new(None),
            panic: Mutex::new(None),
            upload_dropped: AtomicBool::new(false),
            complete: Mutex::new(Some(Box::new(complete))),
        });
        job.spawn(begin);
        Load { job }
    }
}

impl Work {
    /// Completes the batch of the meshes asked for from what the load made, once it made
    /// everything: the materials its meshes are drawn with, created with the samplers they need.
    fn batch(mut self) -> Result<Batch, SceneError> {
        self.made_batch()
    }

    fn made_batch(&mut self) -> Result<Batch, SceneError> {
        let made = std::mem::take(&mut self.made);
        let meshes: Vec<(AssetId, MadeMesh)> = self
            .mesh_ids
            .iter()
            .copied()
            .zip(made.meshes)
            .map(|(id, mesh)| (id, mesh.expect("every mesh is made when nothing failed")))
            .collect();
        let textures: HashMap<AssetId, Texture> = made
            .textures
            .into_iter()
            .map(|(id, texture)| {
                let texture = texture.expect("every texture is made when nothing failed");
                (id, texture)
            })
            .collect();
        let mut samplers: HashMap<TextureSampler, Sampler> = HashMap::new();
        let mut materials = HashMap::with_capacity(made.materials.len());
        // In the order of the meshes that are drawn with them.
        for (_, mesh) in &meshes {
            if let Entry::Vacant(entry) = materials.entry(mesh.material) {
                let cooked = &made.materials[&mesh.material];
                entry.insert(self.create_material(cooked, &textures, &mut samplers)?);
            }
        }
        Ok(Batch {
            meshes,
            materials,
            textures,
            kept_meshes: made.kept_meshes,
            kept_textures: made.kept_textures,
        })
    }

    /// Puts the loaded scene together from the `batch` of its meshes: where its nodes place its
    /// meshes and lights, and what a reload of it keeps.
    fn assemble(&self, scene: CookedScene, batch: Batch) -> Result<LoadedScene, SceneError> {
        let file = |id: AssetId| cooked_file(&self.assets, id);
        let reusable = Reusable {
            meshes: batch
                .meshes
                .iter()
                .map(|(id, mesh)| (*id, (file(*id), mesh.clone())))
                .collect(),
            textures: batch
                .textures
                .iter()
                .map(|(&id, texture)| (id, (file(id), texture.clone())))
                .collect(),
        };
        let textures = batch.textures.len();
        let mut materials = batch.materials;
        let mut material_of: HashMap<AssetId, usize> = HashMap::new();
        let mut mesh_index: HashMap<AssetId, usize> = HashMap::new();
        let mut loaded = LoadedScene {
            meshes: Vec::with_capacity(batch.meshes.len()),
            materials: Vec::new(),
            placements: Vec::new(),
            reusable,
            scene: CookedScene::default(),
            lights: Vec::new(),
            bounds: None,
        };
        // The material and bounds of each mesh, in the order of `loaded.meshes`.
        let mut drawn = Vec::with_capacity(batch.meshes.len());
        for (id, mesh) in batch.meshes {
            let index = match material_of.entry(mesh.material) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let material = materials
                        .remove(&mesh.material)
                        .expect("a batch holds the material of each of its meshes");
                    loaded.materials.push(material);
                    *entry.insert(loaded.materials.len() - 1)
                }
            };
            mesh_index.insert(id, loaded.meshes.len());
            loaded.meshes.push(mesh.mesh);
            drawn.push((index, mesh.bounds));
        }
        let world = scene.world_transforms();
        let placement = |node: usize, reason: String| SceneError::Placement {
            scene: scene.name.clone(),
            node,
            reason,
        };
        for (index, (node, transform)) in scene.nodes.iter().zip(&world).enumerate() {
            let transform = placed(transform);
            for mesh in &node.meshes {
                let mesh = mesh_index[mesh];
                let (material, bounds) = drawn[mesh];
                loaded.bounds = grow(loaded.bounds, transform, bounds)
                    .map_err(|reason| placement(index, reason))?;
                loaded.placements.push(Placement {
                    mesh,
                    material,
                    transform,
                });
            }
        }
        loaded.lights = scene_lights(&scene)?;
        tracing::info!(
            scene = %scene.name,
            meshes = loaded.meshes.len(),
            materials = loaded.materials.len(),
            textures,
            instances = loaded.placements.len(),
            lights = loaded.lights.len(),
            kept_meshes = batch.kept_meshes,
            kept_textures = batch.kept_textures,
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
        // A slot that reads a texture of the wrong kind is a bug of a game's code, which panics,
        // but an error of the cooked data.
        if let Some(reason) = data.misplaced_texture() {
            return Err(RenderError::InvalidMaterial {
                label: cooked.name.clone(),
                reason,
            }
            .into());
        }
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
            place_light(light, placed(&transform)).map_err(|reason| SceneError::Placement {
                scene: scene.name.clone(),
                node: index,
                reason,
            })
        })
        .collect()
}

/// The name of the cooked file that `assets` reads the asset `id` from, which a later load compares
/// to keep the asset: empty for an asset that the manifest does not name.
pub(crate) fn cooked_file(assets: &CookedAssets, id: AssetId) -> String {
    assets
        .manifest()
        .get(id)
        .map(|entry| entry.file.clone())
        .unwrap_or_default()
}

/// A node's transform in the scene's world space, as `CookedScene::world_transforms` gives it in
/// `f32`, with `f64` components, as positions in the world are.
fn placed(transform: &[f32; 16]) -> DAffine3 {
    DAffine3::from_mat4(Mat4::from_cols_array(transform).as_dmat4())
}

/// The light that a node placed by `transform` holds, in world space: at the node's origin, along
/// its −Z axis whatever its scale; refused, with the reason, at a point that is not finite, without
/// a direction, or if the renderer would not draw it ([`Light::check`]).
fn place_light(light: SceneLight, transform: DAffine3) -> Result<Light, String> {
    let position = transform.translation;
    if !position.is_finite() {
        return Err(format!(
            "its light is placed at {position}, which is not finite"
        ));
    }
    // Scale changes the length of the axis, not the direction it gives.
    let direction = transform
        .transform_vector3(DVec3::NEG_Z)
        .try_normalize()
        .ok_or_else(|| {
            "its light has no direction: the node's transform flattens its −Z axis or is not \
             finite"
                .to_owned()
        })?
        .as_vec3();
    let [r, g, b] = light.color;
    let color = LinearColor::rgb(r, g, b);
    let placed = match light.kind {
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
    };
    // Its values were checked when the scene was read, by the rule that this applies too.
    placed.check().map(|()| placed)
}

/// Uploads a cooked texture, as [`Renderer::create_texture`] says.
pub(crate) fn upload_texture(
    gpu: &Gpu,
    label: &str,
    data: &CookedTexture,
) -> Result<Texture, RenderError> {
    let levels: Vec<&[u8]> = data.levels.iter().map(Vec::as_slice).collect();
    Ok(gpu.create_texture_with_levels(
        label,
        data.width,
        data.height,
        texture_format(data.encoding),
        &levels,
    )?)
}

/// Leaves the upload of a cooked texture to `uploader`, which calls `done` once its copies are
/// submitted, as [`Uploader::texture`] says; the texture is returned at once, to be drawn then.
fn queue_texture(
    uploader: &Uploader,
    label: &str,
    data: &CookedTexture,
    done: impl FnOnce(Result<(), GfxError>) + Send + 'static,
) -> Result<Texture, RenderError> {
    let levels: Vec<&[u8]> = data.levels.iter().map(Vec::as_slice).collect();
    Ok(uploader.texture(
        label,
        data.width,
        data.height,
        texture_format(data.encoding),
        &levels,
        done,
    )?)
}

/// The GPU format of a cooked texture's encoding.
fn texture_format(encoding: TextureEncoding) -> TextureFormat {
    match encoding {
        TextureEncoding::Bc7Srgb => TextureFormat::Bc7RgbaUnormSrgb,
        TextureEncoding::Bc7 => TextureFormat::Bc7RgbaUnorm,
        TextureEncoding::Bc5 => TextureFormat::Bc5RgUnorm,
        TextureEncoding::Rgba8Srgb => TextureFormat::Rgba8UnormSrgb,
        TextureEncoding::Rgba8 => TextureFormat::Rgba8Unorm,
    }
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
    bounds: Option<(DVec3, DVec3)>,
    transform: DAffine3,
    mesh: [[f32; 3]; 2],
) -> Result<Option<(DVec3, DVec3)>, String> {
    let [low, high] = mesh.map(|corner| DVec3::from(corner.map(f64::from)));
    let mut bounds = bounds;
    for corner in 0..8 {
        let pick = |bit: usize, low: f64, high: f64| if corner & bit == 0 { low } else { high };
        let point = transform.transform_point3(DVec3::new(
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
    use std::sync::mpsc;
    use std::time::Duration;

    use slime_assets::{AssetKind, CookedMesh, MANIFEST_FILE, Manifest, ManifestEntry, SceneNode};
    use slime_gfx::TextureFormat;
    use slime_tasks::Priority;

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
                components: Vec::new(),
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
        let _holding = crate::loads::tests::HOLDING_THE_POOL
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        let (assets, scene) = squares("dropped", 8);
        // Every thread of the pool waits for the test, held by frame tasks, which every thread
        // takes, so that the load's tasks wait in its queue.
        let threads = slime_tasks::threads();
        let barrier = Arc::new(Barrier::new(threads + 1));
        let (busy, all_busy) = mpsc::channel();
        for _ in 0..threads {
            let barrier = Arc::clone(&barrier);
            let busy = busy.clone();
            slime_tasks::run(Priority::Frame, move || {
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

    #[test]
    fn a_panic_of_a_task_in_its_upload_s_hand_off_is_the_one_raised() {
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        // An empty manifest, in a directory apart from `squares`', which another test removes.
        let directory =
            std::env::temp_dir().join(format!("slime-render-hand-off-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("the directory can be created");
        fs::write(directory.join(MANIFEST_FILE), Manifest::new().to_bytes()).expect("written");
        let assets = CookedAssets::open(&directory).expect("the manifest is read");
        // The upload's callback is dropped by the panic's unwinding, before the task records it.
        let load = renderer.loader().start_load(
            &gpu,
            &assets,
            Reusable::default(),
            |job, _| {
                let _done = job.upload_done();
                panic!("the task's own panic");
            },
            |_| Ok(()),
        );
        let panic = panic::catch_unwind(AssertUnwindSafe(|| load.wait()))
            .expect_err("the load raises its task's panic");
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"the task's own panic"));
        drop(assets);
        let _ = fs::remove_dir_all(directory);
    }
}
