#!/usr/bin/env bash
# Runs every automated check of the definition of done (AGENTS.md). CI runs this script
# unchanged, inside the Nix development shell, in two jobs side by side, one for each of its parts
# (ADR-0123): `nix develop --no-update-lock-file -c scripts/check.sh headless`, every check that
# needs no compositor, and `... scripts/check.sh windowed`, the runs in headless Weston compositors
# (the flag makes an incomplete flake.lock an error instead of a silent update). Without an
# argument, it runs both parts, one after the other.
set -euo pipefail
cd "$(dirname "$0")/.."
# The Nix development shell sets this variable (flake.nix). Without the shell, the GPU tests and the
# samples would fail only after minutes of other checks.
: "${SLIME_LAVAPIPE_ICD:?run this script inside nix develop}"

part=${1:-all}
case "$part" in
    all | headless | windowed) ;;
    *)
        echo "usage: scripts/check.sh [headless|windowed]" >&2
        exit 2
        ;;
esac
# Whether the part named runs.
runs() { [ "$part" = all ] || [ "$part" = "$1" ]; }

step() { printf '\n==> %s\n' "$*"; }

# The sample, cooked from empty directories by the headless part, and drawn by both.
cooked=target/check-cooked

if runs headless; then
    step "Formatting"
    cargo fmt --all --check

    step "Clippy"
    cargo clippy --workspace --all-targets --locked -- -D warnings
    # `--all-targets` leaves out the test targets marked `test = false`, so they are named here, in a
    # separate command: cargo ignores `--test` when `--all-targets` is given.
    cargo clippy --locked -p slime-app --test windowed_lifecycle -- -D warnings
    cargo clippy --locked -p slime-mcp --test drive -- -D warnings
    # The game built for shipping, without the development channel (ADR-0122): the code that stands in
    # for the channel is compiled only there.
    cargo clippy --locked -p hunnawood --no-default-features --all-targets -- -D warnings

    step "Clippy for Windows (compiles the Windows code paths without a Windows machine)"
    cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-app --test windowed_lifecycle --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p slime-mcp --test drive --target x86_64-pc-windows-msvc -- -D warnings
    cargo clippy --locked -p hunnawood --no-default-features --all-targets --target x86_64-pc-windows-msvc -- -D warnings

    step "Build"
    cargo build --workspace --all-targets --locked

    step "Tests (GPU tests on lavapipe, the Vulkan driver that renders on the CPU, as in CI)"
    VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test --workspace --locked
    # glam checks the preconditions of its functions only when its assertions are on, which a game
    # that turns them on also turns on in the engine (feature unification): the renderer and the
    # world must keep them.
    VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test --locked -p slime-render -p slime-world --features glam/debug-glam-assert

    step "Rustdoc"
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

    step "Dependencies (licenses, advisories, sources)"
    cargo deny --locked check

    step "The sample game's list of source assets, and the files of it that are cooked (ADR-0070)"
    scripts/fetch-assets.sh zone/

    step "Source assets (each with a metadata file of its own, ADR-0069)"
    cargo run --locked -p slime-cook -- check examples/assets games/hunnawood/assets/zone

    step "Cooking the sample twice, from empty directories, gives the same bytes; a third cook cooks nothing"
    rm -rf "$cooked"
    cargo run --locked -p slime-cook -- cook examples/assets "$cooked/first"
    cargo run --locked -p slime-cook -- cook examples/assets "$cooked/second"
    diff -r "$cooked/first" "$cooked/second"
    cargo run --locked -p slime-cook -- cook examples/assets "$cooked/first" | grep -q ': 0 assets cooked'

    # A release build: a debug build of the cooker takes minutes to compress the zone's textures.
    step "The sample game's zone, cooked, and the tests that read it"
    cargo run --locked --release -p slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood
    VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test --locked -p hunnawood -- --ignored

    step "Headless samples (no display server)"
    env -u WAYLAND_DISPLAY -u DISPLAY cargo run --locked --example headless -- --frames 600
    env -u WAYLAND_DISPLAY -u DISPLAY cargo run --locked --example window -- --headless --frames 600
    # The world example's world runs without a window; it fails if its ring did not float.
    env -u WAYLAND_DISPLAY -u DISPLAY \
        cargo run --locked --example world -- --assets "$cooked/first" --headless --frames 600
    env -u WAYLAND_DISPLAY -u DISPLAY VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        cargo run --locked --example offscreen -- --output target/triangle.png
