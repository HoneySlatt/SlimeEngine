# Dependencies

Every third-party dependency of the project, with the reason it is used. The policy that admits
dependencies is [ADR-0006](adr/0006-dependency-policy.md); [`deny.toml`](../deny.toml) enforces its
license, advisory and source rules. Update this file in the same change that adds, removes or
repurposes a dependency.

## Crates

Direct dependencies only; `cargo deny` checks the licenses of everything they pull in. Versions
are set in `[workspace.dependencies]` of the root [`Cargo.toml`](../Cargo.toml).

| Crate | Used by | Why | Alternative considered | License | Added |
|---|---|---|---|---|---|
| [ctrlc](https://crates.io/crates/ctrlc) | `slime-app` | Handles SIGINT, SIGTERM and SIGHUP (feature `termination`) so that they stop the run cleanly ([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)) | `signal-hook`, lower level | MIT OR Apache-2.0 | M1.6 |
| [png](https://crates.io/crates/png) | examples and their tests only | Writes the image of the `offscreen` example and reads and writes the reference images of the image tests ([ADR-0018](adr/0018-reference-image-tests.md)) | `image`, which decodes many formats the examples do not need | MIT OR Apache-2.0 | M2.2 |
| [pollster](https://crates.io/crates/pollster) | `slime-gfx` | Waits for wgpu's requests, which are futures: opening the GPU, and the errors caught while a shader module or pipeline is created ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)) | A hand-written executor | MIT OR Apache-2.0 | M2.1 |
| [raw-window-handle](https://crates.io/crates/raw-window-handle) | `slime-platform` | The standard traits through which a graphics library (softbuffer now, wgpu later) draws into a window created by another library; `Window` implements them on purpose ([ADR-0011](adr/0011-windowing-with-winit.md)) | None: it is the interface wgpu and softbuffer accept | MIT OR Apache-2.0 OR Zlib | M1.4 |
| [softbuffer](https://crates.io/crates/softbuffer) | examples only | Fills the window from the CPU so that the compositor shows it, until the GPU layer exists. Temporary: removed in Stage 2 ([ADR-0002](adr/0002-amend-roadmap-stages-0-to-3.md)). Built without its `kms` backend. | A wgpu "clear" pass, which would bring the GPU layer into Stage 1 | MIT OR Apache-2.0 | M1.4 |
| [thiserror](https://crates.io/crates/thiserror) | `slime-gfx`, `slime-platform` | Derives `Display` and `Error` for engine error enums ([ADR-0008](adr/0008-errors-logging-and-assertions.md)) | Hand-written implementations | MIT OR Apache-2.0 | M1.4 |
| [tracing](https://crates.io/crates/tracing) | `slime-app`, `slime-gfx`, `slime-platform`, examples | Logging macros and profiling spans for all engine code ([ADR-0008](adr/0008-errors-logging-and-assertions.md)). Built without its default `attributes` feature, which pulls in a proc-macro stack. | `log`, which has no spans | MIT | M1.1 |
| [tracing-subscriber](https://crates.io/crates/tracing-subscriber) | `slime-app` | Formats, filters (`SLIME_LOG`, feature `env-filter`) and writes log events; forwards `log` records from dependencies | `env_logger`, which only serves `log` | MIT | M1.1 |
| [wgpu](https://crates.io/crates/wgpu) | `slime-gfx` | Access to the GPU through Vulkan (Linux) and Direct3D 12 (Windows), and the WGSL shader compiler; fixed by the roadmap ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). Built with the features `std`, `parking_lot`, `vulkan`, `dx12` and `wgsl` only. | Vulkan and Direct3D 12 directly | MIT OR Apache-2.0 | M2.1 |
| [winit](https://crates.io/crates/winit) | `slime-platform` | Windows, events and input on Wayland and X11 (Linux) and Win32 (Windows); loads the windowing libraries at run time ([ADR-0011](adr/0011-windowing-with-winit.md)). Client-side decorations are built without title text, to leave out the unmaintained `ttf-parser` | Wayland and X11 directly, or SDL3 | Apache-2.0 | M1.4 |

## System libraries

winit, softbuffer and wgpu do not link these libraries: they open them at run time, only when a
window is created or the GPU is opened, so a headless run needs none of them. They are part of the
operating system, not of SlimeEngine; the Nix shell puts them on the loader path
(`LD_LIBRARY_PATH` in [`flake.nix`](../flake.nix)).

| Library | Used for |
|---|---|
| libwayland-client | Wayland |
| libxkbcommon (and libxkbcommon-x11) | Keyboard layouts on Wayland and X11 |
| libX11, libX11-xcb, libXcursor, libXi, libxcb | X11 |
| libvulkan (the Vulkan loader) and a Vulkan driver (Mesa's RADV, ANV or NVK, or a vendor driver) | The GPU on Linux, through wgpu |

On Windows, Direct3D 12 and WARP, its software driver, are part of the operating system.

The checks render with lavapipe, the Vulkan driver of Mesa that runs on the CPU. The Nix shell
gives the path of its driver file in `SLIME_LAVAPIPE_ICD` (from nixpkgs' `mesa` package), and
`scripts/check.sh` points the Vulkan loader at it with `VK_DRIVER_FILES`.

## Development tools

Provided by the Nix development shell ([`flake.nix`](../flake.nix)); contributors without Nix
install them as described in [`CONTRIBUTING.md`](../CONTRIBUTING.md#development-environment).

| Tool | Why | Alternative considered | License |
|---|---|---|---|
| Rust toolchain (rustc, cargo, clippy, rustfmt, rust-analyzer) | Builds and checks the code; version pinned in `rust-toolchain.toml` ([ADR-0003](adr/0003-toolchain-edition-and-msrv.md)) | — | MIT OR Apache-2.0 |
| [rust-overlay](https://github.com/oxalica/rust-overlay) (flake input) | Turns `rust-toolchain.toml` into a Nix package, so Nix and rustup users get the same toolchain | The toolchain packaged by nixpkgs, whose version the project does not choose | MIT |
| [nixpkgs](https://github.com/NixOS/nixpkgs) (flake input) | Provides every other tool of the development shell | — | MIT |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Checks dependency licenses, security advisories and sources ([ADR-0006](adr/0006-dependency-policy.md)) | cargo-audit plus a separate license tool | MIT OR Apache-2.0 |
| [lychee](https://github.com/lycheeverse/lychee) | Checks links and anchors between Markdown files ([ADR-0007](adr/0007-documentation-tooling-and-ci.md)) | markdown-link-check, which needs Node.js | Apache-2.0 OR MIT |
| [Weston](https://gitlab.freedesktop.org/wayland/weston) | A Wayland compositor without screen in which the checks run the windowed sample ([ADR-0013](adr/0013-windowed-smoke-test-in-weston.md)) | Xvfb, which only covers X11 | MIT |

## CI actions

Used by [`.github/workflows/ci.yml`](../.github/workflows/ci.yml), pinned to commit SHAs.

| Action | Why | License |
|---|---|---|
| [actions/checkout](https://github.com/actions/checkout) | Checks out the repository | MIT |
| [cachix/install-nix-action](https://github.com/cachix/install-nix-action) | Installs Nix with flakes enabled, so CI runs the same development shell as contributors | Apache-2.0 |
| [actions/upload-artifact](https://github.com/actions/upload-artifact) | Publishes the examples built by the Windows job, to run by hand on a Windows machine | MIT |

The Windows job uses the Rust toolchain manager (rustup) and the Visual Studio build tools that the
`windows-2025` runner image provides; rustup installs the toolchain of `rust-toolchain.toml`.
