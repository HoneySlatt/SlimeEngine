# Engine Roadmap

> Living document. It defines direction, ordering and exit criteria. It is not a task list:
> the current plan lives in `docs/PLAN.md`, and architecture decisions live in `docs/adr/`.
>
> This is an overview, not a commitment. Not everything here will be built: stages will be
> merged, reduced, reordered or dropped as the project evolves, and each such change is
> recorded in an ADR.

## 1. Vision

A general-purpose, modular, open-source (MIT OR Apache-2.0) creation platform whose runtime is written in Rust, for 3D games first.

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

Each style aims at the best rendering possible: realistic, anime, painted or hand-drawn. Each game sets *visual targets*, reference images on its target hardware at its frame rate, which principle 9 measures against. The engine runs on *hardware tiers*, from handheld PCs and modest computers at high frame rates to high-end ones: handheld, modest and high, each with a reference machine and budgets, and an enthusiast tier with ray tracing at Stage 9c (ADR-0114); a game picks its minimum tier among the first three. No mobile platform is aimed at.

## 2. Architecture

| Layer | Contents | Boundary below |
|---|---|---|
| Games | Rust, C++, scripting, visual scripting, data | Public engine API |
| SDK | C++ wrapper, plugin API, script bindings | Stable C ABI (promised from 1.0) |
| Engine (Rust) | core, platform, gfx, render, ui, assets, reflect, world, jobs, physics, audio, animation, net | Internal Rust API (unstable) |
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

## 4. Quality bar

What the engine, its editor and its games are weighed against, and the budgets that follow (ADR-0175). This section holds the budgets' figures: ADRs say why, `docs/measurements.md` holds what was measured, and `docs/debt.md` what is missed.

### References

Described by their qualities; the repository names no game.

