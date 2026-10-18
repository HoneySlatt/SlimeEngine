# Architecture

This document maps the code: which crates exist, what each one is responsible for, how they
depend on each other, and where to start reading. It describes the repository as it is now;
[`ROADMAP.md`](ROADMAP.md) describes where it is going, and [`adr/`](adr/README.md) records why
it is built this way.

## Repository layout

```text
.
├── .github/workflows/   CI: scripts/check.sh on Linux; tests and samples on Windows
├── Cargo.toml           virtual workspace: shared metadata, lints and dependency versions
├── Cargo.lock           the exact version of every dependency, checked in
├── crates/              engine crates, one directory per crate, named after the package
│   ├── slime-app/       application lifecycle and main loop
│   ├── slime-assets/    assets as games load them: asset IDs, cooked data, the manifest
│   ├── slime-color/     the color type of every API: linear colors, the sRGB transfer function
│   ├── slime-cook/      the asset pipeline's cooker, a library and its program
│   ├── slime-devui/     dev overlay (egui), temporary tooling
│   ├── slime-gfx/       GPU layer (wgpu)
│   ├── slime-platform/  window, its events and input
│   ├── slime-reflect/   reflection: types described at run time, their values read and set by path
│   ├── slime-reflect-derive/  the derive macro of reflection, which slime-reflect re-exports
│   ├── slime-serialize/ serialization: reflected values in TOML documents, or a binary encoding
│   ├── slime-render/    renderer: meshes, PBR materials, camera, lights, shadows, HDR, tone mapping
│   ├── slime-world/     the world: a game's entities, their components and their hierarchy
│   └── slime-world-render/  the bridge from a world to the renderer: what a world draws, each frame
├── deny.toml            dependency license, advisory and source rules
├── docs/                roadmap, plan, decisions (adr/), glossary, dependencies, this file
├── examples/            runnable examples, one per feature, with their tests (`slime-examples`)
├── flake.nix            Nix development shell
├── flake.lock           pinned inputs of the development shell
├── games/               games made with the engine through its public API
│   └── hunnawood/       Hunna Wood, a role-playing game: the sample every stage grows
├── rust-toolchain.toml  pinned Rust toolchain
├── scripts/             check.sh, every automated check as CI runs them; the scripts that fetch
│                        DXC and the sample game's source assets, and write their credits
├── AGENTS.md            rules for every contributor
├── CHANGELOG.md         notable changes, per release
├── CONTRIBUTING.md      environment, checks, how a change is made and reviewed
├── LICENSE              MPL-2.0
└── README.md            what the project is, its state, where to read next
```

[ADR-0004](adr/0004-workspace-layout-and-crate-naming.md) sets the layout and naming rules: the
root is a virtual workspace, engine crates are `crates/slime-<role>/`, games are
`games/<name>/` ([ADR-0055](adr/0055-games-in-the-workspace.md)), and a directory or a crate is
created only when code needs it.

## Crates

| Crate | Role | Depends on | Status |
|---|---|---|---|
| [`slime-platform`](../crates/slime-platform/README.md) | The window, its events and keyboard and mouse input; the only crate that uses the windowing library (winit) | `winit`, `raw-window-handle`, `thiserror`, `tracing` | One window; window, keyboard and mouse events |
| [`slime-app`](../crates/slime-app/README.md) | Application lifecycle, main loop, frame timing, diagnostics setup, headless mode | `slime-platform`, `slime-gfx`, `slime-world`, `ctrlc`, `thiserror`, `tracing`, `tracing-subscriber`, `nu-ansi-term` (Windows only) | Headless runs, and windowed runs drawn by the GPU, which go on with a new device after a lost one when the application asks |
| [`slime-gfx`](../crates/slime-gfx/README.md) | The GPU layer; the only crate that uses wgpu, apart from the dev overlay through its `wgpu` feature | `wgpu`, `pollster`, `raw-window-handle`, `slime-color`, `thiserror`, `tracing` | Opens a GPU device, and another in place of a lost one; draws into offscreen images, 8-bit or 16-bit float, and reads them back; draws into a window; later passes sample what earlier ones drew; depth-only passes; images of four samples per pixel, resolved for anti-aliasing; textures and cube textures whose levels the caller gives, half-precision floats and BC7 and BC5 blocks among their formats |
| [`slime-color`](../crates/slime-color/README.md) | The color type of every API of the engine, `LinearColor`, and the sRGB transfer function, below every crate that holds a color ([ADR-0045](adr/0045-one-linear-color-type.md), [ADR-0079](adr/0079-the-engines-own-world.md)); headless | `slime-reflect` | Linear colors and their constructors, sRGB bytes decoded and encoded |
| [`slime-render`](../crates/slime-render/README.md) | The renderer: draws instances of meshes with their materials, lit by directional, point and spot lights, seen from a camera, into render targets, and reports what a frame cost; built on `slime-gfx`'s types, without wgpu | `slime-gfx`, `slime-assets`, `slime-color`, `glam`, `thiserror`, `tracing` | Textures and scenes cooked by the asset pipeline, loaded onto the GPU, BC7 and BC5 among them, normal textures read by x and y; meshes with tangents, simple shapes made in code (a cube, a sphere, a square), glTF's metallic-roughness materials (five textures, masks, double-sided, values that change while a game runs, found by name in a loaded scene), a perspective camera, the depth test, up to 16 directional, point and spot lights in physical units (`KHR_lights_punctual`), a procedural sky computed on the CPU that surfaces reflect (image-based lighting) and that is drawn behind them with its sun's disc, the shadows of the first directional light that casts them (four cascades in one atlas, filtered over 5 × 5 texels) and of every spot light that casts them (a tile each of one atlas), mirrored instances drawn with their front faces, the scene drawn in HDR with its light exposed, at four samples per pixel (multisample anti-aliasing), an energy-conserving bloom, then tone mapped, the passes ordered by a render graph and recorded four to a submission, statistics with the GPU time of each pass; scenes loaded in the background, and again when their cooked data changed, keeping what did not change; the meshes a game draws kept by asset ID in a cache, loaded in batches as they are asked for, loaded again when their cooked data changes, dropped after a lost device ([ADR-0084](adr/0084-what-the-world-draws.md)); its shaders replaced while it runs |
| [`slime-reflect`](../crates/slime-reflect/README.md) | Reflection: types described at run time, their values as plain data, read and changed by property path through a registry, compared and their differences applied ([ADR-0080](adr/0080-reflection-and-serialization.md), [ADR-0085](adr/0085-reflection.md)); headless | `glam`, `slime-reflect-derive` | `Reflect` and its derive, with stable names, aliases, versions and their upgrades, default values, skipped and transient fields, ranges and units; one value type, references to assets and entities kinds of their own; descriptions of types; a registry, explicit and recursive, that holds types with no Rust type behind them, checks values, sets their parts by path within their fields' ranges and lists their leaves; the standard types, glam's `Vec3`, `DVec3` and `Quat`; no files: `slime-serialize` writes and reads them |
| [`slime-reflect-derive`](../crates/slime-reflect-derive/README.md) | The derive macro of reflection, `#[derive(Reflect)]`, which `slime-reflect` re-exports | `proc-macro2`, `quote`, `syn` | Structs and enums, their doc comments and attributes; generic types, lifetimes, unions and types without a name refused |
| [`slime-serialize`](../crates/slime-serialize/README.md) | Serialization: values of reflected types written to TOML documents by the engine's own writer and read back, from earlier versions of their types too, and the same documents in a binary encoding of the engine's own ([ADR-0080](adr/0080-reflection-and-serialization.md), [ADR-0086](adr/0086-serialization.md), [ADR-0094](adr/0094-the-binary-encoding-of-documents.md)); headless | `slime-reflect`, `toml_edit`, `thiserror` | Documents of any registered type's value, whole or in part; the same bytes for the same value; versions written past 1 and earlier values brought forward by their types' upgrades; strict and lenient reading, errors by line, column and path, limits on size and depth; documents of several values, references to entities written as the names that a world gives them, and to assets as a document names them, their IDs unless it says otherwise; a document's layout read and its errors placed by the program that reads it, as a world reads scene files; the binary encoding written from a text and read through the same reader, its errors by byte |
| [`slime-world`](../crates/slime-world/README.md) | The world: a game's entities, their components and their hierarchy, kept by the engine ([ADR-0079](adr/0079-the-engines-own-world.md)); headless | `slime-assets`, `slime-color`, `slime-reflect`, `slime-serialize`, `glam`, `thiserror`, `tracing` | Entities as handles of an index and a generation, the components attached to them stored by type, queries that read through an iterator or change through a closure ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)); parents and children, transforms with `f64` translations, a uniform scale and a stretch, and where each entity is in the world, always current ([ADR-0083](adr/0083-transforms-and-the-hierarchy.md)); scenes spawned as entities with persistent identities, glTF scenes and scene files whose placements of prefabs carry overrides, resolved by a library of scenes, and written back as scene files that keep their placements ([ADR-0089](adr/0089-scenes-and-prefabs.md), [ADR-0091](adr/0091-scene-documents-and-placements.md), [ADR-0092](adr/0092-a-world-written-back-as-a-scene.md)); what is drawn, meshes by asset ID, lights and hidden subtrees, and the transforms before the last fixed step of what moved during it ([ADR-0084](adr/0084-what-the-world-draws.md)); every component reflected, and listed, read and set by name and property path through the world's registry ([ADR-0085](adr/0085-reflection.md)); saves of what differs from the world as a game makes it at the start of a run, what it spawned while it ran saved as its scenes, loaded back ([ADR-0087](adr/0087-the-worlds-documents.md), [ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md)) |
| [`slime-world-render`](../crates/slime-world-render/README.md) | The bridge from a world to the renderer: builds, at each frame, the instances, the lights and the sky's environment that a world draws, from the GPU objects of an asset cache ([ADR-0084](adr/0084-what-the-world-draws.md)); neither the world nor the renderer depends on it | `slime-world`, `slime-render`, `slime-gfx`, `tracing` | Meshes drawn where the world draws them between fixed steps, those the cache does not hold asked for; lights at their entities; hidden subtrees left out; the game's sky around the world's sun, given once |
| [`slime-assets`](../crates/slime-assets/README.md) | The assets as games load them: asset IDs, cooked data and its manifest, read by ID from a directory of cooked data ([ADR-0069](adr/0069-the-asset-pipeline.md)); headless | `slime-reflect`, `ktx2`, `thiserror` | The asset ID and its text form, a UUID; cooked textures in KTX2 files ([ADR-0071](adr/0071-cooked-textures.md)), meshes, materials and scenes, their nodes' extras and typed components among them; cooked scene files, read as bytes ([ADR-0095](adr/0095-cooked-scene-files.md)); the manifest; reading them by ID; a watch of the manifest, for hot reload |
| [`slime-cook`](../crates/slime-cook/README.md) | The cooker: turns source assets into cooked data before games run; a library and its program, which no game links ([ADR-0069](adr/0069-the-asset-pipeline.md)) | `slime-assets`, `slime-color`, `slime-reflect`, `slime-serialize`, `slime-world`, `bevy_mikktspace`, `blake3`, `block_compression`, `glam`, `gltf`, `png`, `uuid`, `zune-jpeg`, `thiserror` | IDs for source files in metadata files beside them, read and written through `slime-serialize`, their check, and the IDs of the assets inside a file; PNG and JPEG images cooked into textures compressed in BC7 or BC5, every level computed; glTF files imported into meshes (MikkTSpace tangents), materials and scenes; the typed properties of their nodes; scene files checked and cooked into the binary encoding of documents ([ADR-0095](adr/0095-cooked-scene-files.md)); a directory cooked again only where it changed, with its manifest, once or at each change |
| [`slime-devui`](../crates/slime-devui/README.md) | The dev overlay: egui drawn over the engine's frames, fed with the engine's events, and its world panel; tooling, removed once something does at least as well ([ADR-0052](adr/0052-amend-roadmap-before-0-1.md), [ADR-0081](adr/0081-stage-5a-the-world.md)) | `egui`, `egui-wgpu`, `slime-assets`, `slime-color`, `slime-gfx` (with its `wgpu` feature), `slime-platform`, `slime-reflect`, `slime-world` | Panels over a frame; a world's entities listed by name and their components edited through reflection, each edit returned for the caller to apply; no clipboard or input methods |
| [`slime-examples`](../examples/README.md) | Runnable examples; not part of the engine | `slime-app`, `slime-assets`, `slime-color`, `slime-devui`, `slime-gfx`, `slime-platform`, `slime-render`, `slime-world`, `slime-world-render`, `png`, `tracing`; for the tests, `slime-cook` and `gltf` | `hello`, `headless`, `window`, `offscreen`, `scene`, which draws a cooked scene, `world`, which spawns a yard of prefabs from a cooked scene file and draws it from a world; the reference image tests, and tests that cook glTF files and draw them |
| [`hunnawood`](../games/hunnawood/README.md) | Hunna Wood, the game that every stage grows from Stage 4a on; a client of the engine through its public API only, not part of the engine ([ADR-0055](adr/0055-games-in-the-workspace.md), [ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)) | `slime-app`, `slime-assets`, `slime-color`, `slime-devui`, `slime-gfx`, `slime-platform`, `slime-reflect`, `slime-render`, `slime-world`, `slime-world-render`, `tracing` | A forest zone and its hero read from cooked data and spawned in the game's world, its colliders and markers as the game's components, the camera behind the hero, the hero's movement and its collisions with boxes turned about the vertical and cylinders, a key that opens a door, a campfire that flickers, the zone drawn while it loads, the run kept after a lost GPU device, the zone loaded again when its cooked data changes, and the shaders when their files do (`--watch`); the run saved on F5 and on quit, loaded on F9 and resumed on start ([ADR-0088](adr/0088-saving-and-resuming-a-run.md)) |

