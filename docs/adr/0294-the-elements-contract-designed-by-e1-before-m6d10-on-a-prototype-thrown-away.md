# 0294. The element's contract designed by E1, before M6d.10, on a prototype thrown away

- Status: Accepted; supersedes in part
  [0283](0283-uwuis-widgets-and-a-programs-on-one-public-path.md) (when and how the public path is
  designed)
- Date: 2026-11-17

## Context

Several decisions shape one contract, what an element of uwui is, each taken alone:

- signals, and live values that draw one element again
  ([ADR-0205](0205-signals-change-uwuis-interface.md));
- a theme's parts and states ([ADR-0222](0222-what-a-theme-is.md));
- layouts in KDL, a kind from another crate registered with uwui
  ([ADR-0225](0225-layouts-written-in-kdl-reached-from-code-by-their-names.md));
- one public path for uwui's widgets and a program's, designed in uwui's first stage
  ([ADR-0283](0283-uwuis-widgets-and-a-programs-on-one-public-path.md));
- text as a message or a literal
  ([ADR-0284](0284-uwuis-text-a-message-or-a-literal-from-its-authoring-api-fluent-later.md));
- each pointer named ([ADR-0285](0285-every-pointer-named-in-the-hosts-events-the-tree-following-each-with-touch.md));
- actions declared once ([ADR-0293](0293-actions-declared-once-in-uwuis-core-undo-and-the-journal-the-programs.md));
- the canvas keeping its items across frames, in uwui's first stage
  ([uwui's roadmap](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#the-first-stage-the-foundations-and-four-programs)).

A review of the architecture, read-only, on 2026-11-16 and 2026-11-17, at `9dce4803`, found:

- A list's rows are keyed by their number (`crates/uwui/src/tree.rs`, `Kind::List`). A kept field
  given another text drops its composition, its undo and what the person typed
  (`crates/uwui/src/tree/field.rs`, `Field::give`); a field removed commits nothing. Rows that
  reorder move a person's typing onto another item.
- Each frame walks every element of its target twice, gathers the whole display list and sends it
  whole to the GPU (`crates/uwui/src/paint.rs`, `crates/uwui-render/src/canvas.rs`): a still
  frame of the editor's tree visits 24,974 elements and allocates 4,565,512 bytes
  (`benches/counters.txt`). A frame after one value of the inspector changed took 3.36 to 3.89 ms
  of the processor, measured while another session built ([`measurements.md`](../measurements.md)),
  against 4.17 ms from input to present.
- A kind of element cannot be added outside uwui: `Kind` is closed, the style sheets' kinds are a
  private list, and the derives of styles write paths that only uwui's crate reaches.
- The hosts, the renderer, screen readers' and agents' crates read no kind of element: they read
  the display list and the meaning of elements.
- ADR-0283 leaves its design's cost "weighed at M6d.10", without figures.
- Elsewhere, a cache of drawing designed apart from meaning lost accessibility: GPUI's views drawn
  from its cache drop their accessibility nodes
  ([zed-industries/zed#66690](https://github.com/zed-industries/zed/issues/66690), opened
  2026-11-08).

## Decision

The owner's decision of 2026-11-17.

**E1, a milestone of Stage 6d, designs the element's contract whole, before M6d.10,** on track A
after M6d.9b, while track B goes on. It builds nothing kept.

- **First, the frame measured:** a profile of the editor's frame after one value of the inspector
  changed, on the high tier, the machine quiet.
- **Then, the survey** of what uwui's lacks: how rivals let a program's widget do what theirs do,
  keep an item's state by its key in a list that reorders, and draw again only what changed,
  accessibility kept; in uwui's
  [survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md).
- **Then, a prototype thrown away,** on the signals of the branch `m6d1c-bench`, on four cases:
  a list of rows that a person edits, reordered while rows load; one indicator moving on the
  editor's tree; a canvas of a crate of its own that a screen reader and an agent use; a component
  that owns a layer, as the palette of commands.
- **Then, the contract's ADRs,** put to the owner. They cover: a widget written outside uwui,
  measured, laid out, drawn, taking input and the focus, giving its meaning and actions, carrying a
  theme's parts and states; its kind registered for layouts and style sheets, its styles declared
  outside uwui; items keyed by what they show, their state following the key, and what two equal
  keys do; the kinds of change and what the display list keeps across frames, between uwui and
  `uwui-render`, meaning kept with what is kept; what reading a signal of a scope disposed does;
  and how long `Ui::build` stays public. Text and pointers come in as ADR-0284 and ADR-0285 shaped
  them.

**M6d.10 cuts uwui's first stage on E1's figures.** The contract is built in uwui's first stage,
before its library of components, uwui's own widgets then moved onto it, as ADR-0283 has it. The
ADR that designs the public path supersedes what of ADR-0140 and ADR-0148 it changes.

The rest of ADR-0283 stands: what a widget of uwui's can do, a program's can, through the public
API; a faster path inside uwui allowed, measured, if it gives uwui's widgets nothing that a
program's lacks; until then, a surface declares its actions.

## Alternatives considered

- **The plan kept, each decision placed by M6d.10 alone:** nothing added before the move, but each
  part fixing a share of one contract without the others, and ADR-0283's cost weighed without
  figures.
- **A design on paper before M6d.10, the prototype uwui's first milestone after the move:** the move
  delayed less, but M6d.10 deciding without figures, and the contract's ADRs written again after
  the prototype.
- **uwui's tree written again before the move:** written without the contract, while the layers
  above it read no kind of element.

## Consequences

- M6d.10 and the move come later on track A by E1's length; track B is not held.
- The prototype is thrown away; its figures stay in uwui's
  [`measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md).
- The engine's editor tries the contract at its first port
  ([ADR-0277](0277-the-engine-ported-to-uwui-at-the-end-of-each-of-uwuis-stages.md)).
- Widgets written in uwui before the contract is built still move onto it; E1 makes the move's cost
  known before M6d.10.

## Validation and reconsideration

Read: the tree's keys, a field's `give`, a frame's walks and the canvas's upload; the counters and
the inspector's frame in `measurements.md`; the crates that read `Kind`. Assumed: that four cases
show the contract's hard parts, and that a prototype on the branch's signals answers for uwui's
own. Proposing another decision is justified if the profile shows that signals alone bring the
editor's change frame within budget and that the public path changes neither keys nor what a frame
gives the renderer: the plan kept would then serve.
