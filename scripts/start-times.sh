#!/usr/bin/env bash
# Times the starts of the editor and of the game (docs/adr/0189-first-measures-of-time.md): the
# editor on the examples' yard and the game's editor on its zone, each to its first image presented
# and, apart, to its level drawn whole; Hunna Wood to its zone drawn, when the player takes control.
# `hyperfine` times each run whole, from the process's launch to its exit after its first frame
# (`--frames 1`), and the launch alone, a run of `--help` that quits at once; the programs' logs
# give the times since the engine's start. A start, which the quality bar budgets, is the log's
# time to the first image plus the launch.
#
#     scripts/start-times.sh [--gpu] [--cold] [--runs <count>]
#
# The programs run in a headless Weston of their own, on copies of their levels, with a data and a
# state directory of their own: nothing reaches the desktop nor the user's files. Drawn with
# lavapipe, as the checks are; with `--gpu`, on the machine's GPU, through Weston's GL renderer
# (CONTRIBUTING.md, "Measuring"). 20 runs each by default, after 3 that warm the system's caches: a
# warm start. With `--cold`, each run follows the system's caches dropped instead, which asks for
# root through `sudo` before each run: only a person runs it, never an automated check. Run it on a
# machine at rest, in the development shell, once the game's zone is cooked
# (games/hunnawood/README.md).
set -euo pipefail
cd "$(dirname "$0")/.."

gpu=0
cold=0
runs=20
while [ $# -gt 0 ]; do
    case $1 in
        --gpu) gpu=1 ;;
        --cold) cold=1 ;;
        --runs)
            runs=${2:?--runs needs a count}
            shift
            ;;
        *)
            echo "usage: scripts/start-times.sh [--gpu] [--cold] [--runs <count>]" >&2
            exit 2
            ;;
    esac
    shift
done

if [ "$gpu" = 0 ] && [ -z "${SLIME_LAVAPIPE_ICD:-}" ]; then
    echo "SLIME_LAVAPIPE_ICD is not set: run this in the development shell (nix develop)" >&2
    exit 1
fi
zone=target/cooked/hunnawood
if [ ! -d "$zone" ]; then
    echo "the game's zone is not cooked in $zone: games/hunnawood/README.md says how" >&2
    exit 1
fi
cargo build --locked --release --example editor
cargo build --locked --release -p hunnawood -p hunnawood-editor
cargo run --locked --release -p slime-cook -- cook examples/assets target/cooked/examples

work=$(mktemp -d)
weston_pid=
program_pid=
# Stops what runs, each by its own pid, never by its name.
trap 'for pid in $program_pid $weston_pid; do kill "$pid" 2>/dev/null || true; done; rm -rf "$work"' EXIT
cp examples/assets/yard.scene examples/assets/yard.scene.meta "$work/"
cp games/hunnawood/assets/zone/zone.scene games/hunnawood/assets/zone/zone.scene.meta "$work/"

renderer=()
[ "$gpu" = 1 ] && renderer=(--renderer=gl)
XDG_RUNTIME_DIR="$work" weston --backend=headless --socket=start-times --idle-time=0 \
    "${renderer[@]}" >"$work/weston.log" 2>&1 &
weston_pid=$!
for _ in $(seq 100); do
    [ -S "$work/start-times" ] && break
    sleep 0.1
done
[ -S "$work/start-times" ] || {
    cat "$work/weston.log"
    exit 1
}

export XDG_RUNTIME_DIR="$work" WAYLAND_DISPLAY=start-times
export XDG_STATE_HOME="$work/state" XDG_DATA_HOME="$work/data"
unset DISPLAY DBUS_SESSION_BUS_ADDRESS SLIME_LOG SLIME_TRACE
# The loader's own choices would hide the driver named.
unset VK_LOADER_DRIVERS_SELECT VK_LOADER_DRIVERS_DISABLE
if [ "$gpu" = 1 ]; then
    unset VK_DRIVER_FILES
else
    export VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"
fi

