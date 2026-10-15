# 0072. Stage 4b: live assets

- Status: Accepted
- Date: 2026-10-15

## Context

Stage 4b, the second half of the asset pipeline ([ADR-0068](0068-stage-4-in-two-halves.md)), makes
loaded assets live. Its exit, in the roadmap: a texture and a shader hot-reload in the running game;
the game's zone loads while frames go on; the game goes on after a lost GPU device. Where the engine
stands when the stage starts:

- `Renderer::load_scene` loads a cooked scene on the thread that calls it. *Hunna Wood* loads its
  zone, 157 MiB of cooked data, before its window opens.
- A material cannot change once it is made: a game that wants a value to change smoothly, a fire
  that flickers, has no way to ([ADR-0057](0057-what-the-first-game-asks-of-the-next-stages.md)).
  Hot reload needs the same thing.
- The renderer's WGSL shaders are embedded in the program when it is compiled.
- A lost GPU device stops the run, and the engine opens no new one
  ([ADR-0022](0022-losing-the-gpu-device.md)).

Three design studies of the code preceded the plan. The owner took the decisions below on
2026-10-15, one by one, when the stage was planned.

## Decision

**The order of the work: the risk first.** Loading in the background comes first, then the recovery
from a lost device, which reloads with it and holds the most unknowns (drivers, Windows), then
materials that change in place, hot reload of cooked data, hot reload of shaders, and a refining of
the game that uses hot reload.

**Loading in the background.**

- The renderer starts loading a scene and returns a value that the game polls at each frame; when
  the load is done, the game takes the whole scene from it, never a part. Loading a scene and
  waiting for it, `load_scene`, stays.
- The threads that load create the GPU objects themselves, through the `Gpu` they share.
- They run on a task pool that is `slime-render`'s own and private: as many threads as the
  machine's cores but one, at least 1 and at most 4. It is no engine facility; Stage 7's job system
  replaces it ([ADR-0002](0002-amend-roadmap-stages-0-to-3.md)).
- The bar: while *Hunna Wood*'s zone loads, no interval between frames above 33 ms on the owner's
  RX 6900 XT, in a release build at 1280 × 720. If it is missed, the loading threads only read and
  decode, and the main thread uploads, spread over frames.
- While its zone loads, *Hunna Wood* draws its sky and its hero at the start: the camera turns
  around the hero, who does not move, and the zone appears whole once loaded. The same view shows
  while the zone loads again after a lost device. The game's `--frames` counts the frames that draw
  the zone.

**Recovering from a lost GPU device.**

- An application chooses to recover, by a setting of its `AppConfig` and a method of `App` that the
  run calls once a new device is open; the application creates its GPU objects again there. An
  application that does not choose it stops as ADR-0022 says.
- The engine opens the new device through the same wgpu instance, and creates the window's surface
  again, which a new device needs on Vulkan and Direct3D 12.
- It refuses a CPU adapter after losing a hardware one, and gives up after three recoveries in a
  row without a presented frame, when the device ran out of memory, or when no adapter opens.
- *Hunna Wood* chooses to recover: it loads its zone again in the background, behind the loading
  view. A `--lose-device-at <frame>` option of the game and a button of its dev overlay lose the
  device on purpose, for the checks and by hand.

**Materials that change in place.** A game changes a material's values while it runs: base color,
metallic, roughness, normal scale, occlusion strength, emission and alpha cutoff. Its textures, alpha
mode and double-sidedness stay as made. A game finds the materials of a loaded scene by their names.
*Hunna Wood*'s campfire flickers, and its key glows in pulses at night.

**Hot reload of cooked data.**

- `slime-cook watch` cooks again what changes in a directory of sources: it checks the sources every
  half second, with the standard library only. Concurrent cooks stay safe: temporary files are named
  with the process ID, and the manifest is written last, only when its content changes.
- A game that hot-reloads checks its manifest. When the manifest changes, the renderer loads the
  scene again in the background, reusing the GPU objects that did not change, and the game swaps
  the scenes between two frames. A reload that fails keeps the old scene and logs why.
- *Hunna Wood* reads its zone's rules again from a reloaded scene. When the game's layout changed
  (colliders, the start, the key, the door, the campfire, the terrace, the bounds), the run starts
  again from the start, as the restart key does; when only the looks changed, the run goes on.

