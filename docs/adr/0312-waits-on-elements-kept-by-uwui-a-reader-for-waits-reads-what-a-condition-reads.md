# 0312. Waits on elements kept by uwui; a reader for waits reads what a condition reads

- Status: Accepted; supersedes in part
  [0261](0261-each-reader-of-meaning-its-changes-each-target-its-tree-no-number-on-a-screen-readers-action-m6d5d-cut-in-two.md)
  (what each reader of meaning takes); amended by
  [0314](0314-m6d9a2c4-cut-in-two-an-interfaces-wait-gives-the-elements-seen-as-after.md) (`after`, and a check while a change is not framed)
- Date: 2026-11-19

## Context

M6d.9a2c3 ([`stage-6d.md`](../stage-6d.md#m6d9a2c3--waits-in-uwui)) gives uwui waits on a
target's elements, kept across frames, which M6d.9a2c4 puts on the wire as `interface/wait`.
[ADR-0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)
set their shape: a condition on a role, a name and a state, present or absent, answered at once
when it holds, else checked at each frame's end on the elements that changed, through one reader of
meaning for each target, which the waits share.
[ADR-0308](0308-m6d9a2c-cut-in-four-each-familys-messages-described-in-lines-read-from-its-types-a-kept-copy-for-each-version.md)
set the rules of every wait.

The facts, read in the repository on 2026-11-19, at `4efee86c`:

- A reader of meaning ([ADR-0261](0261-each-reader-of-meaning-its-changes-each-target-its-tree-no-number-on-a-screen-readers-action-m6d5d-cut-in-two.md))
  lists the elements whose meaning changed. Its first changes are whole, and so are those after a
  modal root opened or closed on its target.
- No reader lists an element removed. Only the element above it, or the target's top, is marked.
  An element moved to another target is marked for the new target's readers alone.
- An element is marked when only its bounds change, a scroll or a resize, and when only its path
  changes, a root raised or a root below it removed. A resize marks every element of a root.
- Every write of an element's role, name or states marks it; a display that changes marks the
  element and every element below it.
- Making an element's whole `Meaning` walks up the tree for its actions and its bounds, and clones
  its texts: a field's text at each key typed.

A fresh agent attacked the lead's first design before it was built. It found that looking up each
element that the waits kept, after any change, made a hover cost the whole set; that marks of
bounds and paths made a scroll or a resize read the tree; that a whole `Meaning` cloned a field's
text at each key; that the design changed ADR-0261 without an ADR; and tests that could pass
without their code. It found no write of what a condition reads that marks nothing.

## Decision

The lead's decisions, within ADR-0269 and ADR-0308.

**The waits are a type of `uwui::agents`, which a host keeps beside its interface.**

- `Waits::wait(ui, target, condition)` walks the target's elements once. It answers `Met`, with
  the elements matched, when the condition holds; else it holds the wait and gives its `WaitId`.
- `Waits::check(ui, target)`, called after the target's frame, reads what changed, and takes out
  and gives the waits met (`Met`). Its counter, `WaitCheck::visited`, says how many elements it
  read.
- `Waits::cancel(ui, wait)` lets a held wait go; `Waits::clear` lets them all go, their readers
  with them, or without the interface when it is gone.
- It holds the waits of one interface at a time, and refuses another's (`REFUSED`) until its last
  wait goes. It bounds nothing itself: the host bounds what it asks, as `slime-app` will through
  `slime_channel::MAX_WAITS` at M6d.9a2c4.

**A condition is a role, a text that the name holds, states that hold and states that do not,
and a presence.** Its role and name filter as `interface/elements` filters; no role, or an empty
name, matches any. `holding` and `lacking` name states of `agents::State`, which a summary lists by
the same names. Both are needed: "`save` present, not disabled" waits for an enabled `save`, which
"no disabled `save`" does not, since a `save` gone meets it. Present is met once an element
matches; absent, once none does. A wait met ends. Its elements are given in the order of the tree,
as `interface/elements` gives them.

**Each wait keeps the elements that it matched.** A check reads each element listed since the last
check once, and updates each wait's set: added where it matches, taken out otherwise, and taken out
when it is gone, hidden, meaningless or on another target.

**What each reader of meaning takes**, the rule whole, which this ADR states for both kinds:

- A reader for a screen reader, `Ui::add_meaning_reader`, takes what ADR-0261 says: its first
  changes whole, whole again when a modal root opens or closes on its target, and every element
  whose meaning changed, its bounds and path included.
- A reader for waits, which `Waits` makes, one for each target with waits, takes:
  - every element marked for its meaning: its role, name, states, value or text, whether it is
    drawn or means something, the elements above a child added or removed;
  - each element removed, as it is freed, and each element moved to another target, as it leaves;
  - never an element whose bounds or path alone changed, nor a modal root's whole changes: no
    condition reads bounds, paths or actions.

  Its first marks are what changes after it is made, since the wait that makes it walked the
  target. Its marks are taken as a list of elements, without making `MeaningChanges`.
- Both kinds are bounded alike: past twice the tree, and at least 1,024 marks, the next take is
  whole. A check then walks its target.
- A condition that reads actions, bounds or paths would need those marks back: the type that
  holds a condition says so.

**A check reads what a condition reads, without making a `Meaning`:** the role, the name
borrowed, the states, and whether a press opens a layer. A check and its reader trade their lists
of marks, so that neither allocates once both have their room. The name is written in small
letters by `to_lowercase`, as `interface/elements` writes it, once for each element read whose role
and states a wait with a name matches.

**The interface's `after` is M6d.9a2c4's decision.** uwui gives `ElementId`s; the element's
number, never given twice (`MeaningNode::number`), is read inside `uwui::agents`, where M6d.9a2c4
builds the reply. Parsing role and state names from the wire, refusing those it does not know, is
M6d.9a2c4's too.

## Alternatives considered

- **Each element that a wait kept looked up after any change:** no change to the core's marks,
  but a hover then reads every element kept, all the rows in sight for a wait on rows.
- **Readers for waits made like a screen reader's:** one kind of reader, but a dialog opened or
  closed, the case the card names, walks the tree; and a scroll or a resize reads every element
  that moved.
- **A whole `Meaning` for each element read:** one way to read an element, but its actions and
  bounds walked, and its texts cloned at each key typed in a field.
- **The waits held in `Ui`, checked by `Ui::frame`:** no type for a host to keep, but the core
  would hold what only agents use, and every frame would pay for it.
- **A reader for each wait:** simpler state, but marks paid once for each wait, and an interface's
  64 readers shared by every wait of every connection.
- **A condition of one state,** as the card's words read: an enabled button cannot be waited for.
- **`after` decided now:** M6d.9a2c3 builds no reply; deciding the handle without its tool would
  be deciding twice.

## Consequences

- `uwui::agents` gains `Waits`, `Condition`, `Presence`, `State`, `WaitId`, `Waited`, `Met` and
  `WaitCheck`. A summary's states come from `State`, unchanged on the wire.
- The core gains the reader for waits: a mark of bounds or paths is told apart from one of
  meaning, and freeing an element tells the readers for waits of its target. A screen reader's
  changes are unchanged.
- `editor.hierarchy.100k` counts what the waits read and allocate in four frames, which
  `--counters` checks ([`measurements.md`](../measurements.md#m6d9a2c3-waits-in-uwui)).
- M6d.9a2c4 holds a `Waits` in `slime-app` beside the journal's waits, checks a target's waits
  after its frame, counts them against `MAX_WAITS`, cancels them with their requests, and decides
  `after`.

## Validation and reconsideration

Tested in `uwui`: a still frame reads nothing; a disabled button or a hover, the element alone; a
scroll, a resize, a root removed below others, a floating panel raised, and a removal on another
target, nothing of what only moved or is not theirs; waits met and cleared in the order made; an element removed, a box hidden, a dialog closed, a root moved to
another target, each ends a wait for its absence, a dialog read by its own elements; states held
and lacking, a popup, a name of a final sigma as `interface/elements` finds it, a kind changed
under a key, a box named; an element marked then removed read once; a reader past its bound, and
removals past it, walk the target; a cancel, a clear, an interface refused, a full one `BUSY`.
Waits held through random changes over two targets, modal roots, a root that moves, hiding, focus
and hover answer as `interface/elements` answers, after each frame. Each test fails without the code it proves, by a mutation.
Proposing another decision is justified if a condition comes to read actions, bounds or paths, or
if holding readers for waits costs a frame's budget with many agents.
