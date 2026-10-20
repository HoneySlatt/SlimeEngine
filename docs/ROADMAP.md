# Engine Roadmap

> Living document. It defines direction, ordering and exit criteria. It is not a task list:
> the current plan lives in `docs/PLAN.md`, and architecture decisions live in `docs/adr/`.
>
> This is an overview, not a commitment. Not everything here will be built: stages will be
> merged, reduced, reordered or dropped as the project evolves, and each such change is
> recorded in an ADR.

## 1. Vision

A general-purpose, modular, open-source (MPL-2.0) creation platform whose runtime is written in Rust, for 3D games first.

Long-term targets, in order of reach:

1. Linear narrative games
2. Open-world games
3. Large online worlds

The project is a complete platform (runtime, asset pipeline, editor, SDK), not a renderer with extras. A native, full-featured editor is a first-class component.

3D comes first. 2D and stylized games are made with the same renderer, through pieces that the stages add where they are needed: an orthographic camera, sprites, transparency, unlit and custom materials, crisp sampling for pixel art. There is no separate 2D renderer (ADR-0052).

The games the engine is weighed against, as profiles (ADR-0098). The games played alone come first; the online games come after them.

- *Hunna Wood*, the sample: a realistic third-person role-playing game in one open world without loading screens, centred on a story whose choices have consequences, with demanding combat and a horse to ride.
- A space opera across several hand-made planets, played on foot in first or third person and at the controls of cars, motorbikes and spaceships, with loadings the player does not notice, a ship to walk in while it flies, and choices that destroy places.
- A linear, turn-based narrative role-playing game in anime-style 3D, toon-shaded everywhere, with menus of a strong identity.
- A hand-drawn 2D metroidvania: a large map of connected rooms and precise controls.
- A painted action role-playing game with a party, whose combat slows time while orders are given.
- A first-person photorealistic horror game in one small place that changes at each pass.
- A massively multiplayer online role-playing game told in expansions, with about 500 players in one place and few instances.
- A competitive first-person shooter on dedicated servers, at high frame rates on modest computers.
- Online co-op games hosted by a player, with physics-driven characters, vehicles and proximity voice chat.

Each style aims at the best rendering possible: realistic, anime, painted or hand-drawn. Each game sets *visual targets*, reference images on its target hardware at its frame rate, which principle 9 measures against. The engine runs on *hardware tiers*, from modest computers at high frame rates to high-end ones; a game picks its minimum tier. No mobile platform is aimed at.

## 2. Architecture

| Layer | Contents | Boundary below |
|---|---|---|
| Games | Rust, C++, scripting, visual scripting, data | Public engine API |
| SDK | C++ wrapper, plugin API, script bindings | Stable C ABI (promised from 1.0) |
| Engine (Rust) | core, platform, gfx, render, assets, reflect, world, jobs, physics, audio, animation, net | Internal Rust API (unstable) |
| Foundation | OS, wgpu, then native backends (Vulkan / D3D12, Stage 9c), middleware | |

The editor sits beside games, not inside the engine: it is a client of the engine's public systems and never reaches into internals.

## 3. Principles

1. **Dependency order beats feature appeal.** A system is built when what it depends on is solid.
2. **Measure before optimising.** No custom containers or allocators without data showing the need.
3. **No abstraction without a second concrete use.**
4. **Every stage ends with a runnable sample.** An engine that is never used to build something drifts. From Stage 3c on, the sample is one game, grown by each stage; a stage may replace it, with an ADR, as Stage 4a replaces *Nightwatch* with *Hunna Wood* (ADR-0067).
5. **Headless is a normal configuration.** The engine must run without window, renderer or audio. This is what makes CI tests and dedicated servers possible.
6. **Instrumented from day one.** Profiling spans and GPU markers are added as systems are written, not retrofitted.
7. **Existing crates are used wherever they are not the point of the engine,** and wrapped at the boundary so they can be replaced.
8. **Stability is promised in one place only:** the C ABI, from 1.0. Everything else may break during 0.x.
9. **Techniques enter because they solve a measured problem,** not to match another engine's feature list. For rendering, the measured problem is the gap between a game's image and its visual targets (section 1, ADR-0098).
10. **What a person does, an agent does.** Every operation of the editor and of a running game that a person performs is a command that an AI agent can call through the engine's MCP server, the same command, not a path of its own (ADR-0099).

## 4. Decisions

### Fixed

