# 0290. uwui's crates' names kept on crates.io once M6d.10 names them

- Status: Accepted
- Date: 2026-11-16

## Context

`uwui` is kept on crates.io by a version 0.0.0 that points to its repository
([ADR-0197](0197-uwuis-repository-public-from-now.md)). On 2026-11-16 the names of uwui's other
crates were free there: `uwui-render`, `uwui-desktop`, `uwui-agent`, `uwui-mcp`, `uwui-kdl`,
`uwui-macros`, `uwui-accesskit`. M6d.10 names the crates that move, `slime-color` and `slime-input`
renamed and `uwui-desktop`'s name kept until then. crates.io's policies forbid a crate that only
keeps a name for a long time, without genuine development behind it. The engine is not published:
a game takes it through git.

## Decision

The owner's decision of 2026-11-16.

**Each of uwui's crates is kept on crates.io by a version 0.0.0 that points to uwui's repository,
once M6d.10 names it,** as `uwui` is; each publication waits for the owner's go. The engine's
crates are not kept there. The policy of the names SlimeEngine and uwui comes before 0.14, as
[ADR-0207](0207-what-slimeengine-is-to-become.md) has it.

## Alternatives considered

- **The seven names of today kept now:** none taken by another, but names that M6d.10 may change
  kept for nothing, against crates.io's policies.
- **Nothing before uwui's 1.0:** no step to take, but a name taken by another is not given back.

## Consequences

- Until M6d.10, uwui's other crates' names may be taken.
- M6d.10's delivery holds the list of names to keep, which the owner publishes or has published.

## Validation and reconsideration

Checked: each name on crates.io's index with `cargo info`, from outside the workspace.

Proposing another decision is justified if one of these names is taken before M6d.10.
