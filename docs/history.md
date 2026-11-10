# History

This file holds the finished stages of the roadmap, newest first: what each delivered, its release
and its decisions. [`PLAN.md`](PLAN.md) holds the stage in progress, its whole plan in
[`stage-6d.md`](stage-6d.md).

- What each release changed for users: [`CHANGELOG.md`](../CHANGELOG.md).
- A stage's milestones in full, how each was checked and what it measured: the plan as it stood at
  the release, `git show v<version>:docs/PLAN.md`, and the commits that made them.
- The measurements that justify a decision: its ADR, and [`measurements.md`](measurements.md).

## Stage 6b — The engine's interface → 0.6

- Released as 0.6 on 2026-10-27, tag
  [`v0.6.0`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.6.0), with the Windows
  programs that CI built on its GitHub release.
- Decisions: [ADR-0131](adr/0131-stage-6b-and-stage-6c.md) to
  [ADR-0154](adr/0154-floating-panels-and-held-keys.md).
- Delivered: the engine's UI framework, `slime-ui`, with text in several writing systems, a tree of
  elements styled by themes read from files, input, focus and layers, text edited through input
  methods and the clipboard, widgets, menus, a palette of commands and notifications, read and used
  by agents through the MCP server and by screen readers. The dev overlay runs on it, and egui left
  the engine. The editor's core changes a level through commands, undone, redone and journaled,
  driven by an agent through the editor's program, without a window.
- Milestones: M6b.1a the commands and undo; M6b.1b the editor driven by an agent; M6b.2 text in
  several writing systems; M6b.3a the canvas; M6b.3b text drawn, and the display list; M6b.4a
  elements and layout; M6b.4b style sheets and themes; M6b.5a input and focus; M6b.5b what agents
  see; M6b.6a text edited, and what the platform lacks for it; M6b.6b screen readers; M6b.7a the
  widgets of a panel; M6b.7b the look's foundations; M6b.7c layers; M6b.7d rich fields; M6b.7e what
  the editor tells and finds; M6b.8a the editors and the world panel on the engine's interface;
  M6b.8b the overlay over the games' views; M6b.9 Windows check, documentation, release 0.6.
- The owner chose that the interface blends in sRGB's encoding (M6b.3a), and Geist as the font of
  the editor's text (M6b.3b).
- Checked by the owner: each milestone that changed what shows, on Linux and for several in the
  Windows virtual machine, Orca and Narrator reading the examples; the whole in the Windows virtual
  machine at M6b.9, which passed.

## Stage 6a — What the editor stands on → 0.5

- Released as 0.5 on 2026-10-22, with the audit pass and the foundations pass, tag
  [`v0.5.0`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.5.0), with the Windows
  programs that CI built on its GitHub release.
- Decisions: [ADR-0116](adr/0116-stage-6-in-two-halves.md) to [ADR-0130](adr/0130-picking.md);
  measurements in [`measurements.md`](measurements.md), under the milestones that measured.
- Delivered: frustum culling and instancing; one task pool, and copies to the GPU within a budget;
  what the world knows of its changes; a frame that owns what it draws; game time, and a game as a
  simulation and a presentation; the development channel and the MCP server; saves per scene;
  views apart from the renderer, with their settings; asset requests; drawing and picking for the
  editor.
- Its save fixture is `games/hunnawood/fixtures/save.toml`, in the layout of saves per scene
  ([ADR-0125](adr/0125-saves-per-scene.md)).
- Milestones: M6a.1 frustum culling and instancing; M6a.2 the engine's task pool, and copies to the
  GPU within a budget; M6a.3 what the world knows of its changes; M6a.4 a frame that owns what it
  draws; M6a.5 the phases of a frame, game time, and a game as a simulation and a presentation;
  M6a.6 the development channel and the MCP server; M6a.7a driven components, keys and created
  entities written into levels; M6a.7b saves per scene and adoption; M6a.8a views apart from the
  renderer; M6a.8b the settings of a view; M6a.9 asset requests; M6a.10a drawing for the editor;
  M6a.10b picking; M6a.11 Windows check, documentation, release 0.5.
- Checked by the owner: each milestone that changed what shows, on Linux; the whole in the Windows
  virtual machine at M6a.11, which found four defects, fixed before the release.

## The audit pass and the foundations pass

- Done on 2026-10-20, released with 0.5.
- The engine was audited at 0.4, without changing it, for correctness, measured performance and
  architecture against the games it aims at.
  [ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) sums up what the audit
  found and records the owner's decisions; [ADR-0096](adr/0096-the-files-the-engine-reads.md) is the
  threat model that ranks its defects.