| Topic | Decision |
|---|---|
| Engine language | Rust |
| License | MPL-2.0 |
| GPU abstraction | wgpu, then native backends, Vulkan and Direct3D 12, at Stage 9c (ADR-0098). Their types stay inside the `gfx` and `render` crates, and the dev overlay, tooling (ADR-0031); the rest of the engine uses engine-owned handles. |
| Shaders | WGSL to start |
| First platforms | Linux (Wayland and X11) and Windows: games run on both from Stage 2 on. macOS comes later. No mobile platform (ADR-0098). |
| UI | One framework of the engine's own, GPU-rendered, for the editor and the UI of games; egui only for the dev overlay, tooling. No Qt or GTK. (ADR-0098) |
| Game-facing API | Rust first. The C ABI and C++ SDK are derived later from a proven API. |
| Windowing and input | winit, used only inside `platform`; its types never leave that crate (ADR-0011) |
| Entity model | The engine's own: entities, components stored by type, and a transform hierarchy, with `f64` translations (ADR-0079) |
| Serialization format | Driven by the engine's own reflection, without serde: TOML for text, sources and saves; binary formats of the engine's own for cooked data (ADR-0069, ADR-0080) |

### Open

Each is decided by ADR at the stage where it becomes necessary, not earlier.

| Topic | Candidates | Decide at |
|---|---|---|
| Design of the UI framework | Retained, immediate or hybrid; the crates it builds on for text, layout and accessibility (ADR-0098) | Stage 6 |
| Physics backend | rapier3d, Jolt or other, behind an abstraction, against the needs of ADR-0098 | Stage 7 |
| Audio backend | To evaluate, against the needs of ADR-0098 | Stage 7 |
| Scripting language | Lua, C#, dedicated VM, other | Stage 8 |
| Shader language beyond WGSL | Slang, generated WGSL, with the native backends of Stage 9c in view | Stage 9b |
| Network transport | To evaluate | Stage 10b |

## 5. Repository layout

Indicative. The structure in use is described in `docs/ARCHITECTURE.md` and its rules in ADR-0004; a directory is created when its first content exists.

```
engine/
├── crates/          engine crates (core, platform, gfx, render, assets, ...)
├── editor/          editor application and editor crates
├── sdk/             C ABI and C++ wrapper (Stage 11)
├── tools/           asset cooker, CLI, profiler
├── examples/        one runnable sample per stage
├── tests/           integration and image-comparison tests
├── docs/            ROADMAP.md, PLAN.md, adr/
└── flake.nix        reproducible dev shell
```

## 6. Stages

Stages are ordered by dependency. They overlap in practice, and carry no dates on purpose.

### Stage 0 — Foundations

**Goal.** Fix the rules that must survive for the life of the project.

- Cargo workspace, crate naming, edition, MSRV, lints
- Nix dev shell
- CI: build, test, clippy, fmt, documentation (rustdoc warnings, doc tests, Markdown links), dependency licenses and security advisories
- ADR process and first ADRs
- Dependency policy (admission criteria, license compatibility with MPL-2.0)
- `unsafe` policy
- Documentation baseline: README, CONTRIBUTING, architecture overview, glossary, one README per crate

**Exit.** CI is green on a workspace that contains the skeletons of the Stage 1 crates; the policies above exist as ADRs; the documentation baseline exists and CI checks it.

### Stage 1 — Core and platform → 0.0.1

**Goal.** A very small application that is architecturally sound.

- Conventions for errors, logging, assertions and panic reporting
- `platform`: window and input, behind an engine-owned API
- Application lifecycle: create → run loop → shutdown, with the ownership of the main loop decided by ADR
- Frame timing (fixed and variable step)
- A frame limiter, until presenting to the screen paces frames (Stage 2)
- Profiling spans in the main loop
- Headless run mode
- System information logged at startup

A Wayland window shows nothing until the application gives it pixels, and presenting pixels belongs to Stage 2. The Stage 1 sample therefore fills its window with a solid color from the CPU (softbuffer). This dependency is limited to the sample and is removed in Stage 2.

**Not yet.** Rendering, scenes, entities, physics. IDs and handles (Stage 2, with their first user). Configuration files and a virtual filesystem (when a stage needs them, ADR-0068).

**Exit.** A window opens on Wayland, input events are logged, shutdown is clean. The same binary runs N frames headless in CI.

### Stage 2 — GPU layer → 0.0.2

