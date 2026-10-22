//! The asset cache: the GPU objects of the meshes that a game draws, by asset ID, outside any world,
//! held by the requests that ask for them, and dropped past a budget once nothing holds them
//! (`docs/adr/0084-what-the-world-draws.md`, `docs/adr/0113-asset-requests-residency-and-priorities.md`,
//! `docs/adr/0128-asset-requests.md`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use slime_assets::{AssetId, CookedAssets, LoadError};
use slime_gfx::{Gpu, Texture, UploadRank};
use slime_reflect::Reflect;

use crate::cooked::{Batch, Load, Loader, MadeMesh, Reusable, SceneError, cooked_file};
use crate::loads;
use crate::material::Material;
use crate::mesh::Mesh;
use crate::renderer::Renderer;

/// The budget of the textures' memory that a cache starts with: 3 GiB, three quarters of the 4 GiB
/// of video memory for assets of the lowest hardware tier, the handheld one (ADR-0114, ADR-0128).
pub const DEFAULT_TEXTURE_BUDGET: u64 = 3 << 30;
/// The budget of the meshes' memory that a cache starts with: 1 GiB, the rest of the handheld
/// tier's 4 GiB.
pub const DEFAULT_MESH_BUDGET: u64 = 1 << 30;

/// How urgently an asset is wanted: the class of its request (ADR-0113). The tasks of the loads and
/// their copies to the GPU go class by class, in this order, and within a class in the order the
/// loads started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// What a view draws and lacks.
    #[default]
    Now,
    /// What a game fetches ahead: the next area, the arena of the next fight.
    Soon,
    /// What is fetched far ahead, and the editor's thumbnails and previews.
    Later,
}

impl Priority {
    /// The class's place among the others, from 0, the most urgent.
    pub(crate) fn class(self) -> u64 {
        match self {
            Self::Now => 0,
            Self::Soon => 1,
            Self::Later => 2,
        }
    }
}

/// What became of an asset that a handle holds.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum AssetState {
    /// It loads, or waits for its turn.
    Loading,
    /// The cache holds it, ready to be drawn ([`AssetCache::mesh`]).
    Ready,
    /// It cannot be made: why. The cache does not ask for it again until its cooked data changes
    /// ([`AssetCache::reload`]), or a new cache takes the requests ([`AssetCache::with_requests`]).
    Failed(Arc<SceneError>),
    /// The cooked data does not name it ([`LoadError::NotCooked`]): as a mesh that failed, it is
    /// not asked for again until the cooked data changes.
    Missing,
}

/// The requests that hold a cache's assets, and what became of each asset that they hold: what a
/// cache gives back when its device is lost ([`AssetCache::into_requests`]), for the cache that
/// replaces it to load again ([`AssetCache::with_requests`]), so that the handles held stay valid.
/// It holds no GPU object.
#[derive(Debug, Default)]
pub struct AssetRequests {
    book: Arc<Mutex<Book>>,
}

impl AssetRequests {
    /// Requests that hold nothing, for a new cache ([`AssetCache::new`]).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// What the handles of a cache hold, which they and the cache share.
#[derive(Debug, Default)]
struct Book {
    /// Each asset that a handle holds, its handles' priorities by their numbers, and its state.
    assets: HashMap<AssetId, Asked>,
    /// The number of the next handle.
    next: u64,
}

#[derive(Debug)]
struct Asked {
    handles: BTreeMap<u64, Priority>,
    state: AssetState,
}

impl Asked {
    /// The most urgent priority of its handles; `None` without a handle.
    fn priority(&self) -> Option<Priority> {
        self.handles.values().copied().min()
    }
}

/// Locks `book`, which no code panics while holding.
fn lock(book: &Mutex<Book>) -> MutexGuard<'_, Book> {
    book.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A hold on one asset that a request asked for (ADR-0113): while it lives, the cache keeps the
/// asset, or keeps loading it, at its priority. Dropping the last handle of an asset that loads
/// stops its load, if nothing else of the load is held; of one that the cache holds, makes it
/// *releasable*: kept while the cache's budget has room, and dropped, least recently used first,
/// once it has none ([`AssetCache::set_budgets`]). A clone is another hold, at the same priority.
///
/// It holds nothing of the GPU's, and can be sent to another thread or outlive the cache that gave
/// it: it stays valid through new cooked data ([`AssetCache::reload`]) and a lost device
/// ([`AssetCache::into_requests`]).
pub struct AssetHandle {
    id: AssetId,
    number: u64,
    book: Arc<Mutex<Book>>,
}

impl fmt::Debug for AssetHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetHandle")
            .field("id", &self.id)
            .field("priority", &self.priority())
            .finish_non_exhaustive()
    }
}

impl AssetHandle {
    /// A new handle of `id`, at `priority`, in `book`, which holds it.
    fn new(book: &Arc<Mutex<Book>>, id: AssetId, priority: Priority) -> Self {
        let mut held = lock(book);
        let number = held.next;
        held.next += 1;
        held.assets
            .entry(id)
            .or_insert_with(|| Asked {
                handles: BTreeMap::new(),
                state: AssetState::Loading,
            })
            .handles
            .insert(number, priority);
        Self {
            id,
            number,
            book: Arc::clone(book),
        }
    }

    /// The asset it holds.
    #[must_use]
    pub fn id(&self) -> AssetId {
        self.id
    }

