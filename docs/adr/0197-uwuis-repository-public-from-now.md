# 0197. uwui's repository, public from now

- Status: Accepted; amends [0181](0181-uwuis-scope.md) (the repository exists before the move;
  the history of uwui's files joins it at the move) and [0190](0190-uwuis-requirements.md) (the
  repository public from now, unannounced; the name kept on crates.io; the announcement, releases,
  the site and outside contributions at uwui's 1.0)
- Date: 2026-11-06

## Context

[ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md) gives the work after the move to uwui's own
stages. Their roadmap and uwui's quality bar would have been written in this repository, then
moved at M6d.11. A new project starts with its roadmap and its quality bar, before its code.

- SlimeEngine's documents, public, must link to uwui's.
- `deny.toml` refuses a git dependency from an unknown source, and SlimeEngine's public CI could
  not fetch a private repository without a secret.
- GitHub's machines with macOS, which [ADR-0173](0173-the-platforms-of-applications.md) relies on,
  are free for a public repository.
- uwui's code is already public, in SlimeEngine's history.
- The name `uwui` was free on crates.io on 2026-11-06. crates.io's policies forbid a crate that only
  keeps a name for a long time, without genuine function, purpose, or significant development in its
  repository.

## Decision

The owner's decisions of 2026-11-06.

**`HoneySlatt/uwui`, public from now, unannounced,** under MIT OR Apache-2.0. It holds at once
what is uwui's and exists nowhere else twice: its README, its licences, its `AGENTS.md`, its
roadmap with its quality bar, the applications' machines and budgets among them, its open
decisions, a plan that points to SlimeEngine's Stage 6d, an index of ADRs that points here, and a
check of its links in CI.

**Its code comes at the move,** M6d.11, with the logo, the interface's ADRs, its terms of the
glossary, its architecture and its guide for contributors: the history of uwui's files, filtered
from a clone, joins the repository in one merge. Until then, uwui's decisions are this
repository's ADRs, in one numbering.

**Its name is kept on crates.io** by a version 0.0.0 that says it is not usable yet and points to
the repository.

**Its 1.0 brings** the announcement, releases on crates.io, its site and outside contributions,
which its README says until then.

## Alternatives considered

- **A repository on the owner's machine alone, pushed later:** nothing public before the owner
  wishes it, but no backup and no CI, and SlimeEngine's roadmap unable to link to it, keeping a
  summary of what it holds.
- **The repository at the move, as planned:** each decision at its time, but uwui's roadmap
  written here, then moved.
- **A private repository:** nothing visible before the 1.0, but the engine's CI and the macOS
  machines blocked.
- **The name taken on crates.io at the 1.0:** no placeholder, but the name open to anyone once the
  repository shows it.

## Consequences

- Two repositories hold Stage 6d's facts: uwui's direction and bar in uwui's, the milestones that
  change both sides in SlimeEngine's, each linking to the other.
- The merge of the filtered history is a step of M6d.11.
- The version 0.0.0 is replaced by uwui's first release.
