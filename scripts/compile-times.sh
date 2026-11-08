#!/usr/bin/env bash
# Times the steps of compiling that ADR-0215's budgets name (docs/ROADMAP.md, "Budgets"), as
# docs/measurements.md's sections from C1 on hold them: the whole workspace from nothing; a
# function's body changed in the game, built as the editor's Play builds it; a function's body
# changed in an engine crate, built for the game's editor and for the game, then for the examples'
# editor and its game; a public function added to an engine crate; a game's build for shipping
# from nothing. Each step says how many packages cargo compiled, which shows what two programs
# share (docs/adr/0221-compiling-within-its-budgets.md).
#
#     scripts/compile-times.sh [--runs <count>] [--cold-runs <count>] [--shipping-runs <count>]
#
# It builds a copy of the working tree in a directory of its own, removed at the end: the files that
# git tracks, as they are now, and new files once added to git's index; the repository's own
# `target` is not touched. rustc reuses its work when only a file's date changes, so each change is a
# real one: a statement `std::hint::black_box(<number>)` written at the start of a function, a
# public function of another name. After each change, the four programs are built again, untimed,
# so that no step pays for the one before it. `--runs` changes, 7 by default, `--cold-runs` builds
# of the workspace from nothing, 3, and `--shipping-runs` builds for shipping from nothing, 1; the
# table gives the median of each step and its range. A commit without the profile `shipping`, older
# than ADR-0221, is measured in release instead, and the table says so. Run it on a machine at rest,
# in the development shell; the header says the load before and after. cargo's environment is
# cleared of the settings that would change what it builds, but the flags of rustc, which the
# header names: `RUSTFLAGS`, for one, tries another linker.
set -euo pipefail
cd "$(dirname "$0")/.."

runs=7
cold_runs=3
shipping_runs=1
usage="usage: scripts/compile-times.sh [--runs <count>] [--cold-runs <count>] [--shipping-runs <count>]"
while [ $# -gt 0 ]; do
    case $1 in
        --runs) runs=${2:?--runs needs a count}; shift ;;
        --cold-runs) cold_runs=${2:?--cold-runs needs a count}; shift ;;
        --shipping-runs) shipping_runs=${2:?--shipping-runs needs a count}; shift ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
    shift
done
for count in "$runs" "$cold_runs" "$shipping_runs"; do
    [[ $count =~ ^[1-9][0-9]*$ ]] || { echo "$usage" >&2; exit 2; }
done

commit="\`$(git rev-parse --short HEAD)\`"
git diff --quiet HEAD -- || commit="$commit, with the working tree's changes"
load_before=$(cut -d' ' -f1-3 /proc/loadavg)

