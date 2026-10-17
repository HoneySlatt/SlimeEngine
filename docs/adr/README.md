# Architecture Decision Records

An Architecture Decision Record (ADR) is a short document that records one decision about the
engine's architecture, rules or process: the context that made it necessary, what was decided,
which alternatives were rejected and why, and what the decision costs. ADRs are the project's
memory of *why* things are the way they are. [ADR-0001](0001-record-architecture-decisions.md)
explains why the project uses them.

## When to write one

Write an ADR when a decision:

- shapes how crates depend on each other, or what a crate is responsible for;
- sets a convention every contributor must follow (errors, logging, naming, `unsafe`, lints);
- adds a dependency that other code will be built around (small leaf dependencies are recorded
  in [`docs/dependencies.md`](../dependencies.md) instead);
- changes the roadmap: a stage is reshaped, merged, reordered or dropped;
- would be expensive or confusing to reverse without knowing why it was made.

Do not write one for choices that are local to a function or a module and cheap to change.

## Process

1. The person or agent who needs the decision describes 2 or 3 options, their trade-offs, a
   recommendation, and the cost of changing course later.
2. The project owner (the person who validates direction and architecture) decides.
3. The ADR is written in the same change that first applies the decision, so that it describes
   a decision that is actually in effect.
4. An accepted ADR is never edited to change its meaning. To change a decision, write a new ADR
   that supersedes the old one, and set the old one's status to `Superseded by NNNN`. When a new
   ADR changes only part of an older one, the older one's status becomes
   `Accepted; amended by NNNN`. Fixing a typo, a broken link, or a factual error that leaves the
   decision unchanged is fine; the commit message says what was corrected.

## Format

File name: `NNNN-short-title.md`, numbered in order of acceptance, never reused.

```markdown
# NNNN. Title in the imperative or as a noun phrase

- Status: Accepted | Accepted; amended by NNNN | Superseded by NNNN
- Date: YYYY-MM-DD

## Context

What problem forces a decision now, and what constraints apply.

## Decision

What was decided, stated so that a reader can check whether the code follows it.

## Alternatives considered

Each rejected option and the reason it was rejected.

## Consequences

What becomes easier, what becomes harder, the debt accepted, and the cost of reversing the
decision later.
```

