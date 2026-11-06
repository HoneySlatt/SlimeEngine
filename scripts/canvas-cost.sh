#!/usr/bin/env bash
# Measures what the renderer costs an application that draws only interfaces, against the canvas
# alone (docs/adr/0180-one-canvas-and-the-frameworks-foundations.md): the time to build each and
# draw a first canvas, the resident memory each adds, the GPU memory that wgpu allocates for each
# (`Gpu::memory_in_use`) and the video memory that the driver gives the process, its own included
# (`drm-total-vram`, on a GPU that the kernel's DRM drives, not on lavapipe). The device alone is
# measured too, as the floor that both pay. Each part runs in a process of its own, its runs
# alternated with the others', and the figures are the median and range of each.
#
#     scripts/canvas-cost.sh [--gpu] [--runs <count>]
#
# Drawn with lavapipe by default, whose times are a CPU's that compiles the shaders, not a GPU's;
# with `--gpu`, on the machine's GPU. 20 runs a part by default, after one of each that warms the
# system's caches. Run it in the development shell, on a machine at rest. Nothing opens a window.
set -euo pipefail
cd "$(dirname "$0")/.."

gpu=0
runs=20
while [ $# -gt 0 ]; do
    case $1 in
        --gpu) gpu=1 ;;
        --runs)
            runs=${2:?--runs needs a count}
            shift
            ;;
        *)
            echo "usage: scripts/canvas-cost.sh [--gpu] [--runs <count>]" >&2
            exit 2
            ;;
    esac
    shift
done

if [ "$gpu" = 0 ] && [ -z "${SLIME_LAVAPIPE_ICD:-}" ]; then
    echo "SLIME_LAVAPIPE_ICD is not set: run this in the development shell (nix develop)" >&2
    exit 1
fi
# The loader's own choices would hide the driver named.
unset VK_LOADER_DRIVERS_SELECT VK_LOADER_DRIVERS_DISABLE
if [ "$gpu" = 1 ]; then
    unset VK_DRIVER_FILES
else
    export VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"
fi
unset SLIME_LOG SLIME_TRACE

# The tests of the renderer's library, built once; `cost.rs` holds the parts.
tests=$(cargo test --locked --release -p slime-render --lib --no-run --message-format=json |
    python3 -I -c '
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    if message.get("reason") == "compiler-artifact" and message.get("executable") \
            and message["target"]["kind"] == ["lib"]:
        print(message["executable"])
')
[ -x "$tests" ] || {
    echo "the renderer's tests were not built" >&2
    exit 1
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
parts=(device canvas renderer)
run() {
    "$tests" --ignored --exact --nocapture "cost::cost_of_the_$1" 2>&1 | grep -o "cost $1 .*"
}
for part in "${parts[@]}"; do
    run "$part" >/dev/null
done
for _ in $(seq "$runs"); do
    for part in "${parts[@]}"; do
        run "$part" >>"$work/figures"
    done
done

python3 -I - "$work/figures" <<'EOF'
import re
import statistics
import sys

figures = {}
adapter = None
for line in open(sys.argv[1], encoding="utf-8"):
    part = line.split()[1]
    adapter = re.search(r'adapter="([^"]*)"', line)[1]
    for name, value in re.findall(r"(\w+)=([0-9.]+)", line):
        figures.setdefault(part, {}).setdefault(name, []).append(float(value))
print(f"adapter: {adapter}")
names = {
    "open_ms": "the device opened (ms)",
    "device_and_target_resident_kib": "resident memory of the device and the target (KiB)",
    "device_and_target_video_kib": "video memory of the device and the target (KiB)",
    "built_ms": "built (ms)",
    "first_canvas_ms": "built and a first canvas drawn (ms)",
    "resident_kib": "resident memory added (KiB)",
    "gpu_kib": "GPU memory allocated (KiB)",
    "video_kib": "video memory asked of the driver (KiB)",
}
for part, values in figures.items():
    print(f"{part}, {len(values['built_ms'])} runs:")
    for name, label in names.items():
        if name in values:
            series = values[name]
            print(f"  {label}: median {statistics.median(series):.1f}, "
                  f"from {min(series):.1f} to {max(series):.1f}")
EOF
