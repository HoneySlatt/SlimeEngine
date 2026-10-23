#!/usr/bin/env bash
# Makes the fonts that slime-ui's tests and benchmarks read, crates/slime-ui/tests/fonts, from the
# Noto releases that crates/slime-ui/tests/fonts/README.md names (docs/adr/0133-font-files-under-
# the-open-font-license.md): each font cut down to the characters of crates/slime-ui/tests/texts and
# what its layout tables reach from them, without hinting, at its regular weight, but Noto Sans,
# which keeps its weight axis so that the tests can draw it bold. Usage:
#   scripts/subset-fonts.sh <noto> <noto-cjk> <noto-emoji>
# where each argument is a directory that holds the release's files under the names below, checked
# against their SHA-256: on NixOS, share/fonts/noto of the packages noto-fonts and
# noto-fonts-color-emoji, and share/fonts/opentype/noto-cjk of noto-fonts-cjk-sans. It needs
# HarfBuzz's hb-subset (on NixOS, `nix shell nixpkgs#harfbuzz.dev`) and sha256sum. The bytes that it
# makes may differ from one version of hb-subset to another: README.md says which one made those in
# the repository. It writes the fonts once all of them are made.
set -euo pipefail

(($# == 3)) || {
  echo "usage: scripts/subset-fonts.sh <noto> <noto-cjk> <noto-emoji>" >&2
  exit 2
}
command -v hb-subset >/dev/null || {
  echo "hb-subset is not on the path: on NixOS, nix shell nixpkgs#harfbuzz.dev" >&2
  exit 1
}
# The directories, as the caller named them, before the script moves to the repository.
noto=$(realpath "$1")
cjk=$(realpath "$2")
emoji=$(realpath "$3")
cd "$(dirname "$0")/.."
hb-subset --version | head -n 1
fonts=crates/slime-ui/tests/fonts
texts=crates/slime-ui/tests/texts

# Each source: its file, its SHA-256.
check() {
  local file=$1 sum=$2
  [[ -f $file ]] || {
    echo "$file: no such file" >&2
    exit 1
  }
  [[ $(sha256sum "$file" | cut -d' ' -f1) == "$sum" ]] || {
    echo "$file: not the release that $fonts/README.md names (SHA-256)" >&2
    exit 1
  }
}
check "$noto/NotoSans.ttf" 511cc04b442ffcc3b24d65c2cc780bdffaf482b20ba0aebdc8f27b37017ff0f5
check "$noto/NotoSansArabic.ttf" 3b735d0b7f04ddd5c6a3d7fafba8d8216420d3cbbba93ded80f34ba37369de00
check "$noto/NotoSansHebrew.ttf" 2a0af06d3e959ebc3f50a8c0042c24b671b6b3283d1ebbf3e7d39ef2b4ce62ba
check "$noto/NotoSansDevanagari.ttf" 9f82923465ef82fb86c6b99cbdd4947ba832fe3335948de815b18adfd75947c7
check "$noto/NotoSansThai.ttf" 70bcda1aae0976b35ff22660b918d0f610a4bd5c7136cf7313072c0fe0186825
check "$cjk/NotoSansCJK-VF.otf.ttc" d3d8256cdec8dbcb3552284bc6b20c734dd60c2ee9df83b5758e34807c4bac32
check "$emoji/NotoColorEmoji.ttf" 3a44421dc3ce672f80fa0d4fdf7da5b583494ab4517178747f5fa6f287db5c1d

made=$(mktemp -d)
trap 'rm -rf "$made"' EXIT
text=$made/texts.txt
cat "$texts"/*.txt >"$text"

# hb-subset with the options that every font shares: every layout feature kept, so that shaping
# gives what the whole font gives for these characters, and every name, so that a family keeps the
# typographic name that fontique names it by, where the legacy one would name a weight.
subset() {
  hb-subset --text-file="$text" --layout-features='*' --name-IDs='*' "$@"
}
subset --no-hinting --variations='wdth=drop' -o $made/NotoSans.ttf "$noto/NotoSans.ttf"
for name in NotoSansArabic NotoSansHebrew NotoSansDevanagari; do
  subset --no-hinting --variations='wght=400 wdth=100' -o $made/$name.ttf "$noto/$name.ttf"
done
# Shaping splits Thai's sara am, U+0E33, into the nikhahit, U+0E4D, and sara aa: the nikhahit is
# drawn, though the texts never hold it.
subset --no-hinting --variations='wght=400 wdth=100' --unicodes+=0E4D -o $made/NotoSansThai.ttf \
  "$noto/NotoSansThai.ttf"
# The collection's faces 0 and 2: Japanese and Simplified Chinese.
subset --no-hinting --face-index=0 --variations='wght=400' -o $made/NotoSansCJKjp.otf \
  "$cjk/NotoSansCJK-VF.otf.ttc"
subset --no-hinting --face-index=2 --variations='wght=400' -o $made/NotoSansCJKsc.otf \
  "$cjk/NotoSansCJK-VF.otf.ttc"
# Bitmaps, which have no hinting.
subset -o $made/NotoColorEmoji.ttf "$emoji/NotoColorEmoji.ttf"
mv $made/*.ttf $made/*.otf $fonts/
ls -l $fonts
