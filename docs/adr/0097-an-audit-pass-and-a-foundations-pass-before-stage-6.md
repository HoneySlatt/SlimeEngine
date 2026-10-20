# 0097. An audit pass and a foundations pass before Stage 6, and the roadmap after them

- Status: Accepted; amends [0052](0052-amend-roadmap-before-0-1.md) (Stage 9a's content and the
  release map); amended by [0098](0098-the-games-the-engine-aims-at.md) (the criteria of the
  foundations pass), [0099](0099-ai-agents-drive-the-engine.md) (one more criterion of the
  foundations pass), [0100](0100-updates-of-shipped-games.md) (what it leaves for Stage 9b) and
  [0107](0107-the-execution-model.md) (the time of a game, brought from Stage 7 to Stage 6)
- Date: 2026-10-19

## Context

Stage 6, the editor, builds on the world, scenes, saves, serialization and the renderer, and
freezes much of their API: its panels, its undo and its levels use them from its first milestone.
Before planning it, the owner had the engine audited at 0.4, on 2026-10-18 and 2026-10-19, without
changing it, in three parts:

- **Correctness and robustness.** Each defect confirmed came with a test that fails, and the
  defects were ranked on the scale of [ADR-0096](0096-the-files-the-engine-reads.md), the threat
  model decided with them: 56 groups, 3 critical, 8 high, 25 medium and 20 low. The three critical
  ones are in saves and identities: an update of a game that creates one more entity at the start
  applies a save to the wrong entities; reloading a run after a change of level destroys what
  survived the old level; an edited save makes *Hunna Wood* panic or refuse to start at every
  launch.
- **Performance**, measured against budgets on the owner's RX 6900 XT (RADV), lavapipe and the
  game's zone. *Hunna Wood* is within its budget: at worst 2.2 ms of CPU and 4.4 ms of GPU for a
  frame at 3840 × 2160 by night. But the renderer draws each object with a draw call of its own,
  with no culling: 0.6 to 2.4 µs of CPU per object, so that 4 ms hold 870 to 5 000 objects, where a
  large game shows 10 000 to 100 000. Quick wins were measured, as sorting the draws and binding
  only what changes, 13 to 24 % of the zone's render CPU; the build compiles every crate twice; CI
  is at its limit of 20 minutes; nothing tracks a measurement from one commit to the next.
- **Architecture**, against the games the engine aims at. The roadmap puts, after the editor,
  decisions that the editor will freeze: the order of a frame and where engine systems run (Stage
  7), change detection, stable references for undo, how the editor runs a game's code (Stage 8),
  the frame handed to the renderer, the model of an open world and streaming (Stage 10), culling
  and instancing (Stage 9a). For a third-person action role-playing game, skeletal animation comes
  at Stage 9b, two stages after physics and the character controller. Stage 10 ties streaming,
  which an open world needs, to networking, which the first game does not.

Some of it was not finished: the measurements of five quick wins (the order of the bind groups, an
opaque pipeline without `discard`, the passes of a submission, foliage left out of the farther
cascades, the CPU mipmaps of `Gpu::create_texture`), a run of an hour, and every check on Windows.
The owner decided on 2026-10-19, one question at a time.

## Decision

**An audit pass before Stage 6**, of five milestones that [`docs/PLAN.md`](../PLAN.md) details,
each with the defects it fixes and its exit criteria:

- **P1, saves and identities**: what the three critical defects need, and the defects that make a
  save, a scene or a level written back lose or change what it holds.
- **P2, a damaged file gives an error**: ADR-0096 applied to every reader.
- **P3, the loss of the GPU device and the main loop**: a recovery that covers a loss found
  anywhere, and a panic in `render` that stays an ordinary panic.
- **P4, the measured quick wins**, each measured before and after.
- **P5, the build, CI, and measurements tracked from one commit to the next**: the audit's
  benchmarks in the repository, a file of measurements per milestone, a trace that a profiler reads.

**A foundations pass after it, before Stage 6 is planned**, its milestone P6: decisions written as
ADRs, with little code, on what the editor will freeze:

- the execution model: the phases of a frame, where the engine's systems run, the priorities and
  budgets of background work; the parallel job system stays at Stage 7;
- change detection beyond transforms, changes deferred during a traversal, and lifecycle hooks;
- stable references, and operations that undo keeps reversible;
- how the editor runs a game's code: a game becomes a library and its binaries, which amends
  [ADR-0055](0055-games-in-the-workspace.md); playing in the editor starts the game in a child
  process first;
- the frame contract: a frame that the renderer owns, built from the world, which leaves room for a
  render thread;
- a study of open worlds: cells, identities, saves per scene, references across scenes;
- an asset request API with priorities, budgets and eviction;
- a minimum and a target hardware tier, with budgets scaled from the RX 6900 XT's measurements;
- what the GPU layer gives the editor: blending, lines, picking.

Each builds only what Stage 6 uses; the rest keeps its stage.

Neither pass has a release of its own: what they deliver ships with 0.5, the editor's release.

**The roadmap changes as follows.**

- **Stage 6 begins with frustum culling and instancing**, moved from Stage 9a, before the editor's
  viewport, built on the foundations pass's frame contract, hardware tier and GPU layer. Sorting the
  draws comes earlier, as a quick win of P4. Many lights, transparency and the effects stay at 9a.
- **Stage 8 no longer holds the C ABI and the C++ SDK**: they move to Stage 11, towards 1.0,
  derived from an API that the editor and the game will have proven. Stage 8 keeps module
  activation, plugins and scripting.
- **The core of skeletal animation moves to Stage 7**: skeletons, skinning, clips and blending.
  State machines, blend trees, inverse kinematics, retargeting and root motion stay at 9b.
