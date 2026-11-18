#!/usr/bin/env bash
# Builds the test programs that `cargo test` builds with the arguments given, without running them,
# and prints each directory that holds one, once a line. Cargo puts each test program in a
# directory of its own in its build directory, whose layout it keeps internal (the Cargo book's
# "Build cache"): it names them as it builds them. CI uses it on Windows to put DXC beside the
# tests (docs/adr/0066-shipping-dxc-beside-the-programs.md):
#
#   { scripts/test-directories.sh -p slime-render; echo target/debug/examples; } |
#       xargs -d '\n' scripts/fetch-dxc.sh
#
# Run the tests with the same arguments then, or cargo builds them again elsewhere: features change
# what it builds. It fails when cargo fails or names no test program.
set -euo pipefail
cd "$(dirname "$0")/.."

messages=$(cargo test "$@" --no-run --message-format=json)
# Each program that cargo built names its path, whose backslashes JSON doubles on Windows; a library
# names none, `"executable":null`.
programs=$(grep -o '"executable":"[^"]*"' <<< "$messages" |
    sed -e 's/^"executable":"//' -e 's/"$//' -e 's/\\\\/\\/g' || true)
if [ -z "$programs" ]; then
    echo "cargo test $* named no test program" >&2
    exit 1
fi
# In Git Bash on Windows, the paths in its own form, which its tools read.
if command -v cygpath > /dev/null; then
    programs=$(cygpath -u -f - <<< "$programs")
fi
xargs -d '\n' dirname <<< "$programs" | sort -u