    /// What became of the asset, as the cache's last [`AssetCache::poll`],
    /// [`AssetCache::request`] or [`AssetCache::reload`] found it.
    #[must_use]
    pub fn state(&self) -> AssetState {
        lock(&self.book)
            .assets
            .get(&self.id)
            .map_or(AssetState::Loading, |asked| asked.state.clone())
    }

    /// Its priority.
    #[must_use]
    pub fn priority(&self) -> Priority {
        lock(&self.book)
            .assets
            .get(&self.id)
            .and_then(|asked| asked.handles.get(&self.number).copied())
            .unwrap_or_default()
    }

    /// Changes its priority: from the cache's next [`AssetCache::poll`], the load of the asset, if
    /// it loads, goes at the most urgent priority of the asset's handles, among the loads of that
    /// class in the order they started, its waiting copies included.
    pub fn set_priority(&self, priority: Priority) {
        if let Some(asked) = lock(&self.book).assets.get_mut(&self.id) {
            asked.handles.insert(self.number, priority);
        }
    }
}

impl Clone for AssetHandle {
    fn clone(&self) -> Self {
        Self::new(&self.book, self.id, self.priority())
    }
}

impl Drop for AssetHandle {
    fn drop(&mut self) {
        let mut book = lock(&self.book);
        if let Some(asked) = book.assets.get_mut(&self.id) {
            asked.handles.remove(&self.number);
            if asked.handles.is_empty() {
                book.assets.remove(&self.id);
            }
        }
    }
}

/// The assets asked for together ([`AssetCache::request`]), which arrive together: the cache
/// draws none of those it loads for the request before it has every one, but those that fail
/// (ADR-0113). It holds each through a handle, which [`Request::into_handles`] gives, so that each
/// is let go of on its own.
#[derive(Debug)]
#[must_use = "dropping a request lets go of what it asked for"]
pub struct Request {
    handles: Vec<AssetHandle>,
}

impl Request {
    /// The handles of the assets asked for, in their order.
    pub fn handles(&self) -> &[AssetHandle] {
        &self.handles
    }

    /// Whether every asset asked for arrived or cannot: none loads any more.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.handles
            .iter()
            .all(|handle| !matches!(handle.state(), AssetState::Loading))
    }

    /// Changes the priority of each of its handles ([`AssetHandle::set_priority`]).
    pub fn set_priority(&self, priority: Priority) {
        for handle in &self.handles {
            handle.set_priority(priority);
        }
    }

    /// Its handles, one for each asset asked for, in their order, each let go of on its own.
    #[must_use]
    pub fn into_handles(self) -> Vec<AssetHandle> {
        self.handles
    }
}

/// How much of the GPU's memory a cache keeps its textures and its meshes within, in bytes
/// ([`AssetCache::set_budgets`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budgets {
    /// The textures': [`DEFAULT_TEXTURE_BUDGET`] by default.
    pub textures: u64,
    /// The meshes', the pages they share: [`DEFAULT_MESH_BUDGET`] by default.
    pub meshes: u64,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            textures: DEFAULT_TEXTURE_BUDGET,
            meshes: DEFAULT_MESH_BUDGET,
        }
    }
}

/// What a cache's [`AssetCache::report`] says, which tools read through reflection
/// (`docs/adr/0128-asset-requests.md`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(name = "AssetCacheReport")]
#[non_exhaustive]
pub struct CacheReport {
    /// The handles alive, of each priority: now, soon, later.
    pub handles: [u64; 3],
    /// The meshes that load, by the most urgent priority of their handles: now, soon, later.
    pub loading: [u64; 3],
    /// The meshes that could not be made, which the cache does not ask for again.
    pub failed: u64,
    /// The meshes, the pages they share counted.
    pub meshes: KindReport,
    /// The textures that the meshes' materials read.
    pub textures: KindReport,
}

/// What a cache holds of one kind of asset ([`CacheReport`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(name = "AssetKindReport")]
#[non_exhaustive]
pub struct KindReport {
    /// How many it holds that a handle holds, or that a mesh held needs.
    pub held: u64,
    /// How many it holds that nothing holds, which it drops, least recently held first, once past
    /// its budget.
    pub releasable: u64,
    /// The bytes of the GPU's memory that they take, as the cache counts them: what it created,
    /// before what the driver adds to align it, a mesh's whole page included.
    pub bytes: u64,
    /// The bytes that it keeps them within.
    pub budget: u64,
    /// The bytes that it cannot drop: those of the held ones, and, for meshes, of the pages that
    /// a held mesh shares with releasable ones, or a mesh that loads, or that a frame still draws.
    pub pinned: u64,
    /// How many it dropped to keep within its budget, since it was made.
    pub dropped: u64,
}

/// A mesh that the cache holds, with the cooked file it was read from, and the cache's frame in
/// which a handle last held it.
struct Cached {
    file: String,
    made: MadeMesh,
    used: u64,
}

/// A batch that loads, the meshes it loads, and the number it started with.
struct Batching {
    ids: Vec<AssetId>,
    number: u64,
    load: Load<Batch>,
}

