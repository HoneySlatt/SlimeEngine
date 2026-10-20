# 0081. Stage 5a: the world

- Status: Accepted; amended by [0109](0109-stable-references-and-undo.md) (the world panel's
  edits name entities by identity)
- Date: 2026-10-16

## Context

Stage 5a, the first half of Stage 5 ([ADR-0078](0078-stage-5-in-two-halves.md)), gives the engine
its world. Its exit, in the roadmap: *Hunna Wood* runs on the world, which keeps what is drawn and
where what moves was; any component's properties can be listed and edited generically; a run of
the game round-trips through save and load. It is released as 0.3, with Stage 4b's work.

The entity model, the precision of positions, transforms, what the world draws, identities and
saves are decided in [ADR-0079](0079-the-engines-own-world.md); reflection and serialization in
[ADR-0080](0080-reflection-and-serialization.md). Where the engine stands is in those two. Stage
4b's last milestone, M4b.9, holds only the owner's checks by hand when Stage 5a is planned: losing
the GPU device in the Windows virtual machine, from the dev overlay and by a forced timeout
detection and recovery (TDR), and a real GPU reset on Linux, in a KDE Plasma session.

Five design studies of the code preceded the plan. The owner took the decisions below on
2026-10-16, one by one, when the stage was planned.

## Decision

**The order of the work: the risk first.** The world comes first: its storage, then the hierarchy
and the transforms, then how it fits the renderer, the fixed step, recovery from a lost device and
hot reload, then *Hunna Wood* moved onto it. Reflection, generic editing, serialization, the
world's documents and the game's saves follow, then a milestone that checks Windows, rereads the
documents and releases 0.3: ten milestones.

**Generic listing and editing, shown in the game.** A panel of the dev overlay, on F2, lists the
world's entities by name, and edits through the type registry the components of the one selected;
it refuses a value outside its range. It returns each edit as an entity, a property path and a
value, the form that Stage 6's undo will record. It does not undo, add, remove or save, and is
removed once Stage 6's inspector does at least as well, as the rest of the dev overlay
([ADR-0052](0052-amend-roadmap-before-0-1.md)). Tests walk every registered type.

**Saves in *Hunna Wood*.**

- F5 saves the run, and F9 loads the last save. The game saves when the player quits, and resumes
  the save when it starts.
- One save file, in the user's data directory: `$XDG_DATA_HOME`, else `~/.local/share`, on Linux;
  `%APPDATA%` on Windows. `--save <file>` names another; `--new` starts a new run, as R does in the
  game.
- A save made on another layout of the zone starts a new run, as hot reload does
  ([ADR-0072](0072-stage-4b-live-assets.md)).
- The checks never touch the user's save.
- A save is committed as a fixture, which every later version of the game must load.
- No text shows on screen before Stage 7: the log says what was saved or loaded.

**Examples.** A new example, `world`, shows the world, what moves drawn between fixed steps, the
panel and saves, and later Stage 5b's prefabs. `scene` stays the example of the renderer's own API.

**The zone's names.** The zone's script names explicitly the objects that the game refers to, since
names become identities (ADR-0079). Exporting the zone again needs a release of the game's asset
mirror, with the owner's agreement ([ADR-0070](0070-the-sample-games-source-assets.md)).

**CI.** Each milestone that adds a crate or a dependency records how long the two jobs take. The
cooked zone is cached once the Windows job passes 19 minutes, as ADR-0072 decided; past 20 minutes,
the Windows job is split into two jobs that run in parallel.

**Known debt.** The sky's sun, which a game describes twice, is described once when the world draws.
A save made on quit is lost when Windows ends the session; it is documented, F5 stays safe, and a
fix, which needs a new dependency, is decided later.

**Stage 4b stays open.** Its last milestone waits only for the owner's checks by hand, which the
owner runs when they choose; Stage 4b closes after them. Meanwhile, Stage 5a is the stage being
worked on. If the checks have not been run when 0.3 is released, the owner decides then, and the
release notes say so.

**Dependencies.** `syn`, `quote` and `proc-macro2` as direct dependencies, and `toml_edit` with its
four crates (ADR-0080); no other.

**Out of scope**: scene files and prefabs (Stage 5b); the editor: editing the hierarchy, gizmos,
undo and redo, an asset browser, play in editor (Stage 6); systems and a schedule, the job system,
physics and queries, audio, input action mapping, the UI of games, a menu of saves or several slots
(Stage 7); scripts, plugins and the C ABI (Stage 8); values set per instance, animation (Stage 9b);
culling and instancing (Stage 9a); streaming and networking (Stage 10); migration tooling (Stage
11); handles that count references and progressive loading; import settings in metadata files;
moving the cooked formats onto the new serialization; change detection beyond transforms; the
binary encoding of documents (Stage 5b).

**The roadmap.** Stage 5 becomes 5a and 5b (ADR-0078). Its open decisions on the entity model and
the serialization format become fixed decisions (ADR-0079, ADR-0080). Its dependency table gains
`reflect`, on which `assets` depends, and says that the renderer does not depend on the world.

## Alternatives considered

- **The roadmap's order, reflection first**: no component would gain its derive afterwards, but how
  the world fits the renderer and the game would show only at the fifth milestone.
- **Tests alone for generic editing**: less work, but nothing to try, and reflection without a real
  user until Stage 6.
- **F5 and F9 only**: quitting loses the run. **Saving on quit only**: no way back to a moment of
  the run.
- **The save beside the program**, which `cargo clean` deletes and which Program Files forbids, or
  in the current directory, which surprises.
- **The `scene` example moved onto the world**: the renderer's own API would have no example.
- **Closing Stage 4b now, its checks becoming a condition of 0.3's release**: the owner chose to
  keep it open until the checks.
- **A cache of compiled dependencies in CI** rather than two jobs: one more action of a third
  party, cache storage, and runs that depend on what the cache holds.

## Consequences

- The owner plays the game when it runs on the world, when the panel comes, when saves come, and
  from CI's Windows programs at the release.
- The plan shows two stages: 4b, waiting for the owner's checks, and 5a, in progress.
- The panel's code is temporary; the save fixture fixes what later versions must read.
- Each milestone that designs a part records its design in an ADR of its own: the world's storage,
  transforms and precision, what the world draws, reflection, serialization, the world's
  documents.