fi

if runs windowed; then
    if [ "$part" = windowed ]; then
        # What the headless part cooks, when it does not run first: the sample, and the game's zone,
        # which a cache may hold already, cooked again only where it changed.
        step "What the windowed runs draw: the sample and the game's zone, cooked"
        cargo run --locked -p slime-cook -- cook examples/assets "$cooked/first"
        scripts/fetch-assets.sh zone/
        cargo run --locked --release -p slime-cook -- \
            cook games/hunnawood/assets/zone target/cooked/hunnawood
    fi

    step "Windowed samples, the game and the lifecycle test in headless Wayland compositors (Weston), drawn with lavapipe"
    # A private runtime directory keeps these compositors apart from the one the developer may be using.
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
    start_weston slime-check
    # Each run takes a few seconds; `timeout` ends a stalled one, with SIGTERM, which stops the engine
    # cleanly, then SIGKILL if that is not enough, and the failure then prints Weston's log.
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo run --locked --example window -- --frames 120 ||
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
    # The game too (ADR-0055), by day and at night, each run a new one. It draws the zone cooked above;
    # a frame of it takes seconds on lavapipe, most of them in the shadows of the sun or the moon
    # through the leaves (docs/PLAN.md, known debt), so each run draws one frame only. Its saves go to
    # the private runtime directory, never to the user's (ADR-0088).
    for time in "" --night; do
        env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check \
            VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
            timeout -k 10 300 cargo run --locked -p hunnawood -- --new --frames 1 ${time:+"$time"} ||
            { cat "$runtime/slime-check.log"; exit 1; }
    done
    # And once more, losing its GPU device after its first frame of the zone: it must open a new one,
    # load its zone again and draw it once more, or it fails (ADR-0075).
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo run --locked -p hunnawood -- --new --frames 2 --lose-device-at 1 ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # And resumed from a copy of the save committed as a fixture, which it must resume (ADR-0088).
    cp games/hunnawood/fixtures/save.toml "$runtime/save.toml"
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 300 cargo run --locked -p hunnawood -- --save "$runtime/save.toml" --frames 1 2>&1 |
        tee "$runtime/resumed.log" ||
        { cat "$runtime/slime-check.log"; exit 1; }
    grep -q "the run resumes from" "$runtime/resumed.log" ||
        { echo "the game did not resume the run of its save"; exit 1; }
    # And driven by an agent's session through the MCP server, which starts it with a token and
    # connects to its development channel: pauses it, steps it, reads and changes the hero, walks it
    # with a scripted key, captures its image and reads its log (ADR-0122). The test needs a
    # compositor, so it is outside the default test run, as the lifecycle test below is.
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" XDG_DATA_HOME="$runtime/data" WAYLAND_DISPLAY=slime-check \
        VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 600 cargo test --locked -p slime-mcp --test drive ||
        { cat "$runtime/slime-check.log"; exit 1; }
    # The windowed lifecycle test needs the compositor too, so it is outside the default test run
    # (ADR-0024).
    env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
        timeout -k 10 120 cargo test --locked -p slime-app --test windowed_lifecycle ||
        { cat "$runtime/slime-check.log"; exit 1; }
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

if runs headless; then
    step "The game built for shipping holds neither the development channel nor the MCP server (ADR-0122)"
    if cargo tree --locked -p hunnawood --no-default-features -e normal | grep -E 'slime-(channel|mcp) '; then
        echo "the game built for shipping depends on the crates above"
        exit 1
    fi
    # The next `cargo build` or `cargo run` that names the game builds it again with its default
    # features.
    cargo build --locked -p hunnawood --no-default-features
    if grep -a -q -e SLIME_DEV_TOKEN -e slime-dev-channel target/debug/hunnawood; then
        echo "the game built for shipping holds the development channel"
        exit 1
    fi

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
fi

step "All checks passed"