**Goal.** Own the boundary between the engine and wgpu.

- Device, surface, resize, present (replaces the Stage 1 CPU fill), on Linux (Vulkan) and Windows (Direct3D 12)
- Frame pacing, decided by measurement on each platform: on Wayland compositors without the FIFO protocol, presentation can stall while the window is hidden, so the Stage 1 frame scheduler may stay
- Engine-owned resource types for what the triangle and the offscreen render use; their representation is decided by ADR, and the set grows with the renderer in Stage 3
- WGSL shaders compiled from source text, with readable errors (loading from files and hot reload come with the asset pipeline, Stage 4b)
- GPU debug labels and markers
- Offscreen rendering

`gfx` does not depend on `platform`: it accepts window handles through the `raw-window-handle` traits, so that offscreen rendering and its tests run without a window.

**Exit.** A triangle renders and survives resize, on Linux and on Windows. An offscreen render is compared to a reference image in CI. CI has no GPU: the image is rendered with software drivers, Mesa lavapipe on Linux and WARP (the software Direct3D 12 driver) on Windows, and the comparison uses a tolerance so that the same test passes on hardware drivers.

Stage 3 is split in two so that each half ends with a runnable, tested sample.

### Stage 3a — First renderer → 0.0.3

**Goal.** Render a textured scene with a simple lighting model.

- Meshes, camera, depth, textures, a basic material and a directional light
- Render statistics and GPU timings
- Dev overlay (egui, tooling only, until something does at least as well: ADR-0052)
- Temporary direct glTF loading

`egui-wgpu` is built against a specific wgpu version: while the engine's tooling draws with egui, upgrading wgpu waits for egui.

**Exit.** A glTF scene renders with textures, depth and simple lighting under a moving camera, with statistics shown in the overlay, covered by image tests.

### Stage 3b — Physically based renderer → 0.1

**Goal.** Render a simple scene correctly.

- PBR materials, light types, image-based lighting, shadows
- HDR, tone mapping, basic post-processing
- Minimal render graph: declared passes and resources, automatic ordering. It is extracted from the concrete passes at the end of the stage, once at least three of them exist, not designed ahead of them.

**Exit.** A sample scene with camera, lights, PBR materials and shadows, covered by image tests.

### Stage 3c — A first game

**Goal.** Use the engine as a game does, before the asset formats and the world model are designed.

- A small game that uses the public API only: the player moves, interacts with a few objects and reaches an objective, and the game restarts cleanly
- What the game finds hard to do, recorded as proposals for Stages 4 and 5

The stage comes right after 0.1 (it was added by ADR-0052) and has no release of its own. Its game, *Nightwatch*, was the sample until Stage 4a replaced it with *Hunna Wood*, and was never released (ADR-0067).

**Not yet.** Text, physics, sound: the engine does not have them.

**Exit.** The game is played to its objective and restarted, on Linux and on Windows, through the public API only.

### Stage 3d — Anti-aliasing and the shadows of spot lights

**Goal.** Remove the two defects of the renderer that the first game showed (ADR-0056).

- Multisample anti-aliasing (four samples) on the scene pass
- Shadow maps for spot lights: every spot light drawn may cast shadows, each through a tile of one atlas, sized by how many cast them (ADR-0058)
- The cost of the scene pass on Direct3D 12, measured with FXC and with DXC against Vulkan's (ADR-0057)
- A smoother filter for the shadows of every light (ADR-0063)
- Cascaded shadow maps for the directional light, moved from Stage 9a (ADR-0064)

The stage comes right after Stage 3c and has no release of its own; what it delivers is released with 0.2.

**Exit.** In the game, walls and crates stop the light of flashlights and floodlights, and edges are smooth, on Linux and on Windows; image tests cover both.

Stage 4, the asset pipeline, is split in two halves (ADR-0068): a reference to Stage 4 means both.

### Stage 4a — Cooked assets → 0.2

**Goal.** Replace "load a file at startup" with a real pipeline: source → import → engine asset → cook → runtime data.

- Stable asset IDs (the path is not the identity)
- Metadata, dependency tracking
- Importers: glTF and images; audio comes with Stage 7 (ADR-0068)
- Tangents computed, with MikkTSpace, for a mesh that has none (ADR-0057)
- Textures compressed for the GPU (ADR-0069)
- Cooked cache, incremental rebuilds
- The sample game becomes *Hunna Wood*, a role-playing game seen in the third person, which loads its first zone from cooked assets (ADR-0067)

