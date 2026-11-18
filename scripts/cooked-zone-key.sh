#!/usr/bin/env bash
# Prints `hash=<digest>`, the digest of what makes the cooked zone's bytes, which CI's key of the
# cooked zone holds beside the runner's system and processor (docs/adr/0144-the-cooked-zones-key-in-ci.md):
#
# - the packages that the cooker is built from, at the versions that Cargo.lock pins and with their
#   features, in place of the whole lock file, which a dependency of any other crate changes;
# - the sources and manifests of the workspace's crates among them, taken from the same tree, so
#   that a crate that the cooker comes to depend on joins the key by itself;
# - the zone's list of sources, its metadata and scene files, and the toolchain.
#
# This file alone says what the key holds; each job of CI appends its output to `$GITHUB_OUTPUT`.
# It needs cargo, git and sha256sum, which the Nix shell and Git for Windows provide.
set -euo pipefail
cd "$(dirname "$0")/.."

tree=$(cargo tree --locked -p slime-cook --edges normal,build,features --prefix none)

# The workspace's crates are the packages that the tree gives a path, named after their directory
# (docs/ARCHITECTURE.md, "Repository layout"). A package listed again is marked `(*)`.
mapfile -t crates < <(sed -nE 's/^([a-z0-9_-]+) v[^ ]+( \(proc-macro\))? \(([A-Za-z]:)?[\/\\].*/\1/p' <<< "$tree" |
    LC_ALL=C sort -u)
# A change of cargo's output that this reading misses would leave every crate's sources out of the
# key without a word: the cooker's own crate stands for them all.
if ! printf '%s\n' "${crates[@]}" | grep -qx slime-cook; then
    echo "the cooker's crates cannot be read from cargo tree's output" >&2
    exit 1
fi
sources=()
for crate in "${crates[@]}"; do
    if [ ! -f "crates/$crate/Cargo.toml" ]; then
        echo "the cooker depends on the workspace's $crate, which is not crates/$crate" >&2
        exit 1
    fi
    sources+=("crates/$crate/src" "crates/$crate/Cargo.toml" "crates/$crate/build.rs")
done
sources+=(games/hunnawood/assets/sources.tsv 'games/hunnawood/assets/zone/**.meta'
    'games/hunnawood/assets/zone/**.scene' rust-toolchain.toml)

# The packages, the crates of the workspace named by their versions alone, not by where they lie,
# which differs from one machine to the next; then each file by its path and its content.
digest=$( {
    sed -E 's/ \((\*|proc-macro)\)//g; s/ \(([A-Za-z]:)?[\/\\][^)]*\)//' <<< "$tree" | LC_ALL=C sort -u
    git ls-files -z -- "${sources[@]}" | LC_ALL=C sort -z | xargs -0 sha256sum
} | sha256sum | cut -c1-32)
echo "hash=$digest"
