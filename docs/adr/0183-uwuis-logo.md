# 0183. uwui's logo

- Status: Accepted
- Date: 2026-11-05

## Context

uwui's code is licensed MIT OR Apache-2.0 ([ADR-0181](0181-uwuis-scope.md)), which let anyone use,
change and redistribute it. The owner drew its logo, `crates/slime-ui/logo/uwui.svg`. A logo names
a project: under the code's licenses, a fork could present itself with it.

## Decision

**The logo is not under the code's licenses; its author keeps its rights.** It may be shown,
unchanged, to refer to uwui; it may not be the logo of another project, a fork included, nor
suggest that a project is uwui or endorsed by it. The note beside it,
`crates/slime-ui/logo/README.md`, says so, and the repository's README names the exception. The
logo moves with uwui to its own repository.

## Alternatives considered

- **CC BY 4.0**: free to reuse with an attribution; a fork could keep the logo.
- **MIT OR Apache-2.0, as the code**: simplest, but nothing would keep the logo to uwui.

## Consequences

- The repository is under MIT OR Apache-2.0 but for the logo, whose note governs it.
- A fork of uwui takes a logo of its own.
