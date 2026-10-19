#!/usr/bin/env bash
# Takes the engine's measurements (docs/adr/0104-the-build-ci-and-measurements.md) and prints them
# to standard output as a section of docs/measurements.md, titled with the first argument, which
# the caller appends to that file:
#
#   scripts/measure.sh "P5, lavapipe" >> docs/measurements.md
#   scripts/measure.sh "P4, RX 6900 XT" render world   # the benchmarks whose names start so
#
# The arguments after the title select benchmarks by the start of their names, as slime-benches
# takes them; without any, every benchmark that is not run only on request. It measures, in order:
# the build of the workspace and of its tests from an empty target directory of its own, under
# target/, with the default debug settings, which takes minutes and about 8 GB of disk; the run of
# the workspace's tests, once built, doc tests included; then the benchmarks, in a release build. The GPU is the one the engine opens,
# so that `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"` measures lavapipe, as CI's machines have no other.
# Progress goes to standard error.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ $# -lt 1 ]; then
    echo "usage: scripts/measure.sh <title> [<benchmark name prefix>...]" >&2
    exit 1
fi
title=$1
shift

# The build's own debug settings, not a developer's override of them.
unset CARGO_PROFILE_DEV_DEBUG
commit=$(git rev-parse --short HEAD)
changes=$(git status --porcelain)
mkdir -p target
build_target=$(mktemp -d "$PWD/target/measure.XXXXXX")
trap 'rm -rf "$build_target"' EXIT

# The seconds since `start`, a time in nanoseconds, to a tenth.
seconds() {
    local tenths=$((($(date +%s%N) - $1) / 100000000))
    echo "$((tenths / 10)).$((tenths % 10))"
}

echo "measuring the build from an empty target directory" >&2
start=$(date +%s%N)
cargo build --workspace --locked -q --target-dir "$build_target"
build=$(seconds "$start")
start=$(date +%s%N)
if ! tests_log=$(cargo test --workspace --locked --no-run -v --target-dir "$build_target" 2>&1); then
    echo "$tests_log" >&2
    exit 1
fi
tests_build=$(seconds "$start")
# Each compiler run that cargo starts, whatever the compiler's path or wrapper, names its crate; a
# library among them is one that `cargo build` compiled already, compiled again.
if ! grep -q -E '^ *Running .*--crate-name ' <<< "$tests_log"; then
    echo "cargo's output names no compiler run: the count of libraries is unknown" >&2
    exit 1
fi
rebuilt=$(grep -c -E '^ *Running .*--crate-name .*--crate-type (lib|proc-macro)' \
    <<< "$tests_log" || true)
size=$(du -sb "$build_target" | cut -f1)

echo "measuring the workspace's tests" >&2
cargo test --workspace --locked --no-run -q
start=$(date +%s%N)
if ! tests_log=$(cargo test --workspace --locked -q 2>&1); then
    echo "$tests_log" >&2
    exit 1
fi
tests=$(seconds "$start")

echo "running the benchmarks" >&2
benchmarks=$(cargo run --locked --release -q -p slime-benches -- "$@")

echo "## $title"
echo
if [ -n "$changes" ]; then
    echo "Commit \`$commit\`, with changes not committed, on $(date -u +%Y-%m-%d)."
else
    echo "Commit \`$commit\`, on $(date -u +%Y-%m-%d)."
fi
echo
echo "| Build | Measure | Value |"
echo "|---|---|---|"
echo "| \`cargo build --workspace\` | from an empty target directory | $build s |"
echo "| \`cargo test --workspace --no-run\` | after it | $tests_build s |"
echo "| \`cargo test --workspace --no-run\` | libraries compiled again | $rebuilt |"
hundredths=$((size / 10000000))
gigabytes="$((hundredths / 100)).$(printf '%02d' $((hundredths % 100)))"
echo "| target directory | after both | $gigabytes GB |"
echo "| \`cargo test --workspace\` | its tests run, once built, doc tests included | $tests s |"
echo
echo "$benchmarks"