**Exit.** *Hunna Wood* loads its zone from cooked assets and is played by day and by night, on Linux and on Windows. The temporary loader is deleted. Moving a source file breaks no reference.

### Stage 4b — Live assets

**Goal.** Loaded assets arrive in the background, change while the game runs, and outlive the GPU device.

- Asynchronous loading on a basic task pool, the renderer's own (ADR-0072); loading threads create GPU objects through the `Gpu` they share
- Hot reload, and materials whose values change in place, for it and for a game (ADR-0057)
- Recovering from a lost GPU device, for the applications that choose it: the GPU objects are created again from their sources, where a loss stopped the run (ADR-0022, ADR-0075)

The stage comes right after Stage 4a and has no release of its own; what it delivers is released with 0.3.

**Exit.** A texture and a shader hot-reload in the running game. The game's zone loads while frames go on. The game goes on after a lost GPU device.

Stage 5 is split in two halves (ADR-0078): a reference to Stage 5 means both.

### Stage 5a — The world → 0.3

**Goal.** The engine understands its own types and keeps the game's world: what it holds, what it draws, and what a save restores. Reflection is a backbone: serialization, inspector, scripting, replication, undo/redo and visual scripting all depend on it.

- Type registry and reflection, with a derive macro, both the engine's own (ADR-0080)
- Serialization with format versioning, driven by reflection: TOML for text (ADR-0080)
- The engine's own entity model: entities, components, a transform hierarchy (ADR-0079)
- The world keeps what is drawn, and the transforms of what moves at its last two fixed steps, drawn between them: a game changes only what moves (ADR-0057)
- The precision of world positions: `f64` translations, `f32` for the rest, the renderer drawing relative to the camera (ADR-0048, ADR-0079)
- Entity lifetime and stable references
- Saves: what differs between a world and the scenes it was made from (ADR-0079)

The stage comes right after Stage 4b, whose work is released with it.

**Exit.** *Hunna Wood* runs on the world, which keeps what is drawn and where what moves was. Any component's properties can be listed and edited generically. A run of the game round-trips through save and load.

### Stage 5b — Scenes and prefabs → 0.4

**Goal.** Worlds are written in files: scenes, and prefabs placed many times, each placement with values of its own.

- Scene files: text sources with asset IDs, cooked into a binary encoding of the engine's own (ADR-0080, ADR-0089)
- Prefabs placed with overrides, which follow the changes of their prefab, and placements nested (ADR-0089)
- A world, or a part of it, written back as a scene file that keeps its placements, as the editor will save a level (ADR-0089)
- Typed components on the nodes of glTF scenes, read through reflection (ADR-0089)
- What a game spawns while it runs saved as placements of its prefabs (ADR-0089)
- The sample game's zone made of prefabs (ADR-0090)

The stage comes right after Stage 5a, and is released as 0.4 (ADR-0090).

**Exit.** The game's scenes round-trip through save and load as files. Prefabs instantiate with overrides. The sample game's zone is made of prefabs.

Between Stage 5b and Stage 6 come two passes, without a release of their own (ADR-0097): an audit pass, which fixes what an audit of 0.4 found in saves, identities, the readers of files (ADR-0096), the recovery from a lost GPU device, and measured quick wins, and puts the engine's measurements in CI; then a foundations pass, which decides as ADRs what the editor will freeze: the execution model, change detection, stable references and reversible operations, how the editor runs a game's code, the frame contract, a study of open worlds, an asset request API, hardware tiers, and what the GPU layer gives the editor, each weighed against the games of section 1 (ADR-0098) and against AI agents driving the engine (ADR-0099). `docs/PLAN.md` details both.

### Stage 6 — Editor foundation → 0.5

**Goal.** A real editor, built as a client of the engine.

