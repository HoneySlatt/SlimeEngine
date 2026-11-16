# 0276. The engine follows uwui by one exact commit until its 1.0

- Status: Accepted; amends [0194](0194-stage-6d-uwui-leaving-the-engine.md) (how the engine
  follows uwui after the move, settled before M6d.10)
- Date: 2026-11-16

## Context

At M6d.11, uwui's code moves to its own repository, `HoneySlatt/uwui`
([ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md)). ADR-0194 left to M6d.10 how the engine
follows it. uwui is released on crates.io at its 1.0 alone
([ADR-0197](0197-uwuis-repository-public-from-now.md)): until then the engine depends on it
through git. Games depend on the engine through git too ([`README.md`](../../README.md)).

Facts read and tried on 2026-11-16:

- A dependent's resolution ignores its dependency's `Cargo.lock`, as the Cargo book's FAQ says. An
  engine that names uwui by `branch` lets a new game resolve a newer commit than the engine tested;
  a test project failed to compile so. Named by `rev` in the manifest, the same game built.
- Cargo tells git sources apart by their reference as written: a game that names uwui by a short
  form of the engine's commit, or by `branch`, gets two uwuis, whose types do not meet (E0308).
- Some libraries' types cross between the two sides, so both must take one version of each: wgpu,
  through `slime-gfx`'s door ([ADR-0244](0244-the-engine-lends-its-device-through-a-guarded-door.md));
  accesskit, through `slime-platform`'s `accesskit_winit`; rmcp, in `uwui-mcp`'s public API. Over
  the 12 months before, crates.io shows 3 breaking releases of wgpu, 4 of accesskit, 5 of
  accesskit_winit and 12 of rmcp.
- Of the 160 commits of the 30 days before that touched uwui's crates, 107 touched the engine too.
  Sorted by hand: 60 came before uwui was named, built with the editor; 27 since are the move's own
  work; about 4 are features of both sides of the kind that goes on after the move.
- `slime-color` and `slime-input` leave with uwui ([ADR-0181](0181-uwuis-scope.md)); `slime-reflect`,
  the engine's lowest crate, depends on `slime-color`, so the whole engine follows uwui's
  repository, not its interface's crates alone.
- `scripts/check.sh` passes `--locked` to cargo: a `[patch]` to a local checkout of uwui rewrites
  `Cargo.lock`, and the check refuses it.

## Decision

The owner's decision of 2026-11-16.

**Until uwui's 1.0, the engine names uwui by one exact commit, `rev`, written once** in the
workspace's manifest, on uwui's `main`. An engine commit moves it when the engine needs a change of
uwui. After uwui's 1.0, the engine takes uwui's releases from crates.io.

With it, delivered at M6d.11:

- **One commit, checked.** `scripts/check.sh` checks that every uwui crate the workspace names takes
  the same `rev`, and that it is a commit of uwui's `main`.
- **Games do not name uwui.** `slime-app` re-exports `uwui`, `uwui-render` and `slime-color`; a game
  takes them through the engine, whose commit decides uwui's.
- **A change of both sides has its script.** It points the workspace at a local checkout of uwui
  and runs the checks without `--locked`; once uwui's commit is pushed, the engine moves its `rev`
  and `scripts/check.sh` runs locked again.
- **Libraries whose types cross move together.** A breaking release of wgpu, accesskit or rmcp is
  taken by a milestone of both projects, uwui first, the engine at its next `rev`. Both projects'
  `AGENTS.md` say so.
- **No job of uwui's CI builds the engine.** A weekly job of the engine's CI builds it against uwui's
  `main` and records how many commits the engine is behind; it blocks nothing.

## Alternatives considered

- **uwui followed by `branch = "main"`, the commit kept by the engine's `Cargo.lock`:** an update by
  `cargo update`, but a game resolves its own uwui, which the engine may not compile with.
- **uwui followed by its tags:** an exact commit as `rev` gives, but each tag waits for the owner's
  go ([`AGENTS.md`](../../AGENTS.md)), for no gain.
- **uwui kept in the engine's repository by a subtree synced both ways,** as Rust's compiler keeps
  its tools: a change of both sides in one commit, but uwui's code in two places and a tool to
  sync them, for about four such changes in a month.
- **uwui kept here, ADR-0194 undone:** the commits that touch both sides are mostly the move's
  own work, which ends with it.
- **A job of uwui's CI that builds the engine:** blocking, it ties uwui to the engine as ADR-0194
  refused; not blocking, it stays red until an engine milestone moves the `rev`, and each push of
  uwui pays for the engine's build.

## Consequences

- The engine sees a change of uwui only when it moves its `rev`; until then, uwui's fixes wait.
- A change of both sides takes two commits, uwui's first.
- `deny.toml` allows uwui's git source.
- A game that names uwui itself, or `slime-color`, risks two copies; the README shows the
  re-exports.
- uwui's history is never rewritten on `main`: a commit that the engine names must stay fetchable.
- The rhythm at which the engine moves its `rev` before uwui's 1.0, and agents' families served
  over a range of versions, are decided apart.

## Validation and reconsideration

Tried: the resolution of a game under `rev` and under `branch`, on a test project; the patch's
refusal by `--locked`. Counted: the commits that touched both sides, sorted by hand from a sample.
Read: crates.io's breaking releases of wgpu, accesskit, accesskit_winit and rmcp. Assumed: that
changes of both sides stay a few a month after the move.

Proposing another decision is justified if changes of both sides pass about one a week after the
move, or if the weekly job finds the engine behind for more than a uwui milestone while a stage
of the engine runs.
