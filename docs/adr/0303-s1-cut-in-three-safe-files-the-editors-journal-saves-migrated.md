# 0303. S1 cut in three: safe files, the editor's journal, saves migrated

- Status: Accepted
- Date: 2026-11-18

## Context

[S1](../stage-6d.md#s1--saves-migrated-from-layout-5) holds three decisions:
[ADR-0288](0288-saves-migrated-from-their-layout-5-on.md) (saves migrated from layout 5 on),
[ADR-0297](0297-one-crate-of-safe-files-and-the-editors-journal-versioned-built-by-s1.md) (one crate
of safe files, the editor's journal versioned) and `slime-app`'s half of
[ADR-0298](0298-a-panic-ends-a-run-without-stop-the-same-rule-in-both-hosts.md) (a panic ends a run
without `stop`), and four items of [`debt.md`](../debt.md) beside them.

Read on 2026-11-18, before any of it was built: about ten deliverables in six crates and CI's
workflows, a new crate and its four callers among them. The journal and the saves write through the
new crate, so it comes first. One session holds one milestone and its closing review.

## Decision

The owner's decision of 2026-11-18.

**S1 is cut in three, in this order, each a session of track A:**

- **S1a, safe files:** `slime-files` and everything that moves onto it. The writer of a file whole
  or not at all, its directory synced, what Windows gives written in its rustdoc; `HeldFile`, its
  errors an enumeration; `set_aside`; the data and state directories, out of `slime-app`. The
  cooker, `slime-app`, the editor and *Hunna Wood* writing through it, the two copies of the writer
  and the editor's plain write of its keys gone. The `cargo tree` check that the engine's data
  layer and `slime-files` build without wgpu, winit or uwui; the cooked zone's key in CI taken from
  `cargo tree -p slime-cook`, written once.
- **S1b, the editor's journal:** `# journal 1` as its first line, a later version refused with both
  named and left as it is, a journal without the line read as version 1; a crash between a level's
  write and its journal's mark no longer refusing the next start.
- **S1c, saves migrated:** the reader of layouts from 5 on, the fixture of layout 5 loaded in CI,
  the rule of upgrades in the reader's rustdoc, a test through a layout 6 made for it and thrown
  away; `slime-app`'s half of ADR-0298, which changes when *Hunna Wood*'s save is written.

S1's "Done when" is split among the three, each part's checks with the part that builds them. S1 is
done when S1c is.

## Alternatives considered

- **Cut in two,** safe files with the journal, then saves with the panic: a closing review fewer,
  but a first part that likely overruns its session.
- **S1 whole over several sessions:** no cut to record, but one milestone across sessions, against
  the method, and its review only at the end.

## Consequences

- Three closing reviews in place of one.
- Until S1c, a save of another layout is still set aside, and a panic still leaves `run` without
  `stop`.
- The documents that name S1 for one of these items name its part.

## Validation and reconsideration

Read: S1's deliverables, the writers, `slime-app`'s files, the journal's entries, the saves'
reader. Assumed: that each part fits a session. Proposing another decision is justified if a part
overruns its session too.
