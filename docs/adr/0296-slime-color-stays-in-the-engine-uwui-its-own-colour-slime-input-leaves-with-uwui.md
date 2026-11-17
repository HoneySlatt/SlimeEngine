# 0296. `slime-color` stays in the engine, uwui gets a colour of its own; `slime-input` leaves with uwui

- Status: Accepted; supersedes in part [0181](0181-uwuis-scope.md) (what leaves with uwui) and
  [0276](0276-the-engine-follows-uwui-by-one-exact-commit-until-its-1-0.md) (`slime-color` among
  `slime-app`'s re-exports, and the engine's lowest crate following uwui's repository)
- Date: 2026-11-17

## Context

[ADR-0181](0181-uwuis-scope.md) has `slime-color` and `slime-input` leave with uwui at the move,
M6d.11, the engine then taking both from uwui's repository. A read-only review of the engine's
architecture, on 2026-11-17, read what that costs:

- `slime-reflect`, the engine's lowest crate, depends on `slime-color`, so that `slime-serialize`,
  `slime-assets` and `slime-world` would follow uwui's commit
  ([ADR-0276](0276-the-engine-follows-uwui-by-one-exact-commit-until-its-1-0.md)): a server, the
  cooker and every tool that reads a level would fetch uwui's repository.
- Levels and saves store `LinearColor`'s fields, `color = { r = 1.0, g = 0.1, b = 0.05 }`. Their
  shape is the engine's already: `slime-reflect` describes `LinearColor` by hand
  (`crates/slime-reflect/src/impls.rs`), pinned by `tests/linear_color.rs`.
- The engine renders in linear light ([ADR-0045](0045-one-linear-color-type.md)); uwui's colour
  model blends in sRGB's encoding, with wide gamuts and a description on every surface
  ([ADR-0234](0234-uwuis-colour-model-before-its-api-is-stable.md)): the two meanings part.
- The engine is the colour's main user: 59 files of the other engine crates name `slime_color`, 37
  of uwui's.
- `slime-input`'s keys and buttons are what uwui's host's events carry, and its desktop host
  depends on it: a vocabulary of input that is uwui's to keep.

## Decision

The owner's decision of 2026-11-17. With it, the rule in force on what leaves with uwui:

**uwui's crates and `slime-input` leave with uwui at M6d.11; `slime-color` stays in the engine.**
The canvas out of `slime-render` and the translation to AccessKit out of `slime-app` left already,
as ADR-0181 had them.

**uwui gets a colour of its own, free of the engine.** Its type and its first meaning are named by
M6d.10a and built before the history is filtered at M6d.11, so that uwui's crates leave depending
on no engine crate; the colour model of ADR-0234 grows on it in uwui's first stage.

**The engine converts at its bridge.** Where the engine hands a colour to uwui or reads one from
it, `slime-app`'s bridge, the dev overlay's colour wells and the editor, it converts between
`LinearColor` and uwui's colour; no engine crate below them names uwui's.

**`slime-color` keeps its licence,** MIT OR Apache-2.0, and its rule: it depends on no crate.

**`slime-app` re-exports `uwui` and `uwui-render` for games,** as ADR-0276 has it, and no longer
`slime-color`, which a game takes from the engine as any engine crate.

The rest of ADR-0181 and ADR-0276 stands.

## Alternatives considered

- **Both leave with uwui, as planned:** one colour type and no conversion, but the engine's data
  layer following uwui's commit, and a change made for uwui's model reaching the engine's levels
  at each port.
- **`slime-color` a crate of its own, published at 0.x, shared by both:** one type without
  depending on uwui's repository, but a third line of versions to keep, still carrying uwui's
  model or holding uwui back from it.
- **`slime-input` kept in the engine too:** the engine's data layer needs no input; uwui's host
  events would then depend on an engine crate.

## Consequences

- The engine's data layer, `slime-reflect`, `slime-serialize`, `slime-assets` and `slime-world`,
  depends on nothing of uwui's after the move; a check that it builds without wgpu, winit or uwui
  needs no exception.
- The engine's windowed side still follows uwui's commit, through `uwui`, `uwui-render` and
  `slime-input`.
- uwui's public API changes before the move: wherever it names `LinearColor`, it names its own
  colour. M6d.10a and M6d.11 hold that work.
- Two colour types meet at the bridge; a conversion stays where the engine draws an interface.
- Reversing it after the move would move a crate between two repositories again.

## Validation and reconsideration

Read: ADR-0181, ADR-0234, ADR-0276; `slime-reflect`'s description of `LinearColor` and its
reference; the uses of `slime_color` per crate. Assumed: that the conversions at the bridge stay few
and cheap. Proposing another decision is justified if uwui's colour and the engine's turn out to
mean the same at uwui's 1.0, so that one type would serve both without holding either back.