Keep ADRs short. If a decision needs a long explanation, the explanation probably belongs in
[`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) or in the rustdoc of the code it governs, with
the ADR linking to it.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-amend-roadmap-stages-0-to-3.md) | Amend the roadmap for Stages 0 to 3 | Accepted; amended by 0014 |
| [0003](0003-toolchain-edition-and-msrv.md) | Pin the toolchain in one file, use edition 2024, set the MSRV to the pinned version | Accepted |
| [0004](0004-workspace-layout-and-crate-naming.md) | Workspace layout and crate naming | Accepted; amended by 0014, 0055 |
| [0005](0005-lints-and-unsafe-policy.md) | Lints and `unsafe` policy | Accepted |
| [0006](0006-dependency-policy.md) | Dependency policy | Accepted; amended by 0023, 0031 |
| [0007](0007-documentation-tooling-and-ci.md) | Documentation tooling and continuous integration | Accepted; amended by 0015 |
| [0008](0008-errors-logging-and-assertions.md) | Errors, logging and assertions | Accepted; amended by 0019 and 0085 |
| [0009](0009-frame-timing.md) | Frame timing | Accepted |
| [0010](0010-main-loop-ownership.md) | Main loop ownership and the application API | Accepted; amended by 0020 |
| [0011](0011-windowing-with-winit.md) | Windowing and input with winit, behind `slime-platform` | Accepted; amended by 0020 |
| [0012](0012-clean-shutdown-on-termination-signals.md) | Clean shutdown on termination signals | Accepted |
| [0013](0013-windowed-smoke-test-in-weston.md) | Windowed smoke test in a headless Weston compositor | Accepted; amended by 0020, 0024 |
| [0014](0014-amend-roadmap-for-stage-2-and-windows.md) | Amend the roadmap for Stage 2 and for Windows | Accepted |
| [0015](0015-platforms-and-ci.md) | Platforms and CI on Linux and Windows | Accepted |
| [0016](0016-gpu-layer-with-wgpu.md) | The GPU layer: wgpu behind `slime-gfx` | Accepted; amended by 0022, 0031, 0025, 0036 and 0044 |
| [0017](0017-gpu-resource-types.md) | GPU resource types | Accepted; amended by 0026, 0033 and 0044 |
| [0018](0018-reference-image-tests.md) | Reference image tests | Accepted; amended by 0028, 0030 |
| [0019](0019-default-log-filter.md) | Default log filter | Accepted |
| [0020](0020-rendering-in-the-main-loop.md) | Rendering in the main loop, and application errors | Accepted; amended by 0021, 0022, 0025 and 0043 |
| [0021](0021-frame-pacing.md) | Frame pacing | Accepted |
| [0022](0022-losing-the-gpu-device.md) | Losing the GPU device | Accepted; amended by 0025, 0044 and 0075 |
| [0023](0023-dependency-checks-for-the-supported-targets.md) | Check the dependencies of the supported targets only | Accepted |
| [0024](0024-windowed-lifecycle-check-in-weston.md) | Windowed lifecycle check in the headless compositor | Accepted; extended on 2026-10-13 |
| [0025](0025-giving-up-a-lost-surface.md) | Giving up a window's surface that is lost for good | Accepted |
| [0026](0026-resource-binding-and-depth.md) | Resource binding and depth in `slime-gfx` | Accepted; amended by 0032, 0033 and 0047 |
| [0027](0027-the-render-crate.md) | The render crate, what it draws, and glam | Accepted; amended by 0034, 0035, 0037, 0038, 0039, 0040, 0041, 0047 and 0048 |
| [0028](0028-image-tests-tolerate-a-few-pixels.md) | Image tests tolerate a few differing pixels | Accepted; amended by 0030 |
| [0029](0029-temporary-gltf-loading.md) | Temporary glTF loading, in the examples | Superseded by 0069 |
| [0030](0030-image-tests-strict-on-the-reference-driver.md) | Image tests: strict on the reference driver, loose on the others | Accepted; amended by 0054 |
| [0031](0031-dev-overlay-with-egui.md) | The dev overlay with egui, and wgpu for the engine's tooling | Accepted; widened on 2026-10-13; amended by 0052 |
| [0032](0032-mipmaps-computed-at-texture-creation.md) | Mipmaps computed by the engine when a texture is created | Accepted; amended by 0039 and 0071 |
| [0033](0033-gpu-layer-for-multi-pass-rendering.md) | The GPU layer for multi-pass rendering | Accepted; amended by 0038, 0039, 0041, 0047 and 0059 |
| [0034](0034-hdr-rendering-and-tone-mapping.md) | HDR rendering and tone mapping | Accepted; amended by 0035, 0037, 0039, 0040, 0041 and 0051 |
| [0035](0035-physically-based-materials.md) | Physically based materials | Accepted; amended by 0037, 0039 and 0069 |
| [0036](0036-direct3d-12-shader-compiler.md) | The shader compiler of Direct3D 12 | Accepted; amended by 0066 |
| [0037](0037-lights-in-physical-units-and-pre-exposure.md) | Lights in physical units, and pre-exposure | Accepted; amended by 0038, 0039, 0040 and 0042 |
| [0038](0038-shadow-mapping.md) | Shadow mapping | Accepted; amended by 0039, 0041, 0047, 0049, 0050, 0061, 0063 and 0064 |
| [0039](0039-image-based-lighting-computed-on-the-cpu.md) | Image-based lighting computed on the CPU | Accepted; amended by 0041 |
| [0040](0040-bloom.md) | Bloom | Accepted; amended by 0041 |
| [0041](0041-render-graph.md) | The render graph | Accepted |
| [0042](0042-lights-beyond-the-most-drawn-and-checked-scene-values.md) | Lights beyond the most drawn, and checked scene values | Accepted |
| [0043](0043-application-errors-during-a-run.md) | Application errors during a run | Accepted |
| [0044](0044-running-out-of-gpu-memory.md) | Running out of GPU memory | Accepted |
| [0045](0045-one-linear-color-type.md) | One linear color type | Accepted; amended by 0079, 0084 |
| [0046](0046-constructors-for-the-structs-a-game-fills-in.md) | Constructors for the structs a game fills in | Accepted |
| [0047](0047-reversed-depth-with-an-infinite-far-plane.md) | Reversed depth with an infinite far plane | Accepted |
| [0048](0048-rendering-relative-to-the-camera.md) | Rendering relative to the camera | Accepted; amended by 0079 |
| [0049](0049-instances-that-cast-no-shadow.md) | Instances that cast no shadow | Accepted; amended by 0050 |
| [0050](0050-a-shadow-region-that-follows-the-camera.md) | A shadow region that follows the camera | Accepted; amended by 0063, 0064 and 0065 |
| [0051](0051-dithering-8-bit-outputs.md) | Dithering 8-bit outputs | Accepted |
| [0052](0052-amend-roadmap-before-0-1.md) | Amend the roadmap before 0.1 | Accepted; amended by 0056, 0064 and 0067 |
| [0053](0053-one-home-per-fact.md) | One home per fact in the documentation | Accepted |
| [0054](0054-image-tests-allow-0-2-percent-on-other-drivers.md) | Image tests allow 0.2 % of the pixels to differ on other drivers | Accepted |
| [0055](0055-games-in-the-workspace.md) | Games in the workspace | Accepted; amended by 0067 |
| [0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) | Stage 3d: anti-aliasing and the shadows of spot lights | Accepted; amended by 0058 and 0064 |
| [0057](0057-what-the-first-game-asks-of-the-next-stages.md) | What the first game asks of the next stages | Accepted |
| [0058](0058-shadows-for-every-spot-light-in-one-atlas.md) | Shadows for every spot light, in one atlas | Accepted |
| [0059](0059-multisampling-in-the-gpu-layer.md) | Multisampling in the GPU layer | Accepted; amended by 0060 |
| [0060](0060-resolving-the-samples-weighed-by-their-brightness.md) | Resolving the samples weighed by their brightness | Accepted |
| [0061](0061-projecting-the-shadows-of-spot-lights.md) | Projecting the shadows of spot lights | Accepted; amended by 0062 and 0063 |
| [0062](0062-looking-spot-shadows-up-off-the-triangle.md) | Looking spot lights' shadows up off the triangle | Accepted; amended by 0063 and 0065 |
| [0063](0063-a-smoother-shadow-filter.md) | A smoother shadow filter | Accepted; amended by 0064 |
| [0064](0064-cascaded-shadow-maps.md) | Cascaded shadow maps | Accepted; amended by 0065 |
| [0065](0065-reading-the-cascades.md) | Reading the cascades | Accepted |
| [0066](0066-shipping-dxc-beside-the-programs.md) | Shipping DXC beside the programs | Accepted |
| [0067](0067-hunna-wood-replaces-nightwatch.md) | Hunna Wood replaces Nightwatch as the sample game | Accepted |
| [0068](0068-stage-4-in-two-halves.md) | Stage 4 in two halves, and audio import at Stage 7 | Accepted; amended by 0078 |
| [0069](0069-the-asset-pipeline.md) | The asset pipeline: identity, cooking and what a game loads | Accepted; amended by 0080 |
| [0070](0070-the-sample-games-source-assets.md) | The sample game's source assets: free, listed, mirrored | Accepted |
| [0071](0071-cooked-textures.md) | Cooked textures | Accepted |
| [0072](0072-stage-4b-live-assets.md) | Stage 4b: live assets | Accepted; amended by 0078 |
| [0073](0073-loading-scenes-in-the-background.md) | Loading scenes in the background | Accepted; amended by 0074 |
| [0074](0074-uploading-textures-without-holding-up-frames.md) | Uploading textures without holding up frames | Accepted |
| [0075](0075-recovering-from-a-lost-gpu-device.md) | Recovering from a lost GPU device | Accepted |
| [0076](0076-hot-reload-of-cooked-data.md) | Hot reload of cooked data | Accepted; amended by 0084 |
| [0077](0077-hot-reload-of-shaders.md) | Hot reload of shaders | Accepted |
| [0078](0078-stage-5-in-two-halves.md) | Stage 5 in two halves | Accepted; amended by 0090 |
| [0079](0079-the-engines-own-world.md) | The engine's own world | Accepted; amended by 0087 |
| [0080](0080-reflection-and-serialization.md) | Reflection and serialization | Accepted; amended by 0085 and 0086 |
| [0081](0081-stage-5a-the-world.md) | Stage 5a: the world | Accepted |
| [0082](0082-the-worlds-storage-and-handles.md) | The world's storage and handles | Accepted |
| [0083](0083-transforms-and-the-hierarchy.md) | Transforms and the hierarchy | Accepted; amended by 0087 |
| [0084](0084-what-the-world-draws.md) | What the world draws | Accepted |
| [0085](0085-reflection.md) | Reflection | Accepted; amended by 0086 and 0087 |
| [0086](0086-serialization.md) | Serialization | Accepted; amended by 0087 and 0091 |
| [0087](0087-the-worlds-documents.md) | The world's documents | Accepted; amended by 0088 and 0089 |
| [0088](0088-saving-and-resuming-a-run.md) | Saving and resuming a run | Accepted |
| [0089](0089-scenes-and-prefabs.md) | Scenes and prefabs | Accepted |
| [0090](0090-stage-5b-scenes-and-prefabs.md) | Stage 5b: scenes and prefabs | Accepted |
| [0091](0091-scene-documents-and-placements.md) | Scene documents and placements | Accepted |
| [0092](0092-a-world-written-back-as-a-scene.md) | A world written back as a scene | Accepted |
