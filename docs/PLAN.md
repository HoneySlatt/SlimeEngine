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

- Stage 3b — Physically based renderer → 0.1: in progress. Planned on 2026-10-13 and validated by
  the owner. Next: M3b.1.

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M3b.1](#m3b1--gpu-layer-for-multi-pass-rendering) | GPU layer for multi-pass rendering | In progress |
| [M3b.2](#m3b2--hdr-rendering-and-tone-mapping) | HDR rendering and tone mapping | Planned |
| [M3b.3](#m3b3--physically-based-materials) | Physically based materials | Planned |
| [M3b.4](#m3b4--light-types) | Light types | Planned |
| [M3b.5](#m3b5--shadows) | Shadows | Planned |
| [M3b.6](#m3b6--image-based-lighting) | Image-based lighting | Planned |
| [M3b.7](#m3b7--bloom) | Bloom | Planned |
| [M3b.8](#m3b8--render-graph) | Render graph | Planned |
| [M3b.9](#m3b9--windows-check-documentation-release-01) | Windows check, documentation, release 0.1 | Planned |

Stages 0, 1, 2 and 3a are done; see [Completed stages](#completed-stages).

## Stage 3b — Physically based renderer → 0.1

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-3b--physically-based-renderer--01)): a sample
scene with a camera, lights, physically based materials and shadows, covered by image tests.

The renderer of Stage 3a becomes physically based (PBR): surfaces say how much light they reflect
and how, with glTF's metallic-roughness materials; lights have physical units; the scene is drawn
in high dynamic range (HDR), with colors above 1, then mapped to the display by a tone-mapping
pass; a shadow map gives the main light its shadows; image-based lighting (IBL) lights surfaces
with their surroundings; and a minimal render graph, extracted from these passes at the end of the
stage, orders the passes of a frame.

Out of scope, left to the stages that need them: blended transparency (glTF's `BLEND` alpha mode;
`MASK` is in); cascaded shadow maps, and shadows of point and spot lights; environments read from
HDR image files (a decoder or a new dependency, and an asset far above the sample's budget: the sky
is procedural); IBL precomputed on the GPU, compute passes and storage buffers; screen-space
effects (ambient occlusion, reflections) and temporal anti-aliasing; multisampling and FXAA, unless
the image tests or the sample show aliasing that the tolerance cannot absorb, the rule that
admitted mipmaps in Stage 3a; tangents computed with MikkTSpace, as glTF prescribes for files
without tangents (the loader computes simpler ones; the importer of Stage 4 will do it right);
animation and skinning; glTF extensions other than `KHR_lights_punctual` and
`KHR_materials_emissive_strength`; and, as in Stage 3a, a scene model (Stage 5) and the asset
pipeline (Stage 4): the renderer still draws flat lists of instances, and the glTF loader stays
temporary.

Decisions taken when this stage was planned, each recorded as an ADR in the milestone that applies
it:

- HDR first. The scene is drawn into a target of 16-bit floating-point colors (`Rgba16Float`),
  then a fullscreen pass multiplies it by an exposure, maps it to the display's range with a
  tone-mapping operator, and writes the sRGB image of the window. It comes first because every
  later image test depends on this output transform: their references are rendered once against
  it.
- Materials. glTF's metallic-roughness model, with the formulas of the Khronos glTF sample viewer
  (the GGX distribution, Smith's visibility term, Schlick's Fresnel approximation): base color,
  metallic-roughness, normal, occlusion and emissive textures, each created in the format of its
  slot (sRGB for colors, linear for data); the `MASK` alpha mode by discarding fragments;
  double-sided materials drawn without culling.
- Lights. Directional, point and spot lights in glTF's units (lux for directional lights, candela
  for the others), read from the `KHR_lights_punctual` extension through the `gltf` crate's
  feature of that name, a feature of an admitted dependency recorded in `dependencies.md`; a fixed
  maximum number per scene, in a uniform buffer; storage buffers only if a measurement asks for
  them.
- Shadows. One shadow map, for the first directional light: an orthographic projection fitted to
  the scene's bounds, a depth bias, and percentage-closer filtering (PCF) over 3 × 3 samples
  through a comparison sampler.
- IBL. A procedural sky drawn into an environment cube map, its irradiance, its prefiltered
  specular levels and the table of the BRDF's scale and bias, all computed on the CPU at startup:
  no compute pass and no image file. The asset pipeline of Stage 4 may cook them instead.
- Post-processing. The tone-mapping pass with its exposure, and a bloom: a chain of downsampled HDR
  targets, upsampled and added in the tone-mapping pass, so that no blending is needed.
- Render graph. Extracted at the end of the stage from the passes that exist then (shadow, scene,
  sky, bloom, tone mapping), as the roadmap asks: passes declare the targets they read and write,
  the graph owns the targets that live for one frame and orders the passes; no memory aliasing, no
  asynchronous compute.
- GPU layer. `slime-gfx` gains what the passes need, when they need it, without wgpu outside it
  (ADR-0016): float render targets, targets and depth targets that shaders sample, pipelines
  without vertex buffers, passes without color targets, comparison samplers and depth bias in
  M3b.1, for the HDR and shadow passes; cube textures and mip levels given by the caller in M3b.6,
  with IBL, their first user (amending ADR-0032).
- Loader and asset. The temporary glTF loader reads the PBR textures, tangents (computed per
  triangle when the file has none) and lights; `make_scene.py` draws the new textures in code and
  exports tangents and lights; the asset stays under CC0 and under 500 kB.
- No new dependency. Half-precision floats are converted by hand, as bytes are packed by hand
  (ADR-0026); the procedural sky needs no image decoder.
- Windows. The Windows CI job runs the tests and the headless samples at every milestone; the owner
  checks windowed behaviour in the virtual machine at M3b.2, M3b.5 and M3b.9.

Risks, to measure as the stage goes: shadows flip whole pixels where drivers rasterize an edge
differently, which the per-channel tolerance of the image tests cannot absorb (ADR-0028), so the
tolerance may need a measured amendment at M3b.5; every image test renders more passes on lavapipe
and WARP, which lengthens CI; the CPU precomputation of IBL runs in every process that draws the
sample, tests included, and must stay small; upgrading wgpu still waits for egui.

### M3b.1 — GPU layer for multi-pass rendering

- **Goal.** What the passes of the stage need from `slime-gfx` to draw into targets that later
  passes read, proven offscreen on lavapipe and WARP.
- **Crates.** `slime-gfx`; the callers of what it changes (`slime-render`, `slime-devui`, the
  examples and the tests).
- **Delivers.** The `Rgba16Float` format for render targets, read back to the CPU as floating-point
  values; render targets and depth targets that shaders sample through bind groups once a pass has
  drawn into them, while a window's image cannot be sampled; pipelines without a vertex buffer, for
  a fullscreen triangle built from the vertex index; passes and pipelines without a color target,
  which write depth only; comparison samplers, and a depth bias on pipelines; several passes timed
  in one frame, each time read back on its own. ADR: the GPU layer for multi-pass rendering
  (amends 0017 and 0026).
- **Done when.** `slime-gfx` tests pass on lavapipe and on WARP: a float target keeps values above
  1 and below 0 and reads them back as half-precision floats represent them; a fullscreen triangle
  drawn without a vertex buffer covers every pixel; a second pass samples the target a first pass
  drew into; a depth-only pass writes depths that a later pass reads through a comparison sampler,
  which gives 0 on one side of the reference depth and 1 on the other; a depth bias moves a
  surface's depth by the expected amount; three passes timed in one frame each get a time.
- **How to test.** `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test -p slime-gfx`.

### M3b.2 — HDR rendering and tone mapping

- **Goal.** The scene is drawn in high dynamic range and mapped to the display by a tone-mapping
  pass with an exposure.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** The renderer draws the scene into an `Rgba16Float` target with its depth target,
  both following the output's size, then a fullscreen pass samples it, multiplies it by the
  exposure, applies the tone-mapping operator chosen in the ADR, and writes the output target (the
  window's image, or an offscreen sRGB one); `Scene` gains the exposure; `RenderStats` gives the
  GPU time of each pass; the `scene` example shows them in the overlay and lets the user change the
  exposure; the references of the image tests are rendered again on lavapipe and looked at; the
  lighting and material tests of `slime-render` check their values through the tone-mapping
  curve. ADR: HDR rendering and tone mapping (amends 0027: several passes, the scene in HDR).
- **Done when.** The image tests pass on the three drivers; the sample looks right by eye on niri
  and in a screenshot of the headless Weston; the owner ran `scene.exe` in the Windows virtual
  machine.
- **How to test.** `cargo run --example scene`; `cargo test -p slime-examples`.

### M3b.3 — Physically based materials

- **Goal.** Surfaces shaded with glTF's metallic-roughness model.
- **Crates.** `slime-render`; `examples` (the loader, the asset and its Blender script, the image
  tests).
- **Delivers.** `MaterialData` gains the metallic and roughness factors and their texture, a normal
  texture and its scale, an occlusion texture and its strength, the emissive factor, strength and
  texture, the alpha mode (opaque, or masked with a cutoff) and double-sidedness; `MeshData` gains
  tangents; the shader evaluates the BRDF for the directional light, with the camera's position in
  the frame's values; default textures stand in for the slots a material leaves empty; the loader
  reads all of it, creates each texture in its slot's format, and reads tangents or computes them;
  `make_scene.py` draws the new textures and exports tangents; the shapes of the image test gain
  spheres of increasing metallic and roughness. ADR: physically based materials (amends 0027 and
  0029).
- **Done when.** Tests that read pixels back: a white dielectric lit at normal incidence gives the
  value the BRDF predicts, computed in the test; a rougher surface spreads its highlight; a metal
  reflects in its base color; a normal map tilts the shading as a tilted surface would; a masked
  material discards fragments below its cutoff. The image tests pass on the three drivers; the
  sample looks right by eye.
- **How to test.** As M3b.2.

### M3b.4 — Light types

- **Goal.** Several directional, point and spot lights per scene, in physical units.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** `Scene` holds a list of lights: directional (illuminance in lux), point (intensity
  in candela, range) and spot (intensity, range, inner and outer cone angles), up to a fixed
  maximum, in a uniform buffer; the falloff that `KHR_lights_punctual` recommends (the inverse
  square of the distance, brought smoothly to zero at the range) and its cone attenuation; the
  constant ambient light stays until IBL replaces it; the loader reads `KHR_lights_punctual`
  through the `gltf` feature of that name (`dependencies.md` and ADR-0029 updated); the asset
  exports its sun and a point or spot light; the sample's exposure suits physical units. ADR:
  lights and their units.
- **Done when.** Tests that read pixels back: a point light gives a quarter of the light at twice
  the distance, and none beyond its range; a spot light lights fully inside its inner cone, not at
  all outside its outer cone, and in between smoothly; a scene with the maximum number of lights
  draws, and one with more is refused as documented. The image tests pass on the three drivers;
  the sample looks right by eye.
- **How to test.** As M3b.2.

### M3b.5 — Shadows

- **Goal.** The first directional light casts shadows.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** A depth-only shadow pass into a sampleable depth target, from an orthographic
  projection fitted to the bounds of the instances; the scene pass reads it with 3 × 3 PCF through
  a comparison sampler, with a constant and a slope-scaled depth bias; `RenderStats` counts the
  shadow pass's draw calls and gives its GPU time. ADR: shadow mapping.
- **Done when.** Tests that read pixels back: a square above a floor darkens a square of the floor
  of the expected size, with edges blended over the width of the filter; a lit floor shows no
  self-shadowing (shadow acne). The image tests pass on the three drivers, with a tolerance amended
  by a measured ADR if shadow edges require it; the sample looks right by eye; the owner ran
  `scene.exe` in the Windows virtual machine.
- **How to test.** As M3b.2.

### M3b.6 — Image-based lighting

- **Goal.** Surfaces lit by their surroundings: a procedural sky and its reflections.
- **Crates.** `slime-gfx` (cube textures, mip levels given by the caller, `Rgba16Float` textures
  uploaded from the CPU); `slime-render`; `examples`.
- **Delivers.** `slime-gfx`: cube textures, and textures whose mip levels the caller gives, in
  `Rgba16Float` among other formats, with half-precision floats converted by hand (amends
  ADR-0032). `slime-render`: a procedural sky (a gradient and a sun) drawn on the CPU into a small
  cube map, its irradiance and its prefiltered specular levels, and the table of the BRDF's scale
  and bias, computed at startup; a sky pass draws the environment behind the scene instead of the
  clear color; IBL replaces the constant ambient light. ADR: image-based lighting computed on the
  CPU.
- **Done when.** Tests that read pixels back: a white rough sphere inside a uniform environment
  reflects that environment's radiance (the furnace test); a mirror sphere reflects the sky; the
  CPU precomputation of the sample stays under a budget measured and recorded here. The image
  tests pass on the three drivers; the sample looks right by eye.
- **How to test.** As M3b.2.

### M3b.7 — Bloom

- **Goal.** Bright parts of the image glow.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** A chain of HDR targets, each half the size of the one before, filled by
  downsampling the scene, then upsampled back and added to the scene in the tone-mapping pass, with
  a strength; the sample's asset gains an emissive object bright enough to bloom; the overlay shows
  the chain's GPU time. ADR: bloom.
- **Done when.** Tests that read pixels back: a single bright pixel spreads into a symmetric halo,
  and a dark image is unchanged. The image tests pass on the three drivers; the sample looks right
  by eye.
- **How to test.** As M3b.2.

### M3b.8 — Render graph

- **Goal.** The passes of a frame are declared and ordered by a minimal render graph, extracted
  from the passes that exist.
- **Crates.** `slime-render`, unless the ADR shows that the graph belongs in `slime-gfx`.
- **Delivers.** Passes declare the targets they read and write; the graph creates the targets that
  live for one frame at the output's size or a fraction of it, orders the passes by what they read
  and write, records them, and reports each pass's statistics; the renderer's frame (shadow,
  scene, sky, bloom, tone mapping) is built as a graph. ADR: the render graph.
- **Done when.** Tests of the graph without a GPU: passes are ordered by their dependencies, a read
  of a target that no pass writes and a cycle are refused; the image tests give the same pixels on
  lavapipe as before the extraction; the CPU time of a frame is not worse, measured before and
  after.
- **How to test.** `cargo test -p slime-render`; the image tests.

### M3b.9 — Windows check, documentation, release 0.1

- **Goal.** 0.1 released.
- **Crates.** All; CI.
- **Delivers.** The Windows checklist run by the owner in the virtual machine; the documentation
  reread by reviewing agents; `CHANGELOG.md`; the duration of CI measured; the `v0.1.0` tag on a
  commit whose CI is green on both platforms.
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
commit "Release 0.0.2", and 0.0.3 on 2026-10-13: the tag `v0.0.3` is on the commit
"Release 0.0.3".

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3a — First renderer → 0.0.3 | M3.1 GPU resources for a renderer; M3.2 the render crate; M3.3 directional light and material; M3.4 textures and samplers; M3.5 a glTF scene and camera control; M3.6 statistics, GPU timings and the dev overlay; M3.7 Windows check, documentation, release | Done; released as 0.0.3 (tag `v0.0.3`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |

The details of each completed milestone (what it delivered, how it was checked, what was measured)
are in the history of this file and in the commits that implemented it; the measurements that
justify a decision are also in its ADR.

## Known debt

Limitations known and accepted for now. The ones that a contributor could take on alone are also
GitHub issues, linked from their entry; this list stays the reference.

- **Frames are not synchronized with the display's refresh** (ADR-0021; issue
  [#1](https://github.com/HoneySlatt/SlimeEngine/issues/1)). With Mailbox, the default, the
  schedule alone paces frames: on a 60 Hz display, an image is now and then shown twice while the
  next one is dropped. Aligning frames on the compositor's frame callbacks is to reconsider when a
  renderer draws real scenes.
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
- **A lost GPU device is not replaced** (ADR-0022; issue
  [#2](https://github.com/HoneySlatt/SlimeEngine/issues/2)): the run stops with an error, and
  since winit allows one event loop per process, carrying on means starting the process again.
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
- **Input not reported yet** (issue [#3](https://github.com/HoneySlatt/SlimeEngine/issues/3)):
  the state of the modifier keys as a whole, input-method text, touch, touchpad gestures,
  gamepads, dropped files. The dev overlay feels it too: egui gets no modifiers, so shortcuts with
  Ctrl or Shift, and selecting text with Shift, do not work in it.
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
- **WARP's shader compiler crashes the process, now and then, when several threads compile
  shaders at once**, even on separate devices: an access violation (`STATUS_ACCESS_VIOLATION`)
  that other projects trace to `d3d10warp.dll`, and that no test can catch. Seen once in the
  Windows CI job, on 2026-10-13, in the texture tests of
  `slime-gfx`, after about ten green runs; other projects that test on WARP report the same crash,
  and that running the tests one at a time avoids it, which the Windows job does since. The
  engine's own code is not involved.
- **Client-side decorations run `dbus-send` twice when the window is created** (winit's
  sctk-adwaita, on compositors that do not draw decorations), each waiting up to 100 ms when the
  settings portal does not answer; the error they log when no session bus or portal answers is
  hidden by the default log filter (ADR-0019).
- **Upgrading wgpu waits for egui** while `slime-devui` exists: `egui-wgpu` is built against one
  wgpu version (egui 0.36 against wgpu 30 today), so wgpu moves only once egui follows
  (ADR-0031). Ends when the overlay is deleted.
- **The dev overlay blends in an sRGB framebuffer.** egui prefers a linear one for its blending,
  warns once at startup, and its panels look slightly lighter than egui's own demos (ADR-0031).
  Harmless for tooling.
- **Mipmaps are computed on the CPU when a texture is created** (`Gpu::create_texture`), on the
  calling thread, in work proportional to the texture's pixels, and a caller cannot supply levels
  of its own (ADR-0032). To move to the GPU, or to the asset pipeline of Stage 4, if textures
  grow.

## Notes

- CI build cache: not added yet. With wgpu, the Linux job takes about 2.5 minutes and the Windows
  job about 5 (measured on the runs of 2026-10-12; it compiles the dependencies three times: for
  clippy, for the tests, and in release for the artifact), which is acceptable; a cache is to add if
  it grows much further. The dependencies of Stage 3a (glam, gltf, egui) were expected to add to
  it, and each milestone that added one recorded the new duration here. With glam and the render
  crate (M3.2, 2026-10-13): Linux 2 minutes 49 seconds, Windows 4 minutes 59 seconds. With gltf and
  egui (M3.6, 2026-10-13): Linux 3 minutes 22 seconds, Windows 5 minutes 42 seconds, and 6 minutes
  2 seconds once its tests ran one at a time. At the release (M3.7, 2026-10-13), over the ten
  green runs before it: Linux from 2 minutes 35 seconds to 3 minutes 38 seconds, Windows from
  3 minutes 12 seconds to 7 minutes 10 seconds; the Windows runner's speed varies from one run to
  the next more than the code changes between them.
