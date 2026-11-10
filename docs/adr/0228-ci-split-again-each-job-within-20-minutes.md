# 0228. CI split again, each job within 20 minutes

- Status: Accepted; amends [0104](0104-the-build-ci-and-measurements.md) (the Windows tests in
  three jobs), [0123](0123-the-linux-checks-in-two-jobs.md) and
  [0155](0155-the-editors-design.md) (the Linux checks in four jobs)
- Date: 2026-11-10

## Context

Each job of CI must end within the owner's limit of 20 minutes
([ADR-0175](0175-the-quality-bar.md)). Since C1, `check (headless)` and `windows` went past it: on
`3a8e42e`, 22 min 56 s and 23 min 25 s, the cooked zone taken from its cache
([`measurements.md`](../measurements.md#m6d1d2b2-the-time-that-motion-reads)). The same code took
`check (headless)` 15 min 55 s on one day and 23 min 2 s on another: the runners' speed varies by
about 40 %, so a job needs room under its limit, not just to pass it.

Most of each job is building the workspace from nothing, which no cache keeps:

- `check (headless)` ran clippy (4 min 14 s), clippy for Windows (3 min 35 s), the build
  (4 min 33 s), the tests (6 min 7 s) and rustdoc (1 min 16 s), one after the other;
- `windows` built its tests (9 min 4 s), ran them one at a time (about 7 min), then cooked the zone
  and ran the tests that read it (5 min 23 s).

A cache of the compiled dependencies was rejected twice
([ADR-0081](0081-stage-5a-the-world.md), [ADR-0104](0104-the-build-ci-and-measurements.md)): a
third party's action, the cache's storage, and runs that depend on what the cache holds. The
repository is public: its minutes are free, and up to 20 jobs run side by side. The owner decided
on 2026-11-10.

## Decision

**The jobs are split again, as before:**

- `scripts/check.sh` gains a part, `lint`: formatting, clippy for Linux and for Windows, rustdoc,
  the dependencies, and the documentation's links and lengths. `headless` keeps the build, the
  tests, the source assets and their cooking, the headless samples and the game built for
  shipping. CI's Linux jobs are four: `check (lint)`, `check (headless)`, `check (windowed)` and
  `check (game)`.
- The `windows` job becomes three parts of one matrix: `windows (tests)`, the workspace's tests but
  the renderer's, then the headless samples; `windows (render)`, `slime-render`'s tests;
  `windows (game)`, the zone fetched and cooked and the tests that read it. `windows-release` is
  unchanged.

Every job keeps building from nothing; none depends on a cache but the zone's, which ADR-0104
keeps.

Tried on a branch before `main`, each run green on 2026-11-10, the longest job,
`check (headless)`, took 13 min 54 s
([`measurements.md`](../measurements.md#ci-split-again-2026-11-10)). The runners were faster that
day than on `3a8e42e`; at the slowest speed seen, `check (headless)` would take about 19 minutes.

## Alternatives considered

- **A cache of the compiled dependencies:** the largest gain, about half of each job's build, which
  lasts as the code grows. But ADR-0081 and ADR-0104 rejected it; 10 GB of cache for the whole
  repository, where five jobs of several gigabytes and the cooked zone would evict each other; runs
  that depend on what the cache holds; and either a new action of a third party or a script of
  the project's own around `actions/cache`.
- **Both, the split and the cache:** the fastest, but both costs.
- **The limit raised,** to 30 minutes for instance: nothing to change, but slower answers after
  each push, and the problem back as the code grows.

## Consequences

- Three more jobs, each building the workspace again: more minutes in all, free for a public
  repository.
- `check (headless)` has the least room left. When a job nears its limit again, the same choice
  comes back: split again, or the cache rejected here.
- `scripts/check.sh` without an argument runs the four parts, one after the other, as before.
- On Windows, `slime-render`'s tests build with that package's features alone, and the rest with
  the workspace's but the renderer's: features unify over the packages that a command names.
- An image test that fails publishes its images as `image-test-failures-windows-<part>`.
- Each milestone keeps recording its jobs' times in `measurements.md`.
