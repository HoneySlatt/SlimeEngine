# 0113. Asset requests, residency and priorities

- Status: Accepted; amends [0073](0073-loading-scenes-in-the-background.md) (loads given a
  priority, and cancelled) and [0084](0084-what-the-world-draws.md) (assets requested through
  handles, by the bridge, the editor and games, and dropped when nothing holds them); amended by
  [0120](0120-a-frame-that-owns-what-it-draws.md) (after a lost device, the cache is made again
  rather than cleared) and [0128](0128-asset-requests.md) (how requests, handles, budgets and a
  lost device are built)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) decides, before Stage 6 builds the editor on it, how assets are
asked for, kept and dropped (C3-01). The audit of 0.4 measured the figures below on the owner's RX
6900 XT at `v0.4.0`; its report is kept outside the repository, as the plan says.

- **The world holds asset IDs, and the cache loads what the bridge asks for**
  ([ADR-0084](0084-what-the-world-draws.md)). At each frame, `slime-world-render`'s bridge asks the
  renderer's `AssetCache` for the meshes of what it draws; those it neither holds nor loads start
  loading, as one batch, which arrives whole. A load cannot be paused or given a priority
  ([ADR-0073](0073-loading-scenes-in-the-background.md)). The cache drops nothing that it loaded,
  but what new cooked data no longer names, when it loads again (`AssetCache::reload`), and
  everything after a lost device (`AssetCache::clear`): loading ahead and dropping what is no longer
  drawn were left to Stage 10's streaming or a loading screen, and ADR-0084 rejected loads that a
  game asks for beside the bridge's, without a user yet.
- **Some of it is settled already.** P4 shares the cooked data's manifest between batches rather
  than copying it ([ADR-0106](0106-the-measured-quick-wins.md)). Decided, to be built at Stage 6:
  one thread submits the copies to the GPU within a budget of bytes per frame
  ([ADR-0107](0107-the-execution-model.md)); a frame owns what it draws, through counted
  references to its meshes and materials, and the cache loads without the renderer
  ([ADR-0111](0111-the-frame-contract.md)).
- ***Hunna Wood* stays within its budgets**: its cooked data is read and made into GPU objects in
  103 to 150 ms, and the GPU memory that a load takes was 1.035 times its cooked bytes, with no leak
  over 10 loads, before P4 compressed the hero's coat. Without priorities, its zone, asked for right
  after its hero, delays the hero by about 18 ms.
- **What comes asks for more.** The editor's views, thumbnails and previews of prefabs would each
  stay in the GPU's memory for good; an open world does not fit in it; what the player sees must
  load before what is fetched ahead, and a load the player turned away from should stop.
- **The GPU's memory left is not known.** wgpu 30 gives no size of memory in its adapter's
  information or limits; its allocator's report, on Vulkan and Direct3D 12, tells what wgpu
  allocated, not what is left; the threshold that `SLIME_GPU_MEMORY_BUDGET` sets reads the driver's
  budget only to refuse creations, and RADV's not at all
  ([ADR-0044](0044-running-out-of-gpu-memory.md)). ADR-0044 rejected an engine that counts what it
  creates to refuse creations past a budget, and left refusing to a game's streaming.

Engines that stream ask for assets by requests that carry a priority and return a handle that keeps
them in memory while it is held, and that is released, or cancelled while it loads; they keep the
GPU's memory within a budget, lowering the detail of what has gone longest unseen first.

The owner took three decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**A request carries a priority and returns a handle.** Asking the cache for assets returns a
*request handle*; several assets asked for together arrive together, as a batch does. While a
request handle lives, its assets stay in memory, or keep loading. Dropping the last request handle
of an asset cancels its load if it is not done, and makes it *releasable* once it is. A request
handle's priority can change while it lives. Request handles are held by those who need the assets:
the bridge, for what each view draws; the editor, for its thumbnails and previews; a game, for what
it fetches ahead, the next area, the arena of the next boss, which ADR-0084 left without a user. The
world holds asset IDs only: components stay plain data, which reflection, saves and scene files
read, and the world, which runs without a GPU ([ADR-0079](0079-the-engines-own-world.md)), holds
nothing of the cache's. The counted references of a frame (ADR-0111) keep the GPU objects that it
draws alive while it is drawn; they hold nothing in the cache, and an asset dropped then is freed
once the frame is done.

