# 0016. The GPU layer: wgpu behind `slime-gfx`

- Status: Accepted; amended by [0022](0022-losing-the-gpu-device.md) (errors after a loss) and [0031](0031-dev-overlay-with-egui.md) (wgpu for the tooling, optional features) and [0036](0036-direct3d-12-shader-compiler.md) (FXC on Direct3D 12)
- Date: 2026-10-12

## Context

Stage 2 gives the engine access to the GPU. The roadmap fixes wgpu as the GPU abstraction and
confines its types to the `gfx` and `render` crates.
[ADR-0014](0014-amend-roadmap-for-stage-2-and-windows.md) requires Linux and Windows from Stage 2
on, and CI machines have no GPU: tests must run on software drivers, Mesa's lavapipe on Linux and
Microsoft's WARP on Windows. The engine must still run without any GPU when it is headless.

## Decision

- **Crate.** `slime-gfx` owns every use of wgpu. Its public API uses engine-owned types only; no
  wgpu type leaves the crate. It depends on no other engine crate: a window will reach it as
  `raw-window-handle` traits, so offscreen rendering works without a windowing system.
- **wgpu configuration.** wgpu 30, with the features `std`, `parking_lot`, `vulkan`, `dx12` and
  `wgsl`. OpenGL, Metal and WebGPU are left out.
- **Graphics API per platform.** Vulkan on Linux, Direct3D 12 on Windows. The `WGPU_BACKEND`
  environment variable can choose another API that the build supports, for example Vulkan on
  Windows to compare drivers.
- **Adapter.** The high-performance preference: a discrete GPU first, then an integrated one, then
  virtual, then software adapters. A software adapter is therefore used only when nothing else is
  available: in CI, in a virtual machine without a GPU, or when `VK_DRIVER_FILES` restricts the
  Vulkan loader to lavapipe. The adapter in use is logged.
- **Device.** Default limits (the WebGPU defaults) and no optional feature, until something needs
  more: lavapipe and WARP support this baseline, so tests behave the same everywhere.
- **Debugging.** Debug builds pass labels to the driver, for capture tools. The validation layers
  of Vulkan or Direct3D 12 stay off unless `WGPU_VALIDATION=1` is set: they are slow and not
  always installed. wgpu always validates how the engine uses it.
- **Errors.** A failure caused by the environment is returned as a `GfxError`: no adapter, a
  device the adapter refuses, a failed wait. A use of wgpu that wgpu rejects is a bug and panics
  (wgpu's default error handler, consistent with ADR-0008); the engine keeps the environment from
  causing such rejections where it can: a window larger than the device's largest image is drawn at
  that size, and a shader or pipeline that the graphics driver rejects after wgpu's validation is
  returned as an error (ADR-0017). The loss of the device, for example after a driver crash, is
  logged as an error; stopping the run on it comes with rendering.
- **Asynchronous requests.** wgpu's adapter and device requests are futures; `slime-gfx` waits for
  them with `pollster` when the GPU is opened.
- **Software drivers in the checks.** The Nix shell gives the path of lavapipe's driver file
  (`SLIME_LAVAPIPE_ICD`), and `scripts/check.sh` runs the tests with `VK_DRIVER_FILES` set to it.
  The Windows CI runner has no GPU, so wgpu uses WARP there.

## Alternatives considered

- **Vulkan and Direct3D 12 directly** (ash, windows-rs). Full control, at the cost of two
  graphics backends to write and maintain; wgpu is a fixed decision of the roadmap.
- **Vulkan on Windows too, one API everywhere.** Direct3D 12 has the most reliable drivers on
  Windows, is always present, and comes with WARP for CI.
- **OpenGL as a fallback backend.** Adds EGL and GL loading for older machines, with fewer
  capabilities; to reconsider if users report machines without Vulkan.
- **Validation layers on in every debug build.** Slow, noisy when the layers are missing, and
  mostly useful to wgpu's own developers.
- **Asking for the adapter's full limits.** Code would start depending on what one GPU offers, and
  tests on lavapipe or WARP could fail where the developer's GPU passes.

## Consequences

- One API per platform keeps behaviour predictable; comparing with the other API is one
  environment variable away.
- wgpu is a large dependency: clean builds take longer, and its dependencies duplicate some of
  winit's at other versions (cargo-deny warns about them).
- The Vulkan backend is compiled on Windows without being used by default.
- A wgpu validation error panics in release builds too. Before games ship, this policy is to be
  reconsidered (logging the error and dropping the frame instead).

## Corrections

- 2026-10-13: the Debugging bullet holds on Vulkan only. On Direct3D 12, wgpu passes labels to the
  driver in every build, release builds included.
