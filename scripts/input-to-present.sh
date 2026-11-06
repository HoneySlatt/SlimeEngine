#!/usr/bin/env bash
# Measures input to present (docs/adr/0189-first-measures-of-time.md,
# docs/adr/0191-input-in-sway-start-budgets-and-the-editors-rate.md): the editor on a copy of the
# examples' yard, then Hunna Wood, each given a person's input by `wlrctl` in a headless sway of its
# own, their traces read by scripts/frame-pacing.py.
#
#     scripts/input-to-present.sh [--gpu]
#
# The input reaches the window through the compositor, as a person's does, through the protocols of
# virtual pointers and keyboards that wlroots gives: inside this compositor alone, never the
# desktop's. Two runs each, read apart: inputs apart, every 100 ms, the program at rest between
# them; and inputs in a row, about 180 a second, the pointer in motion. Its output refreshes at
# 240 Hz, the high tier's display. Drawn with lavapipe, or with `--gpu` on the machine's GPU through
# sway's GL renderer; a game's frame takes seconds on lavapipe, and gives few images after an
# input. Run it on a machine at rest, in the
# development shell, once the game's zone is cooked (games/hunnawood/README.md).
set -euo pipefail
cd "$(dirname "$0")/.."

gpu=0
case ${1:-} in
    --gpu) gpu=1 ;;
    "") ;;
    *)
        echo "usage: scripts/input-to-present.sh [--gpu]" >&2
        exit 2
        ;;
esac
if [ "$gpu" = 0 ] && [ -z "${SLIME_LAVAPIPE_ICD:-}" ]; then
    echo "SLIME_LAVAPIPE_ICD is not set: run this in the development shell (nix develop)" >&2
    exit 1
fi
if [ ! -d target/cooked/hunnawood ]; then
    echo "the game's zone is not cooked in target/cooked/hunnawood: games/hunnawood/README.md says how" >&2
    exit 1
fi
cargo build --locked --release --example editor
cargo build --locked --release -p hunnawood
cargo run --locked --release -p slime-cook -- cook examples/assets target/cooked/examples

work=$(mktemp -d)
sway_pid=
program_pid=
# Stops what runs, each by its own pid, never by its name.
trap 'for pid in $program_pid $sway_pid; do kill "$pid" 2>/dev/null || true; done; rm -rf "$work"' EXIT
cp examples/assets/yard.scene examples/assets/yard.scene.meta "$work/"
# No Xwayland, which would write into the system's /tmp/.X11-unix.
printf 'output HEADLESS-1 mode --custom 1920x1080@240Hz\ndefault_border none\nxwayland disable\n' \
    >"$work/sway.config"

renderer=pixman
[ "$gpu" = 1 ] && renderer=gles2
mkdir -p "$work/runtime"
chmod 700 "$work/runtime"
# Nothing of the desktop's session: its own runtime directory, no session bus, no display.
env -u WAYLAND_DISPLAY -u DISPLAY -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$work/runtime" \
    WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=$renderer \
    sway -c "$work/sway.config" >"$work/sway.log" 2>&1 &
sway_pid=$!
socket=
for _ in $(seq 100); do
    socket=$(find "$work/runtime" -maxdepth 1 -name 'wayland-*' -type s -printf '%f\n' | head -n 1)
    [ -n "$socket" ] && break
    sleep 0.1
done
[ -n "$socket" ] || {
    cat "$work/sway.log"
    exit 1
}
# Its output must refresh at 240 Hz before a program starts: sway keeps a default mode without a
# word when it refuses the one asked.
refresh=
for _ in $(seq 50); do
    refresh=$(SWAYSOCK=$(find "$work/runtime" -maxdepth 1 -name 'sway-ipc.*.sock' | head -n 1) \
        swaymsg -t get_outputs -r 2>/dev/null |
        python3 -I -c 'import json, sys; print(json.load(sys.stdin)[0]["current_mode"]["refresh"])' \
        2>/dev/null || true)
    [ "$refresh" = 240000 ] && break
    sleep 0.1
done
[ "$refresh" = 240000 ] || {
    echo "sway's output refreshes at ${refresh:-an unknown rate} mHz, not 240 Hz:" >&2
    cat "$work/sway.log" >&2
    exit 1
}

export XDG_RUNTIME_DIR="$work/runtime" WAYLAND_DISPLAY=$socket
export XDG_STATE_HOME="$work/state" XDG_DATA_HOME="$work/data"
unset DISPLAY DBUS_SESSION_BUS_ADDRESS SLIME_LOG
# The loader's own choices would hide the driver named.
unset VK_LOADER_DRIVERS_SELECT VK_LOADER_DRIVERS_DISABLE
if [ "$gpu" = 1 ]; then
    unset VK_DRIVER_FILES
else
    export VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"
fi

# Starts the program named $1, whose command follows, traced, and waits until its log says the
# line that $2 names.
start() {
    local name=$1 ready=$2
    shift 2
    SLIME_TRACE="$work/$name.json" "$@" 2>"$work/$name.log" &
    program_pid=$!
    for _ in $(seq 600); do
        grep -q "$ready" "$work/$name.log" && return
        kill -0 "$program_pid" 2>/dev/null || break
        sleep 0.1
    done
    echo "$name did not say \"$ready\" within 60 s:" >&2
    cat "$work/$name.log" >&2
    exit 1
}

# Inputs apart, each 100 ms after the last, the program at rest between them: the pointer, its
# buttons, the wheel and the keys.
apart() {
    for step in $(seq 200); do
        case $((step % 4)) in
            0) wlrctl pointer click left ;;
            1) wlrctl pointer scroll 0 1 ;;
            2) wlrctl keyboard type a ;;
            *) wlrctl pointer move $((step % 8 < 4 ? 40 : -40)) 15 ;;
        esac
        sleep 0.1
    done
}

# Inputs in a row, about 180 a second, as a mouse moves: the pointer in motion.
in_a_row() {
    for step in $(seq 1000); do
        wlrctl pointer move $((step % 40 < 20 ? 6 : -6)) 2
        sleep 0.004
    done
}

# Stops the program that runs, which must still run and stop well, then reads its trace.
finish() {
    local name=$1
    shift
    sleep 0.5
    kill -0 "$program_pid" 2>/dev/null || {
        echo "$name ended before its inputs did:" >&2
        cat "$work/$name.log" >&2
        exit 1
    }
    kill "$program_pid"
    local status=0
    wait "$program_pid" || status=$?
    program_pid=
    [ "$status" = 0 ] || {
        echo "$name ended with status $status:" >&2
        cat "$work/$name.log" >&2
        exit 1
    }
    echo "$name:"
    grep -m1 -o "GPU adapter name=[^=]*api" "$work/$name.log" | sed 's/ api$//' || true
    scripts/frame-pacing.py "$work/$name.json" --skip 0 "$@"
}

game_frame=$(python3 -I -c 'print(1000 / 60)')
for inputs in apart in_a_row; do
    start "editor, $inputs" "the first image is presented" target/release/examples/editor \
        --level "$work/yard.scene" --assets target/cooked/examples --journal "$work/yard.journal"
    $inputs
    finish "editor, $inputs"
    start "game, $inputs" "the zone is drawn" target/release/hunnawood --new --save "$work/run.save"
    $inputs
    finish "game, $inputs" --frame "$game_frame"
done