/// The meshes that a game draws, on the GPU, by their asset IDs, with the materials they are drawn
/// with and the textures those read: what the world names by ID, it holds as GPU objects, so that
/// the world holds none (`docs/adr/0084-what-the-world-draws.md`).
///
/// A game, the bridge from its world to the renderer (`slime-world-render`) or an editor asks for
/// meshes through requests ([`AssetCache::request`]), each of a [`Priority`], which return handles
/// that hold them (ADR-0113, ADR-0128). The meshes that the cache neither holds nor loads start
/// loading in the background, as one batch per request, by tasks of the engine's pool, as
/// [`Renderer::load_scene_in_background`] loads a scene: the tasks of the most urgent class run
/// first, and their copies go to the GPU first, and within a class those of the requests made
/// first. [`AssetCache::poll`] takes the batches that finished, at each frame: a request's meshes
/// arrive together, each that cannot be made reported alone, by its handles. An asset that several
/// meshes share, a material or a texture, is loaded once and kept once; but two batches loading at
/// once that both read a texture the cache does not hold yet upload it each, and the cache keeps
/// the first.
///
/// - **Holding and letting go.** A mesh that a handle holds stays, whatever the budget. Once its
///   last handle is dropped, it becomes releasable: it stays while the budget of its kind has room,
///   so that what leaves the screen for a moment does not load again, and goes, least recently
///   held first, once the budget has none ([`AssetCache::set_budgets`]). A load whose meshes
///   nothing holds any more stops, without waiting, and those of its meshes that it has not read
///   are left out.
/// - **Budgets.** The cache counts the bytes of what it creates: the textures' levels
///   ([`Texture::bytes`]), and the pages that its meshes share, its own, apart from the renderer's
///   (ADR-0117), which it frees by dropping pages whose meshes are all releasable, least recently
///   held first. A texture goes with the releasable meshes that read it, when none that a handle
///   holds does. What it cannot drop, held or in a page that a held mesh shares, it reports
///   ([`AssetCache::report`]), and warns of when its kind first passes its budget. A page counts
///   while a mesh lives in it, one that a load places or that a frame still draws included, and
///   is not dropped while one that is not the cache's does. The count leaves out the driver's
///   alignment, the materials' small buffers, and a texture dropped while a frame still draws it:
///   it decides what to drop, and never refuses a creation (ADR-0044).
/// - **Errors.** A mesh that cannot be read, or whose material or texture cannot, fails alone: the
///   others of its request arrive (G16). Its handles say why ([`AssetState::Failed`]), or that it
///   is missing from the cooked data ([`AssetState::Missing`]), `poll` returns it once, and the
///   cache does not ask for it again until its cooked data changes.
/// - **Hot reload.** [`AssetCache::reload`] loads again, in the background, what handles hold and
///   new cooked data changed, keeping each mesh and texture whose cooked file the new manifest still
///   names (`docs/adr/0076-hot-reload-of-cooked-data.md`), and makes every material again; the
///   cache goes on giving what it held until the reload finishes, then takes its result at once.
///   Releasable meshes are dropped, and a mesh that the new data no longer lists is dropped once
///   the reload finishes, its handles saying it is missing.
/// - **A lost device.** A cache loads through the renderer that it was made with, and for its
///   device. A game gives its requests back ([`AssetCache::into_requests`]), which drops every GPU
///   object, then makes a new cache with the renderer that replaces it and those requests
///   ([`AssetCache::with_requests`]), which loads again what handles hold, at their priorities:
///   the handles stay valid.
///
/// It needs no window, and can be moved to another thread, or shared for reading. It keeps, from
/// its creation, what its loads take of the renderer: the parts of its materials and its uploader,
/// so that it loads without it (ADR-0111).
pub struct AssetCache {
    assets: CookedAssets,
    /// What its loads take of the renderer it was made with, with pages of its own.
    loader: Loader,
    /// What its handles hold.
    book: Arc<Mutex<Book>>,
    /// The meshes held.
    meshes: HashMap<AssetId, Cached>,
    /// The materials the meshes held are drawn with, and the textures each reads.
    materials: HashMap<AssetId, (Material, Vec<AssetId>)>,
    /// The textures those materials read, each with the cooked file it was read from.
    textures: HashMap<AssetId, (String, Texture)>,
    /// The batches being loaded.
    loads: Vec<Batching>,
    /// The meshes that a batch being loaded loads.
    loading: HashSet<AssetId>,
    /// Stopped loads, kept until none of their tasks runs, so that dropping them waits for none.
    stopped: Vec<Load<Batch>>,
    /// The reload of every mesh held, from new cooked data.
    reload: Option<Load<Batch>>,
    /// The meshes that failed, not asked for again until the cooked data changes, with why.
    failed: HashMap<AssetId, Arc<SceneError>>,
    budgets: Budgets,
    /// The number of its polls: its frames, which say how recently a mesh was held.
    frame: u64,
    /// Whether each kind, the textures' then the meshes', was past its budget at the last poll.
    over: (bool, bool),
    /// How many textures, then meshes, it dropped to keep within its budgets.
    dropped: (u64, u64),
}

impl AssetCache {
    /// An empty cache, which loads from `assets` what `renderer` draws, with the default
    /// [`Budgets`] and requests that hold nothing.
    #[must_use]
    pub fn new(renderer: &Renderer, assets: CookedAssets) -> Self {
        Self::with_requests(renderer, assets, AssetRequests::new())
    }

