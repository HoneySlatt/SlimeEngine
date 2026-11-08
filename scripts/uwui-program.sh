#!/usr/bin/env bash
# Measures what a program on uwui compiles (docs/adr/0180-one-canvas-and-the-frameworks-foundations.md):
# the crates it builds, its build time from nothing and after a change of its own code, and the
# size of its binary. The program is the `widgets` example, built in a package of its own, out of
# the workspace, so that the examples' other dependencies, the editor among them, do not count: it
# depends only on what the example uses. Its lock file and its profiles are the workspace's, so
# that it builds the same versions the same way. It says too whether it compiles the renderer, the world or the editor, which
# ADR-0180 wants a program on uwui to leave out.
#
#     scripts/uwui-program.sh [--features <list>]
#
# `--features` gives `slime-app`'s, as the example's `development,accessibility` (the default);
# `--features ""` for none, as a program shipped without the development channel. Every build runs
# from an empty directory of its own under `target/uwui-program`, with every core of the machine:
# run it in the development shell, on a machine at rest.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

features=development,accessibility
while [ $# -gt 0 ]; do
    case $1 in
        --features)
            features=${2?--features needs a list}
            shift
            ;;
        *)
            echo "usage: scripts/uwui-program.sh [--features <list>]" >&2
            exit 2
            ;;
    esac
    shift
done

work=$root/target/uwui-program
rm -rf "$work"
mkdir -p "$work/src/common"
cp examples/widgets.rs "$work/src/main.rs"
cp examples/common/png_file.rs examples/common/widgets_panel.rs "$work/src/common/"
cp Cargo.lock "$work/"
features_list=$(printf '%s' "$features" | sed 's/[^,][^,]*/"&"/g')
cat >"$work/Cargo.toml" <<EOF
[package]
name = "uwui-program"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
slime-app = { path = "$root/crates/slime-app", features = [$features_list] }
slime-color = { path = "$root/crates/slime-color" }
slime-gfx = { path = "$root/crates/slime-gfx" }
slime-platform = { path = "$root/crates/slime-platform" }
slime-render = { path = "$root/crates/slime-render" }
slime-world = { path = "$root/crates/slime-world" }
uwui = { path = "$root/crates/uwui", features = ["editor-look"] }
uwui-render = { path = "$root/crates/uwui-render" }
png = "0.18.1"
tracing = { version = "0.1.44", default-features = false, features = ["std"] }

# Out of the workspace, which holds the directory.
[workspace]

EOF
# The workspace's profiles, which a package outside it does not inherit.
sed -n '/^\[profile\./,/^$/p' "$root/Cargo.toml" | grep -v '^#' >>"$work/Cargo.toml"
# The example's `#[path]`s name `common/`, beside it.
cd "$work"
# The workspace's lock, adjusted for this package alone, offline: no version changes.
cargo metadata --offline --format-version 1 >/dev/null

# The package itself is not counted.
crates=$(cargo tree --offline -e normal,build --prefix none --format '{p}' | sed 's/ (.*//; s/ (\*)//' |
    sort -u | grep -v '^uwui-program ')
echo "features of slime-app: ${features:-none}"
echo "crates compiled: $(printf '%s\n' "$crates" | wc -l)"
echo "  of the engine: $(printf '%s\n' "$crates" | grep -c -E '^(slime-|uwui)')"
for crate in slime-render slime-world slime-editor; do
    printf '  %s: ' "$crate"
    printf '%s\n' "$crates" | grep -q "^$crate " && echo compiled || echo "not compiled"
done
# The engine's crates are members of the workspace, which `[profile.dev.package."*"]` leaves
# unoptimised; here they are not, and would be optimised with the dependencies (ADR-0221).
for crate in $(printf '%s\n' "$crates" | grep -E '^(slime-|uwui)' | cut -d' ' -f1); do
    printf '\n[profile.dev.package.%s]\nopt-level = 0\n' "$crate" >>Cargo.toml
done

seconds() { python3 -I -c 'import sys, time; print(f"{time.monotonic():.3f}")'; }
build() {
    local start end
    start=$(seconds)
    cargo build --offline --quiet "$@"
    end=$(seconds)
    python3 -I -c "print(f'{$end - $start:.1f} s')"
}
for profile in dev release; do
    rm -rf target
    time=$(build --profile "$profile")
    echo "$profile, from nothing: $time"
    touch src/main.rs
    time=$(build --profile "$profile")
    echo "$profile, after a change of its own code: $time"
done
binary=target/release/uwui-program
echo "release binary: $(stat -c %s "$binary") bytes"
cp "$binary" target/stripped
strip target/stripped
echo "release binary stripped: $(stat -c %s target/stripped) bytes"
