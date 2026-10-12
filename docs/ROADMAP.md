# Engine Roadmap

> Living document. It defines direction, ordering and exit criteria. It is not a task list:
> the current plan lives in `docs/PLAN.md`, and architecture decisions live in `docs/adr/`.
>
> This is an overview, not a commitment. Not everything here will be built: stages will be
> merged, reduced, reordered or dropped as the project evolves, and each such change is
> recorded in an ADR.

## 1. Vision

A general-purpose, modular, open-source (MPL-2.0) 3D creation platform whose runtime is written in Rust.

Long-term targets, in order of reach:

1. Linear narrative games
2. Open-world games
3. Large online worlds

The project is a complete platform (runtime, asset pipeline, editor, SDK), not a renderer with extras. A native, full-featured editor is a first-class component.

## 2. Architecture

| Layer | Contents | Boundary below |
|---|---|---|
| Games | Rust, C++, scripting, visual scripting, data | Public engine API |
| SDK | C++ wrapper, plugin API, script bindings | Stable C ABI (promised from 1.0) |
| Engine (Rust) | core, platform, gfx, render, assets, reflect, world, jobs, physics, audio, animation, net | Internal Rust API (unstable) |
| Foundation | OS, wgpu (Vulkan / D3D12 / Metal), middleware | |

The editor sits beside games, not inside the engine: it is a client of the engine's public systems and never reaches into internals.

## 3. Principles

1. **Dependency order beats feature appeal.** A system is built when what it depends on is solid.
2. **Measure before optimising.** No custom containers or allocators without data showing the need.
3. **No abstraction without a second concrete use.**
4. **Every stage ends with a runnable sample.** An engine that is never used to build something drifts.
5. **Headless is a normal configuration.** The engine must run without window, renderer or audio. This is what makes CI tests and dedicated servers possible.
6. **Instrumented from day one.** Profiling spans and GPU markers are added as systems are written, not retrofitted.
7. **Existing crates are used wherever they are not the point of the engine,** and wrapped at the boundary so they can be replaced.
8. **Stability is promised in one place only:** the C ABI, from 1.0. Everything else may break during 0.x.
9. **Techniques enter because they solve a measured problem,** not to match another engine's feature list.

## 4. Decisions

### Fixed

| Topic | Decision |
|---|---|
| Engine language | Rust |
| License | MPL-2.0 |
| GPU abstraction | wgpu. Its types stay inside the `gfx` and `render` crates; the rest of the engine uses engine-owned handles. |
| Shaders | WGSL to start |
| First platform | Linux (Wayland and X11) |
| Game-facing API | Rust first. The C ABI and C++ SDK are derived later from a proven API. |
| Editor UI | Engine-owned, GPU-rendered UI framework (no Qt/GTK) |

### Open

Each is decided by ADR at the stage where it becomes necessary, not earlier.

| Topic | Candidates | Decide at |
|---|---|---|
| Windowing and input | winit behind the platform layer, or direct Wayland/X11 | Stage 1 |
| Entity model | hecs, bevy_ecs, custom ECS, or non-ECS | Stage 5 |
| Serialization format | Text for sources, binary for cooked data | Stage 4–5 |
| UI framework architecture | Retained, immediate, or hybrid | Stage 6 |
| Physics backend | rapier3d or other, behind an abstraction | Stage 7 |
| Audio backend | To evaluate | Stage 7 |
| Scripting language | Lua, C#, dedicated VM, other | Stage 8 |
| Shader language beyond WGSL | Slang, generated WGSL | Stage 9 |
| Network transport | To evaluate | Stage 10 |

## 5. Repository layout

Indicative. The structure in use is described in `docs/ARCHITECTURE.md` and its rules in ADR-0004; a directory is created when its first content exists.

