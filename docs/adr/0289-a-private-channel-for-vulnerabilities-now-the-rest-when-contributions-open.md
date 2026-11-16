# 0289. A private channel for vulnerabilities now, the rest when contributions open

- Status: Accepted
- Date: 2026-11-16

## Context

SlimeEngine's and uwui's repositories are public: anyone can open an issue or a pull request. The
engine takes no code from outside before 0.14, then under a contributor licence agreement
([ADR-0214](0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)); uwui takes no
contribution from outside before its 1.0 ([ADR-0197](0197-uwuis-repository-public-from-now.md)).
On 2026-11-16 only `CONTRIBUTING.md` and uwui's README said so, and neither repository said how to
report a vulnerability, though the engine reads files from outside and serves tools on the
machine.

## Decision

The owner's decision of 2026-11-16.

**Now, in both repositories:**

- `SECURITY.md`: a vulnerability reported privately, through GitHub's private vulnerability
  reporting, which the owner turns on in each repository's settings; what is supported, `main`
  before 1.0; what matters most.
- A template of pull requests that says that code from outside is not accepted yet, and is closed
  without review; in SlimeEngine, that issues are welcome; in uwui, that they are not taken before
  its 1.0, as its README says.

**When contributions open,** at the engine's 0.14 and at uwui's 1.0: a code of conduct, templates
of issues and the contributor licence agreement, whose text waits for the lawyer (ADR-0214).

## Alternatives considered

- **Everything now:** both repositories complete for outside readers, but a code of conduct without
  a community, and templates of issues of little use yet.
- **Everything when contributions open:** no work, but no channel for a vulnerability until then,
  and pull requests made without knowing that they are closed.

## Consequences

- A vulnerability has a private channel once the owner turns on private reporting.
- No one writes a pull request of code without being told that it is closed.

## Validation and reconsideration

Read: ADR-0197 and ADR-0214 on contributions, both repositories' files for outside readers.

Proposing another decision is justified if outside people take part before contributions open,
when a code of conduct is needed sooner.
