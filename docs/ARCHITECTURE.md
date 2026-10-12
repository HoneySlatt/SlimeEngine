# Architecture

This document maps the code: which crates exist, what each one is responsible for, how they
depend on each other, and where to start reading. It describes the repository as it is now;
[`ROADMAP.md`](ROADMAP.md) describes where it is going, and [`adr/`](adr/README.md) records why
it is built this way.

## Repository layout

```text
.
├── Cargo.toml           virtual workspace: shared metadata, lints and dependency versions
├── crates/              engine crates, one directory per crate, named after the package
│   ├── slime-app/       application lifecycle and main loop (skeleton)
│   └── slime-platform/  windows and input (skeleton)
├── docs/                roadmap, plan, decisions (adr/), glossary, this file
├── flake.nix            Nix development shell
├── rust-toolchain.toml  pinned Rust toolchain
└── AGENTS.md            rules for every contributor
```

[ADR-0004](adr/0004-workspace-layout-and-crate-naming.md) sets the layout and naming rules: the
root is a virtual workspace, engine crates are `crates/slime-<role>/`, and a directory or a crate
is created only when code needs it.

## Crates

| Crate | Role | Depends on | Status |
|---|---|---|---|
| [`slime-platform`](../crates/slime-platform/README.md) | Windows and input; the only crate that uses the windowing library | — | Skeleton |
| [`slime-app`](../crates/slime-app/README.md) | Application lifecycle, main loop, frame timing, diagnostics setup, headless mode | — | Skeleton |

A *skeleton* crate exists with its documentation and inherits the workspace policies, but holds
no code yet.

## Dependency rules

```text
slime-app ──may depend on──▶ slime-platform
```

- Dependencies between crates point one way. `slime-app` may depend on `slime-platform`;
  `slime-platform` never depends on `slime-app`. Neither depends on the other yet.
- Only `slime-platform` uses the windowing library, and its types never appear in a public API.
- A headless run never touches the windowing system.

## Main flows

None yet: the crates contain no code.

## Where to start reading

1. [`README.md`](../README.md) for what the project is and its state.
2. This file for the crate map.
3. [`PLAN.md`](PLAN.md) for the milestone in progress.
4. The `README.md` of the crate you are about to change.

## `unsafe` code

No crate contains `unsafe` code. [ADR-0005](adr/0005-lints-and-unsafe-policy.md) says when it is
allowed and how it must be written.
