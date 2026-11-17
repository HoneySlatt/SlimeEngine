# 0297. One crate of safe files, and the editor's journal versioned, built by S1

- Status: Accepted
- Date: 2026-11-17

## Context

[ADR-0096](0096-the-files-the-engine-reads.md) promises that what the player cannot recover is
written whole, durably, and never over what was not read: a save "is on the disk once reported
written". A read-only review of the engine's architecture, on 2026-11-17, found how the code keeps
that promise:

- Files are written whole by two near copies of one function: `slime_cook::write`
  (`crates/slime-cook/src/lib.rs`), which the editor calls for its levels, its journal, its layouts,
  its recent levels and its speeds, and `saves::write` in *Hunna Wood*
  (`games/hunnawood/src/saves.rs`). The editor depends on the cooker for it; a game cannot, since no
  game links the cooker.
- Neither syncs the directory after the rename: on Linux, the new name is durable only once its
  directory is synced.
- `HeldFile`, `set_aside` and the data and state directories, the rest of the same policy, live in
  `slime-app` (`crates/slime-app/src/data.rs`).
- The editor's choice of keys is written by `std::fs::write` (`crates/slime-editor/src/window.rs`).
- The editor's journal, which holds the steps that the level has not received, carries no version
  (`crates/slime-editor/src/editor.rs`): an editor of another version cannot tell a journal it
  cannot play from a damaged one.

[S1](../stage-6d.md#s1--saves-migrated-from-layout-5) touches saves next on track A
([ADR-0288](0288-saves-migrated-from-their-layout-5-on.md)).

## Decision

The owner's decision of 2026-11-17.

**One crate of the engine holds how files are kept safe:** `slime-files`, on the standard library
alone, below `slime-cook`, `slime-app` and every game. It holds:

- the writer of a file whole or not at all: a file beside it, forced to the disk, renamed, then its
  directory synced where the system allows it;
- `HeldFile`, a file that one program at a time holds;
- `set_aside`, a file that does not read moved under a name of its own, never written over;
- the user's data and state directories.

The cooker, the editor and *Hunna Wood* write through it; the two copies and the plain writes of
user files go. What the directory's sync gives on Windows is checked and written in its rustdoc.

**The editor's journal names its version** in its first line, `# journal 1`. A journal of a later
version is refused with both versions named, and left as it is; a journal without the line is read
as version 1.

**Built by S1,** beside the saves' migration, which ADR-0288 keeps as it is.

## Alternatives considered

- **S1 as planned:** a shorter milestone, but the writer copied again by each game outside this
  repository, and the editor depending on the cooker to write its files.
- **S1 with a real layout 6 that ends with a marker, and loads that warned refused by default:**
  a save cut short refused, and no data dropped without a word, but a larger milestone and a layout
  read for ever. Both stay debt ([`debt.md`](../debt.md)).

## Consequences

- One way to write a file safely, for the engine, its tools and games; `HeldFile` and `set_aside`
  leave `slime-app`, which re-exports them or its callers name the new crate, as S1 chooses.
- A crate more in the workspace, at the bottom of the graph.
- The journal gains a format with a version; a change of its entries comes with its version.

## Validation and reconsideration

Read: the two writers, the editor's callers, `slime-app`'s files, the journal's entries. Assumed:
that a directory's sync is cheap beside a level's or a save's write. Proposing another decision is
justified if the directory's sync costs a frame's budget where a game saves during play.