## Dependency rules

```text
slime-examples ──▶ slime-app ──┬──▶ slime-platform ──▶ winit
       │                       ├──▶ slime-gfx ──────▶ wgpu
       │                       └──▶ slime-world (the game's world)
       ├──▶ slime-render ──────────▶ slime-gfx
       ├──▶ slime-assets (cooked data); the tests: slime-cook (they cook first)
       ├──▶ slime-world-render ──▶ slime-world, slime-render (a world drawn)
       ├──▶ slime-devui ───┬───────▶ slime-gfx (feature wgpu) ──▶ wgpu
       │                   ├───────▶ egui, egui-wgpu ──────────▶ wgpu
       │                   ├───────▶ slime-platform (events)
       │                   └───────▶ slime-world, slime-reflect, slime-assets, slime-color (the world panel)
       └──▶ slime-platform, slime-gfx, slime-color, slime-world (their types: events, GPU objects,
            colors, the world)

hunnawood  ──▶ slime-app, slime-assets, slime-render, slime-platform, slime-gfx, slime-color, slime-devui,
               slime-world, slime-world-render, slime-reflect (public API only)

slime-cook ──▶ slime-assets, slime-reflect, slime-serialize (metadata files),
               slime-world (the layout of the scene files that it checks)
slime-render ──▶ slime-assets (cooked data)
slime-world ──▶ slime-assets (cooked scenes), slime-serialize (saves, scene files)
slime-world-render ──▶ slime-world, slime-render, slime-gfx
slime-gfx, slime-render, slime-cook, slime-world ──▶ slime-color (the color type)
slime-assets, slime-color, slime-world ──▶ slime-reflect ──▶ glam, slime-reflect-derive (reflection)
slime-serialize ──▶ slime-reflect, toml_edit (documents)
```

The graph shows the engine's crates and the third-party crates that shape them; the table above
lists every dependency.

- Dependencies between crates point one way: `slime-app` depends on `slime-platform` and
  `slime-gfx`, never the reverse, and `slime-platform` and `slime-gfx` do not depend on each
  other: the window reaches the GPU layer only as `raw-window-handle` traits. `slime-app` also
  depends on `slime-world`, since it owns the game's world
  ([ADR-0079](adr/0079-the-engines-own-world.md)).
- `slime-render` depends on `slime-gfx` only and does not use wgpu
  ([ADR-0027](adr/0027-the-render-crate.md)). `slime-app` does not depend on it: an application
  creates its `Renderer` in `init`, from the GPU of its `Context`, and calls it in `render`.
- `slime-devui` is the one crate besides `slime-gfx` that reaches wgpu, through the `wgpu` feature
  of `slime-gfx`, to draw egui with egui's own renderer ([ADR-0031](adr/0031-dev-overlay-with-egui.md)).
  The engine's own tests may enable the feature too. `slime-devui` is tooling, a client of the
  engine like the examples: no engine crate depends on it, and a game reaches the feature only
  through `slime-devui`, as *Hunna Wood* does for its dev overlay.
- A game, such as `hunnawood`, is a client of the engine like the examples: it uses the public API
  of the engine's crates only, and no engine crate depends on it
  ([ADR-0055](adr/0055-games-in-the-workspace.md)).
- `slime-world` depends on `slime-assets`, for the cooked scenes it spawns, `slime-color`, for the
  colors of its lights, `slime-reflect`, and `slime-serialize`, for its saves, and on no GPU or
  window, so that a world runs headless, in tests, servers and tools
  ([ADR-0079](adr/0079-the-engines-own-world.md), [ADR-0087](adr/0087-the-worlds-documents.md)).