- Frustum culling and instancing, moved from Stage 9a (ADR-0097), first, before the viewport, on the decisions of the foundations pass
- The execution model (ADR-0107), before play in editor: one task pool of the engine's, with a priority for the frame and one for the background; the copies to the GPU submitted by one owner within a budget of bytes per frame; the phases of a frame, the world read only while it is drawn; game time apart from real time, with pause and one step; a game as a simulation and a presentation, run by a host
- What the world knows of its changes (ADR-0108), before the hierarchy and the inspector: a tick on every value of a component, read since a reader's last tick, with summaries, a log of removals and a structure counter; components that clone; the mark of driven components, before the editor writes levels
- The engine's UI framework, as far as the editor needs it: text in several writing systems, layout, docking, widgets, input routing, focus, drag and drop, theming (ADR-0098)
- Viewport, hierarchy, inspector (driven by reflection), asset browser
- Command and transaction system: every edit is undoable
- Play in editor, the game running in a child process first, as the foundations pass decides (ADR-0097)
- The engine's MCP server: AI agents read the world, act through the editor's commands, capture views, read measurements and logs, cook, run tests and play the game with scripted input; reachable from the user's machine only, absent from shipped builds (ADR-0099)
- The keys and buttons held, kept by the engine, if the viewport's camera moves with them, with the keyboard shared between the editor's UI and the game; at Stage 7 otherwise (ADR-0057)

**Exit.** A level of the game is built, saved and reloaded entirely in the editor, with undo/redo on every operation. The editor uses no private engine API.

### Stage 7 — Runtime systems → 0.6

**Goal.** What a game needs beyond rendering, built on a proper concurrency model.

- Job system, grown from Stage 6's task pool (ADR-0107): dependencies, loops split across threads, work stealing if a measurement asks for it
- Hooks per component type, messages read at fixed phases, and a queue of the changes that parallel systems defer, designed with physics, audio and the events of games (ADR-0108)
- Memory: frame, arena and pool allocators where measurements justify them; allocation tracking
- Visual profiler: CPU, jobs, memory, GPU, frames, from a trace stream
- Physics behind an engine abstraction: bodies, colliders, constraints, queries (a ray or a segment against the world, ADR-0057), character controller; the backend chosen against vehicles (wheels and suspension, motorbikes, free flight), walking inside a moving vehicle, 2D, characters driven by physics, worlds in double precision and a simulation that runs the same way twice (ADR-0098)
- Riding: a character attached to a mount or a seat, with its animations, input and camera (ADR-0098)
- Audio: sources, listener, spatialization, buses, mixing, streaming; the importer of audio files, moved from Stage 4 (ADR-0068); the backend chosen against sound placed precisely around the listener's head, reverberation per room and sounds muffled by walls (ADR-0098)
- The core of skeletal animation: skeletons, skinning, clips, blending, moved from Stage 9b (ADR-0097); poses that are not entities, and bones that things are attached to (ADR-0108)
- Input action mapping, and the keys and buttons held if Stage 6 did not need them
- UI of games: text, menus, HUD, on the engine's UI framework, grown with animation, transforms, masks, effects on text and three-dimensional elements (ADR-0098)

**Exit.** The game has an animated character in a physical level, with sound and a UI, and is profiled end to end.

### Stage 8 — Extensibility → 0.7

**Goal.** Others can extend the engine without modifying it.

- Module activation: a game enables only what it needs
- Plugin architecture for runtime and editor
- Scripting (language decided here), bound through reflection, with reload and debugging

The C ABI and the C++ SDK moved to Stage 11, derived from the API that the editor and the game will have proven (ADR-0097).

**Exit.** The game's gameplay lives in a plugin using only public API.

Stage 9 is split in two (ADR-0052): the narrative vertical slice of Stage 9b needs a production renderer first.

### Stage 9a — Production renderer → 0.8

**Goal.** Render what a shipped game shows, beyond a simple scene.

- Shadows: shadows of point lights, the tiles of spot lights' shadows sized by how large each light shows on screen (ADR-0058)
- Many lights, by a method that keeps several ways of lighting possible in one frame: clustered forward is expected, chosen here by measurement (ADR-0098)
- Transparency
- Temporal anti-aliasing
- Fog, ambient occlusion, decals
- A day-night cycle
- The pieces for 2D and stylized games that no earlier stage needed (section 1)
- The look of a game: how each material is lit, passes of the game's own, its tone-mapping curve and its color grading (ADR-0098)
- The stability of the image as a measured goal: no shimmering, no trails, no detail that pops in (ADR-0098)
- Skin lit through subsurface scattering (ADR-0098)
- Quality set per hardware tier (section 1, ADR-0098)

**Exit.** A level of the game, outdoors and indoors, with a day-night cycle, many lights, transparency and anti-aliasing, within the frame budgets tracked for the sample, covered by image tests.

### Stage 9b — Content production → 0.9

**Goal.** A team can produce a narrative game.

