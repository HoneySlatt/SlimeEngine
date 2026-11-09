# Engine Roadmap

> Living document. It defines direction, ordering and exit criteria. It is not a task list:
> the current plan lives in `docs/PLAN.md`, and architecture decisions live in `docs/adr/`.
>
> This is an overview, not a commitment. Not everything here will be built: stages will be
> merged, reduced, reordered or dropped as the project evolves, and each such change is
> recorded in an ADR.

## 1. Vision

A general-purpose, modular creation platform whose runtime is written in Rust, for 3D games first; its source public, under a licence of its own with a royalty above a threshold (ADR-0214).

Long-term targets, in order of reach:

1. Linear narrative games
2. Open-world games
3. Large online worlds

The project is a complete platform (runtime, asset pipeline, editor, SDK), not a renderer with extras. A native, full-featured editor is a first-class component.

Whom it is for (ADR-0207): a public engine, adopted widely in the end, judged as a whole against Unreal, Unity and Godot, and better as a whole, never on one feature alone. It is made first for a small team helped by AI agents, making games of the class that large studios make; the tools of large teams come later. Until it opens, the owner's games direct it, and an outside need enters only if it is general. It is announced, and opens to contributions of code, at 0.14. It is free until a game has earned 1 M$, then takes 3 % of the revenue above, 2 % for a game on the engine's services (ADR-0214); money comes too from around it, the private layers for consoles, closed online services, a store of plugins and assets after 0.14, support and partners for studios, a hosted assistant, and the owner's games; the engine stays complete without them. Uses other than games are welcome, not aimed at. The engine and its editor send nothing over the network of their own accord.

3D comes first. 2D and stylized games are made with the same renderer, through pieces that the stages add where they are needed: an orthographic camera, sprites, transparency, unlit and custom materials, crisp sampling for pixel art. There is no separate 2D renderer (ADR-0052).

The games the engine is weighed against, as profiles (ADR-0098). The games played alone come first; the online games come after them.

- A realistic third-person role-playing game in one open world without loading screens, centred on a story whose choices have consequences, with demanding combat and a horse to ride. *Hunna Wood*, the owner's game of this profile, was the sample; it leaves this repository and resumes, as a client from outside, once the engine is ready (ADR-0219).
- A space opera across several hand-made planets, played on foot in first or third person and at the controls of cars, motorbikes and spaceships, with loadings the player does not notice, a ship to walk in while it flies, and choices that destroy places.
- A linear, turn-based narrative role-playing game in anime-style 3D, toon-shaded everywhere, with menus of a strong identity.
- A hand-drawn 2D metroidvania: a large map of connected rooms and precise controls.
- A painted action role-playing game with a party, whose combat slows time while orders are given.
- A first-person photorealistic horror game in one small place that changes at each pass.
- A massively multiplayer online role-playing game told in expansions, with about 500 players in one place and few instances.
- A competitive first-person shooter on dedicated servers, at high frame rates on modest computers.
- Online co-op games hosted by a player, with physics-driven characters, vehicles and proximity voice chat.
- A fighting game, its frames replayed when an opponent's input arrives late (ADR-0207).
- A strategy game of thousands of units, one simulation run the same on every machine (ADR-0207).
- A game of survival and building, its world changed in bulk by players together, and kept (ADR-0207).
- A racing game: vehicles at high speed and fast streaming (ADR-0207).

Three profiles lead the stages: the realistic open world, the competitive shooter and the 2D game. Each stage is planned and checked against them; for the others, each ADR checks that its design does not block them (ADR-0207). Each has a small *test game*, a game made only to drive and check the engine, grown only by what the stage in progress needs: `games/open-world`, a realistic slice; `games/arena`, a competitive shooter; `games/metroidvania`, a game in 2D (ADR-0219). The first game sold on the engine is the 2D test game, around Stage 9b, which proves the chain from the store to updates: its code public, its content private (ADR-0219).

Open worlds are aimed at by their density, not their area: no limit of area, each game choosing its size; the engine is measured on *test worlds*, a dense city's district, a dense forest, a large world mixing both and a sparse one at the scale of a planet (section 4, ADR-0217).

Each style aims at the best rendering possible: realistic, anime, painted or hand-drawn. Each game sets *visual targets*, reference images on its target hardware at its frame rate, which principle 9 measures against. The engine as a whole is measured against its *rivals*, the market's best games and engines, named in section 4 as benches, and leads each of them by a margin (ADR-0215). The engine runs on *hardware tiers*, from handheld PCs and modest computers at high frame rates to high-end ones: handheld, modest and high, each with a reference machine and budgets, and an enthusiast tier whose top rung is path tracing in real time (ADR-0114, ADR-0216); a game picks its minimum tier among them; a minimal tier below the handheld one holds 2D and light games (ADR-0208, ADR-0216). The owner's games aim at no mobile platform; mobile and web games for the engine's other users are in the horizon (ADR-0209).

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
4. **Every stage ends with its test games grown and run.** An engine that is never used to build something drifts. From Stage 3c on, the sample was one game, grown by each stage, *Hunna Wood* since Stage 4a (ADR-0067); Stage 6e puts in its place the test games of section 1, one per profile that leads the stages, each grown only by what its stage needs (ADR-0219).
5. **Headless is a normal configuration.** The engine must run without window, renderer or audio. This is what makes CI tests and dedicated servers possible.
6. **Instrumented from day one.** Profiling spans and GPU markers are added as systems are written, not retrofitted.
7. **Existing crates are used wherever they are not the point of the engine,** and wrapped at the boundary so they can be replaced.
8. **Stability is promised from 1.0:** the C ABI, and a public façade of the Rust API, under semantic versioning kept strictly, a 2.0 coming with a tool that migrates code, projects and assets; what lies behind the façade stays free to change. Before 1.0, any API may break; the user's data never does, migrated (ADR-0207).
9. **Techniques enter because they solve a measured problem,** not to match another engine's feature list. The measured problem is the gap to the rivals measured, on each line of the quality bar (section 4, ADR-0215); for rendering, also the gap from a game's image to its visual targets and to the reference path tracer's image (section 1, ADR-0098, ADR-0216).
10. **What a person does, an agent does.** Every operation of the editor and of a running game that a person performs is a command that an AI agent can call through the engine's MCP server, the same command, not a path of its own (ADR-0099).
11. **Finished or absent.** A feature is in a release only once it meets its bar, its budgets, tests and documentation, on the platforms it promises; before, it waits behind an option marked experimental, its state written, never presented as finished (ADR-0207).

## 4. Quality bar

What the engine, its editor and its games are weighed against, and the budgets that follow (ADR-0175, ADR-0215). This section holds the budgets' figures: ADRs say why, `docs/measurements.md` holds what was measured, and `docs/debt.md` what is missed. Each figure is measured as the stage that touches it is built, and the rivals' figures before it (ADR-0215).

### References

The *rivals*: games and engines named as benches of measure only, here and in `docs/measurements.md` with their version, settings and date, never as an inspiration (ADR-0215). On each criterion, the best rival measured is the bar, and the engine leads it by the margins of the budgets below. Each measure on the owner's games and account waits for the owner's consent.

