# SlimeEngine

SlimeEngine is a general-purpose, modular, open-source game engine written in Rust. It aims to
be a complete platform (runtime, asset pipeline, native editor and SDK) for linear narrative
games first, then open-world games, then large online worlds.

## Status

Pre-alpha. Stages 0 (*Foundations*), 1 (*Core and platform*), 2 (*GPU layer*), 3a (*First
renderer*), 3b (*Physically based renderer*), 3c (*A first game*), 3d (*Anti-aliasing and the
shadows of spot lights*), 4a (*Cooked assets*), 4b (*Live assets*), 5a (*The world*) and 5b (*Scenes
and prefabs*) are done; the latest release is 0.4 (git tag `v0.4.0`), which released Stage 5b.
Applications run headless or with a window (Wayland or X11 on Linux, Win32 on Windows) that
receives keyboard and mouse input, with a fixed-step main loop, logging, panic reporting and a clean shutdown on Ctrl+C. The
engine opens the GPU (Vulkan on Linux, Direct3D 12 on Windows), paces frames with its own schedule
and the Mailbox present mode by default, names its GPU work for capture tools, reports a GPU whose
memory runs out as an error a game can handle, and, when the GPU device is lost, stops cleanly or,
for a game that asks, goes on with a new one. Its renderer, `slime-render`, draws instances of
meshes with glTF's metallic-roughness materials, lit by directional, point and spot
lights in physical units (lux and candelas), with the shadows of a sun, in cascades around the
camera, and of spot lights, and the light of a procedural sky that surfaces reflect and that
shows behind them; it draws in high dynamic range, at four samples per pixel so that edges are
smooth, relative to the camera and with reversed depth so that large scenes keep their precision,
then exposes the light, spreads a bloom around it, tone maps it and dithers it, into a window
or into offscreen images that image tests compare with references. The `scene` example draws a
scene cooked from a glTF file, and its lights, under a camera driven by the mouse, with the CPU
time and the GPU time of each pass in a dev overlay. The asset pipeline gives every source file an
ID that survives a move, and its cooker, `slime-cook`, turns glTF scenes and PNG and JPEG images
into cooked data, meshes, materials, scenes and textures compressed in BC7 and BC5, which a game
reads by ID ([ADR-0069](docs/adr/0069-the-asset-pipeline.md)). The
sample game, *Hunna Wood* (`games/hunnawood`), a role-playing game seen in the
third person and made with the public API only, loads its first zone from cooked assets
([ADR-0067](docs/adr/0067-hunna-wood-replaces-nightwatch.md)): a forest path to a ruined chapel,
where the hero finds a key that opens a door, by day and by night. Stage 4b made loaded
assets live: scenes load on threads of the renderer's own while the game draws, a game can go on
after a lost GPU device, a material's values change while it runs, and a texture or a shader
changed on disk changes in the running game. Stage 5a gave the engine a world, reflection and
saves: the world keeps a game's entities, their components and their hierarchy, places them,
spawns cooked scenes in it (`slime-world`), is drawn from an asset cache that loads what it names
(`slime-world-render`, the `world` example), and *Hunna Wood* runs on it; every component is reflected, and reached by name (`slime-reflect`); a panel of the dev
overlay lists the world's entities and edits their components, in the game and the `world` example,
on F2; values are written to TOML documents and read back, from earlier versions of their types
too (`slime-serialize`), as the cooker's metadata files are; a world saves what changed since the
start of a run, and loads it back, which the `world` example checks when it quits, and *Hunna Wood*
saves its run on F5 and when it quits, loads it on F9, and resumes it when it starts. Stage 5b
wrote worlds in files: a scene file places other scenes as prefabs, each placement with
overrides of its own, which a world spawns and writes back (`slime-world`), and what a game spawns
while it runs is saved as its scenes; `slime-serialize` encodes documents in a binary encoding of
its own too, in which the cooker cooks scene files, and Blender's nodes carry typed components. The
`world` example spawns a yard of prefabs from a scene file, and follows a prefab changed while it
runs; *Hunna Wood*'s zone is a scene file that places its land and six lanterns of one prefab.
Since 0.4, an audit pass fixed defects that an audit of 0.4 found in saves and identities, in the
readers of files and in the recovery from a lost GPU device, and made the quick wins that it
measured; a foundations pass decided as ADRs what the editor will freeze. Both ship with 0.5.
[`docs/PLAN.md`](docs/PLAN.md) says what comes next, Stage 6a, what the editor stands on, then
Stage 6b, the editor, and lists the known debt.

## Getting started