- P1 to P5 fixed them ([ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md) to
  [ADR-0106](adr/0106-the-measured-quick-wins.md)), but B4-F01, which M6a.2 fixed. P6 decided what
  the editor freezes ([ADR-0107](adr/0107-the-execution-model.md) to
  [ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)).
- Milestones: P1 saves and identities that never mislead; P2 a damaged file gives an error, never a
  crash; P3 the loss of the GPU device and the main loop; P5 the build, CI, and measurements
  tracked from one commit to the next; P4 the measured quick wins; P6 the foundation decisions.
- Each finding was described in full in the plan until 0.5 (`git show v0.5.0:docs/PLAN.md`); the
  audit's report and its tests are kept outside the repository. What neither pass fixed is in
  [`debt.md`](debt.md#left-by-the-audit-of-04).
- Checked by the owner: P3 and P5, in the Windows virtual machine on 2026-10-27, passed.

## Stage 5b — Scenes and prefabs → 0.4

- Released as 0.4 on 2026-10-18, tag
  [`v0.4.0`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.4.0), with the Windows
  programs that CI built on its GitHub release.
- Decisions: [ADR-0089](adr/0089-scenes-and-prefabs.md) to
  [ADR-0095](adr/0095-cooked-scene-files.md); the measurements of spawning scenes are in ADR-0091.
- Delivered: scene files that place prefabs with overrides, worlds written back as scene files, scene
  files cooked into a binary encoding of the engine's own, typed components on the nodes of glTF
  scenes; *Hunna Wood*'s zone made of prefabs that place its land and six lanterns.
- Its save fixture, `games/hunnawood/fixtures/save.toml`, was made again at M6a.7b in the layout of
  saves per scene, which refuses those of 0.4 ([ADR-0125](adr/0125-saves-per-scene.md)).
- Milestones: M5b.1 scenes placed in a world; M5b.2 a world written back as a scene; M5b.3 saves and
  hot reload with placements; M5b.4 the binary encoding of documents; M5b.5 scene files cooked,
  typed components, prefabs in the `world` example; M5b.6 *Hunna Wood*'s zone made of prefabs; M5b.7
  Windows check, documentation, release 0.4.

## Stage 5a — The world → 0.3

- Released as 0.3 on 2026-10-17, with Stage 4b, tag
  [`v0.3.0`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.3.0), with the Windows
  programs that CI built, `world.exe` among them, on its GitHub release.
- Decisions: [ADR-0078](adr/0078-stage-5-in-two-halves.md) to
  [ADR-0088](adr/0088-saving-and-resuming-a-run.md); the measurements of the world's storage,
  transforms and frame are in ADR-0082 to ADR-0084.
- Delivered: the engine's world, its entities, components and hierarchy, positions in double
  precision, drawn; reflection, through which any component is listed and edited; serialization
  with format versioning; a run saved and resumed; *Hunna Wood* on the world.
- Its save fixture, now `games/hunnawood/fixtures/save-0.3.toml`, is set aside by later versions as
  a save of another layout ([ADR-0090](adr/0090-stage-5b-scenes-and-prefabs.md)).
- Milestones: M5a.1 the world: entities and components; M5a.2 the hierarchy and the transforms;
  M5a.3 what the world draws; M5a.4 *Hunna Wood* on the world; M5a.5 reflection; M5a.6 any component
  listed and edited; M5a.7 serialization with format versioning; M5a.8 the world round-trips; M5a.9
  saving and resuming a run; M5a.10 Windows check, documentation, release 0.3.

## Stage 4b — Live assets

- Done on 2026-10-16; no release of its own: what it delivers is released with 0.3.
- Decisions: [ADR-0072](adr/0072-stage-4b-live-assets.md) to
  [ADR-0077](adr/0077-hot-reload-of-shaders.md).
- Delivered: scenes loaded in the background while frames go on, recovery from a lost GPU device,
  materials that change in place, hot reload of cooked data and of shaders.
- Milestones: M4b.1 a shared GPU and a task pool; M4b.2 the zone loads while frames go on; M4b.3
  recovering from a lost GPU device; M4b.4 *Hunna Wood* goes on after a lost device; M4b.5 materials
  that change in place; M4b.6 hot reload of cooked data; M4b.7 hot reload of shaders; M4b.8 refining
  *Hunna Wood*; M4b.9 Windows check, documentation, close.
- Checked by the owner at its close: the recovery from a lost device, in the Windows virtual machine
  from the dev overlay and by a forced TDR, and in a KDE Plasma session after a real reset of the
  GPU (see [Known debt](debt.md)).

## Stage 4a — Cooked assets → 0.2

- Released as 0.2 on 2026-10-15, tag
  [`v0.2.0`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.2.0), with the Windows
  programs that CI built on its GitHub release, *Hunna Wood* with its zone cooked and the notice
  that credits its assets among them.
- Decisions: [ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md) to
  [ADR-0071](adr/0071-cooked-textures.md).
- Delivered: asset IDs that survive a move; the cooker, which imports glTF scenes and images into
  meshes, materials, scenes and compressed textures; *Hunna Wood*, which replaces *Nightwatch*, its
  first zone played by day and by night.
- The zone's budgets are set by the test that checks them, in `games/hunnawood/src/zone.rs`; what
  was left to refine went to M4b.8.
- Milestones: M4a.1 *Hunna Wood*'s skeleton; M4a.2 asset identity; M4a.3 images; M4a.4 glTF; M4a.5
  the sample scene through the pipeline; M4a.6 the zone's sources; M4a.7 *Hunna Wood* by day; M4a.8
  night; M4a.9 Windows check, documentation, release 0.2.

## Stage 3d — Anti-aliasing and the shadows of spot lights

- No release of its own: what it delivers is released with 0.2.
- Decisions: [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md), and ADR-0058 to
  [ADR-0065](adr/0065-reading-the-cascades.md). The cost of its scene pass on Direct3D 12 with each
  shader compiler made DXC ship beside the Windows programs
  ([ADR-0066](adr/0066-shipping-dxc-beside-the-programs.md)).
- Delivered: multisample anti-aliasing, the shadows of every spot light in one atlas, a smoother
  shadow filter, and the sun's shadows in cascades.
- Milestones: M3d.1 multisampling in the GPU layer; M3d.2 anti-aliasing in the renderer; M3d.3 spot
  lights' shadows: what each tile sees; M3d.4 spot lights' shadows: drawn and read; M3d.5
  *Nightwatch* and the Windows check; M3d.6 a smoother shadow filter; M3d.7 what each cascade sees;
  M3d.8 the cascades read; M3d.9 *Nightwatch* at 3840 × 2160.

## Stage 3c — A first game

- No release of its own: its game, *Nightwatch*, was never released, as *Hunna Wood* replaced it in
  Stage 4a ([ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)).
- Decisions: games live in the workspace ([ADR-0055](adr/0055-games-in-the-workspace.md)); the stage
  that each of its findings goes to is in
  [ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md), and the two that became Stage
  3d in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md).
