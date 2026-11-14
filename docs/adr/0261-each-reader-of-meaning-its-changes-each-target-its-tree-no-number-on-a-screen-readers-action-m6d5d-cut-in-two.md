# 0261. Each reader of meaning its changes, each target its tree; no number on a screen reader's action; M6d.5d cut in two

- Status: Accepted; amends [0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) (the
  crate's name left to M6d.10; late answers numbered) and
  [0249](0249-the-hosts-api-its-items-and-their-table.md) (each accessibility update numbered; a
  screen reader's action with the number of its update)
- Date: 2026-11-14

## Context

M6d.5d, the fourth part of the host's API
([ADR-0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md)), moves `slime-app`'s
translation of the interface's meaning into AccessKit's tree into a crate of uwui's. It also makes
each reader of meaning follow its own changes, gives late answers to a screen reader a number, and
moves `accesskit` out of `slime-app`.

The facts, read in the repository on 2026-11-14:

- uwui keeps the changes of meaning once for everyone: a list of marked elements and a flag on
  each. `Ui::take_meaning_changes` empties both, so a second reader would see nothing.
- The changes cover every target. Their top walks every root; their focus is the active target's
  ([ADR-0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md)).
  Taken after one target's frame, they may hold another target's elements, not yet styled nor laid
  out ([ADR-0258](0258-each-target-its-frame-and-its-time-a-change-asks-for-its-targets-frame.md)).
  A screen reader needs one tree for each window.
- A modal root that opens or closes makes the next changes whole, for every target.
- Marks cost nothing until the changes are first taken: a new `Ui` starts with its next changes
  whole, and a mark then returns at once. The engine takes them only while a screen reader reads.
- No agent reads changes of meaning. Agents read the whole meaning at each request. No milestone of
  M6d.9a plans an event of the interface's changes.
- `accesskit::ActionRequest` (0.25.2) carries no number of the update it was asked on.
  `accesskit` 0.25.2, `accesskit_winit` 0.34.2 and `accesskit_consumer` 0.39.2 are the latest
  releases.

A challenge of the first design found what the alternatives below record. One finding changed the
plan: a screen reader acts on its own copy of the tree, which lags behind the adapter's. AT-SPI's
and UI Automation's adapters release their tree before they call the program's handler, from
another thread. The host can only stamp an action with the last update that the adapter holds,
never older than what the person heard. A number therefore catches only the changes of the last
frame, not the case it was meant for: a row of a list reused for another item, then Enter.

## Decision

The owner's decisions of 2026-11-14.

**M6d.5d is cut in two**, the core first, each half in a session of its own:

- M6d.5d1, each reader of meaning its changes, each target its tree. uwui's core changes, and
  the translation is adapted where it stands, in `slime-app`.
- M6d.5d2, the translation moved into `uwui-accesskit`, with its final API, `accesskit` moved
  there, and the crate's compiling time measured.

Moving the crate first would publish an API that the second half changes at once.

**No number on a screen reader's action.** ADR-0249's numbered updates and actions are dropped.
Today's guard stays:

- an element's number is never given again;
- an element does only the actions that its meaning lists now.

One check is added: a request is done only if it names its reader's tree, and only on an element
of its reader's target.

**The agents' reader.** uwui gives any number of readers their own changes. Two readers of one
target are proven independent in uwui's tests. An event of the interface's changes, which an
agent subscribes to as to `play/time`, joins M6d.9a2's card; track A builds it.

**The crate.** Its name is `uwui-accesskit`, final. Its licence is MIT OR Apache-2.0, as uwui's
crates and AccessKit itself. It depends on `accesskit` 0.25.2 and re-exports it, so that a host
builds its own nodes with the same version. `slime-app` drops its own `accesskit` entry.
`accesskit_consumer` becomes the crate's test dependency. `slime-platform` keeps `accesskit` and
`accesskit_winit` for the window's adapter.

Then the lead's decisions, within those.

**Readers registered in the interface.**

- `Ui::add_meaning_reader(target)` gives a reader, `MeaningReader`, bound to that `Ui` and that
  target. `Ui::remove_meaning_reader` lets it go.
- `Ui::take_meaning_changes(reader, whole)` gives what changed for that reader since it last took
  its changes, and empties that reader's marks alone. It gives none for a reader that was removed,
  or that another `Ui` made.
- Each reader keeps its own state, as the interface kept it for everyone: its list of marked
  elements, whether its next changes are whole, whether its top changed. Each element holds one
  bit for each reader that marked it, so that a reader lists an element once.
- A mark costs nothing without a reader of the element's target, and one push for each reader of
  it. A reader's first changes are whole.
- A reader whose marks pass twice the tree, and at least 1,024, drops them, and its next changes
  are whole. A rare reader beside a frequent one does not make the frequent one's changes whole.
- An interface holds at most 64 readers at once, the bits of a `u64`. `add_meaning_reader` gives
  none past that: one per window and per agent's session leaves room.

**Each target its tree.**

- A reader's changes hold its target's elements alone: those that its target's roots draw.
- Their top is its target's roots, and their focus is that target's own (`Ui::focus_in`).
- A modal root that opens or closes makes the next changes whole for its target's readers alone.
- A root or a layer moved to another target marks what it holds for the new target's readers, and
  the top changes on both targets.
- A target's changes, taken after its frame, no longer hold another target's elements before
  their own frame.

## Alternatives considered

- **One log of marks, each reader holding its place in it**: no registration, but marks cost
  from the start in every program, the game included. Cuts of the log inside one step need care,
  where a slip makes AccessKit panic. And a cursor read against another `Ui` applies a foreign
  log.
- **Each reader's marks without a bit on the element**: no limit on readers, but an element
  marked at each frame, a slider dragged, fills a rare reader's list, which turns whole after a
  few seconds.
- **A number kept, as best one can**: a hash of what each node says, and an action refused when
  it changed after the update that the adapter held. It catches only the last frame's changes,
  refuses silently, and costs memory and code for a guard that ADR-0249's promise still exceeds.
- **The number of any mark, bounds included**: every scroll and every section revealed would
  refuse the clicks asked during it.
- **The crate first, its behaviour unchanged**: its API and documentation written twice.
- **One milestone**: a large change across uwui's core, a new crate, the platform and the engine,
  under one review.
- **The agents' event built now**: track B editing track A's files during M6d.9a1b3b.
- **No event for agents**: an agent waiting for a change would have to poll.
- **`uwui-a11y`**: a name that mixes the translation with the tree that uwui will write for the
  web ([ADR-0181](0181-uwuis-scope.md)).
- **Left to M6d.10**: a rename later, in code, documents and hosts.

## Consequences

- `Ui::take_meaning_changes` takes a reader; its readers in uwui's tests and in `slime-app` change.
- The engine registers a reader while a screen reader reads its window, and removes it when none
  does: marks cost nothing otherwise, as today.
- The debt item on targets sharing one tree of meaning loses that part at M6d.5d1.
- ADR-0249's rules on numbered updates and actions, and its table's "with the number of the update
  it was asked on", no longer hold. Its rows for the whole tree and the accessibility updates are
  built, without a number.
- A screen reader may still act on an element that changed since the person heard it. No platform
  that AccessKit adapts tells which tree a request was made on.
- M6d.9a2's card gains the event of the interface's changes.
- M6d.5d2's checks: the shipping game holds no `uwui-accesskit`; the crate depends on no engine
  crate; CI's `windows (tests)` and `check (headless)` gain its test binary.
