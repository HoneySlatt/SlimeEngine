# 0299. What an agent reads before a crate; a crate's status in its README alone

- Status: Accepted
- Date: 2026-11-17

## Context

`AGENTS.md` had every session read a crate's README and all of `docs/ARCHITECTURE.md` before
changing it. A read-only review of the engine's architecture, on 2026-11-17, counted what that
asks:

- `docs/ARCHITECTURE.md` holds 28,906 words, about 40,000 tokens, read again by each session
  that touches a crate, an agent remembering no session before.
- Its table of crates holds about 6,300 words, about 4,900 of them in its *Status* column; uwui's
  cell alone is about 1,000 words on one line, and lists what `crates/uwui/README.md` lists. A
  crate's status then has two homes, against [ADR-0053](0053-one-home-per-fact.md).
- `docs/ARCHITECTURE.md` changed in 173 commits from 2026-10-15 to 2026-11-16: two tracks that
  edit a cell of one line meet there.

## Decision

The owner's decision of 2026-11-17.

**Before a crate, an agent reads its README, then what `docs/ARCHITECTURE.md`'s first page names:**
that page, the dependency rules, the invariants and ownership, the limits and recovery, and the
flow that the change touches. The rest of the file is reference, read when a task needs it.
`AGENTS.md` points at the first page, which holds the list.

**A crate's status lives in its README alone.** The table of crates keeps each crate's role and
what it depends on; its *Status* column goes at M6d.11, when uwui's rows leave the table, so that
no file shared by two tracks is rewritten meanwhile.

## Alternatives considered

- **Everything read, as before:** nothing missed by an agent, but the cost paid at every session,
  and rules drowned in reference, which agents then learn from the code nearest them instead.
- **The table generated from each README's first paragraph, the sections read capped in words by
  `scripts/check.sh`:** drift made impossible, but a script more for what the decision already
  settles; worth it if the table grows again after the move.

## Consequences

- A session reads about a tenth of what it read before a change; a flow next to the change may be
  missed, which the reviews of milestones look for.
- From M6d.11, a crate's status is changed in its README alone, and the table of crates changes
  only when a crate's role or dependencies do.

## Validation and reconsideration

Counted: the words of `docs/ARCHITECTURE.md`, of its table and its column; the commits that
changed it. Assumed: that the first page's list holds what a change must keep. Proposing another
decision is justified if a review of a milestone finds a change that broke a flow its agent was
not told to read.