- `slime-color` depends on `slime-reflect` only, so that a crate that holds a color does not need
  the GPU layer for it ([ADR-0079](adr/0079-the-engines-own-world.md)).
- `slime-reflect` depends on glam and its derive only, so that every crate that owns a value type
  can depend on it and describe the type where it is defined: `slime-assets`, `slime-color` and
  `slime-world` do ([ADR-0080](adr/0080-reflection-and-serialization.md)). Every component is
  reflected, so that a crate that defines components depends on it
  ([ADR-0085](adr/0085-reflection.md)).
- `slime-serialize` depends on `slime-reflect` and on a TOML parser only, and no crate that only
  describes its types depends on it: the crates that read and write files do, the cooker for its
  metadata files and the world for its saves ([ADR-0086](adr/0086-serialization.md)).
- `slime-world-render` depends on the world and on the renderer, and neither depends on it: the
  world holds no GPU object, and the renderer draws any list of instances it is given
  ([ADR-0079](adr/0079-the-engines-own-world.md), [ADR-0084](adr/0084-what-the-world-draws.md)).
- `slime-assets` depends on no engine crate but `slime-reflect`, and on no GPU, so that it runs
  headless; the cooker, `slime-cook`, depends on it, and no game links the cooker
  ([ADR-0069](adr/0069-the-asset-pipeline.md)). `slime-render` depends on `slime-assets` to turn
  cooked data into its own GPU objects: each system loads its own kinds of asset.
- Only `slime-platform` uses winit, and winit's types never appear in a public API
  ([ADR-0011](adr/0011-windowing-with-winit.md)). `slime_platform::Window` exposes the
  `raw-window-handle` traits so that graphics libraries can draw into it.
- Only `slime-gfx` uses wgpu, and wgpu's types never appear in a public API
  ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)), except through its `wgpu` feature, for the dev
  overlay (above). `slime-gfx` depends on no other engine crate than `slime-color`, so that it works
  without a windowing system.
- A headless run never touches the windowing system or the GPU: it does not create an event loop
  or open a device, so the windowing libraries and the Vulkan loader, which are loaded at run
  time, are not even needed. The checks run the headless sample with no display server to prove
  it.

## Main flows

### Diagnostics setup

An engine program calls `slime_app::init_diagnostics()` first. It installs a global `tracing`
subscriber writing to standard error (unless one already exists), filtered by the `SLIME_LOG`
environment variable, then a panic hook that logs panics before the standard hook prints them,
then logs one line describing the engine and the machine. From then on, every `tracing` event
from the engine, the game or a dependency goes through that subscriber. Source:
[`crates/slime-app/src/diagnostics.rs`](../crates/slime-app/src/diagnostics.rs); conventions:
[ADR-0008](adr/0008-errors-logging-and-assertions.md).

### Running an application

A game implements `slime_app::App` and calls `slime_app::run`, which owns the main loop
([ADR-0010](adr/0010-main-loop-ownership.md)). A *driver* decides when frames run and what time
they represent; every driver runs a frame the same way (`Frames::run_one` in
[`crates/slime-app/src/app.rs`](../crates/slime-app/src/app.rs)):

```text
frame                                span "frame", trace level
├── FrameClock::advance(delta)       the time the frame represents, limited
├── FixedStep::advance(delta)        how many fixed steps are due: n
├── app.fixed_update(ctx, step)      n times, span "fixed_update", between
│                                    world.begin_fixed_step() and world.end_fixed_step(): what
│                                    moves is drawn between its two transforms (ADR-0084)
├── app.update(ctx, &frame)          once, span "update": what it changes is drawn at once;
│                                    then world.update_transforms(), so that render reads the
│                                    transforms at the cost of a lookup
└── with a window:
    ├── surface.acquire(label)       the window's image, its passes named "window, frame
    │                                <index>", or none: skip drawing, after an empty submission
    │                                that lets wgpu release the frame's writes; span "acquire"
    ├── app.render(ctx, target)      draws into the image; span "render"
    └── image.present()              hands it to the window; span "present"
```

The run stops before the next frame once the application has called `ctx.request_exit()` or
`ctx.exit_with_error(error)`, the window was closed, the process received a termination signal, or
`config.frame_limit` frames have run. The frame in progress always finishes, and `app.shutdown` is
always called after an `app.init` that succeeded; when `app.init` returns an error, the run stops
at once and `run` returns it as `RunError::Init`, and an error passed to `exit_with_error` is
returned as `RunError::App` once `app.shutdown` has run. The call order is checked by
[`crates/slime-app/tests/lifecycle.rs`](../crates/slime-app/tests/lifecycle.rs) headless, and by
[`crates/slime-app/tests/windowed_lifecycle.rs`](../crates/slime-app/tests/windowed_lifecycle.rs)
with a window, which the checks run in a headless Weston compositor (ADR-0024). The profiling
spans of each frame are checked by
[`crates/slime-app/tests/profiling_spans.rs`](../crates/slime-app/tests/profiling_spans.rs)
headless, and by the `spans` scenario of `windowed_lifecycle.rs` with a window.

**Headless driver** (`AppConfig::window` is `None`), in `app.rs`:

```text
slime_app::run
├── init_diagnostics()
├── app.init(ctx)
├── loop: frame with delta = the fixed step (simulated time), back to back
└── app.shutdown(ctx)
```

**Windowed driver** (`AppConfig::window` is `Some`), in
[`crates/slime-app/src/windowed.rs`](../crates/slime-app/src/windowed.rs). winit owns the event
loop; `slime-platform` adapts its callbacks to the `EventHandler` trait, which the windowed driver
implements:

```text
slime_app::run
├── init_diagnostics()
└── slime_platform::run_event_loop(window config, driver)
    ├── winit "resumed"          create the window → driver.window_created
    │                            → Gpu::new_for_window: the GPU and the window's surface;
    │                              ctx gets the window and the GPU; app.init(ctx)
    ├── winit "window_event"     Translator::translate (event.rs) → driver.event
    │                            → Resized: surface.resize; app.on_event(ctx, &event);
    │                              CloseRequested → stop
    ├── winit "window_event"     RedrawRequested → driver.redraw_requested → frame with the
    │                            measured delta
    ├── winit "about_to_wait"    → driver.about_to_wait
    │                            → if the next frame is due: window.request_redraw();
    │                              return WaitUntil(next frame) or Exit
    └── winit "exiting"          → driver.exiting → app.shutdown(ctx); the surface, the GPU and
                                   the window are released
```

Frames run on redraw requests ([ADR-0020](adr/0020-rendering-in-the-main-loop.md)). The engine
asks for one every `1 / AppConfig::max_frame_rate` seconds: between them the event loop sleeps
until the next frame is due or an event arrives. A frame that starts a little late does not shift
the schedule, which keeps the average rate; after falling a whole period behind, the schedule
restarts instead of catching up with frames run back to back (`next_due` in `windowed.rs`). The
windowing system also asks for redraws on its own, during a resize for example, and each of them
runs a frame too; such a frame restarts the schedule as well, so the next frame the engine asks for
comes one period after it. On Windows, this is what keeps the window drawn while the user resizes it: the
system then runs its own loop, in which `about_to_wait` is not called.

### Opening the GPU

