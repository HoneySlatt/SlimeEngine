# 0177. Audits and releases

- Status: Accepted; amends [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)
  (each stage closes with an audit, not only 0.4) and [0155](0155-the-editors-design.md) (M6c.9
  follows the release's checklist)
- Date: 2026-11-05

## Context

A reviewing agent checks each milestone against [AGENTS.md](../../AGENTS.md) and the ADRs before
its report. Nothing examines a whole stage: the last audit of the whole engine was the audit of
0.4, on 2026-10-18 and 2026-10-19
([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)), whose findings left
are in [`debt.md`](../debt.md). Stage 6c adds tens of thousands of
lines. A milestone's review sees its own changes, not what they do together: a budget that
drifts over several milestones, a test that no longer fails when its behaviour breaks, a document
that each milestone left a little wrong.

A release, so far, is a tag on a green commit, with its changelog
([`CONTRIBUTING.md`](../../CONTRIBUTING.md#releasing)). Nothing checks that a user's data of the
previous release still opens, or that a crash in a released program can be read.

The owner decided on 2026-11-05, with the quality bar ([ADR-0175](0175-the-quality-bar.md)).

## Decision

**Three depths of examination.**

- **A review** closes each milestone, as now: its changes, against AGENTS.md, the ADRs and the
  quality bar.
- **A stage audit** closes each stage: the whole engine, in every dimension below.
- **A release audit** comes before each release: the stage audit, and the release's checklist.

**The dimensions of a stage audit.** Correctness; edge cases, from empty and huge to malformed,
interrupted and concurrent; performance, every budget of the quality bar measured again;
resources at rest; security and privacy; the safety of the user's data, formats versioned,
migrations tested, writes atomic; tests, each of which must fail when what it names breaks, checked
by mutation with `cargo-mutants`; readability; documentation; dependencies; accessibility; debt.

**How.** The audit's scope, its dimensions and its commit are written first. One reviewing agent
for each dimension, with fresh eyes, on the strongest model available, is given the code and the
bar, never the conclusions expected. Every finding is verified before it counts: a test that
fails, a measurement, or the exact line and the reasoning; a finding not verified is dropped or
marked a doubt. Findings are ranked: data lost, a crash or a breach first, then wrong behaviour,
then a budget missed, then readability and documentation. The owner decides the fixes that change
direction, one at a time; the others are planned. The audit is recorded in
`docs/audits/YYYY-MM-DD-<scope>.md`: its scope, its findings, the decisions and what was fixed; what
stays open goes to `debt.md`.

**The release's checklist.** The changelog; the version; the update from the previous release
tested, its user's data, saves, levels, layouts and keys, opened by the new one; the programs'
debug symbols kept beside the release, so that a crash report's backtrace can be read; a run of the
programs on each platform. Packages signed and built reproducibly are debt until Stage 11.

**At each stage's end**, the dependencies and `flake.lock` are updated, each in a commit of its own.

**For Stage 6c.** Its audit is milestone M6c.Q5, after M6c.8 and before M6c.9, which follows the
checklist for 0.7. `cargo-mutants` enters the development shell, `dependencies.md` and
`CONTRIBUTING.md` with M6c.Q5, and
`CONTRIBUTING.md`'s "Releasing" gains the checklist with M6c.9.

## Alternatives considered

- **The milestones' reviews alone**: no time set apart, but no one sees what the milestones do
  together.
- **One agent for the whole audit**: simpler, but one context filled by every dimension, and one
  model's blind spots in all of them.
- **Findings counted as found**: a faster report, but reviewers' mistakes counted as bugs;
  verification keeps the owner's decisions on facts.
- **Mutation testing at each push**: `cargo-mutants` builds and runs the tests once for each
  mutation, likely hours on this workspace; once a stage is enough to find tests that check nothing.
- **Signed packages now**: a certificate for Windows costs money each year, and the programs are
  not yet installed by users; Stage 11 makes the engine ready for production.

## Consequences

- Each stage ends with an audit milestone before its release.
- `docs/audits/` starts with the audit of Stage 6c.
- A release of the engine can be read when it crashes, and opens what its predecessor wrote.
