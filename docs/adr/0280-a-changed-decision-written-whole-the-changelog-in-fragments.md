# 0280. A changed decision written whole, the changelog in fragments

- Status: Accepted
- Date: 2026-11-16

## Context

The repository's documents are read at every session, by people and by agents. On 2026-11-16:

- About 645,000 words of Markdown; `ARCHITECTURE.md` near 28,000, without a summary.
- 187 of 274 ADRs were amended and 3 superseded: a topic's rule in force is read across a chain.
  ADR-0181 had 19 amendments. ADR-0241's status had grown to a page of precisions decided while
  its milestones ran.
- The index of ADRs was written by hand. Written again from the ADRs, it gained about fifty
  relations that it lacked: the oldest ADRs name a relation on one side alone.
- `CHANGELOG.md` changed in 330 commits; its `[Unreleased]` held two sections `Added` and two
  `Changed`, from changes made at once. `docs/PLAN.md` changed in 573.
- The review of the architecture that day recorded this as debt
  ([`debt.md`](../debt.md#left-by-the-review-of-the-architecture-before-m6d10)).

## Decision

The owner's decision of 2026-11-16, for what is written from now on; the past is not rewritten.

**A changed decision is written whole.** A new ADR that changes an accepted one, even in part,
supersedes it and states the whole rule then in force on its topic; one ADR is read for a topic.
`Amends` is no longer written. An ADR that adds a decision without changing one names the older
in its context alone. A precision decided while a milestone runs goes in the milestone's plan, or
in a new ADR, never into an older ADR's status.

**Chains are consolidated when touched.** When a milestone works on a topic whose rule is spread
across amendments, one new ADR supersedes them, the rule stated whole.

**The index is written from the ADRs.** `scripts/adr-index.py` writes `docs/adr/README.md`'s index
from each ADR's heading and status; a relation counts when either side names it. `scripts/check.sh`
checks that the index is current, that a status names only ADRs that exist, and that a supersession
from this ADR on is named on both sides.

**The changelog in fragments.** A change that users see adds a file to `changelog.d/`, named by its
section, and never edits `CHANGELOG.md`. At a release, `scripts/changelog.py --merge` moves the
fragments into `[Unreleased]`, each at the end of its section, two sections of one name joined.
`scripts/check.sh` checks each fragment's name and form.

**`ARCHITECTURE.md` opens with one page:** the crates by layer and the main flows, each told whole
below it.

Both scripts are Python on its standard library alone; CI's `Docs` workflow runs them beside
lychee ([ADR-0188](0188-a-light-check-for-documentation.md)).

## Alternatives considered

- **The documents kept as they were:** nothing to change, but chains that grow at each decision,
  and each session reading them all to find the rule in force.
- **Every chain consolidated now:** documents clean at once, but a rewrite of many ADRs that the
  next milestones would touch anyway.
- **Towncrier or changie for the fragments:** tools made for it, but a dependency for what a
  script of a hundred lines does.

## Consequences

- An ADR that changes a decision is longer to write: it restates the rule whole.
- Older chains stay until a milestone touches them; their ADRs keep `amends` and `amended by`.
- `CHANGELOG.md` changes at releases alone; `[Unreleased]` there holds what was written before
  this ADR.
- CI's `Docs` workflow takes Python from the pinned nixpkgs, beside lychee.

## Validation and reconsideration

Measured: the words of Markdown, the amended and superseded ADRs, the commits that changed
`CHANGELOG.md` and `docs/PLAN.md`. Tried: the index written again and checked; the merge of two
fragments into a copy of `CHANGELOG.md`, the releases before it unchanged.

Proposing another decision is justified if superseding ADRs grow longer to read than the chains
they replace, or if fragments are forgotten at releases.
