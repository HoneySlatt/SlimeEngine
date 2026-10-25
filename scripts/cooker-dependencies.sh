#!/usr/bin/env bash
# Prints `hash=<digest>`, a digest of the packages that the cooker is built from, at the versions
# that Cargo.lock pins and with their features: what CI's key of the cooked zone holds in place of
# the whole lock file, which a dependency of any other crate changes
# (docs/adr/0144-the-cooked-zones-key-in-ci.md). CI appends its output to `$GITHUB_OUTPUT`. It
# needs cargo and sha256sum, which the Nix shell and Git for Windows provide.
set -euo pipefail
cd "$(dirname "$0")/.."

# The crates of the workspace are named by their versions alone, not by where they lie, which
# differs from one machine to the next: the key holds their sources apart. A package listed again
# is marked `(*)`, which the sort makes one line, in the order of bytes whatever the locale.
digest=$(cargo tree --locked -p slime-cook --edges normal,build,features --prefix none |
    sed -E 's/ \((\*|proc-macro)\)//g; s/ \(([A-Za-z]:)?[\/\\][^)]*\)//' |
    LC_ALL=C sort -u |
    sha256sum |
    cut -c1-32)
echo "hash=$digest"