- Delivered: a small game made with the public API only, and what it found hard, as proposals for
  the next stages.
- Milestones: M3c.1 skeleton; M3c.2 guards, detection and the objective; M3c.3 night, feel and the
  Windows check; M3c.4 findings for the next stages.

## Stage 3b — Physically based renderer → 0.1

- Released as 0.1 on 2026-10-14, tag
  [`v0.1.0`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.1.0), the first GitHub
  release, with the Windows examples that CI built from its commit.
- Decisions: [ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md) to
  [ADR-0054](adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md).
- Delivered: physically based materials, light types in physical units, high-dynamic-range
  rendering and tone mapping, shadows, image-based lighting, bloom and a render graph, then a pass
  over the known debt.
- Milestones: M3b.1 GPU layer for multi-pass rendering; M3b.2 HDR rendering and tone mapping; M3b.3
  physically based materials; M3b.4 light types; M3b.5 shadows; M3b.6 image-based lighting; M3b.7
  bloom; M3b.8 render graph; a pass over the known debt; M3b.9 Windows check, documentation,
  release.

## Stage 3a — First renderer → 0.0.3

- Released as 0.0.3 on 2026-10-13, tag
  [`v0.0.3`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.0.3), on the commit "Release
  0.0.3".
- Decisions: [ADR-0023](adr/0023-dependency-checks-for-the-supported-targets.md) to
  [ADR-0032](adr/0032-mipmaps-computed-at-texture-creation.md).
- Delivered: the first renderer, which draws a glTF scene with textured materials, a depth test and a
  directional light under a camera the mouse moves, its costs shown in a dev overlay, covered by
  image tests.
- Milestones: M3.1 GPU resources for a renderer; M3.2 the render crate; M3.3 directional light and
  material; M3.4 textures and samplers; M3.5 a glTF scene and camera control; M3.6 statistics, GPU
  timings and the dev overlay; M3.7 Windows check, documentation, release.

## Stage 2 — GPU layer → 0.0.2

- Released as 0.0.2 on 2026-10-12, tag
  [`v0.0.2`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.0.2), on the commit "Release
  0.0.2".
- Decisions: [ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md) to
  [ADR-0022](adr/0022-losing-the-gpu-device.md).