Linux and Windows, on x86_64. On Linux, with [Nix](https://nixos.org) and flakes enabled,
`nix develop` at the repository root opens a shell with the pinned Rust toolchain and the project's
tools. [`CONTRIBUTING.md`](CONTRIBUTING.md#development-environment) explains this, the setup without
Nix, and the setup on Windows. Then:

```sh
cargo build --workspace
cargo test --workspace
cargo run --example window   # the examples are listed in examples/README.md
cargo run -p hunnawood       # the game, once its zone is cooked: games/hunnawood/README.md
scripts/check.sh             # every check, exactly as CI runs them (Linux only)
```

Each CI run on GitHub also publishes the `hello`, `headless`, `window`, `offscreen`, `scene` and
`world` examples and the `hunnawood` game built for Windows, to try without a toolchain;
the GitHub release of each version since 0.1 carries what that version had.

The tests run on your own GPU; CI runs them on drivers that render on the CPU, lavapipe on Linux and
WARP on Windows, and [`CONTRIBUTING.md`](CONTRIBUTING.md#gpu) explains how to run them the same way.

## Using SlimeEngine in a game

The engine is not published on crates.io: a game depends on its crates through git, pinned to a
release tag. Between releases, `main` moves ahead of the latest tag, and the crates' READMEs and the
examples describe `main`: to follow them, use `branch = "main"`, or a `rev` for a build that does
not move.

```toml
[dependencies]
slime-app = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }
slime-assets = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }
slime-color = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }
slime-gfx = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }
slime-platform = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }
slime-reflect = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }
slime-render = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }
slime-world = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }
slime-world-render = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.4.0" }

# The renderer computes a sky's lighting on the CPU, several times slower without optimizations.
[profile.dev.package.slime-render]
opt-level = 3
```

- `slime-app` runs the game (the `App` trait and `run`) and owns its world, `slime-platform` holds
  the window's settings and the input events, `slime-gfx` the GPU's objects, `slime-color` the
  colors, `slime-render` the renderer, and `slime-assets` the cooked data that the renderer loads.
  `slime-world` keeps the game's entities and components and saves them, `slime-world-render` draws
  them, and `slime-reflect` describes the components, which derive `Reflect`. The cooker,
  `slime-cook`, is a program of this repository that cooks a game's source assets before it runs,
  as [`games/hunnawood`](games/hunnawood/README.md) shows. Each crate's README shows its entry
  points, and [`examples/`](examples/README.md) holds programs to start from: `world` draws a world,
  `scene` drives the renderer by hand.
- Building needs Rust 1.99 or later and, on Linux, the libraries that
  [`CONTRIBUTING.md`](CONTRIBUTING.md#on-linux-without-nix) lists and a [Vulkan
  driver](CONTRIBUTING.md#gpu).
- On Windows, put DXC's `dxcompiler.dll` beside the game's program, as `scripts/fetch-dxc.sh` does:
  without it, Direct3D 12 compiles the shaders with FXC, whose code draws about half as fast
  ([ADR-0066](docs/adr/0066-shipping-dxc-beside-the-programs.md)).
- A game kept inside a clone of this repository needs an empty `[workspace]` table in its
  `Cargo.toml`, or cargo takes it for a member of the engine's workspace and refuses to build it.
- Nothing is stable before 1.0: a release can break the API, and [`CHANGELOG.md`](CHANGELOG.md)
  says what changed.

## What it does not do yet

The engine is young; [`docs/ROADMAP.md`](docs/ROADMAP.md) says which stage brings what, and the
known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt) lists the smaller gaps. A game made with it
today has:

- No text, game UI, audio, physics, animation or editor.
- An asset pipeline that imports glTF scenes, scene files and PNG and JPEG images only: the cooker,
  `slime-cook`, makes them cooked data, which the renderer loads whole, on threads of its own while
  the game goes on (`Renderer::load_scene_in_background`), and again when it changes
  (`slime-cook watch`), as the renderer's shaders are.
- A world (`slime-world`) of entities, components and their hierarchy, drawn through
  `slime-world-render`, whose components are reflected and edited by name (`slime-reflect`), and
  saved as what changed since the start of a run; *Hunna Wood* saves one run, in one file. Scene
  files are cooked, and written by hand; a world is written back as a scene file through the API,
  and no editor writes them yet.
- A renderer without blended transparency or culling of what the camera does not see; with a
  capped number of lights, and shadows from one directional light and from spot lights, none from
  point lights.
- One window, which cannot go fullscreen, grab or hide the cursor, or report raw mouse motion; no
  gamepads, touch input or input methods.
- A run that stops when the GPU device is lost, unless the game asks to go on with a new one and
  makes its GPU objects again itself.
- Linux and Windows only, on x86_64.

## Where to read next

| Document | What it tells you |
|---|---|
| [`AGENTS.md`](AGENTS.md) | The rules every contributor follows: scope, architecture, code, documentation, commits. Written for AI agents, binding for everyone. |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to set up the environment, check a change and submit it. |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Direction, order of the stages and their exit criteria. An overview, not a commitment. |
| [`docs/PLAN.md`](docs/PLAN.md) | The current stage, its milestones, their status and the known debt. |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | The crates, their roles, how they depend on each other, and where to start reading the code. |
| [`docs/adr/`](docs/adr/README.md) | Every architecture decision, with its context and the alternatives rejected. |
| [`docs/glossary.md`](docs/glossary.md) | Terms the project defines. |
| [`docs/dependencies.md`](docs/dependencies.md) | Every third-party dependency and tool, and why it is used. |
| [`docs/measurements.md`](docs/measurements.md) | What the build and the benchmarks measured, milestone after milestone. |
| [`CHANGELOG.md`](CHANGELOG.md) | Notable changes, per release. |

## License

SlimeEngine is licensed under the [Mozilla Public License 2.0](LICENSE). MPL-2.0 is a file-level
copyleft license: if you distribute a modified version of an engine file, the source of that
file must remain available under MPL-2.0. Files you write yourself, such as a game's code, can
use any license.
