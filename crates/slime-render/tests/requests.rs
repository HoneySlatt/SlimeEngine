//! The asset cache (`docs/adr/0113-asset-requests-residency-and-priorities.md`,
//! `docs/adr/0128-asset-requests.md`), on the cooked squares of `common/palette.rs`: it loads what
//! is asked for once, in batches, keeps the textures it holds, does not ask again for what failed or
//! panicked, and loads and keeps meshes once the renderer it was made with is dropped; a mesh that
//! cannot be read fails alone (G16); a request's meshes stay while a handle holds them, whatever
//! the budget, and become releasable once none does; past its budgets, it drops the releasable
//! textures and pages least recently held first, never a held one, and reports what it cannot
//! drop; its handles stay valid through new cooked data and a lost device; a long session of
//! requests and releases stays within its budgets. The order of the classes and cancelled loads are
//! checked by `cache.rs`'s unit tests. Runs on whatever adapter the machine offers.

#[path = "common/palette.rs"]
mod palette;

use std::fs;
use std::sync::Arc;

use palette::{COLORS, center_of, palette, palette_meshes, settle, whiten_the_second_texture};
use slime_assets::{AssetId, CookedAssets, LoadError, MANIFEST_FILE, Manifest};
use slime_gfx::{Gpu, TextureFormat};
use slime_render::glam::{DAffine3, DVec3};
use slime_render::{
    AssetCache, AssetHandle, AssetState, Budgets, Instance, Priority, Renderer, SceneError,
};

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// A renderer, and a cache of the squares of a new palette named `name`, whose textures are 4
/// texels wide, 16 bytes each.
fn cache(gpu: &Gpu, name: &str) -> (Renderer, AssetCache, std::path::PathBuf) {
    let renderer = Renderer::new(gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let (directory, _) = palette(name, 4);
    let cache = AssetCache::new(
        &renderer,
        CookedAssets::open(&directory).expect("the manifest is read"),
    );
    (renderer, cache, directory)
}

/// Checks that `cache` holds each of the palette's squares in `indices`, and that each is drawn in
/// its material's color, or white where `white` says so.
fn check_cached(
    gpu: &Gpu,
    renderer: &mut Renderer,
    cache: &AssetCache,
    indices: impl IntoIterator<Item = usize>,
    white: impl Fn(usize) -> bool,
) {
    let meshes = palette_meshes();
    for index in indices {
        let (mesh, material) = cache.mesh(meshes[index]).expect("held");
        let placed = DAffine3::from_translation(DVec3::new(2.0 * index as f64, 0.0, 0.0));
        let [r, g, b] = center_of(gpu, renderer, Instance::new(mesh, material, placed));
        let lit = [r, g, b].map(|channel| channel > 100);
        let expected = if white(index) {
            [true; 3]
        } else {
            COLORS[index % 3].map(|channel| channel == 255)[..3]
                .try_into()
                .expect("three channels")
        };
        assert_eq!(lit, expected, "{index}: {r} {g} {b}");
    }
}

/// The handles of the palette's six squares, asked for now, each let go of by setting it to `None`.
fn held(cache: &mut AssetCache, gpu: &Gpu) -> Vec<Option<AssetHandle>> {
    cache
        .request(gpu, palette_meshes(), Priority::Now)
        .into_handles()
        .into_iter()
        .map(Some)
        .collect()
}

/// What became of the square `index`, whose handle `handles` holds.
fn state(handles: &[Option<AssetHandle>], index: usize) -> AssetState {
    handles[index].as_ref().expect("held").state()
}

/// Whether every handle says it is ready.
fn ready(handles: &[AssetHandle]) -> bool {
    handles
        .iter()
        .all(|handle| matches!(handle.state(), AssetState::Ready))
}

#[test]
fn the_cache_loads_what_is_asked_for_once_and_shares_the_materials() {
    let gpu = gpu();
    let (mut renderer, mut cache, _) = cache(&gpu, "loads-once");
    let meshes = palette_meshes();
    assert!(cache.mesh(meshes[0]).is_none());
    let first = cache.request(&gpu, meshes, Priority::Now);
    assert_eq!(cache.loading(), 6);
    let second = cache.request(&gpu, meshes, Priority::Now);
    assert_eq!(cache.loading(), 6, "asked for twice, loaded once");
    assert!(settle(&mut cache, &gpu).is_empty());
    assert!(first.is_done() && ready(first.handles()) && ready(second.handles()));
    // Squares 0 and 3 are drawn with the same material, which the cache holds once.
    let material = |index: usize| cache.mesh(meshes[index]).expect("held").1;
    assert!(std::ptr::eq(material(0), material(3)));
    assert!(!std::ptr::eq(material(0), material(1)));
    assert!(std::ptr::eq(
        material(1),
        cache.material(AssetId::from_u128(201)).expect("held")
    ));
    let third = cache.request(&gpu, meshes, Priority::Now);
    assert_eq!(cache.loading(), 0, "held already");
    assert!(ready(third.handles()));
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
}

#[test]
fn a_second_batch_keeps_the_textures_that_the_cache_holds() {
    let gpu = gpu();
    let (mut renderer, mut cache, directory) = cache(&gpu, "second-batch");
    let meshes = palette_meshes();
    let _first = cache.request(&gpu, meshes[..3].iter().copied(), Priority::Now);
    assert!(settle(&mut cache, &gpu).is_empty());
    // The last three squares are drawn with the same materials, whose textures the cache holds:
    // those are not read again.
    for index in 0..3 {
        fs::remove_file(directory.join(format!("{index}.ktx2"))).expect("removed");
    }
    let _second = cache.request(&gpu, meshes[3..].iter().copied(), Priority::Now);
    assert_eq!(cache.loading(), 3);
    assert!(settle(&mut cache, &gpu).is_empty());
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
}

/// G16: a mesh that cannot be read, or whose texture cannot, fails alone; the others of its
/// request arrive. It is reported once, by its handles too, and not asked for again.
#[test]
fn a_mesh_that_cannot_be_read_fails_alone_and_is_not_asked_for_again() {
    let gpu = gpu();
    let (mut renderer, mut cache, directory) = cache(&gpu, "fails-alone");
    let meshes = palette_meshes();
    // Square 1's file is gone, and the texture of squares 2 and 5.
    fs::remove_file(directory.join("1.mesh")).expect("removed");
    fs::remove_file(directory.join("2.ktx2")).expect("removed");
    let request = cache.request(&gpu, meshes, Priority::Now);
    let mut errors = settle(&mut cache, &gpu);
    errors.sort_by_key(|&(id, _)| id);
    let failed: Vec<AssetId> = errors.iter().map(|&(id, _)| id).collect();
    assert_eq!(failed, [meshes[1], meshes[2], meshes[5]], "{errors:?}");
    assert!(
        errors
            .iter()
            .all(|(_, error)| matches!(&**error, SceneError::Load(_))),
        "{errors:?}"
    );
    assert!(request.is_done());
    for (index, handle) in request.handles().iter().enumerate() {
        let failed = matches!(handle.state(), AssetState::Failed(_));
        assert_eq!(failed, [1, 2, 5].contains(&index), "{index}");
    }
    check_cached(&gpu, &mut renderer, &cache, [0, 3, 4], |_| false);
    // Asked for again, it is not loaded again, and the handle says why at once.
    let again = cache.request(&gpu, [meshes[1]], Priority::Now);
    assert_eq!(cache.loading(), 0);
    assert!(matches!(again.handles()[0].state(), AssetState::Failed(_)));
    assert!(cache.poll(&gpu).is_empty(), "reported once");
    assert_eq!(cache.report().failed, 3);
}

#[test]
fn a_panic_while_the_cache_loads_leaves_it_loading_nothing() {
    let gpu = gpu();
    let (_renderer, mut cache, _) = cache(&gpu, "panic");
    let meshes = palette_meshes();
    let _first = cache.request(&gpu, [meshes[0]], Priority::Now);
    assert!(settle(&mut cache, &gpu).is_empty());
    // Square 3 is drawn with square 0's material, which a batch makes again with the texture the
    // cache holds: on another device, which cannot read it, a bug of the caller's that panics.
    let other = self::gpu();
    let panicking = cache.request(&other, [meshes[3]], Priority::Now);
    let panicked =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| settle(&mut cache, &gpu)));
    assert!(panicked.is_err(), "the batch panicked");
    assert_eq!(cache.loading(), 0, "the batch is over");
    assert!(cache.mesh(meshes[3]).is_none());
    drop(panicking);
    // Its mesh is not asked for again, as a mesh that failed.
    let again = cache.request(&gpu, [meshes[3]], Priority::Now);
    assert_eq!(cache.loading(), 0);
    assert!(matches!(
        again.handles()[0].state(),
        AssetState::Failed(error) if matches!(*error, SceneError::Panicked)
    ));
}