- Animation beyond the core that Stage 7 builds (ADR-0097): state machines, blend trees, IK, retargeting, root motion; motion matching and quadrupeds; 2D animation (ADR-0098)
- Faces through blend shapes, and lip sync; simple secondary motion of hair, cloth and flesh (ADR-0098)
- Material graph → IR → shader generation; values set per instance (ADR-0057); variants, permutation cache, async compilation
- Sequencer: camera, animation, audio, event and VFX tracks
- VFX: CPU and GPU particles
- Navigation: navmesh, pathfinding, spatial queries; the AI of characters beside it (ADR-0098)
- Dialogue and quests, with the state of a story that they read and that scenes depend on; the rules of a particular game stay in the game (ADR-0098)
- Translation tables and voice (ADR-0098)
- Build, cook and package: Development, Shipping, Server, Editor configurations, for Linux and Windows. Builds without a console (Windows games) write their log to a file.
- Updates of shipped games: archives, patches that download only what changed, downloadable content and expansions as packages added to an installed game; from its first release, a save written by a shipped version of a game loads in all of its later versions, checked in CI with one save fixture per shipped version (ADR-0100)

**Exit.** The game grows into a short narrative vertical slice (animated character, cutscene, gameplay) that ships as a packaged build.

The stage is split in two halves when it is planned, if it needs to be (ADR-0098).

Stage 9c is added by ADR-0098: the rendering of the games of section 1, and consoles one day, need native graphics APIs.

### Stage 9c — Native GPU backends

**Goal.** Give the renderer the features of the GPU that wgpu lacks or holds as experimental.

- Vulkan and Direct3D 12 backends behind the engine-owned API of `slime-gfx`
- Work on several queues of the GPU in parallel, ray tracing, mesh shaders, resources without a declaration per binding, explicit memory, upscalers and frame generation, the vendors' tools for latency and GPU crashes
- Whether wgpu stays: as a fallback, for debugging, or for macOS

Before this stage, a feature that wgpu blocks goes through wgpu's access to the API underneath it. The stage comes right after Stage 9b and has no release of its own; what it delivers is released with 0.10.

**Exit.** The game runs on the native backends on Linux and on Windows, and the image tests pass on them as on wgpu.

Stage 10 is split in two (ADR-0097): streaming, which an open world needs, does not wait for networking, which the first game does not need.

### Stage 10a — Large worlds → 0.10

**Goal.** Scale beyond a single level.

- World streaming: cells, async I/O, priorities, memory budgets, HLOD
- Terrain, vegetation and their editing tools
- Large-world coordinates
- High-scale rendering: GPU-driven rendering, advanced culling, virtual texturing, virtual geometry, global illumination, ray tracing. Each is admitted against a measured problem, on the native backends of Stage 9c.
- Crowds and traffic, with hundreds of characters drawn (ADR-0098)
- Terrain that deforms, hair drawn strand by strand, simulated cloth (ADR-0098)
- Water and weather (ADR-0098)

The stage is split in two halves when it is planned (ADR-0098).

**Exit.** The game streams an open world.

### Stage 10b — Networking → 0.11

**Goal.** Scale beyond a single machine.

- Networking: transport, replication, RPC, prediction, reconciliation, lag compensation, interest management for hundreds of players in one place (ADR-0098)
- Synchronized physics, proximity voice chat, games hosted by a player with relays, a world split between servers (ADR-0098)
- The threat model of ADR-0096 extended to the network (ADR-0098)
- The data that servers keep for players converted from one version of a game to the next (ADR-0100)
- Dedicated server build

The stage is split in two halves when it is planned (ADR-0098).

**Exit.** Several clients play the game connected to a dedicated server.

### Stage 11 — Production readiness → 0.12, then 1.0

**Goal.** External projects can rely on the engine.

- Complete editor: material, animation, VFX and audio editors, sequencer, profiler, build tools, plugin manager, saved layouts, multi-window, command palette
- Editor extensibility: custom panels, inspectors, importers, asset types, build steps
- C ABI: opaque handles, versioned structures, function tables, explicit ownership, explicit error codes; moved from Stage 8 (ADR-0097)
- C++ SDK as an ergonomic wrapper over the C ABI, with a C++ sample that builds against it
- Crash reporting and diagnostics
- Migration tooling for assets, projects and serialized formats
- Deprecation policy
- macOS as a supported platform
- User, SDK and engine documentation

