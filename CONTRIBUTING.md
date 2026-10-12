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

The engine supports Linux and Windows, both on x86_64; macOS comes later
([ADR-0015](docs/adr/0015-platforms-and-ci.md)). Development happens mostly on Linux, with Nix.

The Rust toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml): an exact stable
version with clippy, rustfmt, the standard library sources, rust-analyzer, and the Windows
standard library so that the code can be checked for Windows from Linux.
[ADR-0003](docs/adr/0003-toolchain-edition-and-msrv.md) explains why and how it is bumped.

### On Linux, with Nix (recommended)

The repository is a [Nix flake](https://wiki.nixos.org/wiki/Flakes). With Nix installed and flakes
enabled, run this at the repository root:

```sh
nix develop
```

The shell it opens contains the pinned toolchain and the project's tools: `cargo-deny` (checks
dependency licenses and security advisories), `lychee` (checks links in the documentation) and
Weston (a Wayland compositor without screen, in which the checks run the windowed sample). It
also puts the windowing libraries that the engine loads at run time on the loader path.
[`flake.lock`](flake.lock) pins every input, so every contributor and CI get the same versions.
To run a single command in that environment without opening a shell, use
`nix develop -c <command>`.

Nix only sees the files that git tracks. If you change `flake.nix` so that it reads a new file,
`git add` that file before running `nix develop`.

### On Linux, without Nix

Install [rustup](https://rustup.rs). It reads `rust-toolchain.toml` and installs the pinned
toolchain the first time you run `cargo` in the repository. Install the tools with
`cargo install --locked cargo-deny lychee`, and Weston from your distribution's packages; CI uses
the versions pinned by `flake.lock`. To open windows, the examples need the windowing system's
libraries, which are loaded at run time: libxkbcommon for both windowing systems;
libwayland-client for Wayland; libX11, libX11-xcb, libXcursor, libXi, libxcb and libxkbcommon-x11
for X11. Most desktop distributions install them already.

### On Windows

Install [rustup](https://rustup.rs) and the Visual Studio Build Tools with the "Desktop development
with C++" workload, which provides the MSVC compiler and linker and the Windows SDK. rustup reads
`rust-toolchain.toml` like on Linux. `scripts/check.sh` needs Linux and Nix: on Windows, run the
cargo commands listed in [`AGENTS.md`](AGENTS.md#environment-and-commands) instead.

### GPU

The GPU layer uses Vulkan on Linux and Direct3D 12 on Windows
([ADR-0016](docs/adr/0016-gpu-layer-with-wgpu.md)).

- On Linux, the engine needs the Vulkan loader (`libvulkan`) and a Vulkan driver: Mesa's RADV
  (AMD), ANV (Intel) or NVK (NVIDIA), or NVIDIA's own driver. The Nix shell provides the loader.
- On Windows, Direct3D 12 is part of the system. Without a GPU, for example in a virtual machine,
  wgpu uses WARP, Microsoft's software driver.
- `scripts/check.sh` runs the tests and the samples on lavapipe, Mesa's Vulkan driver that renders
  on the CPU, so that they give the same results on every machine and in CI. To run the GPU tests
  on your own GPU, run `cargo test` without `VK_DRIVER_FILES`; to reproduce the checks, run
  `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test` inside `nix develop`. Without Nix, install
  Mesa's Vulkan drivers (on Debian and Ubuntu, `mesa-vulkan-drivers`) and point `VK_DRIVER_FILES`
  at their `lvp_icd.x86_64.json` file.
- `WGPU_BACKEND` chooses another graphics API that the build supports (`WGPU_BACKEND=vulkan` on
  Windows), and `WGPU_VALIDATION=1` turns on the validation layers of Vulkan or Direct3D 12, when
  they are installed.
- Image tests compare a render with a reference image stored in `examples/tests/reference/`,
  within a tolerance ([ADR-0018](docs/adr/0018-reference-image-tests.md)). When one fails, it
  prints the path of the image it rendered; compare the two by eye. If your change alters what a
  test draws on purpose, render the reference again on lavapipe, look at the new image, and commit
  it with the change:

  ```sh
  SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
      cargo test -p slime-examples --test offscreen
  ```

#### Capturing a frame in RenderDoc

Debug builds give GPU capture tools the names of the engine's GPU work: each render pass is named
after its target (the window's pass of frame 42 is `window, frame 42`), `RenderPass::debug_group`
names groups of commands (the samples' triangle is drawn in `triangle`), and every GPU object
carries its label. To look at a frame of the `window` example in
[RenderDoc](https://renderdoc.org), which the development shell does not include, launch the
example under RenderDoc from inside `nix develop`:

```sh
cargo build --example window
RENDERDOC=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#renderdoc)
VK_ADD_IMPLICIT_LAYER_PATH=$RENDERDOC/share/vulkan/implicit_layer.d \
    $RENDERDOC/bin/renderdoccmd capture -w -c /tmp/window target/debug/examples/window
```

`nix build` fetches RenderDoc from the same nixpkgs as the development shell, and
`VK_ADD_IMPLICIT_LAYER_PATH` lets the Vulkan loader find its layer without registering it on the
system. Then start `$RENDERDOC/bin/qrenderdoc` from a terminal outside `nix develop` (the shell's
library path does not suit RenderDoc's interface), attach it to the running example (File, Attach to
Running Instance), and trigger a capture. Captures on Windows, through Direct3D 12, have not been
tried yet.

## Checks

One script runs every automated check, exactly as CI does:

```sh
nix develop -c scripts/check.sh
```

[`scripts/check.sh`](scripts/check.sh) runs rustfmt, clippy with warnings as errors, the build
and tests of every target, rustdoc with warnings as errors, `cargo deny` (dependency licenses,
security advisories and sources; it downloads the advisory database, so it needs network access),
the samples with no display server available, the windowed sample and the windowed lifecycle test
inside a headless Weston compositor, and `lychee` (links between Markdown files). The individual commands are listed in
[`AGENTS.md`](AGENTS.md#environment-and-commands) for when you want to run only one of them.

The script also runs clippy for the Windows target, which catches code that does not compile on
Windows without a Windows machine.

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs on every push to `main` and every
pull request. A Linux job runs the same script; a Windows job runs clippy, the tests and the
headless samples on Windows, runs the release build of the headless ones, and publishes the
`hello`, `headless`, `window` and `offscreen` examples built for Windows as an artifact named
`slime-examples-windows`, to run by hand on a Windows machine.
[ADR-0007](docs/adr/0007-documentation-tooling-and-ci.md) and
[ADR-0015](docs/adr/0015-platforms-and-ci.md) explain the setup.

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