#[test]
fn a_cache_loads_and_keeps_meshes_without_the_renderer_it_was_made_with() {
    let gpu = gpu();
    let (renderer, mut dropped, directory) = cache(&gpu, "alone");
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    // A cache dropped while a batch loads stops it.
    let _held = dropped.request(&gpu, palette_meshes(), Priority::Now);
    drop(dropped);
    let mut cache = AssetCache::new(&renderer, assets);
    drop(renderer);
    let meshes = palette_meshes();
    let _held = cache.request(&gpu, meshes, Priority::Now);
    assert!(settle(&mut cache, &gpu).is_empty());
    // Drawn by another renderer of the same device.
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
}

#[test]
fn a_reload_keeps_what_did_not_change_and_takes_the_rest_at_once() {
    let gpu = gpu();
    let (mut renderer, mut cache, directory) = cache(&gpu, "reload");
    let meshes = palette_meshes();
    let held = cache.request(&gpu, meshes, Priority::Now);
    assert!(settle(&mut cache, &gpu).is_empty());
    whiten_the_second_texture(&directory);
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    cache.reload(&gpu, assets);
    // Until the reload finishes, the cache gives what it held, and the handles say so.
    assert!(cache.reloading());
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
    assert!(ready(held.handles()));
    // The meshes and the textures that did not change are kept: their files are gone.
    assert!(settle(&mut cache, &gpu).is_empty());
    check_cached(&gpu, &mut renderer, &cache, 0..6, |index| index % 3 == 1);
    // The handles stay valid, and say so.
    assert!(ready(held.handles()));
}