    /// A cache that loads from `assets` what `renderer` draws, with the default [`Budgets`], and
    /// what `requests`, given back by another cache ([`AssetCache::into_requests`]), hold: its
    /// first [`AssetCache::poll`] starts loading those meshes, one batch per priority, the most
    /// urgent first. The handles of `requests` hold their meshes in it from then on.
    #[must_use]
    pub fn with_requests(
        renderer: &Renderer,
        assets: CookedAssets,
        requests: AssetRequests,
    ) -> Self {
        Self {
            assets,
            loader: renderer.loader().apart(),
            book: requests.book,
            meshes: HashMap::new(),
            materials: HashMap::new(),
            textures: HashMap::new(),
            loads: Vec::new(),
            loading: HashSet::new(),
            stopped: Vec::new(),
            reload: None,
            failed: HashMap::new(),
            budgets: Budgets::default(),
            frame: 0,
            over: (false, false),
            dropped: (0, 0),
        }
    }

    /// Drops every GPU object that it holds, its loads stopped, waiting on the calling thread for
    /// their running tasks, a mesh or a texture each, and returns its requests: what a game does
    /// when its device is lost, before the new one opens (ADR-0075), to give them to the cache
    /// that replaces it ([`AssetCache::with_requests`]). The handles stay valid, their assets
    /// loading until that cache holds them.
    #[must_use]
    pub fn into_requests(self) -> AssetRequests {
        for asked in lock(&self.book).assets.values_mut() {
            asked.state = AssetState::Loading;
        }
        AssetRequests {
            book: Arc::clone(&self.book),
        }
    }

    /// The cooked data the cache loads from: the last given to [`AssetCache::reload`], or to
    /// [`AssetCache::new`].
    pub fn assets(&self) -> &CookedAssets {
        &self.assets
    }

    /// Whether `handle` holds its asset in this cache: whether a request of this cache gave it, or
    /// one of the cache whose requests it took ([`AssetCache::with_requests`]). A handle of another
    /// cache holds nothing in this one.
    pub fn issued(&self, handle: &AssetHandle) -> bool {
        Arc::ptr_eq(&self.book, &handle.book)
    }

    /// The mesh `id` and the material it is drawn with, if the cache holds them.
    pub fn mesh(&self, id: AssetId) -> Option<(&Mesh, &Material)> {
        let cached = self.meshes.get(&id)?;
        let (material, _) = self.materials.get(&cached.made.material)?;
        Some((&cached.made.mesh, material))
    }

    /// The material `id`, if a mesh that the cache holds is drawn with it. A game changes how
    /// every mesh drawn with it looks through it ([`Material::set_values`]): a reload makes it
    /// again, with the values of the cooked data, and dropping its last mesh drops it.
    pub fn material(&self, id: AssetId) -> Option<&Material> {
        self.materials.get(&id).map(|(material, _)| material)
    }

    /// The budgets it keeps its memory within.
    pub fn budgets(&self) -> Budgets {
        self.budgets
    }

    /// Changes the budgets it keeps its memory within, from its next [`AssetCache::poll`].
    pub fn set_budgets(&mut self, budgets: Budgets) {
        self.budgets = budgets;
    }

    /// Asks for the meshes `ids` at `priority`, with the materials they are drawn with and the
    /// textures those read, and returns the request that holds them. Those that the cache neither
    /// holds nor is loading, and that did not fail, start loading, as one batch in the background;
    /// the handles of those it holds say they are ready, of those that failed why, or that they are
    /// missing. An ID given twice is asked for once, with one handle.
    ///
    /// # Panics
    ///
    /// If the operating system cannot start the threads of the engine's pool, as
    /// [`Renderer::load_scene_in_background`] says; and when the batch's materials are made, if
    /// the cache holds textures that another device than `gpu`'s made: the cache of a lost device
    /// was kept, where a new one is made with the renderer that replaces it.
    pub fn request(
        &mut self,
        gpu: &Gpu,
        ids: impl IntoIterator<Item = AssetId>,
        priority: Priority,
    ) -> Request {
        let mut asked = HashSet::new();
        let mut handles = Vec::new();
        let mut wanted = Vec::new();
        for id in ids {
            if !asked.insert(id) {
                continue;
            }
            let handle = AssetHandle::new(&self.book, id, priority);
            let state = if self.meshes.contains_key(&id) {
                AssetState::Ready
            } else if let Some(error) = self.failed.get(&id) {
                failed_state(id, error)
            } else {
                if self.loading.insert(id) {
                    wanted.push(id);
                }
                AssetState::Loading
            };
            if let Some(entry) = lock(&self.book).assets.get_mut(&id) {
                entry.state = state;
            }
            handles.push(handle);
        }
        self.start(gpu, wanted, priority);
        Request { handles }
    }

    /// Starts loading `ids`, which [`AssetCache::loading`] counts already, as one batch at
    /// `priority`; nothing when there is none.
    fn start(&mut self, gpu: &Gpu, ids: Vec<AssetId>, priority: Priority) {
        if ids.is_empty() {
            return;
        }
        // The textures the cache holds are kept, not uploaded again.
        let reuse = Reusable {
            meshes: HashMap::new(),
            textures: self.textures.clone(),
        };
        let number = loads::number();
        let rank = UploadRank::new(loads::rank(priority.class(), number));
        let load = self
            .loader
            .start_batch(gpu, &self.assets, reuse, ids.clone(), rank);
        self.loads.push(Batching { ids, number, load });
    }

