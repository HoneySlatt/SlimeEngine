# 0277. The engine ported to uwui at the end of each of uwui's stages

- Status: Accepted; amends
  [0276](0276-the-engine-follows-uwui-by-one-exact-commit-until-its-1-0.md) (the rhythm at which
  the engine moves uwui's commit before uwui's 1.0)
- Date: 2026-11-16

## Context

[ADR-0276](0276-the-engine-follows-uwui-by-one-exact-commit-until-its-1-0.md) has the engine name
one exact commit of uwui until uwui's 1.0, and left apart how often the engine moves it.

- No stage of the engine runs between the move and uwui's 1.0: Stage 6e, the end of Stage 6c and
  Stage 7 wait for it ([`ROADMAP.md`](../ROADMAP.md#uwuis-stages)).
- uwui's road to its 1.0 is long: its first stage, the foundations and four programs ported, then
  the rest of its library, its platforms and its tools, cut into stages at M6d.10
  ([uwui's roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#4-stages)).
- From its 1.0, uwui keeps semantic versioning: a change that its API then needs costs a major
  release.
- The editor is uwui's most demanding client: docked panels, lists of 100,000 rows, a viewport
  drawn on the device that the engine lends ([ADR-0244](0244-the-engine-lends-its-device-through-a-guarded-door.md)).
- wgpu, accesskit and rmcp, whose types cross between the two sides, each made several breaking
  releases in the 12 months before ([ADR-0276](0276-the-engine-follows-uwui-by-one-exact-commit-until-its-1-0.md)).
- [ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md) refused porting the editor at each change of
  uwui's API while it changes most.

## Decision

The owner's decision of 2026-11-16.

**At the end of each of uwui's stages, a milestone of the engine moves its commit of uwui** to that
stage's last, ports the editor, the games and the examples to it, and takes the versions of wgpu,
accesskit and rmcp that uwui then uses. Between two stages' ends, the engine moves its commit only
for a fix that it needs.

**A port that outgrows a milestone is reported to the owner** before it goes on, with what it
found of uwui's API.

The first of these milestones follows uwui's first stage. Their number follows the cut of uwui's
stages, made at M6d.10.

## Alternatives considered

- **The engine frozen at the move's commit until uwui's 1.0, then ported once:** no work of the
  engine meanwhile, but one large port, uwui's API frozen before its most demanding client tried
  it, and every breaking release of wgpu, accesskit and rmcp of those months taken at once.
- **One port to a candidate of uwui's 1.0, before it is tagged:** the API tried before it is
  frozen, but a large port, and a fault found so late delays the 1.0; the libraries lag until
  then.
- **The engine moved at each milestone of uwui:** the smallest steps, but the editor ported at each
  change of the API, which ADR-0194 refused.

## Consequences

- A few milestones of the engine run during uwui's stages, one at each stage's end; the engine's
  plan holds each when its stage of uwui ends.
- The editor tries uwui's API before its 1.0 freezes it; what it finds goes to uwui's next stage.
- The weekly job of ADR-0276 tells how far the engine is behind between two stages' ends; being
  behind there is expected, not a fault.

## Validation and reconsideration

Read: both roadmaps' order of stages; the libraries' breaking releases counted for ADR-0276.
Assumed: that a port at a stage's end fits a milestone.

Proposing another decision is justified if ports outgrow a milestone twice, or if uwui's stages
come so often that the ports cost more than the stages that they follow.
