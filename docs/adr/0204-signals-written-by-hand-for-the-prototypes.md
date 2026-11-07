# 0204. Signals written by hand for M6d.1b's prototypes

- Status: Accepted; amends [0202](0202-how-m6d1-compares-ways-of-writing-an-application.md) (the
  library of signals that it left to M6d.1b)
- Date: 2026-11-06

## Context

[ADR-0202](0202-how-m6d1-compares-ways-of-writing-an-application.md) left to M6d.1b a dependency
to come: a library of signals, or one written by hand for the prototypes, put to the owner when a
milestone first needs it. M6d.1b3a needed it, for the signals' prototype, thrown away once M6d.1b
ends.

The bench counts each phase's work and allocations exactly, on one thread, and compares each model
with a new interface built from the same state. Three candidates were weighed:

- `floem_reactive` 0.2.0, Floem's own runtime, which the ideal code follows: MIT OR Apache-2.0,
  one dependency, but its last release in 2024-11, older than the Floem that M6d.1a's survey read;
- `reactive_graph` 0.2.16, Leptos's: maintained and widely used, but made for asynchronous work and
  the web, with many dependencies, and a model further from Floem's;
- a runtime written by hand on the bench's branch.

## Decision

The owner's decision of 2026-11-06: **the signals' runtime is written by hand,** on the branch,
as Floem's works: one thread, signals, memos and bindings owned by scopes, values in a `RefCell`.
No dependency is added. The review compares it with Floem's, and uwui's
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md)
says where it may cost less.

## Alternatives considered

- **`floem_reactive`:** faithful to the reference, already tuned; but a release older than the
  survey's Floem, a dependency to justify for a branch thrown away, and counts that the bench
  could not reach inside it.
- **`reactive_graph`:** mature, but its model and its dependencies far from what the ideal code
  writes.

## Consequences

- The signals' figures are the prototype's, not Floem's; a runtime written by hand may serve its
  model better or worse than a library, which the review weighs.
- The choice of uwui's own signals, if the owner chooses signals, is M6d.1b3b's ADR's, and its
  dependency put to the owner then.
