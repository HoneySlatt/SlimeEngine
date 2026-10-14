# SlimeEngine

SlimeEngine is a general-purpose, modular, open-source game engine written in Rust. It aims to
be a complete platform (runtime, asset pipeline, native editor and SDK) for linear narrative
games first, then open-world games, then large online worlds.

## Status

Pre-alpha. Stages 0 (*Foundations*), 1 (*Core and platform*), 2 (*GPU layer*), 3a (*First
renderer*), 3b (*Physically based renderer*) and 3c (*A first game*) are done; the latest release is
0.1 (git tag `v0.1.0`). Applications run headless or with a window (Wayland or X11 on Linux, Win32
on Windows) that receives keyboard and mouse input, with a fixed-step main loop, logging, panic
reporting and a clean shutdown on Ctrl+C. The engine opens the GPU (Vulkan on Linux, Direct3D 12 on
Windows), paces frames with its own schedule and the Mailbox present mode by default, names its GPU
work for capture tools, reports a GPU whose memory runs out as an error a game can handle, and stops
cleanly when the GPU device is lost. Its renderer, `slime-render`, draws instances of meshes with
glTF's metallic-roughness materials, lit by directional, point and spot lights in physical units
(lux and candelas), with the shadows of a sun around the camera and the light of a procedural sky
that surfaces reflect and that shows behind them; it draws in high dynamic range, relative to the
camera and with reversed depth so that large scenes keep their precision, then exposes the light,
spreads a bloom around it, tone maps it and dithers it, into a window or into offscreen images that
image tests compare with references. The `scene` example draws a glTF scene and its lights under a
camera driven by the mouse, with the CPU time and the GPU time of each pass in a dev overlay.
*Nightwatch* (`games/nightwatch`), a small stealth game made with the public API only, is the sample
that every later stage grows. Next comes Stage 3d, anti-aliasing and the shadows of spot lights;
[`docs/PLAN.md`](docs/PLAN.md) lists the known debt.

## Getting started

Linux and Windows, on x86_64. On Linux, with [Nix](https://nixos.org) and flakes enabled,
`nix develop` at the repository root opens a shell with the pinned Rust toolchain and the project's
tools. [`CONTRIBUTING.md`](CONTRIBUTING.md#development-environment) explains this, the setup without
Nix, and the setup on Windows. Then:

```sh
cargo build --workspace
cargo test --workspace
cargo run --example window   # the examples are listed in examples/README.md
cargo run -p nightwatch      # the first game, games/nightwatch/README.md
scripts/check.sh             # every check, exactly as CI runs them (Linux only)
```

Each CI run on GitHub also publishes the `hello`, `headless`, `window`, `offscreen` and `scene`
examples and the `nightwatch` game built for Windows, to try without a toolchain; the GitHub
release of each version since 0.1 carries what that version had.

The tests run on your own GPU; CI runs them on drivers that render on the CPU, lavapipe on Linux and
WARP on Windows, and [`CONTRIBUTING.md`](CONTRIBUTING.md#gpu) explains how to run them the same way.

## Using SlimeEngine in a game

The engine is not published on crates.io: a game depends on its crates through git, pinned to a
release tag. Between releases, `main` moves ahead of the latest tag, and the crates' READMEs and the
examples describe `main`: to follow them, use `branch = "main"`, or a `rev` for a build that does
not move.

```toml
[dependencies]
slime-app = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.1.0" }
slime-gfx = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.1.0" }
slime-platform = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.1.0" }
slime-render = { git = "https://github.com/HoneySlatt/SlimeEngine", tag = "v0.1.0" }

# The renderer computes a sky's lighting on the CPU, several times slower without optimizations.
[profile.dev.package.slime-render]
opt-level = 3
```

- `slime-app` runs the game (the `App` trait and `run`), `slime-platform` holds the window's
  settings and the input events, `slime-gfx` the GPU's objects and colors, and `slime-render` the
  renderer. Each crate's README shows its entry points, and [`examples/`](examples/README.md) holds
  programs to start from; `scene` is the most complete.
- Building needs Rust 1.99 or later and, on Linux, the libraries that
  [`CONTRIBUTING.md`](CONTRIBUTING.md#on-linux-without-nix) lists and a [Vulkan
  driver](CONTRIBUTING.md#gpu).
- A game kept inside a clone of this repository needs an empty `[workspace]` table in its
  `Cargo.toml`, or cargo takes it for a member of the engine's workspace and refuses to build it.
- Nothing is stable before 1.0: a release can break the API, and [`CHANGELOG.md`](CHANGELOG.md)
  says what changed.

## What it does not do yet

The engine is young; [`docs/ROADMAP.md`](docs/ROADMAP.md) says which stage brings what, and the
known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt) lists the smaller gaps. A game made with it
today has:

- No text, game UI, audio, physics, animation or editor.
- No asset files: the engine reads no image or model format. The examples' glTF loader
  (`examples/common/gltf.rs`) is example code to copy, not an API, and the asset pipeline comes
  later.
- No world or scene graph: the game gives the renderer the list of what to draw at every frame.
- A renderer without anti-aliasing, blended transparency, or culling of what the camera does not
  see; with a capped number of lights, and shadows from one directional light only.
- One window, which cannot go fullscreen, grab or hide the cursor, or report raw mouse motion; no
  gamepads, touch input or input methods.
- A run that stops when the GPU device is lost.
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
| [`CHANGELOG.md`](CHANGELOG.md) | Notable changes, per release. |

## License

SlimeEngine is licensed under the [Mozilla Public License 2.0](LICENSE). MPL-2.0 is a file-level
copyleft license: if you distribute a modified version of an engine file, the source of that
file must remain available under MPL-2.0. Files you write yourself, such as a game's code, can
use any license.