# Prints the median of each time that the log $1 gives since the engine's start, of those that
# the lines named after $2 say; and, when $2 names the launch's figures that hyperfine wrote, the
# start: the median time to the first image plus the launch's mean.
summary() {
    python3 -I - "$@" <<'EOF'
import json
import re
import statistics
import sys

log, launch, names = sys.argv[1], sys.argv[2], sys.argv[3:]
times = {}
lines = "|".join(map(re.escape, names))
for line in open(log, encoding="utf-8"):
    found = re.search(rf"({lines}), ([0-9.]+) ms after the start", line)
    if found:
        times.setdefault(found[1], []).append(float(found[2]))
for what, values in times.items():
    print(f"  {what}: median {statistics.median(values):.1f} ms after the engine's start, "
          f"of {len(values)} runs, from {min(values):.1f} to {max(values):.1f} ms")
first = times.get("the first image is presented")
if launch and first:
    launched = json.load(open(launch, encoding="utf-8"))["results"][0]["mean"] * 1000
    print(f"  the launch, a run of --help: mean {launched:.1f} ms")
    print(f"  start, to the first image: {statistics.median(first) + launched:.1f} ms")
EOF
}

# What runs before each measured run: nothing for a warm start, the system's caches dropped for a
# cold one.
drop_caches="sync && sudo sysctl -q vm.drop_caches=3"
prepare() {
    if [ "$cold" = 1 ]; then
        bash -c "$drop_caches"
    fi
}

# Times the program named $1, whose command follows, with hyperfine, after 3 runs that warm the
# caches for a warm start, its log gathered, and prints what its log says.
measure() {
    local name=$1
    shift
    local log="$work/$name.log"
    if [ "$cold" = 0 ]; then
        for _ in 1 2 3; do
            "$@" 2>"$log" || {
                echo "$name failed:" >&2
                cat "$log" >&2
                exit 1
            }
        done
    fi
    : >"$log"
    local cold_runs=()
    [ "$cold" = 1 ] && cold_runs=(--prepare "$drop_caches")
    hyperfine --shell=bash --runs "$runs" "${cold_runs[@]}" --command-name "$name" \
        "$(printf '%q ' "$@") 2>>$(printf '%q' "$log")"
    # The launch alone: the same program, which quits as soon as it read its options. The game
    # knows no `--help`, and quits as soon with an error.
    hyperfine --shell=none --runs "$runs" "${cold_runs[@]}" --ignore-failure \
        --command-name "$name, launched" --export-json "$work/$name.launch.json" "$1 --help"
    grep -m1 -o "GPU adapter name=[^=]*api" "$log" | sed 's/ api$//' || true
    # A run that quits after its first frame may end before its level is drawn whole.
    summary "$log" "$work/$name.launch.json" "the first image is presented" "the zone is drawn"
}

# Runs the editor named $1, whose command follows, until its log says that it drew its level
# whole, every mesh loaded, then stops it, as many times as `--runs` says, and prints what its log
# says: the time of a level opened, which a run that quits after its first frame does not reach.
opened() {
    local name=$1
    shift
    local log="$work/$name-opened.log"
    : >"$log"
    for _ in $(seq "$runs"); do
        prepare
        "$@" 2>"$log.run" &
        program_pid=$!
        local drawn=0
        for _ in $(seq 600); do
            grep -q "the level is drawn whole" "$log.run" && drawn=1 && break
            kill -0 "$program_pid" 2>/dev/null || break
            sleep 0.1
        done
        kill "$program_pid" 2>/dev/null || true
        wait "$program_pid" || true
        program_pid=
        [ "$drawn" = 1 ] || {
            echo "$name did not draw its level whole within 60 s:" >&2
            cat "$log.run" >&2
            exit 1
        }
        cat "$log.run" >>"$log"
    done
    echo "$name, to its level drawn whole:"
    summary "$log" "" "the level is drawn whole"
}

measure editor target/release/examples/editor --level "$work/yard.scene" \
    --assets target/cooked/examples --journal "$work/yard.journal" --frames 1
opened editor target/release/examples/editor --level "$work/yard.scene" \
    --assets target/cooked/examples --journal "$work/yard.journal"
measure zone-editor target/release/hunnawood-editor --level "$work/zone.scene" \
    --journal "$work/zone.journal" --frames 1
opened zone-editor target/release/hunnawood-editor --level "$work/zone.scene" \
    --journal "$work/zone.journal"
measure game target/release/hunnawood --new --save "$work/run.save" --frames 1