What a request handle meets:

- *a load that fails*: its handle says so, with the error, and the asset is not asked for again
  until the cooked data changes, as today;
- *new cooked data* (`AssetCache::reload`, [ADR-0076](0076-hot-reload-of-cooked-data.md)): what a
  request handle holds loads again in the background, what did not change is kept, and the handle
  gives what it held until the new one is there; an asset that the new data no longer names is
  dropped, and its handle says that it is missing; releasable assets are dropped;
- *a lost device* (`AssetCache::clear`): every GPU object is dropped, and what request handles hold
  loads again on the new device, at their priorities; the handles stay valid.

**What nothing holds stays while there is room.** A releasable asset stays in memory, ranked by when
it was last used. The cache keeps a budget of memory for each kind, textures and meshes: past it, it
drops the releasable assets least recently used first, never one that a request handle holds, and
reports when dropping is not enough. The engine counts what it holds itself, from the sizes of what
it creates, an estimate that leaves out the driver's alignment and overheads: the count decides only
what to drop, never refuses a creation, so that ADR-0044's threshold and its `OutOfMemory` stay as
they are. The default budgets come from the hardware tiers that the foundations pass decides. A long
session of the editor is tested for memory that grows. At Stage 10a, textures stream by their
levels of detail, their mipmaps: under pressure, the finest levels of a far texture go first,
rather than the whole texture, so that an object shows blurred, never missing; cooked textures hold
all their levels already.

**Three classes of priority.**

- *Now*: what a view draws and lacks.
- *Soon*: what a game fetches ahead.
- *Later*: the editor's thumbnails and previews, what is fetched far ahead.

Within a class, requests keep their order. A request handle moves between classes, as an asset
fetched ahead comes into view. Every class runs as [ADR-0107](0107-the-execution-model.md)'s
background tasks; the pool's threads take loading tasks, and the thread that submits copies to the
GPU serves their copies within its budget per frame, class by class in that order. At Stage 10a, a
priority computed at each frame, from an object's size on the screen and its distance, ranks
requests within a class.

**When it is built.** At Stage 6: requests, request handles, cancellation and classes, about half a
milestone, in place of `AssetCache::request`; dropping under a budget, a small milestone. At Stage
10a: textures streamed by their levels of detail, and the priority computed at each frame.

## Alternatives considered

The shape of a request:

- **The bridge asking at each frame, the cache keeping everything**, as today: simple, but nothing
  tells the cache that an asset is no longer needed, so nothing is dropped, and a load cannot be
  cancelled or put before another.
- **Handles counted inside components**, as Bevy keeps them: an asset's life follows its entities;
  but components would no longer be plain data that saves, reflection and scene files read, the
  world would hold the cache's objects, and [ADR-0079](0079-the-engines-own-world.md) rejected
  counters in components, leaving counting on top of asset IDs for later.

Dropping:

- **At once, when the last request handle is dropped**: memory always at its least, but what leaves
  a view for a moment, a rock as the camera turns, a preview as the editor changes tab, loads again
  and again, and shows late.
- **Textures streamed by their levels of detail now**: memory at its least, but it needs each
  object's size on the screen, which culling at Stage 6 and streaming at Stage 10a bring, for no
  problem measured before large worlds.

Priorities:

- **The order of requests alone**: nothing to build, and enough for the hero; but "the view before
  the thumbnails" and "what is seen before what is fetched ahead" cannot be said, and a request made
  for later can hold up what the screen lacks.
- **A priority computed at each frame now**: the finest, but it needs each object's size on the
  screen too.

## Consequences

- `AssetCache::request` gives way to requests that return request handles; the bridge holds them
  for what its views draw, the editor for what it shows, a game for what it fetches ahead.
- The GPU's memory that assets take stays within a budget, counted by the engine; an asset that
  leaves a view and comes back soon is not loaded again.
- A load that nothing holds any more stops; what the player sees loads first.
- Hot reload and a lost device keep request handles valid, as they keep the world's asset IDs.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6 builds requests, request handles, priorities
  and dropping under a budget; Stage 10a streams textures by their levels of detail, with a
  priority computed at each frame.
