# 0053. One home per fact in the documentation

- Status: Accepted
- Date: 2026-10-14

## Context

The same contracts are written in several documents: what `Renderer::render` does, its passes, its
sizes and its errors are in its rustdoc, in the README of `slime-render`, in `docs/ARCHITECTURE.md`,
in ADRs and in `docs/PLAN.md`. When the code changes, some copies are updated and others are not.
Before 0.1, two reviews of the engine, and the checks that followed, found four documents left wrong
that way: the rustdoc of `Renderer::render` described a shadow map fitted to the instances after the
map had come to follow the camera, a known-debt entry said that running out of GPU memory panics
after it had stopped panicking, the documentation of `Event` said that the modifier keys were not
reported after they were, and that of `Sky::sun` named one of its light's fields as meaningless
there but not the other.

The project's documents are its memory: the agents that develop it remember nothing else
(`AGENTS.md`). Writing less is not the remedy; writing each fact once is. The owner decided on
2026-10-14 on a rule in `AGENTS.md`, with the worst duplicates trimmed now and the others as the
documents are touched.

## Decision

Each kind of fact has one home, and the other documents link to it instead of copying it:

- **rustdoc** holds the API's contracts: what an item does, its invariants, errors and panics, its
  limits, and the figures that go with them (sizes, costs, defaults).
- **A crate's README** gives the crate's role, what it depends on, what it must not do, and one or
  two sentences per entry point, which link to the rustdoc for the rest.
- **`docs/ARCHITECTURE.md`** holds the flows across crates, without the figures that the rustdoc
  holds.
- **An ADR** holds why a decision was taken and what was rejected; it is not rewritten as the code
  moves on, only its status.
- **`docs/PLAN.md`** holds status and known debt, and links to where the facts live.
- **`CHANGELOG.md`** says what changed for users, once, when it changes.

A document that needs a fact held elsewhere to be understood says it in a sentence, without its
figures, and links to its home. The rule goes into `AGENTS.md`, beside the table of documents.

## Alternatives considered

- **Keeping the copies and checking them at each change.** It is what failed: nothing tells the
  writer of a change where the other copies are.
- **Writing less documentation.** The reader of `AGENTS.md`'s standard, a developer who has never
  seen the project, and the agents that have no other memory, need the facts; they need them once.
- **Generating the READMEs from the rustdoc.** A tool for what a rule and links do.

## Consequences

- The READMEs of `slime-render` and `slime-gfx`, the worst duplicates, are trimmed first, to their
  role and their entry points; other documents are brought to the rule when a change touches them.
- A reader goes one link further for the details; the link says where.
- A review checks that a change states each new fact in its home only.
