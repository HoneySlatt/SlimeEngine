# 0002. Amend the roadmap for Stages 0 to 3

- Status: Accepted
- Date: 2026-10-12

## Context

Before any code was written, the first stages of [`docs/ROADMAP.md`](../ROADMAP.md) were reviewed
for inconsistencies, badly ordered dependencies and risks. The review found problems that would
either make an exit criterion unreachable or lead to building things before anything uses them:

1. A Wayland compositor does not show a window until the client attaches a buffer of pixels to
   it. Stage 1 excludes rendering and Stage 2 introduces presentation, so the Stage 1 exit
   criterion "a window opens on Wayland" could not be met as written.
2. Stage 1 listed IDs and handles, configuration, a filesystem layer, timers and system
   information. None of them has a user in Stage 1: handles are first needed by the GPU layer
   (Stage 2), configuration files need a serialization format (Stages 4–5), a virtual filesystem
   belongs to the asset pipeline (Stage 4), and the standard library already provides timers and
   file access.
3. With windowing done by winit, who owns the main loop is the central design question of
   Stage 1, because it fixes the shape of the API that games implement. The roadmap did not
   mention it.
4. Without presentation there is no vertical synchronisation, so a windowed loop with nothing to
   wait on spins at 100% of a CPU core.
5. Stage 0 did not check documentation or dependency licenses, although documentation is part of
   every deliverable and the license policy is a Stage 0 policy. Its exit criterion, "CI is green
   on the empty workspace", proves nothing.
6. The dependency table made `gfx` depend on `platform`, but Stage 2 must compare offscreen
   renders in CI, where there is no window.
7. Continuous integration machines have no GPU, so the Stage 2 image comparison needs a software
   driver, whose output is not bit-identical to a hardware driver's.
8. Stage 2 hot-reloaded WGSL, while Stage 4 introduces hot reload for all assets, shaders
   included: two reload mechanisms, the first one thrown away. Stage 2 also listed a full set of
   resource handles before the renderer that will use them exists.
9. Stage 3 bundled a simple renderer, physically based rendering, shadows, HDR, a render graph, a
   debug overlay and a glTF loader into one release, and designed the render graph before the
   passes it would order.

## Decision

The roadmap is amended as follows.

- **Stage 0.** CI also checks rustdoc warnings, doc tests, Markdown links, dependency licenses
  and security advisories. A documentation baseline is part of the stage. New exit criterion: CI
  is green on a workspace containing the skeletons of the Stage 1 crates, the policies exist as
  ADRs, and the documentation baseline exists and is checked by CI. (Problem 5.)
- **Stage 1.** IDs and handles, configuration, filesystem and timers are removed and move to the
  stage that first needs them. System information is reduced to a startup log line. The
  ownership of the main loop is decided by its own ADR. A frame limiter paces the windowed loop
  until presentation does it in Stage 2. The Stage 1 sample makes its window visible by filling it
  with a solid color from the CPU through `softbuffer`; this dependency is limited to the sample
  and removed in Stage 2. (Problems 1 to 4.)
- **Dependency table.** `gfx` depends on `core` and receives window handles through the
  `raw-window-handle` traits, not through `platform`. (Problem 6.)
- **Stage 2.** WGSL hot reload moves to Stage 4, which already covers shader hot reload. Only the
  handles needed by the triangle and the offscreen render are created; the set grows with the
  renderer. (Problem 8.)
- **Stage 2 exit.** Images are rendered in CI with Mesa's software Vulkan driver (lavapipe),
  reference images are produced with it, and comparisons use a tolerance. (Problem 7.)
- **Stage 3** is split into Stage 3a, a first renderer released as 0.0.3, and Stage 3b, the
  physically based renderer released as 0.1. The render graph is extracted from concrete passes at
  the end of Stage 3b, once at least three passes exist. (Problem 9.)

## Alternatives considered

- **Bring a wgpu "clear the window" step into Stage 1** instead of the CPU fill. Rejected: wgpu
  would enter the engine before its boundary (Stage 2) has been designed, and Stage 1 would grow
  a GPU dependency that headless runs do not need.
- **Accept an invisible window in Stage 1.** Rejected: the exit criterion would be unverifiable
  on the project's first platform.
- **Keep the Stage 1 items and build them early.** Rejected: they would be designed without a
  user, against the rule "no abstraction without a second concrete use".
- **Keep Stage 3 whole.** Rejected: a release containing that much work goes a long time without
  a runnable, tested sample, against principle 4 of the roadmap.

## Consequences

- Stage 1 is smaller and every item in it has a user in the Stage 1 sample.
- The Stage 1 sample carries a temporary dependency (`softbuffer`) and a temporary frame limiter;
  both are debt with a known removal date (Stage 2).
- Stage 2 has to provide lavapipe in the CI environment, which the Nix development shell makes
  straightforward.
- The release map gains a version (0.0.3).
- Other problems found by the same review are left for the stages concerned: Stage 3 renders a
  scene before the world model exists (Stage 5); the asset pipeline (Stage 4) needs to serialize
  metadata before the serialization work of Stage 5; and the basic task pool of Stage 4 is
  replaced by the job system of Stage 7. They will be decided when those stages are planned.
- Reversing any of these amendments is a documentation change, done through a new ADR.
