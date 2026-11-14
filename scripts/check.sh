#!/usr/bin/env bash
# Runs every automated check of the definition of done (AGENTS.md); CONTRIBUTING.md, "Checks", says
# what they are. CI runs it unchanged, inside the Nix development shell, in five jobs side by side
# (ADR-0123, ADR-0155, ADR-0228, ADR-0247): `nix develop --no-update-lock-file -c scripts/check.sh lint`, the
# checks that read the code without running it, formatting, clippy, rustdoc, the dependencies and
# the documentation's links, `... scripts/check.sh headless`, the build, the tests and every run
# that needs neither a compositor nor the sample game's zone, `... scripts/check.sh windowed`, the
# examples and the windowed lifecycle test run in headless Weston compositors, and
# `... scripts/check.sh game`, everything that reads the sample game's zone, which it fetches and
# cooks first (the flag makes an incomplete flake.lock an error instead of a silent update), and
# `... scripts/check.sh gles`, uwui-render's canvas on wgpu's GL backend, which nothing else builds
# (ADR-0247). Without an argument, it runs the five parts, one after the other. `docs` checks the
# links between Markdown files and the lengths of AGENTS.md and docs/PLAN.md alone, which the lint
# part does too: CI's `docs` workflow runs it, with lychee alone, on a change of Markdown files
# outside the crates (ADR-0188).
set -euo pipefail
cd "$(dirname "$0")/.."

part=${1:-all}
case "$part" in
    all | lint | headless | windowed | game | gles | docs) ;;
    *)
        echo "usage: scripts/check.sh [lint|headless|windowed|game|gles|docs]" >&2
        exit 2
        ;;
esac
# The Nix development shell sets this variable (flake.nix). Without the shell, the GPU tests and the
# samples would fail only after minutes of other checks.
[ "$part" = docs ] || : "${SLIME_LAVAPIPE_ICD:?run this script inside nix develop}"
# The checks render on lavapipe, which VK_DRIVER_FILES names: a choice of drivers that the user's
# environment makes, as `VK_LOADER_DRIVERS_SELECT=*radeon*`, would hide it from the Vulkan loader.
unset VK_LOADER_DRIVERS_SELECT VK_LOADER_DRIVERS_DISABLE

# Whether the part named runs.
runs() { [ "$part" = all ] || [ "$part" = "$1" ]; }

step() { printf '\n==> %s\n' "$*"; }

# The sample, cooked from empty directories by the headless part, and drawn by the windowed part.
cooked=target/check-cooked

# The windowed runs go into headless Weston compositors. A private runtime directory keeps them apart
# from the one the developer may be using.
runtime=$(mktemp -d)
westons=()
# `kill` fails when a Weston has already exited; that must not skip the rest of the cleanup.
trap 'kill "${westons[@]}" 2>/dev/null || true; rm -rf "$runtime"' EXIT
# Starts a headless Weston on the socket named by the first argument, with the options that follow,
# and waits until it accepts clients. Its log goes to `$runtime/<socket>.log`.
start_weston() {
    local socket=$1
    shift
    XDG_RUNTIME_DIR=$runtime weston --backend=headless --socket="$socket" --idle-time=0 "$@" \
        >"$runtime/$socket.log" 2>&1 &
    westons+=("$!")
    for _ in $(seq 100); do
        [ -S "$runtime/$socket" ] && return
        sleep 0.1
    done
    echo "Weston did not start:"
    cat "$runtime/$socket.log"
    exit 1
}

