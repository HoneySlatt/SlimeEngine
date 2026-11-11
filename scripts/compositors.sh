#!/usr/bin/env bash
# Runs the editor and the game in one compositor of the series, headless, and writes what it found
# (docs/adr/0195-a-series-of-headless-compositors.md,
# docs/adr/0240-the-series-input-its-table-and-its-jobs.md): Weston, sway, KWin, Mutter, or X11
# through Xvfb. CI's `compositors` workflow runs it for each, side by side.
#
#     nix develop .#compositors -c scripts/compositors.sh weston|sway|kwin|mutter|x11
#
# On an output of 1920 by 1080 logical pixels, at 240 Hz where the compositor allows it, it lists
# the compositor's protocols (`wayland-info`); traces the frames of the `window` example, at most
# 240 a second; reads in the editor's log, written with WAYLAND_DEBUG, how it adapts: its
# decorations, its cursor, its clipboard, and a round trip through the clipboard by a person's keys,
# which scripts/compositor-input.py gives where the compositor takes input; runs the agents'
# sessions of the editor and of the game, which find them at rest, and minimizes the game
# (slime-mcp's `edit_window` and `drive`); then reads the window's scale on an output at a
# fractional scale. What it observes must match the compositor's column of docs/compositors.md, or
# it fails. Its report, logs and figures go to target/compositors/<compositor>/; drawn with
# lavapipe, on a session bus of its own. It fetches and cooks the game's zone first, as
# `scripts/check.sh game` does.
set -euo pipefail
cd "$(dirname "$0")/.."

name=${1:-}
case "$name" in
    weston | sway | kwin | mutter | x11) ;;
    *)
        echo "usage: scripts/compositors.sh weston|sway|kwin|mutter|x11" >&2
        exit 2
        ;;
esac
: "${SLIME_COMPOSITORS:?run this script in the shell of the series: nix develop .#compositors}"
unset VK_LOADER_DRIVERS_SELECT VK_LOADER_DRIVERS_DISABLE

# A session bus of its own, which activates no service: KWin and Mutter give their input there,
# and nothing reaches the desktop's.
if [ -z "${SLIME_COMPOSITORS_BUS:-}" ]; then
    bus=$(mktemp -d)
    trap 'rm -rf "$bus"' EXIT
    cat >"$bus/session.conf" <<CONF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=$bus</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF
    status=0
    SLIME_COMPOSITORS_BUS=$bus dbus-run-session --config-file="$bus/session.conf" -- "$0" "$@" ||
        status=$?
    exit "$status"
fi

report=target/compositors/$name
rm -rf "$report"
mkdir -p "$report"
observations=$report/observations.txt
work=$(mktemp -d)
compositor_pid=
input_pid=
# Stops the input's server, then the compositor, each by its own pid, killed if it has not ended
# within 10 s.
stop_compositor() {
    for pid in $input_pid $compositor_pid; do
        kill "$pid" 2>/dev/null || true
        for _ in $(seq 100); do
            kill -0 "$pid" 2>/dev/null || break
            sleep 0.1
        done
        kill -KILL "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done
    input_pid= compositor_pid=
}
trap 'stop_compositor; rm -rf "$work"' EXIT

step() { printf '\n==> %s: %s\n' "$name" "$*"; }
# Writes an observation, which the report and the comparison read.
observe() { echo "$1: $2" | tee -a "$observations"; }

step "Build"
cargo build --locked --example editor --example window
cargo build --locked -p hunnawood
cargo test --locked --no-run -p slime-mcp --test compositor --test edit_window --test drive
cooked=target/cooked/examples
cargo run --locked -p slime-cook -- cook examples/assets "$cooked"
scripts/fetch-assets.sh zone/
cargo run --locked --release -p slime-cook -- cook games/hunnawood/assets/zone target/cooked/hunnawood