**Hot reload of shaders.** The renderer's shaders stay WGSL files beside its Rust code, embedded in
the program. A game that hot-reloads reads them from `crates/slime-render/src` while it runs and
gives the renderer new sources when they change; the renderer makes all its pipelines again, or
none: a shader that does not compile keeps the old pipelines, and its error names its line and
column. Uniform layouts declare their minimum sizes, so that a shader that does not fit its data is
refused when its pipeline is made, not when it draws.

**Hot reload is off by default**, in debug and release builds alike: a game turns it on with
`--watch`.

**The rest of the stage.**

- A milestone refines *Hunna Wood* with hot reload, on a list of at most five points that the owner
  chooses at its start, without new engine features. New source assets, and new releases of the
  mirror, still wait for the owner's agreement ([ADR-0070](0070-the-sample-games-source-assets.md)).
- The stage has no release: what it delivers is released with 0.3, at the end of Stage 5
  (ADR-0068).
- CI caches the cooked zone only if its Windows job, at 17 minutes when the stage starts, passes 19,
  near the 20 minutes the owner allows.
- No new dependency.

**The roadmap.** The dependency table's `assets` row, which promised "a basic task pool from
Stage 4b", loses the promise; the `render` row gains its own task pool for loading, from Stage 4b.

## Alternatives considered

- **The roadmap's order, recovery last**: hot reload sooner, but the riskiest work at the end, where
  an overrun delays the stage's close.
- **Handles with load states**, which the game holds while assets load and which hot reload swaps
  behind it: progressive loading and assets shared between scenes, but they are stable references to
  assets, which Stage 5's world model decides; every draw would go through a registry, and nothing
  needs them yet. The pool and the loading threads of this decision serve them when they come.
- **Threads that only read, and a main thread that uploads within a budget of bytes per frame**:
  it avoids a lock of wgpu's queue that loading threads take while they write a texture and that a
  submission takes too, but contradicts the roadmap and loads slower. It is the fallback if the bar
  is missed.
- **The pool in `slime-assets`, or in a crate of its own**: `slime-assets` has no GPU, and the
  loading threads create GPU objects; a public pool would be an engine facility with one use.
- **A loading bar**: the engine has no text or UI for games before Stages 6 and 7, and a scene that
  loads whole has no progress to show. **The sky alone**: it reads as a frozen game.
- **Watching with the operating system's notifications** (the `notify` crate): instant, but a new
  dependency, with known misses (editors that save by replacing a file), to gain half a second.
  **The game running the cooker**: every game would link the importers and encoders, which
  ADR-0069 keeps out of shipped programs.
- **Shaders as cooked data**: one path for every asset, but the renderer could draw nothing,
  not even a test's triangle, without cooked data, and cooking WGSL would copy it.
- **Hot reload on by default in debug builds**: no option to type, but debug and release builds
  would behave differently, and a shader being written, broken, would fail the game's start.
- **Keeping the zone's rules as at startup**: what the player sees and what stops them would differ
  until a restart. **Keeping the run through a change of layout**: each odd case (the hero inside a
  new wall, a key taken that moved, a door removed) would need a rule of its own.
- **Recovery for every application**: every example would write its recovery, for nothing.
  **Recovery the engine does alone**: it would keep a copy of everything it sent to the GPU, and a
  registry of its objects, the handles above.
- **A larger milestone of game content**: combat, characters or a second zone need animation,
  physics and audio, later stages' work.

## Consequences

- The milestone that brings recovery amends ADR-0022, and records its design in an ADR of its own;
  so do the milestones that bring loading in the background and hot reload.
- A game polls its loads, and swaps whole scenes on hot reload; a scene arrives whole, with no
  progressive loading until handles come.
- A shipped game cannot hot-reload its shaders: it has no sources beside it. Its Windows programs
  hold no cooker either, so hot reload is checked by hand on Linux, and on Windows by CI's tests.
- A game that hot-reloads watches two things, its manifest and the shaders' directory, with one
  loop.
- Reversing a decision costs what its milestone built; nothing else depends on the order of the
  work.
