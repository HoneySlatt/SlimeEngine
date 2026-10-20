# 0118. The task pool and the copies to the GPU

- Status: Accepted; amends [0107](0107-the-execution-model.md) (the loads' tasks run two at a time,
  while the pool's other background work takes every thread but one; a budget of copies lasts a
  frame, or a frame's time without one, and none holds them before the first frame nor 100 ms after
  the last)
- Date: 2026-10-21

## Context

[ADR-0107](0107-the-execution-model.md) decided the engine's task pool, with a *frame* and a
*background* priority, and one owner of the copies to the GPU of what loads bring, within a budget
of bytes per frame; it left to measurement how many threads background tasks may hold and how large
the budget is, and to the milestone that builds them their shape ([`docs/PLAN.md`](../PLAN.md),
M6a.2). Before it, `slime-render` had a pool of its own, of at most four threads, a thread per load
that waited for its tasks, and tasks that submitted their textures' copies as they made them
([ADR-0073](0073-loading-scenes-in-the-background.md),
[ADR-0074](0074-uploading-textures-without-holding-up-frames.md)); the sky's environment started a
thread per core, and so did the cooker's encoder.

The audit of 0.4 found that a load slowed every frame (B4-F01). Measured again at the start of
M6a.2 on the owner's RX 6900 XT ([`measurements.md`](../measurements.md), "M6a.2, before"), with a
benchmark of its own, sixteen textures of 4096² texels in BC7, 341 MiB, loaded while frames are
drawn: frames of up to 24 to 31 ms, where a frame at 60 frames a second has 16.7 ms
([ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)); with *Hunna Wood*'s zone, frames of up to
6.5 ms, and the renderer's CPU two to three times what it is once the zone is loaded.

## Decision

**The pool, `slime-tasks`.** A crate at the bottom of the workspace, on the standard library
alone. Its threads are the machine's logical cores but one, at least one; a free thread takes a
frame task before any background task, and background tasks hold every thread but one when the
pool has more than one. `map` computes items in parallel, the calling thread taking items while it
waits, so that it finishes when called from a task too. A task owns what it reads: a pool that
lent the caller's data to its tasks, as `std::thread::scope` does, would need `unsafe` code, which
[ADR-0005](0005-lints-and-unsafe-policy.md) keeps for a need that a measurement shows. The sky's
environment and the table of the BRDF are computed by frame tasks; the cooker encodes textures as
background tasks; the loads run as background tasks.

**The loads, two tasks at a time.** The tasks of the process's loads, which read cooked files,
create GPU objects and write staging buffers, run at most two at once, in the order they come,
whatever the pool's threads: with *Hunna Wood*'s zone and the textures of 4096² on the RX 6900 XT,
two were the fastest and kept every frame under 13 ms, where four let frames take up to 68 ms and
fourteen, the pool's share, up to 41 ms on the zone, the loads competing with the thread that draws
for the memory's bandwidth and the locks of wgpu and the driver, with the budget of copies below,
8 MiB:

| Loads' tasks at once | Longest frame, textures | Longest frame, zone | Textures whole | Zone whole |
|---|---|---|---|---|
| 1 | 3.8 to 8.9 ms | 1.1 to 2.9 ms | 355 to 358 ms | 57 to 62 ms |
| 2 | 8.7 to 12.7 ms | 1.1 to 1.4 ms | 219 to 223 ms | 42 to 43 ms |
| 4 | 21.7 to 68.3 ms | 1.4 to 1.9 ms | 230 to 307 ms | 48 to 67 ms |
| 8 | 25.9 to 46.4 ms | 4.0 to 29.8 ms | 271 to 275 ms | 80 to 113 ms |
| 14 | 35.5 to 35.8 ms | 28.7 to 41.1 ms | 292 to 307 ms | 113 to 116 ms |

The pool's other background work, which touches neither the GPU nor much memory, as the cooker's
encoding, keeps every thread but one: on two threads, a texture of 4096² would take about five
times as long to cook, by the threads that it would lose.

**The uploader, in `slime-gfx`.** `Uploader` is the one owner of the copies: a load's task creates a
texture, or takes the ranges of a mesh in the renderer's pages
([ADR-0117](0117-frustum-culling-and-instancing.md)), and writes the texels or vertices into
staging buffers, mapped for writing, outside every lock of wgpu's; one thread of the uploader's
submits the copies, at most a budget of bytes per frame, then tells the load. A frame renews the
budget (`Uploader::frame`, which the renderer calls once its frame is submitted, so that the copies
run after it), and so do 16.7 ms without a frame; each call renews it, so that a renderer that
draws several outputs a frame lets as many budgets through. Before the first frame, and once
100 ms have passed without one, the copies go whatever the budget: a program that waits for a load
without drawing, as *Hunna Wood* waits for its hero, has no frame for them to hold up, and held to
the budget its load would take seven to ten times as long (the zone whole after 570 to 600 ms
rather than 60 to 85 ms on the RX 6900 XT, by a benchmark written for the occasion); 100 ms is
three frames of the slowest tier, at 30 frames a second
([ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)), so that a program that draws holds its
copies to the budget. A staging buffer holds at most a budget, its size a power of two
from 64 KiB, and goes back to the uploader once the GPU has run its copies, mapped again, for the
next uploads, up to four budgets of them. The uploader's thread waits rather than computes, as
ADR-0107 allows. Dropped, the uploader submits what waits, whatever the budget, so that no load is
left without an end; a device lost before the copies tells the loads, and a panic of its
submission, a bug, drops the uploads that it held, which their loads raise again. Each renderer has
one, which its loads use; a mesh keeps its ranges until its copy is submitted, so that a mesh made
after a load is cancelled does not take ranges that a late copy would overwrite.

