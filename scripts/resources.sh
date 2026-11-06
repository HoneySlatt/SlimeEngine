#!/usr/bin/env bash
# Measures the memory and the power that the quality bar budgets
# (docs/adr/0193-first-measures-of-resources.md), through crates/slime-mcp/tests/resources.rs: the
# editor on Hunna Wood's zone, its memory and its video memory at rest, over 100 undos and redos
# and over 10 runs of Play; then the game at its 60 frames a second; and, with `--gpu`, the GPU's
# power before, with the editor at rest, with the game running, and after.
#
#     scripts/resources.sh [--gpu] [--heaptrack] [--undos <count>] [--plays <count>]
#
# The programs run in a headless Weston of their own, its output 3840 × 2160 at scale 3, so that the
# game's window, 1280 × 720 logical pixels, fills it, and the editor's, 1280 × 800, is as wide; on
# copies of the zone, with a data and a state directory of their own: nothing reaches the desktop
# nor the user's files. Drawn with lavapipe, which opens no GPU of the kernel and counts no video
# memory, and takes tens of seconds for a frame of the zone; with `--gpu`, on the machine's GPU,
# through Weston's GL renderer (CONTRIBUTING.md, "Measuring"), its power read from the driver's
# sensor. With `--heaptrack`, the editor runs under heaptrack instead, and the script prints its
# heap at each phase, the recording kept in target/resources/ for `heaptrack_gui`; such a run reads
# no power. Run it on a machine at rest, in the development shell, once the game's zone is cooked
# (games/hunnawood/README.md); it builds what it runs, in release. `--undos` and `--plays` measure
# longer series than the budget's 100 undos and redos and 10 runs of Play.
set -euo pipefail
cd "$(dirname "$0")/.."

usage="usage: scripts/resources.sh [--gpu] [--heaptrack] [--undos <count>] [--plays <count>]"
gpu=0
heaptrack=0
counts=()
while [ $# -gt 0 ]; do
    case $1 in
        --gpu) gpu=1 ;;
        --heaptrack) heaptrack=1 ;;
        --undos | --plays)
            counts+=("$1" "${2:?$1 needs a count}")
            shift
            ;;
        *)
            echo "$usage" >&2
            exit 2
            ;;
    esac
    shift
done

if [ "$gpu" = 0 ] && [ -z "${SLIME_LAVAPIPE_ICD:-}" ]; then
    echo "SLIME_LAVAPIPE_ICD is not set: run this in the development shell (nix develop)" >&2
    exit 1
fi
if [ ! -d target/cooked/hunnawood ]; then
    echo "the game's zone is not cooked in target/cooked/hunnawood:" \
        "games/hunnawood/README.md says how" >&2
    exit 1
fi
measure=()
if [ "$heaptrack" = 1 ]; then
    command -v heaptrack >/dev/null || {
        echo "heaptrack is not on the path: run this in the development shell (nix develop)" >&2
        exit 1
    }
elif [ "$gpu" = 1 ]; then
    # The driver's sensor of the GPU's power, in microwatts: amdgpu's average over its last second.
    sensors=(/sys/class/drm/card*/device/hwmon/hwmon*/power1_average)
    if [ "${#sensors[@]}" != 1 ] || [ ! -r "${sensors[0]}" ]; then
        echo "no single sensor of a GPU's power found: ${sensors[*]}" >&2
        exit 1
    fi
    echo "the GPU's power from ${sensors[0]}"
    measure=(--power "${sensors[0]}")
fi
# Play builds the game in the editor's profile, with its development channel; a build without it,
# which scripts/check.sh leaves, would never answer.
cargo build --locked --release -p hunnawood -p hunnawood-editor -p slime-mcp
cargo test --locked --release -p slime-mcp --test resources --no-run

work=$(mktemp -d)
weston_pid=
# Stops what runs, by its own pid, never by its name.
trap 'if [ -n "$weston_pid" ]; then kill "$weston_pid" 2>/dev/null || true; fi; rm -rf "$work"' EXIT
renderer=()
[ "$gpu" = 1 ] && renderer=(--renderer=gl)
XDG_RUNTIME_DIR="$work" weston --backend=headless --socket=resources --idle-time=0 \
    --width=3840 --height=2160 --scale=3 "${renderer[@]}" >"$work/weston.log" 2>&1 &
weston_pid=$!
for _ in $(seq 100); do
    [ -S "$work/resources" ] && break
    sleep 0.1
done
[ -S "$work/resources" ] || {
    cat "$work/weston.log"
    exit 1
}

export XDG_RUNTIME_DIR="$work" WAYLAND_DISPLAY=resources
unset DISPLAY DBUS_SESSION_BUS_ADDRESS SLIME_LOG SLIME_TRACE
# The loader's own choices would hide the driver named.
unset VK_LOADER_DRIVERS_SELECT VK_LOADER_DRIVERS_DISABLE
if [ "$gpu" = 1 ]; then
    unset VK_DRIVER_FILES
else
    export VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"
fi

if [ "$heaptrack" = 0 ]; then
    cargo test --locked --release -p slime-mcp --test resources -- "${measure[@]}" "${counts[@]}"
    exit
fi

mkdir -p target/resources
rm -f target/resources/editor.heaptrack.*
record="$PWD/target/resources/editor.heaptrack"
cargo test --locked --release -p slime-mcp --test resources -- --heaptrack "$record" "${counts[@]}" |
    tee "$work/run.txt"
record=$(ls "$record".*)
echo "heaptrack's recording: $record"
echo
heaptrack_print --file "$record" --print-peaks 0 --print-allocators 0 --print-temporary 0 \
    --massif-detailed-freq 0 --print-massif "$work/massif.txt" | tail -n 6
# The heap at the end of each phase: massif's last snapshot at or before the second that the run
# says.
python3 -I - "$work/run.txt" "$work/massif.txt" <<'EOF'
import re
import sys

phases = []
when = None
for line in open(sys.argv[1], encoding="utf-8"):
    if not line.startswith(" "):
        when = line.split(":")[0]
    found = re.search(r"heaptrack: ([0-9.]+) s after the editor started", line)
    if found:
        phases.append((when, float(found[1])))
snapshots = []
time = None
for line in open(sys.argv[2], encoding="utf-8"):
    if line.startswith("time="):
        time = float(line[5:])
    elif line.startswith("mem_heap_B=") and time is not None:
        snapshots.append((time, int(line[11:])))
if not phases or not snapshots:
    sys.exit("heaptrack's record holds no snapshot of the heap, or the run no phase")
print()
for when, second in phases:
    before = [snapshot for snapshot in snapshots if snapshot[0] <= second]
    if not before:
        sys.exit(f"no snapshot of the heap before {second} s: {when}")
    time, heap = before[-1]
    print(f"heap {heap / 2**20:.1f} MiB at {time:.1f} s: {when}")
EOF