if runs lint; then
    step "Formatting"
    cargo fmt --all --check

    step "Clippy"
    cargo clippy --workspace --all-targets --locked -- -D warnings
    # `--all-targets` leaves out the test targets marked `test = false`, so they are named here, in a
    # separate command: cargo ignores `--test` when `--all-targets` is given.
    cargo clippy --locked -p slime-app --test windowed_lifecycle -- -D warnings
    cargo clippy --locked -p slime-mcp --test drive -- -D warnings
    cargo clippy --locked -p slime-mcp --test views -- -D warnings
    cargo clippy --locked -p slime-mcp --test interface -- -D warnings
    cargo clippy --locked -p slime-mcp --test widgets -- -D warnings
    cargo clippy --locked -p slime-mcp --test edit -- -D warnings
    cargo clippy --locked -p slime-mcp --test edit_window -- -D warnings
    cargo clippy --locked -p slime-mcp --test edit_zone -- -D warnings
    cargo clippy --locked -p slime-mcp --test resources -- -D warnings
    cargo clippy --locked -p slime-mcp --test compositor -- -D warnings
    # The game built for shipping, without the development channel (ADR-0122): the code that stands in
    # for the channel is compiled only there.
    cargo clippy --locked -p hunnawood --no-default-features --all-targets -- -D warnings

    step "Clippy for Windows (compiles the Windows code paths without a Windows machine)"
    cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-app --test windowed_lifecycle --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test drive --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test views --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test interface --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test widgets --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test edit --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test edit_window --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test edit_zone --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test resources --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test compositor --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p hunnawood --no-default-features --all-targets --target x86_64-pc-windows-msvc -- -D warnings

    step "Rustdoc"
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
    # The editor's look, which only the examples' dev-dependency turns on, and so the workspace's
    # documentation leaves out (ADR-0148).
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked -p uwui --features editor-look

    step "Dependencies (licenses, advisories, sources)"
    cargo deny --locked check
fi

