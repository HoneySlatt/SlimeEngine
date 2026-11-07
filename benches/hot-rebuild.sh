#!/usr/bin/env bash
# Times what the reload of a function's body saves (ADR-0212): the same change as `--hot` makes,
# the library's placeholder, saved, then the bench built again by cargo, as an author without a
# reload would before starting the program again. Prints each build's time in milliseconds, then
# the median and range of `runs` (21). Run inside `nix develop`, from the workspace's root, after
# one `cargo build -p slime-benches --features hot`. The source is put back as it was.
set -euo pipefail
runs=${1:-21}
source=benches/src/prototypes/signals/library.rs
original=$(mktemp)
cp "$source" "$original"
trap 'cp "$original" "$source"; rm -f "$original"' EXIT
grep -q '"Search a game"' "$source"
times=()
for run in $(seq 0 "$runs"); do
    if (( run % 2 == 0 )); then from='Search a game'; to='Find a game'; else from='Find a game'; to='Search a game'; fi
    sed "s/\"$from\"/\"$to\"/" "$source" > target/library.rs.hot
    mv target/library.rs.hot "$source"
    start=$(date +%s%N)
    cargo build --quiet -p slime-benches --features hot
    took=$(( ($(date +%s%N) - start) / 1000000 ))
    echo "build $run: $took ms" >&2
    # The first build is the session's first, as `--hot`'s first patch: left out of the median.
    (( run > 0 )) && times+=("$took")
done
printf '%s\n' "${times[@]}" | sort -n | awk '{t[NR]=$1} END {printf "%d builds: %.1f (%.1f–%.1f) ms\n", NR, t[int(NR/2)+1], t[1], t[NR]}'