    /// At each frame: takes the batches that finished loading, and the reload if it finished;
    /// gives each load the priority of what holds it, stopping those that nothing holds; starts
    /// loading what handles hold that nothing loads; drops releasable meshes and textures past the
    /// budgets. Returns the meshes that failed in the batches taken, each with why, in no
    /// particular order: once each. Their handles say it, but those of a mesh that a reload could
    /// not make again, which keeps what the cache held, and is ready.
    ///
    /// # Panics
    ///
    /// With the panic of a task of a load, as [`SceneLoad::take`](crate::SceneLoad::take) says:
    /// the batch that panicked is then over, and its meshes are not asked for again, as meshes
    /// that failed; a reload that panicked is over, and leaves what the cache held whole. The
    /// errors of the batches taken before it are lost.
    pub fn poll(&mut self, gpu: &Gpu) -> Vec<(AssetId, Arc<SceneError>)> {
        self.frame += 1;
        let mut failed = Vec::new();
        let mut index = 0;
        while index < self.loads.len() {
            if !self.loads[index].load.is_finished() {
                index += 1;
                continue;
            }
            // Out of the cache before its result is taken, which raises its panic: the batch's
            // meshes are then failed, not asked for again.
            let Batching { ids, mut load, .. } = self.loads.swap_remove(index);
            for id in &ids {
                self.loading.remove(id);
                self.failed.insert(*id, Arc::new(SceneError::Panicked));
            }
            let batch = load
                .take()
                .expect("a finished load gives its result once")
                .expect("a batch's meshes fail alone, never the batch");
            for id in &ids {
                self.failed.remove(id);
            }
            failed.extend(self.take(batch, false));
        }
        if self.reload.as_ref().is_some_and(Load::is_finished) {
            let mut reload = self.reload.take().expect("a reload");
            let batch = reload
                .take()
                .expect("a finished load gives its result once")
                .expect("a batch's meshes fail alone, never the batch");
            failed.extend(self.take(batch, true));
        }
        self.stopped.retain(Load::is_running);
        self.follow_handles();
        self.load_what_is_held(gpu);
        self.keep_within_budgets();
        failed
    }

    /// Records in the book what became of the meshes, marks those that handles hold as used in
    /// this frame, and gives each load the most urgent priority of what holds it: a load that
    /// nothing holds stops, and leaves out the meshes it loads that nothing holds.
    fn follow_handles(&mut self) {
        let book = lock(&self.book);
        for (id, cached) in &mut self.meshes {
            if book.assets.contains_key(id) {
                cached.used = self.frame;
            }
        }
        let mut index = 0;
        while index < self.loads.len() {
            let batching = &self.loads[index];
            let priority = batching
                .ids
                .iter()
                .filter_map(|id| book.assets.get(id)?.priority())
                .min();
            if let Some(priority) = priority {
                batching
                    .load
                    .rank()
                    .set(loads::rank(priority.class(), batching.number));
                for id in &batching.ids {
                    if !book.assets.contains_key(id) {
                        batching.load.skip(*id);
                    }
                }
                index += 1;
                continue;
            }
            // Kept until none of its tasks runs, so that dropping it waits for none.
            let stopped = self.loads.swap_remove(index);
            stopped.load.stop();
            for id in &stopped.ids {
                self.loading.remove(id);
            }
            self.stopped.push(stopped.load);
        }
    }

    /// Starts loading what handles hold that the cache neither holds, nor loads, nor failed:
    /// what the requests given to a new cache hold, a mesh skipped and asked for again, one batch
    /// per priority, the most urgent first.
    fn load_what_is_held(&mut self, gpu: &Gpu) {
        let mut wanted: BTreeMap<Priority, Vec<AssetId>> = BTreeMap::new();
        {
            let mut book = lock(&self.book);
            for (&id, asked) in &mut book.assets {
                let Some(priority) = asked.priority() else {
                    continue;
                };
                asked.state = if self.meshes.contains_key(&id) {
                    AssetState::Ready
                } else if let Some(error) = self.failed.get(&id) {
                    failed_state(id, error)
                } else {
                    if self.loading.insert(id) {
                        wanted.entry(priority).or_default().push(id);
                    }
                    AssetState::Loading
                };
            }
        }
        for (priority, mut ids) in wanted {
            // In a stable order, whatever the map's.
            ids.sort_unstable();
            self.start(gpu, ids, priority);
        }
    }

    /// How many meshes the batches being loaded load: 0 once every mesh asked for is held, or
    /// failed. A game that shows a scene whole waits for 0. A reload is not counted
    /// ([`AssetCache::reloading`]).
    pub fn loading(&self) -> usize {
        self.loading.len()
    }

    /// Whether a reload goes on.
    pub fn reloading(&self) -> bool {
        self.reload.is_some()
    }

    /// What the cache holds and loads, against its budgets ([`CacheReport`]).
    #[must_use]
    pub fn report(&self) -> CacheReport {
        let book = lock(&self.book);
        let mut report = CacheReport::default();
        for asked in book.assets.values() {
            for priority in asked.handles.values() {
                report.handles[priority.class() as usize] += 1;
            }
        }
        for id in &self.loading {
            if let Some(priority) = book.assets.get(id).and_then(Asked::priority) {
                report.loading[priority.class() as usize] += 1;
            }
        }
        report.failed = self.failed.len() as u64;
        let held_textures = self.held_textures(&book);
        drop(book);
        let pages = self.pages();
        report.meshes = KindReport {
            held: self.meshes.len() as u64 - pages.releasable_meshes,
            releasable: pages.releasable_meshes,
            bytes: self.loader.page_bytes(),
            budget: self.budgets.meshes,
            pinned: pages.pinned,
            dropped: self.dropped.1,
        };
        let pinned: u64 = held_textures
            .iter()
            .filter_map(|id| Some(self.textures.get(id)?.1.bytes()))
            .sum();
        report.textures = KindReport {
            held: held_textures.len() as u64,
            releasable: self.textures.len() as u64 - held_textures.len() as u64,
            bytes: self.texture_bytes(),
            budget: self.budgets.textures,
            pinned,
            dropped: self.dropped.0,
        };
        report
    }