#[test]
fn a_reload_drops_what_the_new_data_no_longer_lists_and_what_nothing_holds() {
    let gpu = gpu();
    let (mut renderer, mut cache, directory) = cache(&gpu, "reload-removed");
    let meshes = palette_meshes();
    let mut handles = held(&mut cache, &gpu);
    assert!(settle(&mut cache, &gpu).is_empty());
    // Square 4 is held no more: releasable, it goes with the reload.
    handles[4] = None;
    // The last square is cooked no more: the new manifest does not list it.
    let path = directory.join(MANIFEST_FILE);
    let manifest = Manifest::from_bytes(&fs::read(&path).expect("read")).expect("a manifest");
    let mut listed = Manifest::new();
    for (id, entry) in manifest.iter().filter(|&(id, _)| id != meshes[5]) {
        listed.insert(id, entry.clone());
    }
    fs::write(&path, listed.to_bytes()).expect("written");
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    cache.reload(&gpu, assets);
    assert!(cache.mesh(meshes[4]).is_none(), "releasable, dropped");
    assert!(
        cache.mesh(meshes[5]).is_some(),
        "given until the reload finishes"
    );
    // The others are loaded again; the last fails, once, as missing, and goes.
    let missing = |errors: &[(AssetId, Arc<SceneError>)]| {
        matches!(
            errors,
            [(id, error)] if *id == meshes[5]
                && matches!(&**error, SceneError::Load(LoadError::NotCooked { .. }))
        )
    };
    let errors = settle(&mut cache, &gpu);
    assert!(missing(&errors), "{errors:?}");
    check_cached(&gpu, &mut renderer, &cache, 0..4, |_| false);
    assert!(cache.mesh(meshes[5]).is_none());
    assert!(matches!(state(&handles, 5), AssetState::Missing));
    assert!(matches!(state(&handles, 0), AssetState::Ready));
    // Asked for again, it is missing, and nothing loads.
    let again = cache.request(&gpu, [meshes[5]], Priority::Now);
    assert!(matches!(again.handles()[0].state(), AssetState::Missing));
    assert_eq!(cache.loading(), 0);
    // So it is in the cache that takes the requests, which says it once.
    let assets = cache.assets().clone();
    let mut cache = AssetCache::with_requests(&renderer, assets, cache.into_requests());
    let errors = settle(&mut cache, &gpu);
    assert!(missing(&errors), "{errors:?}");
    assert!(matches!(state(&handles, 5), AssetState::Missing));
    assert!(matches!(state(&handles, 0), AssetState::Ready));
}

