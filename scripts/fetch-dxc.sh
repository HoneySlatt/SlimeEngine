#!/usr/bin/env bash
# Downloads DXC, the shader compiler of Direct3D 12 that the engine uses when its library lies
# beside the program (docs/adr/0066-shipping-dxc-beside-the-programs.md), checks the archive
# against its SHA-256, and copies `dxcompiler.dll` and its licence into each directory given. CI
# runs it on Windows; a contributor on Windows runs it from Git Bash, for example:
#   scripts/fetch-dxc.sh target/debug/deps target/debug/examples target/debug
# It needs curl, sha256sum and PowerShell, which Git for Windows and Windows provide. The archive
# and what it holds are kept in target/, so that a second run downloads nothing.
set -euo pipefail
cd "$(dirname "$0")/.."

# The release of DXC that the engine ships, and the SHA-256 of its archive as GitHub publishes it.
version=v1.9.2609
archive=dxc_2026_09_29.zip
sha256=ad31b1fc8443175d204f77a611fdb3ef2ec42759bdc2f1167368de24a4a7e7f1

if (($# == 0)); then
  echo "usage: $0 DIRECTORY..." >&2
  exit 2
fi
work=target/dxc-$version
# The library is copied out of the archive last, once the archive has been checked: its presence
# means that the work is done.
if [[ ! -f $work/dxcompiler.dll ]]; then
  mkdir -p "$work"
  curl --fail --silent --show-error --location --output "$work/$archive" \
    "https://github.com/microsoft/DirectXShaderCompiler/releases/download/$version/$archive"
  echo "$sha256  $work/$archive" | sha256sum --check --quiet
  powershell.exe -NoProfile -Command \
    "Expand-Archive -Force -Path '$work/$archive' -DestinationPath '$work/archive'"
  cp "$work/archive/LICENSE-LLVM.txt" "$work/"
  cp "$work/archive/bin/x64/dxcompiler.dll" "$work/"
fi
for directory in "$@"; do
  mkdir -p "$directory"
  cp "$work/dxcompiler.dll" "$directory/"
  cp "$work/LICENSE-LLVM.txt" "$directory/dxcompiler-LICENSE.txt"
done