`slime_gfx::Gpu::new()` creates a wgpu instance limited to the platform's graphics API (Vulkan on
Linux, Direct3D 12 on Windows), asks for the high-performance adapter, logs it, and opens its
device and queue ([`crates/slime-gfx/src/gpu.rs`](../crates/slime-gfx/src/gpu.rs),
[ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). Without a GPU, the adapter is a software one:
lavapipe on Linux, WARP on Windows; the checks force lavapipe on machines that have a GPU by
restricting the Vulkan loader to it with `VK_DRIVER_FILES`. Headless runs never open the GPU.

`slime_gfx::Gpu::new_for_window(window, width, height, present_mode)` does the same for a window:
it creates the window's surface from the window's `raw-window-handle` traits first, asks only for
adapters that can present to it, and configures it in the first sRGB format the window offers and
the requested present mode, FIFO when the window does not offer Mailbox
([`crates/slime-gfx/src/surface.rs`](../crates/slime-gfx/src/surface.rs)). The surface keeps the
window alive, which is why the windowed driver releases it before the event loop closes its
connection to the windowing system.

### Rendering offscreen

Drawing into an image without a window, as the `offscreen` example and the reference image test
do with the triangle of [`examples/common/triangle.rs`](../examples/common/triangle.rs):

```text
gpu = Gpu::new()
├── create_render_target(label, width, height, format)   the image to draw into, 8-bit or
│                                                        Rgba16Float; later passes can sample it
├── create_depth_target(label, width, height)            the depth of what is drawn, for passes
│                                                        that test depth; later passes can sample
│                                                        it (a shadow map)
├── create_multisampled_render_target(label, width, height, format)
│                                                        four samples per pixel, for anti-aliasing;
│                                                        passes only draw into it, and resolve it
├── create_multisampled_depth_target(label, width, height)
│                                                        a depth per sample, for the same passes
├── create_shader_module(label, wgsl)                    rejected → GfxError::Shader
├── create_bind_group_layout(label, entries)             what the shaders read: uniform buffers,
│                                                        each of a declared size, textures and
│                                                        render targets, depth targets, samplers,
│                                                        comparison samplers
├── create_render_pipeline(&descriptor)                  rejected → GfxError::Pipeline (as when
│                                                        its shaders read more than a uniform
│                                                        binding's size); declares
│                                                        its vertex layout (or none: vertices from
│                                                        the vertex index), its fragment stage (or
│                                                        none: depth only), its bind group layouts,
│                                                        depth test and bias, culling, and whether
│                                                        it draws into multisampled targets
├── create_vertex_buffer(label, bytes)
├── create_index_buffer(label, indices)
├── create_uniform_buffer(label, bytes)                  written again with write_uniform_buffer
├── create_texture(label, width, height, format, pixels) an image that shaders sample, its levels
│                                                        of detail computed from its 8-bit pixels
├── create_texture_with_levels(label, width, height, format, levels)
│                                                        the same with the caller's levels, in any
│                                                        format (Rgba16Float: f32_to_f16)
├── create_cube_texture(label, size, format, levels)     six faces per level, read by direction
├── create_sampler(label, &descriptor)                   how they read it
├── create_comparison_sampler(label, compare, filter)    how they compare a depth with a depth
│                                                        target's (shadow lookups, PCF)
├── create_bind_group(label, &layout, entries)           the resources, bound together
├── uniform_offset_alignment()                           the step of a bind group's dynamic
│                                                        offsets into a uniform buffer
├── max_texture_dimension()                              the largest side of a texture or a
│                                                        target (8192 with the default limits)
├── max_buffer_size()                                    the largest buffer (256 MiB with the
│                                                        default limits)
├── create_timer(label, passes)                          a GpuTimer for up to `passes` passes of a
│                                                        submission, when can_time() says the
│                                                        adapter has timestamp queries
├── render(&RenderPassDescriptor { target, clear, depth, timer, resolve }, |pass| ...)
│                                                        one command encoder and render pass; the
│                                                        closure sets the pipeline, the bind groups
│                                                        and the buffers, and draws, into part of
│                                                        the target after set_viewport; a
│                                                        multisampled target is resolved into
│                                                        `resolve`; then submitted, timed by the
│                                                        timer when there is one
├── render_depth(&DepthPassDescriptor { depth, timer }, |pass| ...)
│                                                        a pass without a render target: clears the
│                                                        depth target, draws depth-only pipelines
├── submit(label, |commands| ...)                        several passes (commands.render,
│                                                        commands.render_depth) in one command
│                                                        encoder, then one submission; a timer's
│                                                        passes numbered in recording order
├── read_render_target(&target)                          copy into a buffer, wait for the GPU,
│                                                        map it → Image (8-bit RGBA, top row first)
└── read_float_render_target(&target)                    an Rgba16Float target → FloatImage (f32)
```

A frame of several passes runs them one after another: each submitted by its `render` or
`render_depth`, or several in one submission (`submit`), which costs the CPU less
([ADR-0041](adr/0041-render-graph.md)). A later pass binds the targets an earlier one drew into
(`BindingResource::RenderTarget`, `BindingResource::DepthTarget`). A pass cannot sample what it
draws into: wgpu refuses it and the engine panics
([ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)).
[`crates/slime-gfx/tests/passes.rs`](../crates/slime-gfx/tests/passes.rs) shows each case.

Each GPU object is an engine-owned type holding its wgpu object
([ADR-0017](adr/0017-gpu-resource-types.md); bind groups and depth:
[ADR-0026](adr/0026-resource-binding-and-depth.md); float and sampled targets, depth-only passes:
[ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)). Shader modules and pipelines are
created inside a wgpu error scope, so that a rejection comes back as an error with wgpu's message,
and every other creation inside an out-of-memory scope, so that a buffer, texture or query set that
the memory cannot hold comes back as `GfxError::OutOfMemory` and the caller can react, while memory
that runs out elsewhere loses the device ([ADR-0044](adr/0044-running-out-of-gpu-memory.md)); any
other wgpu error is a bug and panics ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). The test
[`examples/tests/offscreen.rs`](../examples/tests/offscreen.rs) compares the image with a reference
PNG within a tolerance ([ADR-0018](adr/0018-reference-image-tests.md)).

### Rendering a scene

`slime-render` draws meshes with the objects above. The application builds a `Scene` for each
frame, a flat list of instances seen from a camera, and the renderer draws it into any render
target, the window's image or an offscreen one, in passes: the directional light's shadows, in
cascades, the spot lights' shadows, the scene in high dynamic range, the sky behind it, the resolve
of its samples, the bloom's chain, then tone mapping into the target, ordered and recorded by a
render graph
([`crates/slime-render/src/renderer.rs`](../crates/slime-render/src/renderer.rs),
[`crates/slime-render/src/graph.rs`](../crates/slime-render/src/graph.rs), rules:
[ADR-0027](adr/0027-the-render-crate.md), [ADR-0034](adr/0034-hdr-rendering-and-tone-mapping.md),
[ADR-0035](adr/0035-physically-based-materials.md),
[ADR-0037](adr/0037-lights-in-physical-units-and-pre-exposure.md),
[ADR-0038](adr/0038-shadow-mapping.md),
[ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md),
[ADR-0040](adr/0040-bloom.md), [ADR-0041](adr/0041-render-graph.md),
[ADR-0064](adr/0064-cascaded-shadow-maps.md), [ADR-0065](adr/0065-reading-the-cascades.md)):

