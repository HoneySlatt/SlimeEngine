# 0114. Hardware tiers and the GPU's limits

- Status: Accepted; amends [0016](0016-gpu-layer-with-wgpu.md) (the device asks for the adapter's
  limits, above a floor checked at startup),
  [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) (three hardware tiers rather
  than a minimum and a target), [0098](0098-the-games-the-engine-aims-at.md) (a handheld tier below
  the modest one) and [0104](0104-the-build-ci-and-measurements.md) (benchmarks measured at the
  tiers' resolutions and compared with their budgets); amended by
  [0120](0120-a-frame-that-owns-what-it-draws.md) (the floor's figures: WebGPU's default limits) and
  [0138](0138-the-high-tier-at-3840-2160.md) (the high tier at 3840 × 2160)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) writes the engine's hardware tiers, with budgets scaled from the
owner's RX 6900 XT (C2-03). [ADR-0098](0098-the-games-the-engine-aims-at.md) says that the engine
runs on *hardware tiers*, from modest computers at high frame rates to high-end ones, that a game
picks its minimum tier, and that each technique of the renderer belongs to a tier; it asks for a
modest minimum and a high-end tier, with no machine or figure. ADR-0097 asks for a minimum and a
target tier, with budgets scaled from the RX 6900 XT's measurements, and keeps NVIDIA's and Intel's
GPUs untried for now, the owner having no other machine, as known debt.

- **The device asks for WebGPU's default limits** ([ADR-0016](0016-gpu-layer-with-wgpu.md)), the
  limits of the web's graphics standard, and for two optional features only, when the adapter offers
  them: timestamp queries ([ADR-0031](0031-dev-overlay-with-egui.md)) and the compressed formats of
  cooked textures ([ADR-0069](0069-the-asset-pipeline.md)). In wgpu 30, the default limits allow 4
  bind groups per pipeline, all of which the scene pass uses ([ADR-0038](0038-shadow-mapping.md)),
  16 sampled textures and 8 storage buffers per shader stage, 128 MiB per storage binding, 256 MiB
  per buffer, and offsets aligned on 256 bytes. Each instance takes a slot of 256 bytes, half of it
  padding, and the renderer panics past about a million instances, which fill 256 MiB. ADR-0016 left
  WebGPU itself, the web, out, and ADR-0098 aims at no mobile device; every desktop GPU of the last
  ten years offers more than these limits, and so do lavapipe and WARP, the drivers that render on
  the CPU in the checks.
- **No tier has hardware or budgets.** The audit measured the renderer against a GPU budget of
  14 ms per frame without a machine to hold it to. The only GPU at hand is the owner's RX 6900 XT,
  which in rasterized rendering, by triangles without ray tracing, stands about where an RTX 4070 Ti
  or an RX 7800 XT does.

The owner took three decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**The device asks for the adapter's limits, above a floor.** `slime-gfx` asks the adapter for all
the limits it offers. A *floor* of limits, written in the engine, is checked when a device is
opened: below it, the program refuses to start, with an error that names the limit lacking and the
value needed, rather than failing during a game. The floor is low enough for the desktop GPUs of the
three tiers that follow and for lavapipe and WARP; its figures are set when it is built, from what
those offer, and `slime-gfx`'s rustdoc holds them. Modern features, resources reached by an index
(bindless), draws counted by the GPU, 64-bit atomics, ray tracing, are not asked for now: each
enters the floor, or a tier, with the first technique that needs it, at Stages 9a, 9c and 10a,
measured on Direct3D 12 too. What the WARP of CI's Windows runner offers is read, not assumed.

**Three tiers.** Each names a reference machine; the ratio to the RX 6900 XT, in rasterized
rendering, comes from averages of public benchmarks, to be checked on the hardware when there is
some.

| Tier | Reference machine | About the RX 6900 XT's |
|---|---|---|
| Handheld | Steam Deck: a GPU built into its processor, of AMD's RDNA 2 architecture, as the RX 6900 XT is, of about 1.6 TFLOPS (trillions of operations on floating-point numbers per second, against about 23 for the RX 6900 XT), 16 GB of memory shared with the processor, 4 cores, a 1280 × 800 screen | a tenth |
| Modest | the class of an RTX 3060 or an RX 6600: 8 GB of video memory, 6 cores, 16 GB of memory | a half |
| High | the class of an RX 6900 XT, an RTX 4070 Ti or an RX 7800 XT: 16 GB of video memory, 8 cores, 32 GB of memory | one |

The handheld tier is a PC that runs Linux, a fixed machine that is the same for every player and
cheap to obtain for tests. An *enthusiast* tier, of the class of an RTX 4090, at 3840 × 2160
with ray tracing, is added with Stage 9c, when the engine traces rays.

