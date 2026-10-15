# 0036. The shader compiler of Direct3D 12

- Status: Accepted; amends [0016](0016-gpu-layer-with-wgpu.md); amended by
  [0066](0066-shipping-dxc-beside-the-programs.md) (DXC when it lies beside the program)
- Date: 2026-10-13

## Context

On Direct3D 12, wgpu translates the engine's WGSL shaders to HLSL and has Microsoft compile them,
with one of two compilers:

- FXC, part of Windows: present everywhere, old, slow, unmaintained, limited to shader model 5.1;
- DXC, maintained and faster, with shader model 6: a `dxcompiler.dll` (and `dxil.dll`) that the
  application ships or that the machine happens to have.

wgpu 30 defaults to `Dx12Compiler::Auto`, which takes any `dxcompiler.dll` that the Windows DLL
search finds, with no check of its version: in the application's directory, the system
directories, the working directory or `PATH`, where the Vulkan SDK or a Visual Studio developer
prompt put one. FXC is used only when none loads, and a DLL that loads but fails in another way
removes the Direct3D 12 backend. The same game can therefore compile its shaders differently, or
not start, depending on what else is installed. The engine never read `WGPU_DX12_COMPILER`, which
wgpu offers to choose. Nothing the engine draws needs shader model 6 yet.

## Decision

- The engine asks wgpu for FXC (`create_instance` in `crates/slime-gfx/src/gpu.rs`).
- `WGPU_DX12_COMPILER=dxc` (or `dynamicdxc`) still asks for DXC, loaded from `dxcompiler.dll`, for
  a developer who wants to compare. The variable's other values (`fxc`, `staticdxc`, `auto`) give
  FXC: `staticdxc` needs a wgpu feature that the engine does not build, on which wgpu panics, and
  `auto` would bring back the choice that depends on the machine.
- A unit test checks the choice for each value. wgpu logs the compiler it loads at `debug`
  (`SLIME_LOG=wgpu_hal=debug`).
- The question is to reopen, with DXC DLLs of a fixed version shipped with games and downloaded by
  CI, when a stage needs shader model 6.

The owner chose this on 2026-10-13.

## Alternatives considered

- **DXC DLLs of a fixed version, shipped with every game and downloaded by CI.** A modern compiler,
  the same everywhere, but two Microsoft binaries to vet, distribute and keep up to date, for a
  shader model nothing uses yet.
- **wgpu's `static-dxc` feature.** Nothing to ship, but a large new dependency, long to build.
- **Keeping `Auto` and logging the compiler.** The behaviour stays machine-dependent.
- **`Dx12Compiler::Fxc.with_env()`.** It accepts `staticdxc`, on which wgpu panics, and `auto`.

## Consequences

- Shaders compile the same way on every Windows machine, CI included, and no DLL found by chance
  can stop the Direct3D 12 backend.
- FXC compiles slowly: the first creation of each pipeline costs more on Direct3D 12 than with DXC.
- Features of shader model 6 (wave operations, 16-bit types in shaders, mesh shaders) are out of
  reach until this is reopened.