```text
renderer = Renderer::new(&gpu, format)                 the output's format; four shaders and ten
│                                                      pipelines: two depth-only ones for the
│                                                      shadow passes (the second discards what
│                                                      masked materials discard), three for the
│                                                      scene (depth test, into Rgba16Float),
│                                                      culling back faces, front faces or none,
│                                                      the sky's, the resolve's, the bloom's two
│                                                      (down and up) and the tone-mapping one
│                                                      (each a fullscreen triangle); the BRDF
│                                                      table, computed once per process; a black
│                                                      environment; a texel of depth bound in
│                                                      the place of either shadow atlas until
│                                                      it exists, in the group that reads them,
│                                                      bind group 3
├── set_shaders(&gpu, &ShaderSources)                  the ten pipelines made again from new
│                                                      sources, all or none: on an error, the
│                                                      ones it had stay (ADR-0077)
├── create_environment(&gpu, &Sky)                     the sky drawn on the CPU into a 128-texel
│                                                      cube, prefiltered into four levels on
│                                                      every core (environment.rs), uploaded in
│                                                      Rgba16Float; its sun's disc
├── create_mesh(&gpu, label, &MeshData)                vertex and index buffers, after a check;
│                                                      48-byte vertices with their tangent; the
│                                                      box around the positions
├── create_material(&gpu, label, &MaterialData)        a 48-byte uniform buffer and five texture
│                                                      and sampler pairs (one white pixel for
│                                                      each slot left empty) in a bind group,
│                                                      after a check of its values; a texture in
│                                                      the wrong format for its slot panics;
│                                                      Material::set_values checks new values
│                                                      and writes them into the uniform buffer,
│                                                      for the frames submitted after it
└── render(&gpu, &target, &Scene { camera, lights, environment, exposure, bloom, instances })
    │                                                  → RenderStats, or the error of a creation
    │                                                  (ADR-0044); draws the first 16 lights
    ├── every position moved by −camera.eye, in f64 (ADR-0048): the GPU draws relative to the
    │   camera, so that what is near it keeps its precision however far from the origin
    ├── the four cascades of the first directional light that casts shadows (shadow.rs,
    │   ADR-0050, ADR-0064): what the camera sees up to the light's shadow distance split along
    │   its axis into four slices, each twice as deep as the one before; for each, its tile of the
    │   cascades' atlas, an orthographic projection across the light covering the sphere around
    │   its slice, moved to a texel grid fixed in the world, the instances whose boxes reach
    │   across that square, and along the light the depths their boxes span
    ├── each spot light that casts shadows, in the scene's order: its tile of the atlas, 2048 or
    │   1024 texels wide by how many cast them, its perspective view from the light, from 5 cm
    │   out, its cone and a margin, up to 80° from its axis, and the instances whose boxes reach
    │   into it (shadow.rs, ADR-0058, ADR-0061)
    ├── the buffer of the shadow passes' views ← the cascades' views, then each spot light's,
    │   one slot each at the device's alignment; bound at group 3 of the shadow pipelines
    ├── the frame uniform ← the camera's projection times its rotation, the sky's directions from
    │   clip space (its inverse), the exposure, the number of lights and the shadowed light's index,
    │   the sun's disc (direction, radiance), the lights (64 bytes each, at most 16, a spot
    │   light's with the index of its tile), the spot lights' tiles' views and places in their
    │   atlas, and the cascades' views, places and texel widths; bind group 0, with the
    │   environment, the BRDF table and their sampler, built again when the scene names another
    │   environment (a black one for none)
    ├── the instance buffer ← each instance's relative transform,     bind group 2, one dynamic
    │   normal matrix and handedness (−1 when it mirrors), one slot    offset per instance
    │   each, at the device's alignment; grows when a scene needs it, shrinks back after a peak
    ├── the frame's render graph, built again when the frame's shape changes (the target's size,
    │   either shadow pass or the bloom starting or stopping): the passes below, each declaring the
    │   targets it draws into and samples, ordered by them; the HDR target (Rgba16Float) and the
    │   depth target, of four samples per pixel, the HDR target their samples are resolved into,
    │   and, while the bloom runs, its levels (the output divided by 2^i, rounded to the nearest,
    │   to a side of 8 texels), created at the target's size; the groups that read them, built
    │   from the passes' declarations; a GpuTimer per submission
    ├── the passes, in the graph's order, four to a submission named after the target, each timed
    │   by its submission's GpuTimer when the adapter can:
    │   ├── the shadow pass into the cascades' atlas, created by the first frame that needs it, a
    │   │   depth-only pass skipped when no light or no instance casts shadows: for each cascade,
    │   │   its tile (set_viewport) and its view's slot at group 3; for each instance that casts
    │   │   shadows and whose box reaches across its square, the shadow pipeline its material
    │   │   asks for (masked or not), its bind groups and buffers, draw_indexed
    │   ├── the spot lights' shadow pass into the atlas, created by the first frame that needs it,
    │   │   skipped when no spot light casts shadows: for each such light, its tile
    │   │   (set_viewport) and its view's slot; for each instance that casts shadows and whose box
    │   │   reaches into its cone and range, as above (ADR-0058, ADR-0061)
    │   ├── the scene pass into the multisampled HDR target, cleared to black, the group of both
    │   │   atlases (a texel in the place of each until it exists) bound at group 3, the shadowed
    │   │   light looked up in the finest cascades that hold each fragment (ADR-0065), each spot
    │   │   light that casts shadows in its tile (ADR-0062): for each instance, the pipeline that
    │   │   its material's double-sidedness and its transform's mirroring choose (set when it
    │   │   changes), its material's bind group 1, its slot, its buffers, draw_indexed
    │   ├── the sky pass into the multisampled HDR target, keeping it and the depth target: one
    │   │   triangle at depth 0, at infinity, drawn where the depth test of each sample finds no
    │   │   surface, reads the environment along each pixel's direction, adds the sun's disc, times
    │   │   the exposure
    │   ├── the resolve pass into the HDR target of one sample, which the passes below read: one
    │   │   triangle reads the samples of each pixel (textureLoad) and averages them, each weighed
    │   │   by 1 / (1 + its brightest channel), so that bright edges are smooth too (ADR-0060)
    │   ├── unless the scene's bloom is 0: the bloom's passes (bloom.wgsl): each level filtered
    │   │   down from the one above with Jimenez's 13 taps, then each averaged up with the coarser
    │   │   average through a 3 × 3 tent, every level with the same weight, every read mirrored at
    │   │   the edges; no pass reads what it writes
    │   └── the tone-mapping pass into the target: one fullscreen triangle reads each pixel's texel
    │       (textureLoad), mixes in the bloom's first average by the scene's strength, multiplies
    │       by 64, applies Khronos PBR Neutral, dithers an 8-bit target with a triangular noise
    │       of one step hashed from the pixel's position (ADR-0051), and writes it opaque; an sRGB
    │       target encodes it
    └── RenderStats: the scene pass's draw calls and triangles, the shadow passes' draw calls, the
        CPU time of this call, and the GPU time of each pass, the bloom's summed, a few frames old,
        the latest that came back
```

The dev overlay ([`crates/slime-devui/src/lib.rs`](../crates/slime-devui/src/lib.rs),
[ADR-0031](adr/0031-dev-overlay-with-egui.md)) draws after the tone-mapping pass, in the output,
in a pass and a submission of its own that keep the target: the application hands it every window
event, which it translates into egui's, and in `render` builds its panels with egui and lets
egui's wgpu renderer draw them, through the wgpu objects that the `wgpu` feature of `slime-gfx`
gives.

