# 0003. Pin the toolchain in one file, use edition 2024, set the MSRV to the pinned version

- Status: Accepted
- Date: 2026-10-12

## Context

Builds must be reproducible: a contributor, CI and a future release must compile the engine with
the same compiler, or warnings, lint results and generated code drift between them. Development
happens on NixOS, but contributors without Nix must be able to build too, and Windows support,
which comes later, will not have Nix at all.

The minimum supported Rust version (MSRV) matters to people who depend on a crate. Before 1.0,
SlimeEngine promises no API stability and is not published on crates.io (only git tags are
released), so its users build it from the repository with the toolchain the repository asks for.

## Decision

- `rust-toolchain.toml` at the repository root is the single source of truth for the toolchain:
  an exact stable version (currently 1.99.0), with the `clippy`, `rustfmt`, `rust-src` and
  `rust-analyzer` components.
- The Nix development shell (`flake.nix`) builds its toolchain from that file through
  [rust-overlay](https://github.com/oxalica/rust-overlay). Contributors without Nix get the same
  toolchain from rustup, which reads the same file.
- All crates use Rust edition 2024.
- The workspace declares `rust-version` equal to the pinned toolchain version. It is the MSRV.
- The toolchain is bumped on purpose, in its own commit, updating `rust-toolchain.toml` and
  `rust-version` together and fixing any new warning in the same commit.
- CI uses the pinned toolchain only. There is no separate job testing an older compiler.

## Alternatives considered

- **The toolchain packaged by nixpkgs.** Its version follows the nixpkgs revision, not the
  project's choice, and contributors without Nix would have no file describing it.
- **A low MSRV kept N versions behind stable, tested by a dedicated CI job.** Valuable for a
  published library with many dependents; for an engine without stability promises, it costs a
  CI job and forbids recent language features for no user benefit.
- **Nightly Rust.** Unstable features can change or disappear, which conflicts with a project
  meant to last for years.
- **Edition 2021.** No reason to start a new project on the older edition.

## Consequences

- Every contributor and CI build with the same compiler, so warnings and lint results agree.
- The project depends on rust-overlay, a third-party flake (MIT license), to turn the toolchain
  file into a Nix package. Replacing it means writing the toolchain derivation by hand or
  switching to another overlay; nothing outside `flake.nix` depends on it.
- Users of the engine need a recent compiler. Before 1.0 this is acceptable; the MSRV policy
  should be revisited when the engine is published or when its API is promised stable.
- Bumping the toolchain is a deliberate, reviewable change rather than a side effect of updating
  `flake.lock`.