```
engine/
├── crates/          engine crates (core, platform, gfx, render, assets, ...)
├── editor/          editor application and editor crates
├── sdk/             C ABI and C++ wrapper (Stage 8)
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

**Not yet.** Rendering, scenes, entities, physics. IDs and handles (Stage 2, with their first user). Configuration files and a virtual filesystem (Stage 4).

**Exit.** A window opens on Wayland, input events are logged, shutdown is clean. The same binary runs N frames headless in CI.

### Stage 2 — GPU layer → 0.0.2

**Goal.** Own the boundary between the engine and wgpu.

- Device, surface, resize, present, frame pacing (replaces the Stage 1 frame limiter and CPU fill)
- Engine-owned handles for the resources that the triangle and the offscreen render use; the set grows with the renderer in Stage 3
- WGSL loading (hot reload comes with the asset pipeline, Stage 4)
- GPU debug labels and markers
- Offscreen rendering

`gfx` does not depend on `platform`: it accepts window handles through the `raw-window-handle` traits, so that offscreen rendering and its tests run without a window.

**Exit.** A triangle renders and survives resize. An offscreen render is compared to a reference image in CI. CI has no GPU: the image is rendered with a software Vulkan driver (Mesa lavapipe), reference images are produced with that driver, and the comparison uses a tolerance so that the same test passes on hardware drivers.

Stage 3 is split in two so that each half ends with a runnable, tested sample.

### Stage 3a — First renderer → 0.0.3

**Goal.** Render a textured scene with a simple lighting model.

- Meshes, camera, depth, textures, a basic material and a directional light
- Render statistics and GPU timings
- Dev overlay (egui, tooling only, temporary)
- Temporary direct glTF loading

`egui-wgpu` is built against a specific wgpu version: while the overlay exists, upgrading wgpu waits for egui.

**Exit.** A glTF scene renders with textures, depth and simple lighting under a moving camera, with statistics shown in the overlay, covered by image tests.

### Stage 3b — Physically based renderer → 0.1

**Goal.** Render a simple scene correctly.

- PBR materials, light types, image-based lighting, shadows
- HDR, tone mapping, basic post-processing
- Minimal render graph: declared passes and resources, automatic ordering. It is extracted from the concrete passes at the end of the stage, once at least three of them exist, not designed ahead of them.

**Exit.** A sample scene with camera, lights, PBR materials and shadows, covered by image tests.

### Stage 4 — Asset pipeline → 0.2

**Goal.** Replace "load a file at startup" with a real pipeline: source → import → engine asset → cook → runtime data.

- Stable asset IDs (the path is not the identity)
- Metadata, dependency tracking
- Importers: glTF, images, then audio
- Cooked cache, incremental rebuilds
- Asynchronous loading on a basic task pool
- Hot reload

**Exit.** The Stage 3 scene loads from cooked assets and the temporary loader is deleted. Moving a source file breaks no reference. A texture and a shader hot-reload at runtime.

### Stage 5 — Reflection, serialization, world → 0.3

**Goal.** The engine understands its own types and has a world model. Reflection is a backbone: serialization, inspector, scripting, replication, undo/redo and visual scripting all depend on it.

- Type registry and reflection (derive macro)
- Serialization with format versioning
- Entity model (decided here), components, transform hierarchy
- Worlds, scenes, prefabs
- Entity lifetime and stable references

**Exit.** A scene round-trips through save and load. Prefabs instantiate with overrides. Any component's properties can be listed and edited generically.

### Stage 6 — Editor foundation → 0.4

**Goal.** A real editor, built as a client of the engine.

- UI framework: text, layout, docking, widgets, input routing, focus, drag and drop, theming
- Viewport, hierarchy, inspector (driven by reflection), asset browser
- Command and transaction system: every edit is undoable
- Play in editor

**Exit.** The sample scene is built, saved and reloaded entirely in the editor, with undo/redo on every operation. The editor uses no private engine API.

### Stage 7 — Runtime systems → 0.5

**Goal.** What a game needs beyond rendering, built on a proper concurrency model.

- Job system: work stealing, dependencies, priorities
- Memory: frame, arena and pool allocators where measurements justify them; allocation tracking
- Visual profiler: CPU, jobs, memory, GPU, frames, from a trace stream
- Physics behind an engine abstraction: bodies, colliders, constraints, queries, character controller
- Audio: sources, listener, spatialization, buses, mixing, streaming
- Input action mapping

**Exit.** A small playable sample (a character in a physical level, with sound), profiled end to end.

### Stage 8 — Extensibility → 0.6

**Goal.** Others can extend the engine without modifying it.

- Module activation: a game enables only what it needs
- Plugin architecture for runtime and editor
- C ABI: opaque handles, versioned structures, function tables, explicit ownership, explicit error codes
- C++ SDK as an ergonomic wrapper over the C ABI
- Scripting (language decided here), bound through reflection, with reload and debugging

**Exit.** The Stage 7 sample's gameplay lives in a plugin using only public API. A C++ sample builds against the SDK.

### Stage 9 — Content production → 0.7

**Goal.** A team can produce a narrative game.

- Animation: skeletons, skinning, clips, blending, state machines, blend trees, IK, retargeting, root motion
- Material graph → IR → shader generation; variants, permutation cache, async compilation
- Sequencer: camera, animation, audio, event and VFX tracks
- VFX: CPU and GPU particles
- Navigation: navmesh, pathfinding, spatial queries
- Build, cook and package: Development, Shipping, Server, Editor configurations

**Exit.** A short narrative vertical slice (animated character, cutscene, gameplay) ships as a packaged build.

### Stage 10 — Large worlds and networking → 0.8

**Goal.** Scale beyond a single level and a single machine.

- World streaming: cells, async I/O, priorities, memory budgets, HLOD
- Terrain, vegetation and their editing tools
- Large-world coordinates
- High-scale rendering: GPU-driven rendering, advanced culling, virtual texturing, virtual geometry, global illumination, ray tracing. Each is admitted against a measured problem and depends on what wgpu exposes at that time.
- Networking: transport, replication, RPC, prediction, reconciliation, interest management
- Dedicated server build

**Exit.** A streamed open-world sample with several clients connected to a dedicated server.

### Stage 11 — Production readiness → 0.9, then 1.0

**Goal.** External projects can rely on the engine.

- Complete editor: material, animation, VFX and audio editors, sequencer, profiler, build tools, plugin manager, saved layouts, multi-window, command palette
- Editor extensibility: custom panels, inspectors, importers, asset types, build steps
- Crash reporting and diagnostics
- Migration tooling for assets, projects and serialized formats
- Deprecation policy
- Windows and macOS as supported platforms
- User, SDK and engine documentation

**Exit for 0.9.** An external project is built without touching engine source.
**Exit for 1.0.** A compatibility promise can be made and kept. There is no deadline for this.

## 7. Continuous tracks

These run through every stage. They are never "a phase for later".

| Track | Content |
|---|---|
| Testing and CI | Unit, integration, image comparison, serialization round-trip, performance regression |
| Profiling | Instrumentation added with each system |
| Performance budgets | CPU frame, GPU frame, memory, VRAM, load times, tracked per sample |
| Iteration speed | Compile times, hot reload, startup time |
| Diagnostics | Logs, assertions, crash context |
| Documentation | ADRs, architecture notes, API docs, sample walkthroughs |
| Samples | One runnable example per stage, kept working |

## 8. Horizon

Direction only. None of this is planned work until Stage 11 is reached.

- **Online platform for MMO-scale games.** Distinct from the engine: gateway, authentication, persistence, world and instance services, chat, economy, observability, deployment. The engine supplies serialization, replication, headless servers and world partitioning.
- **Consoles.** Only with official SDK access. Requires a native GPU backend if wgpu offers none.
- **Large-team workflows.** Multi-user editing, asset locking, distributed builds, shared derived-data cache.
- **Ecosystem.** Plugin registry, templates, asset packages. After users exist, not before.
- **Advanced world simulation.** Crowds, traffic, weather, water, destruction, procedural generation.
- **Visual scripting.** Node graph → IR → VM or native code, once reflection, scripting and undo/redo are mature.
- **Research track.** Paper → prototype → benchmark → production evaluation → merge.

## 9. Dependencies between systems

| System | Requires |
|---|---|
| platform | core |
| gfx | core; window handles through the `raw-window-handle` traits, not `platform` |
| render | gfx |
| assets | core, basic task pool |
| serialization | reflect |
| world | reflect, serialization, assets |
| editor | world, render, assets, UI framework |
| undo/redo | reflect, editor commands |
| physics, audio, animation, navigation | world |
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
| 0.2 | 4 | Assets |
| 0.3 | 5 | Reflection, serialization, world |
| 0.4 | 6 | Editor |
| 0.5 | 7 | Runtime systems |
| 0.6 | 8 | Plugins, SDK, scripting |
| 0.7 | 9 | Content production |
| 0.8 | 10 | Large worlds, networking |
| 0.9 | 11 | Usable by external projects |
| 1.0 | 11 | Stable API and SDK |