# The compositor's output: 1920 by 1080 logical pixels, refreshing at 240 Hz where it can.
# Starts the compositor at the scale given, and sets the environment through which programs reach
# it; its own files go to the work directory, never the user's.
start_compositor() {
    local scale=$1
    stop_compositor
    rm -rf "$work/runtime" "$work/home"
    mkdir -m 700 "$work/runtime"
    mkdir -p "$work/home"
    export XDG_RUNTIME_DIR=$work/runtime
    export XDG_CONFIG_HOME=$work/home/config XDG_DATA_HOME=$work/home/data
    export XDG_STATE_HOME=$work/home/state XDG_CACHE_HOME=$work/home/cache
    unset WAYLAND_DISPLAY DISPLAY SLIME_COMPOSITOR_INPUT
    # KWin and Mutter draw with EGL on the CPU, as on CI's runners, which have no GPU. nixpkgs'
    # loader of EGL finds the system's drivers in /run/opengl-driver on NixOS, and none elsewhere:
    # there it is given nixpkgs' Mesa, which, given on NixOS, would mix with the system's.
    local egl=(env LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe)
    [ -d /run/opengl-driver/share/glvnd/egl_vendor.d ] ||
        egl+=(__EGL_VENDOR_LIBRARY_FILENAMES="$SLIME_MESA_EGL")
    local log=$report/compositor-$scale.log socket=slime-series
    case "$name" in
        weston)
            # Integer scales alone.
            weston --backend=headless --socket=$socket --idle-time=0 --width=1920 --height=1080 \
                --refresh-rate=240000 --scale="${scale%.5}" >"$log" 2>&1 &
            ;;
        sway)
            # No Xwayland, which would write into the system's /tmp/.X11-unix.
            printf 'output HEADLESS-1 mode --custom 1920x1080@240Hz scale %s\nxwayland disable\n' \
                "$scale" >"$work/sway.config"
            WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
                sway -c "$work/sway.config" >"$log" 2>&1 &
            ;;
        kwin)
            # Its virtual output refreshes at 60 Hz, which neither an option nor a custom mode of
            # its settings changes. `--scale` is lost to the output's settings, which KWin writes
            # at its first start: they are given the scale, and KWin started again.
            local kwin=("${egl[@]}" kwin_wayland --virtual --socket $socket --width 1920 --height 1080
                --no-lockscreen)
            if [ "$scale" != 1 ]; then
                "${kwin[@]}" >"$log" 2>&1 &
                compositor_pid=$!
                for _ in $(seq 100); do
                    [ -s "$XDG_CONFIG_HOME/kwinoutputconfig.json" ] && break
                    sleep 0.1
                done
                stop_compositor
                python3 -I -c 'import json, sys
path, scale = sys.argv[1], float(sys.argv[2])
settings = json.load(open(path))
for section in settings:
    if section["name"] == "outputs":
        for output in section["data"]:
            output["scale"] = scale
json.dump(settings, open(path, "w"), indent=4)' "$XDG_CONFIG_HOME/kwinoutputconfig.json" "$scale"
            fi
            "${kwin[@]}" >>"$log" 2>&1 &
            ;;
        mutter)
            # Its settings in memory, never in the user's dconf.
            "${egl[@]}" GSETTINGS_BACKEND=memory mutter --no-x11 --headless \
                --virtual-monitor 1920x1080@240 --wayland-display $socket >"$log" 2>&1 &
            ;;
        x11)
            # Xvfb gives no refresh rate, and no scale.
            Xvfb -displayfd 3 -screen 0 1920x1080x24 -nolisten tcp 3>"$work/display" >"$log" 2>&1 &
            ;;
    esac
    compositor_pid=$!
    for _ in $(seq 100); do
        if [ "$name" = x11 ]; then
            [ -s "$work/display" ] && break
        else
            # sway names its socket itself, `wayland-1`.
            [ "$name" = sway ] &&
                socket=$(find "$work/runtime" -maxdepth 1 -name 'wayland-*' -type s -printf '%f\n' |
                    head -n 1)
            [ -S "$work/runtime/$socket" ] && break
        fi
        kill -0 "$compositor_pid" 2>/dev/null || break
        sleep 0.1
    done
    if [ "$name" = x11 ]; then
        [ -s "$work/display" ] || { cat "$log"; exit 1; }
        export DISPLAY=:$(cat "$work/display")
    else
        [ -S "$work/runtime/$socket" ] || { echo "$name did not start:"; cat "$log"; exit 1; }
        export WAYLAND_DISPLAY=$socket
    fi
    if [ "$name" = mutter ] && [ "$scale" != 1 ]; then
        gdctl set --logical-monitor --primary --monitor Meta-0 --scale "$scale" >>"$log" 2>&1
    fi
    # The first client may come before the compositor has made its output: wait until one shows.
    for _ in $(seq 100); do
        if [ "$name" = x11 ]; then
            xdotool getdisplaygeometry >/dev/null 2>&1 && break
        else
            wayland-info 2>/dev/null | grep -q "'wl_output'" && break
        fi
        sleep 0.1
    done
    # The input's server, which holds a keyboard and a pointer for the whole session; Weston takes
    # no input.
    [ "$name" = weston ] && return
    local input=$work/input.socket
    rm -f "$input"
    scripts/compositor-input.py serve "$name" "$input" >"$report/input-$scale.log" 2>&1 &
    input_pid=$!
    # Longer than the server waits for its devices, 10 s.
    for _ in $(seq 200); do
        [ -S "$input" ] && break
        kill -0 "$input_pid" 2>/dev/null || break
        sleep 0.1
    done
    [ -S "$input" ] || { echo "the input's server did not start:"; cat "$report/input-$scale.log"; exit 1; }
    export SLIME_COMPOSITOR_INPUT=$input
}

