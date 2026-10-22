# 0128. Asset requests

- Status: Accepted; amends [0073](0073-loading-scenes-in-the-background.md) (loads of a rank, and
  stopped without waiting), [0084](0084-what-the-world-draws.md) (the bridge holds a handle of each
  mesh that the world names), [0113](0113-asset-requests-residency-and-priorities.md) (how requests,
  handles, budgets and a lost device are built), [0117](0117-frustum-culling-and-instancing.md) (an
  asset cache's meshes in pages of its own), [0122](0122-the-development-channel.md) (version 5 of
  the protocol: a game's reports) and [0127](0127-the-settings-of-a-view.md) (`ReflectedSettings`
  becomes `Reflected`, which reports use too)
- Date: 2026-10-22

## Context

[ADR-0113](0113-asset-requests-residency-and-priorities.md) decided how assets are asked for, kept
and dropped: requests of a priority, *now*, *soon* or *later*, that return handles; an asset kept
while a handle holds it, releasable once none does, and dropped, least recently used first, past a
budget of each kind, textures and meshes, which the engine counts from what it creates; requests
served class by class, by the tasks of the loads and by the thread that submits their copies;
handles that stay valid through new cooked data and a lost device. It left the shape of the API to
M6a.9, and so did [ADR-0117](0117-frustum-culling-and-instancing.md), for how budgets count meshes
that share pages, and [ADR-0120](0120-a-frame-that-owns-what-it-draws.md), for how handles carry
over to the cache that a lost device makes a game make again.

Before M6a.9, the renderer's `AssetCache` loaded, as one batch in the background, the meshes that
the bridge from the world (`slime-world-render`) asked for at each frame, and kept everything it
loaded. A batch failed whole at its first error (G16 in `docs/PLAN.md`): one unreadable file hid
every mesh of its batch until the cooked data changed. The loads' tasks ran in the order the loads
started; the uploader submitted copies in the order they came; dropping a load waited, on the
calling thread, for its running tasks. The cache's meshes shared the renderer's pages, with the
meshes that a game makes itself.

The owner took five decisions on 2026-10-22, one at a time:

- **Q1, the grain of a handle**: a request arrives whole, and gives one handle per asset, each let
  go of on its own; rather than one handle per request, which keeps a whole batch while one of its
  meshes is drawn, or one request per asset, which would draw an object piece by piece.
- **Q2, meshes in pages**: the budget of meshes counts the memory of their pages, and the cache
  frees it by emptying the pages whose meshes are all releasable, least recently held first; rather
  than each mesh by its own bytes, which leaves the memory above the count while a page is half
  empty, or pages compacted, which ADR-0117 excludes.
- **Q3, the default budgets**: of the 4 GB of video memory for assets of the lowest tier, the
  handheld one ([ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)), 3 GiB for textures and
  1 GiB for meshes; rather than halves, or one budget for both, which ADR-0113 rejected.
- **Q4, the cache's state for an agent**: named reports that a presentation gives, described by
  reflection, the asset cache's and its views' statistics, which the development channel lists and
  reads; rather than a request of the cache's alone, or more text in the game's state.
- **Q5, handles and a lost device**: the requests are a value that holds no GPU object, which the
  cache gives back when its device is lost and the new cache takes; rather than a cache that
  survives and changes its renderer, which ADR-0120 rejected, or handles that a game makes again.

## Decision

**A request arrives whole, and holds each asset through a handle.**
`AssetCache::request(gpu, ids, priority)` returns a `Request`: the meshes that the cache neither
holds nor loads start loading as one batch, which the cache takes whole at a later
`AssetCache::poll`, once each of its meshes is made or failed. `Request::into_handles` gives one
`AssetHandle` per mesh, which keeps it, or keeps it loading, while it lives; its state says whether
the mesh loads, is ready, failed and why, or is missing from the cooked data; its priority can
change while it lives, and a clone is another hold. The handles live in a book that the cache and
they share, `AssetRequests`, which holds no GPU object. The bridge holds a handle of each mesh that
the world names, asked for at `Priority::Now`, and drops it once the world no longer names it or
hides what names it. `AssetCache::poll` takes the GPU, to start the loads that handles ask for.

**A load goes at the rank of its most urgent handle.** A load's rank is its class, the most urgent
priority of the handles of its meshes, then the order the loads started in. The lane of the loads'
tasks, at most two at a time, runs the task of the lowest rank first, and the uploader submits the
copies of the lowest rank first: `slime_gfx::UploadRank`, which a load shares with its copies and
which changes while they wait, as a handle raised from *soon* to *now* moves its load ahead. A
load's completion goes before every task, as before. A scene's load is of the class *now*.

