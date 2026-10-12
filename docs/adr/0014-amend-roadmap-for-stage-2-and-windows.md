# 0014. Amend the roadmap for Stage 2 and for Windows

- Status: Accepted
- Date: 2026-10-12

## Context

Planning Stage 2 (the GPU layer) raised two kinds of problems with
[`docs/ROADMAP.md`](../ROADMAP.md).

First, measurements on the development machine contradict parts of the Stage 2 text:

1. On Wayland, with Mesa's Vulkan driver, a surface offers only the FIFO (synchronised with the
   display) and MAILBOX present modes. The compositor in use, niri, does not provide the Wayland
   protocols that let FIFO work without frame callbacks (`wp_fifo_v1`, `wp_commit_timing_v1`), and
   compositors send no frame callbacks to hidden windows. wgpu then waits up to one second for each
   frame. If presentation paced the main loop, as the roadmap said ("replaces the Stage 1 frame
   limiter"), a hidden window would slow the whole engine (simulation, input, signal handling) to
   about one frame per second.
2. "Engine-owned handles" presupposed a representation (identifiers into tables) before anything
   needs one. Owning wrappers around wgpu objects are enough for the triangle and the offscreen
   render; a table of identifiers has its first user later (a render graph, asset streaming).
3. "WGSL loading" presupposed a policy for data files, which only the asset pipeline (Stage 4)
   defines.
4. The dependency table made `gfx` depend on `core`, a crate that does not exist and that `gfx`
   does not need.

Second, the owner set a new requirement: as soon as the engine can produce a complete game, even a
very small one, it must produce it for Windows as well as Linux, since most players use Windows.
The roadmap only listed Windows in Stage 11 (0.9), and the first brief said "Windows later". Adding
a platform late is expensive, because assumptions about the first platform accumulate in the code,
the tools and the tests.

## Decision

The roadmap is amended as follows.

- **A1. Stage 2, frame pacing.** Presentation replaces the Stage 1 CPU fill, but frame pacing is
  decided by measurement on each platform; the Stage 1 frame scheduler may stay.
- **A2. Stage 2, resources.** "Engine-owned handles" becomes "engine-owned resource types", whose
  representation is decided by ADR.
- **A3. Stage 2, shaders.** WGSL shaders are compiled from source text, with readable errors;
  loading them from files comes with the asset pipeline (Stage 4), with hot reload.
- **A4. Dependency table.** `gfx` depends on no engine crate.
- **A5. Platforms.** Linux and Windows are both first platforms: games run on both from Stage 2
  on. macOS comes later. Stage 2 uses Vulkan on Linux and Direct3D 12 on Windows.
- **A6. Stage 11** keeps only macOS as a platform to add.
- **A7. Continuous track "Platforms".** Every stage's sample builds and its tests pass in CI on
  Linux and Windows, and windowed behaviour is checked by hand on both. The Stage 2 exit criterion
  names both platforms and both software drivers used by CI (lavapipe and WARP). Stage 9 packages
  builds for Linux and Windows, and builds without a console (Windows games) log to a file.

## Alternatives considered

- **Keep Windows for Stage 11.** Rejected by the owner: games must reach Windows players from the
  first versions able to make one.
- **Add Windows at the first stage that can make a game (around Stage 7).** Every Linux assumption
  made before then would have to be found and removed at once; continuous support costs a CI job
  now and is cheap while the code is small.
- **Keep "frame pacing replaces the frame limiter".** It could stall the engine whenever the window
  is hidden on the owner's own compositor.

## Consequences

- From Stage 2 on, every milestone is checked on Windows: a CI job builds and tests on a Windows
  runner, and the owner checks windowed behaviour in a Windows virtual machine.
- Windows-specific behaviour enters the plan now. The first case found: Windows runs a modal loop
  during an interactive resize or move, during which only redraw events reach the application, so
  frames must be driven by redraw requests (decided in Stage 2).
- CI takes longer and, while the repository is private, Windows runner minutes cost twice as much.
- macOS remains open, and its support will cost what Windows support would have cost late.