**The budget, 8 MiB per frame**, measured as the tasks were, with the loads' tasks two at a time.
The benchmarks draw each frame as soon as the one before is done, a few milliseconds each, so that
their loads end sooner than at 60 frames a second, where 8 MiB per frame sends at most 480 MiB a
second:

| Budget | Longest frame, textures | Longest frame, zone | Textures whole | Zone whole |
|---|---|---|---|---|
| 2 MiB | 12.8 to 17.7 ms | 0.9 to 10.1 ms | 259 to 266 ms | 78 to 99 ms |
| 4 MiB | 14.4 to 17.2 ms | 1.4 to 9.7 ms | 222 to 235 ms | 60 to 67 ms |
| 8 MiB | 9.5 to 10.0 ms | 1.5 to 1.8 ms | 218 to 223 ms | 41 to 43 ms |
| 16 MiB | 11.0 to 20.1 ms | 1.5 to 2.3 ms | 210 to 229 ms | 42 to 44 ms |
| 32 MiB | 9.1 ms | 1.6 to 2.4 ms | 243 to 279 ms | 58 to 73 ms |

**A load without a thread of its own.** A load's first task asks for what it loads; each task
counts, and so does each upload until the uploader tells it; the last to end completes the load,
as a task, which makes its materials and places its nodes. A load stopped by its drop or by an error
lets its later tasks do nothing; its drop waits for the tasks that run, so that once it returns no
task of the load holds the GPU, and drops what the tasks share.

**What a caller needs at once stays at once.** `Gpu::create_texture`, `Renderer::create_texture`
and `Renderer::create_mesh` still write and submit at once, for a texture or a mesh that a program
makes and draws in the same frame; the loads alone go through the uploader.

## Alternatives considered

- **Background tasks capped at two threads in the pool itself**: one number, but the cooker's
  encoding, and every later background computation, five times slower in a process that also
  draws, such as Stage 6b's editor.
- **The loads' tasks on every background thread**, as ADR-0107 put them: the table above, frames of
  25 to 68 ms.
- **A pool that lends the caller's data to its tasks**: no copy of what a task reads, the copy of a
  texture's level being small beside its encoding, but `unsafe` code without a measured need.
- **The uploader in `slime-render`, on primitives of `slime-gfx`** (staging buffers, copies, a
  submission): the renderer would hold the policy, but `slime-gfx` would expose the mapping and the
  copies one by one, which only the uploader uses, and the mechanics of wgpu would spread over two
  crates.
- **The budget whether frames are drawn or not**: one rule, but a load that a program waits for
  without drawing at the pace of 8 MiB each 16.7 ms, seven to ten times as long as without it.
- **A budget per unit of time only**: no tie to the renderer, but the copies would not follow the
  frame they let through, and a display at 144 Hz would give each frame fewer of them.
- **A staging buffer per upload, of the upload's size, made and dropped** (ADR-0074's): no buffer to
  keep, but an allocation of the driver's at each texture, which costs while frames are drawn.

## Consequences

- While frames are drawn, a load costs each at most the copies of 8 MiB and what two tasks at once
  take of the memory and the driver: on the RX 6900 XT, no frame over 11 ms while sixteen textures of 4096² load,
  and the renderer's CPU at what it is once they are loaded
  ([`measurements.md`](../measurements.md), "M6a.2"). B4-F01 is fixed.
- Starting a load no longer starts a thread: about 1 µs on the calling thread, where it took 12 to
  18 µs.
- What a load reads waits in staging memory until its copies are submitted: a load of many hundreds
  of megabytes keeps them there while it lasts, at the pace of the budget. Nothing holds the loads
  back when they read faster than the budget sends; the asset requests of
  [ADR-0113](0113-asset-requests-residency-and-priorities.md), at M6a.9, weigh it.
- The first frame drawn after 100 ms without one may wait on the GPU for the copies submitted just
  before it, whatever the budget: a frame after a wait, which a player does not expect smooth.
- Two renderers have two uploaders, each with its budget; the asset cache that M6a.4 frees from the
  renderer takes the renderer's uploader with it.
- The figures were measured on one machine, which the known debt of
  [`docs/PLAN.md`](../PLAN.md) says, as for the hardware tiers' ratios.
