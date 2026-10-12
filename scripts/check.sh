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

step "Build"
cargo build --workspace --all-targets --locked

step "Tests"
cargo test --workspace --locked

step "Rustdoc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

step "Dependencies (licenses, advisories, sources)"
cargo deny --locked check

step "Headless sample (no display server)"
env -u WAYLAND_DISPLAY -u DISPLAY cargo run --locked --example headless -- --frames 600

step "Documentation links"
# Tracked and new, non-ignored Markdown files. External links are not checked: they fail for
# reasons unrelated to the change being checked.
mapfile -t markdown < <(git ls-files --cached --others --exclude-standard '*.md')
lychee --offline --include-fragments --no-progress "${markdown[@]}"

step "All checks passed"
