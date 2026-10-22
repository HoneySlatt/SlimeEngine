# 0073. Loading scenes in the background

- Status: Accepted; amended by [0074](0074-uploading-textures-without-holding-up-frames.md) (what a
  load costs the frames), [0107](0107-the-execution-model.md) (the engine's task pool, and one owner
  of the copies to the GPU), [0113](0113-asset-requests-residency-and-priorities.md) (loads given
  a priority, and cancelled) and [0128](0128-asset-requests.md) (loads of a rank, and stopped
  without waiting)
- Date: 2026-10-15

## Context

[ADR-0072](0072-stage-4b-live-assets.md) decides that the renderer loads a scene on threads of its
own, which create the GPU objects through the `Gpu` they share, on a task pool private to
`slime-render`, and that a game polls the load and takes the scene whole. It leaves how to the
milestone that builds it, M4b.1.

Where the code stood: `Renderer::load_scene` read and uploaded every asset of a scene on the
calling thread. `Gpu` was `Send` and `Sync` but not `Clone`. Uploading a texture or a mesh reads
nothing of the renderer; creating a material reads three of its objects: the layout of a material's
group, and a white pixel and its sampler for the slots a material leaves empty. wgpu keeps its error
scopes per thread, so the out-of-memory scope that `slime-gfx` wraps every creation in works on any
thread. In wgpu 30, writing a texture through the queue holds the queue's `pending_writes` lock
while it copies the texels into staging memory, and a submission takes the same lock.

Measured before the change, in a release build on an RX 6900 XT with the files in the system's
cache, *Hunna Wood* loaded its zone (74 meshes, 24 materials, 56 textures) in 165 to 191 ms and its
hero (5 meshes, 9 textures) in 172 to 217 ms, before its window showed anything.

## Decision

- **`Gpu` is `Clone`**: a clone is another handle to the same device and queue, which threads
  share; an object created through one clone is used with any.
- **`Renderer::load_scene_in_background(gpu, assets, id)` returns a `SceneLoad`** at once.
  `SceneLoad::take` returns `None` while the load goes on, then the whole `LoadedScene` or the
  `SceneError` that stopped it, once; `SceneLoad::wait` blocks for it. `Renderer::load_scene` is
  starting a load and waiting for it: one way to load.
- **Each load has a thread of its own, which coordinates it.** It reads the scene, starts a task per
  mesh that its nodes place, each once; as each mesh is made, it reads the material that the mesh
  names, if new, a small file, and starts a task per texture of that material not asked for yet.
  When every task has reported, it creates the samplers and materials and places the nodes, in the
  order of the nodes, so that the loaded scene does not depend on which task finished first. A
  texture is named, in error messages and capture tools, after its ID and its source file.
- **The tasks run on the renderer's task pool**: threads started at the first load, as many as the
  machine's cores but one, at least 1 and at most 4, shared by every load of the process and living
  as long as it, which wait for tasks while there are none. A task reads one asset from the cooked
  data and uploads it through a clone of the `Gpu`. A task never waits for another, so that the
  pool cannot block itself.
- **What creating a material takes is a value of its own**, `MaterialParts`, private, which the
  renderer keeps and clones into each load.
- **A load stops without waiting for its queued tasks.** Its tasks take the `Gpu` and the cooked
  data from one place that they share, under a lock, and count themselves as they take them; to
  stop the load, the coordinator empties that place and waits only for the tasks counted, which are
  running or done. A task whose turn comes later finds the place empty, and reports that it did
  nothing, without touching the GPU. Each task drops its clones before it reports, so that once the
  coordinator has returned, no thread of the load holds the `Gpu`: a device that is lost and
  replaced (M4b.3) is not kept alive by a load.
- **The first error wins.** A task that fails, or a material that cannot be read, stops the load;
  the coordinator drops what the running tasks made and returns the error. Which error is first can
  change from one run to the next.
- **A panic is raised again where the load is taken.** A task catches its panic and reports it; the
  coordinator stops the load, then raises it, and `take` or `wait` raises it on the caller's thread
  with its message, even when an asset also failed. The pool's threads go on.
- **Dropping a `SceneLoad` stops its load**: the drop sends the coordinator a message, which reaches
  it at once wherever its tasks are in the pool's queue, and waits for the coordinator to return.

Measured after the change, as before: the zone loads in 75 to 83 ms and the hero in 71 to 74 ms,
with four threads in the pool.

## Alternatives considered

- **Threads started by each load, in a scope** (`std::thread::scope`): no thread outlives a load,
  but two loads at once would run twice as many threads as the machine has cores to spare.
- **A flag that tasks read when their turn comes, and a coordinator that waits for every task**:
  simpler, but dropping a load, or a failure, would wait until the pool's queue reached each of the
  load's tasks, behind those of other loads, tens of milliseconds while a zone loads; and a task
  that kept its clone of the `Gpu` until it reported could keep the device alive after the load.
- **A pool owned by the `Renderer`**: its threads would end with it, but a load in progress would
  have to keep the pool alive past the renderer's drop, and every renderer of a process would
  start its own.
- **The coordinator as a task of the pool**: no extra thread, but with one thread in the pool it
  would wait for tasks queued behind itself, for ever.
- **Reading every material before any mesh, through the manifest's list of what each asset
  needs**: the textures would start sooner, but the load would rely on that list, which the cooker
  writes for other uses, rather than on the meshes themselves; meshes are fast to make, and a
  texture starts as soon as the first mesh that needs it is made.
- **A crate for the pool, such as `rayon`**: a dependency, and a global pool of its own that other
  crates of a game might share, for what a few dozen lines of the standard library do.
- **Uploads from the main thread, within a budget of bytes per frame**: kept as the fallback of
  ADR-0072, if the frames held up by the queue's lock miss the bar that M4b.2 measures.

## Consequences

- A game opens its window and draws while its scenes load, once it polls them; loading is also
  faster, the textures of a scene uploading on several threads.
- The pool's threads live as long as the process, idle once nothing loads. Stage 7's job system is
  to replace them (ADR-0002).
- A load cannot be paused or given a priority, and gives nothing before its whole scene. Dropping
  one costs the caller at most the tasks of that load that are running, a mesh or a texture each.
- A coordinator that panics, a bug, stops its load as it unwinds, so that its queued tasks skip;
  its running tasks drop what they made on the pool's threads.
- While the pool's threads upload textures, the thread that submits frames can wait for the queue's
  lock; how long, on the zone, is M4b.2's measurement.
- After a lost device, a load in progress fails with `SceneError::Render`; a game that recovers
  starts it again with the new `Gpu` (M4b.3).
