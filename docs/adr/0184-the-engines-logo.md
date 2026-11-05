# 0184. The engine's logo

- Status: Accepted
- Date: 2026-11-05

## Context

SlimeEngine is licensed MIT OR Apache-2.0 ([ADR-0182](0182-the-engine-under-mit-or-apache-2-0.md)).
The owner drew its logo, its mascot, `docs/logo/slime.svg`. uwui's logo is kept outside the code's
licenses, so that a fork cannot present itself with it ([ADR-0183](0183-uwuis-logo.md)).

## Decision

**The engine's logo is kept as uwui's is: its author keeps its rights.** It may be shown, unchanged,
to refer to SlimeEngine; it may not be the logo of another project, a fork included, nor suggest
that a project is SlimeEngine or endorsed by it. The note beside it, `docs/logo/README.md`, says
so; the repository's README shows the logo and names the exception.

## Alternatives considered

- **CC BY 4.0**: free to reuse with an attribution; a fork could keep the logo.
- **MIT OR Apache-2.0, as the code**: simplest, but nothing would keep the logo to the engine.

## Consequences

- The repository is under MIT OR Apache-2.0 but for its two logos, whose notes govern them.
