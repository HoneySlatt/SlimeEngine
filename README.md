# SlimeEngine

SlimeEngine is a general-purpose, modular, open-source game engine written in Rust. It aims to
be a complete platform (runtime, asset pipeline, native editor and SDK) for linear narrative
games first, then open-world games, then large online worlds.

## Status

Pre-alpha. The project is in Stage 0, *Foundations*: the repository contains its rules, its
roadmap, its plan, a reproducible development environment and a Cargo workspace whose two crates
are still empty skeletons. There is nothing to run yet. [`docs/PLAN.md`](docs/PLAN.md) shows the
milestone in progress.

## Getting started

Linux on x86_64 only. With [Nix](https://nixos.org) and flakes enabled, `nix develop` at the
repository root opens a shell with the pinned Rust toolchain and the project's tools.
[`CONTRIBUTING.md`](CONTRIBUTING.md#development-environment) explains this and the setup without
Nix. Then:

```sh
cargo build --workspace
cargo test --workspace
```

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

## License

SlimeEngine is licensed under the [Mozilla Public License 2.0](LICENSE). MPL-2.0 is a file-level
copyleft license: if you distribute a modified version of an engine file, the source of that
file must remain available under MPL-2.0. Files you write yourself, such as a game's code, can
use any license.
