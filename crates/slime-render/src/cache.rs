//! The asset cache: the GPU objects of the meshes that a game draws, by asset ID, outside any world
//! (`docs/adr/0084-what-the-world-draws.md`).

use std::collections::{HashMap, HashSet};

use slime_assets::{AssetId, CookedAssets};
use slime_gfx::{Gpu, Texture};

use crate::cooked::{Batch, Load, MadeMesh, Reusable, SceneError, cooked_file};
use crate::material::Material;
use crate::mesh::Mesh;
use crate::renderer::Renderer;

/// The meshes that a game draws, on the GPU, by their asset IDs, with the materials they are drawn
/// with and the textures those read: what the world names by ID, it holds as GPU objects, so that
/// the world holds none (`docs/adr/0084-what-the-world-draws.md`).
///
/// A game asks for the meshes it draws at each frame ([`AssetCache::request`]); those the cache
/// neither holds nor is loading start loading in the background, as a batch, on the renderer's
/// threads, as [`Renderer::load_scene_in_background`] loads a scene. [`AssetCache::poll`] takes the
/// batches that finished, each whole: a mesh is drawn once its batch is, never a part of a batch.
/// An asset that several meshes share, a material or a texture, is loaded once and kept once; but
/// two batches loading at once that both read a texture the cache does not hold yet upload it each,
/// and the cache keeps both until a reload.
///
/// - **Errors.** A batch that fails gives its error to `poll`, and none of its meshes is held; the
///   cache does not ask for them again until its cooked data changes ([`AssetCache::reload`]).
/// - **Hot reload.** [`AssetCache::reload`] loads again, in the background, what changed in new
///   cooked data, keeping each mesh and texture whose cooked file the new manifest still names
///   (`docs/adr/0076-hot-reload-of-cooked-data.md`), and makes every material again; the cache
///   goes on giving what it holds until the reload finishes, then takes its result at once.
/// - **A lost device.** [`AssetCache::clear`] drops every GPU object it holds and cancels its
///   loads; what a game draws afterwards loads again, from the GPU that replaced the lost one,
///   those whose batch failed included.
/// - **Memory.** It keeps what it loaded until it is cleared: nothing is dropped when no one draws
///   it any more.
///
/// It holds no GPU object of its own beyond those, needs no window, and can be moved to another
/// thread, or shared for reading.
pub struct AssetCache {
    assets: CookedAssets,
    /// The meshes held, each with the cooked file it was read from.
    meshes: HashMap<AssetId, (String, MadeMesh)>,
    /// The materials the meshes held are drawn with.
    materials: HashMap<AssetId, Material>,
    /// The textures those materials read, each with the cooked file it was read from.
    textures: HashMap<AssetId, (String, Texture)>,
    /// The batches being loaded, each with the meshes it loads.
    loads: Vec<(Vec<AssetId>, Load<Batch>)>,
    /// The meshes that a batch being loaded loads.
    loading: HashSet<AssetId>,
    /// The reload of every mesh held, from new cooked data.
    reload: Option<Load<Batch>>,
    /// The meshes whose batch failed, not asked for again until the cooked data changes.
    failed: HashSet<AssetId>,
}

impl AssetCache {
    /// An empty cache, which loads from `assets`.
    #[must_use]
    pub fn new(assets: CookedAssets) -> Self {
        Self {
            assets,
            meshes: HashMap::new(),
            materials: HashMap::new(),
            textures: HashMap::new(),
            loads: Vec::new(),
            loading: HashSet::new(),
            reload: None,
            failed: HashSet::new(),
        }
    }

    /// The cooked data the cache loads from: the last given to [`AssetCache::reload`], or to
    /// [`AssetCache::new`].
    pub fn assets(&self) -> &CookedAssets {
        &self.assets
    }

    /// The mesh `id` and the material it is drawn with, if the cache holds them.
    pub fn mesh(&self, id: AssetId) -> Option<(&Mesh, &Material)> {
        let (_, made) = self.meshes.get(&id)?;
        Some((&made.mesh, self.materials.get(&made.material)?))
    }

    /// The material `id`, if a mesh that the cache holds is drawn with it. A game changes how
    /// every mesh drawn with it looks through it ([`Material::set_values`]): a reload makes it
    /// again, with the values of the cooked data.
    pub fn material(&self, id: AssetId) -> Option<&Material> {
        self.materials.get(&id)
    }

    /// Starts loading, as one batch in the background, those of the meshes `ids` that the cache
    /// neither holds nor is loading, and whose earlier batch did not fail; with the materials they
    /// are drawn with, and the textures those read that the cache does not hold. Starts nothing
    /// when there is none.
    ///
    /// # Panics
    ///
    /// If the operating system cannot start the batch's thread, as
    /// [`Renderer::load_scene_in_background`] says; and when the batch's materials are made, if
    /// the cache holds textures that another device than `gpu`'s made: a lost device was not
    /// followed by [`AssetCache::clear`].
    pub fn request(
        &mut self,
        renderer: &Renderer,
        gpu: &Gpu,
        ids: impl IntoIterator<Item = AssetId>,
    ) {
        let mut wanted = Vec::new();
        for id in ids {
            let known = self.meshes.contains_key(&id)
                || self.failed.contains(&id)
                || !self.loading.insert(id);
            if !known {
                wanted.push(id);
            }
        }
        if wanted.is_empty() {
            return;
        }
        // The textures the cache holds are kept, not uploaded again.
        let reuse = Reusable {
            meshes: HashMap::new(),
            textures: self.textures.clone(),
        };
        let ids = wanted.clone();
        let load = renderer.start(
            "slime-render asset load",
            gpu,
            &self.assets,
            reuse,
            move |loader, reports| loader.load_meshes(&ids, reports),
        );
        self.loads.push((wanted, load));
    }