- Games' image: Unreal 5, and the engines that studios keep for themselves, RAGE, Decima, BlackSpace and REDengine, seen through their games. Unreal 5 is measured on scenes rebuilt alike with the engine's own assets: the image against the reference path tracer, and every cost, at equal content.
- Games on PC, measured from outside on the owner's machine and the Steam Deck: frames and their pacing, hitches while crossing the world, memory and video memory, power, the processor's load, the time to control, loads, the size installed and the image's stability. Red Dead Redemption 2, Cyberpunk 2077, Horizon Forbidden West, Crimson Desert and Death Stranding 2 to begin; GTA 6 once it is on PC.
- Physics, sound, characters' AI, animation and the network: Unreal's Chaos, MetaSounds, AI, animation and replication with Iris, on the same scenes, and the games above; for the network, Valorant and Counter-Strike 2 too.
- Stylised games: captures of Genshin Impact, Wuthering Waves and Mouse: P.I. for Hire. Menus: captures of Persona 5 Royal and Metaphor: ReFantazio.
- The editor: Unreal, Unity, Godot and Blender, the same level opened in each; the best editors of games and of 3D content, for how fast they answer, their docking, and frames drawn only when something changes. Compiling: Unreal, Unity and Godot.
- Agents: the same agents given the same tasks in the engine, Unreal, Unity and Godot, each with its own tools for agents.
- The editor's interface: the most refined interfaces of the time, in motion, materials such as glass, type and responsiveness (ADR-0170, ADR-0171). Applications' references are in [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#2-quality-bar), which measures them as the engine is measured: the light sensor, blind tests by people, compiling's budgets and hours without a crash (ADR-0215).

### Hardware

Three tiers, each with a reference machine (ADR-0114, ADR-0138); a game picks the lowest it supports.

| Tier | Reference machine | GPU, against the RX 6900 XT | A realistic game | A competitive shooter | Video memory for assets | Memory of the game |
|---|---|---|---|---|---|---|
| Handheld | Steam Deck: an RDNA 2 GPU of about 1.6 TFLOPS, 16 GB shared, 4 cores, 1280 × 800 | a tenth | 1280 × 800 at 40 frames a second, 25 ms; 30, 33.3 ms, a fallback that a game may choose (ADR-0215) | 1280 × 800 at 60, 16.7 ms | 4 GB, of 16 shared | 6 GB (ADR-0215) |
| Modest | the class of an RTX 3060 or an RX 6600: 8 GB of video memory, 6 cores, 16 GB | a half | 1920 × 1080 at 60, 16.7 ms | 1920 × 1080 at 144, 6.9 ms | 6 GB, of 8 | 6 GB, of 16 |
| High | the class of an RX 6900 XT, an RTX 4070 Ti or an RX 7800 XT: 16 GB of video memory, 8 cores, 32 GB | one | 3840 × 2160 at 60, 16.7 ms | 3840 × 2160 at 144, 6.9 ms | 12 GB, of 16 | 12 GB, of 32 |

- The resolution shown counts; the resolution drawn is free, the image judged by measure, against the reference path tracer and for its stability (ADR-0215).
- An enthusiast tier, the class of an RTX 4090, at 3840 × 2160, its top rung path tracing in real time, which shares its code with the reference path tracer (ADR-0216); it comes with the stage that brings path tracing in real time, Stage 9a or Stage 9c.
- A minimal tier, below the handheld one, for 2D and light games: hardware of about 2015 that has Vulkan, of the class of an Intel HD 520, as the owner's very modest laptop; no renderer on OpenGL (ADR-0216). The handheld tier stays the floor of realistic 3D.
- Realistic games offer a mode of performance at 120 frames a second, 8.3 ms, beside the mode of quality above: on the high tier at 3840 × 2160, and on the modest tier at 1920 × 1080 (ADR-0208, ADR-0215).
- The editor aims at the high tier at 240 frames a second: 4.17 ms a frame. It stays comfortable on the modest tier, its 3D views lightened, its interface never (ADR-0209).
- Before Stage 7, runners of CI at the owner's, on the Steam Deck and on a desktop GPU, measure what is measured by hand today (ADR-0207, ADR-0220).
- Measured: the owner's machine alone, an RX 6900 XT and a Ryzen 7 5800X, and CI's lavapipe and WARP, drivers that render on the CPU. No tier's reference machine is measured; the GPUs' ratios come from public benchmarks.
- A budget of the GPU is checked on the RX 6900 XT, times its tier's ratio.
- The handheld tier's proxy: CPU benchmarks also run pinned to 4 cores (`taskset`); its GPU stays unmeasured. The owner has a Steam Deck, which measures the handheld tier once its measures are taken (ADR-0190).
- Applications on uwui have machines and budgets of their own, in [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#2-quality-bar) (ADR-0197).

### Budgets

CI checks budgets through exact counters, draws, elements, bytes, allocations, and ratios of times, how a cost grows with its size. Absolute times and the GPU's are measured by hand on the owner's machine, at each milestone that touches them. A budget missed is debt until it is met. Applications' budgets are in [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#budgets). *At rest* and *own wake-up* are defined in the [glossary](glossary.md).

| Budget | Figure | Measured by | Checked | State |
|---|---|---|---|---|
| A game's frame | its tier's frame time; the GPU at most about 85 % of it, the main thread about 70 % | `render.instances.10k`, `render.instances.100k`, `render.unseen.10k` | by hand | 10 000 instances within; 100 000 over the main thread's budget at 60 frames a second |
| A game's frames, kept even | 99.9 % within the frame time, none over 1.5 times it; none missed while crossing the world fast or while loading (ADR-0215) | `loading.zone`, `loading.textures`, which report the longest frame; the rivals' by the tool that measures a game from outside, from Stage 6e | by hand | not judged against the figure |
| The lead against the rivals | against games, on hitches' time, the time to control, loads and latency, 2.5 times better than the best; an image steadier than the best's. Against Unreal 5 on the same scenes: 4 times less processor, memory, start and build size; 1.75 times less GPU time and power at equal image; at equal GPU time, an image closer to the reference path tracer. Beating each line stays the least (ADR-0215) | games measured from outside, from Stage 6e; the scenes rebuilt in Unreal 5, from before Stage 7 | by hand, at each stage that touches a line | a target; no rival measured yet |
| The interface's GPU time | 0.5 ms on each tier's machine: on the RX 6900 XT, 50 µs at 1280 × 800, 250 µs at 1920 × 1080, 500 µs at 3840 × 2160 | `render.canvas`, `ui.draw.menu` | by hand | met |
| The editor's interface | 1.5 ms of CPU a frame, on the high tier; from a level of 10 000 entities to one of 100 000, its interface's CPU at most 1.5 times as much (ADR-0186) | `editor.window.zone`, `editor.window.entities` | by hand; the ratio and its counters, elements, elements visited, items and allocations, in CI | met, the ratio too; over in ADR-0041's stall, which the canvas's draw meets on the owner's machine once other programs fill the memory that the CPU maps; the hierarchy's search, 5.43 ms at 100 000 entities, over it |
| A list's frame | 0.25 ms of CPU at the 90th percentile, 100 000 rows, scrolled or still, on a Ryzen 7 5800X | `ui.tree.list.100k`, `devui.world_panel.100k`, `editor.hierarchy.100k` | by hand | the hierarchy within, still, scrolled and after a rename; over, a frame that makes the rows again after a change of the world's structure, about 1 ms, and the world panel's after a rename, 1.09 ms |
| At rest | the editor and its floating panels: 0 frames and 0 own wake-ups a second; a caret that blinks stops after 10 s without input; a game paused or hidden draws nothing | every thread's context switches over 10 s, in a headless Weston (ADR-0185, ADR-0187) | CI; a game hidden on the owner's compositor and minimized on Windows, by hand | met by the editor, a panel floating, and by a game paused, and hidden in a headless Weston and on the owner's compositor; minimized on Windows, not checked yet |
| Input to present | at the 99th percentile, 1 frame of 4.17 ms for the editor; for games, 1.5 of their tier's frame for a realistic game, 25 ms at 60 frames a second, and 1 for a competitive shooter (ADR-0215); measured inside the engine (ADR-0189), and timed to the photon by a light sensor on the screen, so that games of the market compare from outside (ADR-0215) | `scripts/input-to-present.sh`: a person's input in a headless sway, `input to present` in a trace; the light sensor's bench, from Stage 6e | by hand | on the owner's machine, inside the engine, the game within for inputs apart; over in motion in one run of two, 35.0 ms; the editor over in motion, its frames capped at 60 a second (ADR-0191, M6c.Q2f); the compositor, the display, the photon and a tier's machine are not measured |
| Start | warm, from the launch to the first useful frame, the first image presented: the editor 150 ms (ADR-0189); a level opened, the sample's zone drawn whole in the editor, 300 ms; a game to control, 1 s on its tier's machine (ADR-0191); the editor cold, 500 ms (ADR-0192) | `scripts/start-times.sh`: the log's times, plus the launch that `hyperfine` times | by hand | on the owner's machine, a game to control and the editor's cold start within; over, the editor's start and the zone opened; a tier's machine not measured |
| Play's time to control | 3 s, the game compiled, on its largest level | `play.zone`, `play.level.100k` | by hand | met in a release build, 2.22 s at the 90th percentile on 100 000 entities; a debug build over |
| Memory | a game's, per tier, above; the editor on the sample's zone, on the high tier, 256 MiB resident and 1.5 GiB of video memory asked of the driver (ADR-0193); none grows over 100 undos and redos and 10 runs of Play, its heap read by `heaptrack` | `scripts/resources.sh`: the resident size and the video memory from `/proc`, the heap by `heaptrack` | by hand | met on the owner's machine, the editor at most 173 MiB and 986 MiB, the game far within its tier's; a tier's own machine not measured |
| Power | the editor at rest adds no power to the GPU's beyond the desktop's; a game capped at 60 frames a second, the sample's zone at 3840 × 2160, adds at most 50 W to the desktop's on the RX 6900 XT (ADR-0193) | `scripts/resources.sh --gpu`: the GPU's driver's sensor, over 30 s | by hand | met on the owner's machine, the editor within the sensor's watt, the game 37.6 W; the other tiers not measured |
| The Deck's battery | the realistic test game, at 40 frames a second on the Steam Deck, lasts at least 1.5 times as long on its battery as the best game measured at its settings for the Deck (ADR-0215) | both run on a full battery until it is empty | by hand, from Stage 6e | a target |
| The processor's load | on average, at most half of the tier's cores at the game's target rate (ADR-0215) | the cores' load over a run, beside the rivals' | by hand, from Stage 6e | a target |
| Size installed | per km² of world made by hand, at most half the best game's (ADR-0215) | the size installed over the world's area | by hand, from Stage 6e for the rivals, Stage 10a for the engine | a target |
| An answer of the development channel | a median under 20 ms | its test in `slime-channel` | CI, a bound far above the time measured, which shared runners keep | met, 0.14 ms |
| The sample's zone, `games/open-world`'s from Stage 6e (ADR-0219) | at most 1 000 000 triangles, 800 instances, 6 spot lights at night, 384 MiB of textures, 320 MiB cooked | `the_cooked_zone_keeps_within_its_budgets` | CI | met |
| CI's jobs | 20 minutes each | GitHub's times, in `docs/measurements.md` at each milestone | by hand | `windows` close to it |
| Pipelines created during play | none after a level starts: every pipeline in the game's cooked list, created while the level loads; a budget of pipelines per game, set at Stage 9b (ADR-0208) | a replay that lists the pipelines it asks for | CI, from Stage 9b | a target |
| A change seen | an asset, a scene or a shader changed, seen in the running game and the editor within 1 s; a game's Rust code reloaded hot, its state kept, under 1 s, on games of 50,000 and 500,000 lines (ADR-0208, ADR-0215) | from the edit to the change on screen | by hand, from Stage 8 | a target |
| Compiling | on the Ryzen 7 5800X: a change of a game's code to the game running again, 2 s in a debug build; the editor built again after a change of an engine crate, 10 s; the whole engine from nothing, 3 min; a game's build for shipping, 10 min. Against Unreal, Unity and Godot, 4 times less waiting than the best (ADR-0215) | the times of compiling, in `docs/measurements.md`; the rivals', from Stage 6e | by hand, at each milestone, `scripts/compile-times.sh` (ADR-0220, ADR-0221) | measured 2026-11-08 at C1, in debug builds, with mold in the Nix shell: a game's change to it running again about 1.8 s, within, estimated as the build and Play's time to control added, about 2.2 s with rustc's own lld; the editor after an engine change 1.7 s, within; the whole engine from nothing 62 s, within; a game's build for shipping 87 s, within; reloaded hot, not yet (Stage 8); the rivals not measured |
| A replay | bit for bit the same on one build and one platform; across platforms for the games that ask (ADR-0208) | a hash of the world at each step of a replay | CI, from Stage 7 | a target |
| A level merged | two people's or agents' edits of one level merged; an open world's cell opened, and its whole status read, within budgets set at Stage 10a (ADR-0208) | the merge tool on two edits of one cell | CI, from the stage that builds the tool, chosen when Stage 6c resumes | a target |
| A networked game's feel | from an input to its effect seen, at the 99th percentile, set at Stage 10b, under a simulated network (ADR-0208); against the rivals, with the lead's margins: a shooter's felt delay, the peeker's advantage and the server's rate; a fighting game's frames replayed with no visible glitch at 100 ms of round trip; 500 players in one place; the time for another player to take over a game whose host left; the server's processor and bandwidth per player 4 times less than Iris's (ADR-0215) | a game played by agents on several clients; Unreal's replication on the same scenes | CI, from Stage 10b | a target |
| The open world | no limit of area; density, on test worlds: a city's district at the density of the best rival measured, GTA 6 once it is on PC and Cyberpunk 2077 until then, its characters, traffic and towers with their interiors; a forest at the density of foliage of Horizon Forbidden West and Crimson Desert; a test world of 1,000 km² mixing both, made with the tools of generation, crossed at 300 km/h without a hitch or a detail shown late on every tier, the handheld one with less detail and the same density; a world at the scale of a planet, sparse (ADR-0217); read at most 50 MB/s, resident within the tier's video memory for assets (ADR-0208) | the test worlds | by hand, from Stage 10a | a target |
| Characters, in three levels of life | visible: at least 2.5 times the best game's density in its busiest street, at the same frame budget; alive: each character near the player has a schedule, perception and a memory of the player's acts, no hollow extra within a distance set at Stage 9b; simulated out of sight, lighter; the Steam Deck keeps the density, with less detail (ADR-0215) | the test worlds' city, beside the rivals' | by hand, its figures from Stage 10a | a target |
| The distance seen | no visible limit: everything at its true distance to the horizon, from any height, mountains at 50 km and more, a planet from its sky, a city's lights at night, far traffic that still moves; shadows and light held far, simpler; no detail appearing in sight; the Steam Deck sees as far, with less detail (ADR-0215) | judged blind, and measured while crossing the test worlds | by hand, from Stage 10a | a target |
| The editor against the rivals | the same level opened in Unreal, Unity, Godot and Blender: start, level opened, memory, the 3D view's smoothness, input to present, rest; 4 times better than the best on each line; an open world's cell opened and crossed, against Unreal's World Partition, its figures at Stage 10a (ADR-0215) | the rivals' editors on the same level, from Stage 6e | by hand | a target; the rivals not measured |
| Hours without a crash | at least 99.9 % of the editor's hours of use without a crash; no work lost after a crash (ADR-0215) | the hours and crashes counted on the machine; a test that kills the editor at random and finds no work lost | CI, from M6c.Q4 (ADR-0220) | a target |
| The *agent bench* | the engine's tasks, a system of a game written, a level built from a brief, a bug fixed from a crash report, a scene brought within its budget, given to the same agents in the engine, Unreal, Unity and Godot; success, tries, tokens and time, with the models and their versions; the goal, provisional, at least the best's success on fewer tokens (ADR-0215) | its runs, in `docs/measurements.md` | by hand, at each release, once a new game can be made and cooked outside this repository (ADR-0220) | a target |
| The *creation bench* | fixed tasks, a km² of dense forest with a village, a character imported and animated, a lit interior, a short cutscene, a quest with its dialogue, done in the engine and in Unreal by the same person and agents: 4 times less waiting, imports, cooking, shaders and light; 2.5 times less human time, its bias written beside each figure; the quality judged blind (ADR-0215) | its runs, timed | by hand, each task from the stage that builds what it needs | a target |

### Without a figure

- Games' accessibility: controls that can be remapped, subtitles, modes for colour-blind players, camera shake and motion blur that can be turned off, at Stage 7; then all that an engine can give of the Xbox Accessibility Guidelines, native and checked, each game choosing what it turns on (ADR-0208).
- Games' languages: Fluent, voices per language, a fake language of test in CI (ADR-0208).
- Nothing sent: the engine and the editor send nothing over the network of their own accord (ADR-0207).
- Interfaces' accessibility: screen readers, full use by the keyboard, reduced motion and reduced transparency (ADR-0146, ADR-0171).
- Text ready for translation, and written from right to left (ADR-0132).
- Images compared with reference images in CI (ADR-0030, ADR-0054).
- HDR output and a wide colour gamut, at Stage 9a.
- Variable refresh rates: ADR-0021's pacing unchanged with adaptive sync on the owner's display, at 60 frames a second, which divides its refresh; stated supported or not at a rate that does not, with the editor's rate of M6c.Q2f (ADR-0191).
- Bad input never crashes: readers tested with damaged files at each push (ADR-0102), fuzzed from M6c.Q4 (ADR-0176).
- A crash leaves a report on the user's machine, from M6c.Q4 (ADR-0176); testers send theirs by hand. From 0.14, a user may send crash reports and hours of use, anonymous, seeing what leaves and able to stop it: the editor asks at its first start and after a crash, nothing ticked in advance, no as easy as yes (ADR-0215).
- Each stage ends with an audit, each release with its checklist (ADR-0177).
- *Blind tests by people*: captures of the engine's and Unreal's on the same scenes, or of the engine's and the games', compared without knowing which is which, at the end of each stage that touches rendering; the owner and relatives first, the community after 0.14 (ADR-0215).
- The *test of signature*: judges shown the engine's looks among other engines' games guess which share an engine; the engine's are recognised no more often than chance, at the end of each stage that touches rendering; applications on uwui take the same test (ADR-0215).
- Realism, subject by subject: skin, eyes, faces, hair, cloth, vegetation, water, sky and clouds, terrain; each against the reference path tracer, against photographs taken in known conditions and against the rivals' captures, judged blind, with its cost per tier. Faces: a performance captured with a plain camera; a face driven by its voice alone in each language, with its emotions; whole eyes, their refraction, wetness, blinks and small motions; wrinkles that come with an expression (ADR-0215).
- Stylised games of the first rank: a scene for each style, made with the engine's own art, anime with cel shading, a cartoon of the 1930s, painted, drawn by hand in 2D, never copying a game's look; tools that the artist drives, ramps of light, face shadows drawn, outlines steady at every distance, characters lit apart from the world, animation in steps with smears, a line that boils and grain, shadows and indirect light stylised; judged blind against the stylised rivals at the same cost per tier, light on the Steam Deck and the minimal tier (ADR-0215).
- Menus: a menu of reference, with the engine's own art and a strong identity; every input answered in the frame itself; every transition interrupted without a jump; no hitch as any screen opens, all preloaded; sound on the motion within a frame; within the interface's GPU time above; judged blind against the menus' rivals; no library of ready-made effects of menus, which would make menus alike (ADR-0215).
- No look of the engine: nothing is on by default. Each effect after the frame and the curve of tones are chosen in a game's *look file*; a new game starts neutral, its curve Khronos PBR Neutral written in its look file, or from one of several looks very unlike each other. The model of light is the game's, as easy to write as a material. Faces and characters come from people captured or from artists, never from one library ready to use; the store pushes no look (ADR-0215, ADR-0216).
- Physics, sound, characters' AI and animation, against the rivals with the lead's margins, their figures set at the start of the stage that builds each (ADR-0215). Physics: bodies simulated within a frame, stacks that hold still, nothing that passes through a wall at 300 km/h, the cost of each vehicle, cloth and hair. Sound: voices at once and their processor, the time from an action to its sound, no dropout, placement and muffling judged in blind listening. Characters' AI: characters fully alive within a budget, the cost of each, a path's query, the time to react, believability judged in sessions of play by the owner and by agents. Animation: feet that slide, in centimetres a step; input to a change of pose; characters animated within a budget and the cost of each; faces and reactions judged blind.
- Language models in play are optional, off by default, chosen by each game: on the machine, through the engine's own inference, within measured budgets, bound by facts that the authors write, and outside the simulation, saying words and never taking a decision that changes the world (ADR-0217).

## 5. Decisions

### Fixed

| Topic | Decision |
|---|---|
| Engine language | Rust |
| License | A licence of the engine's own, free until a game has earned 1 M$, 3 % above it, 2 % with the engine's services; an interim notice until its text is ready (ADR-0214); uwui under MIT OR Apache-2.0; code published from 2026-11-05 to the notice under MIT OR Apache-2.0, before it under MPL-2.0; the examples, the templates and the test games' code under MIT-0, made for code that people copy (ADR-0219) |
| GPU abstraction | wgpu, then native backends, Vulkan and Direct3D 12, at Stage 9c, sooner if the high and enthusiast tiers need them (ADR-0098, ADR-0216). Their types stay inside the `gfx` and `render` crates, and the engine's own tests, through `slime-gfx`'s `wgpu` feature (ADR-0031); the rest of the engine uses engine-owned handles. |
| The GPU's floor | Vulkan, Direct3D 12 or Metal required on every tier, no renderer on OpenGL; one floor of the GPU's features for every tier, raised at Stage 9a's start: materials in a table read by index, textures in arrays, meshes readable as storage, shaders in 16-bit floats and subgroups, measured on the Steam Deck, RADV, WARP and lavapipe. Applications on uwui keep their renderer on the CPU (ADR-0216) |
| Shaders | WGSL to start |
| First platforms | Linux (Wayland and X11) and Windows: games run on both from Stage 2 on. macOS is built and tested in CI from Stage 6d, and supported once a Mac can check it; wgpu stays for it after Stage 9c. The Steam Deck is a platform of the first rank, its criteria of verification checked before each game's release; consoles come through private layers, outside this repository; virtual reality, and mobile and web games for the engine's other users, are in the horizon (ADR-0209). Applications on uwui aim at macOS for its 1.0, Android phones and the web in a canvas too, iOS within reach through a trial (ADR-0098, ADR-0173, ADR-0181, ADR-0213). |
| UI | uwui, the owner's framework, in a repository of its own, which receives its code at Stage 6d (ADR-0194, ADR-0197), under MIT OR Apache-2.0, which the engine depends on, never the reverse; GPU-rendered, for the editor, the UI of games and applications (ADR-0170, ADR-0179, ADR-0181): a retained tree of its own, on parley, taffy and AccessKit, embedded in a host that calls it, drawn on wgpu by its own canvas (ADR-0098, ADR-0132). The dev overlay, tooling, runs on it since Stage 6b, which removed egui (ADR-0152). No Qt or GTK. A light renderer on the CPU where there is no GPU. Agents reach an application, and the engine, through one core of uwui's, its protocol and its MCP server; the engine adds its families, the world, play and editing, and its channel speaks the core's protocol (ADR-0190, ADR-0218). |
| Game-facing API | Rust first. The C ABI and C++ SDK are derived later from a proven API. |
| Windowing and input | winit, used only inside `platform`, and in uwui's desktop host; its types never leave them (ADR-0011, ADR-0181) |
| Entity model | The engine's own: entities, components stored by type, and a transform hierarchy, with `f64` translations (ADR-0079) |
| Serialization format | Driven by the engine's own reflection, without serde, but for uwui's documents, which uwui reads with serde (ADR-0181): TOML for text, sources and saves; binary formats of the engine's own for cooked data (ADR-0069, ADR-0080) |

### Open

Each is decided by ADR at the stage where it becomes necessary, not earlier. uwui's own, the notation of its UI documents among them, are in [uwui's roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#3-open-decisions).

| Topic | Candidates | Decide at |
|---|---|---|
| Physics backend | rapier3d, Jolt or other, behind an abstraction, against the needs of ADR-0098, a simulation that replays bit for bit and an API that holds (ADR-0208) | Stage 7 |
| Audio backend | To evaluate, against the needs of ADR-0098 | Stage 7 |
| Scripting language | Lua, C#, dedicated VM, WebAssembly, other; one sandboxed runtime for scripts and mods; *one language, two targets*, a game's Rust compiled to WebAssembly while developed and natively to ship (ADR-0208) | Stage 8 |
| Shader language beyond WGSL | Slang, generated WGSL, with the native backends of Stage 9c in view | Stage 9b |
| Network transport | To evaluate | Stage 10b |
| Games' interfaces | uwui's core with a layer of the engine's, confirmed by a measure; a framework of their own if it fails (ADR-0209) | M6d.10 |
| The merge tool of levels | the stage that builds it (ADR-0208) | Stage 6c's resumption |
| The light of each tier | one architecture of dynamic light; on each tier, the way of tracing whose image is closest to the reference path tracer at equal GPU time: rays traced by the hardware, tracing in software, probes updated over frames, probes cooked; ray tracing never required (ADR-0216) | Stage 9a, by measure on each tier |
| When rays start | wgpu's experimental ray queries, measured on RADV, the Steam Deck and Direct3D 12, against a native Vulkan path (ADR-0216) | Stage 9a |
| Virtual geometry's design | the base of all geometry: clusters of triangles in a hierarchy of detail, culling on the GPU, small triangles rasterized in software, and the way of applying materials that goes with it, a visibility buffer if measured better; foliage measured apart (ADR-0216) | Stage 9a's start |
| The shader's three layers | the surface, a material's; the model of light, which a game may write; the gathering of light and its visibility, the engine's, per tier; a system of shader modules that joins them, a dependency put to the owner (ADR-0216) | before Stage 7 |
| Who owns commands, undo and their journal | uwui or the engine, once actions are declared once (ADR-0218) | M6d.10 |
| A budget of pipelines per game | bounded permutations of the material graph (ADR-0208) | Stage 9b |
| The genre of the 2D game sold | the 2D test game's, its directory renamed if it differs (ADR-0219) | when it is planned, around Stage 9b |
| The module of language models in play | optional, off by default; its budgets and the facts that bind it (ADR-0217) | Stage 9b, with characters' AI |
| Learned models | in play, inference on the engine's own compute shaders on every vendor's GPU, without a vendor's kit, the upscaler and the denoiser first, then faces; in tools, capture and voice; each model's licence and training data checked; the vendors' accelerations at Stage 9c (ADR-0216, ADR-0219) | in play, Stage 9a's start; in tools, Stage 9b's start |
| Communities and site | where users talk, the site's form | 0.14 |

## 6. Repository layout

Indicative. The structure in use is described in `docs/ARCHITECTURE.md` and its rules in ADR-0004; a directory is created when its first content exists.

```
engine/
├── crates/          engine crates (core, platform, gfx, render, assets, ...)
├── editor/          editor application and editor crates
├── sdk/             C ABI and C++ wrapper (Stage 11)
├── tools/           asset cooker, CLI, profiler
├── examples/        one runnable sample per stage
├── games/           the test games (ADR-0219)
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

The stage's milestones, their order and the owner's two checkpoints are decided in ADR-0155 and ADR-0175, and detailed in `docs/stage-6c.md` while the stage is paused; one release, 0.7 (ADR-0131). The stage pauses once M6c.Q2e is done, for Stage 6d, uwui's stages to its 1.0 and Stage 6e, then resumes (ADR-0190, ADR-0220); what it gains when it resumes, each step's author and time and no work lost after a crash, is in `docs/stage-6c.md` (ADR-0220).

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
- One core for agents in uwui, through which agents reach an application and the engine: its protocol, the agent's identity, rights and families, and its MCP server; then the actions that an application declares once (ADR-0190, ADR-0213, ADR-0218)
- uwui designed as a project, then its code moved, with its history, to [its repository](https://github.com/HoneySlatt/uwui), which the engine then depends on (ADR-0197)

The stage's milestones are in `docs/stage-6d.md`, the later ones widened so that they lock none of the limits that ADR-0213 to ADR-0219 remove (ADR-0220). It has no release of its own: it is released with 0.7 (ADR-0194).

**Exit.** uwui's code is in its repository, its CI green on Linux, Windows and macOS and its builds for the web, Android and iOS; the engine depends on it, the editor's and the game's checks and budgets unchanged; a program on uwui compiles neither the renderer, the world nor the editor; how an application is written is decided in ADRs; uwui's first stage is planned.

### uwui's stages

In uwui's repository, in [its roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#4-stages): its first stage, the foundations, Opium and its Android prototype and Slatty ported, and Pluto's native client (ADR-0198, ADR-0203, ADR-0206), which holds what this stage held after the move, then its stages to its 1.0. Stage 6e, the end of Stage 6c, then Stage 7, wait for its 1.0 (ADR-0190, ADR-0194, ADR-0220).

### Stage 6e — The market measured

**Goal.** Measure the rivals of section 4 before the stages that are judged against them, as M6d.0 did for uwui (ADR-0220).

- *Hunna Wood* moved, first, to a private repository of its own, as a client from outside; `games/open-world` in its place, with the present zone and its assets, without *Hunna Wood*'s identity, story or characters; the tests, benchmarks and measures moved to it (ADR-0219)
- A tool that measures a game on PC from outside, on the owner's machine and the Steam Deck, on the criteria of section 4's references (ADR-0215)
- The light sensor's bench, which times input to photon
- A tool of blind tests
- The rivals' editors on the same level, and their compiling
- The first figures of each, in `docs/measurements.md`, each measure on the owner's games and account after the owner's consent (ADR-0215)

The stage comes after uwui's stages and before Stage 6c resumes. It has no release of its own.

**Exit.** `games/open-world` holds every check, benchmark and measure that *Hunna Wood* held, and they pass; *Hunna Wood* lives in its own repository; each tool above has measured its rivals, their first figures in `docs/measurements.md`.

### Stage 7 — Runtime systems → 0.8

**Goal.** What a game needs beyond rendering, built on a proper concurrency model.

Before it: the scenes rebuilt alike in Unreal for physics, sound and animation; runners of CI on the Steam Deck and on a desktop GPU (section 4); the ADR of the shader's three layers (section 5) (ADR-0220).

- Job system, grown from Stage 6a's task pool (ADR-0107): dependencies, loops split across threads, work stealing if a measurement asks for it
- The rule of arrival: what changes the simulation enters only at a step that the simulation chose; what serves the image alone arrives when it is ready; the stage's first work in the background follows it (ADR-0217)
- A render thread, decided by measuring the CPU that rendering still costs against the frame of delay it adds (ADR-0111)
- Hooks per component type, messages read at fixed phases, and a queue of the changes that parallel systems defer, designed with physics, audio and the events of games (ADR-0108)
- Memory: frame, arena and pool allocators where measurements justify them; allocation tracking
- Visual profiler: CPU, jobs, memory, GPU, frames, from a trace stream
- Physics behind an engine abstraction: bodies, colliders, constraints, queries (a ray or a segment against the world, ADR-0057), character controller; the backend chosen against vehicles (wheels and suspension, motorbikes, free flight), walking inside a moving vehicle, 2D, characters driven by physics, worlds in double precision and a simulation that runs the same way twice (ADR-0098)
- Riding: a character attached to a mount or a seat, with its animations, input and camera (ADR-0098)
- Audio: sources, listener, spatialization, buses, mixing, streaming; the importer of audio files, moved from Stage 4 (ADR-0068); the backend chosen against sound placed precisely around the listener's head, reverberation per room and sounds muffled by walls (ADR-0098)
- The core of skeletal animation: skeletons, skinning, clips, blending, moved from Stage 9b (ADR-0097); poses that are not entities, and bones that things are attached to (ADR-0108)
- Input action mapping, on the keys and buttons held that Stage 6a keeps (ADR-0107)
- UI of games: text, menus, HUD, on uwui, whose first stage gives it motion, transforms, masks, text styled by ranges and focus driven by gamepads (ADR-0170, ADR-0194); UI on surfaces in the world, screen readers, and UI documents, the games' screens as data (ADR-0098, ADR-0132); the engine's layer over uwui's core, drawn in the render graph with the game's materials, 3D within an interface, the game's time (ADR-0209); the foundations of section 4's menus: every input answered in the frame, transitions interrupted, screens preloaded, sound on the motion (ADR-0215)
- Physics', sound's and animation's criteria (section 4), their figures set at the stage's start, measured against Unreal's on the scenes rebuilt alike (ADR-0215)
- Games get the registry of operations that the editor has, and input actions that tester agents drive (ADR-0220)
- The simulation on every core in a fixed order, replayed bit for bit on one build and one platform, checked in CI by a hash of the world at each step; across platforms as an option; time and random numbers from the engine, seeded (ADR-0208)
- Rewinding while a game is played in a development build: its last seconds kept, stepped frame by frame, each value read (ADR-0209)
- Replays that drive recorded runs through the test games at each push, finding the commit that breaks or slows one (ADR-0208)

The stage is split in three when it is planned: execution, determinism and replays, the profiler and the runners; physics, riding and animation's core; sound, input, games' interfaces and accessibility (ADR-0220).

**Exit.** The test games have an animated character in a physical level, with sound and a UI, and are profiled end to end; physics, sound and animation meet their criteria against Unreal's scenes; the tester agents play the test games end to end and find nothing that blocks, crashes or misses a budget (ADR-0220).

### Stage 8 — Extensibility → 0.9

**Goal.** Others can extend the engine without modifying it.

- Module activation: a game enables only what it needs
- Plugin architecture for runtime and editor
- Scripting (language decided here), bound through reflection, with reload and debugging
- The world updated in place when its scenes change while a game runs, keeping the handles that the game's code holds, moved from Stage 6 (ADR-0089, ADR-0155)
- Mods for every game that enables them: data first; code only in the sandbox that scripts share, with declared rights, limits of memory and time, a failing mod stopped alone; manifests and plugins signed; the editor as the modders' tool (ADR-0208)
- A game's Rust code reloaded hot, its state kept, within the budget of section 4 (ADR-0208, ADR-0215)

The C ABI and the C++ SDK moved to Stage 11, derived from the API that the editor and the game will have proven (ADR-0097).

**Exit.** The test games' gameplay lives in plugins using only public API.

Stage 9 is split in two (ADR-0052): the narrative vertical slice of Stage 9b needs a production renderer first.

### Stage 9a — Production renderer → 0.10

**Goal.** Render what a shipped game shows, beyond a simple scene.

The stage is in three parts, split when it is planned, the foundations first, before any technique is chosen (ADR-0216, ADR-0220).

The foundations:

- Compute shaders and indirect draws in the GPU layer (ADR-0115); the GPU's floor raised for every tier (section 5)
- The shader in three layers, joined by a system of shader modules (section 5); the editor's views and the interfaces of games drawn with materials follow them
- The render graph open to games, in the stage's first milestone: passes that a game registers; buffers and compute in the graph; a size drawn apart from the size shown; targets kept from one frame to the next; a model of resources ready to drive explicit barriers and the aliasing of memory on the native backends (ADR-0216)
- The sky computed on the GPU, which a game may replace
- The reference path tracer first, the ground truth of image tests and visual targets (ADR-0208), and the measure of an image's closeness to it; tracing in software built first, which serves the path tracer on CI's CPU drivers, the modest tier and the probes; the surface and the light that a ray meets computed by the same code as the rasterizer's (ADR-0216)
- Virtual geometry, the base of all geometry, as its design decides (section 5); meshes cooked in clusters come with Stage 9b's cooked archives (ADR-0216)
- Learned inference on the engine's own compute shaders (section 5)
- The backends decided by measure: when rays start (section 5), and the native backends of Stage 9c brought sooner if the high and enthusiast tiers need them (ADR-0216)

Light:

- Dynamic light without long bakes, one architecture for every tier, each tier's way of tracing chosen by its closeness to the path tracer at equal GPU time (section 5); artists light once, whatever the tier (ADR-0216)
- Shadows: shadows of point lights, the tiles of spot lights' shadows sized by how large each light shows on screen (ADR-0058)
- Many lights, by a method that keeps several ways of lighting possible in one frame: clustered forward is expected, chosen here by measurement (ADR-0098)
- The learned denoiser; temporal anti-aliasing; the engine's own temporal upscaler, learned, and FSR, the upscaler run before the effects after the frame (ADR-0216)
- The mode of performance at 120 frames a second (section 4)
- Quality set per hardware tier (section 1, ADR-0098), each technique stating its tier and its fallback, measured against the tiers' budgets (ADR-0114)

Looks and subjects:

- The look of a game in its look file: the curve of tones, each effect after the frame, its color grading; the model of light that a game writes; passes of the game's own and the kinds of materials that it registers (ADR-0098, ADR-0111, ADR-0215)
- Stylised tools, as section 4's bar lists them, and the pieces for 2D and stylized games that no earlier stage needed (section 1, ADR-0215)
- Realism, subject by subject (section 4): skin lit through subsurface scattering, eyes, hair; transparency, fog, ambient occlusion, decals; a day-night cycle; HDR output (ADR-0098, ADR-0215)
- The stability of the image as a measured goal: no shimmering, no trails, no detail that pops in (ADR-0098)
- The blind tests by people and the test of signature (section 4)

**Exit.** A level of each 3D test game, outdoors and indoors, with a day-night cycle, many lights, transparency and anti-aliasing, within the frame budgets of section 4 on each tier, covered by image tests; each tier's image measured against the reference path tracer; the blind tests and the test of signature passed (ADR-0220).

### Stage 9b — Content production → 0.11

**Goal.** A team can produce a narrative game.

The stage is in three parts, in this order, split when it is planned: the chain of production first, so that the first game sold does not wait; then characters, faces and AI; then story, sequencer, menus' timeline and effects (ADR-0220). Inference in tools, capture and voice, is decided at its start (section 5).

The chain of production:

- Material graph → IR → shader generation; values set per instance (ADR-0057); variants, permutation cache, async compilation; a budget of pipelines per game, every pipeline listed when cooked, none created after a level starts (ADR-0208)
- Cooked archives redesigned, with meshes cooked in clusters for virtual geometry (ADR-0216)
- Build, cook and package: Development, Shipping, Server, Editor configurations, for Linux and Windows; packages for applications too (ADR-0173). Builds without a console (Windows games) write their log to a file.
- Updates of shipped games: archives, patches that download only what changed, downloadable content and expansions as packages added to an installed game; from its first release, a save written by a shipped version of a game loads in all of its later versions, checked in CI with one save fixture per shipped version (ADR-0100); the same updates serve applications (ADR-0173)
- A game built, sent to a paired device, started and driven from the editor in one action; Steam's integration as an optional crate, Steam Input and its glyphs, the keyboard on screen, saves in its cloud (ADR-0209)
- The first game sold on the engine, the 2D test game: its code public under MIT-0, its content, levels, art, music and identity, in a private repository (ADR-0207, ADR-0219)

Characters, faces and AI:

- Animation beyond the core that Stage 7 builds (ADR-0097): state machines, blend trees, IK, retargeting, root motion; motion matching and quadrupeds; 2D animation (ADR-0098); animation in steps, with smears, for stylised games (ADR-0215)
- Faces beyond blend shapes, with learned correctives; a face driven by its voice alone in each language, with its emotions; a performance captured with a plain camera; lip sync (ADR-0098, ADR-0215)
- Hair, cloth and flesh simulated; reactions physical, blended with animation (ADR-0215)
- A track of combat for each animation: a blow's frames, its zones that hit, its windows to cancel, its stops on impact (ADR-0208)
- Navigation: navmesh, pathfinding, spatial queries; the AI of characters beside it (ADR-0098), classical and at the market's best: perception, behaviours edited as graphs, schedules, memory of the player's acts and factions saved with the story's state (ADR-0208, ADR-0217); characters' AI's criteria (section 4), and the distance within which no extra is hollow
- Language models in tools write variants of lines and reactions, which authors read and freeze; the optional module of language models in play is decided here (section 5, ADR-0217)

Story, sequencer and effects:

- Dialogue and quests, with the state of a story that they read and that scenes depend on; the rules of a particular game stay in the game (ADR-0098); layers of a level that exist while a condition on the story holds (ADR-0112)
- Translation tables and voice (ADR-0098)
- Sequencer: camera, animation, audio, event and VFX tracks; a timeline that animates menus without code, for designers and agents, a general need of uwui's (ADR-0215)
- VFX: CPU and GPU particles

**Exit.** The 2D test game is sold, through the chain from the store to an update; the open-world test game grows into a short narrative vertical slice (animated character, cutscene, gameplay) that ships as a packaged build; faces and reactions judged blind, and the test of signature passed (ADR-0215, ADR-0220).

Stage 9c is added by ADR-0098: the rendering of the games of section 1, and consoles one day, need native graphics APIs.

### Stage 9c — Native GPU backends

**Goal.** Give the renderer the features of the GPU that wgpu lacks or holds as experimental.

- Vulkan and Direct3D 12 backends behind the engine-owned API of `slime-gfx`
- Work on several queues of the GPU in parallel, ray tracing, mesh shaders, resources without a declaration per binding, explicit memory, upscalers and frame generation, the vendors' tools for latency and GPU crashes
- wgpu stays for macOS, through Metal (ADR-0173); whether it also stays as a fallback or for debugging
- The enthusiast tier's top rung, path tracing in real time, here unless Stage 9a brought it on wgpu (section 4, ADR-0216)
- The vendors' accelerations for learned models, DLSS and XeSS as optional crates (ADR-0216)

Before this stage, a feature that wgpu blocks goes through wgpu's access to the API underneath it. The stage comes right after Stage 9b and has no release of its own; what it delivers is released with 0.12 (ADR-0131, ADR-0170, ADR-0194).

**Exit.** The game runs on the native backends on Linux and on Windows, and the image tests pass on them as on wgpu.

Stage 10 is split in two (ADR-0097): streaming, which an open world needs, does not wait for networking, which the first game does not need.

### Stage 10a — Large worlds → 0.12

**Goal.** Scale beyond a single level.

The stage is in two parts, split when it is planned (ADR-0217, ADR-0220).

Streaming and the data's foundations:

- World streaming: cells, async I/O, priorities, memory budgets, HLOD; cells computed by cooking and kept out of identities, what refers to each other loaded together, a region prepared in a second world then switched to with what follows the player (ADR-0112); textures streamed by their levels of detail, and a priority computed at each frame within the classes of requests (ADR-0113)
- Dense content stored densely: compact typed arrays per cell, its edits kept as differences; an object becomes an entity only when something acts on it, a tree felled, a wall destroyed (ADR-0217)
- Generated scenes, and what is generated named by its generator, the generator's version, its seed and its region, its base generated again when it loads; compact identities; a budget of spawning per frame; saves per region; the save's format gains generated scenes before any game ships (ADR-0217)
- Large-world coordinates
- High-scale rendering beyond Stage 9a's virtual geometry and light: GPU-driven rendering, advanced culling, virtual texturing, each admitted against a measured problem (ADR-0216)
- No visible limit to the distance seen (section 4)
- One file per cell; a budget of cooking time per km² (ADR-0208, ADR-0209)

Destruction, generation and the living world:

- Destruction, on Stage 7's physics: what an artist cut beforehand, and what breaks where it is struck; the same everywhere, kept in saves, the same on every machine online, updating light and the AI's paths; judged blind against the rivals, with the lead's margins on its cost; no world of voxels (ADR-0217)
- Procedural generation. At creation: graphs of rules for terrain, rivers, roads, vegetation, villages and buildings, their result frozen, then changed by hand. In play: small details seeded on the GPU, the same each time; levels or worlds generated from a seed, as each game chooses, the same on every machine, in replays and online; a save keeps the seed and the changes; generation runs in the background within the budgets of frames. The space opera's planets stay made by hand (ADR-0098, ADR-0217)
- Terrain, vegetation and their editing tools
- Crowds and traffic, of thousands of characters, in the three levels of life of section 4 (ADR-0098, ADR-0215)
- Terrain that deforms, hair drawn strand by strand, simulated cloth (ADR-0098)
- Water and weather (ADR-0098)
- The test worlds of section 4, made with the tools of generation, their content mostly generated (ADR-0217)

**Exit.** The test worlds are crossed within the open world's budget of section 4, on every tier; destruction and generation replay and save the same (ADR-0220).

### Stage 10b — Networking → 0.13

**Goal.** Scale beyond a single machine.

- Networking: transport, replication, RPC, prediction, reconciliation, lag compensation, interest management for hundreds of players in one place (ADR-0098); what a run creates numbered by the server, what scenes place named alike on every machine (ADR-0109)
- Synchronized physics, proximity voice chat, games hosted by a player with relays, a world split between servers (ADR-0098)
- The threat model of ADR-0096 extended to the network (ADR-0098)
- The data that servers keep for players converted from one version of a game to the next (ADR-0100)
- Dedicated server build
- No anti-cheat in the kernel; Easy Anti-Cheat and BattlEye as optional crates; the delay felt from an input to its effect, budgeted; a simulator of networks in the editor and CI; a host that leaves replaced; players of different stores together; the relay and the server, their protocols published, run anywhere (ADR-0207, ADR-0208)
- The network's criteria of section 4, against the rivals, the server's cost per player among them (ADR-0215)
- Destruction and generation in sync online, the seed sent rather than the data (ADR-0217)

The stage is split in two halves when it is planned (ADR-0098).

**Exit.** Several clients play the test games connected to a dedicated server; the network's criteria of section 4 are met; destruction and generation stay in sync online (ADR-0220).

### Stage 11 — Production readiness → 0.14, then 1.0

**Goal.** External projects can rely on the engine.

- Complete editor: material, animation, VFX and audio editors, sequencer, profiler, build tools, plugin manager, panels in windows of their own (ADR-0155); the palette of commands came with Stage 6b (ADR-0151), and saved layouts come with Stage 6c
- Editor extensibility: custom panels, inspectors, importers, asset types, build steps
- C ABI: opaque handles, versioned structures, function tables, explicit ownership, explicit error codes; moved from Stage 8 (ADR-0097)
- C++ SDK as an ergonomic wrapper over the C ABI, with a C++ sample that builds against it
- Crash reporting and diagnostics; reports and hours of use sent by the user's choice from 0.14 (section 4, ADR-0215)
- Migration tooling for assets, projects and serialized formats
- Deprecation policy
- macOS as a supported platform, unless a Mac has checked it sooner (ADR-0173)
- User, SDK and engine documentation: a book from nothing to a game shipped, the API's reference, examples that run, a site, checked in CI; templates of games drawn from real ones (ADR-0207)
- Several people and agents in one level at once; the editor's panel of agents, and an assistant within it as an optional crate; a visual editor of games' interfaces; Git within the editor, Lore and Perforce later (ADR-0209)
- The engine announced, contributions opened, a policy of its name and marks, at 0.14 (ADR-0207); the engine's licence and the contributors' licence agreement in force (ADR-0214)
- Documentation written for agents ships with the engine; a command that makes a new game and writes its `AGENTS.md`, and cooking that works outside this repository, before the first agent bench (ADR-0215, ADR-0220)

**Exit for 0.14.** An external project is built without touching engine source; the engine's licence and the contributors' agreement are in force; the agent bench has run (ADR-0220).
**Exit for 1.0.** A compatibility promise can be made and kept. There is no deadline for this.

## 8. Continuous tracks

These run through every stage. They are never "a phase for later".

| Track | Content |
|---|---|
| Testing and CI | Unit, integration, image comparison, serialization round-trip, budgets kept by counters and ratios of times (section 4) |
| Platforms | Every stage's test games build and their tests pass in CI on Linux and Windows; windowed behaviour is checked by hand on both |
| Profiling | Instrumentation added with each system |
| Performance budgets | The quality bar (section 4), measured at each milestone that touches it |
| Rivals measured | From Stage 6e on, the rivals of section 4 measured again as their versions change; each stage judged against them, and each stage that touches rendering ending with its blind tests and its test of signature (ADR-0215, ADR-0220) |
| Iteration speed | Compile times measured at each milestone against their budgets (section 4), by `scripts/compile-times.sh`: one set of features where a program and the game that its Play builds meet, so that an engine change is compiled once for both; dependencies optimised in debug builds; mold as the linker on Linux; a profile for shipping with optimisation across crates (ADR-0220, ADR-0221). Hot reload, startup time; a change seen within its budget (ADR-0208) |
| Diagnostics | Logs, assertions, crash reports, hours of use counted (ADR-0176, ADR-0215) |
| Documentation | ADRs, architecture notes, API docs, sample walkthroughs |
| Samples | One runnable example per stage, kept working; the test games, each grown by what its stage needs and run at its end (ADR-0219) |
| Agents | From Stage 6 on, each system exposed to AI agents through the engine's MCP server as it is written (ADR-0099), on uwui's core for agents (ADR-0218), with rights for each agent, previews before applying, tools found by families and games replayed from a seed; a paired machine on the local network in development builds (ADR-0209). *Tester agents* play the test games end to end through the MCP server at the end of each stage, and report what blocks, crashes or misses a budget; the agent bench runs at each release once it can (section 4); each new game gets an `AGENTS.md` (ADR-0215) |

## 9. Horizon

Direction only. None of this is planned work until Stage 11 is reached.

- **Online platform for MMO-scale games.** A closed, paid service of the owner's, the engine complete without it (ADR-0207). Distinct from the engine: gateway, authentication, persistence, world and instance services, chat, economy, group finding, anti-cheat, observability, deployment. The engine supplies serialization, replication, headless servers and world partitioning.
- **Consoles.** Only with official SDK access, and hardware to test on. Built on the native backends of Stage 9c (ADR-0098), each a private layer, outside this repository, for approved studios (ADR-0209).
- **Virtual reality** through OpenXR, after Stage 9c, when a game asks; **mobile and web games** for the engine's other users, after 1.0 (ADR-0209).
- **Large-team workflows.** Asset locking, distributed builds and cooking, shared derived-data cache; editing at several moved to Stage 11 (ADR-0209).
- **Ecosystem.** A store of plugins and assets after 0.14, its plugins checked and signed, 10 % of sales, none in a creator's first year (ADR-0207, ADR-0214); templates come with Stage 11.
- **Products around the engine,** closed, outside this repository (ADR-0214): support for studios, maintenance extended, services of experts and licences made to measure, after 0.14; a paid programme of partners, from 0.14; a hosted assistant in the editor, beside the agents that people bring free through the MCP server; the online services, paid by use at first.
- **Governance.** A foundation, a second maintainer and a plan of continuation, after 1.0 (ADR-0207).
- **Visual scripting** of every logic. Node graph → IR → VM or native code, once reflection, scripting and undo/redo are mature. The graphs of each domain, dialogue, animation, materials, sound, characters' AI, come with their stages (ADR-0209).
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
| destruction | physics, world, navigation (ADR-0217) |
| procedural generation | world, assets, jobs (ADR-0217) |
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
| 0.7 | 6c | Editor, after uwui's 1.0 and Stage 6e: the stage paused once M6c.Q2e is done (ADR-0190, ADR-0220) |
| — | 6d | uwui leaves the engine, released with 0.7 (ADR-0194) |
| — | — | uwui's stages, to its 1.0, in its repository, before the end of Stage 6c (ADR-0190) |
| — | 6e | The market measured, no release of its own (ADR-0220) |
| 0.8 | 7 | Runtime systems |
| 0.9 | 8 | Plugins, scripting |
| 0.10 | 9a | Production renderer |
| 0.11 | 9b | Content production |
| — | 9c | Native GPU backends, released with 0.12 |
| 0.12 | 10a | Large worlds |
| 0.13 | 10b | Networking |
| 0.14 | 11 | Usable by external projects |
| 1.0 | 11 | Stable API and SDK |
