# 0015. Platforms and CI on Linux and Windows

- Status: Accepted
- Date: 2026-10-12

## Context

[ADR-0014](0014-amend-roadmap-for-stage-2-and-windows.md) makes Windows a first platform, with
Linux, from Stage 2 on. [ADR-0007](0007-documentation-tooling-and-ci.md) runs every check from one
bash script inside the Nix development shell, which does not exist on Windows. GitHub provides
Windows runners with Visual Studio (the MSVC toolchain) and rustup. The owner has a Windows virtual
machine for checks by hand. Most of the engine's code is already portable (winit, ctrlc, tracing),
but nothing verified it.

## Decision

- The supported targets are `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`. On Windows,
  the MSVC toolchain is used, not MinGW: it is the platform's native ABI, the one Windows
  debugging and profiling tools expect, and it needs no MinGW runtime next to a game.
- `rust-toolchain.toml` includes the Windows standard library, and `scripts/check.sh` runs clippy
  for the Windows target too. Linux contributors and the Linux CI job therefore catch code that does
  not compile on Windows, without a Windows machine.
- CI has two jobs:
  - **Linux**: `scripts/check.sh` in the Nix shell, unchanged from ADR-0007, with the clippy step
    above. It runs every check that does not depend on the platform: formatting, rustdoc,
    dependencies, links.
  - **Windows**: rustup installs the toolchain of `rust-toolchain.toml`, then the job runs clippy,
    the tests and the headless samples, builds the examples in release mode and publishes them as
    a CI artifact (kept 14 days), so that they can be run by hand in a Windows virtual machine
    without a Rust toolchain.
- A test that cannot run on a platform is compiled out with `#[cfg]`, and its file says why and how
  the behaviour is checked there instead. The first case is the signal tests, which send Unix
  signals.
- What CI cannot check on Windows (windows, input, rendering on a real GPU, frame pacing) is
  checked by hand in the Windows virtual machine, in the milestones that change it.

This amends ADR-0007: CI runs a second job, on Windows, outside the Nix shell, and the checks are
listed in two places.

## Alternatives considered

- **One cross-platform check program (`cargo xtask`) instead of `scripts/check.sh`.** The checks
  would be listed once for both platforms, but it is more code to maintain now, and the Linux
  checks rely on tools from the Nix shell (Weston, cargo-deny, lychee) that the Windows job does not
  need. To reconsider if the two lists drift apart.
- **Cross-compiling on Linux and running the tests under Wine.** No Windows runner is needed, but
  Wine is not Windows: a failure could be Wine's, and a success would prove little.
- **The GNU target (MinGW).** Easier to cross-compile from Linux, but not the platform's native
  ABI. Cross-compilation from Linux will be decided when games are packaged (Stage 9).
- **Checking Windows only before releases.** Problems would be found late and in bulk.

## Consequences

- The checks are listed in two places (`scripts/check.sh` and the Windows job); a new check needs
  a decision on whether it also runs on Windows.
- CI takes longer, and while the repository is private, Windows runner minutes count double.
- The Nix shell downloads the Windows standard library too.
- A Linux contributor gets Windows compile errors locally, but not Windows run-time failures: those
  appear in the Windows job or in the virtual machine.