    /// Loads again, in the background, every mesh that a handle holds, from `assets`, new cooked
    /// data: each mesh and texture whose cooked file the manifest of `assets` still names is kept,
    /// the others are read and uploaded again, and every material is made again, with the values
    /// of the cooked data. Until the reload finishes, the cache gives what it held;
    /// [`AssetCache::poll`] then takes its result at once.
    ///
    /// The releasable meshes are dropped. A mesh that the reload cannot make again keeps what the
    /// cache held, but one that the manifest of `assets` no longer lists, which nothing can read
    /// again: it is dropped once the reload finishes, its handles saying it is missing. `poll`
    /// returns the error of each.
    ///
    /// The batches being loaded stop, without waiting, as those that nothing holds, since they read
    /// the cooked data before: what handles hold of them is asked for again at the next poll. A
    /// reload going on stops too, and the meshes that failed may be asked for again.
    ///
    /// # Panics
    ///
    /// As [`AssetCache::request`].
    pub fn reload(&mut self, gpu: &Gpu, assets: CookedAssets) {
        self.assets = assets;
        self.failed.clear();
        for batching in std::mem::take(&mut self.loads) {
            batching.load.stop();
            self.stopped.push(batching.load);
        }
        if let Some(reload) = self.reload.take() {
            reload.stop();
            self.stopped.push(reload);
        }
        self.loading.clear();
        {
            let mut book = lock(&self.book);
            self.meshes.retain(|id, _| book.assets.contains_key(id));
            for (id, asked) in &mut book.assets {
                // What the cache holds, it gives until the reload's result replaces it.
                asked.state = if self.meshes.contains_key(id) {
                    AssetState::Ready
                } else {
                    AssetState::Loading
                };
            }
        }
        self.drop_unused();
        if self.meshes.is_empty() {
            return;
        }
        let ids: Vec<AssetId> = self.meshes.keys().copied().collect();
        let reuse = Reusable {
            meshes: self
                .meshes
                .iter()
                .map(|(&id, cached)| (id, (cached.file.clone(), cached.made.clone())))
                .collect(),
            textures: self.textures.clone(),
        };
        let rank = UploadRank::new(loads::rank(Priority::Now.class(), loads::number()));
        self.reload = Some(self.loader.start_batch(gpu, &self.assets, reuse, ids, rank));
    }

    /// Takes what a batch made, in addition to what the cache holds; returns the meshes that
    /// failed. A batch's material that the cache holds already is dropped, so that the material a
    /// game changes stays the one every mesh drawn with it uses; but a reload's replaces it, made
    /// from the new data, as its meshes and textures replace theirs. A mesh that the reload could
    /// not make keeps what the cache held, but one that the new data no longer names.
    fn take(&mut self, batch: Batch, reload: bool) -> Vec<(AssetId, Arc<SceneError>)> {
        let held: HashSet<AssetId> = lock(&self.book).assets.keys().copied().collect();
        for (id, made) in batch.meshes {
            let cached = Cached {
                file: cooked_file(&self.assets, id),
                made,
                used: if held.contains(&id) { self.frame } else { 0 },
            };
            self.meshes.insert(id, cached);
        }
        let mut reads = batch.reads;
        for (id, material) in batch.materials {
            if reload || !self.materials.contains_key(&id) {
                let textures = reads.remove(&id).unwrap_or_default();
                self.materials.insert(id, (material, textures));
            }
        }
        for (id, texture) in batch.textures {
            if reload || !self.textures.contains_key(&id) {
                self.textures
                    .insert(id, (cooked_file(&self.assets, id), texture));
            }
        }
        for (id, error) in &batch.failed {
            if matches!(failed_state(*id, error), AssetState::Missing) {
                self.meshes.remove(id);
            }
            self.failed.insert(*id, Arc::clone(error));
        }
        tracing::debug!(
            meshes = self.meshes.len(),
            materials = self.materials.len(),
            textures = self.textures.len(),
            kept_meshes = batch.kept_meshes,
            kept_textures = batch.kept_textures,
            failed = batch.failed.len(),
            reload,
            "asset cache took a batch"
        );
        self.drop_unused();
        batch.failed
    }

    /// Drops the materials that no mesh held is drawn with, and the textures that no material held
    /// reads.
    fn drop_unused(&mut self) {
        let drawn: HashSet<AssetId> = self
            .meshes
            .values()
            .map(|cached| cached.made.material)
            .collect();
        self.materials.retain(|id, _| drawn.contains(id));
        let read: HashSet<AssetId> = self
            .materials
            .values()
            .flat_map(|(_, textures)| textures.iter().copied())
            .collect();
        self.textures.retain(|id, _| read.contains(id));
    }

    /// The bytes of the textures held.
    fn texture_bytes(&self) -> u64 {
        self.textures
            .values()
            .map(|(_, texture)| texture.bytes())
            .sum()
    }

    /// The textures that a mesh that a handle holds reads, through its material.
    fn held_textures(&self, book: &Book) -> HashSet<AssetId> {
        self.meshes
            .iter()
            .filter(|(id, _)| book.assets.contains_key(id))
            .filter_map(|(_, cached)| self.materials.get(&cached.made.material))
            .flat_map(|(_, textures)| textures.iter().copied())
            .collect()
    }