The shader ([`shader.wgsl`](../crates/slime-render/src/shader.wgsl)) transforms each vertex by its
instance's matrix then the camera's, and its normal by the instance's normal matrix (the cofactor
matrix of its 3 × 3 part, proportional to the inverse transpose, so that normals stay perpendicular
to surfaces scaled unevenly, and finite for a mesh flattened along one axis); each fragment is
shaded with glTF's metallic-roughness model (ADR-0035): the GGX distribution, Smith's visibility and
Schlick's Fresnel under each of the scene's lights (ADR-0037), whose illuminance at the fragment is
a directional light's illuminance, or a point or spot light's intensity over the square of the
distance (as at 1 cm within 1 cm), brought to zero at its range and, for a spot light, times its
cone's attenuation, as `KHR_lights_punctual` recommends; the shadowed light times the share that its
cascades let through, the finest that holds the fragment first, each fading into the next across a
band at its edge (ADR-0064, ADR-0065): in each, the share of nine comparisons over 5 × 5 texels,
weighed as Castaño proposed, that find the fragment lit, moved off its triangle by 4.5 of the
cascade's texels times the sine of the light's incidence (ADR-0050, ADR-0063); each spot light that
casts shadows times the same share in its tile of the atlas, the fragment moved off its triangle by
the sine of its tilt from the light's axis (ADR-0062); the environment's light (ADR-0039), the
diffuse light read from its roughest level along the normal and the specular light from the level of
the roughness along the reflection, weighted by the split sum's Fresnel from the BRDF table with the
light that rough surfaces scatter more than once, times the occlusion; the emission added; the
normal tilted by the normal texture in the frame of the tangent, which follows the model matrix, and
of its bitangent, rebuilt per fragment; back faces of double-sided materials shaded with the normal
reversed; fragments of masked materials below their cutoff discarded; the sum multiplied by the
exposure and divided by 64, so that half precision holds light up to 64 × 65 504 for the bloom
(ADR-0040), and brighter light written as 65 504. The material's textures are read at the texture
coordinates of the vertex. Its sky stages turn each pixel, at infinity, into a direction, read the
environment's first level along it and add the sun's disc, its edge blended over a pixel. The
bloom's shader ([`bloom.wgsl`](../crates/slime-render/src/bloom.wgsl)) filters that light down and
averages it back up into a blurred copy, which the tone-mapping shader
([`tone_mapping.wgsl`](../crates/slime-render/src/tone_mapping.wgsl)) mixes in before it maps the
light to the display's range. A scene that the asset pipeline cooked reaches the renderer through
`Renderer::load_scene_in_background` or `Renderer::load_scene`
([below](#cooking-and-loading-assets)), which create its meshes, materials and textures and flatten
its node hierarchy into instances and lights; mirrored nodes keep their
front faces through the renderer. Conventions: world space is right-handed
with +Y up, distances are in metres and light in physical units (lux, candelas, and nits for the
light of surfaces and the sky), the camera looks down −Z, front faces wind counter-clockwise (a
mirrored instance keeps its front faces), tangents follow glTF, depth is reversed, from 1 at the
near plane to 0 at infinity (ADR-0047), colors are linear light, drawn in high dynamic range,
exposed in the scene pass (ADR-0037), tone mapped (ADR-0034), then encoded by the output's format.

### Rendering in a window

The same objects draw into a window. Each frame of a windowed run, `Frames::run_one` asks the
surface for the window's image (`Surface::acquire`), lets the application draw into its target
(`App::render`, usually with `Gpu::render`), then presents it (`SurfaceFrame::present`). The
surface handles what the graphics API reports about the window, so that the application only sees
whether there is an image to draw:

| Reported | Handled by |
|---|---|
| A new size (from `Event::Resized`) | `Surface::resize`; the surface is configured again before the next image |
| A size above the largest image the device accepts (8192 pixels in a dimension with the default limits) | Limited to it, with a warning; the images are then smaller than the window |
| A size of zero (minimized window) | No image; the frame runs without drawing |
| Outdated configuration | Configured again, then one more try; no image this frame if still outdated |
| Suboptimal image | Drawn and presented, then configured again |
| Image not ready in time (Vulkan), window occluded | No image this frame; on Direct3D 12, wgpu waits up to one second for a late image, then returns it, and the frame is drawn late; on Windows with Vulkan, an image that wgpu handed out but that is still not free after one second loses the device, and the run stops |
| Surface lost, or a configuration the graphics API refuses | Created again from the window; no image this frame. Lost for good (it cannot be created again, or is lost again each time): no image, and the run stops before the next frame, like after a lost device |
| Device lost (see [Losing the GPU](#losing-the-gpu)) | No image; the run stops before the next frame. An image acquired before the loss was found is released instead of presented, so that the surface can be dropped |

How the window shows the images is the *present mode* (`AppConfig::present_mode`). With Mailbox,
the default, presenting never waits, and the frame scheduler alone paces frames, whether the
window is visible or not; windows that do not offer it fall back to FIFO, with which presenting
can block a frame. The measurements behind this choice are in
[ADR-0021](adr/0021-frame-pacing.md).

In GPU capture tools such as RenderDoc, each render pass is named after its target, so the window's
pass of frame 42 appears as `window, frame 42`, and `RenderPass::debug_group` names groups of
commands inside it; the samples' triangle is drawn in a group named `triangle`. On Vulkan, debug
builds pass these names to the driver, and release builds do with `WGPU_DEBUG=1`; Direct3D 12
receives them in every build ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)).

### Cooking and loading assets

The asset pipeline ([ADR-0069](adr/0069-the-asset-pipeline.md),
[ADR-0071](adr/0071-cooked-textures.md)) runs in two programs: the cooker before the game, the
game after it. Nothing passes between them but files.

```text
slime-cook cook <sources> <output>                     the cooker's program, before the game runs
                                                       (watch: cooks again at each change)
└── slime_cook::cook(sources, output)
    ├── scan(sources, GiveIds)                         every source file and its ID, from the
    │                                                  `.meta` beside it, given if it has none
    ├── for each glTF file: GltfFile::open(path, bytes) parsed, validated, its buffers read
    │   ├── output/<key>.import there already?         reused: it lists what the import gave
    │   └── otherwise import(file ID, image IDs)       meshes (MikkTSpace tangents), materials,
    │                                                  scenes, the textures of embedded images;
    │                                                  the image files asked for in their roles
    ├── for each image, in each role asked, or colors:
    │   name = BLAKE3(role, ID, bytes, cooker's version)
    │   ├── output/<name>.ktx2 there already?          reused: nothing is cooked again
    │   └── otherwise image::decode                    PNG or JPEG into 8-bit texels
    │       └── texture::cook_texture(image, role)     every level, averaged for the role, then
    │           └── CookedTexture::to_ktx2             BC7 or BC5 blocks on every thread; written
    │                                                  under a temporary name of the process's,
    │                                                  then renamed
    ├── for each scene file: slime_world::outline      its layout, without its values' types;
    │   │                                              refused, by line, for an asset that no
    │   │                                              source makes, a prefab that is not a
    │   │                                              scene, scene files that place each other
    │   └── slime_serialize::encode                    its binary encoding, written to
    │                                                  output/<name>.scenefile; it needs the
    │                                                  assets of its table
    └── Manifest::to_bytes ──▶ output/manifest         each asset's kind, file, size, source;
                                                       written only if it changed

a game that draws a cooked scene itself, as the `scene` example does (a world loads through an
AssetCache: see "The world", below)
├── slime_assets::CookedAssets::open(output)           reads the manifest
├── renderer.load_scene_in_background(gpu, &assets, id)
│   │                                                  slime-render: a SceneLoad, at once, for a
│   │                                                  scene, the file's ID for a glTF file's
│   │                                                  default one (load_scene: then waits)
│   └── on the load's own thread
│       ├── assets.scene(id)                           its nodes, and the meshes they place
│       ├── a task per mesh, on the task pool          assets.mesh(id), uploaded through a clone
│       │                                              of the Gpu
│       ├── assets.material(id), as meshes name them   each read once; then a task per texture:
│       │                                              assets.texture(id), uploaded in the GPU
│       │                                              format of its encoding, through a buffer
│       │                                              of its own and a copy (ADR-0074)
│       └── once every task has reported              materials and samplers created, nodes
│                                                      placed: the LoadedScene, its instances and
│                                                      its lights in world space
├── load.take(), at each frame                         None while it loads, then the whole scene
└── with --watch, once the scene is shown, at each frame:
    ManifestWatch::poll                                a new manifest, read again now and then:
    └── renderer.reload_scene_in_background(..., &loaded)
                                                       as above, but each mesh and texture of
                                                       the scene loaded before whose cooked file
                                                       the manifest still names is kept; the
                                                       game swaps the scenes when it is loaded

(with --watch, at each frame, from the start)
ShaderWatch::poll                                      the renderer's shader files changed:
└── renderer.set_shaders(&gpu, &sources)               its ten pipelines made again, or none
                                                       (ADR-0077); after a lost device, the new
                                                       renderer is given the files again
```

A load runs on threads of the renderer's own (ADR-0073): the thread of the load reads the scene
and its materials and puts the loaded scene together; the tasks, which read and upload one mesh or
one texture each, run on a task pool that every load of the process shares. A game that hot-reloads
(ADR-0076) watches its manifest, since the cooker writes it last, and only when what it lists
changed.

### The world

A game keeps its entities, their components and their hierarchy in a `slime_world::World`
([ADR-0079](adr/0079-the-engines-own-world.md)): components stored by type, queries that read or
change them ([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)), and, for each entity with a
`Transform`, where it is in the world ([ADR-0083](adr/0083-transforms-and-the-hierarchy.md)):

```text
world.spawn_scene(key, &resolved_scene)             a root entity, then an entity per record of
│                                                    the scene, child of its parent's, with its
│                                                    components; the persistent identity key/path
│                                                    (Scenes and prefabs, below)
├── changing a Transform                            get_mut, query_mut with &mut Transform, insert,
│                                                    remove, set_parent: the entity's subtree marked
│                                                    stale
├── world.global_transform(entity)                  where it is in the world: a lookup if current,
│                                                    else composed from the nearest current ancestor
└── world.update_transforms()                       each stale subtree computed once, from its top
```

`slime-app` owns the game's world, in its `Context`, and calls it around each fixed step: what a
step moves keeps its transform from before the step, and is drawn between the two
([ADR-0084](adr/0084-what-the-world-draws.md)). The world holds no GPU object: its components name
meshes by asset ID, and the GPU objects live in a `slime_render::AssetCache`, which loads them by
itself. `slime-world-render` builds each frame from both, and neither `slime-world` nor
`slime-render` knows the other:

```text
frame
├── world.begin_fixed_step()                        slime-app, before each fixed_update: the
│                                                    transforms that change keep where they were
├── app.fixed_update                                moves entities
├── world.end_fixed_step()                          slime-app: the transforms computed again;
│                                                    what was teleported forgets where it was
├── app.update                                      what it changes is drawn at once
├── world.update_transforms()                       slime-app: render reads lookups
└── app.render
    └── FrameBuilder::build(world, cache, renderer,  slime-world-render
        │                   gpu, fixed_step_alpha)
        ├── cache.poll()                             the batches that finished loading, each whole
        ├── each entity with Meshes and a Transform, where the world draws it at the fraction of
        │   a step (world.interpolated_transform), not hidden by itself or an ancestor: an
        │   instance of each mesh that the cache holds
        ├── cache.request(missing)                   one batch, loaded in the background on the
        │                                            renderer's threads (ADR-0073)
        ├── each entity with a Light: the renderer's light at its origin, along its −Z axis, the
        │   sun first, so that it casts the shadows
        ├── the sky's environment, made again when the game's sky or the sun changed: the first
        │   directional light that shows in the sky
        └── Frame { instances, lights, environment, loading, errors } → frame.scene(camera)
                                                     → renderer.render
```

After a lost device, the game clears the cache and drops the builder's environment; what the world
names loads again by itself. With `--watch`, the game gives new cooked data to
`AssetCache::reload`, which keeps what did not change and takes the rest at once.

### Scenes and prefabs

A scene is spawned under a key as a level, or placed in another scene as a *prefab*: a *placement*,
with *overrides* of its own ([ADR-0089](adr/0089-scenes-and-prefabs.md)). A game reads its scenes
through a `slime_world::SceneLibrary`, by asset ID: `SceneLibrary::of` reads glTF scenes and scene
files from cooked data, each when it is first resolved, and a game or a test can also give it scenes
one by one ([ADR-0095](adr/0095-cooked-scene-files.md)). The library resolves a scene once, with the
world's registry, into a `ResolvedScene` that the world spawns as often as it needs
([ADR-0091](adr/0091-scene-documents-and-placements.md)), and writes back
([ADR-0092](adr/0092-a-world-written-back-as-a-scene.md)):

```text
library.resolve(world.registry(), id)               slime-world
├── a glTF scene: ResolvedScene::from_cooked        a record per node: Transform (its matrix split
│                                                    into a translation, a rotation, a uniform
│                                                    scale and a stretch), Name, Meshes, Light,
│                                                    Extras, and its typed properties, read
│                                                    strictly through the registry; its ID the
│                                                    node's name
└── a scene file, read strictly                     slime-serialize's Document, from its text or
    │                                               its binary encoding, as cooked
    ├── layout                                      [assets], [root], [entities]: IDs, parents,
    │                                               prefabs
    ├── each prefab resolved first, recursively     a cycle refused; nesting and the entities
    │                                               made limited
    ├── records                                     the file's entities, and each placement's
    │                                               prefab's records under its ID
    ├── components                                  read whole, references as paths from the
    │                                               file's root, assets by the table's names
    └── overrides                                   [removed], [added], [changed], applied to the
                                                    values in a save's order; what the prefab no
                                                    longer has set aside, and kept, as are
                                                    references and parents into it

world.spawn_scene(key, &resolved)                   all or nothing: identities key/path, such as
                                                    zone/lamp 3/light; each component made from
                                                    its value; each placement's prefab and
                                                    overrides kept (World::placement); the
                                                    scene's fingerprint for saves

world.spawn_placement(&prefab, parent)              a placement made while the world runs:
                                                    created/N, its entities created/N/ID

world.write_scene(root)                             the subtree back as a scene file (ADR-0092)
├── records                                         the root's components, then the entities of
│                                                   its scene and those created since, which get
│                                                   IDs that they keep, in the order of a walk
├── placements                                      each against its prefab's records, by
│                                                   identity: what differs, and the overrides
│                                                   that the scene wrote, set aside or not, kept
│                                                   while the world holds their target
└── Composer, without transient fields or defaults  the same world, the same bytes; what the
                                                    file cannot hold left out, and reported
```

With new cooked data, `library.reload(assets)` gives the IDs of the scenes whose cooked files
changed, forgets what it read of them, and resolves every scene again when next asked, so that a
scene that places a changed prefab follows it; a game then carries its run into the world spawned
anew (`World::carry`, [ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md)).

### Reflection

Every component is reflected ([ADR-0085](adr/0085-reflection.md)): its type describes itself to a
`slime_reflect::Registry`, which each world keeps, and turns into a `slime_reflect::Value`, plain
data, and back. A world reaches any component by its type's name and a property path, without its
Rust type:

```text
World::new()                                        registers the engine's components and the
│                                                    types their values hold
├── world.insert(entity, component)                 the first of its type registers the type
├── world.register::<T>()                           a type found by name before any entity holds one
└── world.set_property(entity, "Light", path, value)
    ├── storage.value(slot)                          the component as a Value (Reflect::to_value)
    ├── registry.set(type, &mut value, path, new)    the path followed through the value and the
    │                                                type's description; the new part's kind,
    │                                                fields, widths and lengths checked, and its
    │                                                numbers against their fields' ranges
    ├── storage.set_value(slot, &value)              the component read back from the value
    │                                                (Reflect::from_value) in place of the old one
    └── a Transform set this way marks its subtree stale
```

Errors name the component and the path of the part at fault; the component stays as it was.
Serialization writes and reads the same values (below).

The dev overlay's world panel ([`slime_devui::WorldPanel`](../crates/slime-devui/README.md),
[ADR-0081](adr/0081-stage-5a-the-world.md)) is the first editor built on it. It reads a world and
changes nothing: the application applies what it asks for, once its frame is drawn.

```text
render
├── overlay.render(gpu, target, |egui| panel.show(egui, world))
│   ├── world.entities(), their Name                 the list, filtered by name
│   └── world.component_names(selected)
│       └── world.component_value(selected, name)    each component as a Value, shown through
│                                                    its type's description in world.registry();
│                                                    a widget changed gives an Edit: the entity,
│                                                    the component's name, a path and a value
└── for each Edit: world.set_property(...)           a refusal handed back to the panel
                                                     (WorldPanel::refused), which shows it
```

### Serialization

`slime-serialize` writes the values of reflected types as TOML documents and reads them back
([ADR-0086](adr/0086-serialization.md)); its module `format` says how a document holds each kind
of value. The type's description in a registry leads both ways, through the values as a document
holds them: tables, arrays, text, numbers and booleans. The same document is written and read in
the engine's binary encoding too, without its types, through the same reader
([ADR-0094](adr/0094-the-binary-encoding-of-documents.md)).

```text
write(registry, ty, value)                a document of one value, through a Composer
├── Composer::put(place, ty, value)
│   └── Writer::value(ty, value)          the value as a document holds it, by its type's
│                                         description; the types past version 1 noted
└── Composer::finish()                    refused if larger or deeper than a reader reads
    └── text::print(root)                 "$versions" first, then each table's values on their
                                          keys' lines, then its larger tables under headers:
                                          the engine's own writer, LF endings

encode(text)                              the document that a text holds, a composer's too, in
                                          the binary encoding: its strings once, then its
                                          values, each a tag and what it holds; without its
                                          types, which a program that knows none, such as a
                                          cooker, can encode

read(registry, ty, text, mode)            through a Document
├── Document::parse(text, mode)
│   ├── text::parse(text)                 toml_edit's parser: the document's values, and where
│   │                                     each value and key lies; size and depth limited
│   │   or, for Document::parse_binary,
│   ├── binary::decode(bytes)             the same values from the bytes; size, depth, counts
│   │                                     and strings checked, never a panic
│   └── "$format", "$versions"            a later version refused
└── Document::root(ty) → Reader::value    at each value of a type of an earlier version, the
    │                                     type's upgrades first (Registry::upgrade), on the
    │                                     document's value; then its description reads it:
    │                                     kinds, fields by name or alias, defaults, ranges
    └── errors placed by line and column  at the key of a field unknown or given twice, at the
                                          value otherwise; by byte in the binary encoding,
                                          found by walking the bytes again
```

`read_as` and `write_as` do the same for a Rust type, which `from_value` then checks. The cooker
reads and writes its metadata files so, as values of `Metadata`.

### Saving and loading a world

A save holds what differs between a world and `start`, the world that the game makes at the start
of a run, from the same scenes and as it prepares them ([ADR-0087](adr/0087-the-worlds-documents.md)).
Entities are matched by their persistent identities: a scene's entity by its key and its path
through the placements, `zone/door`, `zone/lamp 3/light`; one created outside a scene by its number,
`created/3`. A save is one document of several values, written through `slime-serialize`'s
`Composer` and read through its `Document`, references to entities written as persistent
identities. What the world spawned while it ran, a scene under a key or a placement made
while it ran, is saved as its scene, by its asset ID, and what differs from it; loading spawns it
again through a scene library ([ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md)).

```text
world.save(&start, &about)                           slime-world
├── about                                            what the game says of the save, which
│                                                    World::about reads alone
├── layout, next_created, [scenes]                   the save's layout; the world's count of
│                                                    created entities; each scene that start
│                                                    spawned and that no other replaced: its key,
│                                                    name and a fingerprint of what it resolves
│                                                    to, prefabs included
├── [spawned]                                        what the world spawned while it ran, by its
│                                                    root, a key or created/N: its scene's asset
│                                                    ID, name and fingerprint; its entities
│                                                    compared with what the scene spawns
├── [removed], [parents]                             the entities of start, or of what was
│                                                    spawned, gone; the components gone, the
│                                                    parents changed
├── [created."created/N"]                            each entity created since, whole: its parent
│                                                    and components
├── [added], [changed]                               the components added, whole; what changed of
│                                                    the others, as parts (Registry::difference)
└── Composer::finish                                 slime-serialize: the same worlds, the same
                                                     bytes; Unsaved entities and transient fields
                                                     left out

start.load(text, &mut scenes)                        a world the game made as at the start of a run
├── read_layout                                      first reading: all that can refuse the save,
│                                                    before the world changes: its layout, what
│                                                    it spawns and creates, its count, its scenes,
│                                                    what it removes, the parents
├── spawn what ran                                   what the world spawned while it ran, each
│                                                    scene as the library resolves it now, under
│                                                    the same identity, another scene under its
│                                                    key destroyed first; what it cannot give set
│                                                    aside
├── create                                           the created entities spawned under their
│                                                    numbers, so that references can name them
├── read_values                                      second reading: the components' values,
│                                                    references resolved; what cannot be read set
│                                                    aside
└── apply                                            removed, parents, added, changed
                                                     (Registry::overlay): what the world no longer
                                                     has set aside; the scenes changed since named

run.carry(&old_start, start, &mut scenes)            hot reload: the run saved against the start
                                                     world made before the files changed, and
                                                     loaded into the one made from them
```

### Losing the GPU

The driver can lose the device: it crashes or is reset, the GPU is removed, or it runs out of
memory. Memory that runs out in a creation that no caller hears of, made by a library built on
wgpu such as egui's renderer, gives the device up the same way
([ADR-0044](adr/0044-running-out-of-gpu-memory.md)). `slime-gfx` records the loss in wgpu's
device-lost callback, and from then on its error handler logs wgpu's errors instead of panicking,
since every use of the device fails, and creations return `GfxError::DeviceLost`, which says why
(`LossKind`); `Gpu::lose` loses a device on purpose, for tests and tools. Before each frame, the
windowed driver checks `Gpu::check()` with the other reasons to stop: after a loss, the run stops
like after an exit request, and `run` returns `RunError::Gfx` with `GfxError::DeviceLost`
([ADR-0022](adr/0022-losing-the-gpu-device.md)). An application that sets
`AppConfig::restore_lost_device` goes on instead
([ADR-0075](adr/0075-recovering-from-a-lost-gpu-device.md)):

```text
between two frames, the device lost
└── WindowedDriver::restore                          slime-app, before deciding to stop
    ├── App::gpu_lost(ctx)                           the application drops its objects of the
    │                                                lost device, and its clones of the Gpu
    ├── Context::take_gpu()                          the run's last reference but the surface's
    ├── Surface::restore(lost)                       slime-gfx: the lost device and its surface
    │   │                                            released, the window's surface created
    │   │                                            again through the same wgpu instance
    │   └── Gpu::open(instance, surface, lost)       a new device; refused after a loss to memory,
    │                                                or for a CPU adapter after a hardware one
    ├── Context::set_gpu(new, format)
    └── App::gpu_restored(ctx)                       the application creates its objects again
```

A game that draws its world drops its renderer in `gpu_lost`, clears its `AssetCache` and releases
its `FrameBuilder`'s environment; its world, in the `Context`, is untouched, and the cache loads
what it names again from the new device ([ADR-0084](adr/0084-what-the-world-draws.md)).

Nothing may hold the lost device when the new one opens: Direct3D 12 opens no device on an
adapter while a removed one exists in the process. The run does not restore when it is stopping
anyway or its window's surface was lost for good; after three new devices in a row without a frame
shown, or when none opens, it gives up and stops, `run` returning `GfxError::CannotRestore` or the
error that opening a device gave. A program without
`slime-app` replaces its device with `Gpu::restore`, or `Surface::restore` for a window.

The window's surface can be lost for good as well, when it cannot be created again after a loss or
is lost again each time it is: `Surface::check` reports it, and the run stops the same way with
`GfxError::SurfaceLost` ([ADR-0025](adr/0025-giving-up-a-lost-surface.md)).

### Termination signals

`run` installs, once per process, a handler for SIGINT (Ctrl+C), SIGTERM and SIGHUP
([`crates/slime-app/src/signal.rs`](../crates/slime-app/src/signal.rs)). The handler runs on a
thread of its own: it sets a flag and hands its log line to another thread, so that a blocked
standard error cannot keep a second signal from being handled; before each frame, the driver
records the flag in the `Context` as an exit request, so the run stops like any other: the frame
in progress finishes and `app.shutdown` runs. A second signal, half a second or more after the
first, ends the process at once with exit code 130; one that comes sooner (a closing terminal sends
SIGHUP twice) is ignored
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).

