# 0188. A light check for documentation

- Status: Accepted; amends [0007](0007-documentation-tooling-and-ci.md) (a change of documentation
  alone runs only the link check)
- Date: 2026-11-05

## Context

CI ran its five jobs on every push, a change of `docs/PLAN.md` alone included: about 14 minutes of
runners, the longest job's, to check again code that had not changed. Of its checks, only the links
between Markdown files (lychee, [ADR-0007](0007-documentation-tooling-and-ci.md)) read such a
change.

Not every Markdown file is documentation alone: each crate's `README.md` is its rustdoc
(`#![doc = include_str!("../README.md")]`), whose code examples run as tests, and which rustdoc
checks with warnings as errors.

The owner chose on 2026-11-05 a light job for documentation.

## Decision

- The `CI` workflow runs on a push or a pull request that changes anything but Markdown files
  outside `crates/` (its `paths` filter). Its weekly run stays whole.
- A `Docs` workflow, `.github/workflows/docs.yml`, runs on a push or a pull request that changes a
  Markdown file, or itself. Its one job checks the links between Markdown files, as the headless
  part of `scripts/check.sh` does.
- `scripts/check.sh docs` runs that check alone, and needs only lychee, not the development shell.
  The job takes lychee from the nixpkgs that `flake.lock` pins
  (`nix shell --inputs-from . nixpkgs#lychee`), the development shell's version.

## Alternatives considered

- **Every push runs CI whole**: nothing to maintain, but 14 minutes of runners and a wait for each
  change of a plan or a measurement.
- **One workflow whose jobs each look at what changed**: one file, but each job then needs a step
  that compares the commits, or a new action that does.
- **lychee's own action**: a new dependency of CI, with its own version of lychee, which could judge
  links unlike the development shell's.

## Consequences

- A change of documentation alone is checked in a minute or so, by one job.
- A change of documentation and code together runs both workflows: the links are checked twice.
- A pull request that changes documentation alone gets no status from the `CI` workflow: a rule of
  the repository that required its jobs would block it. None does.
- A Markdown file that something else reads, outside `crates/`, would need the filter widened: none
  does today.
