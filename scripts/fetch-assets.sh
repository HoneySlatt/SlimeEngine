#!/usr/bin/env bash
# Fetches the source assets of Hunna Wood that games/hunnawood/assets/sources.tsv lists
# (docs/adr/0070-the-sample-games-source-assets.md) into games/hunnawood/assets/: each file from
# its mirror, or from where it was published if the mirror fails, checked against its SHA-256.
# A file already there with the right SHA-256 is not downloaded again. With PREFIX arguments, only
# the files whose path starts with one of them are fetched, for example:
#   scripts/fetch-assets.sh              # everything, the sources of the zone's Blender script too
#   scripts/fetch-assets.sh zone/        # what the cooker cooks
# Before fetching anything, it checks the whole list, and refuses an asset under another licence
# than CC0-1.0. It needs curl and sha256sum, which Git for Windows provides too.
set -euo pipefail
cd "$(dirname "$0")/.."

assets=games/hunnawood/assets
list=$assets/sources.tsv

fail() {
  echo "$list:$1: $2" >&2
  exit 1
}

# The list first, whole: a wrong line stops the script before any download.
declare -A seen=()
line=0
asset=
while IFS= read -r text || [[ -n $text ]]; do
  line=$((line + 1))
  [[ -z $text || $text == \#* ]] && continue
  IFS=$'\t' read -r -a fields <<<"$text"
  case ${fields[0]} in
    asset)
      ((${#fields[@]} == 7)) || fail $line "an asset line has 7 fields"
      [[ ${fields[5]} == CC0-1.0 ]] ||
        fail $line "asset ${fields[1]} is under ${fields[5]}: only CC0-1.0 is accepted (ADR-0070)"
      asset=${fields[1]}
      ;;
    file)
      ((${#fields[@]} == 6)) || fail $line "a file line has 6 fields"
      [[ -n $asset ]] || fail $line "a file line comes after the line of its asset"
      path=${fields[1]}
      [[ $path != /* && /$path/ != */../* && /$path/ != */./* ]] ||
        fail $line "$path is not a path inside $assets"
      [[ -z ${seen[$path]:-} ]] || fail $line "$path is listed twice"
      seen[$path]=1
      [[ ${fields[2]} =~ ^[0-9]+$ ]] || fail $line "the size of $path is not a number"
      [[ ${fields[3]} =~ ^[0-9a-f]{64}$ ]] || fail $line "the SHA-256 of $path is not 64 hex digits"
      [[ ${fields[4]} == https://* && ${fields[5]} == https://* ]] ||
        fail $line "the addresses of $path are not https"
      ;;
    *) fail $line "a line starts with asset or file" ;;
  esac
done <"$list"

# Whether the file `$1` exists and has the SHA-256 `$2`.
matches() {
  [[ -f $1 ]] && echo "$2  $1" | sha256sum --check --quiet --status
}

fetched=0
kept=0
while IFS=$'\t' read -r kind path _ sha256 published mirror; do
  [[ $kind == file ]] || continue
  if (($# > 0)); then
    wanted=
    for prefix in "$@"; do
      [[ $path == "$prefix"* ]] && wanted=1
    done
    [[ -n $wanted ]] || continue
  fi
  target=$assets/$path
  if matches "$target" "$sha256"; then
    kept=$((kept + 1))
    continue
  fi
  mkdir -p "$(dirname "$target")"
  for address in "$mirror" "$published"; do
    if curl --fail --silent --show-error --location --retry 3 --output "$target.part" "$address" &&
      matches "$target.part" "$sha256"; then
      mv "$target.part" "$target"
      break
    fi
    echo "$path: $address did not give the file listed" >&2
  done
  rm -f "$target.part"
  matches "$target" "$sha256" || {
    echo "$path: no address gave the file listed" >&2
    exit 1
  }
  fetched=$((fetched + 1))
done < <(grep -v '^#' "$list")
echo "$assets: $fetched fetched, $kept already there"
