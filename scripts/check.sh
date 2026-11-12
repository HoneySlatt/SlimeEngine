#!/usr/bin/env bash
# Runs every automated check of the definition of done (AGENTS.md); CONTRIBUTING.md, "Checks", says
# what they are. CI runs it unchanged, inside the Nix development shell, in four jobs side by side
# (ADR-0123, ADR-0155, ADR-0228): `nix develop --no-update-lock-file -c scripts/check.sh lint`, the
# checks that read the code without running it, formatting, clippy, rustdoc, the dependencies and
# the documentation's links, `... scripts/check.sh headless`, the build, the tests and every run
# that needs neither a compositor nor the sample game's zone, `... scripts/check.sh windowed`, the
# examples and the windowed lifecycle test run in headless Weston compositors, and
# `... scripts/check.sh game`, everything that reads the sample game's zone, which it fetches and
# cooks first (the flag makes an incomplete flake.lock an error instead of a silent update). Without
# an argument, it runs the four parts, one after the other. `docs` checks the links between
# Markdown files and the lengths of AGENTS.md and docs/PLAN.md alone, which the lint part does too:
# CI's `docs` workflow runs it, with lychee alone, on a change of Markdown files outside the crates
# (ADR-0188).
set -euo pipefail
cd "$(dirname "$0")/.."

part=${1:-all}
case "$part" in
    all | lint | headless | windowed | game | docs) ;;
    *)
        echo "usage: scripts/check.sh [lint|headless|windowed|game|docs]" >&2
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
    step "The game built for shipping holds neither the development channel, the MCP server nor the dev overlay (ADR-0122, ADR-0152)"
    if cargo tree --locked -p hunnawood --no-default-features -e normal | grep -E 'slime-(channel|mcp|devui) '; then
        echo "the game built for shipping depends on the crates above"
        exit 1
    fi
    # The interface itself is slime-app's, in every build, and so is uwui-render, which its bridge
    # draws the interface with (ADR-0244); the editor's look and its fonts are the overlay's.
    if cargo tree --locked -p hunnawood --no-default-features -e features | grep 'uwui feature "editor-look"'; then
        echo "the game built for shipping holds the editor's look"
        exit 1
    fi
    # The next `cargo build` or `cargo run` that names the game builds it again with its default
    # features.
    cargo build --locked -p hunnawood --no-default-features
    if grep -a -q -e SLIME_DEV_TOKEN -e slime-dev-channel target/debug/hunnawood; then
        echo "the game built for shipping holds the development channel"
        exit 1
    fi

    step "The game holds no adapter for screen readers, which games leave out until Stage 7 (ADR-0146)"
    if cargo tree --locked -p hunnawood -e normal | grep -E 'accesskit_winit '; then
        echo "the game depends on AccessKit's adapter"
        exit 1
    fi
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
