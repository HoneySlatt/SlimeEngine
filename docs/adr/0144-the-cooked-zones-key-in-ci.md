# 0144. The cooked zone's key in CI

- Status: Accepted; amends [0104](0104-the-build-ci-and-measurements.md) (the key of the cooked
  zone holds the packages that the cooker is built from, not the whole lock file and workspace
  manifest)
- Date: 2026-10-25

## Context

[ADR-0104](0104-the-build-ci-and-measurements.md) keeps the sample game's cooked zone in CI's cache,
keyed on what makes its bytes, the workspace's `Cargo.toml` and `Cargo.lock` among them. Both files
change whenever any crate of the workspace gains or changes a dependency, the cooker's or not: each
job then cooks the zone again, which takes minutes. From M6b.2 to M6b.5b, such runs took the
`check (windowed)` and `windows` jobs past the limit of 20 minutes that the project keeps
(`docs/PLAN.md`, known debt), M6b.5b's by up to 51 seconds, when the cooker had not changed. The
owner chose on 2026-10-25 to narrow the key.

## Decision

**The key holds the packages that the cooker is built from.** `scripts/cooker-dependencies.sh`
prints a digest of `cargo tree` for `slime-cook`: every package that its build reaches, at the
version that `Cargo.lock` pins, with the features turned on, the crates of the workspace named by
their versions alone. Each job writes it to its step's output, and the key of the cooked zone holds
it in place of `Cargo.toml` and `Cargo.lock`. The rest of the key is unchanged: the zone's sources'
list, their metadata and scene files, the sources of the cooker's crates, the toolchain, the
runner's system and processor.

## Alternatives considered

- **Keep the whole lock file**, as before: simple, but every dependency added to another crate, as
  the next milestones add, cooks the zone again in each job.
- **Raise the limit to 25 minutes**: no change to CI, but the jobs grow with each milestone, and
  cooking again when nothing that the cooker reads changed is wasted.
- **A key of the cooker's dependencies written by hand**: no step, but a list that drifts when the
  cooker gains a dependency.

## Consequences

- A milestone that adds a dependency to another crate, or changes the workspace's manifest, keeps
  the cooked zone; one that changes a package the cooker is built from, or its features, cooks it
  again, as before.
- Each job runs `cargo tree` before its cache step: the Linux jobs in the Nix shell, which they
  then enter first there, the Windows jobs with the toolchain they installed.
- The workspace's profiles leave the key: they change how fast the cooker runs, not what it writes;
  and the cook checks each file that it reuses from the cache before reusing it.