**Exit for 0.12.** An external project is built without touching engine source.
**Exit for 1.0.** A compatibility promise can be made and kept. There is no deadline for this.

## 7. Continuous tracks

These run through every stage. They are never "a phase for later".

| Track | Content |
|---|---|
| Testing and CI | Unit, integration, image comparison, serialization round-trip, performance regression |
| Platforms | Every stage's sample builds and its tests pass in CI on Linux and Windows; windowed behaviour is checked by hand on both |
| Profiling | Instrumentation added with each system |
| Performance budgets | CPU frame, GPU frame, memory, VRAM, load times, tracked per sample |
| Iteration speed | Compile times, hot reload, startup time |
| Diagnostics | Logs, assertions, crash context |
| Documentation | ADRs, architecture notes, API docs, sample walkthroughs |
| Samples | One runnable example per stage, kept working; from Stage 3c on, the game that each stage grows |
| Agents | From Stage 6 on, each system exposed to AI agents through the engine's MCP server as it is written (ADR-0099) |

## 8. Horizon

Direction only. None of this is planned work until Stage 11 is reached.

- **Online platform for MMO-scale games.** Distinct from the engine: gateway, authentication, persistence, world and instance services, chat, economy, group finding, anti-cheat, observability, deployment. The engine supplies serialization, replication, headless servers and world partitioning.
- **Consoles.** Only with official SDK access, and hardware to test on. Built on the native backends of Stage 9c (ADR-0098).
- **Large-team workflows.** Multi-user editing, asset locking, distributed builds, shared derived-data cache.
- **Ecosystem.** Plugin registry, templates, asset packages. After users exist, not before.
- **Advanced world simulation.** Destruction, procedural generation. Crowds, traffic, water and weather moved to Stage 10a (ADR-0098).
- **Visual scripting.** Node graph → IR → VM or native code, once reflection, scripting and undo/redo are mature.
- **Research track.** Paper → prototype → benchmark → production evaluation → merge.

## 9. Dependencies between systems

| System | Requires |
|---|---|
| platform | no engine crate (ADR-0004) |
| gfx | color only (ADR-0079); window handles through the `raw-window-handle` traits, not `platform` |
| render | gfx, assets, color: it loads its own kinds of asset (ADR-0069), on a task pool of its own from Stage 4b (ADR-0072), the engine's from Stage 6 (ADR-0107). Not world: `slime-world-render`, a bridge between them, builds each frame from the world (ADR-0079) |
| assets | reflect only (ADR-0069, ADR-0080) |
| color | reflect only (ADR-0079) |
| reflect | its derive macro only (ADR-0080) |
| serialization | reflect |
| world | reflect, serialization, assets, color |
| editor | world, render, assets, UI framework |
| UI of games | render, input action mapping, UI framework |
| undo/redo | reflect, editor commands |
| physics, audio, animation, navigation | world |
| riding | physics, animation, input action mapping |
| AI of characters | world, navigation |
| dialogue and quests | world, serialization |
| plugins, C ABI, scripting | reflect, stable module boundaries |
| material graph | render, reflect, UI framework |
| streaming | world, assets, jobs, profiler |
| networking | reflect, serialization, world, headless mode |
| visual scripting | scripting, reflect, undo/redo |

## 10. Release map

| Version | Stage | Content |
|---|---|---|
| 0.0.1 | 1 | Core and platform |
| 0.0.2 | 2 | GPU layer |
| 0.0.3 | 3a | First renderer |
| 0.1 | 3b | Physically based renderer |
| — | 3c | A first game, never released: replaced at Stage 4a (ADR-0067) |
| — | 3d | Anti-aliasing and the shadows of spot lights, released with 0.2 |
| 0.2 | 4a | Cooked assets |
| — | 4b | Live assets, released with 0.3 |
| 0.3 | 5a | Reflection, serialization, world |
| 0.4 | 5b | Scenes and prefabs |
| — | — | An audit pass and a foundations pass, released with 0.5 (ADR-0097) |
| 0.5 | 6 | Editor |
| 0.6 | 7 | Runtime systems |
| 0.7 | 8 | Plugins, scripting |
| 0.8 | 9a | Production renderer |
| 0.9 | 9b | Content production |
| — | 9c | Native GPU backends, released with 0.10 |
| 0.10 | 10a | Large worlds |
| 0.11 | 10b | Networking |
| 0.12 | 11 | Usable by external projects |
| 1.0 | 11 | Stable API and SDK |
