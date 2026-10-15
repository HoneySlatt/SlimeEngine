# 0066. Shipping DXC beside the programs

- Status: Accepted; amends [0036](0036-direct3d-12-shader-compiler.md) (FXC by default)
- Date: 2026-10-15

## Context

On Direct3D 12, wgpu translates the engine's shaders to HLSL and has one of Microsoft's two
compilers compile them: FXC, part of Windows, or DXC, a `dxcompiler.dll` that the program ships.
[ADR-0036](0036-direct3d-12-shader-compiler.md) chose FXC, so that every machine compiles shaders
the same way, and left DXC to the developer who asks for it with `WGPU_DX12_COMPILER=dxc`.

*Nightwatch*'s scene pass took 2.9 ms on Direct3D 12 and 1.2 ms on Vulkan, on the same GPU, and
[ADR-0057](0057-what-the-first-game-asks-of-the-next-stages.md) asked Stage 3d to tell whether FXC
was the cause. In the owner's Windows virtual machine, on an RX 6900 XT, over 600 frames of the
game's first view at 1920 × 1080, the scene pass took:

| | Before Stage 3d | After Stage 3d |
|---|---|---|
| Direct3D 12, FXC | 2.89 ms | 3.74 ms |
| Direct3D 12, DXC 1.9.2609 | 1.15 ms | 1.97 ms |
| Vulkan | 1.15 ms | 1.79 ms |

At 3840 × 2054 with FXC, it took 10.4 ms, two thirds of a frame at 60 Hz. The other passes cost
about the same with either compiler. FXC's code is the cause.

What DXC asks of a program, as of its release 1.9.2609 and wgpu 30:

- `dxcompiler.dll` alone, about 21 MiB (10 MB compressed). Since DXC 1.8.2505 it validates and
  signs shaders itself, without `dxil.dll`, which wgpu 30 never loads.
- Its licence, the University of Illinois/NCSA licence of LLVM, allows it to be shipped, with its
  text beside it.
- wgpu loads it from the path it is given. A path that does not load leaves no Direct3D 12 adapter;
  wgpu's default, `Auto`, falls back to FXC when it finds no `dxcompiler.dll`, but takes any that
  Windows' search finds, on `PATH` too, whatever its version.
- With DXC, wgpu offers no adapter whose driver lacks shader model 6, which FXC's code does not
  need.

## Decision

- **DXC when its library lies beside the program, FXC otherwise.** The engine looks for
  `dxcompiler.dll` in the directory of the running program and, if it is there, gives wgpu its
  full path, so that Windows loads that very file and no other. Without it, FXC compiles the
  shaders, and a warning in the log says so. The compiler is logged at the `info` level
  (`create_instance` in `crates/slime-gfx/src/gpu.rs`).
- **`WGPU_DX12_COMPILER` still chooses**: `fxc` asks for FXC, `dxc` or `dynamicdxc` for DXC
  wherever Windows finds `dxcompiler.dll`. Its other values are ignored, as ADR-0036 decided.
- **The engine's programs ship DXC 1.9.2609.** `scripts/fetch-dxc.sh` downloads Microsoft's
  release archive, checks it against the SHA-256 written in the script, and copies
  `dxcompiler.dll` with its licence, as `dxcompiler-LICENSE.txt`, into the directories it is
  given. CI puts it beside the tests and the samples on Windows, which then run DXC's code on WARP,
  and beside the programs it publishes, in one directory. A newer DXC is taken by changing the
  version and the checksum in the script.

The owner chose this on 2026-10-15.

## Alternatives considered

- **wgpu's `static-dxc` feature**, which links DXC into the program: nothing to ship, but its build
  downloads a prebuilt library of 26 MB from one maintainer's releases at every clean build, and
  that DXC dates from 2023, older than the 1.8.2502 that wgpu documents as its minimum.
- **Keeping FXC**: nothing to ship, and the scene pass twice as slow on Direct3D 12.
- **Vulkan by default on Windows**: the fastest measured, but CI, which renders with WARP, could
  not test it, and Vulkan drivers on Windows vary more than Direct3D 12's.
- **wgpu's `Auto`**: a `dxcompiler.dll` found on `PATH`, of any version, would compile the shaders.
- **DXC by its name rather than its full path**: a program without the library would have no
  Direct3D 12 at all.
- **Shipping `dxil.dll` too**: DXC no longer needs it.

## Consequences

- On Direct3D 12, a program shipped with DXC draws its scene pass about twice as fast, close to
  Vulkan.
- The programs' directory holds 21 MiB more. A program shipped without `dxcompiler.dll` runs with
  FXC, slower, and says so in its log; a game made outside this repository puts the library beside
  its program itself, with `scripts/fetch-dxc.sh` or by hand.
- A `dxcompiler.dll` beside the program that does not load leaves no Direct3D 12 adapter, and so
  does, with DXC, a driver without shader model 6.
- Shader model 6's features come within reach, which nothing uses yet.
- The Windows CI job downloads 32 MB from GitHub at every run.
