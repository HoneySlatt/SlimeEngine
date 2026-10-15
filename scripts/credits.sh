#!/usr/bin/env bash
# Writes the notice that credits Hunna Wood's assets to standard output, from the list of its source
# assets, games/hunnawood/assets/sources.tsv (docs/adr/0070-the-sample-games-source-assets.md), so
# that the notice and the list never disagree. CI puts it beside the game's Windows program:
#   scripts/credits.sh > target/windows/hunnawood-CREDITS.txt
set -euo pipefail
cd "$(dirname "$0")/.."

cat <<'EOF'
Hunna Wood: the credits of its assets

Hunna Wood's zone and hero are built, by a script of SlimeEngine (games/hunnawood/zone/), from free
assets that their authors dedicated to the public domain under CC0 1.0: they ask for nothing, and are
thanked here. The list of every file, with its address and SHA-256, is
games/hunnawood/assets/sources.tsv in SlimeEngine's repository
(https://github.com/HoneySlatt/SlimeEngine).

EOF
awk -F'\t' '$1 == "asset" { printf "- %s, from %s (%s), by %s; %s\n", $2, $3, $4, $5, $6 }' \
    games/hunnawood/assets/sources.tsv