/// A request's meshes stay while a handle holds them, whatever the budget, and the cache reports
/// what it cannot drop; once the last handle goes, they are releasable, dropped once past the
/// budget.
#[test]
fn a_request_s_meshes_stay_while_a_handle_holds_them() {
    let gpu = gpu();
    let (_renderer, mut cache, _) = cache(&gpu, "held");
    let meshes = palette_meshes();
    let request = cache.request(&gpu, meshes, Priority::Soon);
    assert!(settle(&mut cache, &gpu).is_empty());
    let report = cache.report();
    assert_eq!(report.handles, [0, 6, 0]);
    assert_eq!((report.meshes.held, report.meshes.releasable), (6, 0));
    assert_eq!((report.textures.held, report.textures.bytes), (3, 3 * 16));
    // No room at all: everything is held, and kept.
    cache.set_budgets(Budgets {
        textures: 0,
        meshes: 0,
    });
    cache.poll(&gpu);
    assert!(meshes.iter().all(|&id| cache.mesh(id).is_some()));
    let report = cache.report();
    assert_eq!(report.textures.pinned, 3 * 16, "{report:?}");
    assert_eq!(report.meshes.pinned, report.meshes.bytes, "{report:?}");
    assert!(report.meshes.bytes > 0);
    // Let go of: past the budgets, dropped, with their page and textures.
    drop(request);
    cache.poll(&gpu);
    assert!(meshes.iter().all(|&id| cache.mesh(id).is_none()));
    let report = cache.report();
    assert_eq!(report.handles, [0; 3]);
    assert_eq!((report.textures.bytes, report.textures.dropped), (0, 3));
    assert_eq!(report.meshes.dropped, 6);
}

/// Past the textures' budget, the cache drops the releasable textures least recently held first,
/// with the releasable meshes that read them, never a texture that a held mesh reads; a page that a
/// held mesh shares stays.
#[test]
fn past_its_budget_the_cache_drops_what_was_held_least_recently_first() {
    let gpu = gpu();
    let (_renderer, mut cache, _) = cache(&gpu, "least-recently");
    let meshes = palette_meshes();
    // Each texture is read by squares i and i + 3.
    let mut handles = held(&mut cache, &gpu);
    assert!(settle(&mut cache, &gpu).is_empty());
    // Let go of texture 0's squares, then, a frame later, of texture 1's: texture 2's stay held.
    handles[0] = None;
    handles[3] = None;
    cache.poll(&gpu);
    handles[1] = None;
    handles[4] = None;
    cache.poll(&gpu);
    // Room for two textures of the three: texture 0, the least recently held, goes.
    cache.set_budgets(Budgets {
        textures: 2 * 16,
        ..cache.budgets()
    });
    cache.poll(&gpu);
    let held = |cache: &AssetCache| -> Vec<bool> {
        meshes.iter().map(|&id| cache.mesh(id).is_some()).collect()
    };
    assert_eq!(held(&cache), [false, true, true, false, true, true]);
    assert_eq!(cache.report().textures.bytes, 2 * 16);
    // No room: texture 1 goes too, but texture 2, which held squares read, stays, reported.
    cache.set_budgets(Budgets {
        textures: 0,
        ..cache.budgets()
    });
    cache.poll(&gpu);
    assert_eq!(held(&cache), [false, false, true, false, false, true]);
    let report = cache.report();
    assert_eq!((report.textures.bytes, report.textures.pinned), (16, 16));
    assert_eq!(report.textures.dropped, 2);
    // The four squares that read them went with them; their page, shared with the held ones,
    // stays whole.
    assert_eq!(report.meshes.dropped, 4);
    assert_eq!(report.meshes.pinned, report.meshes.bytes);
    drop(handles);
}

