# 0291. A panic says why: no bare `unwrap` outside tests

- Status: Accepted
- Date: 2026-11-16

## Context

[ADR-0282](0282-errors-logging-assertions-uwuis-public-enumerations-open-to-new-cases.md) has a
violated invariant panic, and a failure that depends on the environment returned as an error. No
tool tells the two apart. On 2026-11-16, outside tests, the workspace held 196 calls that may
panic on an `Option` or a `Result`: 194 `expect("…")`, each saying which invariant it holds, and 2
bare `unwrap()`, one in the benchmarks and one in a helper of `slime-world`'s tests outside a test
function. Clippy's `unwrap_used` names a bare `unwrap`; its `allow-unwrap-in-tests` leaves tests
alone. Clippy's `missing_panics_doc`, already on, asks a public function that may panic to say so.

## Decision

The owner's decision of 2026-11-16.

**Outside tests, a call that may panic says why:** `expect("…")`, whose message names the invariant
that it holds, never a bare `unwrap()`. The workspace turns on `clippy::unwrap_used`, which
`scripts/check.sh` makes an error; `clippy.toml` allows it in tests, where a failure says enough.
An `expect` never stands on what comes from outside the program, which returns an error
([ADR-0096](0096-the-files-the-engine-reads.md)); review checks it, and the tests of damaged files
([ADR-0102](0102-a-damaged-file-gives-an-error.md)) catch what review misses.

## Alternatives considered

- **`expect` refused too, each allowed with its reason:** every panic justified by an attribute,
  but 194 attributes that say what their messages already say.
- **No lint:** nothing to do, but bare `unwrap` free to come back.

## Consequences

- The two bare `unwrap()` became `expect`.
- An `expect` on outside input is found by review and by the damaged files' tests, not by a lint.

## Validation and reconsideration

Counted: the calls by `clippy::unwrap_used` and `clippy::expect_used` on the workspace's libraries
and programs, then on every target with `allow-unwrap-in-tests`.

Proposing another decision is justified if an `expect` on outside input reaches a release.
