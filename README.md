# SlimeEngine

SlimeEngine is a general-purpose, modular, open-source game engine written in Rust. It aims to
be a complete platform (runtime, asset pipeline, native editor and SDK) for linear narrative
games first, then open-world games, then large online worlds.

## Status

Pre-alpha. Stage 0 (*Foundations*) and Stage 1 (*Core and platform*) are implemented and pass
every check, locally and in CI. Applications run headless or with a window (Wayland or X11) that
receives keyboard and mouse input, with a fixed-step main loop, logging, panic reporting and a
clean shutdown on Ctrl+C. The input translation is tested, but key presses, mouse buttons and
scrolling still have to be checked by hand. Stage 2, the GPU layer, is in progress: the engine
opens a GPU device (Vulkan on Linux, Direct3D 12 on Windows) and renders into offscreen images,
checked against a reference image, but does not draw into the window yet. Version
0.0.1 will be tagged once input has been checked; [`docs/PLAN.md`](docs/PLAN.md) lists what
remains and the known debt.

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

Each CI run on GitHub also publishes the examples built for Windows, to try without a toolchain.

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