/// What handles hold is loaded again by the cache that takes the requests of a cache whose device
/// was lost, at their priorities; the handles stay valid.
#[test]
fn handles_stay_valid_through_a_lost_device() {
    let gpu = gpu();
    let (renderer, mut cache, directory) = cache(&gpu, "lost-device");
    let meshes = palette_meshes();
    let now = cache.request(&gpu, meshes[..3].iter().copied(), Priority::Now);
    let later = cache.request(&gpu, meshes[3..].iter().copied(), Priority::Later);
    assert!(settle(&mut cache, &gpu).is_empty());
    let requests = cache.into_requests();
    assert!(!now.is_done(), "loading again until a new cache holds them");
    drop(renderer);
    // The renderer that replaces the lost one, on the same device here.
    let mut renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
    let assets = CookedAssets::open(&directory).expect("the manifest is read");
    let mut cache = AssetCache::with_requests(&renderer, assets, requests);
    cache.poll(&gpu);
    assert_eq!(cache.report().loading, [3, 0, 3]);
    assert!(settle(&mut cache, &gpu).is_empty());
    assert!(ready(now.handles()) && ready(later.handles()));
    check_cached(&gpu, &mut renderer, &cache, 0..6, |_| false);
}

/// A long session of requests and releases, at every priority, keeps the memory that the cache
/// counts within its budgets, but what handles hold.
#[test]
fn a_long_session_of_requests_and_releases_stays_within_the_budgets() {
    let gpu = gpu();
    let (_renderer, mut cache, _) = cache(&gpu, "session");
    let meshes = palette_meshes();
    cache.set_budgets(Budgets {
        textures: 16,
        meshes: 0,
    });
    let mut held: Vec<Vec<AssetHandle>> = Vec::new();
    // A small generator of the same numbers at each run.
    let mut seed = 0x2545_f491_u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as usize
    };
    let priorities = [Priority::Now, Priority::Soon, Priority::Later];
    for _ in 0..200 {
        let picked: Vec<AssetId> = (0..1 + next() % 3)
            .map(|_| meshes[next() % meshes.len()])
            .collect();
        let request = cache.request(&gpu, picked, priorities[next() % 3]);
        held.push(request.into_handles());
        if held.len() > 2 {
            held.remove(next() % held.len());
        }
        let failed: Vec<Arc<SceneError>> = settle(&mut cache, &gpu)
            .into_iter()
            .map(|(_, error)| error)
            .collect();
        assert!(failed.is_empty(), "{failed:?}");
        // Once the loads that nothing holds any more have ended, which free what they made.
        let within = |cache: &mut AssetCache| {
            cache.poll(&gpu);
            let report = cache.report();
            [&report.textures, &report.meshes]
                .iter()
                .all(|kind| kind.bytes <= kind.budget.max(kind.pinned))
        };
        let mut tries = 0;
        while !within(&mut cache) {
            tries += 1;
            assert!(tries < 1000, "{:?}", cache.report());
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    held.clear();
    settle(&mut cache, &gpu);
    cache.poll(&gpu);
    let report = cache.report();
    assert_eq!(
        (report.textures.bytes, report.meshes.held),
        (0, 0),
        "{report:?}"
    );
}