    /// The pages of the meshes held, as [`AssetCache::keep_within_budgets`] drops them. A page
    /// that holds a mesh that is not the cache's, one that a load places or one that the cache
    /// dropped and a frame still draws, counts as held: dropping the cache's meshes would not free
    /// it yet.
    fn pages(&self) -> Pages {
        let lives = self.loader.page_meshes();
        let book = lock(&self.book);
        let mut pages: HashMap<*const crate::mesh::Page, Page> = HashMap::new();
        for (&id, cached) in &self.meshes {
            let page_of = &cached.made.mesh.place.page;
            let page = pages.entry(Arc::as_ptr(page_of)).or_insert_with(|| Page {
                bytes: page_of.bytes,
                meshes: Vec::new(),
                held: false,
                used: 0,
            });
            page.meshes.push(id);
            page.held |= book.assets.contains_key(&id);
            page.used = page.used.max(cached.used);
        }
        for (address, page) in &mut pages {
            page.held |= lives
                .get(address)
                .is_some_and(|&lives| lives > page.meshes.len());
        }
        let releasable_meshes = pages
            .values()
            .flat_map(|page| &page.meshes)
            .filter(|id| !book.assets.contains_key(id))
            .count() as u64;
        let pinned = pages
            .values()
            .filter(|page| page.held)
            .map(|page| page.bytes)
            .sum();
        Pages {
            pages: pages.into_values().collect(),
            releasable_meshes,
            pinned,
        }
    }

    /// Drops releasable meshes and textures, least recently held first, while a kind is past its
    /// budget: for the meshes, pages whose meshes are all releasable, whole; for the textures,
    /// those that only releasable meshes read, with those meshes. Warns once when a kind passes its
    /// budget and cannot be brought back within it.
    fn keep_within_budgets(&mut self) {
        let before = (self.textures.len(), self.meshes.len());
        let mut over = self.loader.page_bytes().saturating_sub(self.budgets.meshes);
        if over > 0 {
            let mut pages: Vec<Page> = self
                .pages()
                .pages
                .into_iter()
                .filter(|page| !page.held)
                .collect();
            pages.sort_by_key(|page| page.used);
            for page in pages {
                if over == 0 {
                    break;
                }
                for id in &page.meshes {
                    self.meshes.remove(id);
                }
                over = over.saturating_sub(page.bytes);
            }
            self.drop_unused();
        }
        let mesh_over = over > 0;
        let mut over = self.texture_bytes().saturating_sub(self.budgets.textures);
        if over > 0 {
            let held = self.held_textures(&lock(&self.book));
            // Each releasable texture, the meshes that read it, and when one was last held.
            let mut readers: HashMap<AssetId, (Vec<AssetId>, u64)> = HashMap::new();
            for (&id, cached) in &self.meshes {
                let Some((_, textures)) = self.materials.get(&cached.made.material) else {
                    continue;
                };
                for texture in textures.iter().filter(|texture| !held.contains(texture)) {
                    let (meshes, used) = readers.entry(*texture).or_default();
                    meshes.push(id);
                    *used = (*used).max(cached.used);
                }
            }
            let mut releasable: Vec<(AssetId, Vec<AssetId>, u64)> = readers
                .into_iter()
                .map(|(texture, (meshes, used))| (texture, meshes, used))
                .collect();
            releasable.sort_by_key(|&(texture, _, used)| (used, texture));
            for (texture, meshes, _) in releasable {
                if over == 0 {
                    break;
                }
                let Some((_, made)) = self.textures.get(&texture) else {
                    continue;
                };
                over = over.saturating_sub(made.bytes());
                for id in &meshes {
                    self.meshes.remove(id);
                }
                self.drop_unused();
            }
        }
        // What went with what was dropped is counted too: the textures of a page's meshes.
        self.dropped.0 += (before.0 - self.textures.len()) as u64;
        self.dropped.1 += (before.1 - self.meshes.len()) as u64;
        let over = (over > 0, mesh_over);
        if over.0 && !self.over.0 {
            tracing::warn!(
                bytes = self.texture_bytes(),
                budget = self.budgets.textures,
                "the textures that handles hold pass the asset cache's budget"
            );
        }
        if over.1 && !self.over.1 {
            tracing::warn!(
                bytes = self.loader.page_bytes(),
                budget = self.budgets.meshes,
                "the meshes that handles hold, and the pages they share, pass the asset cache's \
                 budget"
            );
        }
        self.over = over;
    }
}

/// What became of the mesh `id`, which failed with `error`: missing, if it is the mesh that the
/// cooked data does not name, not one of its material or textures.
fn failed_state(id: AssetId, error: &Arc<SceneError>) -> AssetState {
    match &**error {
        SceneError::Load(LoadError::NotCooked { id: missing, .. }) if *missing == id => {
            AssetState::Missing
        }
        _ => AssetState::Failed(Arc::clone(error)),
    }
}

/// The pages of a cache's meshes ([`AssetCache::pages`]).
struct Pages {
    pages: Vec<Page>,
    /// The meshes that no handle holds.
    releasable_meshes: u64,
    /// The bytes of the pages that cannot be freed yet ([`Page::held`]).
    pinned: u64,
}

/// A page of a cache's meshes: its bytes, the cache's meshes it holds, whether it cannot be freed
/// yet, a handle holding one of them or another mesh living in it, and the last frame in which one
/// was held.
struct Page {
    bytes: u64,
    meshes: Vec<AssetId>,
    held: bool,
    used: u64,
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use slime_gfx::TextureFormat;

