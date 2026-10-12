# 0001. Record architecture decisions

- Status: Accepted
- Date: 2026-10-12

## Context

SlimeEngine is meant to live for years and to be read and changed by people who were not there
when its decisions were made. It is also developed mostly by AI agents that keep no memory from
one working session to the next. A decision that exists only in a conversation is lost, and
the next contributor either re-decides it, possibly differently, or works around it.

## Decision

Every architecture decision is recorded as an Architecture Decision Record (ADR) in `docs/adr/`,
following the process and format described in [`docs/adr/README.md`](README.md):

- one decision per ADR, numbered in order of acceptance;
- each ADR states its context, the decision, the alternatives rejected and the consequences,
  including the cost of reversing it;
- options are proposed by whoever needs the decision; the project owner decides;
- the ADR is written in the change that first applies the decision;
- accepted ADRs are superseded by new ADRs, never rewritten.

Changes to [`docs/ROADMAP.md`](../ROADMAP.md) (a stage reshaped, merged, reordered or dropped)
are architecture decisions and follow the same process.

## Alternatives considered

- **Decisions in commit messages or pull request descriptions.** Hard to find once the history
  grows, and they describe a change rather than the reasoning that still applies.
- **A single design document updated in place.** It records the current state but loses the
  alternatives that were rejected and why, which is exactly what stops a decision from being
  re-litigated.
- **No formal record.** Not viable for a project whose main contributors have no memory between
  sessions.

## Consequences

- Contributors must read `docs/adr/` before changing architecture, and must not re-decide what an
  ADR already covers without writing a superseding ADR.
- Every architecture change carries a small writing cost.
- Reversing this decision would mean moving the reasoning elsewhere; nothing in the code depends
  on it.
