# Contributing to SlimeEngine

This guide explains how to set up a development environment, check a change and submit it. The
rules themselves are in [`AGENTS.md`](AGENTS.md). That file is written for AI agents, which do
most of the development, but its rules on scope, architecture, code, documentation and commits
apply to every contributor.

## Before you start

- [`docs/PLAN.md`](docs/PLAN.md) says which stage and milestone are in progress. Work outside the
  current milestone is discussed with the project owner before it starts.
- [`docs/adr/`](docs/adr/README.md) holds the decisions already made. Do not re-decide them; if
  one no longer fits, propose a new ADR that supersedes it.

## Development environment

Linux on x86_64 is the only supported platform for now.

The Rust toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml): an exact stable
version with clippy, rustfmt, the standard library sources and rust-analyzer.
[ADR-0003](docs/adr/0003-toolchain-edition-and-msrv.md) explains why and how it is bumped.

### With Nix (recommended)

The repository is a [Nix flake](https://wiki.nixos.org/wiki/Flakes). With Nix installed and flakes
enabled, run this at the repository root:

```sh
nix develop
```

The shell it opens contains the pinned toolchain and the project's tools: `cargo-deny` (checks
dependency licenses and security advisories) and `lychee` (checks links in the documentation).
[`flake.lock`](flake.lock) pins every input, so every contributor and CI get the same versions.
To run a single command in that environment without opening a shell, use
`nix develop -c <command>`.

Nix only sees the files that git tracks. If you change `flake.nix` so that it reads a new file,
`git add` that file before running `nix develop`.

### Without Nix

Install [rustup](https://rustup.rs). It reads `rust-toolchain.toml` and installs the pinned
toolchain the first time you run `cargo` in the repository. Install the tools with
`cargo install --locked cargo-deny lychee`; CI uses the versions pinned by `flake.lock`. To open
windows, the examples need the windowing system's libraries, which are loaded at run time:
libwayland-client and libxkbcommon for Wayland; libX11, libXcursor, libXi and libxcb for X11.
Most desktop distributions install them already.

## Checks

One script runs every automated check, exactly as CI does:

```sh
nix develop -c scripts/check.sh
```

[`scripts/check.sh`](scripts/check.sh) runs rustfmt, clippy with warnings as errors, the build
and tests of every target, rustdoc with warnings as errors, `cargo deny` (dependency licenses,
security advisories and sources; it downloads the advisory database, so it needs network access),
the samples with no display server available, the windowed sample inside a headless Weston
compositor, and `lychee` (links between Markdown files). The individual commands are listed in
[`AGENTS.md`](AGENTS.md#environment-and-commands) for when you want to run only one of them.

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs the same script on every push to
`main` and every pull request; [ADR-0007](docs/adr/0007-documentation-tooling-and-ci.md) explains
the setup.

The lints they enforce are set once for the whole workspace in [`Cargo.toml`](Cargo.toml);
[ADR-0005](docs/adr/0005-lints-and-unsafe-policy.md) explains them and the rules for `unsafe`
code.

## Making a change

1. Keep each commit to one logical change, small enough to review. Write the subject in the
   imperative, in English; explain in the body why the change is needed when it is not obvious.
2. Update every document the change affects. The table in
   [`AGENTS.md`](AGENTS.md#files) says which document covers what.
3. Do not commit code that fails the checks of the
   [definition of done](AGENTS.md#definition-of-done).

## Architecture decisions

Decisions that affect the architecture, a project-wide convention or the roadmap go through the
ADR process described in [`docs/adr/README.md`](docs/adr/README.md): propose options with their
trade-offs and a recommendation, the owner decides, and the ADR is written in the change that
applies the decision.

## Dependencies

Ask before adding a dependency, and explain why it is needed, which alternative was considered
and its license. [ADR-0006](docs/adr/0006-dependency-policy.md) gives the admission criteria and
the allowed licenses; `cargo deny` enforces the license and advisory rules. Record every
dependency in [`docs/dependencies.md`](docs/dependencies.md) in the same change that adds it, and
declare its version once, in `[workspace.dependencies]` of the root `Cargo.toml`.

## Review

A change is reviewed against the definition of done and the rules in `AGENTS.md`, and against
the ADRs it touches.

## License of contributions

SlimeEngine is licensed under the [Mozilla Public License 2.0](LICENSE). Contributions are
accepted under the same license.
