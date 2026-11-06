# 0192. The budget of a cold start

- Status: Accepted; amends [0175](0175-the-quality-bar.md) (a cold start's budget)
- Date: 2026-11-06

## Context

[ADR-0175](0175-the-quality-bar.md) left cold starts to be measured first, their figure proposed
after. [ADR-0189](0189-first-measures-of-time.md) defined a *cold* start: from the process's launch
to its first useful frame, the first image presented, after the system dropped its caches, as after
a reboot; `scripts/start-times.sh --cold` drops them before each run, through `sudo`.

The owner measured it on 2026-11-06 on the RX 6900 XT
([`measurements.md`](../measurements.md#m6cq2d-cold-starts-rx-6900-xt)): the editor's first image
about 0.41 s after its launch, the game's editor on its zone about 0.44 s; warm, under 0.2 s. The
owner chose the figure the same day.

## Decision

The editor and applications, cold, from their launch to their first useful frame: within 500 ms on
the high tier. The warm start keeps its 150 ms.

## Alternatives considered

- **1 s**: room for years, but a cold start that grows by 400 ms would pass without debt.
- **300 ms**: passed today; debt to pay by reading less from the disk at the start, a smaller
  program, pipelines kept in a cache, steps run side by side.

## Consequences

- Met on the owner's machine; the high tier's other machines and the other tiers are not measured.
- A cold start is measured by a person, who gives `sudo`: no check runs it.
