# Architecture Decision Records

An Architecture Decision Record (ADR) is a short document that records one decision about the
engine's architecture, rules or process: the context that made it necessary, what was decided,
which alternatives were rejected and why, and what the decision costs. ADRs are the project's
memory of *why* things are the way they are. [ADR-0001](0001-record-architecture-decisions.md)
explains why the project uses them.

## When to write one

Write an ADR when a decision:

- shapes how crates depend on each other, or what a crate is responsible for;
- sets a convention every contributor must follow (errors, logging, naming, `unsafe`, lints);
- adds a dependency that other code will be built around (small leaf dependencies are recorded
  in [`docs/dependencies.md`](../dependencies.md) instead);
- changes the roadmap: a stage is reshaped, merged, reordered or dropped;
- would be expensive or confusing to reverse without knowing why it was made.

Do not write one for choices that are local to a function or a module and cheap to change.

## Process

1. The person or agent who needs the decision describes 2 or 3 options, their trade-offs, a
   recommendation, and the cost of changing course later.
2. The project owner (the person who validates direction and architecture) decides.
3. The ADR is written in the same change that first applies the decision, so that it describes
   a decision that is actually in effect.
4. An accepted ADR is never edited to change its meaning. To change a decision, write a new ADR
   that supersedes the old one, and set the old one's status to `Superseded by NNNN`. Fixing a
   typo or a broken link is fine.

## Format

File name: `NNNN-short-title.md`, numbered in order of acceptance, never reused.

```markdown
# NNNN. Title in the imperative or as a noun phrase

- Status: Accepted | Superseded by NNNN
- Date: YYYY-MM-DD

## Context

What problem forces a decision now, and what constraints apply.

## Decision

What was decided, stated so that a reader can check whether the code follows it.

## Alternatives considered

Each rejected option and the reason it was rejected.

## Consequences

What becomes easier, what becomes harder, the debt accepted, and the cost of reversing the
decision later.
```

Keep ADRs short. If a decision needs a long explanation, the explanation probably belongs in
[`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) or in the rustdoc of the code it governs, with
the ADR linking to it.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-amend-roadmap-stages-0-to-3.md) | Amend the roadmap for Stages 0 to 3 | Accepted |
| [0003](0003-toolchain-edition-and-msrv.md) | Pin the toolchain in one file, use edition 2024, set the MSRV to the pinned version | Accepted |
| [0004](0004-workspace-layout-and-crate-naming.md) | Workspace layout and crate naming | Accepted |
| [0005](0005-lints-and-unsafe-policy.md) | Lints and `unsafe` policy | Accepted |
| [0006](0006-dependency-policy.md) | Dependency policy | Accepted |
| [0007](0007-documentation-tooling-and-ci.md) | Documentation tooling and continuous integration | Accepted |
| [0008](0008-errors-logging-and-assertions.md) | Errors, logging and assertions | Accepted |
| [0009](0009-frame-timing.md) | Frame timing | Accepted |
| [0010](0010-main-loop-ownership.md) | Main loop ownership and the application API | Accepted |
