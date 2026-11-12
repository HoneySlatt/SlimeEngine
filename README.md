<p align="center">
  <img src="docs/logo/slime.svg" alt="SlimeEngine's logo: a small white winged creature with pink ears and a teal pompom on a stalk" width="192">
</p>

# SlimeEngine

SlimeEngine is a general-purpose, modular game engine written in Rust, its source public. It aims to
be a complete platform (runtime, asset pipeline, native editor and SDK) for linear narrative
games first, then open-world games, then large online worlds.
It is free until a game earns 1 M$, with a royalty above, and made first for a small team helped by AI agents;
[`docs/ROADMAP.md`](docs/ROADMAP.md#1-vision) says whom it is for and when it opens.

## Status

Pre-alpha. The roadmap is cut into *stages*, each ending with a release, and a stage into
*milestones*, each a change that is checked whole.

- Latest release: 0.6, tag `v0.6.0`, which closed Stage 6b, the engine's interface.
- In progress on `main`: Stage 6d, in which uwui, the engine's UI framework, leaves for
  [its own repository](https://github.com/HoneySlatt/uwui). [`docs/PLAN.md`](docs/PLAN.md) says
  where it stands.
- Paused: Stage 6c, the editor, to be released as 0.7 with Stage 6d, which resumes after uwui's
  1.0 ([`docs/stage-6c.md`](docs/stage-6c.md)).
- The finished stages: [`docs/history.md`](docs/history.md). What comes next:
  [`docs/ROADMAP.md`](docs/ROADMAP.md).

What `main` does today, on Linux (Wayland or X11) and Windows, on x86_64; the editor's window and
docking came after 0.6, as [`CHANGELOG.md`](CHANGELOG.md) says:

- **Platform.** Programs run headless or in a window, with keyboard and mouse input, a fixed-step
  loop, frames on demand for a program that asks for it, logging, panic reporting and a clean shutdown
  (`slime-app`, `slime-platform`).
- **GPU.** Vulkan on Linux, Direct3D 12 on Windows, frames paced by the engine, GPU memory running
  out reported as an error (`slime-gfx`).
- **Renderer.** glTF's metallic-roughness materials and unlit ones, lights in physical units, the
  shadows of a sun in cascades and of spot lights, a procedural sky, high dynamic range with bloom
  and tone mapping, four samples per pixel; what the camera does not see is left out, and instances
  are drawn together; several views, picking and drawings for the editor (`slime-render`).
- **Assets.** The cooker, `slime-cook`, turns glTF scenes and PNG and JPEG images into meshes,
  materials, scenes and compressed textures, read by an ID that survives a move. A game asks for
  them by requests that carry a priority, loaded in the background within budgets of memory, and
  loaded again when a source or a shader changes (`slime-assets`).
- **World.** Entities, components and their hierarchy (`slime-world`), reflected and reached by name
  (`slime-reflect`); scene files that place prefabs with overrides, written as text or in a binary
  encoding (`slime-serialize`); saves that cost what the run changed.
- **Interface.** `uwui`: text in every writing system, a tree of elements laid out by flexbox and
  grid, themes read from files, widgets, menus, docking, input methods and screen readers, drawn by
  the renderer (`uwui-render`). The *dev overlay*, panels over a running game that show and edit
  its world, is built on it (`slime-devui`).
- **Editor.** `slime-editor` opens a level and changes it through commands, undone and redone: a
  hierarchy, viewports with picking and handles, prefabs placed, a history, Play inside the editor
  while the level is edited, panels in regions of tabs. Its README:
  [`crates/slime-editor`](crates/slime-editor/README.md).
- **Agents.** An AI agent drives a running game or the editor through the engine's MCP (Model
  Context Protocol) server, `slime-mcp`: it starts, pauses and steps a game, reads and changes its
  world, captures its image and plays it, using an interface through its elements rather than a
  mouse.
- **The sample game.** *Hunna Wood* (`games/hunnawood`), a role-playing game seen in the third
  person and made with the public API only: its first zone, a forest path to a ruined chapel, by day
  and by night ([`games/hunnawood`](games/hunnawood/README.md)).

[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) explains how these parts fit together.

## Getting started

On Linux, with [Nix](https://nixos.org) and flakes enabled,
`nix develop` at the repository root opens a shell with the pinned Rust toolchain and the project's
tools. [`CONTRIBUTING.md`](CONTRIBUTING.md#development-environment) explains this, the setup without
Nix, and the setup on Windows. Then:

```sh
cargo build --workspace
cargo test --workspace
cargo run --example window   # the examples are listed in examples/README.md
cargo run -p hunnawood       # the game, once its zone is cooked: games/hunnawood/README.md
cargo run --example editor   # the editor; on a copy of its level: examples/README.md
scripts/check.sh             # every check, exactly as CI runs them (Linux only)
cargo run --quiet -p slime-mcp  # the MCP server through which an agent drives a game: crates/slime-mcp
```

Each CI run on GitHub also publishes the `hello`, `headless`, `window`, `offscreen`, `interface`,
`widgets`, `scene` and `world` examples and the `hunnawood` game built for Windows, to try without a
toolchain; the GitHub release of each version since 0.1 carries what that version had.

The tests run on your own GPU; CI runs them on drivers that render on the CPU, lavapipe on Linux and
WARP on Windows, and [`CONTRIBUTING.md`](CONTRIBUTING.md#gpu) explains how to run them the same way.

## Using SlimeEngine in a game

The engine is not published on crates.io: a game depends on its crates through git, pinned to a
release tag. Between releases, `main` moves ahead of the latest tag, and the crates' READMEs and the
examples describe `main`: to follow them, use `branch = "main"`, or a `rev` for a build that does
not move.

```toml
[dependencies]
slime-app = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
slime-assets = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
slime-color = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
slime-gfx = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
slime-platform = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
slime-reflect = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
slime-render = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
slime-world = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
slime-world-render = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0" }
# A game's interface (`Presentation::ui`), drawn through the renderer's canvas in 0.6, through
# `slime-app`'s `InterfaceRenderer` on `main`. uwui's crates bear
# their names from 0.7 on, and on `main`, where `package` goes; 0.6 names them `slime-ui` and
# `slime-ui-render`.
uwui = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0", package = "slime-ui" }
uwui-render = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0", package = "slime-ui-render" }
# The dev overlay, in a development build alone: `dep:slime-devui` in the game's feature.
slime-devui = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.6.0", optional = true }

# The renderer computes a sky's lighting on the CPU, several times slower without optimizations.
[profile.dev.package.slime-render]
opt-level = 3

# The other dependencies optimised in debug builds, the engine's crates among them, outside its
# workspace; and a profile for shipping, optimised across crates, as the engine's workspace has them
# (docs/adr/0221-compiling-within-its-budgets.md).
[profile.dev.package."*"]
opt-level = 1

[profile.shipping]
inherits = "release"
lto = "fat"
codegen-units = 1
```

- `slime-app` runs the game: its rules as a `Simulation`, what the player sees as a `Presentation`.
  [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md#crates) gives each crate's role, and each crate's
  README its entry points. The cooker cooks a game's source assets before it runs,
  as [`games/hunnawood`](games/hunnawood/README.md) shows. [`examples/`](examples/README.md) holds
  programs to start from: `world` draws a world, `scene` drives the renderer by hand.
- A game that tools and agents drive declares a `development` feature, on by default, which turns
  on `slime-app/development`, the *development channel* through which tools drive the game, and the
  dev overlay, an optional dependency of the game. It is built for shipping with
  `cargo build --profile shipping --no-default-features`, which leaves both out, as [`games/hunnawood`](games/hunnawood/Cargo.toml) does
  ([`crates/slime-app/README.md`](crates/slime-app/README.md),
  [`crates/slime-devui/README.md`](crates/slime-devui/README.md)).
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

The main gaps. [`docs/ROADMAP.md`](docs/ROADMAP.md) says which stage brings what; the smaller gaps
are in [`docs/debt.md`](docs/debt.md).

- No audio, physics or animation.
- Screen readers read the examples' interfaces; games leave them out until Stage 7, runtime systems.
- The editor is not finished: [`docs/stage-6c.md`](docs/stage-6c.md) lists what Stage 6c still
  brings, the inspector among it.
- The cooker imports glTF scenes, scene files and PNG and JPEG images only, and a scene is loaded
  whole.
- *Hunna Wood* keeps one run, in one file, and refuses the saves of the versions before 0.5.
- Blended transparency for unlit materials only, unsorted; a physically based material refuses it
  until Stage 9a, the production renderer.
- A capped number of lights, and no shadows from point lights.
- One window per process, which cannot go fullscreen; a system dialog to open a file, none to save
  one; no gamepads or touch input.
- A lost GPU device stops the run, unless the game asks to go on with a new one and makes its GPU
  objects again itself.

## Where to read next

| Document | What it tells you |
|---|---|
| [`AGENTS.md`](AGENTS.md) | The rules every contributor follows: scope, architecture, code, documentation, commits. Written for AI agents, binding for everyone. |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to set up the environment, check a change and submit it. |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Direction, order of the stages and their exit criteria. An overview, not a commitment. |
| [`docs/PLAN.md`](docs/PLAN.md) | The current stage's status, the milestone in progress and the checks by hand. |
| [`docs/stage-6d.md`](docs/stage-6d.md) | Stage 6d's whole plan while it is in progress: its rules and each milestone in full. |
| [`docs/debt.md`](docs/debt.md) | The known debt still open: what is missing or wrong, why it stayed, and where it is followed. |
| [`docs/history.md`](docs/history.md) | The finished stages, a few lines each, with their versions and ADRs. |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | The crates, their roles, how they depend on each other, and where to start reading the code. |
| [`docs/adr/`](docs/adr/README.md) | Every architecture decision, with its context and the alternatives rejected. |
| [`docs/glossary.md`](docs/glossary.md) | Terms the project defines. |
| [`docs/dependencies.md`](docs/dependencies.md) | Every third-party dependency and tool, and why it is used. |
| [`docs/measurements.md`](docs/measurements.md) | What the build and the benchmarks measured, milestone after milestone. |
| [`CHANGELOG.md`](CHANGELOG.md) | Notable changes, per release. |

## License

SlimeEngine is to be licensed under a licence of its own: free until a game has earned 1 M$ of
gross revenue, 3 % of the revenue above it, 2 % for a game that uses the engine's online services
([ADR-0214](docs/adr/0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)). Until its text
is published, [`LICENSE.md`](LICENSE.md) is an interim notice: all rights reserved, the source
visible. uwui, `crates/uwui`, `crates/uwui-render`, `crates/uwui-kdl` and `crates/uwui-macros`,
stays under MIT OR Apache-2.0; the examples, in `examples/`, are under MIT-0, to be copied into games. What was
published before keeps its licence: MIT OR Apache-2.0 from 2026-11-05 to the notice, the Mozilla
Public License 2.0 before, releases up to 0.6 included. A game built with the engine can use any
license. The two logos are not under these licenses: their terms are in [`docs/logo/README.md`](docs/logo/README.md), the engine's, and
[`crates/uwui/logo/README.md`](crates/uwui/logo/README.md), uwui's.