# A program of the engine's, drawn with lavapipe, without the series' bus, as CI runs it.
run() {
    env -u DBUS_SESSION_BUS_ADDRESS VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" SLIME_SERIES=$name "$@"
}

start_compositor 1

step "Protocols"
if [ "$name" = x11 ]; then
    echo "X11 has no protocols of Wayland's: wayland-info lists none" >"$report/protocols.txt"
else
    wayland-info >"$report/protocols.txt"
    scripts/compositors.py protocols "$report/protocols.txt" | tee -a "$observations"
fi

step "Frames: the window example's, 600 of them at most 240 a second, traced"
run SLIME_TRACE="$work/window.json" SLIME_LOG=info timeout -k 10 120 \
    target/debug/examples/window --frames 600 --frame-rate 240 2>"$report/window.log" ||
    { cat "$report/window.log" "$report/compositor-1.log"; exit 1; }
grep -o 'backend="[a-z0-9]*"' "$report/window.log" | head -n 1 | tr -d '"' | sed 's/backend=/Backend: /' |
    tee -a "$observations"
scripts/frame-pacing.py "$work/window.json" | tee "$report/frames.txt"

step "How the editor adapts: an agent's session through the MCP server, with a person's input"
debug=
[ "$name" = x11 ] || debug=client
run WAYLAND_DEBUG=$debug timeout -k 10 300 \
    cargo test --locked -p slime-mcp --test compositor -- "$cooked" | tee "$work/adapts.txt" ||
    { cat "$report/compositor-1.log"; exit 1; }
grep '^clipboard: ' "$work/adapts.txt" | sed 's/^clipboard: /Clipboard round trip: /' |
    tee -a "$observations"
editor_log=$(sed -n 's/^log: //p' "$work/adapts.txt")
cp "$editor_log" "$report/editor.log"
if grep -q "fields will copy and paste nothing" "$report/editor.log"; then
    observe "Clipboard" "none"
else
    observe "Clipboard" "opened"
fi
[ "$name" = x11 ] || scripts/compositors.py trace "$report/editor.log" | tee -a "$observations"

step "The agents' sessions of the editor and of the game, and their rest"
run timeout -k 10 300 cargo test --locked -p slime-mcp --test edit_window -- "$cooked" ||
    { cat "$report/compositor-1.log"; exit 1; }
run timeout -k 10 300 cargo test --locked -p slime-mcp --test drive | tee "$work/drive.txt" ||
    { cat "$report/compositor-1.log"; exit 1; }
grep '^minimized: ' "$work/drive.txt" | sed 's/^minimized: /Game minimized: /' | tee -a "$observations"
observe "Agents' sessions and rest" "pass"

step "Scale: the window example on an output at a fractional scale"
case "$name" in
    x11) observe "Window's scale" "— X11 gives no scale for an output" ;;
    *)
        if [ "$name" = weston ]; then start_compositor 2; else start_compositor 1.5; fi
        run SLIME_LOG=info timeout -k 10 120 target/debug/examples/window --frames 30 \
            2>"$report/window-scaled.log" ||
            { cat "$report/window-scaled.log"; exit 1; }
        # The scale that the window was created at, or the last that the compositor gave it after.
        scale=$(grep -o 'scale_factor[=:] *[0-9.]*' "$report/window-scaled.log" | tail -n 1 |
            grep -o '[0-9.]*$')
        observe "Window's scale" "$scale"
        ;;
esac
stop_compositor

step "Compared with docs/compositors.md"
scripts/compositors.py compare docs/compositors.md "$name" "$observations" | tee "$report/compared.txt"
