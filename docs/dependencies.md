# Dependencies

Every third-party dependency of the project, with the reason it is used. The policy that admits
dependencies is [ADR-0006](adr/0006-dependency-policy.md); [`deny.toml`](../deny.toml) enforces its
license, advisory and source rules. Update this file in the same change that adds, removes or
repurposes a dependency.

## Crates

Direct dependencies only; `cargo deny` checks the licenses of everything they pull in for the
supported targets ([ADR-0023](adr/0023-dependency-checks-for-the-supported-targets.md)). Versions
are set in `[workspace.dependencies]` of the root [`Cargo.toml`](../Cargo.toml).

| Crate | Used by | Why | Alternative considered | License | Added |
|---|---|---|---|---|---|
| [base64](https://crates.io/crates/base64) | `slime-mcp` | Encodes the captures of a game's image that the MCP server returns, in base64, as the Model Context Protocol carries images ([ADR-0122](adr/0122-the-development-channel.md)) | An encoder of the server's own, a few dozen lines to write and test | MIT OR Apache-2.0 | M6a.6 |
| [bevy_mikktspace](https://crates.io/crates/bevy_mikktspace) | `slime-cook` | Computes the tangents of a mesh that has none with MikkTSpace, as glTF asks and as normal textures are baked against, the same as Blender's on the sample scene ([ADR-0069](adr/0069-the-asset-pipeline.md)). A port of the reference C library to safe Rust, without dependencies | Tangents computed per triangle, which the renderer did before and which shade differently at seams | Zlib AND (MIT OR Apache-2.0) | M4a.4 |
| [blake3](https://crates.io/crates/blake3) | `slime-cook` | Derives the IDs of the assets inside a source file from the file's ID, their kind and their name, with its key derivation; and names what the cooker makes by the hash of what made it ([ADR-0069](adr/0069-the-asset-pipeline.md)). Built with its `pure` feature, without its SIMD code in C and assembly, so that no C compiler is needed | SHA-256 (`sha2`), slower, without key derivation | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | M4a.2 |
| [block_compression](https://crates.io/crates/block_compression) | `slime-cook`; the tests of `slime-gfx` | Encodes BC7 and BC5 blocks on the CPU, a port of Intel's ISPC encoders to Rust: the cooker compresses textures with it ([ADR-0071](adr/0071-cooked-textures.md)); and decodes them, so that the tests of compressed textures compare what the GPU reads with what the blocks hold ([ADR-0069](adr/0069-the-asset-pipeline.md)). Built without its default features (`bc6h`, and `wgpu`, which encodes on the GPU) | `intel_tex_2`, the same encoders in ISPC, native code that the build links | MIT | M4a.3 |
| [ctrlc](https://crates.io/crates/ctrlc) | `slime-app` | Handles SIGINT, SIGTERM and SIGHUP (feature `termination`) so that they stop the run cleanly ([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)) | `signal-hook`, lower level | MIT OR Apache-2.0 | M1.6 |
| [ktx2](https://crates.io/crates/ktx2) | `slime-assets` | Writes the header, level index and data format descriptor of KTX2 files, the Khronos Group's container for GPU textures, which `CookedTexture::to_ktx2` puts together: the files of cooked textures ([ADR-0071](adr/0071-cooked-textures.md)); its reader checks their layout in tests. `CookedTexture::from_ktx2` reads them with a reader of the engine's own, which checks a file before it copies its levels ([ADR-0102](adr/0102-a-damaged-file-gives-an-error.md)) | A container of our own, which no other tool could open | Apache-2.0 | M4a.3 |
| [gltf](https://crates.io/crates/gltf) | `slime-cook`; the examples' tests | Reads and validates glTF files, their meshes, materials, textures, lights, node hierarchy and the `extras` of nodes (feature `extras`, M4a.4), for the cooker's importer: `.gltf` files and GLB ([ADR-0069](adr/0069-the-asset-pipeline.md)). The examples' tests write small glTF files with its types, for the cooker to import. Built without its `import` feature, which would bring the `image` and `base64` crates (its other default features, `names` and `utils`, stay on), and with its `KHR_materials_emissive_strength` (M3b.3) and `KHR_lights_punctual` (M3b.4) features, which read those extensions and add no crate; the cooker decodes images with `png` and `zune-jpeg`. Added for the temporary loader of the examples ([ADR-0029](adr/0029-temporary-gltf-loading.md)), which M4a.5 deleted | A parser of our own on `serde_json`: more code to write and keep right | MIT OR Apache-2.0 | M3.5; features M3b.3, M3b.4, M4a.4 |
| [getrandom](https://crates.io/crates/getrandom) | `slime-channel`; `slime-app` through it, with its `development` feature | Draws the development channel's token, 128 bits of the system's randomness, which the standard library does not give yet ([ADR-0122](adr/0122-the-development-channel.md)). Already built for `uuid` | The system's randomness reached by the engine's own code, which needs `unsafe` (ADR-0005) or a crate of Windows bindings; a value made from the time and the process, which another user could guess | MIT OR Apache-2.0 | M6a.6 |
| [egui](https://crates.io/crates/egui) | `slime-devui`, which re-exports it; the examples and `hunnawood` through it | The immediate-mode UI of the dev overlay, tooling only ([ADR-0031](adr/0031-dev-overlay-with-egui.md)), not the editor's, which builds on the engine's own UI framework ([ADR-0098](adr/0098-the-games-the-engine-aims-at.md)). Its default fonts, in the crate `epaint_default_fonts` (SIL Open Font License 1.1 and Ubuntu Font Licence 1.0, allowed for that crate only in `deny.toml`), ship in the programs that draw the overlay, *Hunna Wood* among them, with their licenses ([ADR-0105](adr/0105-egui-fonts-ship-with-the-dev-overlay.md)) | Statistics in the log only; a text renderer of our own | MIT OR Apache-2.0 | M3.6 |
| [egui-wgpu](https://crates.io/crates/egui-wgpu) | `slime-devui` | Draws egui with wgpu, the same wgpu as `slime-gfx`, through its `wgpu` feature ([ADR-0031](adr/0031-dev-overlay-with-egui.md)). Built without its default features, which would enable every wgpu backend; `egui-winit` is not used, the engine translates its own events | A renderer of our own for egui's triangles | MIT OR Apache-2.0 | M3.6 |
| [glam](https://crates.io/crates/glam) | `slime-render` and `slime-world`, which re-export it; the examples and the game through them; `slime-cook`, for the vectors of the meshes it imports; `slime-reflect`, which describes its vectors and quaternions; the tests of `slime-serialize`, which write them | Vectors, matrices and quaternions for the renderer and its API: the camera's matrices in WebGPU's convention, the instances' transforms ([ADR-0027](adr/0027-the-render-crate.md)); for the world, `f64` positions and the transforms of its entities ([ADR-0083](adr/0083-transforms-and-the-hierarchy.md)); reflection describes them where its trait is, as the orphan rule asks ([ADR-0080](adr/0080-reflection-and-serialization.md)) | `nalgebra`, heavier and less used by graphics code; vectors and matrices of our own, outside the engine's purpose | MIT OR Apache-2.0 | M3.2 |
| [png](https://crates.io/crates/png) | `slime-cook`; the examples and their tests; `slime-mcp` | Decodes the PNG images that the cooker imports ([ADR-0069](adr/0069-the-asset-pipeline.md)); writes the image of the `offscreen` example and reads and writes the reference images of the image tests ([ADR-0018](adr/0018-reference-image-tests.md)); writes the captures that the MCP server returns ([ADR-0122](adr/0122-the-development-channel.md)) | `image`, which decodes many formats the examples do not need | MIT OR Apache-2.0 | M2.2 |
| [proc-macro2](https://crates.io/crates/proc-macro2), [quote](https://crates.io/crates/quote) and [syn](https://crates.io/crates/syn) | `slime-reflect-derive` | Write the derive macro of reflection, `#[derive(Reflect)]`: tokens, their quoting, and the parsing of the type it is given ([ADR-0080](adr/0080-reflection-and-serialization.md)). Already built by the workspace, for `thiserror`'s derive, so they add no crate to the build; `syn` with its default features, without `full`, which a derive does not need | Every type described by hand, written twice and a field forgotten without an error; `venial` or `unsynn`, lighter parsers that the build would add beside `syn` | MIT OR Apache-2.0 | M5a.5 |
| [rmcp](https://crates.io/crates/rmcp) | `slime-mcp` | The official Rust implementation of the Model Context Protocol, through which agents call the engine's MCP server: its server, its tools described from the types of their parameters (`macros`), and its transport on the standard input and output (`transport-io`), without its default features ([ADR-0099](adr/0099-ai-agents-drive-the-engine.md), [ADR-0122](adr/0122-the-development-channel.md)). It brings `tokio`, `serde`, `serde_json`, `schemars` (MIT) and their dependencies, into the server only | The protocol written by the engine on `serde_json`, each of its versions followed by hand; implementations from the community, behind the official one | Apache-2.0 | M6a.6 |
| [serde](https://crates.io/crates/serde) | `slime-mcp` | Reads the parameters of the server's tools, as `rmcp` hands them over (feature `derive`); already built for `rmcp` | None: `rmcp`'s tools take their parameters so | MIT OR Apache-2.0 | M6a.6 |
| [serde_json](https://crates.io/crates/serde_json) | the tests of `slime-mcp` | Writes and reads the messages of the agent's sessions that the server's tests play (`drive`, `views`); already built for `rmcp` | A writer and a reader of JSON of the test's own | MIT OR Apache-2.0 | M6a.6 |
| [tokio](https://crates.io/crates/tokio) | `slime-mcp` | The asynchronous runtime that `rmcp` runs on, on one thread (features `macros` and `rt`); the requests to the game wait on its threads for blocking work | None: `rmcp` runs on it | MIT | M6a.6 |
| [pollster](https://crates.io/crates/pollster) | `slime-gfx` | Waits for wgpu's requests, which are futures: opening the GPU, and the errors caught while a shader module or pipeline is created ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)) | A hand-written executor | MIT OR Apache-2.0 | M2.1 |
| [raw-window-handle](https://crates.io/crates/raw-window-handle) | `slime-platform`, `slime-gfx` | The standard traits through which a graphics library draws into a window created by another library: `slime_platform::Window` implements them on purpose ([ADR-0011](adr/0011-windowing-with-winit.md)), and `slime-gfx` draws into any window that implements them ([ADR-0020](adr/0020-rendering-in-the-main-loop.md)). `slime-gfx` enables its `alloc` feature, for the implementations on `Arc`. | None: it is the interface wgpu accepts | MIT OR Apache-2.0 OR Zlib | M1.4 |
| [thiserror](https://crates.io/crates/thiserror) | `slime-app`, `slime-assets`, `slime-channel`, `slime-cook`, `slime-editor`, `slime-gfx`, `slime-platform`, `slime-render`, `slime-serialize`, `slime-world` | Derives `Display` and `Error` for engine error enums ([ADR-0008](adr/0008-errors-logging-and-assertions.md)) | Hand-written implementations | MIT OR Apache-2.0 | M1.4 |
| [toml_edit](https://crates.io/crates/toml_edit) | `slime-serialize` | Parses TOML documents, the engine's text format for sources and saves, and gives where each key and value lies in the text, so that errors say their line and column ([ADR-0080](adr/0080-reflection-and-serialization.md), [ADR-0086](adr/0086-serialization.md)). Its parser only (feature `parse`), which brings `toml_parser`, `winnow` and `toml_datetime`, all MIT OR Apache-2.0; without its writer, since the engine writes its documents itself, and without `serde`. Already in the lock file, for winit on Android | `toml`, which reads through `serde`; a TOML parser of our own | MIT OR Apache-2.0 | M5a.7 |
| [tracing](https://crates.io/crates/tracing) | `slime-app`, `slime-channel`, `slime-editor`, `slime-gfx`, `slime-platform`, `slime-render`, `slime-world`, `slime-world-render`, examples, `hunnawood` | Logging macros and profiling spans for all engine code ([ADR-0008](adr/0008-errors-logging-and-assertions.md)). Built without its default `attributes` feature, which pulls in a proc-macro stack. | `log`, which has no spans | MIT | M1.1 |
| [nu-ansi-term](https://crates.io/crates/nu-ansi-term) | `slime-app`, on Windows only | Asks the console to interpret the escape sequences that color the log (`enable_ansi_support`), so that colors are used in the older console host too, and only when the console accepts; already built as a dependency of `tracing-subscriber` | A Win32 call of our own, which needs `unsafe` (ADR-0005) and a crate of Windows bindings | MIT | after M3b.3, by the owner's decision |
| [tracing-subscriber](https://crates.io/crates/tracing-subscriber) | `slime-app` | Formats, filters (`SLIME_LOG`, feature `env-filter`) and writes log events; forwards `log` records from dependencies (feature `tracing-log`, named in `crates/slime-app/Cargo.toml`); carries the layer that writes a trace of the spans (`SLIME_TRACE`, [ADR-0104](adr/0104-the-build-ci-and-measurements.md)) | `env_logger`, which only serves `log` | MIT | M1.1 |
| [uuid](https://crates.io/crates/uuid) | `slime-cook` | Makes the random asset IDs of source files, UUIDs of version 4, from the operating system's random source (feature `v4`, through `getrandom`), and the UUIDs of version 8 that the IDs of the assets inside a file are written as (feature `v8`) ([ADR-0069](adr/0069-the-asset-pipeline.md)). `slime-assets` reads and writes the text of an ID itself, so that games do not link it | Random bytes from `getrandom` directly, and the UUID's bits set by hand | MIT OR Apache-2.0 | M4a.2 |
| [zune-jpeg](https://crates.io/crates/zune-jpeg) | `slime-cook` | Decodes the JPEG images that the cooker imports, the format of most photographed textures, into 8-bit texels, its size checked from the header before the texels are allocated ([ADR-0069](adr/0069-the-asset-pipeline.md)). Its default features keep its SIMD code for x86, behind its own `unsafe` | Converting every JPEG to PNG before cooking: files about twice as large | MIT OR Apache-2.0 OR Zlib | M4a.3 |
| [wgpu](https://crates.io/crates/wgpu) | `slime-gfx`; `slime-devui` through its `wgpu` feature | Access to the GPU through Vulkan (Linux) and Direct3D 12 (Windows), and the WGSL shader compiler; fixed by the roadmap ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). Built with the features `std`, `parking_lot`, `vulkan`, `dx12` and `wgsl` only. The dev overlay reaches it to draw egui ([ADR-0031](adr/0031-dev-overlay-with-egui.md)) | Vulkan and Direct3D 12 directly | MIT OR Apache-2.0 | M2.1 |
| [winit](https://crates.io/crates/winit) | `slime-platform` | Windows, events and input on Wayland and X11 (Linux) and Win32 (Windows); loads the windowing libraries at run time ([ADR-0011](adr/0011-windowing-with-winit.md)). Client-side decorations are built without title text, to leave out the unmaintained `ttf-parser` | Wayland and X11 directly, or SDL3 | Apache-2.0 | M1.4 |

## System libraries

winit and wgpu do not link these libraries: they open them at run time, only when a window is
created or the GPU is opened, so a headless run needs none of them. They are part of the
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

## Shipped with the Windows programs

| Program | Why | Alternative considered | License |
|---|---|---|---|
| [DXC](https://github.com/microsoft/DirectXShaderCompiler) 1.9.2609, its `dxcompiler.dll` | The shader compiler of Direct3D 12 that the engine uses when its library lies beside the program: the scene pass runs about twice as fast as with FXC's code ([ADR-0066](adr/0066-shipping-dxc-beside-the-programs.md)). [`scripts/fetch-dxc.sh`](../scripts/fetch-dxc.sh) downloads Microsoft's release and checks its SHA-256; CI puts it beside the tests and samples it runs on Windows and the programs it publishes, with its licence as `dxcompiler-LICENSE.txt` | FXC, part of Windows ([ADR-0036](adr/0036-direct3d-12-shader-compiler.md)); wgpu's `static-dxc` feature, whose build downloads a DXC of 2023 | University of Illinois/NCSA (LLVM's) |

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
| [Weston](https://gitlab.freedesktop.org/wayland/weston) | A Wayland compositor without screen in which the checks run the windowed samples and the game ([ADR-0013](adr/0013-windowed-smoke-test-in-weston.md)) | Xvfb, which only covers X11 | MIT |

[Blender](https://www.blender.org) 5.2 is not part of the shell: it builds the examples' sample
scene (`examples/assets/make_scene.py`) and *Hunna Wood*'s zone (`games/hunnawood/zone/`) from
scripts, and is needed only to change them. Its licence, GPL-2.0-or-later, does not reach what it
exports; the alternative, scripting the zone without a modelling tool, would leave out the
libraries' models and the glTF exporter.

`curl` and `sha256sum` download and check the sample game's source assets and DXC
(`scripts/fetch-assets.sh`, `scripts/fetch-dxc.sh`). They are the system's, outside the shell; Git
for Windows provides them on Windows. `scripts/fetch-dxc.sh` unpacks DXC's archive with PowerShell,
which Windows provides.

## CI actions

Used by [`.github/workflows/ci.yml`](../.github/workflows/ci.yml), pinned to commit SHAs.

| Action | Why | License |
|---|---|---|
| [actions/checkout](https://github.com/actions/checkout) | Checks out the repository | MIT |
| [cachix/install-nix-action](https://github.com/cachix/install-nix-action) | Installs Nix with flakes enabled, so CI runs the same development shell as contributors | Apache-2.0 |
| [actions/upload-artifact](https://github.com/actions/upload-artifact) | Publishes the examples and the game built by the `windows-release` job, to run by hand on a Windows machine, and, when the `windows` job fails, the images that failing image tests rendered on WARP | MIT |
| [actions/cache](https://github.com/actions/cache) | Keeps the files of the sample game's zone that `scripts/fetch-assets.sh` fetched, from one run of each job to the next, while their list is unchanged ([ADR-0070](adr/0070-the-sample-games-source-assets.md)), and the zone cooked, while what makes its bytes is unchanged ([ADR-0104](adr/0104-the-build-ci-and-measurements.md)) | MIT |

The Windows jobs use the Rust toolchain manager (rustup) and the Visual Studio build tools that the
`windows-2025` runner image provides; rustup installs the toolchain of `rust-toolchain.toml`.