**What nothing holds stops, without waiting.** At each poll, a load whose meshes no handle holds
stops: the tasks that start from then on do nothing, and the cache keeps it until none of its tasks
runs, so that dropping it waits for none. A mesh of a load that nothing holds any more is left out,
unless a task has started reading it. A reload stops the loads under way the same way. Dropping the
cache, or giving its requests back, still waits for the running tasks, which hold the GPU.

**A mesh fails alone** (G16). In an asset cache's batch, a mesh that cannot be read, or whose
material or one of its textures cannot, or whose material the renderer refuses, fails alone, with
its error, which every mesh that needs the same material or texture shares; the others of the
request arrive. `poll` returns each failure once, its handles say it, as missing when the cooked
data does not name the mesh, and the cache does not ask for it again until the cooked data changes.
A load that panics marks its meshes failed (`SceneError::Panicked`) before its panic is raised. A
scene's load still fails whole.

**The cache counts what it creates, and drops past its budgets.** `Budgets` hold the bytes that the
cache keeps its textures and its meshes within, by default 3 GiB and 1 GiB, which a game changes
(`AssetCache::set_budgets`). A texture counts the bytes of its levels (`Texture::bytes`); the
meshes count the pages they share, which are the cache's own, apart from the renderer's, so that
the count is the cache's alone. At each poll, past the meshes' budget, the cache drops the pages
whose meshes are all releasable, the least recently held first; past the textures', the textures
that only releasable meshes read, the least recently held first, with those meshes. Materials and
textures go with the last mesh that needs them. What it cannot drop, held or in a page that a held
mesh shares, it reports, and warns of when a kind first passes its budget. A page counts while a
mesh lives in it, one that a load places or that a frame still draws included, and is not dropped
while one that is not the cache's does, since dropping the others would not free it. The count
leaves out the driver's alignment, the materials' small buffers, and a texture dropped while a
frame still draws it: it decides what to drop and never refuses a creation, as ADR-0044 and
ADR-0113 say.

**Handles stay valid through new cooked data and a lost device.** `AssetCache::reload` loads again
what handles hold, keeping what did not change, and drops the releasable meshes; a mesh that the new
data no longer lists is dropped once the reload finishes, its handles saying it is missing. A mesh
that the reload cannot make again keeps what the cache held, its handles saying it is ready. `poll`
returns the error of each.
`AssetCache::into_requests` drops every GPU object of a cache whose device is lost and returns its
`AssetRequests`; `AssetCache::with_requests` makes the cache of the new device with them, whose
first poll loads again what handles hold, one batch per priority. A frame builder given a cache that
did not issue its handles (`AssetCache::issued`), as one made again without those requests, asks it
for its meshes again, rather than drawing none of them.

**A game gives tools its reports.** `slime-app`'s `Reflected`, the value with the registry of its
type that ADR-0127 named `ReflectedSettings`, carries reports too: `Presentation::reports` gives
named reports, made when a tool asks. Version 5 of the development channel's protocol lists their
names in the game's state and reads one (`Request::Report`), as a document in the engine's
notation; the MCP server's `read_report` gives it. `AssetCache::report` gives a `CacheReport`: the
handles of each priority, the meshes that load, those that failed, and for each kind what is held,
what is releasable, its bytes, its budget, what it cannot drop and what it dropped. The `world`
example and *Hunna Wood* report `assets`, their cache's, and `views`, their views' `RenderStats`,
which reflection now describes, their times in seconds through a reflected `Duration`.

## Alternatives considered

Those of the five decisions are above. Besides:

- **The cache's meshes in the renderer's pages**, as before: one set of pages, but the budget would
  count the meshes that a game makes itself, and a page of the game's meshes would hold the cache's
  memory.
- **Ranks in the lane alone**: the tasks of what a view lacks would run first, but its copies would
  wait behind those of what is fetched ahead, a budget per frame at a time.
- **Stopping a load by dropping it**, as before: no load kept, but the frame that lets go of a
  request would wait for its running tasks.
- **A report of the cache's own in the protocol**: one request fewer to name, but each report to
  come, of the audio or the physics, would add its request, its reply and its tool.

## Consequences

- `AssetCache::request` takes a priority and returns a request that a caller keeps: dropped at once,
  it lets go of what it asked for, whose load stops. `AssetCache::poll` takes the GPU and returns
  each failed mesh with its error; `Frame::errors` holds them.
- The GPU memory of the cache's assets stays within its budgets but what handles hold, and an asset
  that leaves the screen and comes back while there is room does not load again.
- What the player sees loads before what is fetched ahead, and its copies go first.
- A cache's meshes take pages of their own: a game that makes meshes itself and loads others through
  a cache has two sets of pages, each bound when a draw changes page.
- Tools and games of version 4 of the channel refuse each other.
