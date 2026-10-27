# 0105. egui's fonts ship with the dev overlay

- Status: Superseded by [0152](0152-the-dev-overlay-on-the-engines-interface.md) (egui and its
  fonts left the engine at M6b.8b); it amended [0031](0031-dev-overlay-with-egui.md) (where egui's
  fonts go), and was amended by [0132](0132-the-engines-ui-framework.md) (egui and its fonts leave
  with the overlay's rewriting, at Stage 6b)
- Date: 2026-10-19

## Context

[ADR-0031](0031-dev-overlay-with-egui.md) allows, for one crate, `epaint_default_fonts`, two
licenses that [ADR-0006](0006-dependency-policy.md) does not: the SIL Open Font License 1.1 and the
Ubuntu Font Licence 1.0, under which egui's default fonts come. It does so because the fonts are
tooling, drawn by the dev overlay, "and ship with no game"; `deny.toml` and
[`docs/dependencies.md`](../dependencies.md) say the same.

The audit of 0.4 ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)) found
that this is not so (G54, low): *Hunna Wood* keeps the dev overlay, and its program embeds the fonts,
as `scene.exe` and `world.exe` do. The releases already carry the fonts' licenses beside the
programs ([`docs/PLAN.md`](../PLAN.md), how a release is made). The owner decided on 2026-10-19
that the fonts may ship, the dev overlay staying in the game.

## Decision

- **egui's fonts ship in the programs that draw the dev overlay**: the examples that have one and
  *Hunna Wood*. The exception of `deny.toml` stays limited to `epaint_default_fonts`, and its
  reason is that the overlay ships, with its fonts, in the engine's programs.
- **A program that embeds them travels with their licenses**, as the Ubuntu Font Licence and the
  Open Font License ask: a release puts them beside the programs, as it does now.
- Everything else in ADR-0006 and ADR-0031 stands; the overlay stays tooling, removed once
  something does at least as well ([ADR-0052](0052-amend-roadmap-before-0-1.md)).

## Alternatives considered

- **The game without the dev overlay in its release build**, behind a feature: the game would lose
  the panel of what a frame costs and the button that loses the GPU device, which its checks by
  hand use, and the overlay would have a second way to build.
- **Fonts of the engine's own choosing, under licenses that ADR-0006 allows, given to egui**: work
  for tooling that the editor's UI framework replaces
  ([ADR-0098](0098-the-games-the-engine-aims-at.md)).

## Consequences

- `deny.toml`'s exception, `docs/dependencies.md` and the overlay's documents say that the fonts
  ship with the programs that draw the overlay.
- A game built on the engine that keeps the overlay carries the fonts and their licenses too.
