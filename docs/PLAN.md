# Plan

This file says what is being worked on now. It lists the milestones of the current stage, their
status, how each one is checked, and the known debt. Direction beyond this stage is in
[`ROADMAP.md`](ROADMAP.md); decisions are in [`adr/`](adr/README.md). A *stage* is a step of the
roadmap; a *milestone* is a small, verifiable step of a stage.

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread. From Stage 2 on, every milestone is also checked on Windows
([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage 3a — First renderer → 0.0.3: in progress. Planned on 2026-10-12 and validated by the
  owner. M3.1 and M3.2 are done; next: M3.3.
- Between 0.0.2 and this stage, a stabilization pass on 2026-10-12 corrected what an audit of
  0.0.2 found: the corrections are listed in [`CHANGELOG.md`](../CHANGELOG.md) under
  *Unreleased*, and what was found without being corrected is in [Known debt](#known-debt).

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M3.1](#m31--gpu-resources-for-a-renderer) | GPU resources for a renderer | Done |
| [M3.2](#m32--the-render-crate-camera-meshes-instances) | The render crate: camera, meshes, instances | Done |
| [M3.3](#m33--directional-light-and-material) | Directional light and material | Planned |
| [M3.4](#m34--textures-and-samplers) | Textures and samplers | Planned |
| [M3.5](#m35--a-gltf-scene-and-camera-control) | A glTF scene and camera control | Planned |
| [M3.6](#m36--statistics-gpu-timings-and-the-dev-overlay) | Statistics, GPU timings and the dev overlay | Planned |
| [M3.7](#m37--windows-check-documentation-release-003) | Windows check, documentation, release 0.0.3 | Planned |

Stages 0, 1 and 2 are done; see [Completed stages](#completed-stages).

## Stage 3a — First renderer → 0.0.3

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-3a--first-renderer--003)): a glTF scene
renders with textures, depth and simple lighting under a moving camera, with statistics shown in
the overlay, covered by image tests.

Out of scope, left to the stages that need them: physically based materials, shadows, HDR and
post-processing, a render graph, multisampling, transparency, animation and skinning, glTF
extensions (Stage 3b and later); mipmaps, unless the image tests or the sample show aliasing that
the tolerance cannot absorb; and a scene model, which comes with the world of Stage 5: the
renderer draws a flat list of instances that the application builds, and the temporary glTF loader
flattens the file's node hierarchy into such a list.

Decisions taken when this stage was planned, each recorded as an ADR in the milestone that applies
it:

- Crates. A new `slime-render` crate holds the renderer: meshes, materials, camera, light,
  statistics. It is built on `slime-gfx`'s engine-owned types only, without wgpu, so that the
  renderer is the second concrete use of each of them. The dev overlay lives in a temporary
  `slime-devui` crate, the only crate besides `slime-gfx` that reaches wgpu, through a feature of
  `slime-gfx` documented as internal and removed with the overlay (amends ADR-0016).
- GPU resources (M3.1). Typed buffers (`IndexBuffer`, `UniformBuffer`) rather than one generic
  buffer; bind group layouts and bind groups as engine-owned types; depth targets; a pass
  descriptor that says whether the target is cleared or kept (amends ADR-0017).
- Math. `glam`; its types appear in `slime-render`'s API, the exception that ADR-0006 allows for a
  dependency whose purpose it is.
- glTF. The `gltf` crate without its default features (neither `image` nor `base64`), in the
  examples only: GLB files with embedded buffers, PNG images decoded with `png`. The loader is
  deleted in Stage 4.
- Overlay. `egui` and `egui-wgpu`, the latter without its default features (they would enable
  every wgpu backend), and no `egui-winit`: `slime-devui` translates the engine's events itself,
  since winit's types do not leave `slime-platform` (ADR-0011).
- GPU timings. wgpu's timestamp queries, written by the engine rather than taken from the
  `wgpu-profiler` crate, requested only when the adapter offers them; everything works without
  them (amends ADR-0016, which asked for no optional feature).
- Sample asset. A small scene made in Blender under CC0, under 500 kB, embedded in the sample;
  `--scene <file>` loads another GLB file. A Khronos sample model under CC-BY 4.0 was the
  alternative.
- No `bytemuck`: bytes are produced with `to_le_bytes`, as ADR-0017 chose; no `image` crate.
- Windows: the Windows CI job runs the tests and the headless samples at every milestone; the owner
  checks windowed behaviour in the virtual machine at M3.2, M3.5 and M3.6.

### M3.1 — GPU resources for a renderer

- **Goal.** Everything a 3D renderer needs from `slime-gfx`, proven offscreen on lavapipe and WARP.
- **Crates.** `slime-gfx`; the examples and tests that call `Gpu::render`.
- **Delivers.** Index buffers and indexed draws; uniform buffers, written through the `Gpu` whenever
  their content changes; bind group layouts and bind groups, holding uniform buffers now (textures
  and samplers come in M3.4, dynamic offsets when M3.2 needs them); the pipeline descriptor gains
  its bind group layouts, a depth test and a cull mode; depth targets; `Gpu::render` takes a pass
  descriptor: the target, whether to clear it or keep its content, and an optional depth target.
  ADR: resource binding and depth (amends 0017).
- **Done when.** `slime-gfx` tests pass on lavapipe and on WARP: an indexed quad fills the target; a
  uniform offset moves a triangle between two renders; with the depth test, the nearer of two
  overlapping triangles wins whatever the draw order, and the later one wins without it; back
  faces are culled when asked; a pipeline whose shader bindings do not match its layouts is
  rejected as `GfxError::Pipeline`.
- **How to test.** `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test -p slime-gfx`.
- **Checked** on 2026-10-12: the tests of `crates/slime-gfx/tests/drawing.rs` (an indexed quad; a
  uniform offset written between two renders; a pass that keeps the image; the depth test with
  both draw orders, and the last draw winning without it; culling of back and of front faces; a
  depth target of another size rejected) and of `errors.rs` (a binding without a layout rejected
  as `GfxError::Pipeline`, with wgpu's message naming the binding; empty or misaligned buffers
  rejected at creation) pass on lavapipe, and on WARP in the Windows CI job. Dynamic offsets,
  textures and samplers are left to the milestones that use them, as ADR-0026 says.

### M3.2 — The render crate: camera, meshes, instances

- **Goal.** Draw a list of mesh instances from a camera, with depth, into any render target.
- **Crates.** New `slime-render`; `examples` gains `scene`, the sample of this stage, and its image
  test.
- **Delivers.** `Renderer::new(gpu, format)`, whose depth buffer follows the size of the target it
  draws into; `Mesh` from CPU data (positions, normals, texture coordinates, indices); `Camera`
  (perspective projection, look-at); `Instance` (a mesh, a material of one color, a transform);
  `Renderer::render(gpu, target, &scene)`, which clears the target, draws the instances and
  returns `RenderStats` (draw calls, triangles); per-instance transforms in one uniform buffer
  with dynamic offsets; conventions: right-handed axes with +Y up, as glTF, the camera looking
  down −Z, depth from 0 to 1, linear colors in shaders. Bytes for the GPU are packed with
  `to_le_bytes`. The `scene` example shows cubes under a camera that orbits them;
  `scripts/check.sh` runs it in the headless Weston; the image test of the cubes renders them
  overlapping. ADR: the render crate and glam.
- **Done when.** The image test passes on lavapipe, WARP and the owner's GPU; the sample shows the
  cubes under the moving camera on niri; `scene --frames 120` draws in the headless Weston; the
  owner ran `scene.exe` in the Windows virtual machine.
- **How to test.** `cargo run --example scene`; `cargo test -p slime-examples --test scene`.
- **Checked** on 2026-10-13: the image test of the cubes (`examples/tests/scene.rs`, reference
  `cubes.png`, three cubes of two flat colors, the orange one in front of the blue one) passes on
  lavapipe, which rendered the reference, and on the owner's RX 6900 XT (RADV) with no pixel
  differing at all, flat colors leaving nothing to round; the Windows CI job runs it on WARP. The
  `scene` example drew 300 frames on niri with RADV (3 draw calls and 36 triangles per frame),
  and `scene --frames 120` draws in the headless Weston with lavapipe in the checks. The owner's
  run of `scene.exe` in the Windows virtual machine, which the CI artifact now contains, is
  pending. Dynamic offsets joined `slime-gfx` with the instances, as ADR-0026 planned; the
  material of one color came with this milestone rather than M3.3, since instances need a color
  from the start.

### M3.3 — Directional light and material

- **Goal.** A simple lighting model: one directional light with an ambient term, and materials
  with a base color.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** `DirectionalLight` (direction, color, ambient light) in the scene; normals
  transformed with the instance; Lambert diffuse shading in WGSL on the material's base color, in
  linear colors, which the sRGB target encodes. The image test shows lit cubes; the tolerance of
  ADR-0018 is measured on WARP and RADV with shading in the picture, and amended if it does not
  hold.
- **Done when.** The image test passes on the three drivers; the lit cubes look right by eye.
- **How to test.** As M3.2.

### M3.4 — Textures and samplers

- **Goal.** Textured materials.
- **Crates.** `slime-gfx` (textures, samplers, their bind group entries); `slime-render`;
  `examples`.
- **Delivers.** `Texture` (two-dimensional, 8-bit RGBA, sRGB or linear, uploaded from bytes,
  without mipmaps) and `Sampler` (nearest or linear filtering; repeat, clamp or mirror addressing:
  what glTF samplers express) in `slime-gfx`, bound through bind groups; `Material` gains a base
  color texture and its sampler; the sample generates a checker texture; the image test renders
  the textured cube. Mipmaps are added only if the image tests or the sample show aliasing that
  the tolerance cannot absorb, and the ADR of M3.1 is then amended.
- **Done when.** The image test passes on the three drivers.
- **How to test.** As M3.2.

### M3.5 — A glTF scene and camera control

- **Goal.** The sample renders a glTF scene under a camera the user moves.
- **Crates.** `examples` (the loader, the asset, the sample, its image test).
- **Delivers.** A temporary loader in `examples/common/gltf.rs`, built on the `gltf` crate without
  its default features: GLB files with embedded buffers, PNG images decoded with `png`; meshes,
  materials, textures and samplers created through the renderer; the node hierarchy flattened into
  instances; a clear error for what is not supported (external files, JPEG images, skins,
  animations, extensions). A sample asset in `examples/assets/`, a scene made in Blender under
  CC0 (under 500 kB), embedded in the sample with `include_bytes!`; `--scene <file>` loads another
  GLB file. Camera: automatic orbit around the scene's bounds, orbit with the left button, zoom
  with the wheel. The image test `scene-gltf` renders the asset at fixed camera poses. ADR:
  temporary glTF loading, and its removal in Stage 4.
- **Done when.** The sample shows the asset on niri, through Xwayland and in the Windows virtual
  machine; the image test passes on the three drivers; a Khronos sample model loads with
  `--scene`.
- **How to test.** `cargo run --example scene -- --scene <file.glb>`; the image test.

### M3.6 — Statistics, GPU timings and the dev overlay

- **Goal.** Render statistics shown in an overlay while the scene runs, with the GPU time of the
  frame.
- **Crates.** `slime-gfx` (timestamp queries, the wgpu access feature); `slime-render`
  (statistics); new `slime-devui`; `examples`.
- **Delivers.** `RenderStats` complete: draw calls, triangles, the CPU time of `render`, and the
  GPU time of the pass from timestamp queries when the adapter offers them, read a few frames
  later so that nothing waits. `slime-devui`, a temporary crate: `egui` and `egui-wgpu` without
  default features; the engine's events translated to egui's; the overlay drawn into the frame's
  target after the scene, in a pass that keeps the image; F1 shows and hides it. The sample shows
  the frame time, the GPU time, draw calls, triangles and the adapter's name. ADR: the dev overlay
  and wgpu access (amends 0016), optional GPU features.
- **Done when.** The overlay is visible on niri, in a screenshot of the headless Weston and in the
  Windows virtual machine; the GPU time shows on RADV and on Direct3D 12; whether lavapipe and WARP
  offer timestamps is measured and recorded here; the image tests are unchanged.
- **How to test.** `cargo run --example scene`, then F1.

### M3.7 — Windows check, documentation, release 0.0.3

- **Goal.** 0.0.3 released.
- **Crates.** All; CI.
- **Delivers.** The Windows checklist run by the owner in the virtual machine (`scene.exe`: window,
  resize, camera, overlay, GPU time); `scene.exe` in the CI artifact; the documentation reread by
  a reviewing agent (`ARCHITECTURE.md`, the glossary, `dependencies.md`, the crate READMEs);
  `CHANGELOG.md`; the duration of CI measured; the `v0.0.3` tag on a commit whose CI is green on
  both platforms.
- **Done when.** The checks are green on both platforms; the tag is pushed.
- **How to test.** Push and read the CI run; the Windows artifact in the virtual machine.

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)):

1. In one commit, set the version in the root `Cargo.toml`, rename the `[Unreleased]` section of
   `CHANGELOG.md` to `[<version>] - <date>`, and update this plan.
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.

0.0.1 was released on 2026-10-12, before Windows joined CI: the tag `v0.0.1` is on the commit
"Release 0.0.1". 0.0.2 was released the same way on 2026-10-12: the tag `v0.0.2` is on the
commit "Release 0.0.2".

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |

The details of each completed milestone (what it delivered, how it was checked, what was measured)
are in the history of this file and in the commits that implemented it; the measurements that
justify a decision are also in its ADR.

## Known debt

- **Frames are not synchronized with the display's refresh** (ADR-0021). With Mailbox, the
  default, the schedule alone paces frames: on a 60 Hz display, an image is now and then shown
  twice while the next one is dropped. Aligning frames on the compositor's frame callbacks is to
  reconsider when a renderer draws real scenes.
- **Without Mailbox, presenting can still stall the loop.** Windows that do not offer it, and
  applications that choose `PresentMode::Fifo`, wait for the display: one frame per second for a
  hidden window on niri. In a headless Weston with lavapipe, `acquire` waits for Weston's frame
  callbacks for a time that depends on the machine's load: 120 frames of the `window` example took
  15 s on 2026-10-12 (release build, waits of 130 ms on average), 9 s and 25 s in other
  measurements the same day, against 2 s with Mailbox.
- **A hidden window keeps drawing** at the scheduled rate (2 to 3 % of one CPU core on the
  development machine): the engine cannot tell on Wayland whether its window is visible. winit
  reports occlusion on X11, which the engine ignores: under a compositing manager, Xwayland
  included, the X server reports windows as visible anyway.
- **wgpu's errors panic, in release builds too** (ADR-0016): a use of wgpu that it rejects is
  treated as a bug. Before games ship, logging the error and dropping the frame is to be
  considered. Running out of GPU memory panics as well on the paths where wgpu does not treat it as
  the loss of the device (ADR-0022). The engine avoids the rejections that the environment can
  cause: a window larger than the device's largest image is drawn at that size (`Surface::resize`),
  and a shader or pipeline that the driver rejects is returned as an error.
- **A lost GPU device is not replaced** (ADR-0022): the run stops with an error, and since winit
  allows one event loop per process, carrying on means starting the process again.
- **Captures in RenderDoc were not tried on Windows** (Direct3D 12).
- **Frames pause on Windows while the user moves the window, holds its title bar or a border, or
  opens the system menu**: the system then runs a loop of its own, in which no redraw arrives and
  the engine's loop does not wake, so frames and the check for termination signals pause until it
  ends (ADR-0020; the move was measured in M2.4). Keeping frames going would mean asking for
  redraws from inside each frame and waiting there for the schedule.
- **wgpu-hal logs an error when an adapter cannot present to the window**, even though the engine
  then uses another one: seen with RADV in a headless Weston, where only lavapipe can present.
- **No title text in client-side decorations.** On compositors that do not draw decorations
  (GNOME), the title bar has no text, because winit's text renderer depends on an unmaintained
  crate (ADR-0011).
- **Only `App::init` can return an error** (ADR-0020). An application that hits an error in
  another method logs it and requests an exit.
- **Input not reported yet:** the state of the modifier keys as a whole, input-method text,
  touch, touchpad gestures, gamepads, dropped files.
- **The interpolation factor is not passed to applications.** `FixedStep::alpha` computes it, but
  `run` keeps its `FixedStep` private, so an application cannot interpolate between fixed steps
  yet. To expose when a renderer needs it.
- **A compositor that stops answering blocks the engine.** Before the first frame, winit waits for
  the window's first configuration without a timeout, so a termination signal is noticed only when
  a second one ends the process; after `shutdown`, releasing the window or the surface can block
  too (seen with a Weston stopped by SIGSTOP).
- **On Direct3D 12, a covered window or a locked session was not measured.** In `acquire`, wgpu
  waits on the swap chain's frame-latency object, up to one second, if the compositor stops taking
  images; whether Windows does so for such windows is unknown.
- **Client-side decorations run `dbus-send` twice when the window is created** (winit's
  sctk-adwaita, on compositors that do not draw decorations), each waiting up to 100 ms when the
  settings portal does not answer; the error they log when no session bus or portal answers is
  hidden by the default log filter (ADR-0019).

## Notes

- CI build cache: not added yet. With wgpu, the Linux job takes about 2.5 minutes and the Windows
  job about 5 (measured on the runs of 2026-10-12; it compiles the dependencies three times: for
  clippy, for the tests, and in release for the artifact), which is acceptable; a cache is to add if
  it grows much further. The dependencies of this stage (glam, gltf, egui) are expected to add to
  it: each milestone that adds one records the new duration here. With glam and the render crate
  (M3.2, 2026-10-13): Linux 2 minutes 49 seconds, Windows 4 minutes 59 seconds.
