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

- Stage 5b — Scenes and prefabs: done and released as 0.4 on 2026-10-18
  ([ADR-0089](adr/0089-scenes-and-prefabs.md) to [ADR-0095](adr/0095-cooked-scene-files.md)).
  Worlds are written in files, scene files that place prefabs with overrides, cooked into a binary
  encoding of the engine's own; *Hunna Wood*'s zone is made of prefabs, on Linux and on Windows.
- The audit pass and the foundations pass before Stage 6: done on 2026-10-20
  ([ADR-0096](adr/0096-the-files-the-engine-reads.md),
  [ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)). An audit of 0.4
  found defects that lose or change a player's saves, files that crash the engine, a recovery from a
  lost GPU device that covers one case, and quick wins that it measured. P1
  ([ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md)), P2
  ([ADR-0102](adr/0102-a-damaged-file-gives-an-error.md)), P3
  ([ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)), P5
  ([ADR-0104](adr/0104-the-build-ci-and-measurements.md),
  [ADR-0105](adr/0105-egui-fonts-ship-with-the-dev-overlay.md)) and P4
  ([ADR-0106](adr/0106-the-measured-quick-wins.md)) fixed them, and M6a.2 B4-F01, which P4 left.
  P6 decided as ADRs what the editor will freeze: the execution model
  ([ADR-0107](adr/0107-the-execution-model.md)), what the world knows of its changes
  ([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md)), stable references and undo
  ([ADR-0109](adr/0109-stable-references-and-undo.md)), how the editor runs a game's code
  ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md)), the frame contract
  ([ADR-0111](adr/0111-the-frame-contract.md)), levels, saves and worlds for open worlds
  ([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md)), asset requests
  ([ADR-0113](adr/0113-asset-requests-residency-and-priorities.md)), hardware tiers
  ([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)), and what the GPU layer gives the
  editor ([ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)). The passes' checks by hand
  wait for the owner ([the list](#checks-by-hand-waiting-for-the-owner)); what both deliver ships
  with 0.5.
- The games the engine aims at, described on 2026-10-19
  ([ADR-0098](adr/0098-the-games-the-engine-aims-at.md)): nine profiles, from *Hunna Wood* to a
  massively multiplayer game, listed in [`ROADMAP.md`](ROADMAP.md). They add criteria to P6, one UI
  framework of the engine's own for the editor and games, a stage of native GPU backends (9c), and
  needs to Stages 7, 9a, 9b, 10a and 10b. [ADR-0099](adr/0099-ai-agents-drive-the-engine.md):
  AI agents drive the engine through an MCP server of its own, from Stage 6, each later stage
  exposing its systems to it. [ADR-0100](adr/0100-updates-of-shipped-games.md): updates of shipped
  games at Stages 9b and 10b, and a shipped game's saves that load in all its later versions.
- Stage 6a — What the editor stands on: planned on 2026-10-20 and validated by the owner
  ([ADR-0116](adr/0116-stage-6-in-two-halves.md)), to be released as 0.5 at its close, with both
  passes. Stage 6 is split in two halves: Stage 6b, the editor, is planned in detail at Stage 6a's
  close and released as 0.6. M6a.1, frustum culling and instancing, is done
  ([ADR-0117](adr/0117-frustum-culling-and-instancing.md)), and so is M6a.2, the engine's task
  pool and the copies to the GPU within a budget
  ([ADR-0118](adr/0118-the-task-pool-and-the-copies-to-the-gpu.md)). Next: M6a.3.

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M6a.1](#m6a1--frustum-culling-and-instancing) | What the camera does not see is not drawn, and the instances of one mesh and one material are drawn together | Done |
| [M6a.2](#m6a2--the-engines-task-pool-and-copies-to-the-gpu-within-a-budget) | A load never slows the frames, and the engine's compute work runs on one task pool | Done |
| [M6a.3](#m6a3--what-the-world-knows-of-its-changes) | Each reader of the world asks what changed since it last read; components clone | Planned |
| [M6a.4](#m6a4--a-frame-that-owns-what-it-draws) | A frame owns what it draws, and loading needs no renderer | Planned |
| [M6a.5](#m6a5--the-phases-of-a-frame-game-time-and-a-game-as-a-simulation-and-a-presentation) | Each phase of a frame says who may change the world; game time pauses and steps; a game's rules run without a GPU | Planned |
| [M6a.6](#m6a6--the-development-channel-and-the-mcp-server) | An agent drives a running game without a person | Planned |
| [M6a.7](#m6a7--identities-and-saves-per-scene) | A save holds what was changed and costs what changed; an entity outlives its level | Planned |
| [M6a.8](#m6a8--views-apart-from-the-renderer) | One renderer draws several views, each with its own settings | Planned |
| [M6a.9](#m6a9--asset-requests) | What a view lacks loads first, and assets stay within a budget of memory | Planned |
| [M6a.10](#m6a10--rendering-for-the-editor) | The renderer draws what an editor shows, and says what is under the cursor | Planned |
| [M6a.11](#m6a11--windows-check-documentation-release-05) | Stage 6a is done and released as 0.5, with the audit pass and the foundations pass | Planned |
| [P1](#p1--saves-and-identities-that-never-mislead) | A save, a scene file and a level written back hold what the world holds and name the entities they mean; a game resumes only a save it can play, and never loses one | Done |
| [P2](#p2--a-damaged-file-gives-an-error-never-a-crash) | No damaged or edited file makes the engine panic, hang or exhaust its memory (ADR-0096) | Done |
| [P3](#p3--the-loss-of-the-gpu-device-and-the-main-loop) | A lost GPU device is recovered wherever the loss is found, and a panic in `render` stays an ordinary panic | Done |
| [P5](#p5--the-build-ci-and-measurements-tracked-from-one-commit-to-the-next) | Every crate compiled once, CI under its limit, and the engine's measurements tracked from one commit to the next | Done |
| [P4](#p4--the-measured-quick-wins) | The quick wins that the audit measured, each measured again before and after | Done; M6a.2 fixed B4-F01, which it left |
| [P6](#p6--the-foundation-decisions) | What the editor will freeze, decided as ADRs | Done |

Stages 0, 1, 2, 3a, 3b, 3c, 3d, 4a, 4b, 5a and 5b are done; see [Completed
stages](#completed-stages).

## Stage 6a — What the editor stands on

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-6a--what-the-editor-stands-on--05)): *Hunna
Wood* runs on what the editor stands on, on Linux and on Windows. What the camera does not see is
not drawn, and the rest is drawn by instances, measured at the hardware tiers' resolutions against
their budgets ([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)); its zone loads without
a frame missing its time; its game time pauses and steps; an agent drives it through the engine's
MCP server, pausing and stepping it, reading and changing its world, capturing its image and
playing it with scripted input; its saves are differences per scene, which cost what changed. The
`world` example draws a frame in several views and picks the object under the cursor.

This is the first half of Stage 6 ([ADR-0116](adr/0116-stage-6-in-two-halves.md)): what the
editor stands on, built and checked without the editor's interface, on the decisions of the
foundations pass. Where the engine stands at the start:

- **Every instance is drawn, with a draw call of its own**, its values in a slot of 256 bytes of a
  uniform buffer. The shadow passes leave out the instances that a cascade or a spot light does not
  reach; the scene pass draws them all. M6a.1 changed it.
- **Threads started four ways**: `slime-render`'s private pool of up to four threads, which loads;
  a thread per load, which waits for its tasks; a thread per logical core for the sky's
  environment; the cooker's own. The loading tasks submit their copies to the GPU as they read, and
  a load slows every frame (B4-F01, [ADR-0107](adr/0107-the-execution-model.md)). M6a.2 changed it.
- **Only transforms tell that they changed**: every other reader reads the whole world each time
  ([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md)).
- **A frame borrows**: `FrameBuilder::build` and `AssetCache::request` take the `Renderer`, and
  `Frame`, `Scene` and `Instance` borrow what they draw; a renderer draws one output of one size
  ([ADR-0111](adr/0111-the-frame-contract.md)).
- **A game is one block**: every method of `App` receives a `Context` that holds the world, the GPU
  and the window, and *Hunna Wood* changes its world while it draws; one time, without pause or
  step (ADR-0107).
- **Nothing links a running game to a tool**: agents reach it through tests, logs and captures of
  their own ([ADR-0099](adr/0099-ai-agents-drive-the-engine.md),
  [ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md)).
- **A save is the difference from a start world that the game makes again whole**, at each save;
  nothing outlives its scene; *Hunna Wood* puts its fire's intensity back before each save
  (`unflicker`); a created entity written into its level exists twice once carried
  ([ADR-0109](adr/0109-stable-references-and-undo.md),
  [ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md)).
- **The asset cache keeps all it loaded**, until new cooked data or a lost device, and loads
  without priorities or cancellation
  ([ADR-0113](adr/0113-asset-requests-residency-and-priorities.md)).
- **No blending, lines, integer targets or picking**, and the renderer's passes are a closed list
  ([ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)).

Out of scope: Stage 6b's work, the editor's commands beyond the dev overlay's edits, its journal,
undo and redo, the UI framework, the editor's window, viewport, hierarchy, inspector, gizmos and
asset browser, levels of several files, the former identities that the editor writes, `EntityRef`,
play in the editor, a game started again after a change of its code, and *Hunna Wood*'s editor
package; the job system, a render thread, hooks, messages and deferred changes (Stage 7); a copy of
the world and its restore (with their first user); a frame updated by what changed rather than
built again (on measurement, ADR-0111); kinds of material of a game's own, transparency sorted by
depth, compute shaders and indirect draws (Stage 9a); layers that depend on the story (Stage 9b);
textures streamed by their levels of detail, cells and transfer between worlds (Stage 10a).

Decisions taken when the stage was planned, by the owner on 2026-10-20, with the alternatives they
rejected, are in [ADR-0116](adr/0116-stage-6-in-two-halves.md): Stage 6 in two halves, each
released, Stage 6a as 0.5 and Stage 6b as 0.6, every later stage released one version later; the
order below, each milestone after what it needs, the riskier first; the development channel and
the MCP server before play in the editor; identities and saves per scene in Stage 6a; the frame
that owns what it draws before the split of a game; Stage 6b beginning with the editor's core. What
each milestone builds was decided in ADR-0107 to ADR-0115; each milestone that designs a part
records its design in a new ADR, which amends the ADR that decided the part where it changes it.

Risks:

- **The size of the half.** Eleven milestones, most of them large; M6a.7 is split in two within
  Stage 6a if it outgrows one.
- **Every program changes.** M6a.3 gives guards on mutable access, M6a.4 a frame without lifetimes
  and M6a.5 the split of `App`, which its thirteen implementations (eight tests of `slime-app`, four
  examples and *Hunna Wood*), the three examples of the crates' READMEs and the benchmarks follow.
  The order makes each change once.
- **Costs per access.** Ticks and counted references add work to each write and each instance:
  M6a.3 and M6a.4 measure them on the benchmarks of 100 000 entities, before and after.
- **Concurrency.** The task pool and the owner of the copies change how a load meets a lost device:
  P3's scenarios run again at M6a.2.
- **A port that listens.** The development channel is local, guarded by a token and absent from
  shipped builds; M6a.6 extends [ADR-0096](adr/0096-the-files-the-engine-reads.md)'s threat model to
  it.
- **Saves.** M6a.7 changes the layout of saves: the fixture is made again, and whether the saves of
  layouts 2 and 3, those of 0.4 and those written since P1, are still read is decided and tested.
- **One GPU.** Budgets are checked on the owner's RX 6900 XT only ([Known debt](#known-debt)).
- **The cost of the checks.** CI's jobs stay under the owner's 20 minutes
  ([ADR-0104](adr/0104-the-build-ci-and-measurements.md)): each milestone that adds a crate records
  how long they take.

### M6a.1 — Frustum culling and instancing

- **Goal.** What the camera does not see is not drawn, and the instances of one mesh and one
  material are drawn by one draw call: a frame costs what is seen, not the world's size.
- **Findings.** G29: beyond 1 048 576 meshes drawn, `render` panics in wgpu. The known debt: every
  instance drawn with a draw call of its own, its values in a slot of 256 bytes, half of it padding.
- **Crates.** `slime-gfx`, `slime-render`, `slime-benches`.
- **Delivers.** In `slime-gfx`, as [ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)
  decides, amending ADR-0026: a storage buffer, and buffers that declare several uses and are
  written by ranges; meshes that share buffers, an indexed draw from a base vertex; a range of
  instances per draw. In `slime-render`: the instances' values in a storage buffer, read by the
  instance's index; the camera's view culled by each instance's box; each pass's draws grouped by
  pipeline, material and mesh into instanced draws, the shadow passes' included; the instances
  beyond what the device's limits hold left out with a warning. The render benchmarks at the
  hardware tiers' resolutions, 1280 × 800, 1920 × 1080 and 2560 × 1440, and
  [`measurements.md`](measurements.md) comparing each figure with its budget scaled to the RX
  6900 XT (ADR-0114). The ADR of the design; the renderer's rustdoc and README,
  [`benches/README.md`](../benches/README.md), `docs/ARCHITECTURE.md`, the glossary,
  `CHANGELOG.md`; the known debt above retired.
- **Done when.** The image tests pass unchanged, within the tolerances of
  [ADR-0054](adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md); tests: an instance outside
  the view is not drawn, and its shadow still falls in the view; the instances of one mesh and one
  material take one draw call per pipeline; past the device's limits, the rest are left out with a
  warning, without a panic (G29); `render.instances.10k` and `render.unseen.10k` measured before
  and after, on lavapipe and on the RX 6900 XT, and `render.instances.100k` on the RX 6900 XT
  alone, which lavapipe cannot run, all at the tiers' resolutions; CI is green.
- **How to test.** `cargo test -p slime-gfx -p slime-render`; `cargo run --release -p
  slime-benches -- render render.instances.100k`.
- **By hand.** The owner plays the zone by day and at night, and finds it unchanged.
- **Outcome.** Done on 2026-10-20. The owner's check by hand, on Linux: the owner played the zone
  by day and at night on 2026-10-21, from `cargo run --release -p hunnawood -- --new --save <a file
  of its own>`, and found it unchanged.
  [ADR-0117](adr/0117-frustum-culling-and-instancing.md) records the design, and amends ADR-0026,
  ADR-0027, ADR-0106 and ADR-0115: the scene pass culls by each instance's box against the
  camera's view; each pass draws each run of instances of one pipeline, material and mesh with one
  draw call, the shadow passes by mesh alone but for masked materials; the instances' values in a
  storage buffer, read through a list per draw; the meshes in pages shared by the renderer and its
  loads; typed buffers whose uses are those of their type, written by ranges, where "Delivers" asked
  for buffers that declare several uses, which the owner chose on 2026-10-20. The tests:
  `a_square_above_a_floor_darkens_a_square_of_the_floor_blended_over_six_texels`, whose square lies
  outside the view, is not drawn, and still shadows the floor in it;
  `the_instances_of_one_mesh_and_one_material_take_one_draw_call_per_pipeline`;
  `the_instances_drawn_beyond_what_the_values_buffer_holds_are_left_out_of_every_pass` (G29);
  `an_instanced_draw_reads_each_instance_s_values_and_a_mesh_from_its_base_vertex`;
  `meshes_share_a_page_until_it_is_full_and_a_page_is_freed_with_its_last_mesh`. The image tests
  pass unchanged, on lavapipe and on the RX 6900 XT. On the RX 6900 XT
  ([`measurements.md`](measurements.md), sections "M6a.1, before" and "M6a.1", two runs before and
  three after, the machine installing a game), the CPU of `Renderer::render` at the three tiers'
  sizes: `render.instances.10k` 2.0 to 2.6 ms, where it took 8.9 to 17.3 ms; `render.unseen.10k`
  1.3 to 1.4 ms, where it took 14.5 to 23.4 ms; `render.instances.100k` 17 to 21 ms, where it took
  73 to 93 ms. The passes on the GPU, summed: 0.15 to 0.32 ms for 10 000 cubes, where they took
  0.27 to 0.46 ms; 0.26 to 0.50 ms for 100 000, where they took 2.0 to 2.3 ms. On lavapipe, 65 to
  160 ms for `render.instances.10k`, where it took 223 to 346 ms; lavapipe now runs
  `render.instances.100k`, which is no longer run on request only. Within the GPU budgets of every
  tier; the CPU of 100 000 cubes is beyond the main thread's budget at 60 frames a second, which
  the known debt says. `world.frame.100k`, 13.1 to 13.6 ms, still costs less than drawing its
  frame: ADR-0111's condition for building the frame from what changed does not hold yet. Retired
  from the known debt: one draw call per instance, the slots of 256 bytes, and G29.

### M6a.2 — The engine's task pool, and copies to the GPU within a budget

- **Goal.** A load never slows the frames, and the engine's compute work runs on one task pool,
  with a priority for what a frame waits for.
- **Findings.** B4-F01, which P4 left to P6's execution model: during a load, the render CPU went
  from 0.5 to 2.9 ms on the audit's RX 6900 XT, and to 16.6 ms at the 99th percentile with textures
  of 4096² texels; on *Hunna Wood*'s zone, from 0.54 to 0.77 ms at the median and 2.2 ms at the 90th
  percentile ([P4's outcome](#p4--the-measured-quick-wins)).
- **Crates.** A new crate at the bottom of the workspace, on the standard library alone;
  `slime-render`, `slime-world-render`, `slime-cook`, `slime-benches`.
- **Delivers.** The engine's task pool, as [ADR-0107](adr/0107-the-execution-model.md) decides:
  the machine's logical cores but one, at least one; tasks of a *frame* and of a *background*
  priority, a free thread taking a frame task first, background tasks never holding every thread;
  `slime-render`'s private pool, the threads of the sky's environment and the cooker's moved onto
  it, the sky made by frame tasks. Loading tasks write texels and vertices into staging buffers
  reused from one load to the next; one thread of the renderer submits the copies, within a budget
  of bytes per frame set by measurement; a load ends with its last task, without a thread that
  waits for it. A benchmark of a load of textures of 4096², the audit's case. Its design recorded in
  an ADR where it adds to ADR-0107; the figures in the rustdoc; the new crate's README;
  `docs/ARCHITECTURE.md`'s threads and loading, `benches/README.md`, `measurements.md`, the
  glossary, `CHANGELOG.md`; the known debt of B4-F01 retired.
- **Done when.** Tests: a frame task is taken before every background task, and finds a thread
  while background tasks fill the pool; a scene still arrives whole; the cook gives the same bytes;
  the lost-device scenarios of P3 pass; on the RX 6900 XT, while the zone loads and while the
  textures of 4096² load, the render CPU stays near what it is once loaded, and no frame takes more
  than 16.7 ms, the frame of 60 frames a second (ADR-0114); CI is green.
- **How to test.** `cargo test --workspace`; `cargo run --release -p slime-benches -- loading
  loading.zone`; `scripts/check.sh`.
- **By hand.** The owner starts the game, and sees the zone appear as before.
- **Outcome.** Done on 2026-10-21. The owner's check by hand waits
  ([the list](#checks-by-hand-waiting-for-the-owner)).
  [ADR-0118](adr/0118-the-task-pool-and-the-copies-to-the-gpu.md) records the design, and amends
  ADR-0107: the pool, `slime-tasks`, on the standard library alone, its tasks owning what they
  read, without `unsafe` code; the sky's environment and the table of the BRDF computed by frame
  tasks, the cooker's encoding and the loads by background tasks; the loads' tasks two at a time,
  where ADR-0107 gave them every background thread, which let frames take up to 68 ms, while the
  pool's other background work keeps every thread but one; the uploader in `slime-gfx`, one per
  renderer, 8 MiB of copies per frame, renewed by each frame and by 16.7 ms without one, and no
  budget before the first frame nor 100 ms after the last, so that a load that a program waits
  for without drawing is not held back; a load completed by its last task or copy, without a
  thread of its own. The tests: `a_frame_task_finds_a_thread_while_background_tasks_fill_the_pool`
  and `a_free_thread_takes_a_frame_task_before_every_background_task` in `slime-tasks`;
  `two_tasks_of_loads_run_at_once_at_most_completions_first_then_the_earlier_loads`;
  `a_scene_loaded_in_the_background_holds_each_asset_once_where_its_nodes_place_it` and the
  cache's tests, a scene still arriving whole; the uploader's tests in `slime-gfx`'s
  `tests/uploads.rs` and `tests/textures.rs`, among them a device lost while copies wait;
  `a_load_on_a_lost_device_ends_with_the_loss_and_never_hangs`;
  `a_panic_of_a_task_in_its_upload_s_hand_off_is_the_one_raised`; the cook twice from empty
  directories, the same bytes, and P3's scenarios of a lost device, in `scripts/check.sh`. On the
  RX 6900 XT ([`measurements.md`](measurements.md), sections "M6a.2, before" and "M6a.2", two runs
  each): while sixteen textures of 4096² load, the longest frame 7.6 to 10.6 ms, where frames took
  up to 23.9 and 26.2 ms, and the render CPU 455 to 557 µs at the 90th percentile, where it took
  649 to 766 µs, beside 692 to 707 µs once they are loaded; while the zone loads, the longest
  frame 1.3 to 4.3 ms, where it took 6.3 ms, the render CPU 753 to 869 µs at the 90th percentile,
  where it took 1.4 to 1.8 ms, beside 811 to 821 µs once loaded. The meshes come whole sooner: the
  textures' after 208 to 215 ms, where they took 260 to 277 ms, the zone's after 43 to 46 ms,
  where they took 70 to 77 ms, and loaded without a frame drawn, the zone's after 49 to 55 ms,
  where they took 72 to 78 ms. Retired from the known debt: B4-F01; added: what a load reads waits
  in staging memory.

### M6a.3 — What the world knows of its changes

- **Goal.** Each reader of the world asks what was added, changed or removed since it last read,
  rather than reading the whole world; components clone.
- **Findings.** G32, in part: `World::is_hidden` walks the ancestors of what draws, at each frame.
- **Crates.** `slime-world`; `slime-world-render`, `slime-devui`, *Hunna Wood*, the examples and
  the benchmarks, for the guards and `Clone`.
- **Delivers.** As [ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md) decides: on every
  value of a component, the tick at which it was added and the one at which it last changed;
  mutable access through a guard that stamps the value written, and a variant that stamps only a
  value that differs; a summary per storage and per block; a log of removals; a structure counter;
  the marks of stale transforms kept by the same guard; a flag of the entities that an ancestor
  hides, which `World::is_hidden` reads; `Component: Clone`. The world panel makes its list again
  only when the structure counter moves. A benchmark of a reader of 100 000 entities of which 1 %
  changed. The width of a tick and the size of a block set by measurement; its design recorded in
  an ADR where it adds to ADR-0108; the rustdoc, `docs/ARCHITECTURE.md`'s world,
  `benches/README.md`, `measurements.md`, the glossary, `CHANGELOG.md`.
- **Done when.** Tests: each of two readers sees a value written through a guard once, at its own
  pace; the variant does not stamp a value set equal; removed components and destroyed entities are
  in the log, with their ticks; the structure counter moves when an entity is spawned or destroyed,
  or changes parent or name, and only then; a reader skips the blocks that did not change; the
  hidden flag follows `Hidden` and changes of parent; `world.frame.100k`, `world.transforms.100k`
  and a reader of 100 000 entities of which 1 % changed measured before and after; CI is green.
- **How to test.** `cargo test -p slime-world`; `cargo run --release -p slime-benches -- world`.

### M6a.4 — A frame that owns what it draws

- **Goal.** A frame is a value of its own, built from the world, then drawn without it, and loading
  needs no renderer: what views, the editor's previews and one day a render thread need.
- **Findings.** G17, medium: `FrameBuilder::build` borrows the asset cache, the `Frame` that it
  returns borrows its meshes and materials, and `AssetCache::request` borrows the `Renderer`, so
  that no thread can draw one frame while the next is built.
- **Crates.** `slime-render`, `slime-world-render`, `slime-gfx`; the examples, *Hunna Wood* and the
  benchmarks.
- **Delivers.** As [ADR-0111](adr/0111-the-frame-contract.md) decides: `Frame`, `Scene` and
  `Instance` without lifetimes, their meshes and materials held by counted references, the frame
  holding its lights and its environment; an asset cache that keeps what it needs of the renderer,
  `AssetCache::request` and `FrameBuilder::build` taking no `Renderer`; a material of a kind
  registered with the renderer, which gives its shaders, its parameters described by reflection and
  the passes it draws in, the physically based kind alone. As
  [ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md) decides: the device asks for the
  adapter's limits, above a floor written in `slime-gfx` and checked when a device opens, below
  which it refuses with an error that names the limit and the value needed; what CI's WARP offers,
  read from its job's log. The ADR of the design; the rustdoc, the READMEs, the renderer's rule on
  what it keeps among them, `docs/ARCHITECTURE.md`'s frame, `measurements.md`, the glossary,
  `CHANGELOG.md`; G17 retired.
- **Done when.** A frame built on one thread is drawn on another, `Send` and `'static` as the
  `threads` modules check; the image tests pass unchanged; a cache loads and keeps meshes without a
  renderer; the physically based kind draws as the material did; a floor above what an adapter
  offers refuses it, naming the limit; lavapipe and WARP open above the floor in CI; the cost of the
  counted references measured on `world.frame.100k`, and on `render.instances.100k` on the RX
  6900 XT; CI is green.
- **How to test.** `cargo test --workspace`; `cargo run --release -p slime-benches -- world render
  render.instances.100k`.

### M6a.5 — The phases of a frame, game time, and a game as a simulation and a presentation

- **Goal.** Each phase of a frame says who may change the world; game time pauses, slows down and
  steps; a game's rules run without a GPU or a window, as tests, the editor's child process and one
  day a server run them.
- **Crates.** `slime-app`, `slime-world`, `slime-devui`, `slime-platform` if the keys held need it;
  the examples, *Hunna Wood* and the benchmarks.
- **Delivers.** As [ADR-0107](adr/0107-the-execution-model.md) decides: `App` giving way to a
  simulation and a presentation, run by a host, `run` the host of a game's program; the six phases
  of a frame, extraction and rendering given the world as `&World` or not at all, in the two calls
  of [ADR-0111](adr/0111-the-frame-contract.md); a change asked outside the first three phases
  queued as a command and applied at the next frame's start, the first command a value set at a
  property path of a component, on an entity named by its persistent identity
  ([ADR-0109](adr/0109-stable-references-and-undo.md)), which the dev overlay's world panel sends;
  the input given as data, the keys and buttons held kept by the engine
  ([ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md)); real time and game time,
  with a speed, a pause and one step, a fixed step of the same duration at every speed. *Hunna
  Wood* becomes a library and its program ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md),
  amending [ADR-0055](adr/0055-games-in-the-workspace.md)): its rules a simulation whose fixed step
  no longer waits for its view, game time kept paused by its program until the zone is drawn; its
  fire's flicker out of `render`; its camera in the presentation; its overlay's edits commands, and
  its cooked data that changed applied at the start of a frame; keys of its overlay that pause and
  step. The examples, and those of the READMEs, moved. The ADR of the design; the rustdoc,
  `slime-app`'s README, `docs/ARCHITECTURE.md`'s flows, the glossary, `CHANGELOG.md`, the game's
  README.
- **Done when.** Tests, headless and in Weston: the phases run in their order, and a presentation
  that writes the world does not compile (`compile_fail`); at a speed of a quarter, a step runs at
  every fourth frame at 60 Hz and the presentation interpolates between steps; paused, no step
  runs; one step runs exactly one; a command sent during a frame is applied at the next one's start;
  *Hunna Wood*'s rules run through its library for hundreds of steps without a GPU; the lifecycle
  and lost-device scenarios pass; CI is green.
- **How to test.** `cargo test --workspace`; `scripts/check.sh`.
- **By hand.** The owner plays the zone, pauses and steps it from the overlay, and finds it
  otherwise unchanged.

### M6a.6 — The development channel and the MCP server

- **Goal.** An agent, or a test, drives a running game without a person: it pauses and steps it,
  reads and changes its world, captures its image, plays it with scripted input, reads its logs and
  measurements. Each later milestone exposes its system to it.
- **Crates.** `slime-app`, whose host listens; `slime-gfx` or `slime-render`, for captures of the
  game's image, which a window's image does not allow today
  ([ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)); the MCP server, a program of the
  workspace; *Hunna Wood* and the examples.
- **Delivers.** As [ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md) decides: a game started
  with a secret token in its environment listens on `127.0.0.1`, on a port that the system
  chooses, made known to whoever started it, and accepts a connection only with that token; started
  without one, it does not listen. Messages in the binary encoding, behind a version of the protocol
  that both ends check: the host's controls, pause, speed, one step and stop; commands; questions on
  the world, through reflection; captures of its image; scripted input; and from the game, its
  state, its logs and its measurements. How the token is drawn from the system's randomness, and
  whether that needs a dependency; how a build is made for development or for shipping, the channel
  and the server left out of the latter. As
  [ADR-0099](adr/0099-ai-agents-drive-the-engine.md) decides: the engine's MCP server, which starts
  a game with a token, and gives an agent these operations, cooking, the tests, and reloading what
  hot reload reloads; its dependency asked for, with the alternatives considered. ADR-0096's threat
  model extended to the channel and the server; the ADR of the design; the server's README,
  `AGENTS.md`'s and `CONTRIBUTING.md`'s commands, `docs/ARCHITECTURE.md`,
  [`dependencies.md`](dependencies.md), the glossary, `CHANGELOG.md`.
- **Done when.** Tests start *Hunna Wood* with a token, in Weston on lavapipe, connect to it, pause
  it, step it, read the hero's transform by its identity, change a value, walk the hero with
  scripted input and capture its image; a connection without the token, or of another version of
  the protocol, is refused with a message; a game started without a token opens no port; a build for
  shipping holds neither the channel nor the server; a test drives the game through the server's
  tools; the agent that builds the milestone checks a change of the game on screen through the
  server; CI is green.
- **How to test.** `cargo test --workspace`; `scripts/check.sh`.
- **By hand.** In the Windows virtual machine: the system's firewall asks nothing when the game
  listens.

### M6a.7 — Identities and saves per scene

- **Goal.** A save, and a level written back, hold what an author or a player changed, no more; a
  save costs what changed; an entity outlives its level.
- **Findings.** The known debt: a created entity written into its level exists twice once carried,
  which the audit's test showed again on 2026-10-20; what a start creates without a key can take
  another's saved changes; a key under which the world remembers a scene of which nothing lives;
  P1's way round G02, a level that keeps something of the one before it under a key of its own.
- **Crates.** `slime-world`, `slime-serialize`; *Hunna Wood*, the `world` example, the benchmarks
  and the MCP server.
- **Delivers.** As [ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md) decides: the mark of
  driven components, which saves, the overrides of prefabs and levels written back leave as their
  author wrote them, in the place of *Hunna Wood*'s `unflicker`. As
  [ADR-0109](adr/0109-stable-references-and-undo.md) decides: what a start creates in code holds a
  key or is `Unsaved`, else saving refuses, naming the entities; a created entity that a scene
  written back holds is carried once, known by the ID that the world gave it. As
  [ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md) decides: saves made of differences
  per scene, each scene compared with its base, what it spawns and what the game adds when it spawns
  it, without a start world made again; the differences of a scene unloaded kept, and applied again
  when it loads; what persists, what resets and what is never saved, chosen per scene and per
  entity; adoption, an entity taken out of its scene under a key of its own, its former identity
  written, so that `World::spawn_scene` no longer refuses a level because something of the one
  before it lives; a run carried through hot reload the same way; the cost of a save following the
  ticks of M6a.3, and a benchmark of a save of 100 000 entities of which 1 % changed. A new layout
  of saves, with whether the saves of layouts 2 and 3, those of 0.4 and those written since P1, are
  still read decided, and *Hunna Wood*'s fixture made again
  ([ADR-0100](adr/0100-updates-of-shipped-games.md)). Saving and loading a running game exposed to
  the MCP server. The ADR of the design; the rustdoc, `docs/ARCHITECTURE.md`'s saves,
  `benches/README.md`, `measurements.md`, the glossary, `CHANGELOG.md`, the game's README; the
  known debt above retired. If it outgrows a milestone, it is split in two within Stage 6a: first
  the driven mark, the keys and the carried entity, then saves per scene and adoption.
- **Done when.** The audit's test of the torch, moved into the repository, passes; tests: a driven
  component is saved and written back as its author wrote it, and computed again after a load; a
  start that creates an entity without a key refuses to save, naming it; a save of 100 000 entities
  of which 1 % changed costs what changed, measured; a scene unloaded, then loaded again, gets back
  its differences; a scene reset by a signal of the game forgets them, a persistent one keeps them;
  an adopted entity outlives the level that replaced its own, with its references, through a save
  and a load; hot reload carries a run as before; a test saves and loads a running game through
  the MCP server; *Hunna Wood*'s tests pass on the zone and its fixture resumes; CI is green.
- **How to test.** `cargo test -p slime-world -p slime-serialize`; `cargo test -p hunnawood --
  --ignored`.
- **By hand.** The owner plays: F5, F9, quit and resume.

### M6a.8 — Views apart from the renderer

- **Goal.** One renderer draws several views, each with its own camera, size and settings, and
  makes once what views share; a view resized frees its old targets before it makes new ones.
- **Findings.** G18: a resize keeps the targets of the old size while it creates the new ones, and
  after `GfxError::OutOfMemory` keeps targets that no longer fit. G37: if creating the spot lights'
  shadow atlas runs out of memory, the sun loses its shadows without a word. G47: a multisampled
  output target makes `render` panic inside wgpu. The known debt: a renderer whose frame changes
  shape reports no tone-mapping or bloom time, and one renderer per output size.
- **Crates.** `slime-render`, `slime-world-render`; the examples, *Hunna Wood*, the benchmarks and
  the MCP server.
- **Delivers.** As [ADR-0111](adr/0111-the-frame-contract.md) decides: what views share made once,
  the pipelines, the spot lights' shadows, the environment and the BRDF table; for each view, its
  camera, size, settings, targets, timers and history; a part of the render graph per view; the
  sun's cascade atlases from a pool that views drawn in turn share; a view's settings, a
  perspective or orthographic projection, an offset of its projection, its render scale, its
  anti-aliasing, a mode of debugging that shows normals or shadows, the passes it runs; several
  views per frame; a view's targets made again once its size holds still, the old ones freed
  first. The `world` example draws a second view, orthographic, from above. Any view's image
  captured through the MCP server. The ADR of the design; the rustdoc, `docs/ARCHITECTURE.md`'s
  rendering, the glossary, `CHANGELOG.md`; the known debt above retired.
- **Done when.** Image tests: each of two views of a frame gives the image that it gives drawn
  alone; an orthographic view; a render scale of one half; the two modes of debugging; two halves of
  an image drawn with offsets give the whole; four views of 1280 × 720 with the sun measured against
  the 508 MiB of four renderers today; a view resized never holds both sizes' targets at once, and a
  size that does not fit leaves the view without targets, with an error, rather than with old ones;
  the tests that confirmed G18, G37 and G47 pass; a test captures each of two views through the
  MCP server; CI is green.
- **How to test.** `cargo test -p slime-render -p slime-world-render`; `cargo run --example world`.
- **By hand.** The owner looks at the `world` example's two views.

### M6a.9 — Asset requests

- **Goal.** What a view lacks loads before what is fetched ahead, a load that nothing needs any more
  stops, and the memory that assets take stays within a budget.
- **Findings.** G16, in part: one unreadable file hides its whole batch of meshes, until the cooked
  data changes. Trying a passing error again, such as GPU memory running out, is Stage 6b's. Since
  M6a.1, meshes share pages, and dropping one frees no GPU memory until its page empties
  ([ADR-0117](adr/0117-frustum-culling-and-instancing.md)): how the budgets count meshes is this
  milestone's to decide.
- **Crates.** `slime-render`, `slime-world-render`; *Hunna Wood*, the examples, the benchmarks and
  the MCP server.
- **Delivers.** As [ADR-0113](adr/0113-asset-requests-residency-and-priorities.md) decides:
  requests that carry a priority, *now*, *soon* or *later*, and return a request handle, the assets
  asked for together arriving together; an asset kept, or kept loading, while a handle holds it;
  the last handle dropped cancels a load or makes the asset releasable; a priority changed while
  the handle lives; the bridge from the world to the renderer, `slime-world-render`, holding
  handles for what each view draws; the owner of the copies to
  the GPU serving the classes in their order, within its budget; releasable assets ranked by their
  last use, and dropped least recently used first past a budget per kind, textures and meshes, by
  default the minimum hardware tier's ([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)),
  never one that a handle holds, with a report when that is not enough; what the engine holds
  counted from the sizes of what it creates; a load that fails reported by its handle, without
  keeping the other assets of its request from arriving; new cooked data and a lost device keeping
  handles valid. The cache's requests and memory read through the MCP server. The ADR of the
  design; the rustdoc, `docs/ARCHITECTURE.md`'s loading, the glossary, `CHANGELOG.md`.
- **Done when.** Tests: a request's assets stay while a handle holds them; dropping the last handle
  cancels a load in progress and makes a loaded asset releasable; requests of the three classes are
  served in their order, and one raised from *soon* to *now* moves ahead; past its budget, the cache
  drops the releasable assets least recently used and never a held one, and reports when it cannot
  drop enough; an unreadable mesh no longer hides the others of its request (G16); handles stay
  valid through new cooked data and a lost device; a long session of requests and releases keeps
  the memory within its budget; *Hunna Wood*'s hero arrives before its zone; a test reads the
  cache's requests and memory through the MCP server; CI is green.
- **How to test.** `cargo test -p slime-render -p slime-world-render`; `cargo run --release -p
  slime-benches -- loading loading.zone`.

### M6a.10 — Rendering for the editor

- **Goal.** The renderer draws what an editor shows, lines, blended overlays and unlit icons, and
  says which object is under the cursor.
- **Crates.** `slime-gfx`, `slime-render`, `slime-world-render`; the examples and the MCP server.
- **Delivers.** As [ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md) decides, in
  `slime-gfx`: blending, lines, a render target of integers, a target read back without waiting, a
  part of a texture written, a pass that resolves a depth of four samples into one, amending
  ADR-0059. In `slime-render`: the unlit kind of material, the second kind; two points of each
  view's graph where a client registers its draws, in the scene pass after the opaque instances,
  and after tone mapping, amending ADR-0041. Picking by the GPU: an image of the instances'
  numbers, at one sample per pixel, read back a frame or two later without holding up a frame,
  given up without a panic on a lost device, each number mapped by the bridge to the persistent
  identity of the entity drawn. Picking by the CPU: a ray against the instances' bounds, then against the triangles of the
  meshes that it meets, kept in the CPU's memory for it. A view's image sampled by another pipeline
  through `slime-gfx`, which Stage 6b's interface shows. The `world` example draws a grid at the
  first point, and the box of what is clicked at the second. The entity under a pixel of a view
  named through the MCP server. The ADR of the design; the rustdoc, `docs/ARCHITECTURE.md`, the
  glossary, `CHANGELOG.md`.
- **Done when.** Image tests: lines; blending; an unlit material; a grid, drawn at the first point,
  hidden by the objects in front of it; lines drawn at the second point over everything; picking by
  the GPU and by the CPU name the same entity under a set of pixels of a scene without masked
  materials, and the GPU's passes between the cut-out leaves of a masked one to what is behind; a
  readback never holds up a frame; a device lost while picking gives no answer and no panic; a
  test names the entity under a pixel through the MCP server; CI is green.
- **How to test.** `cargo test -p slime-gfx -p slime-render -p slime-world-render`; `cargo run
  --example world`.
- **By hand.** The owner clicks objects in the `world` example.

### M6a.11 — Windows check, documentation, release 0.5

- **Goal.** Stage 6a is done, checked on Windows, and released as 0.5 with the audit pass and the
  foundations pass.
- **Delivers.** The owner's check, in the Windows virtual machine, of the programs that CI builds,
  with the checks by hand that wait in [the list](#checks-by-hand-waiting-for-the-owner) if the
  owner makes them then; the documents reread as a newcomer would read them; `CHANGELOG.md`'s
  section of 0.5; the release, as [Releasing](#releasing) says; Stage 6b planned with the owner.
- **Done when.** The owner played *Hunna Wood* and ran the `world` example on Windows; the documents
  are reread and corrected; CI is green on the release commit; the tag and the GitHub release exist,
  with the Windows programs.
- **By hand.** In the Windows virtual machine: *Hunna Wood* played, paused and stepped, saved and
  resumed; the `world` example's views and picking; the firewall, when the game listens (M6a.6).

## Stage 6b — The editor

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-6b--the-editor--06)): a level of the game is
built, saved and reloaded entirely in the editor, with undo/redo on every operation; the editor uses
no private engine API. Released as 0.6 ([ADR-0116](adr/0116-stage-6-in-two-halves.md)).

The stage is planned in detail with the owner at Stage 6a's close, with a study of the UI
framework's design ([ADR-0098](adr/0098-the-games-the-engine-aims-at.md)), and split again then if
it needs to be. Its milestones, in an order provisional until then:

1. **The editor's core, without its interface**
   ([ADR-0109](adr/0109-stable-references-and-undo.md)): the editor's commands, which name entities
   by their persistent identities, an entity created under a given identity and at a given rank
   among its siblings, a deletion undone by creating its subtree again, the journal, undo and redo,
   the selection, a level opened and written back; the MCP server acting through the same commands
   ([ADR-0099](adr/0099-ai-agents-drive-the-engine.md)).
2. **The engine's UI framework**, in two or three milestones (ADR-0098): its design and its
   dependencies, decided then; text in several writing systems, layout, widgets, input routing,
   focus, drag and drop, theming, docking, lists that lay out only what shows; what the platform
   lacks for an editor, the clipboard, input methods and cursors among it
   ([ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)'s list).
3. **The editor's window and its viewport**: *Hunna Wood*'s editor, the package
   `games/hunnawood-editor` ([ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md)); a level
   opened; a view in a panel; its camera; the grid and the icons of lights; a click that selects,
   and the selection outlined; G34, with the editor's model of windows.
4. **Play in the editor** (ADR-0110): the game started in a child process on the level being
   edited, with a save of its own, in its own window; pause, speed, one step and stop; the game
   started again where it was after a change of its code; the world updated in place when a prefab
   changes while the game runs, keeping the handles that its code holds
   ([ADR-0089](adr/0089-scenes-and-prefabs.md)); the three seconds from Play to control, measured.
5. **The hierarchy and the inspector**: a tree that lays out only what shows (G46), reparenting by
   drag and drop, an inspector driven by reflection, which changes a value only when the user
   changed it (G33), overrides marked, applied and reverted; `EntityRef`, a reference to an entity
   of any scene, and its picker ([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md));
   the rest of G32; what becomes of the dev overlay's world panel
   ([ADR-0081](adr/0081-stage-5a-the-world.md)).
6. **Gizmos and placing**: moving, turning and scaling a selection; creating, duplicating and
   deleting.
7. **Levels of several files** (ADR-0112): parts of a level opened alone, former identities
   written when an entity moves out of its file, a prefab made of a selection among them
   ([ADR-0092](adr/0092-a-world-written-back-as-a-scene.md)); a valid change of a prefab that makes
   a level placing it refuse, checked before saving; whether a level that cannot spawn opens
   leniently, to be repaired (ADR-0089); G31, G40, G42, G43 and G45; whether a file says the parents
   and the order of a placement's entities, and keeps the comments of a file written by hand
   ([Known debt](#known-debt)).
8. **The asset browser**: the cooker's index of assets, thumbnails, assets dragged into the view,
   cooking from the editor; a load that a passing error failed tried again (G16); G20, G23, G49 to
   G53 and G56.
9. **Windows check, documentation, release 0.6.**

## The audit pass and the foundations pass

On 2026-10-18 and 2026-10-19, before Stage 6, the engine was audited at 0.4 (`v0.4.0`), without
changing it, in three parts: correctness and robustness, each defect confirmed by a test that
fails; performance measured against budgets, on the owner's RX 6900 XT (RADV), on lavapipe and on
*Hunna Wood*'s zone; architecture, against the games the engine aims at.
[ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md) sums up what it found
and records the owner's decisions; [ADR-0096](adr/0096-the-files-the-engine-reads.md) is the
threat model that ranks its defects: critical, high, medium or low.

The audit numbered its findings, and this plan keeps the numbers so that a fix can name what it
fixes: G01 to G56 are groups of defects, ranked on ADR-0096's scale; B1 to B4 are performance (the
build, the world, the renderer, loading); C1 to C7 are architecture. Each finding is described here
in full. The audit's report and its tests are kept outside the repository: each fix starts by
moving into the repository the test that confirmed its defect, failing, then makes it pass.

The milestones run in the order of the status table: P5 comes before P4, which measures each gain
with P5's benchmarks. Each follows the definition of done, its review included, and starts on the
owner's go. Neither pass has a release of its own: what they deliver ships with 0.5, Stage 6a's
release ([ADR-0116](adr/0116-stage-6-in-two-halves.md)).

The audit left unfinished: the measurements of five quick wins, which P4 made; a run of an hour,
how memory, open files and frame times move over a long game; and every check on Windows, which the
milestones below give to the owner's virtual machine where they touch it. Not covered by the audit
at all: NVIDIA and Intel GPUs (see [Known debt](#known-debt)), X11, a game outside the workspace,
the delay from a key to the image shown, and determinism between Linux and Windows.

Out of scope: the editor's features; frustum culling and instancing, which begin Stage 6
(ADR-0097); what the audit left to later stages, which ADR-0097 lists; the defects that neither
pass fixes, which the [known debt](#left-by-the-audit-of-04) records with the stage each waits for.

Risks:

- **The cost of the checks.** CI's Windows job took 19 minutes 48 seconds at M5b.6, of the 20 that
  the owner allows, and P1 to P3 add tests. The first milestone that takes CI past 20 minutes
  brings forward P5's two changes that shorten it, a cache of the cooked zone and every crate
  compiled once.
- **The identities of P1.** A stable identity for what a game creates in code changes what saves
  and hot reload name, as the editor's undo will: P1 keeps the saves of 0.4 loading, or sets them
  aside whole, and P6 builds undo's references on it.
- **Fuzzing in CI.** Tests that mutate valid files at random must stay within a time budget, and
  reproduce a failure from its seed.

### Checks by hand waiting for the owner

The checks by hand of the milestones done that the owner has not made yet. The owner decided on
2026-10-20 to make them later, together: each milestone adds its own here when it is done, and a
check leaves the list once made, its result written in its milestone's outcome. The game is run
with `--new --save <a file of its own>`, so that the owner's save is left alone.

- **P2, in the Windows virtual machine.** `scene.exe` of CI's artifact, on a copy of its `assets`
  directory whose first mesh has one byte changed: an error that names the file, then status 1.
- **G20, in the Windows virtual machine.** A source asset whose `.meta` differs from its own name
  by case only, cooked twice: whether its ID changes at each cook, and whether `slime-cook watch`
  cooks it again without end (see [Known debt](#left-by-the-audit-of-04)).
- **P3, in the Windows virtual machine.** A timeout detection and recovery (TDR) forced while the
  zone loads, then a second while the game makes its GPU objects again: a new device each time,
  the log saying "the GPU device was lost; a new one replaces it", and the game going on. The
  windows of `hunnawood.exe` and `scene.exe` resized several times, maximized and restored: no
  "ResizeBuffers failed", "cannot be configured" or "surface was lost" in the log, which only
  Direct3D 12 can show. Optionally, the dev overlay's button that loses the device, four times
  within five minutes: three new devices, then the game stops with an error of a lost device.
- **P5, in the Windows virtual machine.** The benchmarks on Direct3D 12, `cargo run --release -p
  slime-benches -- render world loading`; the figures of WARP that the Windows job's log of
  `af512af` prints.
- **M6a.2, on Linux.** The game started with `cargo run --release -p hunnawood -- --new --save <a
  file of its own>`: the zone appears as before.

### P1 — Saves and identities that never mislead

- **Goal.** A save, a scene file and a level written back hold what the world holds, and are applied
  to the entities they name; a game resumes only a save it can play, and never loses one.
- **Findings.**
  - G01, critical. An entity that code creates (`World::spawn`) is named `created/<n>`, its rank of
    creation since the world began, and a save names what it changed by those numbers. When a later
    version of a game creates one more entity in its start world, or creates them in another order,
    or a level edited and carried through hot reload (`World::carry`) adds one, every number
    shifts: a save moves or destroys another entity, without a word, or is refused whole
    (`IdentityInUse`), which *Hunna Wood* takes for a save of another layout and sets aside.
  - G02, critical. After a run changes level by spawning another scene under the same key
    ([ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md)), loading its save, or carrying it
    through hot reload, destroys what of the old level survived in the run, a companion taken along
    or the hero placed in the level; after two changes of level, the save is refused whole.
  - G03, critical. *Hunna Wood* resumes a save it has not checked: it keeps handles to entities of
    its start world that loading may have destroyed, does not check that the loaded world can still
    be played, and sets a save aside only when loading returns an error. Saves edited by hand made
    it panic at its first step, at every launch, or refuse to start. It renames whatever is at the
    save's path, a directory included, and reads it whole before checking its size.
  - G08, high. The derive writes each bound of a range as an `f64`, whatever the field's type, and
    checks compare the value widened to `f64`: `0.7` in an `f32` field, `0.699999988` once widened,
    is refused by a range that starts at 0.7, so that a save, a scene file or a level written back
    fails whole; an `i64` beyond 2⁵³ passes a maximum it exceeds.
  - G09, high. An alias equal to the name, or an alias, of another field or variant is accepted,
    and a value reads back as another without a word: a field renamed, a save read and written
    again, and `max_health` went from 100 to 40.
  - G10, high. `World::write_scene` leaves out, without reporting it, what a scene file itself
    marks `Unsaved`, smoke or an effect made again at each run: a level written back loses it.
  - G11, high. *Hunna Wood* resumes a save only when the fingerprint of its zone's layout is the
    same, a hash of 506 numbers rounded on a grid. Seven of them lie within one ulp of the grid's
    boundaries, so that a change of one ulp, which an update that does not touch the zone can make,
    sets every save aside.
  - G24, medium. `World::save` compares a reference between entities by the bits of its handle in
    each world: when the start world was made in another order than the run's, a reference that the
    run changed is not saved.
  - G35, medium. *Hunna Wood* sets a save aside under a fixed name, replacing the one set aside
    before; and it resumes a save of a later version that holds a component it does not know, then
    writes it back without it.
  - G36, medium. Nothing keeps two copies of the game from sharing a save, and every writer goes
    through the same temporary file: the last to quit overwrites the other's run, and two saves at
    once leave a file mixed from both.
  - G39, low. A section of a save misspelled, or of the wrong kind, is ignored without a word.
- **Crates.** `slime-world`, `slime-reflect`, `slime-reflect-derive`, `slime-serialize`,
  `slime-devui` (the world panel's ranges), *Hunna Wood*.
- **Delivers.**
  - An identity that a game chooses for what it creates in code while it makes its start world, as
    a scene's key, so that saves and hot reload name it whatever the order; `created/<n>` kept for
    what the run creates, numbered against the start world when a save loads.
  - A change of level that keeps what survived it; references compared by the identity they name.
  - Aliases that collide refused when a type is derived or registered; a range checked in its
    field's own type, by one function that the reader, the writer, the registry and the world
    panel share.
  - `World::write_scene` writing what a file marks `Unsaved`, and leaving out, reported, only what
    the run created; a save's unknown or malformed section reported.
  - *Hunna Wood*: a save checked before it is resumed; a save of a later version, or one that loaded
    with warnings, set aside rather than written back; saves set aside under names of their own,
    never replacing an earlier one; its zone's layout compared within a tolerance rather than by a
    hash; one copy of the game at a time writing the save, each writer through a temporary file of
    its own.
  - [ADR-0080](adr/0080-reflection-and-serialization.md) (aliases),
    [ADR-0085](adr/0085-reflection.md) (ranges), [ADR-0087](adr/0087-the-worlds-documents.md),
    [ADR-0088](adr/0088-saving-and-resuming-a-run.md),
    [ADR-0092](adr/0092-a-world-written-back-as-a-scene.md) and ADR-0093 amended; the rustdoc of
    `World::spawn`, `World::load` and `World::write_scene`; the game's README; `CHANGELOG.md`.
- **Done when.** The tests that confirmed each finding, moved into the repository, pass; a save of
  0.4, the game's fixture among them, loads as before or is set aside whole, never applied in part;
  the game's tests pass on the zone; CI is green.
- **How to test.** `cargo test -p slime-world -p slime-reflect -p slime-serialize -p slime-devui`;
  `cargo test -p hunnawood -- --ignored`; `cargo run -p hunnawood -- --new --save <file>`.
- **By hand.** On Linux: play, F5, F9, quit and resume. In the Windows virtual machine: two copies
  of the game at once; the game killed during F5, then started again.
- **Outcome.** Done on 2026-10-19.
  [ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md) records how each finding is
  fixed, and amends the ADRs above. Each defect is confirmed by a test that failed before its fix
  and passes with it; the audit's own tests of G03, G11, G35 and G36 were not available, and
  theirs were written from the findings above. The tests, by finding:
  - G09: `an_alias_that_another_field_has_is_a_bug_in_the_type` and four cases of
    `a_description_that_is_not_one_is_refused` (`slime-reflect`), and two `compile_fail` doc tests.
  - G08: `a_number_is_compared_with_the_bounds_in_its_own_type` and
    `a_bound_given_as_a_constant_is_compared_exactly` (`slime-reflect`), with a `compile_fail` doc
    test of a constant bound; `numbers_are_checked_against_their_range_in_their_own_type`
    (`slime-serialize`); `a_value_at_its_field_s_bound_is_saved_spawned_and_written_back`
    (`slime-world`).
  - G39: `a_section_that_a_save_does_not_hold_is_a_warning_and_one_of_the_wrong_kind_refuses_it`.
  - G24: `a_reference_that_the_run_changed_is_saved_whatever_order_the_start_was_made_in`.
  - G10: `a_level_written_back_unchanged_keeps_what_its_file_marks_unsaved` and
    `unsaved_is_written_but_what_was_created_with_it_and_transient_fields_are_left_out`.
  - G01: `an_update_that_creates_one_more_entity_at_the_start_never_moves_another`,
    `an_entity_that_a_run_creates_under_a_key_is_saved_and_loads_back`,
    `a_hot_reload_after_a_level_gains_a_marker_moves_only_the_enemy_that_the_run_moved`,
    `what_a_run_changed_of_the_start_made_in_code_loads_back_into_the_same_start` and
    `a_save_that_gives_a_key_to_a_scene_and_to_an_entity_it_creates_is_refused`.
  - G02: `a_level_spawned_under_a_key_waits_until_nothing_of_the_one_before_lives`,
    `what_the_start_placed_in_a_level_outlives_the_run_leaving_it` and
    `a_scene_that_the_start_places_in_a_level_outlives_the_run_leaving_it`.
  - G03, in *Hunna Wood*: `a_save_whose_run_cannot_be_played_is_not_resumed`, which removed the key
    and made the game panic at its next restart, `a_save_is_read_up_to_the_size_of_the_largest_one`
    and `what_is_not_a_file_at_the_save_s_place_is_neither_read_nor_moved`.
  - G11: `a_save_is_resumed_on_its_layout_given_a_few_units_in_the_last_place`, whose two layouts
    0.4's fingerprint tells apart, and
    `a_layout_is_the_same_within_a_ten_thousandth_wherever_its_numbers_lie`; on the cooked zone,
    `the_zone_s_layout_is_the_same_a_unit_in_the_last_place_away` moves each of its layout's numbers
    by one unit in the last place, up and down. Measured once, as P1 was built: seven of those
    moves, of the 793 numbers, changed 0.4's fingerprint, as the audit found.
  - G35: `a_component_of_a_type_or_a_variant_that_this_program_does_not_know_is_a_warning`
    (`slime-world`), which loading set aside, so that a game could not tell it from what an update
    removed; `a_save_of_a_later_version_or_holding_what_this_one_does_not_know_is_not_resumed`,
    `saves_set_aside_keep_names_of_their_own` and
    `a_save_not_resumed_is_set_aside_under_a_name_that_says_why`.
  - G36: `one_copy_of_the_game_at_a_time_holds_a_save` and
    `a_save_is_written_whole_or_not_at_all_through_a_file_of_its_own`.

  The save fixture of 0.4 resumes as before, by the fingerprint of its layout that it says, and the
  game's tests pass on the zone. The game was played in a compositor without a screen, two copies
  on one save: the second said that the other held the save and played a new run that it did not
  save, the first saved its run as it stopped, and a third resumed it. A review of the commits that
  P1 pushed first found, in the fix of G01, an edited save that gave one key to two entities, which
  is refused since, and rustdoc and reports that said more than the code did, corrected. The owner
  made the checks by hand, on Linux and in the Windows virtual machine, and found that everything
  works. CI took 14 minutes 4 seconds on Linux and 19 minutes 4 seconds on Windows, 56 seconds
  under the 20 minutes that the owner allows. The owner decided on 2026-10-19 to keep the start's
  fingerprint without values, to weigh again in P6, and the save fixture of 0.4 until the game's
  first release. What P1 leaves is in the [known debt](#known-debt), and its questions in P6.

### P2 — A damaged file gives an error, never a crash

- **Goal.** No damaged or edited file makes the engine panic, hang or exhaust its memory
  ([ADR-0096](adr/0096-the-files-the-engine-reads.md)): cooked data, the manifest, documents and
  scene files, glTF and image sources.
- **Findings.**
  - G04, high. The rule of a valid light is copied in the cooker, the world and the renderer, and
    not applied at every entry: the renderer's reader of cooked scenes checks only a light's
    position and direction; the world checks each `Transform` alone, never their composition
    through the hierarchy, which can overflow; the sky does not check its sun; nothing bounds color
    × intensity, which can make inf × 0 = NaN in the shader; and `Renderer::render` panics on an
    invalid light ([ADR-0042](adr/0042-lights-beyond-the-most-drawn-and-checked-scene-values.md)).
    A damaged light made the game panic at its first frame, or the bloom whiten the screen.
  - G05, high. The readers of cooked data trust the cooker. `CookedAssets` reads the file that the
    manifest names without checking the name (`../`, an absolute path) or the file's kind (a pipe
    hangs the load), whole before comparing its size; `CookedTexture::from_ktx2` hands the bytes to
    the `ktx2` crate, which panics on one byte of its descriptor, then copies every level it
    announces before checking them, 419 MB for a file of 1 MB; a texture of the wrong kind for its
    slot, sRGB color or linear data, trips an `assert!`; the asset cache stays loading after a
    panic caught.
  - G06, high. Size limits do not bound memory: a save of 16 MiB of nested empty arrays makes the
    reader of documents allocate 7 GB, and three scene files of 1 MB, each placing a prefab many
    times, a gigabyte, where the rustdoc,
    [ADR-0086](adr/0086-serialization.md) and [ADR-0091](adr/0091-scene-documents-and-placements.md)
    promise a bound.
  - G07, high. Reading a save that produces many warnings is quadratic: each warning recounts its
    line from its start, or, in the binary encoding, builds a decoder again; a line of 1 150 000
    unknown fields took 488 s.
  - G19, medium. The cooker takes any file under the expected name as good: a cooked file deleted,
    emptied or cut short is never cooked again, and the game advises a cook that repairs nothing.
    It renames what it writes without forcing it to the disk, and writes a new `.meta` without a
    temporary file.
  - G21, medium. The glTF importer does not refuse a damaged or hostile source cleanly: huge
    finite values make the tangents NaN, and `CookedMesh::to_bytes` panics; a `.gltf` reads its
    buffers wherever their paths point, `..`, an absolute path, a pipe or `/dev/zero`, whole before
    comparing their size.
  - G22, medium. JPEG images in CMYK are cooked with wrong colors, without a word (`zune-jpeg`
    0.5.15's conversion); asking it for RGB fixes them, and the repository's JPEG images keep their
    bytes.
  - G41, low. A panic in a game's upgrade or check during `World::load` leaves the world half
    loaded: entities created empty, removals done.
- **Crates.** `slime-assets`, `slime-render`, `slime-world`, `slime-world-render`,
  `slime-serialize`, `slime-reflect`, `slime-cook`.
- **Delivers.**
  - One public check of a light, which the cooker, the reader of cooked scenes, the world, the
    frame builder (on the composed transform) and the sky use; the renderer leaves out a light it
    cannot draw, with a warning that names it, amending ADR-0042; the shader bounds what one light
    adds.
  - The manifest's names checked; a cooked file checked before it is opened, and read up to its
    size; a reader of KTX2 for the layout that the cooker writes, which bounds the levels and their
    sides before it copies them; a texture's kind checked against its slot.
  - A bound on memory before a document is parsed, from a linear count of its values, and before a
    scene resolves, from its prefabs' records times their placements; warnings placed in one pass,
    and capped.
  - A cooker that checks what it reuses, forces what it writes to the disk, and imports glTF files
    from inside its sources only, without panicking on their data; JPEG images in CMYK decoded
    right.
  - A game's upgrades and checks called so that a panic sets their value aside, the world as it
    was.
  - Tests that damage the input of each reader: cut at every byte, bytes changed, valid files
    mutated at random.
  - ADR-0042, ADR-0086 and ADR-0091 amended; the known debt on NaN in the bloom corrected; the
    `ktx2` defect reported to its authors, with the owner's agreement.
- **Done when.** The tests that confirmed each finding, moved into the repository, pass; the
  mutation tests pass within their time budget; the `scene` example, on a copy of the cooked data
  with one byte changed, ends with an error, not a panic; CI is green.
- **How to test.** `cargo test --workspace`; `scripts/check.sh`.
- **Outcome.** Done on 2026-10-19. [ADR-0102](adr/0102-a-damaged-file-gives-an-error.md) records
  how each finding is fixed, and amends ADR-0042, ADR-0086 and ADR-0091, and ADR-0069, ADR-0071 and
  ADR-0076, which said that the cooker reuses any file under its name. Each defect is confirmed by a
  test that failed before its fix and passes with it; the audit's own tests were not available, and
  these were written from the findings above. The tests, by finding:
  - G41: `a_panic_in_a_type_s_upgrade_or_check_sets_its_value_aside` (`slime-world`).
  - G07: `many_warnings_cost_time_in_proportion_with_the_document_and_are_capped`
    (`slime-serialize`), 200 000 unknown fields on one line, and
    `a_save_that_holds_many_unknown_keys_warns_of_the_first_and_counts_the_rest` (`slime-world`).
  - G22: `a_jpeg_image_in_cmyk_is_decoded_into_its_colors` (`slime-cook`), on an image of 401 bytes
    made for it.
  - G05: in `slime-assets`, `a_damaged_texture_is_refused_or_read_never_a_panic`, which found the
    `ktx2` crate's panic on byte 168 set to 255, in the data format descriptor;
    `a_texture_that_announces_more_levels_than_its_file_holds_is_refused_before_they_are_copied`, a
    file of 1 MiB whose 4 000 levels, copied, exhausted a limit of 3 GB;
    `a_texture_longer_than_the_gpu_takes_is_refused`;
    `a_manifest_that_names_a_file_outside_its_directory_is_refused`; and
    `a_cooked_file_is_checked_before_it_is_read`, whose link to `/dev/zero` was read until the
    memory ran out. In `slime-render`, `a_cooked_texture_of_the_wrong_kind_for_its_slot_is_refused`
    and `a_panic_while_the_cache_loads_leaves_it_loading_nothing`.
  - G19: `a_cooked_file_deleted_emptied_or_cut_short_is_cooked_again` (`slime-cook`), on a copy of
    the examples' sources.
  - G21: `positions_too_large_for_their_tangents_are_refused`, whose triangle a search among 20 000
    random ones found, and `a_gltf_file_reads_its_buffers_from_files_inside_its_sources_only`
    (`slime-cook`).
  - G06: `a_document_of_more_values_than_a_reader_reads_is_refused_before_it_is_read`
    (`slime-serialize`), whose 16 MiB of empty arrays were parsed in 13.6 s of a debug build, and
    `a_scene_whose_placements_copy_more_than_the_limit_is_refused_before_any_is_made`
    (`slime-world`), which passed 3 GB before it stalled at a limit of 5 GB, and
    `a_save_that_spawns_more_bytes_of_scenes_than_a_scene_holds_sets_the_rest_aside`, a save edited
    to spawn a scene of 2 MiB 130 times, which the loader spawned whole.
  - G04: `a_light_that_cannot_be_drawn_is_refused` (`slime-assets`);
    `a_light_that_cannot_be_drawn_is_left_out`,
    `a_light_far_brighter_than_any_adds_no_more_than_its_bound` and
    `a_sun_whose_disc_is_brighter_than_a_float_holds_leaves_the_rest_of_the_sky_as_it_is`
    (`slime-render`), the last two of which lit pixels that the light does not reach, and whitened
    the sky; and `a_light_placed_where_no_number_reaches_is_left_out` (`slime-world-render`).

  The mutation tests are `tests/damaged.rs` in `slime-assets` (textures, meshes, materials, cooked
  scenes, the manifest), `slime-serialize` (documents as text and in the binary encoding),
  `slime-world` (a save, a scene file) and `slime-cook` (a GLB file cooked as the cooker does, PNG
  and JPEG images, a metadata file), each under a few seconds of a debug build; they found no panic
  past the fixes above. `scripts/check.sh` runs the `scene` example on a copy of the cooked sample
  whose first mesh has one byte changed: it ends with status 1 and an error that names the file. A
  review of P2's commits found that an ID of a few megabytes, which every entity copied repeats,
  made gigabytes of a resolved scene or a save's spawned scene under the limits
  (`a_placement_whose_id_each_copy_repeats_counts_it_in_the_limit`,
  `a_save_that_spawns_a_scene_under_a_long_key_counts_it_in_the_limit`, both of which exhausted a
  limit of 3 GB); that the binary encoding placed each error from its first byte again, 13 s of a
  debug build where the text took 2 (the warnings test, since in both encodings); that the cooker
  read a reused scene file whole before checking it; that a light beyond what the shader's 32-bit
  floats measure from the camera reached it as an infinity, which lavapipe happens to draw as no
  light; and documents that said more than the code did. All are fixed. What P2 leaves is in the
  [known debt](#known-debt): a cooked file damaged within its ranges, a save's capacity in values,
  the cost of checking what a cook reuses. The owner decided on 2026-10-19 not to report the
  `ktx2` crate's panic to its authors: the engine no longer reads with it. CI took 10 minutes 36
  seconds on Linux and 16 minutes 11 seconds on Windows.

### P3 — The loss of the GPU device and the main loop

- **Goal.** A lost GPU device is recovered wherever the loss is found
  ([ADR-0075](adr/0075-recovering-from-a-lost-gpu-device.md)), and a panic in `render` stays an
  ordinary panic.
- **Findings.**
  - G14, medium. A window's image acquired and never presented crashes the program on Vulkan:
    wgpu-hal panics when the swapchain is destroyed. After a panic in `App::render` that drew, that
    second panic, during unwinding, aborts the process with a core dump; and a loss found by another
    thread between the engine's check and the present makes `Surface::restore` panic. The known
    debt said that the engine always presents, which these two paths contradict.
  - G15, medium. The recovery covers a loss found between two frames, on the main thread, the only
    one that the tests make. Found elsewhere, a loss panics in a loading thread (`unmap` of buffers
    never mapped) or in `Device::poll`, or comes back as a failed readback; found during a frame or
    during `gpu_restored`, it stops the run as an error of the application; the dev overlay panics
    on a lost device; a loss that comes back a few frames after each restore is restored without
    end; after a failed restore, the error that the application gives in `shutdown` is lost.
- **Crates.** `slime-gfx`, `slime-app`, `slime-devui`, `slime-render`, *Hunna Wood*, the `world`
  example.
- **Delivers.** A window's image released when it is not presented; a loss recorded before it is
  logged; `GfxError::DeviceLost` an ordinary error wherever `slime-gfx` finds a loss; a loss that a
  frame or `gpu_restored` returns restored like the others, and the run given up after a number of
  losses within a window of time; the application's error kept first when the run ends;
  `Overlay::new` returning an error; *Hunna Wood* and the `world` example skipping a frame rather
  than quitting. New scenarios in the windowed lifecycle test: a panic in `render` after drawing
  (exit status 101, no abort), a loss in the order a real one comes (the device invalid before the
  loss is recorded), a loss during `gpu_restored`, losses soon after each restore. ADR-0075 amended;
  the case reported to wgpu, with the owner's agreement.
- **Done when.** The tests that confirmed each finding, moved into the repository, pass, the new
  scenarios among them, in Weston; CI is green.
- **How to test.** `cargo test -p slime-gfx -p slime-app -p slime-devui`; `scripts/check.sh`.
- **By hand.** In the Windows virtual machine: a timeout detection and recovery (TDR) forced while
  the zone loads, then while the game restores its GPU objects.
- **Outcome.** Done on 2026-10-19.
  [ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md) records how each
  finding is fixed, and amends ADR-0022, ADR-0072 and ADR-0075. Each defect that lavapipe can show
  is confirmed by a test that failed before its fix and passes with it; the audit's own tests were
  not available, and these were written from the findings above. The tests, by finding:
  - G14: the windowed lifecycle test's `panic-in-render`, whose process wgpu aborted, panicking
    again as it destroyed the window's swap chain while the first panic unwound, and
    `lost-unrecorded`, the device made invalid in `render` as wgpu does before it reports a
    driver's loss, its loss recorded at the next frame, on which `Surface::restore` panicked in
    wgpu.
  - G15: `a_device_invalid_before_its_loss_is_recorded_gives_errors_not_panics` (`slime-gfx`), on
    which creating a texture panicked in wgpu's unmap of its uploads, and reading a target back in
    its mapping; `an_overlay_is_not_created_on_a_lost_device` (`slime-devui`), on which egui's
    renderer panicked; the lifecycle test's `lost-in-gpu-restored` and `loss-returned-by-render`,
    which stopped the run as `RunError::App`, and `lost-soon-after-each-restore`, a device lost at
    every other frame, which opened 15 devices in a run of 35 frames and ended `Ok`.

  Not shown by a test: a wait that is the first to find a driver's loss, on which wgpu's
  `Device::poll` panics by its source, since lavapipe cannot lose a device as a driver does (the
  engine's waits no longer poll); the error that `run` returns after a restore that fails, since
  none fails in a windowed run; and the loss recorded before it is logged, an order of two lines.
  *Hunna Wood* and the examples keep passing a frame's errors to `exit_with_error`, which now
  leaves a loss to the run: the frame is skipped. The `lost-now-and-then` scenario, whose device,
  lost at every other frame, was replaced four times, is replaced by
  `lost-soon-after-each-restore`, given up at the fourth loss. Every creation now asks wgpu whether
  the device is valid, which the rustdoc of `Gpu::still_valid` measures. The three behaviours of
  wgpu that P3 works around are not reported to its authors yet: that waits for the owner's
  agreement. A review of P3's changes found, from wgpu's sources, that the surface kept the last
  image's texture while it was configured again, which Direct3D 12 refuses to resize, so that each
  resize of the window would have dropped a frame and created the surface again on Windows; that
  *Hunna Wood* turned a load that a lost device failed into a message, which hid the loss, so that
  a loss found by a loading thread stopped the game; that shaders and pipelines were created
  without the question to wgpu; and documents that said more than the code did. All are fixed. The
  checks by hand in the Windows virtual machine, a TDR forced while the zone loads, then while the
  game restores its objects, are the owner's, and wait in the
  [list of checks by hand](#checks-by-hand-waiting-for-the-owner). CI took 14 minutes 19 seconds on
  Linux and 16 minutes 22 seconds on Windows.

### P5 — The build, CI, and measurements tracked from one commit to the next

- **Goal.** Every crate compiled once, CI under its limit with a margin, and the engine's
  measurements tracked from one commit to the next.
- **Findings.**
  - B1-F01. Every crate of the workspace is compiled twice: the procedural macro
    `slime-reflect-derive` is built with the `dev` profile under `cargo build --workspace` and with
    the `build-override` profile under `cargo test`, so that every crate that depends on it is built
    again, 19.7 to 25.8 s and 9.3 GB at each run of `scripts/check.sh`. One line of `Cargo.toml`
    removes it.
  - B1-F02. A debug `target` directory takes 25 GB after the steps of `scripts/check.sh`;
    dependencies built without debug information would bring it near 7 GB.
  - B1-F05. CI's Windows job is 12 s under its limit, and the lever that
    [ADR-0081](adr/0081-stage-5a-the-world.md) decided past 19 minutes, keeping the cooked zone in a
    cache, is not applied: each job cooks the zone again.
  - C6-04, C7-08. No measurement is tracked from one commit to the next, and the audit found
    documented figures stale or wrong: the workspace's tests take 42 to 50 s on lavapipe, where the
    known debt says 23.9 s; compiling the shaders again after an edit holds the game 46 to 50 ms,
    where [ADR-0077](adr/0077-hot-reload-of-shaders.md) gives 7 ms, measured with the shaders
    already in the driver's cache.
  - G54, low. egui's default fonts are in *Hunna Wood*'s binary, where
    [ADR-0031](adr/0031-dev-overlay-with-egui.md), `deny.toml` and
    [`dependencies.md`](dependencies.md) say that they ship with no game; the owner decided on
    2026-10-19 that they may, the dev overlay staying in the game, which P5 writes in an ADR.
- **Crates.** The workspace's `Cargo.toml`, `scripts/check.sh`, the CI workflow, the crates whose
  measurements are ported.
- **Delivers.** The profiles that compile each crate once, and dependencies without debug
  information if the measurement holds on Windows. The cooked zone in CI's cache, keyed on its
  sources, the cooker, `slime-assets`, `Cargo.lock` and the platform. The audit's benchmarks of the
  world, the renderer, loading and the build in the repository, a scene of the size the engine aims
  at, and a file of measurements, versioned, taken again at each milestone; a trace of a frame that
  a profiler reads (any dependency it needs asked first). The figures of the known debt and the ADRs
  that the measurements contradict corrected. An ADR that amends ADR-0031 on egui's fonts, with
  `deny.toml` and `dependencies.md`.
- **Done when.** `cargo build --workspace` then `cargo test --workspace` compiles nothing twice; CI
  takes, on Windows, a measured margin under its 20 minutes; the file of measurements holds the
  figures that P4 starts from; CI is green.
- **How to test.** `scripts/check.sh`, timed, and the size of `target`; the benchmarks as the
  measurements file says.
- **By hand.** In the Windows virtual machine, once the benchmarks are in the repository: the
  measurements of rendering and loading on Direct3D 12; the Windows job's log of `af512af`, whose
  WARP figures no one compared.
- **Outcome.** Done on 2026-10-19.
  [ADR-0104](adr/0104-the-build-ci-and-measurements.md) records the build's profiles, CI's cache of
  the cooked zone and its two Windows jobs, the benchmarks, the file of measurements and the trace,
  and amends ADR-0004 and ADR-0077; [ADR-0105](adr/0105-egui-fonts-ship-with-the-dev-overlay.md)
  amends ADR-0031: egui's fonts ship with the dev overlay, as `deny.toml` and
  [`dependencies.md`](dependencies.md) now say (G54).
  - B1-F01, B1-F02: `cargo test --workspace` after `cargo build --workspace` compiles no library
    again, where it compiled 12, and the target directory of both is a third smaller; ADR-0104
    gives each change's figures. The dependencies keep their line tables, which the plan did not
    say: a backtrace through wgpu or winit keeps its files and lines, for 1.2 GB more than no
    debug information. CI's Windows jobs build the tests without `cargo build` before them, so
    that B1-F01 does not shorten them; what the line tables change on Windows is not measured,
    the runners' speed varying more from one run to the next.
  - B1-F05: every job keeps the cooked zone in its cache, which the cook then reuses. The Windows
    job took 13 minutes 38 seconds before P5, then 21 minutes 54 seconds and 19 minutes 5 seconds
    with the same work on slower runners, the cook taking 1 minute 54 seconds to 3 minutes 9
    seconds of it: past 20 minutes, ADR-0081 had decided to split it in two jobs that run in
    parallel, which P5 does. With the two jobs, on `2d71729`, whose new key cooked the zone again
    in each job, CI took 13 minutes 4 seconds on Linux, 11 minutes 35 seconds for `windows` and 11
    minutes 7 seconds for `windows-release`; on `1cc469b`, the cooked zone in the cache, 9 minutes
    9 seconds, 10 minutes 26 seconds and 8 minutes 51 seconds: the longer Windows job 9 minutes 34
    seconds under the limit, on a runner whose tests took 6 minutes 16 seconds, where the slowest
    of P5's took 7 minutes 20 seconds.
  - C6-04, C7-08: the benchmarks are in the repository, a program of the workspace,
    [`slime-benches`](../benches/README.md), and [`measurements.md`](measurements.md) holds their
    figures and the build's, measured on lavapipe on a machine of 4 logical CPUs. They show four
    of P4's findings there: the sky made again at 14 frames of 24 around a sun that did not move
    (G13); a parent of twice as many children emptied in four times as long (G27); a scene file of
    four times as many placements resolved in more than six times as long (G26); a load started in
    5 ms with a manifest of 10 000 assets, 74 ms with 100 000 (G25). The workspace's tests take
    79 s there, which the known debt now says. ADR-0077's 7 ms was measured with every shader in
    the driver's cache, which an edited shader is not: ADR-0104 amends it, and
    `Renderer::set_shaders`'s rustdoc names the machine of each figure. On lavapipe, an edit cost
    from 2 to 11 ms more than unchanged sources in two runs; on the RX 6900 XT, 0.8 ms more before
    P4 and none after it, in one run each.
  - The figures of lavapipe are not those of a GPU: P4's gains of the renderer and of loading
    (B3-F02, B3-F05, B4-F01) are measured on the owner's RX 6900 XT, in sections of that GPU
    before P4 and after it, measured on 2026-10-20 (see P4's outcome), and
    `render.instances.100k`, which lavapipe (Mesa 26.2.4) cannot run, crashing in
    `lvp_descriptor_set_create`, runs only there; the crash is not reported to Mesa.
  - The trace: `SLIME_TRACE` names a file into which any program that calls `init_diagnostics`
    writes its spans, which Perfetto opens. *Hunna Wood*, run for 20 frames, wrote a trace of 717
    events, each frame's phases and each submission to the GPU, which parses whole. The renderer's
    passes and the loading threads have no span of their own yet, and what a trace costs a frame
    is not measured.
  - `loading.zone` places each mesh of the cooked zone once rather than spawning its scene, whose
    components only the game registers: the zone's bytes loaded while frames are drawn, not its
    layout.
  - A review of P5 found the Windows job past its limit with the split that ADR-0081 decided not
    made; the benchmark of the sky counting the frame that makes the sky first, the zone's drawn
    without a sun; a trace that made and dispatched every log record of the dependencies; a
    script that could count no library compiled again when it could not read cargo's output; and
    documents that said more than the code did. All are fixed, but the cost of a trace, which is
    documented.
  - Not done, the owner's by hand, which wait in the
    [list of checks by hand](#checks-by-hand-waiting-for-the-owner): the measurements of
    rendering and loading on Direct3D 12 in the Windows virtual machine, and the Windows job's log
    of `af512af`.

### P4 — The measured quick wins

- **Goal.** The quick wins that the audit measured, each measured again before and after with P5's
  benchmarks, without a change of architecture.
- **Findings.**
  - B3-F02. The renderer binds the vertex and index buffers at every draw, even when they do not
    change, and draws in the order it receives: binding only what changes and sorting the draws
    took 13 to 24 % of the render CPU and 6 to 13 % of the GPU off the zone, 26 to 40 % of the CPU
    and 78 to 92 % of the GPU off 10 000 objects.
  - B3-F05. The objects' values are written at each frame into a new buffer, 256 bytes per object,
    half of it padding: keeping the buffer took 22 % off a frame of 10 000 objects; sending only
    the slots used is to measure.
  - B2-F06. The world looks up a component's storage and an entity's ancestors at high cost: about
    sixty lines made building a frame of 100 000 entities 20 to 32 % faster.
  - G27, medium. Removing a child walks all its siblings: destroying or moving many entities of a
    large zone is quadratic, 1.8 s to empty a parent of 100 000 children. The shape of
    `World::children` is decided before the editor's hierarchy.
  - G13, medium. The sky, 23 ms of work on every core, is computed again every other frame for a sun
    that did not move: interpolating two equal rotations does not give back the same rotation, and
    the frame builder compares the sun exactly.
  - G12, medium. Beyond 16 lights, the frame builder gives the renderer the world's lights in their
    order of storage: the lamps near the player may stay dark. The nearest to the camera first,
    amending ADR-0042 and [ADR-0084](adr/0084-what-the-world-draws.md).
  - G25, medium, and B4-F03. Each new batch of loading copies the whole manifest on the main thread:
    0.5 ms at 10 000 assets, 8 ms at 100 000, 87 to 91 ms at a million.
  - G26, medium. Resolving a level is quadratic in its placements, each override looking its
    placement up among all the file's entries: 12 s for 80 000 placements moved.
  - B4-F01. During a load, the four loading threads slow every frame: the render CPU went from 0.5
    to 2.9 ms, and 16.6 ms at the 99th percentile with textures of 4096²: one thread submitting to
    the GPU, and a budget of bytes per frame.
  - B1-F06. Three textures of the hero's coat, 2048 × 2043 texels, are left uncompressed because a
    side is not a multiple of 4: 26 % of the cooked bytes, 41 ms more before the first frame, and no
    warning.
  - Not measured by the audit: the order of the bind groups, an opaque pipeline without `discard`,
    the number of passes per submission, foliage left out of the farther cascades, the CPU mipmaps
    of `Gpu::create_texture`. Each is measured; a change that a measurement justifies is proposed
    to the owner.
- **Crates.** `slime-render`, `slime-world`, `slime-world-render`, `slime-assets`, `slime-cook`,
  `slime-gfx`.
- **Delivers.** Each change with its measurement before and after in P5's file; ADR-0042 and
  ADR-0084 amended for the lights' order; the known debt that each change retires or corrects.
- **Done when.** Each gain is measured and recorded; the image tests pass with the tolerances of
  [ADR-0054](adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md); CI is green.
- **How to test.** The benchmarks of P5; `scripts/check.sh`.
- **By hand (Linux).** The owner plays the zone by day and at night and finds it unchanged.
- **Outcome.** Done on 2026-10-20, but B4-F01, which P6's execution model weighs.
  [ADR-0106](adr/0106-the-measured-quick-wins.md) records each change and the five possible gains
  measured and left as they are, and amends ADR-0026, ADR-0042, ADR-0069, ADR-0071, ADR-0083 and
  ADR-0084. Every figure below was measured on lavapipe, on a machine of 4 logical CPUs: before,
  in [`measurements.md`](measurements.md)'s section "P5, lavapipe" and in two runs at `b55721c`,
  the last commit before P4; after, in its section "P4, lavapipe" and in runs at each change. What
  the GPU's own work gains is not measured.
  - G27: a child leaves its parent in constant time, the others keeping their order, which
    `children_leave_from_anywhere_and_the_others_keep_their_order` checks. A parent of 10 000
    children is emptied in 0.13 to 0.19 ms, where it took 50 to 56 ms; one of 20 000 in 0.20 to
    0.42 ms, where it took 202 to 223 ms. `World::children` gives an iterator, `Children`, rather
    than a slice. Computing the transforms costs what it did: 4.9 to 5.3 ms before, 5.1 ms after.
  - B2-F06: `FrameBuilder::build` over 100 000 entities takes 16 to 18 ms, where it took 23 to
    26 ms.
  - G26: the instructions that resolving a scene file takes double with its placements, 3.9
    billion for 8 000, 7.8 billion for 16 000, counted by callgrind; 8 000 placements resolve in
    206 to 236 ms, where they took 305 to 322 ms. The rest is parsing the file and encoding its
    values.
  - G13: the frames of `world.sky` that make the sky again went from 14 of 24 to none, the median
    frame from 73 to 79 ms to 0.2 µs;
    `an_entity_set_where_it_is_during_a_step_is_drawn_exactly_there` checks that an entity set
    where it is at each step is drawn exactly there.
  - G12: beyond `MAX_LIGHTS`, `Frame::scene` gives the renderer the sun, the other directional
    lights, then the lights nearest the camera, which
    `beyond_the_lights_drawn_the_nearest_to_the_camera_come_first` checks.
  - G25 and B4-F03: a load starts in 19 to 29 µs with a manifest of 10 000 assets or of 100 000,
    where it took 4.7 to 8.6 ms and 74 to 107 ms.
  - B1-F06: textures whose sides are not multiples of 4 are stretched to whole blocks and
    compressed, which `sides_that_blocks_do_not_divide_are_stretched_to_whole_blocks` checks. The
    zone's cooked bytes went from 259.5 MB to 209.4 MB. The time until a frame draws the zone's
    meshes whole varies more from one run to the next than this change could be told from: 1.91
    to 2.79 s before, 1.75 to 2.12 s after.
  - B3-F02: each pass draws by pipeline, material and mesh, and binds only what changed, which
    `draws_go_by_pipeline_then_material_then_mesh_as_they_first_appear` checks. In three runs
    alternated before the renderer's changes (`2b770c8`) and after them, the scene pass of
    `render.instances.10k` took 128 to 146 ms before, 135 to 138 ms after, and the CPU of
    `render.unseen.10k`, added for it, 231 to 239 ms before, 225 to 238 ms after: no gain on
    lavapipe, where ordering 10 000 instances costs about 0.8 ms of CPU. A GPU gains more, as the
    audit measured: see below, on the RX 6900 XT. The section "P4, lavapipe" was taken in a slower
    moment: its scene pass took 155 ms, and its `render.shaders`, which P4 did not change,
    9 ms more than in P5's section.
  - B3-F05: the renderer keeps the instances' values from one frame to the next and sends only the
    slots used, through `Gpu::write_uniform_buffer_start`. Writing and sending the values of 10 000
    instances, timed in the renderer by code not kept, went from 1.54 to 1.13 ms at the median,
    from 2.0 to 1.19 ms at the 90th percentile.
  - B4-F01: not changed. One thread that submits within a budget of bytes per frame would change
    what ADR-0072 and ADR-0074 decided, and budget background work, which P6's execution model
    decides; its gain shows only on a GPU. The owner set it aside on 2026-10-20, with P4's checks
    by hand, for P6's execution model to weigh; the [known debt](#known-debt) states it meanwhile.
  - The five possible gains that the audit had not measured are measured, and none is changed:
    ADR-0106 gives each figure and why.
  - On the RX 6900 XT, on 2026-10-20, in one run at `b55721c` and one at P4's last commit, the
    sections "P5, RX 6900 XT" and "P4, RX 6900 XT" of [`measurements.md`](measurements.md): the
    CPU of `render.instances.10k` went from 17.9 to 8.8 ms at the median, its scene pass on the
    GPU from 0.93 to 0.15 ms; the CPU of `render.instances.100k` from 131 to 74 ms, its scene pass
    from 27.4 to 1.2 ms; `FrameBuilder::build` over 100 000 entities from 20.1 to 11.7 ms; the
    first frame that draws the zone's meshes whole from 220 to 161 ms after it is opened. While the
    zone loads, the render CPU is 0.96 and 0.77 ms at the median, 2.4 and 2.2 ms at the 90th
    percentile, against 0.64 and 0.54 ms once it is loaded: B4-F01 on the zone's textures, of
    2048² texels at most. One run each does not tell how much these figures vary.
  - A review of P4 found the vector of the instances' values keeping its peak size, a hasher of
    addresses of its own whose 60 µs per 10 000 instances were not worth its code, a pass binding
    again what its pipelines share, the `Debug` of `Children` printing every node of the world,
    the cooker copying textures it did not stretch, rustdoc that said what the code no longer did,
    the start write of a uniform buffer without a test of its contract, and ADR-0106 silent on two
    ADRs that it amends. All are fixed.
  - The owner's check by hand, on Linux: the owner played the zone on 2026-10-20, from
    `cargo run -p hunnawood -- --new --save <a file of its own>`, and found it unchanged.

### P6 — The foundation decisions

- **Goal.** What the editor will freeze, decided as ADRs, each presented to the owner on its own,
  with its options, and each building only what Stage 6 uses; the rest keeps its stage.
- **The decisions**, with the audit's findings that ask for them:
  - The execution model: the phases of a frame, where the engine's systems run, the priorities and
    budgets of background work (C1-01, C1-06, C5-01, C7-05). Four mechanisms of threads exist
    today, none with priorities; the parallel job system stays at Stage 7.
    [ADR-0098](adr/0098-the-games-the-engine-aims-at.md) adds: a game time apart from real time,
    that slows while the UI and the camera run at real speed; background work budgeted so that a
    hidden loading never drops a frame; a simulation that holds the authority apart from what is
    shown, so that a server can one day run it alone.
    P4 leaves B4-F01, the frames that a load slows, which M6a.2 fixed: one thread
    that submits to the GPU within a budget of bytes per frame, against what ADR-0072 and
    ADR-0074 decided.
    **Decided** on 2026-10-20, in [ADR-0107](adr/0107-the-execution-model.md): one task pool of
    the engine's with a frame and a background priority, one owner of the copies to the GPU within
    a budget of bytes per frame, the phases of a frame, game time apart from real time, and a game
    as a simulation and a presentation run by a host; built at Stage 6, before play in the editor.
  - Change detection beyond transforms, changes deferred during a traversal, lifecycle hooks, and
    the state that systems compute apart from the state an author writes (C1-02, C1-04, C1-07,
    C5-02, C5-04).
    ADR-0098 adds: a world cheap to snapshot and simulate again, as the prediction of a shooter
    needs; change detection precise enough to send only what changed; identities of entities that
    can hold from one machine to another.
    **Decided** on 2026-10-20, in [ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md):
    a tick on every value of a component, with summaries, a log of removals and a structure
    counter; driven components, and poses that are not entities; hooks, messages and deferred
    changes at Stage 7; components that clone, and a copy of the world built with its first user.
    The identities that hold from one machine to another go with stable references, below.
  - Stable references and operations that undo keeps reversible, so that undoing a deletion gives
    back the same entity, built on P1's identities (C1-03, C4-01).
    P1 leaves a question, which the owner decided on 2026-10-19 to weigh here, with stable
    references: what a game creates in code without a key is named by its
    rank of creation, and a save tells a start that created others by a fingerprint of their
    numbers, the types of their components and their names, not of their values
    ([ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md)). Entities alike in their
    types, without names, created in another order, are not told apart, and a save's changes go to
    the entity of each number. A fingerprint that holds their values too would tell them apart, at
    the cost of setting aside all that a save holds of them at an update that changes one value.
    Until then, the fingerprint holds no values, and a game names by a key what it creates at its
    start.
    **Decided** on 2026-10-20, in [ADR-0109](adr/0109-stable-references-and-undo.md): the
    editor's commands name entities by their persistent identities and are kept in a journal, a
    deletion undone by creating its entities again under the same identities, at their rank; what
    the editor creates named at once as its level's file names it, and a created entity written
    into a level carried once; what a start creates in code has a key, or saving refuses; across
    machines, what a run creates numbered by the server; a compact form in memory on measurement.
  - How the editor runs a game's code: a game becomes a library and its binaries, amending
    [ADR-0055](adr/0055-games-in-the-workspace.md); playing in the editor starts the game in a
    child process first (C4-02, C6-01, C7-02).
    [ADR-0099](adr/0099-ai-agents-drive-the-engine.md) adds: the channel between the editor and
    the game's process also serves tools outside the editor, the engine's MCP server among them.
    **Decided** on 2026-10-20, in [ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md): a
    game's editor links its library and reaches its types through reflection only; Play starts
    the game in a child process, in its own window; a development channel on `127.0.0.1`, guarded
    by a token, links it to the editor, the MCP server and tests; a change of code starts the game
    again on a save of its run; running the game in the editor's process is decided again if Play
    takes more than 3 s or the channel cannot inspect what the editor needs.
  - The frame contract (G17, C2-04, C2-09, C7-04). `FrameBuilder::build` borrows the asset cache,
    and the frame it returns holds references into it, so that no render thread can draw one frame
    while the next is built: a frame that the renderer owns, built from the world, loading taken
    apart from the renderer, and views apart from the renderer, which today keeps targets and
    shadow atlases per output.
    ADR-0098 adds: what a game hands the renderer for an object is not limited to the fields of
    the physically based material, and a frame may hold several views.
    **Decided** on 2026-10-20, in [ADR-0111](adr/0111-the-frame-contract.md): a frame that owns
    what it draws, and an asset cache that loads without the renderer, the render thread decided
    at Stage 7 on measurement; views apart from the renderer, with settings of their own; a
    material of a kind registered with the renderer; the frame built again each time until a
    measurement asks to update only what changed.
  - A study of open worlds: cells, identities, saves per scene, references across scenes (C3-02,
    C4-03, C4-05, C7-03).
    ADR-0098 adds: several worlds, and a state that follows the player from one to the other; a
    whole region loaded during play, then switched to; content of a scene that depends on the state
    of the story, that state belonging to one player or to a step of the story that all share;
    cells that serve streaming and, one day, the split of a world between servers.
    P1 leaves a question: how an entity lives on when another scene takes the key of the scene
    that spawned it, as a companion that follows the hero from one level to the next. For now,
    `World::spawn_scene` refuses the new scene until nothing of the old one lives (G02), and a level
    that keeps something of the one before it goes under a key of its own.
    **Decided** on 2026-10-20, in [ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md):
    a level made of several files, cells kept out of identities, former identities written by the
    editor; `EntityRef`, a reference to an entity of any scene that waits for its target; saves made
    of differences per scene, a game choosing what persists, what resets and what is never saved;
    adoption of what outlives its scene, transfer between worlds at Stage 10a, layers that depend
    on the story at Stage 9b.
  - An asset request API, with priorities, cancellation, a memory budget and eviction (C3-01).
    **Decided** on 2026-10-20, in
    [ADR-0113](adr/0113-asset-requests-residency-and-priorities.md): requests that carry a
    priority, now, soon or later, and return a handle that keeps their assets in memory or loading;
    a load cancelled when no handle holds it; what nothing holds dropped least recently used first
    under a budget of memory per kind; textures streamed by their levels of detail at Stage 10a.
  - Hardware tiers, with budgets scaled from the RX 6900 XT's measurements (C2-03): a modest
    minimum at high frame rates, and a high-end tier for the visual targets of the realistic games
    (ADR-0098); the engine keeps WebGPU's default limits today.
    **Decided** on 2026-10-20, in [ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md):
    the adapter's limits above a floor checked at startup, modern features with the techniques
    that need them; three tiers, handheld, modest and high, each with a reference machine and
    budgets of frame time and memory, an enthusiast tier at Stage 9c; budgets checked by hand on
    the RX 6900 XT, scaled by each tier's ratio.
  - What the GPU layer gives the editor and the start of Stage 6: blending, lines, an unlit
    material, picking, passes that a client inserts in a view; storage buffers and instanced draws
    for culling and instancing (C2-02, C2-05).
    **Decided** on 2026-10-20, in [ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md):
    the GPU layer grows by pieces with their first users, now known; a milestone of rendering for
    the editor early in Stage 6, with blending, lines, an unlit material and picking by the GPU and
    the CPU; two points of each view's graph where a client inserts its
    draws.
- **Delivers.** The ADRs, accepted by the owner; [`ROADMAP.md`](ROADMAP.md) and
  [`ARCHITECTURE.md`](ARCHITECTURE.md) where they change; the code that a decision needs to be
  measured, and no more.
- **Done when.** Every decision is accepted and written; Stage 6 is planned with the owner on them,
  its first milestone frustum culling and instancing.
- **Outcome.** Done on 2026-10-20. The owner took its nine decisions that day, one at a time, each
  presented as three or four questions with their options: ADR-0107 to ADR-0115 record them, each
  reviewed against the code before it was pushed, and amend the earlier ADRs that their status
  lines name. No code was written for them: what each builds, and when, is Stage 6's. Stage 6 was
  planned with the owner the same day, on them:
  [ADR-0116](adr/0116-stage-6-in-two-halves.md) splits it in two halves, each released, and orders
  [Stage 6a](#stage-6a--what-the-editor-stands-on), whose first milestone, M6a.1, is frustum culling
  and instancing.

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
   that `scene.exe`, `world.exe` and `hunnawood.exe` embed in their dev overlay, which the Ubuntu
   Font Licence asks to travel with them: CI keeps its artifacts 14 days, a release keeps them.

0.0.1 was released on 2026-10-12, before Windows joined CI: the tag `v0.0.1` is on the commit
"Release 0.0.1". 0.0.2 was released the same way on 2026-10-12: the tag `v0.0.2` is on the
commit "Release 0.0.2", and 0.0.3 on 2026-10-13: the tag `v0.0.3` is on the commit
"Release 0.0.3". 0.1 was released on 2026-10-14: the tag `v0.1.0` is on the commit "Release 0.1.0",
and its GitHub release carries the Windows examples that CI built from that commit.
0.2 was released on 2026-10-15: the tag `v0.2.0` is on the commit "Release 0.2.0", and its GitHub
release carries the Windows programs that CI built from that commit, *Hunna Wood* with its zone
cooked and the notice that credits its assets among them. 0.3 was released on 2026-10-17, with
Stages 4b and 5a: the tag `v0.3.0` is on the commit "Release 0.3.0", and its GitHub release carries
the Windows programs that CI built from that commit, `world.exe` among them.
Stage 5b was released as 0.4 on 2026-10-18: the tag `v0.4.0` is on the commit "Release 0.4.0", and
its GitHub release carries the Windows programs that CI built from that commit.

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3a — First renderer → 0.0.3 | M3.1 GPU resources for a renderer; M3.2 the render crate; M3.3 directional light and material; M3.4 textures and samplers; M3.5 a glTF scene and camera control; M3.6 statistics, GPU timings and the dev overlay; M3.7 Windows check, documentation, release | Done; released as 0.0.3 (tag `v0.0.3`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3b — Physically based renderer → 0.1 | M3b.1 GPU layer for multi-pass rendering; M3b.2 HDR rendering and tone mapping; M3b.3 physically based materials; M3b.4 light types; M3b.5 shadows; M3b.6 image-based lighting; M3b.7 bloom; M3b.8 render graph; a pass over the known debt; M3b.9 Windows check, documentation, release | Done; released as 0.1 (tag `v0.1.0`), with the Windows examples on its GitHub release. Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3c — A first game | M3c.1 skeleton; M3c.2 guards, detection and the objective; M3c.3 night, feel and the Windows check; M3c.4 findings for the next stages | Done; no release of its own: its game, *Nightwatch*, is never released, as *Hunna Wood* replaces it in Stage 4a ([ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)). The stage each of its findings goes to is in [ADR-0057](adr/0057-what-the-first-game-asks-of-the-next-stages.md), and the two that became Stage 3d in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md). |
| 3d — Anti-aliasing and the shadows of spot lights | M3d.1 multisampling in the GPU layer; M3d.2 anti-aliasing in the renderer; M3d.3 spot lights' shadows: what each tile sees; M3d.4 spot lights' shadows: drawn and read; M3d.5 *Nightwatch* and the Windows check; M3d.6 a smoother shadow filter; M3d.7 what each cascade sees; M3d.8 the cascades read; M3d.9 *Nightwatch* at 3840 × 2160 | Done; no release of its own: what it delivers is released with 0.2. Its decisions are in [ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) and ADR-0058 to [ADR-0065](adr/0065-reading-the-cascades.md); the cost of its scene pass on Direct3D 12 with each shader compiler, which made DXC ship beside the Windows programs, is in [ADR-0066](adr/0066-shipping-dxc-beside-the-programs.md). |
| 4a — Cooked assets → 0.2 | M4a.1 *Hunna Wood*'s skeleton; M4a.2 asset identity; M4a.3 images; M4a.4 glTF; M4a.5 the sample scene through the pipeline; M4a.6 the zone's sources; M4a.7 *Hunna Wood* by day; M4a.8 night; M4a.9 Windows check, documentation, release 0.2 | Done, released as 0.2 on 2026-10-15. Its decisions are in [ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md) to [ADR-0071](adr/0071-cooked-textures.md); the zone's budgets are set by the test that checks them, in `games/hunnawood/src/zone.rs`, and what was left to refine went to Stage 4b's M4b.8. |
| 4b — Live assets | M4b.1 a shared GPU and a task pool; M4b.2 the zone loads while frames go on; M4b.3 recovering from a lost GPU device; M4b.4 *Hunna Wood* goes on after a lost device; M4b.5 materials that change in place; M4b.6 hot reload of cooked data; M4b.7 hot reload of shaders; M4b.8 refining *Hunna Wood*; M4b.9 Windows check, documentation, close | Done on 2026-10-16; no release of its own: what it delivers is released with 0.3, at the end of Stage 5a. Its decisions are in [ADR-0072](adr/0072-stage-4b-live-assets.md) to [ADR-0077](adr/0077-hot-reload-of-shaders.md). The owner checked the recovery from a lost device by hand at its close: in the Windows virtual machine, from the dev overlay and by a forced TDR, and in a KDE Plasma session after a real reset of the GPU (see [Known debt](#known-debt)). |
| 5a — The world → 0.3 | M5a.1 the world: entities and components; M5a.2 the hierarchy and the transforms; M5a.3 what the world draws; M5a.4 *Hunna Wood* on the world; M5a.5 reflection; M5a.6 any component listed and edited; M5a.7 serialization with format versioning; M5a.8 the world round-trips; M5a.9 saving and resuming a run; M5a.10 Windows check, documentation, release 0.3 | Done, released as 0.3 on 2026-10-17 with Stage 4b. Its decisions are in [ADR-0078](adr/0078-stage-5-in-two-halves.md) to [ADR-0088](adr/0088-saving-and-resuming-a-run.md), with the measurements of the world's storage, transforms and frame in ADR-0082 to ADR-0084; its save fixture, now `games/hunnawood/fixtures/save-0.3.toml`, is set aside by later versions as a save of another layout ([ADR-0090](adr/0090-stage-5b-scenes-and-prefabs.md)), and how long CI took with each crate it added is in the [Notes](#notes). |
| 5b — Scenes and prefabs → 0.4 | M5b.1 scenes placed in a world; M5b.2 a world written back as a scene; M5b.3 saves and hot reload with placements; M5b.4 the binary encoding of documents; M5b.5 scene files cooked, typed components, prefabs in the `world` example; M5b.6 *Hunna Wood*'s zone made of prefabs; M5b.7 Windows check, documentation, release 0.4 | Done, released as 0.4 on 2026-10-18. Its decisions are in [ADR-0089](adr/0089-scenes-and-prefabs.md) to [ADR-0095](adr/0095-cooked-scene-files.md), with the measurements of spawning scenes in ADR-0091; its save fixture is `games/hunnawood/fixtures/save.toml`, the zone made of prefabs placing its land and six lanterns. The audit of 0.4 that followed it is the [audit pass](#the-audit-pass-and-the-foundations-pass). |

The details of each completed milestone (what it delivered, how it was checked, what was measured)
are in the history of this file and in the commits that implemented it; the measurements that
justify a decision are also in its ADR.

## Known debt

Limitations known and accepted for now. The ones that a contributor could take on alone are also
GitHub issues, linked from their entry; this list stays the reference.

- **What a load reads waits in staging memory until its copies are submitted**
  ([ADR-0118](adr/0118-the-task-pool-and-the-copies-to-the-gpu.md)). The renderer's uploader sends
  at most a budget of bytes per frame, and nothing holds the loads back when they read faster: a
  load of many hundreds of megabytes keeps them in the CPU's memory while it lasts. The asset
  requests of M6a.9 ([ADR-0113](adr/0113-asset-requests-residency-and-priorities.md)) weigh it.
  How many of the loads' tasks run at once, and the budget, were measured on the owner's RX 6900 XT
  alone; each renderer has an uploader of its own, and two renderers send twice the budget.
- **The first frame that draws shadows costs more, and more still while a scene loads**
  ([ADR-0074](adr/0074-uploading-textures-without-holding-up-frames.md)). It makes the renderer
  create its shadow atlas and its frame again; during a load, the GPU, busy with the load's copies,
  presents that frame late, past 33 ms in some runs, measured before M6a.2. M6a.2 holds the copies
  to a budget once a frame is drawn, but not those before the first frame, which that frame waits
  for; it was not measured again. *Hunna Wood* draws its hero, and so its first shadows, from its
  first frame. Creating the atlases with the renderer would remove it, at a cost in
  memory for every program, shadows or not.
- **The zone costs up to 8 s per frame on lavapipe**, the CPU driver of the checks, where the RX
  6900 XT takes under 2 ms: most of it goes to the four shadow cascades of the sun, each 2048 ×
  2048 texels, through which thousands of overlapping leaf cards are drawn with a shader that
  discards. The checks run the game for one frame by day, one at night, two around a lost device
  and one resumed from the save fixture. The oaks' cards made smaller and four times as many at
  M4b.8 changed it little: the day's shadow pass took 6.0 s
  instead of 6.2 s, in a release build. Leaves that cast no shadow in the farther cascades would
  lower it, to measure with views of the zone.
- **`cargo test` leaves out the tests of the cooked zone**: they need the zone fetched and cooked,
  so they are ignored (`#[ignore]`); `scripts/check.sh` and CI's `windows` job fetch and cook the
  zone, then run them (`cargo test -p hunnawood -- --ignored`).
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
  session with it. KDE Plasma's KWin recovers from resets: on 2026-10-16, after a reset forced on
  the owner's machine in a Plasma session, *Hunna Wood* opened a new device 70 ms after it found the
  loss and drew its zone again 2 s later, its run kept, while the terminal it was started from and
  Plasma's panel ended. On Windows, the desktop's compositor survives resets, and the game went on
  after a forced timeout detection and recovery (TDR) in the owner's virtual machine.
- **A lost GPU device is replaced only for the applications that ask for it** (ADR-0075; issue
  [#2](https://github.com/HoneySlatt/SlimeEngine/issues/2)): the others stop with an error, and
  since winit allows one event loop per process, carrying on means starting the process again. An
  application that asks drops and creates its GPU objects again itself, as *Hunna Wood* does. A
  real driver reset can only be checked by hand, as it was at Stage 4b's end (above). A restore
  that fails in a windowed run is not tested: nothing makes one fail on purpose there, nor, so,
  that `run` then returns the application's error first (ADR-0103).
- **Three behaviours of wgpu 30 are worked around, and not reported to its authors**
  ([ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)): wgpu holds a
  device invalid before it reports the loss, without a way to ask, and reports the use of objects
  created in between as a misuse; `Device::poll` panics on a lost device; on Vulkan, wgpu panics
  when it destroys a swap chain whose image work in flight still holds. The engine asks with a
  buffer of 4 bytes after each creation, waits for the GPU by submitting empty work, which wakes
  later after the GPU finishes than a poll would (`Gpu::wait_idle` says how much), and releases a
  surface's last image before its swap chain. P3 asks the owner's agreement to report them.
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
  (`crates/slime-app/src/signal.rs`): what a game saves in `shutdown` is lost then. *Hunna Wood*
  saves its run there since M5a.9: the run since the last F5 is lost when Windows ends the session
  (ADR-0088). A fix needs a new dependency, which ADR-0081 leaves to a later decision.
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
- **Upgrading wgpu waits for egui** while the dev overlay draws with it (ADR-0031); the editor
  uses the engine's own UI framework (ADR-0098). `egui-wgpu` is built against one wgpu version
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
- **A cook reads and hashes every source file at every run, and reads every cooked file it
  reuses**, and cooked files that no source makes any more stay in the output directory (ADR-0071,
  ADR-0102): nothing records a source's size and time to skip unchanged ones, which is to measure
  on *Hunna Wood*'s zone, nor cleans the output, where each change that `slime-cook watch` cooks
  leaves the files it replaced (ADR-0076). The rustdoc of `slime_cook::cook` measures what reading
  the cooked files costs.
- **A cooked file damaged within its ranges is not found** (ADR-0102): the cooker cooks again a
  file deleted, emptied, cut short or that its reader refuses, and a game refuses such a file with
  an error, but texels or numbers changed within their ranges draw wrong, without a word.
  Checksums may come beside the rule, with archives (ADR-0096).
- **Two watches repeat the same polling** (ADR-0077): `slime_assets::ManifestWatch` and
  `slime_render::ShaderWatch` each read their files every half second and tell a failure once,
  with the same dozen lines, over what each reads. No crate that both reach suits a type for both
  yet.
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
  misuse, and nothing reports it. The engine presents every frame it acquires, a panic in
  `App::render` aside, whose frame the surface releases when it is dropped, as it releases an image
  that a lost device refused to present
  ([ADR-0103](adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md)). A `Drop` that
  presents was rejected: it would show a black or half-drawn frame instead of skipping it, and
  present during unwinding.
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
- **The dev overlay can still panic on a lost device** (ADR-0022, ADR-0103). `Overlay::render`
  skips the frame when the device was lost earlier in it, and `Overlay::new` returns an error on a
  lost device, but two cases panic inside egui-wgpu or wgpu: a loss that egui-wgpu's own buffer
  upload is the first to find, and a device lost between `Overlay::new`'s check and egui-wgpu's
  creation of its buffers (`create_buffer_init`). Both are documented under `# Panics`; closing
  them needs a change in egui-wgpu or wgpu.
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
- **Every instance of a scene costs CPU time at each frame** (`Renderer`,
  [ADR-0117](adr/0117-frustum-culling-and-instancing.md)). Frustum culling and instancing make a
  frame's draws and its GPU work follow what each pass sees, but the renderer keeps nothing from
  one frame to the next: at each frame, it places and tests every instance's box against the
  camera's view and against every cascade and spot light, on one thread, ranks every instance's
  material and mesh, sorts those drawn, and packs and sends their values, about 0.2 µs per
  instance of 100 000 cubes on the owner's machine (`render.instances.100k`,
  [`measurements.md`](measurements.md)). A render scene kept from one frame to the next, with
  stable keys, and culling and draws decided by the GPU come with Stage 9a (ADR-0097).
- **A page of meshes keeps the ranges of dropped meshes until new ones fill them or it empties**
  ([ADR-0117](adr/0117-frustum-culling-and-instancing.md)). Nothing moves a mesh to pack its
  page: a game that drops half its meshes keeps their pages' memory while one mesh stays in each.
  Dropping a mesh frees no GPU memory until its page empties: how the asset cache's budgets
  (ADR-0113) count meshes in pages is M6a.9's to decide. Capture tools name a page's buffers, not
  its meshes.
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
  too, which costs overdraw. On lavapipe, opaque materials drawn without `discard` did not make the
  scene pass faster ([ADR-0106](adr/0106-the-measured-quick-wins.md)); on a GPU, not measured:
  compare the scene pass's GPU time on a scene with much overdraw against a build whose opaque
  materials use a fragment entry point without `discard` (six pipelines instead of three).
- **Edges that a masked material cuts inside a triangle stay aliased** (ADR-0059). Multisampling
  smooths the edges of triangles, but the scene shader runs once per pixel, and the `MASK` alpha
  mode keeps or discards all of a pixel's samples at once. Smoothing those edges, alpha to
  coverage, behaves differently from one driver to the next, which image tests would see. The leaf
  cards of *Hunna Wood*'s trees and its ferns are masked.
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
  are untried, and stay so for now, the owner having no other machine (ADR-0097). The ratios that
  scale the hardware tiers' budgets to the RX 6900 XT are estimates from public benchmarks until
  the tiers' machines are measured ([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)):
  a GPU rented in the cloud, a Steam Deck, or contributors would end it.
- **The environment is computed on the CPU, in every process that draws a sky** (ADR-0039): 23 ms
  in a release build on the owner's 16 threads, 162 ms on one core, and the BRDF table once (15
  ms); about eight times longer in a debug build, which every test that draws a sky with a gradient
  or a sun pays, most in `slime-render`'s `tests/environment.rs` and the examples' `tests/gltf.rs`,
  whose every load of a file computes the sample's sky: 9 s and 13 s, timed by hand at P5, of the
  workspace's 79 s of tests on lavapipe, on a machine of 4 logical CPUs
  ([measurements](measurements.md) holds the total). A sky that changes needs a new environment:
  no time of day. To measure: `cargo test --release -p slime-render --lib -- --ignored
  --nocapture`, against the budget of M3b.6. Not yet scheduled: to move to the cooker, or to
  compute passes if skies change at run time; if the tests' time matters first, building
  `slime-render` optimized in debug builds would shorten it, at the cost of stepping through it in
  a debugger.
- **The sky's sun is described twice in the renderer's own API** (ADR-0039). A game that draws
  with it gives the same `DirectionalLight` to `Sky::sun`, for the disc and the glow, and to
  `Scene::lights`, for its light and its shadows; two different lights show a disc where no light
  comes from. The `scene` example takes both from the file's first directional light. A world gives
  it once ([ADR-0084](adr/0084-what-the-world-draws.md)), as *Hunna Wood*'s does.
- **A node renamed in Blender loses what saves held of it** (ADR-0079, ADR-0087). A scene's entity
  is known by its node's name: loading a save made before the rename sets aside what it held of
  the node, and reports it. *Hunna Wood*'s zone script names every node and fails when Blender
  numbers one, so that only a change of the script renames a node; the saves made before it set
  aside what they held of it, or are saves of another layout. Identities that a scene file keeps for
  a former name wait for a game that has players (ADR-0089).
- **Some meshes of *Hunna Wood*'s zone are named with Blender's numbers** (`wooden_lantern_01.001`
  in the lantern, `capital.004` in the land): the script checks the names of its objects, not of
  their meshes. The cooker derives a mesh's ID from its name, so that a later change of the script
  that numbers a mesh otherwise changes the mesh's ID. Naming the meshes changes the export, and
  waits for the next release of the mirror.
- **Blender's exports differ from one run to the next** (G55, low): the hero's, the lantern's on a
  machine with another number of threads, the four textures that Blender renders, and the sample
  scene's sphere. Blender computes the tangents of a mesh of more than 10 000 faces in parallel, in
  an order that changes; its renders write their date and duration in the PNG files' text; and its
  UV sphere orders its faces otherwise in each process. The hero kept on the mirror is the one the
  game reads. Running the scripts on one thread, with `--factory-startup` and a pinned version of
  Blender, and removing the PNG files' text, would make them reproducible.
- **The camera's yaw is not saved** (ADR-0088). It is the game's, outside the world: a run resumed
  or loaded puts the camera behind the hero.
- **A save cut short may load in part** (ADR-0087). A save is a document of sections: one cut at the
  boundary of a section is still a document, and loads what it holds without an error. *Hunna Wood*
  writes its saves whole or not at all (ADR-0088), as every game that writes saves must.
- **A save is text, of limited size** (ADR-0087, ADR-0102). Saving refuses a save larger, or of more
  values, than what `slime-serialize` reads, about 22 000 entities that have a name and a transform,
  a value counted for each `=`, `,`, `[`, `{` and `.` of the text, a float's point included; loading
  costs more than saving, much of it parsing the text: the rustdoc of `World::save` and
  `World::load` has the figures. *Hunna Wood*'s saves are far below the limit. The binary encoding
  exists since M5b.4, but saves stay text until a measurement asks (ADR-0089): a larger world needs
  saves in the binary encoding, or read in parts.
- **What a start creates without a key, alike in its types and without a name, can take another's
  saved changes** (ADR-0101). A save tells a start that created other entities outside scenes by a
  fingerprint of their numbers, the types of their components and their names: a later version of
  a game that creates two such entities in the other order is not told apart, and the save's
  changes go to the entity of each number. A game names by a key what it creates at its start
  (`World::spawn_keyed`). [ADR-0109](adr/0109-stable-references-and-undo.md) decides that saving
  refuses a world whose start created an entity that has no key and is not `Unsaved`, built at
  M6a.7.
- **A created entity written into its level exists twice once the run is carried into it**
  (ADR-0092, ADR-0093). An entity created in code under a scene's root keeps `created/<n>`, and
  the file that `World::write_scene` writes holds it under an ID of its own, `zone/torch-0rdx`;
  once the file is cooked again, `World::carry` gives the file's entity and the run's, without a
  word. The audit's test showed it at 0.4, and again on 2026-10-20.
  [ADR-0109](adr/0109-stable-references-and-undo.md) decides that carrying recognizes the entity
  by the ID that the world gave it, built at M6a.7, before the editor writes levels.
- **`World::spawn_keyed` gives a key under which the world remembers a scene of which nothing
  lives** (ADR-0101): the entity created is then saved as that scene's root, and set aside with the
  scene when a save loads where the scene cannot be resolved. `World::spawn_scene` refuses a key
  whose scene has an entity alive; a key whose scene is gone whole is the world's to forget or
  refuse. [ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md) decides adoption and
  saves per scene, which settle it when they are built, at M6a.7.
- **`Range::contains` stays public beside `Range::holds`**: two ways to check a number against a
  range, the first in `f64`, which only tests' helpers use. Every check of a field's range uses
  `Range::holds` (ADR-0101); the helpers can move to it, and `contains` go.
- ***Hunna Wood* compares the saves of 0.4 by the fingerprint of their layout** (ADR-0101), as 0.4
  did, its fragility at the boundaries of its rounding included, until the save fixture is made
  again: the owner decided to keep it until the game's first release, before which ADR-0100 allows
  making it again. Later saves say their layout's numbers.
- **Files stay beside *Hunna Wood*'s save**: `save.toml.lock`, which locks the save while a copy of
  the game runs, and `save.toml.<process id>.new`, when the game is killed while it writes. Neither
  is read again, nor removed.
- ***Hunna Wood*'s lock holds a save by its path** (ADR-0101): two copies that reach one save by two
  paths, one through a symbolic link, take two lock files and do not exclude each other; on a file
  system without locks, the game neither resumes nor saves, and says so; a copy of 0.4, which takes
  no lock, still writes a save that a later copy holds.
- **A save that removes a component of a type that the program does not know sets it aside**
  (ADR-0101): `removed.components` names it, and loading reports that the entity holds no such
  component, where a value of an unknown type is a warning. Writing the save back loses the
  removal, which a later version that knows the type would have applied; *Hunna Wood* sets aside
  the saves of later versions by the version that they say, which covers it for the game.
- **Children and queries in another order after loading** (ADR-0087). A world loaded from a save
  holds its entities in other slots, and an entity's children in the order the save set them: what
  must go the same way after loading cannot depend on either order.
- **Spawning a large scene makes each component from its value** (ADR-0091 holds the figures; on
  the game's zone, at M5b.6, reading and resolving its scenes took 1.24 to 1.75 ms in a release
  build, twice what 0.3's one scene took). A scene resolves and spawns far slower than as many
  entities spawned with `World::spawn` (ADR-0083): each component is made from its value, and each
  entity's identity is a list of texts, held twice. ADR-0089 orders the remedies, inside the world:
  typed components copied rather than made from values, identities that share their parts, scenes
  prepared on another thread.
- **A world written back writes what it holds of what spawning set aside** (ADR-0092). A
  reference, or a parent, that a scene file wrote into a placement whose prefab no longer had its
  entity is written as the world holds it, no entity or the root: what it named is lost, as a save
  loses it. An entity of a placement moved out of the place that its prefab gives it is written in
  that place, and the children of a placement's entity in its prefab's order, then the file's; both
  are reported: a scene file has no parents for a placement's entities, nor an order among them.
  Stage 6b decides whether a file says them.
- **A scene file written back loses its comments** (ADR-0089): `World::write_scene` writes the
  canonical form. Stage 6b decides whether its editor keeps the comments of files written by hand.
- **The cooked glTF scene is a format apart from the binary encoding** (ADR-0089, ADR-0095). A
  glTF file's scenes are cooked into a format of their own, `slime_assets::CookedScene`, and only
  the typed properties of their nodes into the binary encoding: cooked scenes have two formats, and
  two readers. The renderer's scene loader, the `scene` example and the image tests read the format;
  moving it onto the binary encoding would make `slime-render` depend on serialization, and waits
  for a reason.
- **After a lost device, what is hidden loads only once shown** (ADR-0084): the cache loads what is
  drawn. In *Hunna Wood*, a key taken or a door opened before the loss shows a few frames after R
  shows them again.
- **Any change of *Hunna Wood*'s cooked data makes a new game of its scenes.** With `--watch`, the
  game makes a new game of the scenes before it knows whether a scene changed, a texture saved in an
  image editor included (`take_assets` in `games/hunnawood/src/main.rs`): about 2 ms, for a tool of
  development.
- **A zone loaded again with `--watch` changes in parts.** Its entities change at once, the meshes,
  materials and textures that the cache holds when its reload ends, and new meshes once their batch
  is loaded; *Hunna Wood* swapped whole scenes before it ran on the world.
- **The tests that edit every leaf of every component are written twice** (ADR-0085): once in
  `slime-world` for its components, once in *Hunna Wood* for the game's, about fifty lines that
  choose another value for a leaf. One copy would need a test-support crate, or a feature of
  `slime-world` that only tests enable; neither is worth it yet.
- **An edit through reflection reads the whole component again** (ADR-0085): a field that its type
  skips takes its `Default` value, and a rotation is edited whole, as `Transform`'s check refuses a
  coordinate edited alone. The panel of M5a.6 edits a rotation whole.
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
  [ADR-0111](adr/0111-the-frame-contract.md) decides views apart from the renderer, each with its
  own targets and timers, built at M6a.8.
- **Tiny, very bright sources blink in the bloom** (ADR-0040). Without a firefly filter (Karis's
  average dims the sun's halo and makes it flicker, measured), a highlight smaller than a pixel
  blooms as its coverage of the pixel changes from frame to frame. Temporal anti-aliasing, or a cap
  on the light a pixel brings to the chain, belong to Stage 9a (ADR-0052).
- **A pixel whose light is not a number spreads through the bloom** (ADR-0040). The renderer refuses
  values that are not finite in meshes and materials, and the skies' radiances; it leaves out a
  light that it cannot draw, as the frame builder does a sun, and bounds the light that each light
  and the sun's disc give a point (ADR-0102). Not everything that reaches the shaders is checked: a
  normal of zero length, which `MeshData` asks callers not to give, can make its pixels NaN in the
  high-dynamic-range target, and so can, at an exposure of 0, an emission so strong that, added to
  the light of the scene, it is not finite; the bloom averages them into its levels, and from there
  into every pixel within reach of their halo, a blot where one surface was wrong. Dropping such
  light before the bloom would keep it to its pixels.
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

### Left by the audit of 0.4

Defects that the audit of 0.4 confirmed and that neither the audit pass nor the foundations pass
fixes ([ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)), each with its
number, its rank on [ADR-0096](adr/0096-the-files-the-engine-reads.md)'s scale and the stage it
waits for. A test that confirms each is kept outside the repository, and comes in with its fix.

- **One unreadable file hides its whole batch of meshes** (G16, medium; M6a.9, and Stage 6b for
  passing errors). The asset cache loads meshes in batches, all those a frame asks for in one, 430
  for *Hunna Wood*'s zone: a batch that fails draws none of its meshes, and is not asked for again
  until the cooked data changes (ADR-0084), a passing error such as GPU memory running out
  included. M6a.9's requests fail asset by asset; trying a passing error again is to decide with
  the editor's asset browser.
- **A resize keeps the targets of the old size while it creates the new ones** (G18, medium;
  M6a.8): the GPU memory of the frame's targets is twice as large for that moment, and after
  `GfxError::OutOfMemory` the renderer keeps targets that no longer fit, which nothing releases.
- **On Windows, a `.meta` whose name differs from its source's by case only takes a new ID at every
  cook** (G20, medium; Stage 6b), and `slime-cook watch` cooks again without end: the scan matches
  the two files by the file system's rule, which ignores case on NTFS, then looks the ID up by the
  exact path. Not run on Windows by the audit: it waits in the
  [list of checks by hand](#checks-by-hand-waiting-for-the-owner).
- **Importing a glTF file takes time quadratic in its named meshes** (G23, medium; Stage 6b): 38 to
  57 s for 80 000 meshes, as each name is counted against the whole list.
- **The world's storage keeps 4 MB per component type and per million slots, and gives nothing back
  after destruction** (G28, medium; Stage 10a): each type keeps an index of 4 bytes per slot up to
  the highest slot that held it, and neither the index, the nodes nor the slots shrink
  ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)).
- **Masked foliage loses its coverage in its smaller mip levels** (G30, medium; Stage 9a): the
  cooker averages alpha from one level to the next, so that the share of texels above the cutoff
  falls in the levels drawn far away, down to none, and trees fade in the distance. Preserving
  coverage per level for textures that a masked material reads is to do before large exteriors.
- **`World::write_scene` writes, without an error, a level that cannot be opened again** (G31,
  medium; Stage 6b): it never counts the entities that the file gives once its placements resolve,
  which the reader refuses beyond `MAX_ENTITIES`, nor what they take, refused beyond
  `MAX_SCENE_SIZE`.
- **A very deep hierarchy costs quadratic time** (G32, medium; `is_hidden` at M6a.3, the rest in
  Stage 6b): spawning, loading, saving and each frame walk every entity's ancestors (`set_parent`'s
  search for a cycle, `World::unsaved`, `is_hidden`, `interpolated_transform`).
- **The world panel rounds a number that was only clicked or crossed with the keyboard, and writes
  the rounding into the world** (G33, medium; Stage 6b): egui's `DragValue` shows as many decimals
  as the drag's speed gives, and writes back the text it shows when it loses the focus. The rule for
  the editor's inspector: a field changes a value only when the user changed it.
- **A GPU object used with another `Gpu` acts on another object without a word, and no API opens a
  second window on the same GPU** (G34, medium; Stage 6b): each `Gpu` numbers its objects from zero,
  and a window's surface comes only with a new `Gpu`. The editor's model of windows decides.
- **If creating the spot lights' shadow atlas runs out of memory, the sun loses its shadows without
  a word** (G37, low; M6a.8), until a frame creates it: the atlases' bind group is not built
  again.
- **Between two fixed steps, the children of a turning parent cut the arc's chord while their
  siblings stay on it** (G38, low; Stage 7): a child moved by its parent is drawn in a straight line
  between its two positions. Interpolating through the hierarchy is to decide before skeletal
  animation turns many hierarchies.
- **A string on several lines in a scene file checked out on Windows keeps its CRLF line ends**
  (G40, low; Stage 6b): `.gitattributes` keeps `*.toml`, `*.meta` and `*.tsv` at LF, not `*.scene`,
  so that the value and the cooked bytes depend on the checkout.
- **A negative integer, or one too large, for an unsigned field gives a wrong message** (G42, low;
  Stage 6b): "not an integer" or "not text", rather than its range.
- **The writer of documents accepts what the reader refuses** (G43, low; Stage 6b): a reserved key
  `$…` at the root, and a field given twice, the second lost.
- **A camera target farther than about 2 × 10¹⁹ m draws nothing, without a message** (G44, low;
  later): `view_from_eye` squares the direction in `f32`.
- **`World::write_scene` loops without end when more than 1 679 616 entities need IDs from one
  base** (G45, low; Stage 6b): it draws four base-36 characters until one is free.
- **The world panel costs about 1.25 µs per entity and per frame** (G46, low; Stage 6b): 125 to 145
  ms per frame at 100 000 entities, every row laid out, visible or not. The editor's hierarchy needs
  a virtualized list.
- **A multisampled output target makes `render` panic inside wgpu** (G47, low; M6a.8), a panic
  that its rustdoc does not list.
- **`Registry::register_dynamic` accepts version 0, which the derive refuses** (G48, low; Stage 8):
  the values are written, then never read back, with a wrong message.
- **Two cooks in one process share their temporary files** (G49, low; Stage 6b): named after the
  process, they separate processes, not threads, and one cook fails at random, naming a file that
  exists.
- **An output directory inside the sources breaks the next cook** (G50, low; Stage 6b): cooked glTF
  scenes take the `.scene` extension of scene files, which the next scan reads as sources.
- **After a glTF file is moved with its `.meta`, the cooker's cache keeps the old path** (G51, low;
  Stage 6b): the manifest names it, and warnings blame tables that are right.
- **Two `.meta` with one ID, one of them without a source, make the cook stop without saying which
  is the orphan** (G52, low; Stage 6b).
- **`Manifest::from_bytes` refuses a valid manifest whose names are short** (G53, low; Stage 6b): it
  assumes 45 bytes per entry where the least is 37.
- **With `--watch`, after the cooked data is cleaned, a reload that the game refuses leaves it
  unable to save** (G56, low; Stage 6b): `SceneLibrary::reload` forgets the changed scenes before
  the game accepts them, and the files of the old manifest are gone.
- **The dev overlay is built into the game that ships**, egui and its fonts with it (B1-F08):
  an estimated 10 s of the game's release build, 46 s on the owner's machine. The owner
  chose on 2026-10-19 to keep it in the game, and its fonts with it (P5); building it only in
  development would need a feature of the game.

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
  the repository being public and its minutes free, and to reconsider past 20 minutes. With
  `slime-world` (M5a.1, 2026-10-16): Linux 10 minutes 25 seconds, Windows 17 minutes 20 seconds.
  With `slime-color` and `slime-world-render` (M5a.3, 2026-10-16): Linux 12 minutes 33 seconds,
  Windows 17 minutes 26 seconds. With `slime-reflect` and `slime-reflect-derive` (M5a.5,
  2026-10-16): Linux 9 minutes 6 seconds, Windows 18 minutes 27 seconds. With `slime-serialize`
  (M5a.7, 2026-10-16): Linux 13 minutes 26 seconds, Windows 15 minutes 31 seconds. With no crate
  added since, at M5b.6 (2026-10-18): Linux 14 minutes 17 seconds, Windows 19 minutes 48 seconds,
  12 seconds under the limit. At P5 (2026-10-19), with the same work, from 13 minutes 38 seconds to
  21 minutes 54 seconds on Windows, as the runners' speed varied: the Windows job is split in two
  jobs that run in parallel (ADR-0081, ADR-0104), and every job keeps the cooked zone in its cache:
  on `2d71729`, the zone cooked again, Linux 13 minutes 4 seconds, `windows` 11 minutes 35
  seconds, `windows-release` 11 minutes 7 seconds; on `1cc469b`, the cooked zone in the cache, 9
  minutes 9 seconds, 10 minutes 26 seconds and 8 minutes 51 seconds.
