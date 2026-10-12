# slime-gfx

The GPU layer: the boundary between the engine and wgpu, the library that talks to the graphics
drivers.

## Status

Partial. It opens a GPU device without a window; rendering arrives in the next milestones of
Stage 2 (see `docs/PLAN.md`).

## Role

- Owns every use of wgpu. Its public API uses engine-owned types only; no wgpu type leaves this
  crate (`docs/adr/0016-gpu-layer-with-wgpu.md`).
- Opens the GPU with the graphics API of each platform: Vulkan on Linux, Direct3D 12 on Windows.
- Works without a window: offscreen rendering and its tests need no windowing system.

## Entry points

- `Gpu::new()` opens the most capable adapter of the machine and its device, and logs the adapter.
  The `WGPU_BACKEND` environment variable can choose another graphics API, and `VK_DRIVER_FILES`
  can restrict Vulkan to one driver, such as lavapipe.
- `Gpu::adapter_name()` names the adapter in use.
- `Gpu::wait_idle()` blocks until the GPU has finished its work.
- `GfxError` says why the GPU could not be opened or used; wgpu's own error is its `source()`.

## Dependencies

- `wgpu`: access to the GPU through Vulkan and Direct3D 12, and the WGSL shader compiler.
- `pollster`: waits for wgpu's requests, which are futures, when the GPU is opened.
- `thiserror`: derives `GfxError`'s `Display` and `Error` implementations.
- `tracing`: logging.

## Must not

- Depend on `slime-platform` or any windowing code: windows reach it as `raw-window-handle`
  traits, so that offscreen rendering works without a windowing system.
- Expose wgpu's types in its public API.