- Games: the best open worlds of large studios of the time, for their image, their streaming without a hitch, their animation and how fast they answer.
- The editor: the best editors of games and of 3D content, for how fast they answer, their docking, and frames drawn only when something changes.
- The editor's interface: the most refined interfaces of the time, in motion, materials such as glass, type and responsiveness (ADR-0170, ADR-0171). Applications' references are in [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#2-quality-bar).

### Hardware

Three tiers, each with a reference machine (ADR-0114, ADR-0138); a game picks the lowest it supports.

| Tier | Reference machine | GPU, against the RX 6900 XT | A realistic game | A competitive shooter | Video memory for assets | Memory of the game |
|---|---|---|---|---|---|---|
| Handheld | Steam Deck: an RDNA 2 GPU of about 1.6 TFLOPS, 16 GB shared, 4 cores, 1280 × 800 | a tenth | 1280 × 800 at 30 frames a second, 33.3 ms | 1280 × 800 at 60, 16.7 ms | 4 GB, of 16 shared | 8 GB |
| Modest | the class of an RTX 3060 or an RX 6600: 8 GB of video memory, 6 cores, 16 GB | a half | 1920 × 1080 at 60, 16.7 ms | 1920 × 1080 at 144, 6.9 ms | 6 GB, of 8 | 12 GB, of 16 |
| High | the class of an RX 6900 XT, an RTX 4070 Ti or an RX 7800 XT: 16 GB of video memory, 8 cores, 32 GB | one | 3840 × 2160 at 60, 16.7 ms | 3840 × 2160 at 144, 6.9 ms | 12 GB, of 16 | 24 GB, of 32 |

- An enthusiast tier, the class of an RTX 4090, at 3840 × 2160 with ray tracing, comes with Stage 9c.
- The editor aims at the high tier at 240 frames a second: 4.17 ms a frame.
- Measured: the owner's machine alone, an RX 6900 XT and a Ryzen 7 5800X, and CI's lavapipe and WARP, drivers that render on the CPU. No tier's reference machine is measured; the GPUs' ratios come from public benchmarks.
- A budget of the GPU is checked on the RX 6900 XT, times its tier's ratio.
- The handheld tier's proxy: CPU benchmarks also run pinned to 4 cores (`taskset`); its GPU stays unmeasured. The owner has a Steam Deck, which measures the handheld tier once its measures are taken (ADR-0190).
- Applications on uwui have machines and budgets of their own, in [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#2-quality-bar) (ADR-0197).

### Budgets

CI checks budgets through exact counters, draws, elements, bytes, allocations, and ratios of times, how a cost grows with its size. Absolute times and the GPU's are measured by hand on the owner's machine, at each milestone that touches them. A budget missed is debt until it is met. Applications' budgets are in [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#budgets). *At rest* and *own wake-up* are defined in the [glossary](glossary.md).

| Budget | Figure | Measured by | Checked | State |
|---|---|---|---|---|
| A game's frame | its tier's frame time; the GPU at most about 85 % of it, the main thread about 70 % | `render.instances.10k`, `render.instances.100k`, `render.unseen.10k` | by hand | 10 000 instances within; 100 000 over the main thread's budget at 60 frames a second |
| A game's frames, kept even | 99 % within the frame time, none over twice it; a load in the background makes none miss it | `loading.zone`, `loading.textures`, which report the longest frame | by hand | not judged against the figure |
| The interface's GPU time | 0.5 ms on each tier's machine: on the RX 6900 XT, 50 µs at 1280 × 800, 250 µs at 1920 × 1080, 500 µs at 3840 × 2160 | `render.canvas`, `ui.draw.menu` | by hand | met |
| The editor's interface | 1.5 ms of CPU a frame, on the high tier; from a level of 10 000 entities to one of 100 000, its interface's CPU at most 1.5 times as much (ADR-0186) | `editor.window.zone`, `editor.window.entities` | by hand; the ratio and its counters, elements, elements visited, items and allocations, in CI | met, the ratio too; over in ADR-0041's stall, which the canvas's draw meets on the owner's machine once other programs fill the memory that the CPU maps; the hierarchy's search, 5.43 ms at 100 000 entities, over it |
| A list's frame | 0.25 ms of CPU at the 90th percentile, 100 000 rows, scrolled or still, on a Ryzen 7 5800X | `ui.tree.list.100k`, `devui.world_panel.100k`, `editor.hierarchy.100k` | by hand | the hierarchy within, still, scrolled and after a rename; over, a frame that makes the rows again after a change of the world's structure, about 1 ms, and the world panel's after a rename, 1.09 ms |
| At rest | the editor and its floating panels: 0 frames and 0 own wake-ups a second; a caret that blinks stops after 10 s without input; a game paused or hidden draws nothing | every thread's context switches over 10 s, in a headless Weston (ADR-0185, ADR-0187) | CI; a game hidden on the owner's compositor and minimized on Windows, by hand | met by the editor, a panel floating, and by a game paused, and hidden in a headless Weston and on the owner's compositor; minimized on Windows, not checked yet |
| Input to present | at the 99th percentile, 1 frame of 4.17 ms for the editor, 2 of their tier's frame for games, 33.3 ms at 60 frames a second; measured inside the engine (ADR-0189) | `scripts/input-to-present.sh`: a person's input in a headless sway, `input to present` in a trace | by hand | on the owner's machine, games within, but at the bound in motion in one run of two; the editor over in motion, its frames capped at 60 a second (ADR-0191, M6c.Q2f); the compositor, the display and a tier's machine are not measured |
| Start | warm, from the launch to the first useful frame, the first image presented: the editor 150 ms (ADR-0189); a level opened, the sample's zone drawn whole in the editor, 300 ms; a game to control, 1 s on its tier's machine (ADR-0191); the editor cold, 500 ms (ADR-0192) | `scripts/start-times.sh`: the log's times, plus the launch that `hyperfine` times | by hand | on the owner's machine, a game to control and the editor's cold start within; over, the editor's start and the zone opened; a tier's machine not measured |
| Play's time to control | 3 s, the game compiled, on its largest level | `play.zone`, `play.level.100k` | by hand | met in a release build, 2.22 s at the 90th percentile on 100 000 entities; a debug build over |
| Memory | a game's, per tier, above; the editor on the sample's zone, on the high tier, 256 MiB resident and 1.5 GiB of video memory asked of the driver (ADR-0193); none grows over 100 undos and redos and 10 runs of Play, its heap read by `heaptrack` | `scripts/resources.sh`: the resident size and the video memory from `/proc`, the heap by `heaptrack` | by hand | met on the owner's machine, the editor at most 173 MiB and 986 MiB, the game far within its tier's; a tier's own machine not measured |
| Power | the editor at rest adds no power to the GPU's beyond the desktop's; a game capped at 60 frames a second, the sample's zone at 3840 × 2160, adds at most 50 W to the desktop's on the RX 6900 XT (ADR-0193) | `scripts/resources.sh --gpu`: the GPU's driver's sensor, over 30 s | by hand | met on the owner's machine, the editor within the sensor's watt, the game 37.6 W; the other tiers not measured |
| An answer of the development channel | a median under 20 ms | its test in `slime-channel` | CI, a bound far above the time measured, which shared runners keep | met, 0.14 ms |
| The sample's zone | at most 1 000 000 triangles, 800 instances, 6 spot lights at night, 384 MiB of textures, 320 MiB cooked | `the_cooked_zone_keeps_within_its_budgets` | CI | met |
| CI's jobs | 20 minutes each | GitHub's times, in `docs/measurements.md` at each milestone | by hand | `windows` close to it |

### Without a figure

- Games' accessibility: controls that can be remapped, subtitles, modes for colour-blind players, camera shake and motion blur that can be turned off, at Stage 7.
- Interfaces' accessibility: screen readers, full use by the keyboard, reduced motion and reduced transparency (ADR-0146, ADR-0171).
- Text ready for translation, and written from right to left (ADR-0132).
- Images compared with reference images in CI (ADR-0030, ADR-0054).
- HDR output and a wide colour gamut, at Stage 9a.
- Variable refresh rates: ADR-0021's pacing unchanged with adaptive sync on the owner's display, at 60 frames a second, which divides its refresh; stated supported or not at a rate that does not, with the editor's rate of M6c.Q2f (ADR-0191).
- Bad input never crashes: readers tested with damaged files at each push (ADR-0102), fuzzed from M6c.Q4 (ADR-0176).
- A crash leaves a report on the user's machine, and nothing leaves it, from M6c.Q4 (ADR-0176).
- Each stage ends with an audit, each release with its checklist (ADR-0177).

## 5. Decisions

### Fixed

| Topic | Decision |
|---|---|
| Engine language | Rust |
| License | MIT OR Apache-2.0, as uwui (ADR-0182); releases before 0.7 under MPL-2.0 |
| GPU abstraction | wgpu, then native backends, Vulkan and Direct3D 12, at Stage 9c (ADR-0098). Their types stay inside the `gfx` and `render` crates, and the engine's own tests, through `slime-gfx`'s `wgpu` feature (ADR-0031); the rest of the engine uses engine-owned handles. |
| Shaders | WGSL to start |
| First platforms | Linux (Wayland and X11) and Windows: games run on both from Stage 2 on. macOS is built and tested in CI from Stage 6d, and supported once a Mac can check it; wgpu stays for it after Stage 9c. Games aim at no mobile platform and no web. Applications on uwui aim at Android phones and the web in a canvas too, macOS and iOS within reach (ADR-0098, ADR-0173, ADR-0181). |
| UI | uwui, the owner's framework, in a repository of its own, which receives its code at Stage 6d (ADR-0194, ADR-0197), under MIT OR Apache-2.0, which the engine depends on, never the reverse; GPU-rendered, for the editor, the UI of games and applications (ADR-0170, ADR-0179, ADR-0181): a retained tree of its own, on parley, taffy and AccessKit, embedded in a host that calls it, drawn on wgpu by its own canvas (ADR-0098, ADR-0132). The dev overlay, tooling, runs on it since Stage 6b, which removed egui (ADR-0152). No Qt or GTK. A light renderer on the CPU where there is no GPU; agents reach an application through a channel and an MCP server of uwui's own (ADR-0190). |
| Game-facing API | Rust first. The C ABI and C++ SDK are derived later from a proven API. |
| Windowing and input | winit, used only inside `platform`, and in uwui's desktop host; its types never leave them (ADR-0011, ADR-0181) |
| Entity model | The engine's own: entities, components stored by type, and a transform hierarchy, with `f64` translations (ADR-0079) |
| Serialization format | Driven by the engine's own reflection, without serde, but for uwui's documents, which uwui reads with serde (ADR-0181): TOML for text, sources and saves; binary formats of the engine's own for cooked data (ADR-0069, ADR-0080) |

### Open

Each is decided by ADR at the stage where it becomes necessary, not earlier. uwui's own, the notation of its UI documents among them, are in [uwui's roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#3-open-decisions).

| Topic | Candidates | Decide at |
|---|---|---|
| Physics backend | rapier3d, Jolt or other, behind an abstraction, against the needs of ADR-0098 | Stage 7 |
| Audio backend | To evaluate, against the needs of ADR-0098 | Stage 7 |
| Scripting language | Lua, C#, dedicated VM, other | Stage 8 |
| Shader language beyond WGSL | Slang, generated WGSL, with the native backends of Stage 9c in view | Stage 9b |
| Network transport | To evaluate | Stage 10b |

## 6. Repository layout

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

## 7. Stages

Stages are ordered by dependency. They overlap in practice, and carry no dates on purpose.

### Stage 0 — Foundations

**Goal.** Fix the rules that must survive for the life of the project.

- Cargo workspace, crate naming, edition, MSRV, lints
- Nix dev shell
- CI: build, test, clippy, fmt, documentation (rustdoc warnings, doc tests, Markdown links), dependency licenses and security advisories
- ADR process and first ADRs
- Dependency policy (admission criteria, license compatibility with the engine's license)
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

`egui-wgpu` is built against a specific wgpu version: while the engine's tooling drew with egui, until Stage 6b removed it (ADR-0152), upgrading wgpu waited for egui.

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

Between Stage 5b and Stage 6 come two passes, without a release of their own (ADR-0097): an audit pass, which fixes what an audit of 0.4 found in saves, identities, the readers of files (ADR-0096), the recovery from a lost GPU device, and measured quick wins, and puts the engine's measurements in CI; then a foundations pass, which decides as ADRs what the editor will freeze: the execution model, change detection, stable references and reversible operations, how the editor runs a game's code, the frame contract, a study of open worlds, an asset request API, hardware tiers, and what the GPU layer gives the editor, each weighed against the games of section 1 (ADR-0098) and against AI agents driving the engine (ADR-0099). `docs/PLAN.md` details both. What they deliver ships with 0.5, Stage 6a's release (ADR-0116).

Stage 6 is split in three parts (ADR-0116, ADR-0131): what the editor stands on, built and checked without the editor's interface; the engine's interface, its UI framework, with the editor's core; then the editor. A reference to Stage 6 means all three; in an ADR older than ADR-0131, Stage 6b means the editor, now Stage 6b and Stage 6c.

### Stage 6a — What the editor stands on → 0.5

**Goal.** Everything that the editor stands on, built and checked without the editor's interface, and used by the sample game.

- Frustum culling and instancing, moved from Stage 9a (ADR-0097), first, on the decisions of the foundations pass, with the GPU layer's storage buffers and instanced draws (ADR-0115), and the benchmarks measured at the hardware tiers' resolutions (ADR-0114)
- The execution model (ADR-0107): one task pool of the engine's, with a priority for the frame and one for the background; the copies to the GPU submitted by one owner within a budget of bytes per frame; the phases of a frame, the world read only while it is drawn; game time apart from real time, with pause and one step; a game as a simulation and a presentation, run by a host; the keys and buttons held, kept by the engine and given to the simulation as input (ADR-0057)
- What the world knows of its changes (ADR-0108): a tick on every value of a component, read since a reader's last tick, with summaries, a log of removals and a structure counter; components that clone
- The frame contract (ADR-0111): a frame that owns what it draws, an asset cache that loads without the renderer, views apart from the renderer with settings of their own, and materials of a kind registered with the renderer, the physically based one first; the adapter's limits asked for above a floor checked at startup (ADR-0114)
- A game is a library and its program (ADR-0110)
- The development channel, the link between a running game and the tools that drive it, and the engine's MCP server, before play in the editor (ADR-0116): AI agents pause, step and read a running game, change its world, capture its image, read its measurements and logs, cook, run tests and play the game with scripted input; reachable from the user's machine only, absent from shipped builds (ADR-0099, ADR-0110)
- Identities and saves, before the editor writes levels: the mark of driven components (ADR-0108); a key for each entity that a game's code creates at the start of a run, and an entity that a run created, once written into its level, recognized there when hot reload carries the run, not created again (ADR-0109); saves made of differences per scene with what persists chosen by the game, and entities adopted out of their scene (ADR-0112)
- Assets asked for by requests that carry a priority and return a handle, their loads cancelled when no handle holds them, and dropped least recently used first under a budget of memory (ADR-0113)
- A milestone of rendering for the editor: blending and lines, an unlit material, picking by the GPU and by the CPU, two points of each view's graph where a client inserts its draws, such as the editor's grid, icons, gizmos and outline (ADR-0115)

**Exit.** *Hunna Wood* runs on what the editor stands on, on Linux and on Windows: what the camera does not see is not drawn and the rest is drawn by instances, measured at the hardware tiers' resolutions against their budgets; its zone loads without a frame missing its time; its game time pauses and steps; an agent drives it through the engine's MCP server, pausing and stepping it, reading and changing its world, capturing its image and playing it with scripted input; its saves are differences per scene, which cost what changed. The `world` example draws a frame in several views and picks the object under the cursor.

### Stage 6b — The engine's interface → 0.6

**Goal.** The engine's UI framework, built and proven by the dev overlay that games ship, and the editor's core without its interface.

- The editor's core first, without its interface: a command and transaction system, every edit undoable, its entities named by their persistent identities, a deletion undone by creating its entities again under the same identities, the commands kept in a journal; what the editor creates named at once as its level's file names it (ADR-0109); the MCP server acts through the same commands (ADR-0099); the editor's own program, without a window yet, which an agent drives through the MCP server, and a game's editor package beside it, *Hunna Wood*'s (ADR-0110, ADR-0134, ADR-0135)
- The engine's UI framework (ADR-0098, ADR-0132): a retained tree of its own, built by code again when what it reads changes, its actions turned into commands; text in several writing systems; styles, and themes read from files; layout, and lists that lay out only what shows; input, focus and layers; text edited, with input methods and the clipboard; widgets; themes that compute their colors, and transitions that style sheets declare (ADR-0149); what it means, read by agents through the MCP server and by screen readers; drawn by the renderer's canvas
- Shaped for the games of Stage 7 from its first line: transforms, clips and layers in what it draws, roots that are not a window, text that can be translated and written from right to left, actions to which gamepads will be mapped (ADR-0132)
- The dev overlay rewritten on the framework, and egui removed (ADR-0132)
- Font files under the SIL Open Font License (ADR-0133)

**Exit.** The dev overlay of the game and of the `world` example runs on the engine's UI framework, on Linux and on Windows, its theme read from a file, and the framework draws text in several writing systems; an agent reads and drives the overlay's panels through the engine's MCP server, and a screen reader reads those of the `world` example; the list of a world of 100 000 entities stays within the budget that Stage 6b sets for it; egui is gone. The editor's core changes a level through commands, undone and redone, written back and opened again, driven by an agent.

### Stage 6c — The editor → 0.7

**Goal.** A real editor, built as a client of the engine, on its UI framework.

- The editor's window, one window of the system per process, which reads the editor's core and asks it through typed requests; every action a named operation of one registry, which menus, shortcuts, the palette and agents run alike (ADR-0155)
- Frames on demand: an idle editor draws nothing, its viewports and its interface drawn again only when what they show changed (ADR-0155)
- Viewport, hierarchy, inspector (driven by reflection), gizmos, a drag made one undo step, asset browser with thumbnails, cooking from the editor (ADR-0155)
- Docking, in steps: regions with tabs, splits that the user resizes and floating panels first, panels dragged to dock last, which may move to Stage 11; layouts saved per user and per project; dialogs to open and save files, and drag and drop (ADR-0131, ADR-0155); menu bars came with Stage 6b (ADR-0150)
- Levels made of several scene files, which name their parts; former identities written when the editor moves an entity out of its file, and references to entities of other scenes, `EntityRef` (ADR-0112, ADR-0155); a level that holds what the program does not know opened to be repaired, that data kept (ADR-0155)
- Play in the editor, the game running in a child process first (ADR-0097), in its own window, linked to the editor and to tools by the development channel, given private copies of the scenes not written, the editor's edits reaching it as commands while it runs; a change of the game's code starts it again where it was (ADR-0110, ADR-0155)
- The sample game's zone moved onto a kit of prefabs, which the editor places (ADR-0155)
- The quality bar written and checked: rest and the counters of budgets in CI, the hierarchy's search spread over frames, readers fuzzed and crash reports, then an audit of the stage before its release (ADR-0175, ADR-0176, ADR-0177)

The stage's milestones, their order and the owner's two checkpoints are decided in ADR-0155 and ADR-0175, and detailed in `docs/stage-6c.md` while the stage is paused; one release, 0.7 (ADR-0131). The stage pauses once M6c.Q2e is done, for Stage 6d and uwui's stages to its 1.0, then resumes (ADR-0190).

**Exit.** A level of the game is built, saved and reloaded entirely in the editor, with undo/redo on every operation. The editor uses no private engine API.

<!-- The anchor of this stage's former title, which ADR-0190 links to. -->
<a id="stage-6d--the-interface-for-applications--08"></a>

### Stage 6d — uwui leaves the engine

**Goal.** uwui, the owner's UI framework, leaves this repository for its own, decoupled from the engine and designed as a project of its own, the editor and the game working as before (ADR-0194). How an application is written is decided first, so that the host's API is written once.

- The figures before: the screens of reference chosen and frozen; the programs of uwui's pilots measured with their present frameworks, and their rivals of the market (ADR-0198); the owner's very modest laptop named and measured; what the renderer costs an application against the canvas alone (ADR-0180)
- How an application is written, decided with the owner and thrown away as code: how its state changes its interface, the format of layouts, Rust's code reloaded with its state kept, the interface's executor, the time of each frame, an application started in one call; GPUI with gpui-kit, Iced, Slint, Dioxus, Flutter, Compose, SwiftUI, Floem, Rune and Makepad read, four models prototyped and measured on uwui's tree (ADR-0190, ADR-0199, ADR-0202)
- uwui decoupled: typed styles; its documents read with `serde`; its canvas on wgpu, its layout chosen against WebGL2's limits; the host's API, `slime-app` its first host (ADR-0181)
- A series of headless compositors, Weston, sway, KWin, Mutter and X11 through Xvfb, in a job of CI of its own (ADR-0195)
- uwui's desktop host on winit, frames at the display's rate and none at rest, on every compositor of the series, its clipboard, input methods and accessibility; built and tested on macOS in CI, checked by hand on macOS (ADR-0173, ADR-0191, ADR-0196); a check that a program on uwui compiles neither the renderer, the world nor the editor
- uwui's channel and MCP server, through which agents reach an application (ADR-0190)
- uwui designed as a project, then its code moved, with its history, to [its repository](https://github.com/HoneySlatt/uwui), which the engine then depends on (ADR-0197)

The stage's milestones are in `docs/PLAN.md`. It has no release of its own: it is released with 0.7 (ADR-0194).

**Exit.** uwui's code is in its repository, its CI green on Linux, Windows and macOS and its builds for the web, Android and iOS; the engine depends on it, the editor's and the game's checks and budgets unchanged; a program on uwui compiles neither the renderer, the world nor the editor; how an application is written is decided in ADRs; uwui's first stage is planned.

### uwui's stages

In uwui's repository, in [its roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#4-stages): its first stage, the foundations, three programs ported and a media application written anew (ADR-0198), which holds what this stage held after the move, then its stages to its 1.0. The end of Stage 6c, then Stage 7, wait for its 1.0 (ADR-0190, ADR-0194).

### Stage 7 — Runtime systems → 0.8

**Goal.** What a game needs beyond rendering, built on a proper concurrency model.

- Job system, grown from Stage 6a's task pool (ADR-0107): dependencies, loops split across threads, work stealing if a measurement asks for it
- A render thread, decided by measuring the CPU that rendering still costs against the frame of delay it adds (ADR-0111)
- Hooks per component type, messages read at fixed phases, and a queue of the changes that parallel systems defer, designed with physics, audio and the events of games (ADR-0108)
- Memory: frame, arena and pool allocators where measurements justify them; allocation tracking
- Visual profiler: CPU, jobs, memory, GPU, frames, from a trace stream
- Physics behind an engine abstraction: bodies, colliders, constraints, queries (a ray or a segment against the world, ADR-0057), character controller; the backend chosen against vehicles (wheels and suspension, motorbikes, free flight), walking inside a moving vehicle, 2D, characters driven by physics, worlds in double precision and a simulation that runs the same way twice (ADR-0098)
- Riding: a character attached to a mount or a seat, with its animations, input and camera (ADR-0098)
- Audio: sources, listener, spatialization, buses, mixing, streaming; the importer of audio files, moved from Stage 4 (ADR-0068); the backend chosen against sound placed precisely around the listener's head, reverberation per room and sounds muffled by walls (ADR-0098)
- The core of skeletal animation: skeletons, skinning, clips, blending, moved from Stage 9b (ADR-0097); poses that are not entities, and bones that things are attached to (ADR-0108)
- Input action mapping, on the keys and buttons held that Stage 6a keeps (ADR-0107)
- UI of games: text, menus, HUD, on uwui, whose first stage gives it motion, transforms, masks, text styled by ranges and focus driven by gamepads (ADR-0170, ADR-0194); UI on surfaces in the world, screen readers, and UI documents, the games' screens as data (ADR-0098, ADR-0132)

**Exit.** The game has an animated character in a physical level, with sound and a UI, and is profiled end to end.

### Stage 8 — Extensibility → 0.9

**Goal.** Others can extend the engine without modifying it.

- Module activation: a game enables only what it needs
- Plugin architecture for runtime and editor
- Scripting (language decided here), bound through reflection, with reload and debugging
- The world updated in place when its scenes change while a game runs, keeping the handles that the game's code holds, moved from Stage 6 (ADR-0089, ADR-0155)

The C ABI and the C++ SDK moved to Stage 11, derived from the API that the editor and the game will have proven (ADR-0097).

**Exit.** The game's gameplay lives in a plugin using only public API.

Stage 9 is split in two (ADR-0052): the narrative vertical slice of Stage 9b needs a production renderer first.

### Stage 9a — Production renderer → 0.10

**Goal.** Render what a shipped game shows, beyond a simple scene.

- Shadows: shadows of point lights, the tiles of spot lights' shadows sized by how large each light shows on screen (ADR-0058)
- Many lights, by a method that keeps several ways of lighting possible in one frame: clustered forward is expected, chosen here by measurement (ADR-0098); compute shaders and indirect draws in the GPU layer (ADR-0115)
- Transparency
- Temporal anti-aliasing
- Fog, ambient occlusion, decals
- A day-night cycle
- The pieces for 2D and stylized games that no earlier stage needed (section 1)
- The look of a game: how each material is lit, passes of the game's own, its tone-mapping curve and its color grading (ADR-0098); the kinds of materials that a game registers (ADR-0111); more points of a view's graph where a game inserts its own passes (ADR-0115)
- The stability of the image as a measured goal: no shimmering, no trails, no detail that pops in (ADR-0098)
- Skin lit through subsurface scattering (ADR-0098)
- Quality set per hardware tier (section 1, ADR-0098), each technique stating its tier and its fallback, measured against the tiers' budgets (ADR-0114)

**Exit.** A level of the game, outdoors and indoors, with a day-night cycle, many lights, transparency and anti-aliasing, within the frame budgets tracked for the sample, covered by image tests.

### Stage 9b — Content production → 0.11

**Goal.** A team can produce a narrative game.

- Animation beyond the core that Stage 7 builds (ADR-0097): state machines, blend trees, IK, retargeting, root motion; motion matching and quadrupeds; 2D animation (ADR-0098)
- Faces through blend shapes, and lip sync; simple secondary motion of hair, cloth and flesh (ADR-0098)
- Material graph → IR → shader generation; values set per instance (ADR-0057); variants, permutation cache, async compilation
- Sequencer: camera, animation, audio, event and VFX tracks
- VFX: CPU and GPU particles
- Navigation: navmesh, pathfinding, spatial queries; the AI of characters beside it (ADR-0098)
- Dialogue and quests, with the state of a story that they read and that scenes depend on; the rules of a particular game stay in the game (ADR-0098); layers of a level that exist while a condition on the story holds (ADR-0112)
- Translation tables and voice (ADR-0098)
- Build, cook and package: Development, Shipping, Server, Editor configurations, for Linux and Windows; packages for applications too (ADR-0173). Builds without a console (Windows games) write their log to a file.
- Updates of shipped games: archives, patches that download only what changed, downloadable content and expansions as packages added to an installed game; from its first release, a save written by a shipped version of a game loads in all of its later versions, checked in CI with one save fixture per shipped version (ADR-0100); the same updates serve applications (ADR-0173)

**Exit.** The game grows into a short narrative vertical slice (animated character, cutscene, gameplay) that ships as a packaged build.

The stage is split in two halves when it is planned, if it needs to be (ADR-0098).

Stage 9c is added by ADR-0098: the rendering of the games of section 1, and consoles one day, need native graphics APIs.

### Stage 9c — Native GPU backends

**Goal.** Give the renderer the features of the GPU that wgpu lacks or holds as experimental.

- Vulkan and Direct3D 12 backends behind the engine-owned API of `slime-gfx`
- Work on several queues of the GPU in parallel, ray tracing, mesh shaders, resources without a declaration per binding, explicit memory, upscalers and frame generation, the vendors' tools for latency and GPU crashes
- wgpu stays for macOS, through Metal (ADR-0173); whether it also stays as a fallback or for debugging
- The enthusiast hardware tier, at 4K with ray tracing (ADR-0114)

Before this stage, a feature that wgpu blocks goes through wgpu's access to the API underneath it. The stage comes right after Stage 9b and has no release of its own; what it delivers is released with 0.12 (ADR-0131, ADR-0170, ADR-0194).

**Exit.** The game runs on the native backends on Linux and on Windows, and the image tests pass on them as on wgpu.

Stage 10 is split in two (ADR-0097): streaming, which an open world needs, does not wait for networking, which the first game does not need.

### Stage 10a — Large worlds → 0.12

**Goal.** Scale beyond a single level.

- World streaming: cells, async I/O, priorities, memory budgets, HLOD; cells computed by cooking and kept out of identities, what refers to each other loaded together, a region prepared in a second world then switched to with what follows the player (ADR-0112); textures streamed by their levels of detail, and a priority computed at each frame within the classes of requests (ADR-0113)
- Terrain, vegetation and their editing tools
- Large-world coordinates
- High-scale rendering: GPU-driven rendering, advanced culling, virtual texturing, virtual geometry, global illumination, ray tracing. Each is admitted against a measured problem, on the native backends of Stage 9c.
- Crowds and traffic, with hundreds of characters drawn (ADR-0098)
- Terrain that deforms, hair drawn strand by strand, simulated cloth (ADR-0098)
- Water and weather (ADR-0098)

The stage is split in two halves when it is planned (ADR-0098).

**Exit.** The game streams an open world.

### Stage 10b — Networking → 0.13

**Goal.** Scale beyond a single machine.

- Networking: transport, replication, RPC, prediction, reconciliation, lag compensation, interest management for hundreds of players in one place (ADR-0098); what a run creates numbered by the server, what scenes place named alike on every machine (ADR-0109)
- Synchronized physics, proximity voice chat, games hosted by a player with relays, a world split between servers (ADR-0098)
- The threat model of ADR-0096 extended to the network (ADR-0098)
- The data that servers keep for players converted from one version of a game to the next (ADR-0100)
- Dedicated server build

The stage is split in two halves when it is planned (ADR-0098).

**Exit.** Several clients play the game connected to a dedicated server.

### Stage 11 — Production readiness → 0.14, then 1.0

**Goal.** External projects can rely on the engine.

- Complete editor: material, animation, VFX and audio editors, sequencer, profiler, build tools, plugin manager, panels in windows of their own (ADR-0155); the palette of commands came with Stage 6b (ADR-0151), and saved layouts come with Stage 6c
- Editor extensibility: custom panels, inspectors, importers, asset types, build steps
- C ABI: opaque handles, versioned structures, function tables, explicit ownership, explicit error codes; moved from Stage 8 (ADR-0097)
- C++ SDK as an ergonomic wrapper over the C ABI, with a C++ sample that builds against it
- Crash reporting and diagnostics
- Migration tooling for assets, projects and serialized formats
- Deprecation policy
- macOS as a supported platform, unless a Mac has checked it sooner (ADR-0173)
- User, SDK and engine documentation

**Exit for 0.14.** An external project is built without touching engine source.
**Exit for 1.0.** A compatibility promise can be made and kept. There is no deadline for this.

## 8. Continuous tracks

These run through every stage. They are never "a phase for later".

| Track | Content |
|---|---|
| Testing and CI | Unit, integration, image comparison, serialization round-trip, budgets kept by counters and ratios of times (section 4) |
| Platforms | Every stage's sample builds and its tests pass in CI on Linux and Windows; windowed behaviour is checked by hand on both |
| Profiling | Instrumentation added with each system |
| Performance budgets | The quality bar (section 4), measured at each milestone that touches it |
| Iteration speed | Compile times, hot reload, startup time |
| Diagnostics | Logs, assertions, crash reports (ADR-0176) |
| Documentation | ADRs, architecture notes, API docs, sample walkthroughs |
| Samples | One runnable example per stage, kept working; from Stage 3c on, the game that each stage grows |
| Agents | From Stage 6 on, each system exposed to AI agents through the engine's MCP server as it is written (ADR-0099) |

## 9. Horizon

Direction only. None of this is planned work until Stage 11 is reached.

- **Online platform for MMO-scale games.** Distinct from the engine: gateway, authentication, persistence, world and instance services, chat, economy, group finding, anti-cheat, observability, deployment. The engine supplies serialization, replication, headless servers and world partitioning.
- **Consoles.** Only with official SDK access, and hardware to test on. Built on the native backends of Stage 9c (ADR-0098).
- **Large-team workflows.** Multi-user editing, asset locking, distributed builds, shared derived-data cache.
- **Ecosystem.** Plugin registry, templates, asset packages. After users exist, not before.
- **Advanced world simulation.** Destruction, procedural generation. Crowds, traffic, water and weather moved to Stage 10a (ADR-0098).
- **Visual scripting.** Node graph → IR → VM or native code, once reflection, scripting and undo/redo are mature.
- **Research track.** Paper → prototype → benchmark → production evaluation → merge.

## 10. Dependencies between systems

| System | Requires |
|---|---|
| platform | input only, the engine's vocabulary of input (ADR-0004, ADR-0142) |
| gfx | color only (ADR-0079); window handles through the `raw-window-handle` traits, not `platform` |
| render | gfx, assets, color, reflect, tasks: it loads its own kinds of asset (ADR-0069), describes its materials' parameters by reflection (ADR-0111), on a task pool of its own from Stage 4b (ADR-0072), the engine's, `slime-tasks`, from Stage 6a (ADR-0107). Not world: `slime-world-render`, a bridge between them, builds each frame from the world (ADR-0079) |
| assets | reflect only (ADR-0069, ADR-0080) |
| color | reflect only (ADR-0079) |
| reflect | its derive macro only (ADR-0080) |
| serialization | reflect |
| world | reflect, serialization, assets, color |
| UI framework | color, input; its documents read with `serde`; headless; drawn on wgpu by its own canvas, which render composes over its views (ADR-0132, ADR-0181) |
| hosts of applications | the UI framework, and a platform's own layer: winit on the desktop, a Kotlin program on Android, the page on the web; never world, a game's host, the editor or games (ADR-0170, ADR-0181) |
| editor | world, render, assets, UI framework; its core, without its interface, today: app, assets, channel, cook, reflect, serialization, world (ADR-0134) |
| UI of games | render, input action mapping, UI framework |
| undo/redo | the world's commands, serialization (ADR-0134) |
| physics, audio, animation, navigation | world |
| riding | physics, animation, input action mapping |
| AI of characters | world, navigation |
| dialogue and quests | world, serialization |
| plugins, C ABI, scripting | reflect, stable module boundaries |
| material graph | render, reflect, UI framework |
| streaming | world, assets, jobs, profiler |
| networking | reflect, serialization, world, headless mode |
| visual scripting | scripting, reflect, undo/redo |

## 11. Release map

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
| — | — | An audit pass and a foundations pass, released with 0.5 (ADR-0097, ADR-0116) |
| 0.5 | 6a | What the editor stands on |
| 0.6 | 6b | The engine's interface: its UI framework, and the editor's core |
| 0.7 | 6c | Editor, after uwui's 1.0: the stage paused once M6c.Q2e is done (ADR-0190) |
| — | 6d | uwui leaves the engine, released with 0.7 (ADR-0194) |
| — | — | uwui's stages, to its 1.0, in its repository, before the end of Stage 6c (ADR-0190) |
| 0.8 | 7 | Runtime systems |
| 0.9 | 8 | Plugins, scripting |
| 0.10 | 9a | Production renderer |
| 0.11 | 9b | Content production |
| — | 9c | Native GPU backends, released with 0.12 |
| 0.12 | 10a | Large worlds |
| 0.13 | 10b | Networking |
| 0.14 | 11 | Usable by external projects |
| 1.0 | 11 | Stable API and SDK |
