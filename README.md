# SlimeEngine

SlimeEngine is a general-purpose, modular, open-source game engine written in Rust. It aims to
be a complete platform (runtime, asset pipeline, native editor and SDK) for linear narrative
games first, then open-world games, then large online worlds.

## Status

Pre-alpha. Stages 0 (*Foundations*), 1 (*Core and platform*), 2 (*GPU layer*) and 3a (*First
renderer*) are done; the latest release is 0.0.3 (git tag `v0.0.3`). Applications run headless or
with a window (Wayland or X11 on Linux, Win32 on Windows) that receives keyboard and mouse input,
with a fixed-step main loop, logging, panic reporting and a clean shutdown on Ctrl+C. The engine
opens the GPU (Vulkan on Linux, Direct3D 12 on Windows), paces frames with its own schedule and the
Mailbox present mode by default, names its GPU work for capture tools, and stops cleanly when the
GPU device is lost. Its first renderer, `slime-render`, draws instances of meshes from a camera,
with a depth test, each with a physically based material (glTF's metallic-roughness model, with base
color, metallic-roughness, normal, occlusion and emissive textures), lit by directional, point and
spot lights in physical units (lux and candelas), a sun casting shadows and a procedural sky that
surfaces reflect and that shows behind them, into the window or into offscreen images that image
tests compare with references; the `scene` example draws a glTF scene and its lights under a camera
driven by the mouse, with its frame time, CPU time, the GPU time of each pass and an exposure slider
in a dev overlay. Stage 3b, the physically based renderer, is in progress: the renderer draws the
scene in high dynamic range, shades it with glTF's metallic-roughness materials under lights in
physical units, the shadows of its sun and the light of its sky, exposes it, and maps it to the
display with a tone-mapping pass. [`docs/PLAN.md`](docs/PLAN.md) lists what comes next and the known
debt.

## Getting started

Linux and Windows, on x86_64. On Linux, with [Nix](https://nixos.org) and flakes enabled,
`nix develop` at the repository root opens a shell with the pinned Rust toolchain and the project's
tools. [`CONTRIBUTING.md`](CONTRIBUTING.md#development-environment) explains this, the setup without
Nix, and the setup on Windows. Then:

```sh
cargo build --workspace
cargo test --workspace
cargo run --example window   # the examples are listed in examples/README.md
scripts/check.sh             # every check, exactly as CI runs them (Linux only)
```

Each CI run on GitHub also publishes the `hello`, `headless`, `window`, `offscreen` and `scene`
examples built for Windows, to try without a toolchain.

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
