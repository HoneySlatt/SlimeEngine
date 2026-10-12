# Contributing to SlimeEngine

This guide explains how to set up a development environment, check a change and submit it. The
rules themselves are in [`AGENTS.md`](AGENTS.md). That file is written for AI agents, which do
most of the development, but its rules on scope, architecture, code, documentation and commits
apply to every contributor.

## Before you start

- [`docs/PLAN.md`](docs/PLAN.md) says which stage and milestone are in progress. Work outside the
  current milestone is discussed with the project owner before it starts.
- [`docs/adr/`](docs/adr/README.md) holds the decisions already made. Do not re-decide them; if
  one no longer fits, propose a new ADR that supersedes it.

## Development environment

Linux is the only supported platform for now. The project has no code yet: the development
environment arrives in milestone M0.2 of the plan.

## Making a change

1. Keep each commit to one logical change, small enough to review. Write the subject in the
   imperative, in English; explain in the body why the change is needed when it is not obvious.
2. Update every document the change affects. The table in
   [`AGENTS.md`](AGENTS.md#files) says which document covers what.
3. Do not commit code that fails the checks of the
   [definition of done](AGENTS.md#definition-of-done).

## Architecture decisions

Decisions that affect the architecture, a project-wide convention or the roadmap go through the
ADR process described in [`docs/adr/README.md`](docs/adr/README.md): propose options with their
trade-offs and a recommendation, the owner decides, and the ADR is written in the change that
applies the decision.

## Dependencies

Ask before adding a dependency, and explain why it is needed, which alternative was considered
and its license. A written dependency policy arrives in milestone M0.4.

## Review

A change is reviewed against the definition of done and the rules in `AGENTS.md`, and against
the ADRs it touches.

## License of contributions

SlimeEngine is licensed under the [Mozilla Public License 2.0](LICENSE). Contributions are
accepted under the same license.