- Delivered: the GPU layer, which draws a triangle in the window on Linux (Vulkan) and Windows
  (Direct3D 12) and offscreen images that CI compares with a reference; frame pacing; GPU debug
  markers; a clean stop when the device is lost. Windows joined CI.
- Milestones: M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and
  reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers,
  robustness, release.

## Stage 1 — Core and platform → 0.0.1

- Released as 0.0.1 on 2026-10-12, before Windows joined CI, tag
  [`v0.0.1`](https://github.com/HoneySlatt/SlimeEngine/releases/tag/v0.0.1), on the commit "Release
  0.0.1".
- Decisions: [ADR-0008](adr/0008-errors-logging-and-assertions.md) to
  [ADR-0013](adr/0013-windowed-smoke-test-in-weston.md).
- Delivered: logging and diagnostics, frame timing, the lifecycle and a headless mode, a window and
  its input on Wayland and X11, a clean shutdown.
- Milestones: M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode;
  M1.4 window; M1.5 input; M1.6 clean shutdown.

## Stage 0 — Foundations

- Done when CI went green on the first push to GitHub, on 2026-10-12; no release.
- Decisions: [ADR-0001](adr/0001-record-architecture-decisions.md) to
  [ADR-0007](adr/0007-documentation-tooling-and-ci.md).
- Delivered: the repository and its ADR process, the Nix environment, the workspace with its lints
  and crate skeletons, the dependency policy and CI.
- Milestones: M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate
  skeletons; M0.4 dependency policy and CI.

## CI and its limit of 20 minutes

How long CI took as the engine grew, and what was done about it. The limit itself and its current
figures are in [`debt.md`](debt.md), "CI's jobs and their limit of 20 minutes".

- Until 0.1, CI had no build cache: the Linux job grew from about 2.5 minutes to 5 minutes 40
  seconds, and the Windows job from about 5 minutes to 11 minutes 32 seconds, as wgpu, glam, gltf
  and egui came. The Windows runner's speed varied from one run to the next more than the code did.
- At 0.1, the owner decided to keep CI as it is, the repository being public and its minutes free,
  and to reconsider past 20 minutes.
- Stage 5a's crates took the Linux job to 13 minutes 26 seconds and the Windows job to 18 minutes
  27 seconds; at M5b.6, the Windows job took 19 minutes 48 seconds.
- At P5, the Windows job ranged from 13 minutes 38 seconds to 21 minutes 54 seconds: it was split
  into `windows` and `windows-release`, side by side, and every job kept the cooked zone in its
  cache (ADR-0081, [ADR-0104](adr/0104-the-build-ci-and-measurements.md)).
- [ADR-0123](adr/0123-the-linux-checks-in-two-jobs.md) split the Linux checks into two jobs.
- Until [ADR-0144](adr/0144-the-cooked-zones-key-in-ci.md), the cooked zone's cache was keyed by the
  whole `Cargo.lock` and root `Cargo.toml`: a milestone that added any dependency cooked the zone
  again in each job, which took a job past its limit at M6b.2, M6b.3a, M6b.3b, M6b.4b and M6b.5b.
  ADR-0144 keys it on the packages that the cooker is built from.
- At M6b.7b, a change of `slime-color` cooked the zone again: `check (windowed)` took 21 minutes 55
  seconds and `windows` 21 minutes 39 seconds.
- At M6b.8b, the game's agent session made `check (windowed)` take 23 minutes 31 seconds: the owner
  decided to split the windowed part again ([ADR-0155](adr/0155-the-editors-design.md)).
- After 0.6, two runs failed, the game's session past its own limit of 15 minutes.
- M6c.0 split the checks into `headless`, `windowed` and `game`, and drew the zone without shadows
  in the game's session, where 426 s became 21 s on four cores. On `a199369`, the three Linux jobs
  took 13 minutes 35 seconds, 4 minutes 42 seconds and 4 minutes 14 seconds. `windows`, which built
  its image and timing tests again to print their figures, now prints them from its one run of the
  tests: it went from 19 minutes 43 seconds on `a199369` to 11 minutes 20 seconds on `bb4552b`.
- From C1, `check (headless)` and `windows` went past their limit again, the runners slower: 22
  minutes 56 seconds and 23 minutes 25 seconds on `3a8e42e`, at M6d.1d2b2. The owner chose to split
  them again rather than cache the compiled dependencies
  ([ADR-0228](adr/0228-ci-split-again-each-job-within-20-minutes.md)): a `lint` part beside
  `headless`, and the Windows tests in three jobs; the longest job then took 13 minutes 54 seconds.