    use super::*;
    use crate::cooked::tests::squares;
    use crate::loads::LOADS_AT_ONCE;
    use crate::loads::tests::HOLDING_THE_POOL;

    /// How long a test waits for what must happen.
    const PATIENCE: Duration = Duration::from_secs(10);

    /// The `index`th square of `squares`.
    fn square(index: u128) -> AssetId {
        AssetId::from_u128(100 + index)
    }

    /// Takes every place of the loads' lane with tasks that wait until the senders returned are
    /// dropped, so that the tasks of loads wait in the lane meanwhile; `None` on a machine whose
    /// pool has too few background threads for it.
    fn hold_the_lane() -> Option<Vec<mpsc::Sender<()>>> {
        // Two tasks that hold the lane take two of the pool's background threads, which the pool
        // gives from three threads on (`slime_tasks::threads`).
        if slime_tasks::threads() < 3 {
            return None;
        }
        let holders = UploadRank::new(0);
        let (started, all_started) = mpsc::channel();
        let releases = (0..LOADS_AT_ONCE)
            .map(|_| {
                let (release, released) = mpsc::channel::<()>();
                let started = started.clone();
                loads::run(&holders, move || {
                    started.send(()).expect("the test waits");
                    let _ = released.recv();
                });
                release
            })
            .collect();
        for _ in 0..LOADS_AT_ONCE {
            all_started.recv_timeout(PATIENCE).expect("the holders run");
        }
        Some(releases)
    }

    /// Polls `cache` until nothing loads and the loads it stopped have ended.
    fn settle(cache: &mut AssetCache, gpu: &Gpu) {
        for _ in 0..10_000 {
            cache.poll(gpu);
            if cache.loading() == 0 && cache.stopped.is_empty() {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("the cache still loads after 10 s");
    }

    /// Each request's load takes the rank of its class, then of its start among the loads: a
    /// handle raised from soon to now moves its load ahead of those of soon, behind the loads of
    /// now started before; and the lane and the uploader serve the lowest rank first
    /// (`loads::tests`, `slime-gfx`'s `tests/uploads.rs`).
    #[test]
    fn each_load_takes_the_rank_of_its_most_urgent_handle() {
        let _holding = HOLDING_THE_POOL
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(releases) = hold_the_lane() else {
            return;
        };
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        let (assets, _) = squares("cache-ranks", 4);
        let mut cache = AssetCache::new(&renderer, assets);
        let later = cache.request(&gpu, [square(0)], Priority::Later);
        let soon = cache.request(&gpu, [square(1)], Priority::Soon);
        let now = cache.request(&gpu, [square(2)], Priority::Now);
        let raised = cache.request(&gpu, [square(3)], Priority::Soon);
        raised.set_priority(Priority::Now);
        cache.poll(&gpu);
        let rank_of = |id: AssetId| {
            let batching = cache
                .loads
                .iter()
                .find(|batching| batching.ids.contains(&id))
                .expect("loading");
            (batching.load.rank().get(), batching.number)
        };
        let mut ranks: Vec<(u64, u64, &str)> = [
            (square(0), "later"),
            (square(1), "soon"),
            (square(2), "now"),
            (square(3), "raised"),
        ]
        .into_iter()
        .map(|(id, name)| {
            let (rank, number) = rank_of(id);
            (rank, number, name)
        })
        .collect();
        ranks.sort_unstable();
        let order: Vec<&str> = ranks.iter().map(|&(.., name)| name).collect();
        assert_eq!(order, ["now", "raised", "soon", "later"]);
        for (rank, number, name) in &ranks {
            let class = match *name {
                "now" | "raised" => 0,
                "soon" => 1,
                _ => 2,
            };
            assert_eq!(*rank, loads::rank(class, *number), "{name}");
        }
        drop(releases);
        settle(&mut cache, &gpu);
        assert!([later, soon, now, raised].iter().all(Request::is_done));
    }

    /// The last handle of a load's meshes dropped while its tasks wait stops it without waiting
    /// for them; one of a load's meshes let go of is left out of it, the others arriving.
    #[test]
    fn a_load_that_nothing_holds_stops_and_a_mesh_let_go_of_is_left_out() {
        let _holding = HOLDING_THE_POOL
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(releases) = hold_the_lane() else {
            return;
        };
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        let (assets, _) = squares("cache-stopped", 4);
        let mut cache = AssetCache::new(&renderer, assets);
        let dropped = cache.request(&gpu, [square(0), square(1)], Priority::Now);
        let mut kept = cache
            .request(&gpu, [square(2), square(3)], Priority::Now)
            .into_handles();
        assert_eq!(cache.loading(), 4);
        drop(dropped);
        let let_go = kept.pop().expect("square 3's");
        drop(let_go);
        cache.poll(&gpu);
        assert_eq!(cache.loading(), 2, "the first load stopped");
        assert_eq!(cache.stopped.len(), 1, "kept until its tasks end");
        assert_eq!(cache.report().loading, [1, 0, 0]);
        drop(releases);
        settle(&mut cache, &gpu);
        let held: Vec<bool> = (0..4)
            .map(|index| cache.mesh(square(index)).is_some())
            .collect();
        assert_eq!(held, [false, false, true, false]);
        assert!(matches!(kept[0].state(), AssetState::Ready));
    }
}
