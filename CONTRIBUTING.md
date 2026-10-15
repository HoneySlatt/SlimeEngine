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

The engine supports Linux and Windows, both on x86_64
([ADR-0015](docs/adr/0015-platforms-and-ci.md)); macOS comes later
([ADR-0014](docs/adr/0014-amend-roadmap-for-stage-2-and-windows.md)). Development happens mostly
on Linux, with Nix.

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
Weston (a Wayland compositor without screen, in which the checks run the windowed samples and the
games). It also puts the windowing libraries that the engine loads at run time on the loader path.
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
for X11. Without libxkbcommon, or libxkbcommon-x11 on X11, the examples panic instead of
reporting that the windowing system cannot be reached. Most desktop distributions install them
already.

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
  wgpu uses WARP, Microsoft's software driver. On WARP, run the tests one at a time
  (`cargo test --workspace -- --test-threads=1`), as the Windows CI job does: WARP's shader
  compiler crashes the process now and then when several threads compile shaders at once (see
  [Known debt](docs/PLAN.md#known-debt)).
- On Windows, Direct3D 12 compiles shaders with DXC when its library, `dxcompiler.dll`, lies
  beside the program, and otherwise with FXC, part of Windows, whose code runs slower: a warning in
  the log says so ([ADR-0066](docs/adr/0066-shipping-dxc-beside-the-programs.md)). From Git Bash,
  `scripts/fetch-dxc.sh target/debug/deps target/debug/examples target/debug` downloads DXC and
  puts it beside the tests, the samples and the game that cargo builds, as the Windows CI job
  does.
  `WGPU_DX12_COMPILER=fxc` asks for FXC, and `WGPU_DX12_COMPILER=dxc` for DXC wherever Windows
  finds `dxcompiler.dll`.
- `scripts/check.sh` runs the tests and the samples on lavapipe, Mesa's Vulkan driver that renders
  on the CPU, so that they give the same results on every machine and in CI. To run the GPU tests
  on your own GPU, run `cargo test` without `VK_DRIVER_FILES`; to reproduce the checks, run
  `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test` inside `nix develop`. Without Nix, install
  Mesa's Vulkan drivers (on Debian and Ubuntu, `mesa-vulkan-drivers`) and point `VK_DRIVER_FILES`
  at their `lvp_icd.x86_64.json` file.
- `WGPU_BACKEND` chooses another graphics API that the build supports (`WGPU_BACKEND=vulkan` on
  Windows), `WGPU_VALIDATION=1` turns on the validation layers of Vulkan or Direct3D 12, when
  they are installed, and `WGPU_DEBUG=1` passes the engine's labels and debug groups to the
  Vulkan driver in release builds, as debug builds always do (Direct3D 12 receives them in every
  build; `WGPU_DISCARD_HAL_LABELS=1` keeps them back).
- `SLIME_GPU_MEMORY_BUDGET`, a percentage from 1 to 100, makes creations fail with
  `GfxError::OutOfMemory` once the memory in use would pass that share of what the driver allows,
  to try how a game handles a full GPU memory
  ([ADR-0044](docs/adr/0044-running-out-of-gpu-memory.md)). It changes nothing on drivers whose
  memory budget wgpu does not read, RADV among them; the out-of-memory tests of `slime-gfx` then
  say they are skipped.
- Image tests compare a render with a reference image stored in `examples/tests/reference/`,
  within a tolerance that depends on the driver
  ([ADR-0018](docs/adr/0018-reference-image-tests.md), amended by
  [ADR-0028](docs/adr/0028-image-tests-tolerate-a-few-pixels.md),
  [ADR-0030](docs/adr/0030-image-tests-strict-on-the-reference-driver.md) and
  [ADR-0054](docs/adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md)). When one fails, it
  prints the path of the image it rendered; compare the two by eye. If your change alters what a
  test draws on purpose, render the reference again on lavapipe, look at the new image, and commit
  it with the change (this renders the references of every image test; each test names its
  reference in its module comment):

  ```sh
  SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
      cargo test -p slime-examples
  ```

#### Looking at a window without a display

The checks run the windowed samples in a headless Weston. To see what such a window shows, run
Weston with a renderer that keeps an image and with its debug protocol, which authorizes the
screenshot client, start the sample in it, and take the screenshot while it runs:

```sh
runtime=$(mktemp -d)
cargo run -p slime-cook -- cook examples/assets target/cooked/examples   # what `scene` draws
XDG_RUNTIME_DIR=$runtime weston --backend=headless --renderer=pixman --debug \
    --socket=slime-shot --idle-time=0 &
XDG_RUNTIME_DIR=$runtime WAYLAND_DISPLAY=slime-shot VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
    cargo run --example scene -- --frames 600 &
sleep 5 && (cd "$runtime" && XDG_RUNTIME_DIR=$runtime WAYLAND_DISPLAY=slime-shot weston-screenshooter)
```

The screenshot is written in the runtime directory as `wayland-screenshot-<date>.png`.

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

### Changing assets while a game runs

`slime-cook watch` cooks a directory of sources again each time one of its files changes, and a
game started with `--watch` loads its scenes again when their cooked data changes, keeping what did
not change: a texture saved in an image editor, or a scene exported again from Blender, shows in the
running game shortly after ([ADR-0076](docs/adr/0076-hot-reload-of-cooked-data.md)).
In two terminals:

```sh
cargo run --release -p slime-cook -- watch games/hunnawood/assets/zone target/cooked/hunnawood
cargo run -p hunnawood -- --watch
```

The `scene` example takes `--watch` too, with `examples/assets` cooked into
`target/cooked/examples`. A change of the zone's layout starts the game's run again; a change of how
it looks does not ([`games/hunnawood/README.md`](games/hunnawood/README.md#playing)). Each change
leaves the cooked files it replaced in the directory of cooked data, and a watch stopped with Ctrl+C
the temporary files of its cook: delete the directory to clean it.

## Checks

One script runs every automated check, exactly as CI does:

```sh
nix develop -c scripts/check.sh
```
[`scripts/check.sh`](scripts/check.sh) runs rustfmt, clippy with warnings as errors, the build and
tests of every target, the renderer's tests again with glam's assertions on (which a game that turns
them on also turns on in the engine), rustdoc with warnings as errors, `cargo deny` (dependency
licenses, security advisories and sources; it downloads the advisory database, so it needs network
access), the list of the sample game's source assets (`scripts/fetch-assets.sh`), `slime-cook check`
(every source asset has a metadata file of its own,
[ADR-0069](docs/adr/0069-the-asset-pipeline.md)), the sample's assets cooked twice and compared, the
sample game's zone fetched and cooked, and the tests that read it, the
samples with no display server available, the windowed samples, the game and the windowed
lifecycle test inside a headless Weston compositor (and that test's resize scenario in a second
one, at scale 2), and `lychee` (links between Markdown files). The individual commands are listed
in [`AGENTS.md`](AGENTS.md#environment-and-commands) for when you want to run only one of them.

The script also runs clippy for the Windows target, which catches code that does not compile on
Windows without a Windows machine.

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs on every push to `main` and every
pull request. A Linux job runs the same script; a Windows job runs clippy, the tests, then the
image tests, the GPU timing test and the multi-pass test again with `--nocapture` so that its log
shows what they measured on WARP, then the headless samples on Windows, runs the release build of
the headless ones, and publishes the `hello`, `headless`, `window`, `offscreen` and `scene`
examples built for Windows, with the sample cooked for `scene`, as an artifact named
`slime-examples-windows`, to run by hand on a Windows machine. The Windows job runs the tests one
at a time (`--test-threads=1`): WARP's shader compiler crashes the process, now and then, when
several threads compile shaders at once. It does not stop at the first test program that fails,
and when a step fails, it publishes instead the images that the failing image tests rendered on
WARP, as an artifact named `image-test-failures-windows`.
[ADR-0007](docs/adr/0007-documentation-tooling-and-ci.md) and
[ADR-0015](docs/adr/0015-platforms-and-ci.md) explain the setup.

The lints they enforce are set once for the whole workspace in [`Cargo.toml`](Cargo.toml);
[ADR-0005](docs/adr/0005-lints-and-unsafe-policy.md) explains them and the rules for `unsafe`
code.

## Making a change

1. Keep each commit to one logical change, small enough to review. Write the subject in the
   imperative, in English; explain in the body why the change is needed when it is not obvious.
2. Update every document the change affects. The table in
   [`AGENTS.md`](AGENTS.md#files) says which document covers what, and each fact has one home that
   the others link to ([one home per fact](AGENTS.md#one-home-per-fact)).
3. Do not commit code that fails the checks of the
   [definition of done](AGENTS.md#definition-of-done).
4. A source asset, a glTF, PNG or JPEG file that the asset pipeline imports, is committed with its
   metadata file, `<file>.meta`, which holds its asset ID
   ([ADR-0069](docs/adr/0069-the-asset-pipeline.md)): `cargo run -p slime-cook -- scan <directory>`
   writes one for each new file. Move or rename the two together, so that the asset keeps its ID;
   a copied file needs its own, so do not copy the metadata file with it.
   The source assets that the sample game downloads are not committed: they are listed, with
   their authors and licences, in `games/hunnawood/assets/sources.tsv`, mirrored, and fetched by
   `scripts/fetch-assets.sh` ([ADR-0070](docs/adr/0070-the-sample-games-source-assets.md);
   [`games/hunnawood/README.md`](games/hunnawood/README.md#its-assets) says how to add one, and
   how to build the zone again with Blender).

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
