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
| [0004](0004-workspace-layout-and-crate-naming.md) | Workspace layout and crate naming | Accepted; amended by 0014 |
| [0005](0005-lints-and-unsafe-policy.md) | Lints and `unsafe` policy | Accepted |
| [0006](0006-dependency-policy.md) | Dependency policy | Accepted; amended by 0023, 0031 |
| [0007](0007-documentation-tooling-and-ci.md) | Documentation tooling and continuous integration | Accepted; amended by 0015 |
| [0008](0008-errors-logging-and-assertions.md) | Errors, logging and assertions | Accepted; amended by 0019 |
| [0009](0009-frame-timing.md) | Frame timing | Accepted |
| [0010](0010-main-loop-ownership.md) | Main loop ownership and the application API | Accepted; amended by 0020 |
| [0011](0011-windowing-with-winit.md) | Windowing and input with winit, behind `slime-platform` | Accepted; amended by 0020 |
| [0012](0012-clean-shutdown-on-termination-signals.md) | Clean shutdown on termination signals | Accepted |
| [0013](0013-windowed-smoke-test-in-weston.md) | Windowed smoke test in a headless Weston compositor | Accepted; amended by 0020, 0024 |
| [0014](0014-amend-roadmap-for-stage-2-and-windows.md) | Amend the roadmap for Stage 2 and for Windows | Accepted |
| [0015](0015-platforms-and-ci.md) | Platforms and CI on Linux and Windows | Accepted |
| [0016](0016-gpu-layer-with-wgpu.md) | The GPU layer: wgpu behind `slime-gfx` | Accepted; amended by 0022, 0031, 0025, 0036 |
| [0017](0017-gpu-resource-types.md) | GPU resource types | Accepted; amended by 0026, 0033 |
| [0018](0018-reference-image-tests.md) | Reference image tests | Accepted; amended by 0028, 0030 |
| [0019](0019-default-log-filter.md) | Default log filter | Accepted |
| [0020](0020-rendering-in-the-main-loop.md) | Rendering in the main loop, and application errors | Accepted; amended by 0021, 0022, 0025 |
| [0021](0021-frame-pacing.md) | Frame pacing | Accepted |
| [0022](0022-losing-the-gpu-device.md) | Losing the GPU device | Accepted; amended by 0025 |
| [0023](0023-dependency-checks-for-the-supported-targets.md) | Check the dependencies of the supported targets only | Accepted |
| [0024](0024-windowed-lifecycle-check-in-weston.md) | Windowed lifecycle check in the headless compositor | Accepted; extended on 2026-10-13 |
| [0025](0025-giving-up-a-lost-surface.md) | Giving up a window's surface that is lost for good | Accepted |
| [0026](0026-resource-binding-and-depth.md) | Resource binding and depth in `slime-gfx` | Accepted; amended by 0032, 0033 |
| [0027](0027-the-render-crate.md) | The render crate, what it draws, and glam | Accepted; amended by 0034 and 0035 |
| [0028](0028-image-tests-tolerate-a-few-pixels.md) | Image tests tolerate a few differing pixels | Accepted; amended by 0030 |
| [0029](0029-temporary-gltf-loading.md) | Temporary glTF loading, in the examples | Accepted; amended by 0035 |
| [0030](0030-image-tests-strict-on-the-reference-driver.md) | Image tests: strict on the reference driver, loose on the others | Accepted |
| [0031](0031-dev-overlay-with-egui.md) | The dev overlay with egui, and wgpu for the engine's tooling | Accepted; widened on 2026-10-13 |
| [0032](0032-mipmaps-computed-at-texture-creation.md) | Mipmaps computed by the engine when a texture is created | Accepted |
| [0033](0033-gpu-layer-for-multi-pass-rendering.md) | The GPU layer for multi-pass rendering | Accepted |
| [0034](0034-hdr-rendering-and-tone-mapping.md) | HDR rendering and tone mapping | Accepted; amended by 0035 |
| [0035](0035-physically-based-materials.md) | Physically based materials | Accepted |
| [0036](0036-direct3d-12-shader-compiler.md) | The shader compiler of Direct3D 12 | Accepted |
