# 0044. Running out of GPU memory

- Status: Accepted; amends [0016](0016-gpu-layer-with-wgpu.md),
  [0017](0017-gpu-resource-types.md) and [0022](0022-losing-the-gpu-device.md)
- Date: 2026-10-14

## Context

A game creates GPU objects all along: when a level loads, and in an open world while it streams
the parts the player approaches. When the GPU's memory is full, wgpu 30 reports an error of the
out-of-memory kind for the memory of a buffer, a texture, a sampler or a query set, and leaves the
device usable. The engine's error handler took every error outside an error scope as a bug
([ADR-0016](0016-gpu-layer-with-wgpu.md)) and panicked: the game crashed with a technical
message, at the moment it is most likely to happen, on the players with the smallest GPUs.

For the memory it cannot do without, wgpu loses the device instead: the staging memory that
carries an object's data to the GPU, and the memory of the other kinds of objects (views, bind
groups, shaders, pipelines, command buffers). [ADR-0022](0022-losing-the-gpu-device.md) already
stops the run cleanly then.

An audit of the debt before 0.1 listed the crash. The owner first chose to stop the run as on a
lost device, then decided on 2026-10-14 that every creation is fallible: the game receives the
error and reacts, as streaming a world needs.

## Decision

- Every method of `Gpu` that creates a GPU object returns a `Result`. The creation runs inside a
  wgpu error scope for out-of-memory errors, pushed on the creating thread (wgpu's scopes are per
  thread), and returns the new `GfxError::OutOfMemory`, naming the object's label, when the scope
  caught one. Nothing is created then, and the device can still be used: the game frees memory or
  asks for less, and tries again.
- On a lost device, nothing is created: the creation returns `GfxError::DeviceLost`, where
  ADR-0022 had it return an object that could not be used. A creation that loses the device, its
  data upload included, returns `DeviceLost` too, and the run stops as ADR-0022 says.
- An out-of-memory error outside the engine's scopes comes from a creation that no caller can be
  told of, made by a library built on wgpu, such as egui's renderer (ADR-0031). It is logged, and
  the device is given up as if it were lost: `Gpu::check` returns `DeviceLost`, whose message says
  that the memory ran out, and the run stops cleanly instead of panicking.
- The renderer passes these errors on, as `RenderError::Gfx`, from every method that creates GPU
  objects: `Renderer::new`, `create_environment`, `create_mesh`, `create_material`, and `render`,
  which creates its targets when the output's size changes, the bloom's levels when the bloom
  resumes, and a larger instance buffer when a scene needs one. A frame that fails draws nothing,
  and the renderer keeps the targets it had: the next call tries again, after the game has freed
  memory or with a smaller output.
- Other wgpu errors remain bugs, which panic. A creation's invalid arguments, such as an empty
  buffer or a texture larger than the device allows, still panic before anything is created.
- The `SLIME_GPU_MEMORY_BUDGET` environment variable, a percentage from 1 to 100, sets wgpu's
  threshold for resource creation: buffers, textures and query sets fail with `OutOfMemory` once
  the memory in use would pass that share of what the driver allows the process. It works on
  Direct3D 12, and on Vulkan with drivers that report a memory budget (`VK_EXT_memory_budget`),
  RADV and lavapipe among them. Drivers count the memory already in use differently: lavapipe
  counts the whole machine's, so that below a share that depends on what else runs, the device
  does not even open. It is a tool to try a game with less memory and to test these errors, not a
  size: the tests of `slime-gfx` search for a share at which the device opens and then runs out.

This amends ADR-0016 (an out-of-memory error is not a bug), ADR-0017 (the creation methods return
a `Result`) and ADR-0022 (a lost device creates nothing, and memory that runs out where no caller
hears of it gives the device up).

## Alternatives considered

- **Stop the run as on a lost device.** Simple, and the first choice, but the game could not
  react: a level that does not fit would end it, and streaming could not fall back to lower
  resolutions.
- **Fallible creation for buffers and textures only.** The objects that hold most memory, but two
  rules where one does, and samplers and query sets fail the same way in wgpu.
- **The engine counts the memory it creates and refuses past a budget.** It does not know the
  driver's alignment and overheads, nor what other processes use; wgpu's threshold works from the
  driver's own figures.
- **A memory threshold by default.** The driver may still give memory past it, by moving some to
  system memory; refusing it is a policy that belongs to the game's streaming, when it exists.
- **Testing with a fixed percentage.** lavapipe's count depends on the machine's memory: a share
  that opens the device on one machine fails to on another.

## Consequences

- Every caller of a creation handles a `Result`; the examples propagate it with `?`, the tests use
  `expect`.
- Each creation pushes and pops an error scope: microseconds, at creation time, not per frame.
- Running out of staging memory while uploading an object's data still loses the device; so does
  running out in a library's creation.
- Asynchronous loading (Stage 4 of `docs/ROADMAP.md`) and world streaming with memory budgets
  (Stage 10) start from fallible creation.
