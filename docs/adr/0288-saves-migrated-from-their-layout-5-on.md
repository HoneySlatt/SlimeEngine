# 0288. Saves migrated from their layout 5 on

- Status: Accepted; supersedes in part [0207](0207-what-slimeengine-is-to-become.md) (the saves
  of layouts 1 to 4, set aside rather than migrated)
- Date: 2026-11-16

## Context

[ADR-0207](0207-what-slimeengine-is-to-become.md) promises that before 1.0 the user's data is never
broken: levels, saves, layouts and keys are migrated. On 2026-11-16:

- A save is a document of the world, its *layout*, how the world's parts are written in it, at
  version 5 (`crates/slime-world/src/document.rs`): it changed four times, and the reader refuses
  every layout but its own.
- *Hunna Wood* sets aside a save that it cannot read, as `slime_app::data::set_aside` does: nothing
  is lost, nothing is migrated.
- Scene files read every layout up to their own; dock layouts are migrated
  ([ADR-0241](0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)).
- No player keeps a save: those of layouts 1 to 4 are the test runs of the engine's sessions.
- Stage 11 brings tools that migrate assets, projects and formats ([`ROADMAP.md`](../ROADMAP.md)).

## Decision

The owner's decision of 2026-11-16.

**A save is migrated from its layout 5 on.** A change of the saves' layout comes with its upgrade
from the layout before it; the reader reads every layout from 5 up to its own, each upgraded to the
next in turn, as scene files read theirs. A fixture of each layout from 5 on is loaded in CI.

**A save of layouts 1 to 4 is set aside, never deleted,** as today: a test run's, which no
player keeps.

**From now on,** a change of the layout that comes without its upgrade does not pass review; S1,
on track A after V1, builds the reader of older layouts and the fixture of layout 5.

The rest of ADR-0207's promise on data stands: levels, layouts and keys migrated.

## Alternatives considered

- **Layouts 1 to 4 migrated too:** the promise kept whole, but four layouts that no one uses
  rebuilt from git's history to make their fixtures.
- **Saves left to Stage 11's tools:** no work now, but a promise written and false for stages, and
  each change of the layout until then one more upgrade to write back.

## Consequences

- Each change of the saves' layout costs its upgrade and its fixture, at the time it is made.
- A test run saved before layout 5 starts anew; its save stays where it was set aside.
- The quality bar's scenario of older files holds for saves once S1 is done
  ([`ROADMAP.md`](../ROADMAP.md#architecture-and-stability)).

## Validation and reconsideration

Read: the reader's check of its layout, *Hunna Wood*'s setting aside, scene files' reading of older
layouts. Assumed: that no one keeps a save of layouts 1 to 4 that matters.

Proposing another decision is justified if a save of layouts 1 to 4 turns out to matter to
someone.
