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

- Stage 4a — Cooked assets: done and released as 0.2 on 2026-10-15. *Hunna Wood* loads its zone
  from cooked assets and is played by day and by night, on Linux and on Windows.
- Stage 4b — Live assets: planned on 2026-10-15 and validated by the owner
  ([ADR-0072](adr/0072-stage-4b-live-assets.md)). M4b.1 done on 2026-10-15: the renderer loads
  scenes on threads of its own, which create their GPU objects through clones of the `Gpu`
  ([ADR-0073](adr/0073-loading-scenes-in-the-background.md)). M4b.2 done on 2026-10-15: the game
  draws while its zone loads ([ADR-0074](adr/0074-uploading-textures-without-holding-up-frames.md)).
  M4b.3 done on 2026-10-15: an application that asks for it goes on after a lost GPU device, with a
  new one ([ADR-0075](adr/0075-recovering-from-a-lost-gpu-device.md)). M4b.4 done on 2026-10-15:
  *Hunna Wood* goes on after a lost device, its run kept; its checks by hand with a real driver
  reset, on Linux and on Windows, are M4b.9's. M4b.5 done on 2026-10-15: a game changes a
  material's values while it runs, and *Hunna Wood*'s campfire flickers. M4b.6 built: a texture
  changed on disk changes in the running game; the owner tries it.

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M4b.1](#m4b1--a-shared-gpu-and-a-task-pool) | The renderer loads a scene on threads of its own, which create its GPU objects | Done |
| [M4b.2](#m4b2--the-zone-loads-while-frames-go-on) | *Hunna Wood* draws while its zone loads | Done |
| [M4b.3](#m4b3--recovering-from-a-lost-gpu-device) | An application that chooses it goes on after a lost GPU device | Done |
| [M4b.4](#m4b4--hunna-wood-goes-on-after-a-lost-device) | *Hunna Wood* goes on after a lost GPU device | Done |
| [M4b.5](#m4b5--materials-that-change-in-place) | A game changes a material's values while it runs | Done |
| [M4b.6](#m4b6--hot-reload-of-cooked-data) | A texture changed on disk changes in the running game | Built; the owner tries it |
| [M4b.7](#m4b7--hot-reload-of-shaders) | A shader changed on disk changes in the running game | Planned |
| [M4b.8](#m4b8--refining-hunna-wood) | *Hunna Wood* refined with hot reload, on the owner's list | Planned |
| [M4b.9](#m4b9--windows-check-documentation-close) | Stage 4b is done; Stage 5 is planned | Planned |

Stages 0, 1, 2, 3a, 3b, 3c, 3d and 4a are done; see [Completed stages](#completed-stages).

## Stage 4b — Live assets

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-4b--live-assets)): a texture and a shader
hot-reload in the running game; the game's zone loads while frames go on; the game goes on after a
lost GPU device.

This is the second half of Stage 4, the asset pipeline
([ADR-0068](adr/0068-stage-4-in-two-halves.md)). The first half turned source files into cooked
data that a game loads by ID ([ADR-0069](adr/0069-the-asset-pipeline.md)); this one makes what is
loaded live. Three things, where the engine stands at the start:

- **Loading in the background.** `Renderer::load_scene` loads a scene on the thread that calls it,
  and *Hunna Wood* loads its zone before its window opens. The stage loads scenes on threads of the
  renderer's own, which create the GPU objects themselves, while the game goes on drawing.
- **Hot reload**: replacing, in a running game, the cooked data or the shaders that changed on disk,
  without restarting it. A material cannot change once it is made
  ([ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md)), and the shaders are
  embedded in the program. The stage makes a material's values change in place, which a game uses
  too (a fire that flickers), then reloads cooked data, then shaders.
- **Recovering from a lost GPU device.** A lost device stops the run
  ([ADR-0022](adr/0022-losing-the-gpu-device.md)). The stage opens a new device, for the
  applications that choose it, which create their GPU objects again.

Out of scope: handles to assets and their load states, progressive loading, assets shared between
scenes (Stage 5's world model, ADR-0072); a public task pool or job system (Stage 7); a loading bar
and any text of games (Stages 6 and 7); watching files through the operating system's
notifications; reloading a game's code; recovering for applications that do not choose it;
reloading shaders in a shipped game; audio (Stage 7).

Decisions taken when the stage was planned, by the owner on 2026-10-15, with the alternatives they
rejected, are in [ADR-0072](adr/0072-stage-4b-live-assets.md):

- **The order**: the risk first, recovery before hot reload.
- **Loading**: a load that the game polls, which gives the scene whole; a task pool private to
  `slime-render`; the bar of 33 ms between frames; the sky and the hero while the zone loads.
- **Recovery**: chosen by each application; the same wgpu instance and a new surface; no CPU adapter
  after a hardware one; giving up after three recoveries without a frame.
- **Materials**: their values change in place, their textures do not; found by name.
- **Hot reload**: `slime-cook watch`, which checks the sources every half second; shaders read from
  their source files; off unless `--watch`; the game starts again from the start when its layout
  changed, and goes on when only its looks did.
- **The rest**: a refining of the game, on at most five points that the owner chooses; no release;
  no new dependency.

Risks:

- **Frames held up by loading.** In wgpu 30, writing a texture through the queue holds a lock that a
  submission takes too, so the loading threads can delay frames; M4b.2 measures it against the bar,
  with the fallback that ADR-0072 names. M4b.2 removed it otherwise (ADR-0074).
- **Recovery on real drivers.** A device destroyed by a test is not a driver reset. Real resets are
  checked by hand: on the owner's AMD GPU through the driver's recovery, which needs root and which
  the owner runs, and on Windows by a forced timeout detection and recovery (TDR) in the virtual
  machine. Direct3D 12's second swap chain on one window, the dev overlay's egui renderer, which can
  panic on a lost device (see [Known debt](#known-debt)), and threads still loading with the lost
  device are the likely failures.
- **Shader reload on Direct3D 12**: making every pipeline again pauses the game, for a time not
  measured yet.
- **The cost of the checks.** The zone costs up to 8 s per frame on lavapipe, and every scenario that
  draws it adds that; the Windows CI job takes 17 of the 20 minutes that the owner allows.

### M4b.1 — A shared GPU and a task pool

- **Goal.** The renderer loads a scene on threads of its own, which create its GPU objects.
- **Crates.** `slime-gfx`, `slime-render`.
- **Delivers.** A `Gpu` that threads share; the renderer's task pool, private; starting the load of
  a scene, polling it, taking the scene once loaded, with `load_scene` as starting and waiting; the
  first error of a load returned and the rest of its work cancelled; a panic of a loading thread
  raised again on the thread that takes the load; a load dropped cancelled and its threads joined;
  the interval between frames measured while the zone loads today, as the baseline; the ADR on
  loading in the background; the documents (`slime-render`'s README, `ARCHITECTURE.md`'s threads,
  the glossary).
- **Done when.** Tests: a texture created on another thread draws; a scene loaded in the background
  draws the same pixels as one loaded on the calling thread; an asset that fails fails the load;
  two loads at once; a load dropped half-way; a panic raised again; CI is green.
- **How to test.** `cargo test -p slime-gfx -p slime-render`.
- **Outcome.** Done on 2026-10-15 ([ADR-0073](adr/0073-loading-scenes-in-the-background.md)). The
  baseline, before the change: *Hunna Wood*'s window showed nothing while its zone and its hero
  loaded, one after the other, on the main thread, so no interval between frames could be measured
  then. On the pool's threads, both load in under half the time; the ADR holds the figures. The game
  still loads before its first frame, by waiting: M4b.2 makes it draw meanwhile. Two checks were
  changed from what this milestone planned. No loader on the calling thread remains to compare
  pixels with, so the test of a scene loaded in the background checks where each mesh is placed,
  which meshes and materials are shared, and the color each square shows. A load cannot be dropped
  half-way on purpose, so the test of a dropped load drops it while every thread of the pool is
  busy and its tasks wait in the queue, and checks that the drop returns at once.

### M4b.2 — The zone loads while frames go on

- **Goal.** *Hunna Wood* draws while its zone loads.
- **Crates.** `games/hunnawood`; `slime-render` if the measurement asks for the fallback.
- **Delivers.** The window opened at once; the loading view, the sky and the hero at the start, the
  camera turning, the hero still; the zone appearing whole; `--frames` counting the frames that draw
  the zone; the intervals between frames measured while the zone loads, in a release build at
  1280 × 720 on the RX 6900 XT, with 1, 2 and 4 loading threads; the fallback if the bar is missed;
  the game's README.
- **Done when.** While the zone loads, no interval between frames above 33 ms on the RX 6900 XT; a
  test shows the rules still until the zone is there; the game runs in the headless Weston; the
  owner plays it; CI is green.
- **How to test.** `cargo run --release -p hunnawood`.
- **Outcome.** Done on 2026-10-15. The owner ran it on Linux, in a window of 1901 × 2082 pixels:
  the zone came after three frames, none more than 20 ms apart. The bar holds, by day and at night,
  with a margin ([ADR-0074](adr/0074-uploading-textures-without-holding-up-frames.md) holds
  the figures). Measuring it found what held frames up, and changed three things that the
  milestone did not plan: textures reach the GPU through buffers of their own, in `slime-gfx`,
  rather than through wgpu's queue writes, whose lock held up the renderer's submissions;
  `slime-render` gives the lights of a cooked scene before it is loaded (`scene_lights`), so that
  the game computes its night sky, around the zone's moon, before its first frame; and the hero
  loads before the first frame, so that the first shadows do not come in the middle of the zone's
  load. The loading view is the sky and the hero, lit by the zone's lights. On the RX 6900 XT, with
  the files in the system's cache, the zone is drawn after a few frames, or from the first; it takes
  longer on slower machines. The fallback of ADR-0072, uploads from the main thread, was not
  needed; the owner chose the buffers of `slime-gfx` over it, once the two were laid out. The
  `scene` example loads its scene in the background too, as the rule of one example per notable
  feature asks.

### M4b.3 — Recovering from a lost GPU device

- **Goal.** An application that chooses it goes on after a lost GPU device.
- **Crates.** `slime-gfx`, `slime-app`; `slime-devui` and `slime-render` for what they create.
- **Delivers.** The kind of a loss: the driver's, the application's own, memory that ran out; a new
  device opened through the lost one's instance, with or without a window, and the window's surface
  created again; the run's setting that chooses recovery and the method of `App` that it calls once
  the new device is open; the lost device kept until the new one opens; the limits: no CPU adapter
  after a hardware one, three recoveries in a row without a presented frame, a device that ran out
  of memory; a way to lose the device on purpose; scenarios of the windowed lifecycle test where the
  device is restored, and lost again; the ADR that amends ADR-0022; the documents.
- **Done when.** Tests: a new device draws the same pixels as the lost one did; an object of the
  lost device is refused by the new one; a device that ran out of memory is not replaced; an
  application that does not choose recovery stops as before; the two scenarios pass in the headless
  Weston; CI is green.
- **How to test.** `cargo test -p slime-gfx -p slime-app`; `scripts/check.sh`.
- **Outcome.** Done on 2026-10-15
  ([ADR-0075](adr/0075-recovering-from-a-lost-gpu-device.md)). `Gpu::lose` records the loss at
  once, without the poll that destroying a device through wgpu needed: the `device-lost` scenario
  now goes through the release of the window's image by `SurfaceFrame::present` after a loss,
  which was checked by hand only, and the tests of `slime-app` and `slime-render` no longer need
  wgpu. A device lost because memory ran out is refused by the test that makes it run out; the
  refusal of a CPU adapter is tested where the tests run on one, lavapipe and WARP. A real driver
  reset is M4b.4's, by hand. A review found that Direct3D 12 opens no device on an adapter while a
  removed one exists in the process: the owner chose two steps, `App::gpu_lost` to release every
  object of the lost device, then `App::gpu_restored`, as Microsoft's templates do, over keeping
  one step and recovering on Vulkan only. The `lost-now-and-then` scenario checks that the count of
  new devices in a row starts again after a frame shown.

### M4b.4 — Hunna Wood goes on after a lost device

- **Goal.** *Hunna Wood* goes on after a lost GPU device.
- **Crates.** `games/hunnawood`.
- **Delivers.** The game choosing recovery: its view, statistics and dev overlay made again, its
  zone loaded again in the background behind the loading view, its run kept (where the hero
  stands, the key, the door, the time of day); `--lose-device-at <frame>`; a button of the dev
  overlay that loses the device; `scripts/check.sh` running the game with `--lose-device-at`; the
  game's README.
- **Done when.** The game, losing its device in the headless Weston, draws frames after the loss with
  its run kept; the owner's reports from Linux, with a driver reset, and from the Windows virtual
  machine, with a forced TDR.
- **How to test.** `cargo run -p hunnawood -- --lose-device-at 300`; the button of the dev overlay
  (F1).
- **Outcome.** Done on 2026-10-15, without the owner's reports, which M4b.9 takes. The game
  loses its device on purpose, from `--lose-device-at` or the dev overlay, and goes on with a new
  one: on the RX 6900 XT, in a window, and on lavapipe, in the headless Weston, where
  `scripts/check.sh` now runs it. Its run is kept, and the log says where it was. On Windows, CI's
  tests replace a lost device without a window on WARP, through Direct3D 12; nothing has yet
  replaced one with a window there. A real driver reset was not checked: on Linux, the two that
  happened on the owner's machine that day, caused by other programs, ended the compositor and the
  session with it (see [Known debt](#known-debt)), and the Windows virtual machine was out of
  reach. The owner chose to leave both checks to M4b.9: on Linux in a KDE Plasma session, on
  Windows with the button and a forced TDR.

### M4b.5 — Materials that change in place

- **Goal.** A game changes a material's values while it runs.
- **Crates.** `slime-render`, `games/hunnawood`.
- **Delivers.** A material's values read and changed in place, its textures fixed; a loaded scene's
  materials found by name; *Hunna Wood*'s campfire flickering, its flames, its embers and its light
  together, and the key glowing in pulses at night; the documents.
- **Done when.** Tests: a material changed draws as one made with the new values; values refused
  at creation are refused in place too; a name that no material has finds none; the owner sees the
  fire.
- **How to test.** `cargo test -p slime-render`; `cargo run -p hunnawood -- --night`.
- **Outcome.** Done on 2026-10-15. The owner looked at the fire and the key at night, on Linux. A
  material keeps its values in its uniform buffer, which `Material::set_values` writes for the
  frames submitted after it, and its clones share them. Creation refused no value before: the
  test that the plan asked for needed it to, so creation and changes now refuse the same values,
  out of glTF's ranges or not finite, which the cooked zone and the samples keep within; the
  cooker refuses them in glTF files, as it refuses lights that glTF forbids. The
  game finds the campfire's light as the night's point light at the campfire, since the light's
  node has no name of its own (Blender named it `campfire.001`). The `scene` example changes how
  strongly its ring glows, as the rule of one example per notable feature asks. The rustdoc of
  `MaterialValues` and `Material::set_values` holds the contracts; ADR-0072 holds the decision.

### M4b.6 — Hot reload of cooked data

- **Goal.** A texture changed on disk changes in the running game.
- **Crates.** `slime-cook`, `slime-assets`, `slime-render`, `games/hunnawood`, `slime-examples`.
- **Delivers.** `slime-cook watch`; temporary files named with the process ID; the manifest written
  only when its content changes; cooked data that reads a new manifest; a scene loaded again in the
  background, reusing the GPU objects that did not change; `--watch` in *Hunna Wood* and the `scene`
  example; the zone's rules read again, the run starting again when the game's layout changed; a
  reload that fails keeping the old scene; how the cooker names a glTF file's outputs measured,
  since a name made from the whole file changes every output when one part changes, and changed if
  it defeats the reuse; the ADR on hot reload; the documents (`CONTRIBUTING.md`, the game's
  README, `examples/README.md`).
- **Done when.** Tests: `watch` cooks a changed image, and only what it makes; two cooks at once
  leave the cooked data whole; the manifest unchanged by a cook that changed nothing; a reloaded
  scene draws the new texture and keeps the objects that did not change; a change of layout starts
  the run again, a change of looks does not; by hand, a texture of the zone changed while the game
  runs, and the owner tries it; CI is green.
- **How to test.** `cargo run --release -p slime-cook -- watch games/hunnawood/assets/zone
  target/cooked/hunnawood` beside `cargo run -p hunnawood -- --watch`.

### M4b.7 — Hot reload of shaders

- **Goal.** A shader changed on disk changes in the running game.
- **Crates.** `slime-gfx`, `slime-render`, `games/hunnawood`, `slime-examples`.
- **Delivers.** Uniform layouts that declare their minimum sizes; the renderer's shaders as sources
  that it can be given while it runs, embedded by default; its pipelines made again from new
  sources, all or none, the old ones kept on an error that names its line and column; with
  `--watch`, *Hunna Wood* and the `scene` example reading the shaders from `crates/slime-render/src`
  and giving them to the renderer when they change; the pause measured on Direct3D 12; the
  documents.
- **Done when.** Tests: a shader that does not fit its data refused when its pipeline is made; new
  sources equal to the embedded ones draw the same pixels; a broken shader keeps the old pipelines
  and reports its line and column; by hand, a change in a shader seen in the running game; CI is
  green on both platforms.
- **How to test.** `cargo run -p hunnawood -- --watch`, then edit a shader.

### M4b.8 — Refining Hunna Wood

- **Goal.** *Hunna Wood* refined with hot reload, on the owner's list.
- **Crates.** `games/hunnawood`: its zone's script and assets.
- **Delivers.** At most five points, which the owner chooses at the start of the milestone, among
  what was left to refine while playing at Stage 4a or what they noticed since: the leaf cards,
  which read as cards up close; the stone and the ground, warmer than the zone's grey palette; the
  fire, three glowing tongues until particles; the hero's smooth face; the capital's picture,
  larger than the sizes decided (see [Known debt](#known-debt)). No new engine feature; new source
  assets and new releases of the mirror only with the owner's agreement
  ([ADR-0070](adr/0070-the-sample-games-source-assets.md)).
- **Done when.** The owner's report on each point; the tests of the cooked zone, budgets included,
  pass; CI is green.
- **How to test.** Playing it.

### M4b.9 — Windows check, documentation, close

- **Goal.** Stage 4b is done; Stage 5 is planned.
- **Crates.** All, for their documents.
- **Delivers.** The documents reread as a newcomer would; `CHANGELOG.md`'s `[Unreleased]` section;
  the owner plays the game from CI's Windows programs, and loses its device there, from the dev
  overlay and with a forced TDR (`dxcap -forcetdr`); the driver reset on Linux, in a KDE Plasma
  session, since niri and Hyprland do not survive one (see [Known debt](#known-debt)): the checks
  by hand that M4b.4 left; Stage 4b marked done; Stage 5 planned with the owner. No release: what
  the stage delivers is released with 0.3, at the end of Stage 5 (ADR-0068).
- **Done when.** The exit criterion holds, on Linux and on Windows: hot reload by hand on Linux and
  through CI's tests on Windows, since CI's Windows programs hold no cooker and no shader sources
  (ADR-0072); CI is green.
- **How to test.** Reading; CI; playing it.

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)):

1. In one commit, set the version in the root `Cargo.toml` and the tag that the README's "Using
   SlimeEngine in a game" names, rename the `[Unreleased]` section of `CHANGELOG.md` to
   `[<version>] - <date>`, and update this plan.
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.
4. From 0.1 on, create the GitHub release of the tag, its `CHANGELOG.md` section as its notes,
   with the `slime-examples-windows` artifact of that commit's CI run attached as
   `slime-examples-windows-<version>.zip`, holding beside the programs, and DXC's library and
   licence that CI puts with them, `LICENSE.txt`, a `NOTICE.txt` and the licenses of the egui fonts
   that `scene.exe` embeds, which the Ubuntu Font Licence asks to travel with them: CI keeps its
   artifacts 14 days, a release keeps them.

0.0.1 was released on 2026-10-12, before Windows joined CI: the tag `v0.0.1` is on the commit
"Release 0.0.1". 0.0.2 was released the same way on 2026-10-12: the tag `v0.0.2` is on the
commit "Release 0.0.2", and 0.0.3 on 2026-10-13: the tag `v0.0.3` is on the commit
"Release 0.0.3". 0.1 was released on 2026-10-14: the tag `v0.1.0` is on the commit "Release 0.1.0",
and its GitHub release carries the Windows examples that CI built from that commit.
0.2 was released on 2026-10-15: the tag `v0.2.0` is on the commit "Release 0.2.0", and its GitHub
release carries the Windows programs that CI built from that commit, *Hunna Wood* with its zone
cooked and the notice that credits its assets among them.

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3a — First renderer → 0.0.3 | M3.1 GPU resources for a renderer; M3.2 the render crate; M3.3 directional light and material; M3.4 textures and samplers; M3.5 a glTF scene and camera control; M3.6 statistics, GPU timings and the dev overlay; M3.7 Windows check, documentation, release | Done; released as 0.0.3 (tag `v0.0.3`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3b — Physically based renderer → 0.1 | M3b.1 GPU layer for multi-pass rendering; M3b.2 HDR rendering and tone mapping; M3b.3 physically based materials; M3b.4 light types; M3b.5 shadows; M3b.6 image-based lighting; M3b.7 bloom; M3b.8 render graph; a pass over the known debt; M3b.9 Windows check, documentation, release | Done; released as 0.1 (tag `v0.1.0`), with the Windows examples on its GitHub release. Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3c — A first game | M3c.1 skeleton; M3c.2 guards, detection and the objective; M3c.3 night, feel and the Windows check; M3c.4 findings for the next stages | Done; no release of its own: its game, *Nightwatch*, is never released, as *Hunna Wood* replaces it in Stage 4a ([ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)). The stage each of its findings goes to is in [ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md), and the two that became Stage 3d in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md). |
| 3d — Anti-aliasing and the shadows of spot lights | M3d.1 multisampling in the GPU layer; M3d.2 anti-aliasing in the renderer; M3d.3 spot lights' shadows: what each tile sees; M3d.4 spot lights' shadows: drawn and read; M3d.5 *Nightwatch* and the Windows check; M3d.6 a smoother shadow filter; M3d.7 what each cascade sees; M3d.8 the cascades read; M3d.9 *Nightwatch* at 3840 × 2160 | Done; no release of its own: what it delivers is released with 0.2. Its decisions are in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) and ADR-0058 to [ADR-0065](adr/0065-reading-the-cascades.md); the cost of its scene pass on Direct3D 12, with each shader compiler, is in the [Known debt](#known-debt). |
| 4a — Cooked assets → 0.2 | M4a.1 *Hunna Wood*'s skeleton; M4a.2 asset identity; M4a.3 images; M4a.4 glTF; M4a.5 the sample scene through the pipeline; M4a.6 the zone's sources; M4a.7 *Hunna Wood* by day; M4a.8 night; M4a.9 Windows check, documentation, release 0.2 | Done, released as 0.2 on 2026-10-15. Its decisions are in [ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md) to [ADR-0071](adr/0071-cooked-textures.md); the zone's budgets are set by the test that checks them, in `games/hunnawood/src/zone.rs`, and what was left to refine goes to [M4b.8](#m4b8--refining-hunna-wood). |

The details of each completed milestone (what it delivered, how it was checked, what was measured)
are in the history of this file and in the commits that implemented it; the measurements that
justify a decision are also in its ADR.

## Known debt

Limitations known and accepted for now. The ones that a contributor could take on alone are also
GitHub issues, linked from their entry; this list stays the reference.

- **The first frame that draws shadows costs more, and more still while a scene loads**
  ([ADR-0074](adr/0074-uploading-textures-without-holding-up-frames.md)). It makes the renderer
  create its shadow atlas and its frame again; during a load, the GPU, busy with the load's copies,
  presents that frame late, past 33 ms in some runs. *Hunna Wood* draws its hero, and so its first
  shadows, from its first frame. Creating the atlases with the renderer would remove it, at a cost in
  memory for every program, shadows or not.
- **The zone costs up to 8 s per frame on lavapipe**, the CPU driver of the checks, where the RX
  6900 XT takes under 2 ms: most of it goes to the four shadow cascades of the sun, each 2048 ×
  2048 texels, through which thousands of overlapping leaf cards are drawn with a shader that
  discards. The checks run the game for one frame by day and one at night. Leaves that cast no shadow in the
  farther cascades, or fewer and smaller cards, would lower it, to measure with views of the zone.
- **The tests of the cooked zone run in `scripts/check.sh` only**: they need the zone fetched and
  cooked, so `cargo test` leaves them out (`#[ignore]`), and CI runs them on Linux, not on Windows.
- **The capital's picture is 4096 × 1536 texels**, beyond the sizes decided for Stage 4a (2048 for
  the ground, the walls, the cliffs and the hero, 1024 for small props): it spans the whole view
  north of the zone. It takes 8 MiB compressed.
- **Frames are not synchronized with the display's refresh** (ADR-0021; issue
  [#1](https://github.com/HoneySlatt/SlimeEngine/issues/1)). With Mailbox, the default, the
  schedule alone paces frames: on a 60 Hz display, an image is now and then shown twice while the
  next one is dropped. On a display of 144 or 165 Hz, the default 60 frames a second last 2 or 3
  refreshes in turn, which shows as judder in motion: a game can raise `AppConfig::max_frame_rate`
  to the display's rate, which the engine does not report, or choose `PresentMode::Fifo`, which
  shows one image per refresh but can stall as the next entry says. Aligning frames on the
  compositor's frame callbacks is to reconsider when a renderer draws real scenes.
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
  On Windows, a window covered by another keeps drawing too; a minimized one, of size zero, draws
  nothing (M3b.9).
- **wgpu's errors panic, in release builds too** (ADR-0016): a use of wgpu that it rejects is
  treated as a bug. Before games ship, logging the error and dropping the frame is to be
  considered. Running out of GPU memory is not such a use: creations return
  `GfxError::OutOfMemory`, and memory that runs out where no caller hears of it gives the device up,
  which stops the run (ADR-0044). The engine avoids the rejections that the environment can
  cause: a window larger than the device's largest image is drawn at that size (`Surface::resize`),
  and a shader or pipeline that the driver rejects is returned as an error (`GfxError::Shader` or
  `GfxError::Pipeline` on Direct3D 12; on Vulkan, wgpu treats the driver's failure as a lost
  device, `GfxError::DeviceLost`); a window's surface that the graphics API refuses to configure
  counts as lost (ADR-0025). The refusal of a configuration is not tested: nothing the tests
  control makes a conformant driver refuse one.
- **On Linux, a game goes on after a real reset of the GPU only if the compositor does**
  (ADR-0075). A reset reaches every program that uses the GPU. Mesa ends the ones that did not ask
  to be told of it, as niri, Hyprland and COSMIC do not: on 2026-10-15, programs that hung the
  owner's RX 6900 XT made the amdgpu driver reset it twice, and niri ended each time, taking the
  session with it. KDE Plasma's KWin recovers from resets, and a game built on SlimeEngine can go
  on there; checking it by hand is part of M4b.9. On Windows, the desktop's compositor survives
  resets.
- **A lost GPU device is replaced only for the applications that ask for it** (ADR-0075; issue
  [#2](https://github.com/HoneySlatt/SlimeEngine/issues/2)): the others stop with an error, and
  since winit allows one event loop per process, carrying on means starting the process again. An
  application that asks drops and creates its GPU objects again itself, as *Hunna Wood* does. A
  real driver reset can only be checked by hand, which M4b.9 does. A restore that fails in a windowed run is
  not tested: nothing makes one fail on purpose there.
- **In capture tools, the sky pass shows as a second `scene`.** A pass is named after the target it
  draws into, and the sky pass draws into the scene's high-dynamic-range target, after the scene
  pass (`declare_frame` in `crates/slime-render/src/renderer.rs`); only its pipeline is named `sky`.
  Seen in RenderDoc on Direct3D 12 (M3b.9).
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
- **Input not reported yet** (issue [#3](https://github.com/HoneySlatt/SlimeEngine/issues/3)):
  input-method text, touch, touchpad gestures, gamepads, dropped files, the mouse's raw motion.
  The window cannot grab or hide the cursor, go fullscreen, or change its title once created.
- **A compositor that stops answering blocks the engine.** Before the first frame, winit waits for
  the window's first configuration without a timeout, so a termination signal is noticed only when
  a second one ends the process; after `shutdown`, releasing the window or the surface can block
  too (seen with a Weston stopped by SIGSTOP).
- **Logging off or shutting down Windows ends a game without `App::shutdown`.** Windows tells a
  window of it with end-session messages, which winit does not report, and sends no console event
  to a process that has loaded `user32.dll`, as every engine process has
  (`crates/slime-app/src/signal.rs`): what a game saves in `shutdown` is lost then.
- **WARP's shader compiler crashes the process, now and then, when several threads compile
  shaders at once**, even on separate devices: an access violation (`STATUS_ACCESS_VIOLATION`)
  that other projects trace to `d3d10warp.dll`, and that no test can catch. Seen once in the
  Windows CI job, on 2026-10-13, in the texture tests of
  `slime-gfx`, after about ten green runs; other projects that test on WARP report the same crash,
  and that running the tests one at a time avoids it, which the Windows job does since, and which
  `CONTRIBUTING.md` asks of contributors on WARP. The engine's own code is not involved.
- **Client-side decorations run `dbus-send` twice when the window is created** (winit's
  sctk-adwaita, on compositors that do not draw decorations), each waiting up to 100 ms when the
  settings portal does not answer; the error they log when no session bus or portal answers is
  hidden by the default log filter (ADR-0019).
- **Upgrading wgpu waits for egui** while the engine's tooling draws with it: the dev overlay, and
  from Stage 6 the editor (ADR-0031, ADR-0052). `egui-wgpu` is built against one wgpu version
  (egui 0.36 against wgpu 30 today), so wgpu moves only once egui follows.
- **The dev overlay blends in an sRGB framebuffer.** egui prefers a linear one for its blending,
  warns once at startup, and its panels look slightly lighter than egui's own demos (ADR-0031).
  Harmless for tooling.
- **Mipmaps of textures made in code are computed on the CPU when they are created**
  (`Gpu::create_texture`, whose documentation gives the cost), on the calling thread, in work
  proportional to the texture's pixels (ADR-0032): a level of tens of large textures waits seconds.
  A caller with levels of its own gives them to `Gpu::create_texture_with_levels` (ADR-0039), as
  cooked textures do, whose levels the cooker computed (ADR-0071).
- **Cooked bytes are the same from one cook to the next on one platform, not across platforms**
  (ADR-0071): the cooker computes the sRGB curve with the platform's mathematical functions, which
  may round differently on Linux and Windows. Tests compare decoded values across platforms; a
  table of the curve's values would make the bytes the same everywhere.
- **A cook reads and hashes every source file at every run**, and cooked files that no source makes
  any more stay in the output directory (ADR-0071): nothing records a source's size and time to
  skip unchanged ones, nor cleans the output. To measure on *Hunna Wood*'s zone.
- **Every offscreen render target and depth target can be sampled** (ADR-0033): they all carry the
  usage that allows it, which may cost some compression on some GPUs. Measured at M3b.5 on an RX
  6900 XT (RADV), with two builds that differ only by it: no cost (the scene pass took 43.9 µs
  with it and 44.0 µs without at 1920 × 1080, ADR-0033's correction). Other GPUs are not measured.
- **A pipeline that does not fit its pass, or a target sampled by the pass that draws into it,
  panics only when the commands of its submission are finished** inside `Gpu::submit` (or
  `Gpu::render`, `Gpu::render_depth`), with wgpu's message naming the pass, rather than at the
  faulty call (ADR-0033, ADR-0041). For the renderer's frame, the render graph refuses a pass that
  samples what it draws into before wgpu sees it.
- **On X11, losing the connection to the X server ends the process** with exit status 1, inside
  the X11 library (Xlib). `EventHandler::exiting`, `App::shutdown` and the return of `run` do not
  happen, although ADR-0010 promises that `shutdown` is called once `init` has been. winit
  installs no IO error handler. Installing one (`XSetIOErrorExitHandler`, libX11 1.7 or later)
  would need a new direct dependency such as `x11-dl`. On Wayland, a compositor that dies ends the
  loop cleanly.
- **Without libxkbcommon, the windowing library panics** rather than returning
  `PlatformError::Connect`. On X11 it panics at once, also without libxkbcommon-x11 or without the
  X server's XKB extension. On Wayland it panics when the compositor first reports a keyboard,
  which can be in the middle of a run. Turning it into an error would mean probing the libraries
  before winit does.
- **A termination signal received after `run` has returned no longer ends the process by itself**
  (ADR-0012). `ctrlc` cannot remove the handler that `run` installs. A first signal received then
  only stops later runs, and logs that the run stops after the current frame; only a second one
  ends the process, with exit code 130. This matters for a program that keeps working after
  `run`, for example under a service manager, which waits out its stop timeout. Ending the process
  at once when no run is active would amend ADR-0012.
- **A window frame dropped without being presented keeps one of the window's images on Vulkan.**
  wgpu 30 discards such a frame with wgpu-hal's `discard_texture`, which does nothing on Vulkan.
  The image stays acquired until the surface is configured again, and once none is left,
  `Surface::acquire` waits one second and returns `None`. `SurfaceFrame` documents this as a
  misuse, and nothing reports it; the engine always presents. A `Drop` that presents was rejected:
  it would show a black or half-drawn frame instead of skipping it, and present during unwinding.
- **An oversize window's surface is configured again at every frame on X11.** A window wider or
  taller than the device's largest image (8192 pixels with the default limits) gets images of that
  size, smaller than the window. On X11, Mesa's Vulkan drivers mark such a swapchain suboptimal, and
  every image of it too. `Surface::acquire` treats each suboptimal image as a reason to configure
  the surface again, at the same clamped size, so every frame waits for the GPU to finish and
  rebuilds the window's images. The frames are correct, and only `debug` logs show it; Wayland and
  Direct3D 12 are not affected. Not setting `stale` in the `Suboptimal` arm of `Surface::next_image`
  while `oversize` is set is not enough: measured, the `Outdated` branch then configures the surface
  again. Asking the device for the adapter's own largest image (`max_texture_dimension_2d`) instead
  of the default limit would make such windows rare. To measure, before and after: start Xvfb with a
  screen wider than 9000 pixels; with `WAYLAND_DISPLAY` unset, `DISPLAY` set to it and lavapipe, run
  the child of the `oversize` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` with
  `SLIME_LOG=wgpu_core=debug`; count wgpu-core's `configuring surface with` records (about one per
  frame without the fix, one with it) and note the frame times.
- **Termination signals are ignored for the rest of the process when `ctrlc` cannot start its
  thread.** `ctrlc` installs its handlers, then starts the thread that waits for what they catch.
  When that thread cannot start (a limit on threads reached when `run` installs the handler), the
  handlers stay installed and SIGINT, SIGTERM and SIGHUP (Ctrl+C and Ctrl+Break on Windows) are
  ignored: only SIGKILL, or closing the console on Windows, ends the process. The engine logs a
  warning that says so and gives the system's error. Restoring the default dispositions would need
  a direct dependency on libc or nix.
- **Any `trace` directive in `SLIME_LOG` makes wgpu build its per-command trace records.**
  tracing-subscriber sets the `log` crate's maximum level from the most verbose directive. With
  `slime_app=trace`, wgpu-core then formats and filters out about five records per draw while it
  encodes a pass, which counts in the caller's span (`render` in a windowed run): measured with
  `RenderStats::cpu_time` for a scene of 1000 instances, 0.38 ms more per frame, 35 %, with
  `slime_app=trace` than with `SLIME_LOG` unset. Alternatively, profile with
  `perf record -g` and look for `tracing_log::LogTracer::log`, `EnvFilter::enabled` and
  `wgpu_core::resource::Labeled::error_ident`. Capping the `log` level needs `log` or
  `tracing-log` as a direct dependency (ADR-0006). Span close lines are also written while the
  enclosing span is entered, so `frame`, `render` and, with `slime_gfx=trace`,
  `RenderStats::cpu_time` include them: take timing baselines with `SLIME_LOG` unset.
- **On Windows with Vulkan (`WGPU_BACKEND=vulkan`), an acquire that waits more than one second
  loses the device.** wgpu-hal 30.0.1 waits on a fence after `vkAcquireNextImageKHR` hands out an
  image, on Windows only, and maps a timeout of that wait to `DeviceError::Unexpected`, which
  wgpu-core treats as the loss of the device. A GPU frame longer than about one second, or a
  compositor that holds the window's images that long (a covered window, a locked session), then
  stops the run with `GfxError::DeviceLost`, with a reason that blames the driver; on Linux the
  frame is skipped instead. Not reproduced: in the owner's Windows 11 virtual machine with an AMD
  driver, the `scene` example kept running with its window covered for six seconds and with the
  session locked (2026-10-13). To report to wgpu: that wait's timeout should give
  `SurfaceError::Timeout`, as its other waits do.
- **Seen from below, the sample scene's walls and crates flicker where they touch the floor.** Their
  bottom faces lie in the floor's plane, and since M3b.3 the floor is double-sided, so from below
  both are drawn at the same depth (z-fighting). The `scene` example keeps its camera above the
  floor, but another viewer of the asset sees it. Lifting them by a few millimetres in
  `examples/assets/make_scene.py` fixes it, at the cost of rendering the glTF references again.
- **A mouse button can stay down on Wayland when the compositor takes the pointer away during a
  press without taking the keyboard focus.** winit 0.30 reports only `CursorLeft` then, and
  `Translator` (`crates/slime-platform/src/event.rs`) releases held buttons only when the focus is
  lost. Releasing them on `CursorLeft` would be right on Wayland only: on X11 and Windows the
  cursor leaves the window during an ordinary drag.
- **On Windows, the first of two held Shift keys let go of is released late.** Windows sends a
  single key-up for both Shift keys, so the engine releases the first one only when the second
  goes up. Windows 11 marks the second Shift's press as a repeat, so neither its press nor its
  release is reported (seen in the owner's virtual machine on 2026-10-13). Polling `GetKeyState`,
  as GLFW and SDL do, would remove the delay and report both keys, but needs `windows-sys` as a
  direct dependency.
- **After a dead key, a press and its release report different logical keys.** On Wayland and
  X11, a character key that does not combine with a pending dead key is reported as
  `Key::Unidentified` with no text; when the keys combine, the press is the composed character and
  the release the plain one. winit exposes no better value; rebuilding one would need libxkbcommon
  called directly. Documented in `KeyEvent::logical`.
- **A key pressed just after the window gains the focus can be swallowed on X11 and Windows.**
  winit reads which keys are down when it handles the focus event, not when the focus changed. A
  key pressed in between counts as held before the focus and is not reported until it is pressed
  again; on X11 the gap covers `App::init` when the window manager focuses the window while it
  runs. The translator cannot tell such a press from X11's unmarked first auto-repeat. On Windows,
  trusting `KeyEvent::repeat` would close the gap, at the cost of a rule that differs between
  platforms; left as documented.
- **No test covers the empty submission of frames without a window image.** When
  `Surface::acquire` gives no image, it submits an empty batch, so that wgpu releases the writes
  an application makes outside `App::render`. Without it, 2000 frames of a surface resized to
  zero, each writing 64 KiB to a uniform buffer, grew the process by about 120 MiB (lavapipe in a
  headless Weston, 2026-10-13), measured by a program kept out of the repository. The
  `draws-nothing` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` checks the same for
  frames whose image nothing draws, not for frames without an image.
- **A GPU time can read 0 once when the timestamp counter wraps.** On adapters whose timestamps
  have 36 valid bits (Intel's Vulkan driver, about once every one to one and a half hours), a
  timed pass that runs across the wrap ends before it starts. `GpuTimer::latest` then gives 0 for
  that pass until the next time replaces it. wgpu does not tell how many bits are valid, so the
  wrapped difference cannot be recovered. Dropping such a sample (`end.checked_sub(start)` in
  `crates/slime-gfx/src/timer.rs`) is a one-line change if it ever matters.
- **The instance buffer is uploaded whole every frame.** `Renderer::render` uploads
  `capacity × stride` bytes (256-byte slots with the default limits), where the capacity is a
  power of two from 64 that fits the frame's instances, up to four times as many: it shrinks back
  once a frame uses a quarter of it or less, so that a 10-instance frame after a 100 000-instance
  one costs what it costs on a new renderer (0.054 ms against 21.4 ms before, on lavapipe).
  Uploading only the live slots would amend ADR-0026's rule that a uniform buffer is always
  written at its creation size.
- **The dev overlay can still panic on a lost device** (ADR-0022). `Overlay::render` skips the
  frame when the device was lost earlier in it, but two cases panic inside egui-wgpu or wgpu: a
  loss that egui-wgpu's own buffer upload is the first to find, and `Overlay::new` on a device
  already lost (egui-wgpu creates its buffers with `create_buffer_init`). Both are documented
  under `# Panics`; closing them needs a change in egui-wgpu or wgpu.
- **Exposed light beyond 64 × 65 504 is clamped** (ADR-0035, ADR-0037, ADR-0039, ADR-0040). The
  scene and sky shaders write exposed light divided by 64 into the `Rgba16Float` target, and light
  above 64 × 65 504 as 65 504, the largest half, rather than leave it to the driver, which may store
  an infinity that the tone curve turns into NaN; lavapipe and RADV clamp it themselves, so that no
  test can show either clamp. The sun's disc, 1.47 × 10⁹ nits for the sample's sun, is clamped from
  an exposure of 2.85 × 10⁻³, six and a half stops above the sample's; emission above
  64 × 65 504 / exposure, and any light once the exposure is raised about 13.5 stops above the
  sample's, are clamped too, and the bloom spreads them with the energy they keep. A sky's radiance
  is at most 65 504 nits before the exposure: brighter zenith, horizon or ground is refused, and a
  brighter glow kept within it. Divided by 64, exposed light below 64 × 2⁻¹⁴, about 0.004, is
  stored in half precision's subnormals, in steps of 64 × 2⁻²⁴: black on an 8-bit display, whose
  darkest step is about 0.007 exposed; an output of higher precision would show its steps.
- **Every fragment evaluates every light of the scene** (ADR-0037). The scene pass has no light
  culling: its cost grows with the number of lights, about 20 µs per light for a screen of 1920 ×
  1080 fragments on an RX 6900 XT (2026-10-13), 338 µs for 16, the most a scene holds, and 10 to 20
  times more on an integrated GPU; a light's range saves no work. More lights would need clustered
  or tiled culling and a storage buffer, a measured decision of Stage 9a (ADR-0052).
- **Point lights cast no shadows, and light what objects hide** (ADR-0037; the first directional
  light that casts shadows casts them, ADR-0038, and so do spot lights, ADR-0058). The sample's ring
  light, a point light, reaches 4 m, and gives little beyond a metre. The shadows of point lights
  come with Stage 9a (ADR-0052).
- **Every instance is drawn, with a draw call of its own** (`Renderer`). Nothing leaves out the
  instances that the camera does not see, and instances of the same mesh and material are not
  drawn together: a frame's CPU and GPU time grow with every instance of the scene, wherever it is.
  Frustum culling and instancing are left to Stage 9a (ADR-0052).
- **Cascades are fitted to spheres around slices of the view** (ADR-0064). Each covers its slice
  whichever way the camera looks, which keeps its texels steady, but a wide view makes the spheres
  large: in the first game, *Nightwatch* (deleted at M4a.7), with the single map's 30 m, the texels
  near the player would be about half as wide as its texels, not a quarter, by ADR-0064's study;
  that game's moon cast its shadows up to 16 m to make them finer. Fitting the cascades to what the camera sees at each frame,
  or virtual shadow maps, is left to Stage 9a.
- **Shadows shift, and thin objects close to a surface lose theirs (peter-panning)** (ADR-0050).
  The normal offset that keeps surfaces from shadowing themselves looks a point up 4.5 texels × the
  sine of the light's incidence off its triangle (ADR-0063, ADR-0065): at the default distance,
  under a light 45° from the normal, 1.2 cm in the first cascade and 18 cm in the last, and 26 cm
  there at grazing. A shadow shifts by up to that much, and a sheet lying closer than that above a
  floor casts nothing on it.
- **Every draw of the scene pass runs a shader that can discard fragments** (ADR-0035). The `MASK`
  alpha mode discards in the one fragment shader that every material uses, opaque materials with a
  cutoff of 0. Some GPUs then test depth after the shader rather than before, for opaque draws
  too, which costs overdraw. Not measured: compare the scene pass's GPU time on a scene with much
  overdraw against a build whose opaque materials use a fragment entry point without `discard`
  (six pipelines instead of three).
- **Edges that a masked material cuts inside a triangle stay aliased** (ADR-0059). Multisampling
  smooths the edges of triangles, but the scene shader runs once per pixel, and the `MASK` alpha
  mode keeps or discards all of a pixel's samples at once. Smoothing those edges, alpha to
  coverage, behaves differently from one driver to the next, which image tests would see. No game
  uses masked materials yet; the leaf cards of *Hunna Wood*'s trees and its ferns will, once the game loads its
  zone (M4a.7).
- **The sky's light is neither shadowed nor occluded** (ADR-0039). A surface under an object, or
  inside a box, receives the whole sky, but for its material's occlusion texture: no ambient
  occlusion (screen-space effects are outside the stage), no reflection probes, and mirrors reflect
  the sky, never the objects around them. An interior is lit by the whole sky through its walls:
  until then, draw it without an environment, or with a sky of lower radiances, and light it with
  point and spot lights. Khronos's Sponza shows it (M3b.9): its arcades are lit by the sky as its
  open courtyard is, in the same blue.
- **Glass is drawn opaque.** The importer leaves out glTF's `KHR_materials_transmission` and
  `KHR_materials_volume`, and warns that it does: the renderer neither lets light through a surface
  nor blends one (Khronos's FlightHelmet and ABeautifulGame, M3b.9). Transparency is left to
  Stage 9a (ADR-0052).
- **One GPU, an AMD, and two CPU drivers have run the engine.** The image tests' tolerances
  (ADR-0030), the measurements of this plan and the samples' looks come from an RX 6900 XT, with
  RADV on Linux and AMD's driver on Windows, from lavapipe and from WARP: NVIDIA's and Intel's GPUs
  are untried.
- **The environment is computed on the CPU, in every process that draws a sky** (ADR-0039): 23 ms
  in a release build on the owner's 16 threads, 162 ms on one core, and the BRDF table once (15
  ms); about eight times longer in a debug build, which every test that draws a sky with a gradient
  or a sun pays: the workspace's tests took 23.9 s instead of 9.7 s on lavapipe on the owner's
  machine, most in `slime-render`'s `tests/environment.rs` (6.2 s) and the examples'
  `tests/gltf.rs` (5.7 s), whose every load of a file computes the sample's sky. A sky that changes
  needs a new environment: no time of day. To measure: `cargo test --release -p slime-render --lib
  -- --ignored --nocapture`, against the budget of M3b.6. To move to the asset pipeline of Stage 4,
  or to compute passes if skies change at run time; if the tests' time matters first, building
  `slime-render` optimized in debug builds would shorten it, at the cost of stepping through it in
  a debugger.
- **The sky's sun is described twice** (ADR-0039). A game gives the same `DirectionalLight` to
  `Sky::sun`, for the disc and the glow, and to `Scene::lights`, for its light and its shadows;
  two different lights show a disc where no light comes from. The sample takes both from the
  file's first directional light.
- **Rough metals reflect the sky with multiple scattering compensated, the lights without**
  (ADR-0039, as the Khronos glTF Sample Viewer): a white metal of roughness 1 reflects all of a
  uniform sky, but its lights keep glTF's single-scattering BRDF, under which it would reflect
  31 % of that sky; rough metals are darker in the sun than in the shade, relatively.
- **A roughness between two prefiltered levels reads a blend of their lobes** (ADR-0039): the four
  levels are 0.25 apart, and a lobe in between is approximated by mixing the nearest two, the split
  sum's usual approximation; each level is within 2.2 % of its own lobe's average.
- **The image tests use up to 75 of the 131 pixels that RADV may have off by more than 8**
  (ADR-0030, ADR-0054): the glTF scene's front view 75, on the wall's mortar joints seen at a
  grazing angle (76 before its textures were cooked); `shapes` 28, on edges and the metal spheres'
  reflections of the sky (49 at M3b.6, before the bloom; 33 before the shadow map followed the
  camera and the output was dithered). A change that adds fine or reflective detail may exceed it;
  measure it again then.
- **Four passes per submission is a number measured on one machine** (ADR-0041). On the owner's
  RX 6900 XT (RADV), whose memory that the CPU can map is a 256 MiB window, one submission of a
  whole bloomed frame stalled in the kernel once the desktop's programs and a test program filled
  that window to 241 MiB (a mean 2.4 ms, 8 ms in one frame of ten, at 1920 × 1080); submissions of
  2 to 6 passes did not, 8 did. The cause in the kernel is not established; Direct3D 12 and other
  GPUs are not measured, and a GPU with resizable BAR has no such window. To measure: the CPU time
  of `render` at 1920 × 1080 with the sample's bloom, for several values of
  `PASSES_PER_SUBMISSION` in `crates/slime-render/src/graph.rs`, with the window filled to
  several levels, by the desktop's programs or by a program that allocates device-local,
  host-visible memory (on Linux, `mem_info_vis_vram_used` in the GPU's sysfs directory tells how
  full it is).
- **The render graph trusts what a pass says it samples from imported targets** (ADR-0041). The
  groups that sample transient targets are built from the declarations, but the scene pass's group
  holding the shadow atlases is built by the renderer: a pass that binds an imported target must
  declare it, or the graph orders it without knowing.
- **A renderer whose frame changes shape at every call reports no tone-mapping or bloom time**
  (ADR-0041). Each change of shape (the output's size, the shadow pass or the bloom starting or
  stopping) builds the frame again with new timers, and the last submission's times come back
  during the next call, by then read from newer timers; such a renderer also creates its targets
  again at every call, as it did before. Use one renderer per output size.
- **Tiny, very bright sources blink in the bloom** (ADR-0040). Without a firefly filter (Karis's
  average dims the sun's halo and makes it flicker, measured), a highlight smaller than a pixel
  blooms as its coverage of the pixel changes from frame to frame. Temporal anti-aliasing, or a cap
  on the light a pixel brings to the chain, belong to Stage 9a (ADR-0052).
- **A pixel whose light is not a number spreads through the bloom** (ADR-0040). Materials' values
  are not checked: a NaN in a base color or an emission makes its pixels NaN in the
  high-dynamic-range target, and the bloom averages them into its levels, and from there into
  every pixel within reach of their halo, a blot where one surface was wrong. Checking materials'
  values as lights' are (ADR-0042), or dropping such light before the bloom, would keep it to its
  pixels.
- **The bloom has one shape** (ADR-0040): every level the same weight, down to a side of 8 texels;
  no radius, threshold or tint for a game to set. A window resized across a power of two adds or
  removes a level: the glow's reach changes a little while the edge is dragged.
- **Light near the right and bottom edges blooms a little more or less** (ADR-0040). Where an
  output's side is not a multiple of 2^levels, the coarse levels' last texels straddle the edge: a
  pixel's halo there holds 0.83 to 1.37 of its light at ten sizes measured, up to 1.9 at the far
  corner (1023 × 1023, 2047 × 1151), within 1.011 at 1280 × 720 and about 1.1 at 1920 × 1080; inside
  the image and at the top and left edges, its light exactly. To measure: the CPU model of
  `tests/bloom.rs` at the size in question. A fix would pad the chain beyond those edges.
- **glTF minification filters are approximated.** `slime-gfx` ties mip blending to the
  minification filter and gives every texture mipmaps, so only `NEAREST_MIPMAP_NEAREST` and
  `LINEAR_MIPMAP_LINEAR` are exact (documented in the importer, `crates/slime-cook`). Revisit when
  an engine-side user needs a separate mipmap filter or a base-level-only read.
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
  the next more than the code changes between them. At 0.1 (M3b.9, 2026-10-14): Linux 5 minutes 40
  seconds, Windows 11 minutes 32 seconds, of which the tests take 4 minutes 31 seconds and the
  release build of the examples 3 minutes 24 seconds. The owner decided then to keep CI as it is,
  the repository being public and its minutes free, and to reconsider past 20 minutes.
