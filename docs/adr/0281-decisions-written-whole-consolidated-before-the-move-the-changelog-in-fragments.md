# 0281. Decisions written whole, consolidated before the move; the changelog in fragments

- Status: Accepted; supersedes
  [0280](0280-a-changed-decision-written-whole-the-changelog-in-fragments.md)
- Date: 2026-11-16

## Context

The repository's documents are read at every session, by people and by agents. On 2026-11-16:

- About 645,000 words of Markdown, 360,000 of them in ADRs; `ARCHITECTURE.md` near 28,000.
- 191 ADRs in force were amended, 58 once, 6 ten times or more. ADR-0241's status had grown to a
  page of precisions decided while its milestones ran.
- The amendments join 258 ADRs into one web: they cannot be consolidated by independent groups,
  only by topic.
- At M6d.11, the interface's ADRs move to uwui's repository
  ([ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md)); chains cross the two sides, ADR-0181 for
  one, amended by ADRs of the engine and of uwui.
- The index of ADRs was written by hand. Written again from the ADRs, it gained about fifty
  relations that it lacked: the oldest ADRs name a relation on one side alone.
- `CHANGELOG.md` changed in 330 commits; its `[Unreleased]` held two sections `Added` and two
  `Changed`, from changes made at once.

[ADR-0280](0280-a-changed-decision-written-whole-the-changelog-in-fragments.md) set the rules
below the same day, chains consolidated only when a milestone touched them. This ADR states them
whole, with a milestone that consolidates every chain: the first ADR written under its own rule.

## Decision

The owner's decisions of 2026-11-16.

**A changed decision is written whole.** A new ADR that changes an accepted one, even in part,
supersedes it and states the whole rule then in force on its topic; one ADR is read for a topic.
`Amends` is no longer written. An ADR that adds a decision without changing one names the older
in its context alone. A precision decided while a milestone runs goes in the milestone's plan, or
in a new ADR, never into an older ADR's status.

**Every chain consolidated at D1, before the move.** A milestone of its own, D1, between M6d.10
and M6d.11, writes one ADR for each topic whose rule is spread across amendments, the rule in force
stated whole; each ADR that it replaces becomes `Superseded`. Topics are cut so that each falls to
uwui or to the engine, and the move carries uwui's. Each topic is written by a delegated task and
checked against the ADRs that it replaces by another; the owner validates the cut of topics before
they are written. Stage 6d's own ADRs, ADR-0194 and those that changed its milestones, are one of
these topics, D1 among its milestones.

**After D1, chains are consolidated when touched.** A milestone that works on a topic whose rule
is still spread across ADRs writes the one that supersedes them.

**The index is written from the ADRs.** `scripts/adr-index.py` writes `docs/adr/README.md`'s index
from each ADR's heading and status; a relation counts when either side names it. `scripts/check.sh`
checks that the index is current, that a status names only ADRs that exist, and that a supersession
from ADR-0280 on is named on both sides.

**The changelog in fragments.** A change that users see adds a file to `changelog.d/`, named by its
section, and never edits `CHANGELOG.md`. At a release, `scripts/changelog.py --merge` moves the
fragments into `[Unreleased]`, each at the end of its section, two sections of one name joined.
`scripts/check.sh` checks each fragment's name and form.

**`ARCHITECTURE.md` opens with one page:** the crates by layer and the main flows, each told whole
below it.

Both scripts are Python on its standard library alone; CI's `Docs` workflow runs them beside
lychee ([ADR-0188](0188-a-light-check-for-documentation.md)).

## Alternatives considered

- **Chains consolidated only when a milestone touches them,** as ADR-0280 had it: no milestone of
  its own, but chains that no milestone touches kept for good, and uwui's repository starting with
  chains that the move cuts in two.
- **Every chain consolidated now:** clean ADRs at once, but both tracks held for several sessions,
  and part of the work done again after M6d.10's decisions.
- **Each stage's audit consolidating what its stage touched:** regular, but partial, and the move's
  cut left as it is.
- **The documents kept as they were:** chains that grow at each decision, each session reading them
  all to find the rule in force.
- **Towncrier or changie for the fragments:** tools made for it, but a dependency for what a script
  of a hundred lines does.

## Consequences

- An ADR that changes a decision is longer to write: it restates the rule whole.
- D1 takes several sessions, estimated at 40 to 60 ADRs of consolidation, not measured; a rule
  restated wrongly is the risk that its checks answer.
- Until D1, chains stay; their ADRs keep `amends` and `amended by`.
- `CHANGELOG.md` changes at releases alone; `[Unreleased]` there holds what was written before
  ADR-0280.
- CI's `Docs` workflow takes Python from the pinned nixpkgs, beside lychee.

## Validation and reconsideration

Measured: the words of Markdown and of ADRs, the ADRs amended and how often, the web that the
amendments make, the commits that changed `CHANGELOG.md`. Tried: the index written again and
checked; the merge of two fragments into a copy of `CHANGELOG.md`, the releases before it
unchanged. Estimated: D1's size.

Proposing another decision is justified if D1's first topics show that a superseding ADR is longer
to read than the chain it replaces, or if fragments are forgotten at releases.
