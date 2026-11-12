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
dependency licenses and security advisories), `lychee` (checks links in the documentation),
`hyperfine` (times the programs' starts), `heaptrack` (records a program's heap over its run),
`brotli` (compresses a build for the web, to weigh it), mold (links what is built for Linux, which the shell sets through
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS`, [ADR-0221](docs/adr/0221-compiling-within-its-budgets.md)),
sway and `wlrctl` (a compositor without screen that takes virtual input, and the tool that gives
it) and Weston (a Wayland compositor without screen, in which the checks run the windowed samples
and the games). It also puts the windowing libraries that the
engine loads at run time on the loader path.
[`flake.lock`](flake.lock) pins every input, so every contributor and CI get the same versions.
To run a single command in that environment without opening a shell, use
`nix develop -c <command>`.

Nix only sees the files that git tracks. If you change `flake.nix` so that it reads a new file,
`git add` that file before running `nix develop`.

### On Linux, without Nix

Install [rustup](https://rustup.rs). It reads `rust-toolchain.toml` and installs the pinned
toolchain the first time you run `cargo` in the repository. Install the tools with
`cargo install --locked cargo-deny lychee hyperfine`, and Weston, sway, wlrctl, heaptrack and brotli
from your distribution's packages; CI uses the versions pinned by `flake.lock`. To open windows, the
examples need the windowing system's libraries, which are loaded at run time: libxkbcommon for both
windowing systems; libwayland-client for Wayland; libX11, libX11-xcb, libXcursor, libXi, libxcb and libxkbcommon-x11
for X11. Without libxkbcommon, or libxkbcommon-x11 on X11, the examples panic instead of
reporting that the windowing system cannot be reached. A program that asks for the system's fonts
loads libfontconfig, and finds none without it. The editor's dialog to open a level loads
libdbus-1, through which it asks the desktop's portal, or runs `zenity`; without either, the editor
waits for the level's path typed. Most desktop distributions install them already.
Without the shell, cargo links with rustc's own lld rather than mold: to build as the shell does,
install mold and set `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=mold"`.
cargo keeps apart what it builds with each set of rustc's flags: going from the shell to a build
without it builds the workspace once more, beside the first. Inside the shell, that variable takes
the place of a `build.rustflags` of one's own cargo configuration; a `RUSTFLAGS` takes the place
of both.

### On Windows

Install [rustup](https://rustup.rs) and the Visual Studio Build Tools with the "Desktop development
with C++" workload, which provides the MSVC compiler and linker and the Windows SDK. rustup reads
`rust-toolchain.toml` like on Linux. `scripts/check.sh` needs Linux and Nix: on Windows, run the
cargo commands listed in [Commands](#commands) instead.

### Commands

The project's commands, run at the repository's root. Their comments say what each does and where
its details live; [Checks](#checks) says what `scripts/check.sh` runs.

```sh
nix develop                                               # enter the dev shell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
scripts/check.sh                                          # everything above, plus the checks below
cargo run -p slime-cook -- cook examples/assets target/cooked/examples  # cook what the scene, world and interface examples read
cargo run --release -p slime-cook -- watch <sources> <output>  # cook again each change (ADR-0076)
cargo run --example <name>                                # run an example (list in examples/README.md)
cargo run --release -p slime-benches -- [<name>...]       # run the benchmarks (benches/README.md)
taskset -c 0-3 cargo run --release -p slime-benches -- [<name>...]  # the CPU benchmarks on 4 cores, the handheld tier's proxy (ADR-0175)
cargo run -p slime-benches -- --counters [<name>...]      # check the benchmarks' counters and ratios, as CI does, on lavapipe (ADR-0186)
cargo run -p slime-benches -- --write-counters [<name>...]  # write the counters that a change changed on purpose into benches/counters.txt
scripts/measure.sh "<title>" >> docs/measurements.md      # time the build, run the benchmarks (ADR-0104)
scripts/compile-times.sh [--runs <count>] [--cold-runs <count>] [--shipping-runs <count>]  # time the steps of compiling that the budgets name, on a copy of the working tree (ADR-0221)
scripts/frame-pacing.py <trace.json>                      # the frames' pacing and the inputs to present in a trace written with SLIME_TRACE (ADR-0187, ADR-0189)
scripts/start-times.sh [--gpu] [--cold] [--runs <count>]  # time the editor's and the game's starts in a headless Weston (ADR-0189)
scripts/input-to-present.sh [--gpu]                       # measure their input to present, a person's input given in a headless sway (ADR-0191)
nix develop .#compositors -c scripts/compositors.sh weston|sway|kwin|mutter|x11  # run the editor and the game in a headless compositor of the series, against docs/compositors.md (ADR-0195, ADR-0240)
scripts/resources.sh [--gpu] [--heaptrack] [--undos <count>] [--plays <count>]  # measure the editor's and the game's memory, video memory and the GPU's power in a headless Weston (ADR-0193)
scripts/canvas-cost.sh [--gpu] [--runs <count>]           # measure what the renderer costs an interface against the canvas alone (ADR-0180)
scripts/uwui-program.sh [--features <list>]               # measure what a program on uwui compiles: crates, build times, binary (ADR-0180)
scripts/fetch-assets.sh                                   # fetch the game's source assets (games/hunnawood/README.md)
blender --background --factory-startup --python games/hunnawood/zone/build_zone.py  # rebuild the game's zone (same README)
cargo run -p hunnawood                                    # play the game (games/hunnawood/README.md)
cargo run -p hunnawood -- --watch                         # load its zone and the shaders again as they change
cargo run -p hunnawood -- --new --save <file>             # play a new run, saved to <file>, not to the user's save
cargo build -p hunnawood --no-default-features           # the game without the development channel and the dev overlay (ADR-0122, ADR-0152)
cargo build --profile shipping -p hunnawood --no-default-features  # the game built for shipping, optimised across its crates (ADR-0221)
cargo run --example editor -- --level <a copy of a level> --journal <file>  # the editor's window on the examples' yard (ADR-0135, ADR-0156); --headless without a window
cargo run --example editor -- --journal <file> --output <file.png>  # the editor's window drawn into an image, without a window
cargo run -p hunnawood-editor -- --level <a copy of the zone> --journal <file>  # the game's editor on its zone, Play building and running `hunnawood` (games/hunnawood-editor/README.md)
cargo run --example text -- --output <file.png>           # the interface's text in several writing systems, rasterized by the CPU (ADR-0136)
cargo run --example interface -- --output <file.png>      # an interface built as a tree of elements, styled by a theme: text, boxes, images, a view (ADR-0140, ADR-0141)
cargo run --example widgets -- --output <file.png>        # the widgets of a panel in the editor's look: check boxes, sliders, sections, tabs, two panels, scroll bars (ADR-0148), menus, a list to choose from, tooltips, a floating panel (ADR-0150), rich fields, notifications, the palette of commands (ADR-0151), a color well (ADR-0152)
UWUI_KDL_SEED=<seed> cargo test -p uwui-kdl --test hostile  # replay the random inputs that failed uwui's reader of KDL, from the seed it printed (crates/uwui-kdl/README.md)
scripts/subset-fonts.sh <noto> <noto-cjk> <noto-emoji>    # make the fonts of uwui's tests again (crates/uwui/tests/fonts/README.md)
scripts/screen-reader-check.sh <the interface example's program> <cooked data>  # read and use it through AT-SPI, as a screen reader does, on a Wayland display (ADR-0147)
cargo run --quiet -p slime-mcp                            # the engine's MCP server, through which an agent drives a game (crates/slime-mcp/README.md)
```

`scripts/check.sh lint`, `headless`, `windowed` and `game` run one of its four parts, as CI's four
Linux jobs do ([Checks](#checks)); `scripts/check.sh docs` checks the links between Markdown files and
the lengths of `AGENTS.md` and `docs/PLAN.md` alone, without the development shell, as CI's
`docs` workflow does.

### GPU

The GPU layer uses Vulkan on Linux and Direct3D 12 on Windows
([ADR-0016](docs/adr/0016-gpu-layer-with-wgpu.md)).

- On Linux, the engine needs the Vulkan loader (`libvulkan`) and a Vulkan driver: Mesa's RADV
  (AMD), ANV (Intel) or NVK (NVIDIA), or NVIDIA's own driver. The Nix shell provides the loader.
- On Windows, Direct3D 12 is part of the system. Without a GPU, for example in a virtual machine,
  wgpu uses WARP, Microsoft's software driver. On WARP, run the tests one at a time
  (`cargo test --workspace -- --test-threads=1`), as the Windows CI job does: WARP's shader
  compiler crashes the process now and then when several threads compile shaders at once (see
  [Known debt](docs/debt.md)).
- On Windows, Direct3D 12 compiles shaders with DXC when its library, `dxcompiler.dll`, lies
  beside the program, and otherwise with FXC, part of Windows, whose code runs slower: a warning in
  the log says so ([ADR-0066](docs/adr/0066-shipping-dxc-beside-the-programs.md)). From Git Bash,
  `scripts/fetch-dxc.sh target/debug/deps target/debug/examples target/debug` downloads DXC and
  puts it beside the tests, the samples and the game that cargo builds, as the Windows CI job does
  for the tests and the samples.
  `WGPU_DX12_COMPILER=fxc` asks for FXC, and `WGPU_DX12_COMPILER=dxc` for DXC wherever Windows
  finds `dxcompiler.dll`.
- `scripts/check.sh` runs the tests and the samples on lavapipe, Mesa's Vulkan driver that renders
  on the CPU, so that they give the same results on every machine and in CI. To run the GPU tests
  on your own GPU, run `cargo test` without `VK_DRIVER_FILES`; to reproduce the checks, run
  `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test` inside `nix develop`. If your environment
  chooses Vulkan drivers, with `VK_LOADER_DRIVERS_SELECT` or `VK_LOADER_DRIVERS_DISABLE`, that
  choice hides lavapipe from the Vulkan loader: unset them, as the script does
  (`env -u VK_LOADER_DRIVERS_SELECT -u VK_LOADER_DRIVERS_DISABLE ...`). Without Nix, install
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

  The same writes again the fixtures of styles, `examples/tests/reference/*-styles.txt`, the
  fingerprints of the `widgets` example's styles and of the editor's window
  (`examples/common/style_fixture.rs`): a change of the look or of `uwui::style::Style` changes
  them on purpose; a change of the cascade must not
  ([ADR-0229](docs/adr/0229-styles-typed-as-declarations.md)).

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

### Writing and cooking a scene file

A scene file, `.scene`, is TOML that places entities and other scenes, glTF scenes or scene files,
as prefabs, each placement with values of its own
([ADR-0091](docs/adr/0091-scene-documents-and-placements.md)); the rustdoc of
`slime_world::SceneLibrary` lays out its keys. `examples/assets/yard.scene` places the sample scene
and four lamps of `lamp.scene`:

```toml
[assets]
lamp = { id = "6647a9c8-d378-4f9a-89d7-3ec9fc2b172a", path = "lamp.scene" }

[entities]
"lamp 2" = { prefab = "lamp" }

[changed."lamp 2".Transform]
translation = { x = -3.5, z = 3.5 }

[changed."lamp 2/light".Light.Point]
color = { r = 1.0, g = 0.1, b = 0.05 }
```

The table of assets names each asset that the file uses by a short name: its ID decides, which a
source's metadata file holds; its path, relative to the file's directory and written with `/`, is a
hint, and the cooker warns when it is stale. Values name entities by their paths from the file's
root, and assets by their short names. A new scene file gets its metadata file as any source does,
from `cargo run -p slime-cook -- scan <directory>` or from the first cook; `check` refuses a source
without one. The cooker reads its layout and refuses, naming the file and the line, an asset that
the sources do not make, a prefab that is not a scene, a parent missing, and scene files that place
each other ([ADR-0095](docs/adr/0095-cooked-scene-files.md)); a game checks the values when it
resolves the scene, through its types, and names then the entity, the component and the field at
fault, without the line, since it reads the cooked file; its tests resolve each of its scenes. A
Blender node gives a game's component as a custom property named `slime:<type name>`: a text that
holds the component's value in the same notation, on one line, `{ height = 2.6 }`, or a boolean or a
number for a tuple struct of one field, which the cooker keeps typed. A type past version 1 gives
the version that its value is written in in the node's property `slime:$versions`,
`{ "hunnawood::Collider" = 2 }`, as a scene file does under `$versions`: a type left out is at
version 1, and its value is brought through the type's upgrades.

### Changing assets while a game runs

`slime-cook watch` cooks a directory of sources again each time one of its files changes, and a
game started with `--watch` loads its scenes again when their cooked data changes, keeping what did
not change: a texture saved in an image editor, or a scene exported again from Blender, shows in the
running game shortly after ([ADR-0076](docs/adr/0076-hot-reload-of-cooked-data.md)). The
same option reads the renderer's shaders, in `crates/slime-render/src`, when they change: a shader
that compiles draws the next frame, and one that does not leaves the frames as they were, the log
naming its file, line and column ([ADR-0077](docs/adr/0077-hot-reload-of-shaders.md)).
In two terminals:

```sh
cargo run --release -p slime-cook -- watch games/hunnawood/assets/zone target/cooked/hunnawood
cargo run -p hunnawood -- --watch
```

The `scene` and `world` examples take `--watch` too, with `examples/assets` cooked into
`target/cooked/examples`. A change of the zone's layout starts the game's run again; a change of how
it looks does not ([`games/hunnawood/README.md`](games/hunnawood/README.md#playing)). Each change
leaves the cooked files it replaced in the directory of cooked data, and a watch stopped with Ctrl+C
the temporary files of its cook: delete the directory to clean it.

### Driving a running game

A game built with its default features holds the engine's development channel, when they turn on
`slime-app`'s `development` feature, as *Hunna Wood*'s do
([ADR-0122](docs/adr/0122-the-development-channel.md)): started with a token in its environment, it
listens on the machine's own address for the tools that drive it; started without one, it does not.
The engine's MCP server, `slime-mcp`, starts a game so, and gives an agent the tools to pause and
step it, read and change its world, play it with keys, save and load its run to files of the agent's
own, capture its image or one of its views, change how a view draws, pick the entity at a pixel,
read its reports and its log, and read and use its interface element by element, its fields,
sliders and handles given values ([ADR-0143](docs/adr/0143-what-agents-see-of-the-interface.md),
[ADR-0146](docs/adr/0146-screen-readers.md)); its
[README](crates/slime-mcp/README.md) says how to give it to an agent. The game opens its window on
the display that the server's environment names: to keep it off your desktop, give the server the
environment of a headless Weston, `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY`, with
`VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"` so that lavapipe draws it, as `scripts/check.sh` runs the
server's test. The editor's program is driven the same way: the server starts it with `start_game`,
as a game, on a copy of a level and with a journal of the agent's (`--level`, `--journal`), and its
editor's tools edit the level, `run_operation` running any operation of its menus by name. It opens
its window as a game does, in the server's environment; `--headless` runs it without one. Once its
Play runs the game, `drive_played_game` points the server's game tools at that game, the editor's
tools still reaching the editor ([ADR-0165](docs/adr/0165-editing-while-playing.md)).

A build for shipping leaves the channel and the dev overlay out: name the game alone, without its
default features, in the profile for shipping,
`cargo build --profile shipping -p hunnawood --no-default-features`. A build of the whole
workspace turns the channel on for every program, as cargo unifies the features of a build, through
`slime-editor`; one of its tests or examples (`cargo test --workspace`, `--all-targets`) turns on
the adapter of screen readers too, `slime-app`'s `accessibility` feature, which the examples turn
on and games leave out until Stage 7 ([ADR-0146](docs/adr/0146-screen-readers.md)).

### Screen readers

The examples give screen readers their windows' trees, through AccessKit's adapter: on Linux to the
desktop's AT-SPI service on the session bus, which Orca reads, and on Windows to UI Automation,
which Narrator reads. `scripts/screen-reader-check.sh`, which `scripts/check.sh` runs in a headless
Weston, reads and uses the `interface` example through AT-SPI, as Orca does, on a session bus of
its own ([ADR-0147](docs/adr/0147-the-screen-readers-check-in-ci.md)); Orca and Narrator themselves
are checked by hand by the owner, on the `interface` example and on the `world` example's dev
overlay ([`docs/PLAN.md`](docs/PLAN.md)). To try one, start Orca (`orca`, or the desktop's setting
of accessibility), then the `interface` example, or the `world` example, whose panels F2, F3 and
F6 reach. A
program costs nothing until a screen reader asks for its tree. With
`SLIME_LOG=info,zbus=warn,slime_app::accessibility=debug`, the log says each action that a screen
reader asks.

### Measuring

The engine's benchmarks are a program of the workspace, `slime-benches`; its
[README](benches/README.md) says what each measures and how its figures are read.
[`scripts/measure.sh`](scripts/measure.sh) runs them after timing the build, and prints a section
of [`docs/measurements.md`](docs/measurements.md), which keeps them from one milestone to the next
([ADR-0104](docs/adr/0104-the-build-ci-and-measurements.md)). A change that claims a gain shows the
figures before it and after it, measured on the same machine, driver and build:

```sh
cargo run --release -p slime-benches -- world        # the benchmarks whose names start with `world`
scripts/measure.sh "<milestone>, <GPU>" >> docs/measurements.md
```

[`scripts/compile-times.sh`](scripts/compile-times.sh) times the steps of compiling that the
quality bar's budgets name ([`docs/ROADMAP.md`](docs/ROADMAP.md#budgets)), at each milestone: the
workspace from nothing, a function's body changed in a game or an engine crate and built for an
editor and the game that its Play builds, a public function added, a game built for shipping. It
builds a copy of the working tree, prints a table of medians and ranges with the crates that each
step compiled, and is run in the development shell on a machine at rest
([ADR-0221](docs/adr/0221-compiling-within-its-budgets.md)).

To see where a frame's time goes, `SLIME_TRACE=<file>` makes a program that calls
`slime_app::init_diagnostics`, as the examples, the game and the benchmarks do, write a trace of
its spans, which [Perfetto](https://ui.perfetto.dev) opens in the browser, without sending the
file anywhere; the rustdoc of `slime_app::TRACE_ENV_VAR` says what it holds.
[`scripts/frame-pacing.py`](scripts/frame-pacing.py) reads such a trace and prints how the frames
were paced, their rate and the intervals between their starts
([ADR-0187](docs/adr/0187-a-hidden-window-at-rest.md)):

```sh
SLIME_TRACE=trace.json cargo run --release --example window -- --frames 600
scripts/frame-pacing.py trace.json
```

The same trace holds, on a track of its own, the interval from each input that a person gives the
window to the present of the image after it, which `scripts/frame-pacing.py` reads too, in frames
of the editor's budget of [input to present](docs/ROADMAP.md#budgets), or of a game's with
`--frame <milliseconds>` ([ADR-0189](docs/adr/0189-first-measures-of-time.md)).
[`scripts/input-to-present.sh`](scripts/input-to-present.sh) measures it for the editor and the
game in a headless sway whose output refreshes at 240 Hz, `wlrctl` giving them a person's input, on
a machine at rest. On a desktop, by hand, the editor runs on a copy of a level, with a journal and a
state directory of its own, so that it writes none of the user's keys, layout and recent levels:

```sh
cp examples/assets/yard.scene examples/assets/yard.scene.meta /tmp/
XDG_STATE_HOME=/tmp/editor-state XDG_DATA_HOME=/tmp/editor-data SLIME_TRACE=editor.json \
    cargo run --release --example editor -- \
    --level /tmp/yard.scene --journal /tmp/yard.journal
scripts/frame-pacing.py editor.json
```

[`scripts/start-times.sh`](scripts/start-times.sh) times the warm starts that the
[quality bar](docs/ROADMAP.md#4-quality-bar) budgets: the editor on the yard and the game's editor
on its zone, to their first image presented and to their level drawn whole, and *Hunna Wood* to its
zone drawn. `hyperfine` times each process whole and its launch alone, after three runs that fill
the system's caches; the programs' logs give the times since the engine's start
(`slime_app::since_start`); a start is the log's time to the first image plus the launch. It runs
them in a headless Weston, with lavapipe, or with `--gpu` on the machine's GPU through Weston's GL
renderer, on a machine at rest. `--cold` drops the system's caches before each run instead, through
`sudo`: a person runs it, never a check.

[`scripts/resources.sh`](scripts/resources.sh) measures the memory and the power that the quality
bar budgets ([ADR-0193](docs/adr/0193-first-measures-of-resources.md)): the editor on the game's
zone, its resident memory and its video memory at rest, then over the undos, redos and runs of Play
that the budget names; then the game at its schedule. The editor runs in a headless Weston of its
own, at the high tier's resolution, driven through the MCP server by
`crates/slime-mcp/tests/resources.rs`. With `--gpu`, it draws on the machine's GPU, whose driver
counts its video memory, and reads the GPU's power from the driver's sensor before anything starts,
with the editor at rest, with the game running and after; on a machine at rest, its desktop still.
With `--heaptrack`, the editor runs under `heaptrack`, and the script prints its heap at the end of
each phase: the resident size counts memory that the allocator keeps after the program freed it,
the heap does not. The recording stays in `target/resources/`, which `heaptrack_gui` opens to show
where the memory went. `--undos` and `--plays` measure longer series than the budget's.

[`scripts/canvas-cost.sh`](scripts/canvas-cost.sh) measures what the renderer costs an application
that draws only interfaces, against the canvas alone
([ADR-0180](docs/adr/0180-one-canvas-and-the-frameworks-foundations.md)): the time to build each
and draw a first canvas, the resident memory each adds, the GPU memory that wgpu allocates and the
video memory that the driver gives the process. Each part, and the device alone, runs in a process
of its own, a test of the benchmarks' `tests/canvas_cost.rs` that `cargo test` ignores; with
`--gpu`, on the machine's GPU. Nothing opens a window.
[`scripts/uwui-program.sh`](scripts/uwui-program.sh) builds the `widgets` example in a package of
its own, under `target/uwui-program`, with the workspace's lock: the crates that a program on uwui
compiles, whether the renderer, the world or the editor are among them, its builds from nothing
and after a change of its code, and the size of its binary.

A windowed run measured on the machine's GPU, kept off the desktop in a headless Weston, needs
Weston's GL renderer, `weston --backend=headless --renderer=gl`: with the default one, the GPU's
Vulkan driver (RADV, at least) cannot present to it, and wgpu takes the CPU driver instead without
failing. The log's `GPU adapter` line says which one the run took. The checks keep the default
renderer, as they draw with lavapipe.

Debug builds give the dependencies the tables that place a backtrace's frames in their source
only, not the rest of their debug information. To step through a dependency in a debugger, build
it with all of it:

```sh
cargo build --config 'profile.dev.package."*".debug = true'
```

## Checks

One script runs every automated check, exactly as CI does:

```sh
nix develop -c scripts/check.sh
```
[`scripts/check.sh`](scripts/check.sh) runs rustfmt, clippy with warnings as errors, the build and
tests of every target, the renderer's and the world's tests again with glam's assertions on
(`--features glam/debug-glam-assert`, which a game that turns them on also turns on in the engine),
rustdoc with warnings as errors, once more for `uwui` with its feature `editor-look`,
`cargo deny --locked check` (dependency licenses, security advisories and sources; it downloads the
advisory database, so it needs network access), the list of the sample game's source assets
(`scripts/fetch-assets.sh`), `slime-cook check` (every source asset has a metadata file of its own,
[ADR-0069](docs/adr/0069-the-asset-pipeline.md)), the sample's assets cooked twice from empty
directories and their bytes compared, then a third time, which must cook nothing, the sample game's
zone fetched and cooked, the tests that read it, *Hunna Wood*'s editor's included
(`cargo test -p hunnawood -p hunnawood-editor -- --ignored`), and an agent's session through the MCP
server that edits a copy of the zone with that editor, the samples with no display server
available, the `editor` example's window drawn into an image, and an agent's session that edits a
copy of the examples' yard with the `editor` example run without its window, the windowed samples,
the `interface` example's interface used by an agent's session through the MCP server, which
presses a button that changes its world and gives a field a value, and read and used through AT-SPI
as a screen reader does (`scripts/screen-reader-check.sh`,
[ADR-0147](docs/adr/0147-the-screen-readers-check-in-ci.md)), the `widgets` example drawn into an
image, run in a window and its widgets, menus, list to choose from and floating panel each used by
an agent's session through the MCP server ([ADR-0148](docs/adr/0148-widgets.md),
[ADR-0150](docs/adr/0150-layers.md)), the `editor` example's window used by an agent's session
through the MCP server, its operations run by name, its menu and its palette, a row of its
hierarchy dropped before another, an entity picked in its viewport, then locked and hidden by its
row's buttons, its second viewport captured, a panel moved to another region and a border between
two dragged, which must then be at rest, a panel floating, none of its threads waking in 10 s
([ADR-0156](docs/adr/0156-the-editors-window-and-its-operations.md),
[ADR-0185](docs/adr/0185-rest-checked-in-ci.md),
[ADR-0159](docs/adr/0159-picking-and-the-selection.md),
[ADR-0160](docs/adr/0160-two-viewports-and-blenders-way.md),
[ADR-0169](docs/adr/0169-the-eye-and-the-lock.md), [ADR-0172](docs/adr/0172-regions-of-tabs.md)),
the `editor` example's start, its log saying when it presented its first image, in a run that quits
after it, and when it drew its level whole, in a run stopped then
([ADR-0189](docs/adr/0189-first-measures-of-time.md)), the `world` example's two views captured,
the map's normals shown, its reports read and an entity
picked at a pixel, by an agent's session through the MCP server, the benchmarks' counters, which
must equal those that `benches/counters.txt` records, and their ratios of times, each within its
budget, in a debug build on lavapipe ([ADR-0186](docs/adr/0186-counters-and-ratios-in-ci.md)), the `scene` example on a copy of
the cooked sample with one byte changed, which must end with an error and not a panic, the game (its
new runs, one that loses its GPU device after its first frame and must go on, one resumed from a
copy of its save fixture, and one driven by an agent's session through the MCP server, which must
be at rest while paused, then minimized, its game time running
([ADR-0187](docs/adr/0187-a-hidden-window-at-rest.md)), their saves kept apart from the user's) and the windowed lifecycle test
inside a headless Weston compositor of its own, at 240 Hz (and that test's resize and oversize
scenarios in a second one, at scale 2), the game built for
shipping, which must hold neither the development channel nor the dev overlay
([ADR-0152](docs/adr/0152-the-dev-overlay-on-the-engines-interface.md)), the game, which must hold
no adapter for screen readers ([ADR-0146](docs/adr/0146-screen-readers.md)), `lychee` (links
between Markdown files), and the lengths of the two files that every agent reads at each
session's start: `AGENTS.md`, at most 1,500 words, and `docs/PLAN.md`, at most 2,000; what does
not fit goes to its home ([`AGENTS.md`](AGENTS.md#one-home-per-fact)).
[Commands](#commands) lists the individual commands, to run only one of them.

The script also runs clippy for the Windows target, which catches code that does not compile on
Windows without a Windows machine.

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs on every push to `main`, every
pull request, and every Monday at 06:17 UTC on `main`, without a push, but a push or a pull request
that changes Markdown files outside `crates/` alone: the `docs` workflow
([`.github/workflows/docs.yml`](.github/workflows/docs.yml)) then checks their links, with lychee
alone, in a minute or so ([ADR-0188](docs/adr/0188-a-light-check-for-documentation.md)). The
crates' READMEs are their rustdoc, whose code runs as tests: a change of them runs CI whole. The
weekly run runs CI whole: a project at rest still
learns of new advisories against its dependencies (`cargo deny`) and of changes to CI's
environment. A push during the weekly run supersedes it, as it supersedes any run of its branch.
GitHub turns the weekly run off after 60 days without activity in a public repository; a click in
the repository's Actions tab turns it on again. Four Linux jobs run the same script side by side, each one of its four parts
([ADR-0123](docs/adr/0123-the-linux-checks-in-two-jobs.md),
[ADR-0155](docs/adr/0155-the-editors-design.md),
[ADR-0228](docs/adr/0228-ci-split-again-each-job-within-20-minutes.md)): `scripts/check.sh lint`,
the checks that read the code without running it, formatting, clippy for Linux and for Windows,
rustdoc, the dependencies and the documentation's links, `scripts/check.sh headless`, the build,
the tests and every run that needs neither a compositor nor the sample game's zone,
`scripts/check.sh windowed`, the examples and
the windowed lifecycle test run in headless Weston compositors, which cooks first what they draw,
and `scripts/check.sh game`,
everything that reads the sample game's zone, which it fetches and cooks first: the tests that read
it, the agent's session with its editor, the benchmarks' counters, and the game's runs in a headless
Weston compositor; without
an argument, the script runs the four, one after the other. Four Windows jobs run beside them
([ADR-0104](docs/adr/0104-the-build-ci-and-measurements.md),
[ADR-0228](docs/adr/0228-ci-split-again-each-job-within-20-minutes.md)). Three are the parts of
`windows`: `windows (tests)` runs the workspace's tests but the renderer's, then the headless
samples; `windows (render)` runs the renderer's tests; `windows (game)` fetches and cooks the sample
game's zone and runs the tests that read it. They run the tests with `--nocapture`, so that the log
shows what the image tests, the GPU timing test and the multi-pass test measured on WARP, the
renderer's shaders made again and the device's limits, and one at a time (`--test-threads=1`):
WARP's shader compiler crashes the process, now and then, when several threads compile shaders at
once. They do not stop at the first test program that fails, and when a step fails, each publishes
the images that the failing image tests rendered on WARP, as an artifact named
`image-test-failures-windows-<part>`. The fourth, `windows-release`, runs clippy, builds in release and runs the headless samples, and publishes the
`hello`, `headless`, `window`, `offscreen`, `interface`, `widgets`, `scene` and `world` examples and
the `hunnawood` game built for Windows, with the sample cooked for `scene` and `world` and the zone
cooked for the game, as an artifact named `slime-examples-windows`, to run by hand on a Windows
machine. Each job that cooks the zone, `check (game)`, `windows (game)` and `windows-release`, keeps
it in its cache, keyed on what makes its bytes.
[ADR-0007](docs/adr/0007-documentation-tooling-and-ci.md) and
[ADR-0015](docs/adr/0015-platforms-and-ci.md) explain the setup.

The lints they enforce are set once for the whole workspace in [`Cargo.toml`](Cargo.toml);
[ADR-0005](docs/adr/0005-lints-and-unsafe-policy.md) explains them and the rules for `unsafe`
code.

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](docs/adr/0004-workspace-layout-and-crate-naming.md)). Tags and GitHub releases wait for the
owner's go.

1. In one commit: set the version in the root `Cargo.toml` and the tag that the README's "Using
   SlimeEngine in a game" names, with its list of crates; rename the `[Unreleased]` section of
   `CHANGELOG.md` to `[<version>] - <date>`; update [`docs/PLAN.md`](docs/PLAN.md) and [`docs/history.md`](docs/history.md).
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.
4. Create the GitHub release of the tag, its `CHANGELOG.md` section as its notes, with the
   `slime-examples-windows` artifact of that commit's CI run attached as
   `slime-examples-windows-<version>.zip`: CI keeps its artifacts 14 days, a release keeps them.
   Beside the programs, CI puts DXC's library and its licence, and the licenses of the fonts and
   icons that they embed: parts of Noto in `interface.exe`; Geist, Geist Mono and Lucide in
   `widgets.exe`, and in the dev overlay of `scene.exe`, `world.exe` and `hunnawood.exe`. Add
   `LICENSE.md` and a `NOTICE.txt`.

## Making a change

1. Keep each commit to one logical change, small enough to review. Write the subject in the
   imperative, in English; explain in the body why the change is needed when it is not obvious.
2. Update every document the change affects. [`AGENTS.md`](AGENTS.md#one-home-per-fact) says
   which document covers what: each fact has one home, which the others link to.
3. Do not commit code that fails the checks of the
   [definition of done](AGENTS.md#definition-of-done).
4. A source asset, a glTF, PNG, JPEG or scene file (`.scene`) that the cooker reads, is committed
   with its metadata file, `<file>.meta`, which holds its asset ID
   ([ADR-0069](docs/adr/0069-the-asset-pipeline.md)): `cargo run -p slime-cook -- scan <directory>`
   writes one for each new file. Move or rename the two together, so that the asset keeps its ID;
   a copied file needs its own, so do not copy the metadata file with it.
   The source assets that the sample game downloads are not committed: they are listed, with
   their authors and licences, in `games/hunnawood/assets/sources.tsv`, mirrored, and fetched by
   `scripts/fetch-assets.sh` ([ADR-0070](docs/adr/0070-the-sample-games-source-assets.md);
   [`games/hunnawood/README.md`](games/hunnawood/README.md#its-assets) says how to add one, and
   how to build the zone again with Blender).
5. Code that reads a file from outside the program, cooked data, the manifest, a save, a scene
   file, a source asset or its metadata file, follows
   [ADR-0096](docs/adr/0096-the-files-the-engine-reads.md): a damaged or edited file gives an
   error, never a panic, a hang or memory exhausted, and the reader's tests damage its input,
   through the helper that every crate's tests share, `crates/slime-assets/tests/common/damage.rs`
   ([ADR-0102](docs/adr/0102-a-damaged-file-gives-an-error.md)).

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

SlimeEngine is to be licensed under a licence of its own, with a royalty above a threshold;
[`LICENSE.md`](LICENSE.md) is its interim notice
([ADR-0214](docs/adr/0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)). Contributions of
code are not accepted before 0.14; from then, under a contributor licence agreement, by which the
contributor keeps the copyright and grants the owner the right to use, sell and license it again.
uwui's crates, `crates/uwui` and `crates/uwui-render`, are under MIT OR Apache-2.0, and take
contributions under both, without further terms. The examples are under MIT-0, code made to be
copied into games ([ADR-0219](docs/adr/0219-test-games-in-place-of-hunna-wood.md)).