    /// Takes the batches that finished loading, and the reload if it finished; returns the errors
    /// of those that failed, in no particular order.
    ///
    /// # Errors
    ///
    /// Each error is [`SceneError::Load`] for an asset that cannot be read from the cooked data, or
    /// [`SceneError::Render`] for an object that the GPU cannot create, as
    /// [`Renderer::load_scene`] says; never [`SceneError::Placement`]. A batch that fails holds
    /// nothing; a reload that fails leaves what the cache held whole.
    ///
    /// # Panics
    ///
    /// With the panic of a loading thread, as [`SceneLoad::take`](crate::SceneLoad::take) says.
    pub fn poll(&mut self) -> Vec<SceneError> {
        let mut errors = Vec::new();
        let mut index = 0;
        while index < self.loads.len() {
            let Some(result) = self.loads[index].1.take() else {
                index += 1;
                continue;
            };
            let (ids, _) = self.loads.swap_remove(index);
            for id in &ids {
                self.loading.remove(id);
            }
            match result {
                Ok(batch) => self.take(batch, false),
                Err(error) => {
                    self.failed.extend(ids);
                    errors.push(error);
                }
            }
        }
        if let Some(result) = self.reload.as_mut().and_then(Load::take) {
            self.reload = None;
            match result {
                Ok(batch) => self.take(batch, true),
                Err(error) => errors.push(error),
            }
        }
        errors
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

    /// Loads again, in the background, every mesh the cache holds, from `assets`, new cooked data:
    /// each mesh and texture whose cooked file the manifest of `assets` still names is kept, the
    /// others are read and uploaded again, and every material is made again, with the values of
    /// the cooked data. Until the reload finishes, the cache gives what it held;
    /// [`AssetCache::poll`] then takes its result at once, or its error, which leaves what the
    /// cache held whole.
    ///
    /// The batches being loaded are dropped, since they read the cooked data before: the meshes
    /// they loaded are asked for again. A reload going on is dropped too, and the meshes whose
    /// batch failed may be asked for again. Dropping a load waits, on the calling thread, for its
    /// tasks that are running, a mesh or a texture each: a game that reloads once nothing loads
    /// ([`AssetCache::loading`]) waits for none.
    ///
    /// # Panics
    ///
    /// As [`AssetCache::request`].
    pub fn reload(&mut self, renderer: &Renderer, gpu: &Gpu, assets: CookedAssets) {
        self.assets = assets;
        self.failed.clear();
        self.loads.clear();
        self.loading.clear();
        self.reload = None;
        if self.meshes.is_empty() {
            return;
        }
        let ids: Vec<AssetId> = self.meshes.keys().copied().collect();
        let reuse = Reusable {
            meshes: self.meshes.clone(),
            textures: self.textures.clone(),
        };
        self.reload = Some(renderer.start(
            "slime-render asset reload",
            gpu,
            &self.assets,
            reuse,
            move |loader, reports| loader.load_meshes(&ids, reports),
        ));
    }

    /// Drops every GPU object the cache holds, and cancels its loads and its reload, as dropping a
    /// [`SceneLoad`](crate::SceneLoad) cancels its own: once it returns, no thread of the cache
    /// holds the `Gpu`; it waits, on the calling thread, for the loads' tasks that are running.
    /// What a game draws afterwards loads again, the meshes whose batch failed included. A game
    /// clears its cache when its device is lost (`App::gpu_lost` in `slime-app`).
    pub fn clear(&mut self) {
        self.loads.clear();
        self.loading.clear();
        self.reload = None;
        self.meshes.clear();
        self.materials.clear();
        self.textures.clear();
        self.failed.clear();
    }

    /// Takes what a batch made: in addition to what the cache holds, or, for a reload, in place of
    /// it. A batch's material that the cache holds already is dropped, so that the material a game
    /// changes stays the one every mesh drawn with it uses.
    fn take(&mut self, batch: Batch, reload: bool) {
        for (id, mesh) in batch.meshes {
            self.meshes
                .insert(id, (cooked_file(&self.assets, id), mesh));
        }
        for (id, material) in batch.materials {
            if reload || !self.materials.contains_key(&id) {
                self.materials.insert(id, material);
            }
        }
        for (id, texture) in batch.textures {
            if reload || !self.textures.contains_key(&id) {
                self.textures
                    .insert(id, (cooked_file(&self.assets, id), texture));
            }
        }
        tracing::debug!(
            meshes = self.meshes.len(),
            materials = self.materials.len(),
            textures = self.textures.len(),
            kept_meshes = batch.kept_meshes,
            kept_textures = batch.kept_textures,
            reload,
            "asset cache took a batch"
        );
    }
}
