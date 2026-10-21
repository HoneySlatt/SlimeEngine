# 0123. The Linux checks in two jobs

- Status: Accepted; amends [0007](0007-documentation-tooling-and-ci.md) (CI runs the checks' script
  in two parts) and [0104](0104-the-build-ci-and-measurements.md) (CI's jobs)
- Date: 2026-10-21

## Context

CI runs `scripts/check.sh` on Linux, in the same Nix shell that contributors use, so that a local
run and CI check the same things ([ADR-0007](0007-documentation-tooling-and-ci.md)); the owner
allows each of CI's jobs 20 minutes. [ADR-0081](0081-stage-5a-the-world.md) split the Windows job
in two jobs that run in parallel past 20 minutes, which
[ADR-0104](0104-the-build-ci-and-measurements.md) did.

M6a.6 added a test that plays an agent's session through the engine's MCP server
([ADR-0122](0122-the-development-channel.md)): it drives *Hunna Wood* in a headless Weston
compositor on lavapipe, the Vulkan driver that renders on the CPU. Each of its requests waits for a
frame of the zone, which takes seconds there. On its first run, the Linux job took 24 minutes 37
seconds, the session 8 minutes of it, from 14 minutes 14 seconds before. M6a.7 to M6a.10 each
expose a system to the development channel, and their tests drive the game the same way.

## Decision

**The script has two parts, and CI runs each in a job of its own, side by side.**

- `scripts/check.sh headless` runs every check that needs no compositor: formatting, clippy for
  Linux and Windows, the build, the tests, rustdoc, `cargo deny`, the assets' checks and cooks, the
  tests of the cooked zone, the headless samples, the game built for shipping and the documents'
  links.
- `scripts/check.sh windowed` runs what draws in headless Weston compositors: the windowed samples,
  the game, the windowed lifecycle test and the agent's session. Run alone, it first cooks what
  they draw, the sample and the game's zone, which CI's cache holds already.
- Without an argument, the script runs both, one after the other, as before: a developer runs one
  command, and a local run does what the two jobs do together.

The workflow runs the two parts as a matrix of one job, `check (headless)` and `check (windowed)`,
with the same caches of the zone. A failure of one does not stop the other.

## Alternatives considered

- **A shorter agent's session**, with half of its requests: about 4 minutes saved, the job near 20
  minutes still, and each milestone that exposes a system to the channel would bring it back.
- **A budget of 30 minutes for the job**: no work, but the job's own limit is 30 minutes, which
  leaves no margin, and each push waits longer for its checks.
- **A job for each test in Weston**: more parallel, but a build for each, and a script that CI
  would no longer run in the parts that a developer runs.

## Consequences

- The Linux checks take the time of the longer part, and the build is made twice, once in each
  job; CI uses one more runner.
- A check is added to the part that it belongs to: one that needs a compositor goes to `windowed`.
- The jobs are named `check (headless)` and `check (windowed)`.