# A relative TMPDIR would put the copy inside the repository.
temporary=${TMPDIR:-/tmp}
[[ $temporary == /* ]] || temporary=/tmp
work=$(mktemp -d "$temporary/compile-times.XXXXXX")
trap 'rm -rf "$work"' EXIT
copy=$work/repo
results=$work/results
mkdir "$copy"
git ls-files -z --cached | tar --null --files-from=- --ignore-failed-read -cf - 2>/dev/null |
    tar -xf - -C "$copy"
cd "$copy"
# What cargo builds, and where, is the copy's own: no target, build directory, wrapper or profile
# of the environment's.
export CARGO_TARGET_DIR=$copy/target
unset CARGO_BUILD_TARGET_DIR CARGO_BUILD_BUILD_DIR RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER \
    CARGO_BUILD_RUSTC_WRAPPER CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER CARGO_INCREMENTAL
while read -r name; do unset "$name"; done < <(env | sed -n 's/^\(CARGO_PROFILE_[A-Z0-9_]*\)=.*/\1/p')

# Runs `cargo build --locked <arguments>`, and writes the step's time and the packages it compiled.
step() {
    local label=$1 start end output
    shift
    echo "compile-times: $label" >&2
    start=$(date +%s.%N)
    if ! output=$(cargo build --locked "$@" 2>&1); then
        echo "$output" >&2
        exit 1
    fi
    end=$(date +%s.%N)
    printf '%s\t%s\t%s\n' "$label" "$(awk "BEGIN { printf \"%.2f\", $end - $start }")" \
        "$(grep -c '^ *Compiling ' <<<"$output" || true)" >>"$results"
}

# Builds the four programs, untimed, so that the next step starts from them all up to date.
catch_up() {
    local arguments
    for arguments in "-p hunnawood-editor" "-p hunnawood --bin hunnawood" "--example editor" \
        "--workspace --example world"; do
        # shellcheck disable=SC2086 # each holds several arguments
        cargo build --locked $arguments >/dev/null 2>&1 || { cargo build --locked $arguments; exit 1; }
    done
}

# Writes `std::hint::black_box(<number>);` as the first statement of the function that `anchor`, a
# whole line of `file`, opens, or changes the number that it gives.
change_body() {
    local file=$1 anchor=$2 number=$3
    if grep -q 'std::hint::black_box([0-9]*); // compile-times' "$file"; then
        sed -i "s|std::hint::black_box([0-9]*); // compile-times|std::hint::black_box($number); // compile-times|" "$file"
    else
        grep -qxF "$anchor" "$file" || { echo "compile-times: no line '$anchor' in $file" >&2; exit 1; }
        awk -v anchor="$anchor" -v number="$number" '
            { print }
            $0 == anchor { print "        std::hint::black_box(" number "); // compile-times" }
        ' "$file" >"$file.new"
        mv "$file.new" "$file"
    fi
}

# Adds a public function, of a name of its own for each run, to the end of `file`.
add_function() {
    local file=$1 run=$2
    sed -i '/^\/\/\/ Added by scripts\/compile-times.sh\.$/,$d' "$file"
    printf '/// Added by scripts/compile-times.sh.\npub fn compile_times_%s() {}\n' "$run" >>"$file"
}

game=games/hunnawood/src/simulation.rs
game_anchor='    fn save_run(&mut self, world: &World) {'
world=crates/slime-world/src/world.rs
world_anchor='    pub fn spawn(&mut self) -> Entity {'
input=crates/slime-input/src/lib.rs

for _ in $(seq "$cold_runs"); do
    rm -rf target
    step 'the workspace from nothing, `--workspace`' --workspace
done
step 'then the game'"'"'s editor, `-p hunnawood-editor`' -p hunnawood-editor
step 'then the game as Play builds it, `-p hunnawood --bin hunnawood`' -p hunnawood --bin hunnawood
step 'then the examples'"'"' editor, `--example editor`' --example editor
step 'then its game as Play builds it, `--workspace --example world`' --workspace --example world

for run in $(seq "$runs"); do
    change_body "$game" "$game_anchor" "$run"
    step 'a function'"'"'s body changed in the game, `-p hunnawood --bin hunnawood`' -p hunnawood --bin hunnawood
    catch_up
    change_body "$world" "$world_anchor" "$run"
    step 'a function'"'"'s body changed in `slime-world`, `-p hunnawood-editor`' -p hunnawood-editor
    step 'then the game as Play builds it' -p hunnawood --bin hunnawood
    catch_up
    change_body "$world" "$world_anchor" "$((run + runs))"
    step 'a function'"'"'s body changed in `slime-world`, `--example editor`' --example editor
    step 'then its game as Play builds it' --workspace --example world
    catch_up
    add_function "$input" "$run"
    step 'a public function added to `slime-input`, `-p hunnawood-editor`' -p hunnawood-editor
    catch_up
done

if grep -q '^\[profile\.shipping\]' Cargo.toml; then
    shipping=(--profile shipping)
    shipping_label='the game for shipping from nothing, `--profile shipping -p hunnawood --no-default-features`'
else
    shipping=(--release)
    shipping_label='the game in release from nothing, no profile for shipping, `--release -p hunnawood --no-default-features`'
fi
for _ in $(seq "$shipping_runs"); do
    rm -rf target
    step "$shipping_label" "${shipping[@]}" -p hunnawood --no-default-features --bin hunnawood
done

load_after=$(cut -d' ' -f1-3 /proc/loadavg)
# The flags that reached rustc for Linux, in cargo's order: the first that is set wins.
if [ -n "${CARGO_ENCODED_RUSTFLAGS:-}" ]; then
    flags="\`CARGO_ENCODED_RUSTFLAGS\` set"
elif [ -n "${RUSTFLAGS:-}" ]; then
    flags="\`RUSTFLAGS\` \`$RUSTFLAGS\`"
elif [ -n "${CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS:-}" ]; then
    flags="Linux's \`$CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS\`"
else
    flags="no flags of rustc set"
fi
cat <<EOF
Commit $commit, on $(date +%F): $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //'),
$(nproc) logical CPUs, $(rustc --version | cut -d' ' -f1-2), $flags;
runs: $runs of each change, $cold_runs of the workspace and $shipping_runs for shipping from nothing;
load average $load_before before, $load_after after.

| Step | Median | Range | Packages compiled |
|---|---|---|---|
EOF
# The steps in their first order, each with the median and range of its times and its packages.
awk -F'\t' '
    !($1 in count) { order[++steps] = $1 }
    { count[$1]++; times[$1, count[$1]] = $2; packages[$1] = packages[$1] (packages[$1] == "" ? "" : " ") $3 }
    END {
        for (s = 1; s <= steps; s++) {
            label = order[s]; n = count[label]
            for (i = 1; i <= n; i++) sorted[i] = times[label, i] + 0
            for (i = 2; i <= n; i++) for (j = i; j > 1 && sorted[j - 1] > sorted[j]; j--) {
                swap = sorted[j]; sorted[j] = sorted[j - 1]; sorted[j - 1] = swap
            }
            median = n % 2 ? sorted[(n + 1) / 2] : (sorted[n / 2] + sorted[n / 2 + 1]) / 2
            split(packages[label], each, " "); same = 1
            for (i = 2; i <= n; i++) if (each[i] != each[1]) same = 0
            range = n > 1 ? sprintf("%.2f to %.2f s", sorted[1], sorted[n]) : "one run"
            compiled = same ? each[1] : packages[label]
            printf "| %s | %.2f s | %s | %s |\n", label, median, range, compiled
        }
    }' "$results"