- **Stage 10 is split in two**, as Stage 9 was: Stage 10a, Large worlds, released as 0.10, holds
  streaming, terrain, vegetation, large-world coordinates and rendering at very large scale; Stage
  10b, Networking, released as 0.11, holds networking and the dedicated server. Stage 11 is
  released as 0.12, then 1.0.

## Alternatives considered

- **Only the milestones that lose runs or crash (P1 to P3).** Shorter, but the quick wins and the
  tracked measurements would slip into Stage 6, mixed with the editor. **No pass**: the editor
  would build on identities that corrupt saves, and fixing them later would touch the editor too.
- **The foundation decisions as the first milestones of Stage 6**, or as each need arises: the
  editor's panels risk being built before the decisions they rest on, the risk that the audit
  pointed out.
- **Culling and instancing in the audit pass**: earlier, but built before the decisions that frame
  them, and likely built again. **At the end of the foundations pass**: framed, but a pass meant as
  decisions grows by a milestone of rendering code. **Kept at 9a**: the editor and larger levels
  would meet a ceiling of a few thousand objects a frame. **GPU-driven rendering as well**: too
  early, without a second version of the GPU layer and a persistent render scene.
- **A complete job system before the editor**: nothing measured asks for it; the measured lever is
  to prioritize and bound background work, which the execution model decides.
- **Stage 8 unchanged**: Stage 6 would invent a temporary way of running a game's code.
- **Splitting Stage 10 only, the animation kept at 9b**: the game's character would stay without
  animation until 9b. **Stage 10 kept whole**: streaming would wait for networking to be released.
- **A release at the end of the audit pass**: it changes APIs, so it would be 0.5, and every later
  release would shift by one, for a release with nothing new to show.

## Consequences

- Several milestones come before the editor starts, and Stage 6 starts with rendering work.
- Stage 7 grows by the core of animation; Stage 8 shrinks; Stage 11 gains the C ABI and the SDK.
- The release map gives 0.10 to Stage 10a, 0.11 to Stage 10b, and 0.12, then 1.0, to Stage 11; the
  stage numbers that documents and ADRs cite stay right, a reference to Stage 10 now meaning both
  halves.
- What the audit found that neither pass fixes is in the plan's known debt, each entry with the
  stage it waits for; what it found for later stages is weighed when each is planned (below).
- No NVIDIA or Intel GPU is tried for now, the owner having no other machine: the hardware tier is
  written from one GPU, which the known debt records.

What the audit leaves to later stages, to weigh when each is planned, none of it decided:

- **Stage 6**: an editor core without UI, with virtualized lists, and commands that tests and agents
  can drive; a harness that drives a game or the editor without a person (scripted input, world
  fingerprints, captures); what the platform lacks for an editor (clipboard, input methods,
  cursors, dropped files, dialogs, the raw motion of the mouse and its capture); written criteria
  for keeping egui, and the UI of games decided apart; one renderer per output, so that four views
  of 1280 × 720 take 508 MiB, a view to separate from the renderer before the viewport; what
  reflection lacks for an inspector; typed asset references and assets of a game's own types; one
  index of assets, the cooker's, for the asset browser; a valid change of a prefab that makes the
  levels placing it refuse, unchecked before saving; a level that holds in one text file, with its
  size limit and its merges in a team; the state that systems compute apart from the state an
  author writes, where `Unsaved` is too coarse.
- **Stage 7**: a channel of events (contacts, animation events, sounds, rules); the time of a game
  (step number, pause, slow motion, a step at a time) and a written contract of determinism;
  propagation of transforms on one thread, about 35 ns per entity moved; physics, comparing Jolt
  and Rapier in double precision; the profiler's 7 spans written as log lines, to instrument and
  export.
- **Stage 8**: typed access to components without allocation for scripts and the C ABI, the
  reflected `Value` costing about 6 µs and 180 allocations to read, and reflection quadratic in the
  number of fields; restarting and resuming a game as a service of the engine, where code changes
  take 1.5 to 2 s plus the replay of a save.
- **Stage 9a**: multisampling wired through the whole frame while what follows (temporal
  anti-aliasing, upscalers) is temporal, to decide when 9a is planned; a render scene kept from one
  frame to the next, with stable keys; the camera subtracted on the CPU per object; the render
  graph grown (buffers, compute, history, insertion points); output to HDR displays; one bind group
  per material and WGSL without modules, WESL to add to the candidates of 9b; wgpu re-evaluated
  with a measurement on Direct3D 12; shadow culling, each object tested against every cascade and
  spot light on one thread.
- **Stage 9b**: archives, asynchronous reading, patches and downloadable content; rate-distortion
  optimized encoding, supercompression and quantized vertices; a cooker that decodes and builds
  mipmaps on one thread, rereads every source at every cook, and encodes BC7 at 4.1 megapixels a
  second on 16 threads.
- **Stage 10a**: a save that costs the whole world rather than what changed (0.72 s for 100 000
  entities of which none moved); a world that keeps forever every scene spawned under a key, and a
  scene library that never forgets one; 2.5 to 3.6 KB of memory per entity of a scene, where a
  typed spawn takes 380 bytes; a spawned scene 8 to 10 times slower than typed spawns; destroying an
  entity at a cost per component type the world ever met; reading a save's header parses the whole
  save; streaming on one file per asset, four reads in flight, without priorities or eviction; hot
  reload at a cost proportional to the whole project; the manifest one block, read whole; the
  world's storage, which keeps 4 MB per component type per million slots and never shrinks; masses
  of vegetation, each plant an entity.
- **Stage 10b**: two rules that keep networking possible at little cost, for the execution model of
  the foundations pass and Stage 7 to weigh: all of a game's state lives in the world, and systems
  run by numbered steps, from the commands of their step.