### Frame timing

`slime_app::time` has no clock of its own: the driver gives `FrameClock` the duration of each
frame (measured, or simulated in headless runs), gets a `FrameTime` back, and gives its delta to
`FixedStep`, which says how many fixed steps the frame must run. Source:
[`crates/slime-app/src/time.rs`](../crates/slime-app/src/time.rs); rules:
[ADR-0009](adr/0009-frame-timing.md).

### Threads

`run` drives the application from the main thread, and the renderer records and submits its
passes on the thread that calls it. The renderer loads cooked scenes, and the asset cache its
batches of meshes, on threads of its own ([above](#cooking-and-loading-assets), ADR-0073): a
thread per load, and a task pool private to `slime-render`, which lives as long as the process;
they create GPU objects through clones of the `Gpu`. What may cross threads is stated, so that
later stages (a render thread, the job system) can rely on it, and checked when the tests of the
crates that own these types compile (a `threads` module in their `lib.rs`, or in `slime-devui`'s
tests):

- **Shared between threads** (`Send` and `Sync`): the `Gpu`, whose clones share its device, its
  `Surface` and every GPU object it creates (targets, textures, samplers, buffers, bind groups and
  their layouts, shader modules, pipelines), images read back, meshes, materials, environments,
  cameras, lights, scenes and instances, loaded scenes and loads in progress, the asset cache,
  statistics, windows, events, the application's `Context` and `AppConfig`, the dev overlay, worlds
  and entity handles, the frame builder of `slime-world-render` and its frames, reflected values
  and registries, and the error types of those crates. wgpu keeps its error scopes per thread, so
  GPU objects can be created from several threads.
  wgpu fails to configure a window's surface when another thread submits work while it waits for
  the GPU, as the threads of a load do: the `Gpu`'s submissions, from any thread, wait while the
  surface is configured.
- **Moved between threads, used by one at a time** (`Send` only): a `GpuTimer`, which reads its
  times back through a cell, and so the `Renderer` that holds them; `Renderer::render` takes it
  mutably anyway.

## Platforms

The engine targets Linux (`x86_64-unknown-linux-gnu`) and Windows (`x86_64-pc-windows-msvc`)
([ADR-0015](adr/0015-platforms-and-ci.md)). Platform differences are kept in few places:

- winit hides the windowing systems (Wayland and X11 on Linux, Win32 on Windows).
  `slime-platform` has platform-specific code in three places of `event_loop.rs` and one of
  `event.rs`:
  - `backend_name` names the windowing system in the log.
  - On X11, `resumed` gives winit a maximum window size of 65 535 physical pixels, the protocol's
    limit; beyond it, winit panics.
  - On Windows, `exiting` hides the window, which winit destroys only from inside the event loop.
  - On Windows, `Translator::translate` releases both Shift keys on a key-up of either, since
    Windows sends a single key-up for the two.

  Where winit's events differ between platforms, `slime-platform` makes them agree: `Translator` in
  `event.rs` pairs key and mouse button events across focus changes. One difference shapes the main
  loop: on Windows, the system runs its own loop while the user resizes or moves a window, which is
  why frames run on redraw requests.
- `ctrlc` hides signals (Unix) and console events (Windows); `crates/slime-app/src/signal.rs`
  describes how each platform behaves.
- wgpu hides the graphics APIs; `slime-gfx` only chooses which one to ask for on each platform
  (`DEFAULT_BACKENDS` in `crates/slime-gfx/src/gpu.rs`).
- On Windows, `slime-app` asks the console to interpret the escape sequences that color its log,
  through `nu-ansi-term`, and colors it only when the console accepts (`use_colors` in
  `crates/slime-app/src/diagnostics.rs`): Windows Terminal interprets them already, and the older
  console host once asked.
- Tests that cannot run on a platform are compiled out with `#[cfg]`, and their file says why: the
  signal tests run on Unix only, and the two windowed test programs of `slime-app`
  (`windowed_errors.rs` and `windowed_lifecycle.rs`) on Linux only; having no test harness, they
  keep a `main` that does nothing elsewhere.

`scripts/check.sh` compiles the whole workspace for Windows on Linux, and CI runs the tests and
the headless samples on a Windows runner.

## Where to start reading

1. [`README.md`](../README.md) for what the project is and its state.
2. This file for the crate map.
3. [`PLAN.md`](PLAN.md) for the milestone in progress.
4. The `README.md` of the crate you are about to change.
5. [`examples/`](../examples/README.md) for runnable programs that use the engine.

## `unsafe` code

No crate contains `unsafe` code. [ADR-0005](adr/0005-lints-and-unsafe-policy.md) says when it is
allowed and how it must be written.
