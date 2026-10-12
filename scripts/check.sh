#!/usr/bin/env bash
# Runs every automated check of the definition of done (AGENTS.md). CI runs this script
# unchanged, inside the Nix development shell: `nix develop -c scripts/check.sh`.
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

step "Formatting"
cargo fmt --all --check

step "Clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings

step "Clippy for Windows (compiles the Windows code paths without a Windows machine)"
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings

step "Build"
cargo build --workspace --all-targets --locked

step "Tests (GPU tests on lavapipe, the Vulkan driver that renders on the CPU, as in CI)"
VK_DRIVER_FILES="${SLIME_LAVAPIPE_ICD:?run this script inside nix develop}" \
    cargo test --workspace --locked

step "Rustdoc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

step "Dependencies (licenses, advisories, sources)"
cargo deny --locked check

step "Headless samples (no display server)"
env -u WAYLAND_DISPLAY -u DISPLAY cargo run --locked --example headless -- --frames 600
env -u WAYLAND_DISPLAY -u DISPLAY cargo run --locked --example window -- --headless --frames 600
env -u WAYLAND_DISPLAY -u DISPLAY VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
    cargo run --locked --example offscreen -- --output target/triangle.png

step "Windowed sample in a headless Wayland compositor (Weston), drawn with lavapipe"
# A private runtime directory keeps this compositor apart from the one the developer may be using.
runtime=$(mktemp -d)
XDG_RUNTIME_DIR=$runtime weston --backend=headless --socket=slime-check --idle-time=0 \
    >"$runtime/weston.log" 2>&1 &
weston=$!
trap 'kill "$weston" 2>/dev/null; rm -rf "$runtime"' EXIT
for _ in $(seq 100); do
    [ -S "$runtime/slime-check" ] && break
    sleep 0.1
done
env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=slime-check \
    VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo run --locked --example window -- --frames 120 ||
    { cat "$runtime/weston.log"; exit 1; }

step "Documentation links"
# Tracked and new, non-ignored Markdown files. External links are not checked: they fail for
# reasons unrelated to the change being checked.
mapfile -t markdown < <(git ls-files --cached --others --exclude-standard '*.md')
lychee --offline --include-fragments --no-progress "${markdown[@]}"

step "All checks passed"