if runs lint; then
    step "Only slime-app's bridge and the engine's tests open slime-gfx's door to wgpu (ADR-0244)"
    # The `wgpu` feature is on wherever slime-app is, in every game: this check, not the feature,
    # keeps the door to slime-gfx itself, the bridge (crates/slime-app/src/drawing.rs) and the
    # engine's tests, the `tests` folders of its crates. Any mention counts, a comment's too.
    if git grep -n -E 'with_wgpu|WgpuLoan|EngineObjects' -- '*.rs' \
        ':!crates/slime-gfx/' ':!crates/slime-app/src/drawing.rs' ':(glob,exclude)crates/*/tests/**'; then
        echo "the lines above open slime-gfx's door: only slime-app's bridge and the engine's tests may"
        exit 1
    fi
    # uwui-render names no engine crate but slime-color, which leaves with it; uwui's own links to
    # the engine leave at M6d.11 (docs/stage-6d.md). The tree is read first, so that a failure of
    # cargo stops the check rather than passing it.
    uwui_render_dependencies=$(cargo tree --locked -p uwui-render -e normal --depth 1 --prefix none)
    if grep -E '^slime-' <<< "$uwui_render_dependencies" | grep -v '^slime-color '; then
        echo "uwui-render depends on the engine crates above"
        exit 1
    fi
    # uwui-accesskit names no engine crate either, its tests none but slime-color, the color of a
    # color well, which leaves with uwui (ADR-0262).
    uwui_accesskit_dependencies=$(cargo tree --locked -p uwui-accesskit -e normal,dev --target all \
        --depth 1 --prefix none)
    if grep -E '^slime-' <<< "$uwui_accesskit_dependencies" | grep -v '^slime-color '; then
        echo "uwui-accesskit depends on the engine crates above"
        exit 1
    fi
    # Each crate under MIT OR Apache-2.0 holds both licences' texts, and LICENSE.md names it among
    # those that the engine's notice does not cover, or LICENSE.md would leave it under that notice.
    licensed_crates=0
    for manifest in crates/*/Cargo.toml; do
        grep -q '^license = "MIT OR Apache-2.0"$' "$manifest" || continue
        licensed_crates=$((licensed_crates + 1))
        crate=$(dirname "$manifest")
        for file in LICENSE-MIT LICENSE-APACHE; do
            [ -f "$crate/$file" ] || { echo "$crate is under MIT OR Apache-2.0 but holds no $file"; exit 1; }
        done
        grep -q -F "\`$crate\`" LICENSE.md || {
            echo "LICENSE.md does not name $crate among the crates under MIT OR Apache-2.0"
            exit 1
        }
    done
    # uwui's own crate is among them: a manifest's line written otherwise would skip them all.
    if ! grep -q '^license = "MIT OR Apache-2.0"$' crates/uwui/Cargo.toml || [ "$licensed_crates" -eq 0 ]; then
        echo "no crate's manifest reads as MIT OR Apache-2.0: the check above would pass on any"
        exit 1
    fi
    # uwui reads its documents in KDL, without the engine's reflection: neither slime-reflect nor
    # slime-serialize is in its tree, its tests' included, on any platform and with any feature
    # (ADR-0241).
    uwui_dependencies=$(cargo tree --locked -p uwui -e normal,build,dev --target all --all-features \
        --prefix none)
    if grep -E '^slime-(reflect|serialize) ' <<< "$uwui_dependencies"; then
        echo "uwui depends on the engine's reflection through the crates above"
        exit 1
    fi
    # uwui's core for agents depends on no other crate of uwui's nor of the engine's, so that a tool
    # speaks the protocol without building uwui's text and layout (ADR-0246).
    uwui_agent_dependencies=$(cargo tree --locked -p uwui-agent -e normal --target all \
        --all-features --prefix none)
    if grep -E '^(uwui|slime)-' <<< "$uwui_agent_dependencies" | grep -v '^uwui-agent '; then
        echo "uwui-agent depends on the crates above"
        exit 1
    fi

    step "Nothing in the workspace builds wgpu's GL backend, which uwui-render's gles feature alone turns on (ADR-0247)"
    # glow, the GL bindings that the backend alone brings, stands for it in the tree.
    workspace_dependencies=$(cargo tree --locked --workspace -e normal,build,dev --target all \
        --prefix none)
    if grep '^glow ' <<< "$workspace_dependencies"; then
        echo "the workspace builds wgpu's GL backend: only scripts/check.sh gles may"
        exit 1
    fi
fi

if runs headless; then
    step "Build"
    cargo build --workspace --all-targets --locked

    step "Tests (GPU tests on lavapipe, the Vulkan driver that renders on the CPU, as in CI)"
    VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test --workspace --locked
    # glam checks the preconditions of its functions only when its assertions are on, which a game
    # that turns them on also turns on in the engine (feature unification): the renderer and the
    # world must keep them.
    VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test --locked -p slime-render -p slime-world --features glam/debug-glam-assert

    step "Source assets (each with a metadata file of its own, ADR-0069)"
    cargo run --locked -p slime-cook -- check examples/assets

    step "Cooking the sample twice, from empty directories, gives the same bytes; a third cook cooks nothing"
    rm -rf "$cooked"
    cargo run --locked -p slime-cook -- cook examples/assets "$cooked/first"
    cargo run --locked -p slime-cook -- cook examples/assets "$cooked/second"
    diff -r "$cooked/first" "$cooked/second"
    cargo run --locked -p slime-cook -- cook examples/assets "$cooked/first" | grep -q ': 0 assets cooked'

    step "Headless samples (no display server)"
    env -u WAYLAND_DISPLAY -u DISPLAY cargo run --locked --example headless -- --frames 600
    env -u WAYLAND_DISPLAY -u DISPLAY cargo run --locked --example window -- --headless --frames 600
    # The world example's world runs without a window; it fails if its ring did not float.
    env -u WAYLAND_DISPLAY -u DISPLAY \
        cargo run --locked --example world -- --assets "$cooked/first" --headless --frames 600
    env -u WAYLAND_DISPLAY -u DISPLAY VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        cargo run --locked --example offscreen -- --output target/triangle.png
    # An interface built as a tree of elements, styled by the sample's style sheet and theme cooked
    # above, drawn into an image, its text in eight writing systems (ADR-0139, ADR-0140, ADR-0141).
    env -u WAYLAND_DISPLAY -u DISPLAY VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        cargo run --locked --example interface -- --assets "$cooked/first" --output target/interface.png
    # The widgets of a panel, in the editor's look, drawn into an image (ADR-0148).
    env -u WAYLAND_DISPLAY -u DISPLAY VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        cargo run --locked --example widgets -- --output target/widgets.png
    # The interface's text, laid out and rasterized by the CPU into an image (ADR-0136).
    cargo run --locked --example text -- --output target/text.png
    # The editor's window on the yard, drawn into an image without a window, its journal one of the
    # check's own (ADR-0156).
    rm -f target/editor-check.journal
    env -u WAYLAND_DISPLAY -u DISPLAY VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        cargo run --locked --example editor -- --assets "$cooked/first" \
        --journal target/editor-check.journal --output target/editor.png
    # An agent's session through the MCP server edits a copy of the yard with the `editor` example,
    # run without its window: steps made, undone and redone, some by the operations' names, the
    # level written back and opened again, a step not written recovered from the journal, and the
    # yard played in the `world` example without its window (ADR-0135, ADR-0156, ADR-0164).
    cargo build --locked --example editor --example world
    env -u WAYLAND_DISPLAY -u DISPLAY \
        timeout -k 10 300 cargo test --locked -p slime-mcp --test edit -- "$cooked/first"
fi

if runs windowed; then
    if [ "$part" = windowed ]; then
        # What the headless part cooks, when it does not run first.
        step "What the windowed runs draw: the sample, cooked"
        cargo run --locked -p slime-cook -- cook examples/assets "$cooked/first"
    fi

    step "Windowed samples and the lifecycle test in headless Wayland compositors (Weston), drawn with lavapipe"
    # Built first, with the features of the runs below, so that `timeout` bounds the runs alone: a
    # build from nothing, its dependencies optimised (ADR-0221), takes longer on CI's runners.
    cargo build --locked --examples
    cargo test --locked --no-run -p slime-mcp --test interface --test widgets --test edit_window --test views
    cargo test --locked --no-run -p slime-app --test windowed_lifecycle
    start_weston slime-check
    # Each run takes a few seconds; `timeout` ends a stalled one, with SIGTERM, which stops the engine
    # cleanly, then SIGKILL if that is not enough, and the failure then prints Weston's log.
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo run --locked --example window -- --frames 120 ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # The same interface over the window, which the canvas draws into without its sRGB encoding.
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo run --locked --example interface -- --assets "$cooked/first" --frames 120 ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # An agent's session through the MCP server finds a button of its bar by its role and its name,
    # presses it, reads the count that the press changed in its world, captures the button, and opens
    # and closes its dialog (ADR-0143).
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo test --locked -p slime-mcp --test interface -- "$cooked/first" ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # The widgets of a panel over the window, and an agent's session through the MCP server that uses
    # each of them through its actions (ADR-0148).
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo run --locked --example widgets -- --frames 120 ||
        { cat "$runtime/slime-check.log"; exit 1; }
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo test --locked -p slime-mcp --test widgets ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # The editor's window used by an agent's session through the MCP server: operations run by
    # name, its menu and its palette, a level opened by its path typed where no dialog of the
    # system opens, the first run's hint, and at rest, a panel floating, none of its threads waking
    # in 10 s (ADR-0156, ADR-0157, ADR-0185). Without the developer's session bus of D-Bus, which
    # its adapter for screen readers would reach, the editor at rest is the one that CI runs.
    cargo build --locked --example editor
    env -u DISPLAY -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo test --locked -p slime-mcp --test edit_window -- "$cooked/first" ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # The editor's start as scripts/start-times.sh times it: on a copy of the yard, with a journal
    # and a state of its own, it presents its first image and quits after its first frame, its log saying when,
    # since the start; then, run again until its log says that it drew its level whole, it is
    # stopped by its pid (ADR-0189).
    cp examples/assets/yard.scene examples/assets/yard.scene.meta "$runtime/"
    editor=(env -u DISPLAY -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check
        XDG_STATE_HOME="$runtime/state" XDG_DATA_HOME="$runtime/data" VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"
        target/debug/examples/editor --level "$runtime/yard.scene" --assets "$cooked/first"
        --journal "$runtime/yard.journal")
    timeout -k 10 120 "${editor[@]}" --frames 1 2>"$runtime/editor.log" ||
        { cat "$runtime/editor.log" "$runtime/slime-check.log"; exit 1; }
    grep -q "the first image is presented" "$runtime/editor.log" || {
        echo "the editor's log does not say when it presented its first image"
        cat "$runtime/editor.log"
        exit 1
    }
    "${editor[@]}" 2>"$runtime/editor.log" &
    editor_pid=$!
    for _ in $(seq 600); do
        grep -q "the level is drawn whole" "$runtime/editor.log" && break
        kill -0 "$editor_pid" 2>/dev/null || break
        sleep 0.1
    done
    kill "$editor_pid" 2>/dev/null || true
    wait "$editor_pid" || true
    grep -q "the level is drawn whole" "$runtime/editor.log" || {
        echo "the editor's log does not say, within 60 s, when it drew its level whole"
        cat "$runtime/editor.log"
        exit 1
    }
    # A client of AT-SPI, the service that Orca reads, reads and uses the same example as a screen
    # reader does, on a bus of D-Bus of its own (ADR-0146, ADR-0147).
    cargo build --locked --example interface
    env -u DISPLAY -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 180 scripts/screen-reader-check.sh target/debug/examples/interface "$cooked/first" ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # The scene example draws the sample cooked above.
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo run --locked --example scene -- --assets "$cooked/first" --frames 120 ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # On a copy of the sample with one byte of a mesh changed, the first of its name, which is then not
    # UTF-8, it ends with an error, status 1, not a panic, status 101 (ADR-0096).
    damaged="$cooked/damaged"
    rm -rf "$damaged"
    cp -r "$cooked/first" "$damaged"
    mesh=$(find "$damaged" -name '*.mesh' | sort | head -n 1)
    printf '\377' | dd of="$mesh" bs=1 seek=12 conv=notrunc status=none
    status=0
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo run --locked --example scene -- --assets "$damaged" --frames 120 ||
        status=$?
    [ "$status" -eq 1 ] || {
        echo "the scene example ended with status $status on damaged cooked data, where an error gives 1"
        cat "$runtime/slime-check.log"
        exit 1
    }
    # The world example draws it from a world, and fails if no frame drew all of it.
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo run --locked --example world -- --assets "$cooked/first" --frames 120 ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # An agent's session through the MCP server captures each of its two views, the yard and the map
    # (ADR-0126), shows the map's normals, a setting of the view (ADR-0127), reads its reports
    # (ADR-0128) and picks an entity at a pixel (ADR-0130). The test needs a compositor, so it is
    # outside the default test run.
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo test --locked -p slime-mcp --test views -- "$cooked/first" ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # The windowed lifecycle test needs a compositor too, so it is outside the default test run
    # (ADR-0024). Its own refreshes at 240 Hz: on Wayland, the redraws that a program asks for wait for
    # the compositor's signal, which then comes every 4.2 ms, and its burst of redraws runs frames
    # less than 10 ms apart, ahead of their schedule (ADR-0187).
    start_weston slime-check-fast --refresh-rate=240000
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check-fast VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo test --locked -p slime-app --test windowed_lifecycle ||
        { cat "$runtime/slime-check-fast.log"; exit 1; }
    # Nothing resizes a window in that compositor once it exists. One at scale 2 reports its scale after
    # the first frame, and the window then doubles in size: the lifecycle test's `resized` and `oversize`
    # scenarios run there, alone (ADR-0024): the window resized, alone and while another thread submits
    # work, and doubled past the largest image that the device accepts.
    start_weston slime-check-scaled --scale=2
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check-scaled \
        SLIME_APP_TEST_SCALED=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo test --locked -p slime-app --test windowed_lifecycle ||
        { cat "$runtime/slime-check-scaled.log"; exit 1; }
fi

if runs game; then
    step "The sample game's list of source assets, and the files of it that are cooked (ADR-0070)"
    scripts/fetch-assets.sh zone/

    step "The sample game's source assets (each with a metadata file of its own, ADR-0069)"
    cargo run --locked -p slime-cook -- check games/hunnawood/assets/zone

    # A release build: a debug build of the cooker takes minutes to compress the zone's textures. A
    # cache may hold the zone cooked already, which is cooked again only where it changed.
    step "The sample game's zone, cooked, and the tests that read it"
    cargo run --locked --release -p slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood
    # Its editor's too, which writes the zone back with its comments (ADR-0135).
    VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test --locked -p hunnawood -p hunnawood-editor -- --ignored
    # An agent's session through the MCP server edits a copy of the zone with the game's editor, run
    # without its window: a component of the game changed, an entity created, undone, redone,
    # written back.
    cargo build --locked -p hunnawood-editor
    # Built first, so that `timeout` bounds the runs alone, here and in the game's runs below.
    cargo test --locked --no-run -p slime-mcp --test edit_zone --test drive --test edit
    cargo build --locked -p hunnawood
    env -u WAYLAND_DISPLAY -u DISPLAY timeout -k 10 300 cargo test --locked -p slime-mcp --test edit_zone

    step "The benchmarks' counters and ratios, on lavapipe (ADR-0186)"
    # Each scenario that gives counters runs once, its ratios of times in whole series, and its
    # counters must equal those that benches/counters.txt records; the zone's apart, which only
    # this part has cooked.
    env -u WAYLAND_DISPLAY -u DISPLAY VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        cargo run --locked -p slime-benches -- --counters
    env -u WAYLAND_DISPLAY -u DISPLAY VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        cargo run --locked -p slime-benches -- --counters editor.window.zone

    step "The game in a headless Wayland compositor (Weston), drawn with lavapipe"
    start_weston slime-check-game
    # The game (ADR-0055), by day and at night, each run a new one. A frame of its zone takes seconds
    # on lavapipe, most of them in the shadows of the sun or the moon through the leaves (docs/debt.md),
    # so each run draws one frame only. Its saves go to the private runtime directory,
    # never to the user's (ADR-0088).
    for time in "" --night; do
        env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check-game \
            VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
            timeout -k 10 300 cargo run --locked -p hunnawood -- --new --frames 1 ${time:+"$time"} ||
            { cat "$runtime/slime-check-game.log"; exit 1; }
    done
    # And once more, losing its GPU device after its first frame of the zone: it must open a new one,
    # load its zone again and draw it once more, or it fails (ADR-0075).
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check-game \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo run --locked -p hunnawood -- --new --frames 2 --lose-device-at 1 ||
        { cat "$runtime/slime-check-game.log"; exit 1; }
    # And resumed from a copy of the save committed as a fixture, which it must resume (ADR-0088).
    cp games/hunnawood/fixtures/save.toml "$runtime/save.toml"
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check-game \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo run --locked -p hunnawood -- --save "$runtime/save.toml" --frames 1 2>&1 |
        tee "$runtime/resumed.log" ||
        { cat "$runtime/slime-check-game.log"; exit 1; }
    grep -q "the run resumes from" "$runtime/resumed.log" ||
        { echo "the game did not resume the run of its save"; exit 1; }
    # And driven by an agent's session through the MCP server, which starts it with a token and
    # connects to its development channel: pauses it, draws its view without shadows, steps it,
    # reads and changes the hero, walks it with a scripted key, saves its run and loads it back, uses
    # its dev overlay, captures its image, finds it at rest while paused, then minimized, its game
    # time running, none of its threads waking in 10 s, and reads its log (ADR-0122, ADR-0125,
    # ADR-0152, ADR-0185, ADR-0187). The test needs a compositor, so it is outside the default test
    # run, as the lifecycle test is.
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check-game \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo test --locked -p slime-mcp --test drive ||
        { cat "$runtime/slime-check-game.log"; exit 1; }
fi

if runs headless; then
    step "The game built for shipping holds neither the development channel, the MCP server, the dev overlay nor uwui's answers to agents (ADR-0122, ADR-0152, ADR-0259)"
    # Each detector runs first on the game built with its default features, where it must find what
    # it looks for: a crate or a feature renamed, an output of cargo changed or a string moved would
    # otherwise leave a check that always passes. Each tree is read into a variable first, so that a
    # failure of cargo stops the check rather than passing it. The MCP server is a program that no
    # game links, so it has no such control.
    shipped_crates=(slime-channel slime-devui uwui-agent)
    # uwui's features are read in its inverted tree, the one that shows a feature that another
    # feature turns on, as `slime-app`'s `development` turns on `agents`. The editor's look and its
    # fonts are the overlay's; the interface itself is slime-app's, in every build, and so is
    # uwui-render, which its bridge draws the interface with (ADR-0244).
    shipped_features=(editor-look agents)
    # The channel's variable and announcement, a method of the interface's family, and a refusal
    # that uwui's answers write. A text of code that no call reaches, which the linker drops, is not
    # found: the trees find its crate and its feature then.
    shipped_strings=(SLIME_DEV_TOKEN slime-dev-channel interface/elements "is no offset: finite numbers are")
    game_binary="${CARGO_TARGET_DIR:-target}/debug/hunnawood"
    # This part's build of the workspace above built the game with its default features, and more
    # that other crates turn on; the trees read the game's own.
    [ -f "$game_binary" ] || { echo "$game_binary is missing: the game was not built"; exit 1; }
    for build in development shipping; do
        flags=()
        if [ "$build" = shipping ]; then
            flags=(--no-default-features)
            # The next `cargo build` or `cargo run` that names the game builds it again with its
            # default features.
            cargo build --locked -p hunnawood "${flags[@]}"
        fi
        crates=$(cargo tree --locked -p hunnawood "${flags[@]}" -e normal --target all --prefix none)
        features=$(cargo tree --locked -p hunnawood "${flags[@]}" -e features --target all -i uwui \
            --prefix none)
        found=()
        for crate in "${shipped_crates[@]}"; do
            if grep -q "^$crate " <<< "$crates"; then found+=("crate $crate"); fi
        done
        if [ "$build" = shipping ] && grep -q '^slime-mcp ' <<< "$crates"; then
            found+=("crate slime-mcp")
        fi
        for feature in "${shipped_features[@]}"; do
            if grep -q "^uwui feature \"$feature\"" <<< "$features"; then found+=("uwui's feature $feature"); fi
        done
        for text in "${shipped_strings[@]}"; do
            if grep -a -q -F -e "$text" "$game_binary"; then found+=("the text \"$text\""); fi
        done
        if [ "$build" = development ]; then
            wanted=$((${#shipped_crates[@]} + ${#shipped_features[@]} + ${#shipped_strings[@]}))
            if [ "${#found[@]}" -ne "$wanted" ]; then
                printf '%s\n' "${found[@]}"
                echo "the check finds only the above in the game built with its default features: it would pass on any build"
                exit 1
            fi
        elif [ "${#found[@]}" -ne 0 ]; then
            printf '%s\n' "${found[@]}"
            echo "the game built for shipping holds the above"
            exit 1
        fi
    done

    step "The game holds no adapter for screen readers nor uwui-accesskit, which games leave out until Stage 7 (ADR-0146, ADR-0261)"
    # The game with slime-app's `accessibility` feature turned on holds both crates, or the search
    # below would pass on any build. Each tree is read first, so that a failure of cargo stops the
    # check rather than passing it.
    screen_reader_crates=(accesskit_winit uwui-accesskit)
    control_crates=$(cargo tree --locked -p hunnawood --features=default,slime-app/accessibility \
        -e normal --target all --prefix none)
    for crate in "${screen_reader_crates[@]}"; do
        if ! grep -q "^$crate " <<< "$control_crates"; then
            echo "the game with slime-app's accessibility holds no $crate: the check would pass on any build"
            exit 1
        fi
    done
    for flags in --features=default --no-default-features; do
        game_crates=$(cargo tree --locked -p hunnawood "$flags" -e normal --target all --prefix none)
        for crate in "${screen_reader_crates[@]}"; do
            if grep -q "^$crate " <<< "$game_crates"; then
                echo "the game built with $flags depends on $crate"
                exit 1
            fi
        done
    done
fi

if runs gles; then
    step "uwui-render's gles feature, wgpu's GL backend, on Linux and Windows"
    cargo clippy --locked -p uwui-render --all-targets --features gles -- -D warnings
    cargo clippy --locked -p uwui-render --all-targets --features gles --target x86_64-pc-windows-msvc -- -D warnings

    # wgpu asks EGL for desktop GL 3.3 first: Mesa, told to offer desktop GL up to 3.2 alone,
    # refuses it, and wgpu falls back to OpenGL ES 3.0, whose shaders are WebGL2's; the tests
    # refuse any other context. glvnd's loader of EGL and Mesa's driver of it are named, which a
    # runner outside NixOS lacks (flake.nix).
    mesa_gles=(
        LD_LIBRARY_PATH="$SLIME_GLVND:$LD_LIBRARY_PATH" __EGL_VENDOR_LIBRARY_FILENAMES="$SLIME_MESA_EGL"
        LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe
        MESA_GL_VERSION_OVERRIDE=3.2 MESA_GLES_VERSION_OVERRIDE=3.0
    )

    step "uwui-render's tests on lavapipe at WebGL2's limits, its canvas reading from a texture (ADR-0243)"
    # The device gives no storage buffer; the test that compares OpenGL ES with lavapipe runs too.
    env -u WAYLAND_DISPLAY -u DISPLAY "${mesa_gles[@]}" \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" UWUI_RENDER_LIMITS=webgl2 \
        cargo test --locked -p uwui-render --features gles

    step "uwui-render's tests on Mesa's OpenGL ES 3.0 on the CPU, at WebGL2's limits, and compared with lavapipe's (ADR-0247)"
    env -u WAYLAND_DISPLAY -u DISPLAY "${mesa_gles[@]}" \
        WGPU_BACKEND=gl UWUI_RENDER_LIMITS=webgl2 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        cargo test --locked -p uwui-render --features gles
fi

if runs lint || [ "$part" = docs ]; then
    step "Documentation links"
    # Tracked and new, non-ignored Markdown files. External links are not checked: they fail for
    # reasons unrelated to the change being checked.
    if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
        mapfile -t markdown < <(git ls-files --cached --others --exclude-standard '*.md')
    else
        # Outside a git checkout, such as an archive of a release: every Markdown file but the build's.
        mapfile -t markdown < <(find . -name '*.md' -not -path './target/*' -not -path './.git/*')
    fi
    lychee --offline --include-fragments --no-progress "${markdown[@]}"

    step "The files that every session reads, within their lengths"
    # Every agent reads them at each session's start (AGENTS.md): their limits keep that cost low.
    for file_and_limit in "AGENTS.md 1500" "docs/PLAN.md 2000"; do
        read -r file limit <<< "$file_and_limit"
        words=$(wc -w < "$file")
        if [ "$words" -gt "$limit" ]; then
            echo "$file has $words words, more than $limit: move what is not status or a rule to its home (AGENTS.md, \"One home per fact\")"
            exit 1
        fi
    done
fi

step "All checks passed"