**Budgets per tier.** A frame's resolution and rate, for the two kinds of game that ADR-0098 sets
the widest apart:

| Tier | A realistic game | A competitive shooter |
|---|---|---|
| Handheld | 1280 × 800 at 30 frames per second, 33.3 ms | 1280 × 800 at 60, 16.7 ms |
| Modest | 1920 × 1080 at 60, 16.7 ms | 1920 × 1080 at 144, 6.9 ms |
| High | 2560 × 1440 at 60, 16.7 ms | 2560 × 1440 at 144, 6.9 ms |

- The GPU takes at most about 85 % of a frame, 14 ms at 60 frames per second; the main thread, its
  simulation, extraction and the recording of the GPU's work, at most about 70 %, the CPU's and the
  GPU's work overlapping; the rest is a margin.
- 99 % of frames keep the frame's time and the budgets above; no frame takes more than twice the
  frame's time; and a load in the background makes no frame miss its time, as ADR-0098 asks of a
  hidden load.
- Memory:

| Tier | Video memory for assets | Memory of the game |
|---|---|---|
| Handheld | 4 GB, of 16 shared | 8 GB |
| Modest | 6 GB, of 8 | 12 GB, of 16 |
| High | 12 GB, of 16 | 24 GB, of 32 |

The asset cache's budgets of [ADR-0113](0113-asset-requests-residency-and-priorities.md) default to
the video memory for assets of the game's minimum tier, which the cache splits between textures and
meshes when it is built. A technique of the renderer says from which tier it is
available, and what replaces it below.

**Budgets are checked on the RX 6900 XT, scaled.** A tier's budget, multiplied by its ratio, gives
what the RX 6900 XT may take at that tier's resolution: the modest tier's 14 ms of GPU at 1920 ×
1080 are about 7 ms on the RX 6900 XT at that size, the handheld tier's 28 ms at 1280 × 800 about
2.8 ms at that size. `slime-benches` measures at the tiers' resolutions, and
[`measurements.md`](../measurements.md) compares each figure with its scaled budget. Those
measurements are among the owner's checks by hand, at each milestone that changes rendering or
loading: CI runs no benchmark ([ADR-0104](0104-the-build-ci-and-measurements.md)), and its drivers
render on the CPU. One GPU tried stays known debt; renting a GPU in the cloud, obtaining a Steam
Deck, the handheld tier's own machine, or contributors with other GPUs end it. A runner of CI with a
real GPU comes when contributors do.

**When it is built.** The floor of limits and the adapter's limits at Stage 6, with the frame
contract ([ADR-0111](0111-the-frame-contract.md)): the adapter's larger buffers raise the bound of a
million instances, which instancing's storage buffers then decide;
the benchmarks at the tiers' resolutions with the first milestone of Stage 6 that measures
rendering.

## Alternatives considered

The GPU's limits:

- **WebGPU's default limits kept, every modern feature optional with a fallback**: runs everywhere,
  but two paths for all that matters, the bound of a million instances kept, and a price paid for
  the web, which is not aimed at.
- **A modern floor now**, of the class of Vulkan 1.2 and Direct3D 12's shader model 6.6: one path,
  driven by the GPU; but decided before any technique uses it, wgpu 30 offering on Direct3D 12 less
  than that floor needs, and CI's Windows job having to carry a downloaded WARP under Microsoft's
  license.

The tiers:

- **Two tiers, a minimum and a target**: two sets of budgets, but nothing for handheld PCs, the most
  concrete modest computer, and one that runs Linux.
- **Four tiers now, the enthusiast's among them**: the whole scale at once, but a tier with no
  technique of its own before ray tracing at Stage 9c.

The budgets:

- **Tiers without the engine's budgets**, each game setting its own: flexible, but a technique of
  the engine has no figure to be measured against, and "it belongs to the modest tier" says nothing.
- **A runner of CI with a real GPU now**, on the owner's machine or rented: GPU regressions caught
  at each commit; but a runner on the owner's desktop runs each commit's code with the owner's
  rights, beside the owner's session, and loads the GPU while the owner uses it; a rented GPU costs
  every month.

## Consequences

- A program refuses to start on a GPU below the floor, saying why; above it, the renderer is no
  longer bound by WebGPU's defaults.
- Each technique of the renderer from Stage 6 on states its tier and its fallback, and is measured
  against the budgets scaled to the RX 6900 XT.
- The ratios are estimates until the tiers' hardware is measured; [`docs/PLAN.md`](../PLAN.md)'s
  known debt says so beside the single GPU tried.
- The asset cache's default budgets follow the minimum tier of the game.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6 asks for the adapter's limits above a floor;
  Stage 9a sets the quality of each tier; Stage 9c adds the enthusiast tier.
