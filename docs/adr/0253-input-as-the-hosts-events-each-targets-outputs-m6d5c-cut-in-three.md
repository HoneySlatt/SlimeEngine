# 0253. Input as the host's events, answered at once, and each target's outputs: M6d.5c cut in three

- Status: Accepted; amends [0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) (the
  moves between two frames wait for their reader),
  [0249](0249-the-hosts-api-its-items-and-their-table.md) (the rows "Pointer", "Wheel", "Cursor"
  and "Input method's state": what waits for its reader) and
  [0251](0251-the-engines-host-clock-and-its-times.md) (an event's time carried by the event);
  amended by [0255](0255-the-clipboard-through-the-outputs-and-the-hosts-questions.md) (the answer's third state carries no text)
  and [0256](0256-each-root-on-its-target-each-target-its-input-the-active-target-m6d5c3-cut-in-two.md) (M6d.5c3 cut in two; an event for a target without roots; the engine's one move)
- Date: 2026-11-13

## Context

M6d.5c's card ([`stage-6d.md`](../stage-6d.md#m6d5c--input-and-output-in-uwui)) builds what
[ADR-0249](0249-the-hosts-api-its-items-and-their-table.md)'s column "Built by" gives it: events
with their target, time, source and answer; the host's questions; the outputs of each target; the
clipboard through events; and the cases where uwui assumes one surface.
[ADR-0251](0251-the-engines-host-clock-and-its-times.md) left it two of its own: the events carry
their time in place of `Ui::set_event_time`, and one frame's time serves every root until the
targets come.

The facts, read in the repository on 2026-11-13:

- A host gave uwui input through seven methods: `Ui::pointer_moved`, `pointer_left`,
  `pointer_button`, `wheel`, `modifiers`, `key` and `ime`. They are called about 580 times, all
  but seven in tests, each after `Ui::set_event_time`.
- A host read the pointer's shape and the input method from `Ui::cursor` and `Ui::text_input`,
  which said what holds now. `slime-app` kept what it last set, so as to set only what changed.
- `Ui::frame` is called about 650 times; roots are made from 86 literal `RootSettings`.
- The hit test, the pointer, the hover, the capture, the focus, the modal roots, the layers and
  the display list are each one for the whole `Ui`.
- Each move of the pointer walked the roots' trees twice, once for the hover and once for the
  pointer's rest.
- winit hands every move to the loop. A mouse at 1,000 Hz gives about four moves for each frame
  at 240 Hz. The web's and Android's coalesced events give a frame's moves as one.
- No reader in uwui nor in the engine needs:
  - the moves between two that it handles: the fields, sliders, splitters, drags and the editor's
    viewport read the last place alone;
  - the wheel's phase;
  - a field's purpose: winit 0.30's `set_ime_purpose` knows only normal, a password and a
    terminal, and a phone's pad of digits would refuse what uwui's fields of numbers read,
    `sqrt(2)/2` or `10cm`;
  - the pointer's entering, since its first move gives its place;
  - whether the cursor shows, since uwui never hides it.

A challenge of a first design found the faults that the alternatives below record.

## Decision

The owner's decisions of 2026-11-13: the cut in three; the items without a reader left to their
readers; `host::Person` reserved for tests; the contract of a presentation's interface written
rather than an output told again on demand. Then the lead's within them.

**M6d.5c is cut in three**, each part in a session of its own:

- M6d.5c1, the events and the outputs on one target:
  - one entry for the pointer, the wheel, the modifiers, the keys, and input methods'
    composition and commit;
  - the cursor and the input method's state as outputs;
  - the engine through them;
  - a pointer's move counted and timed.
- M6d.5c2, the clipboard and the host's questions:
  - copy, cut and paste as events, with the answer's third state and the text that the handler
    writes;
  - numbered reads of the clipboard, the `Clipboard` trait gone, the editor's paste a request
    and its answer;
  - the deletion around the caret;
  - the questions: the text around the caret, a range's rectangle, the character at a point,
    whether a point is uwui's.
- M6d.5c3, the targets:
  - each root on its target;
  - each target's hit test, pointer, hover, capture, focus, modal root, layers, display list,
    frame and frame's time;
  - a layer over another target's root;
  - input to present, measured.

Each call of input changes once: the target is in the event from M6d.5c1.

**One entry for events.**

- `Ui::event(Event) -> Answer`. An `Event` holds:
  - its target, `TargetId`;
  - its time, `EventTime`, with its source;
  - its input, `Input`: the pointer moved, left, a button; the wheel; the modifiers; a key; an
    input method.
- uwui handles each event at once, so that every query that follows sees its effect.
- The time moves the interface's clock, and never back.
- `Answer` is `Taken` or `NotTaken`, `#[must_use]`, and a host reads it through
  `Answer::taken`.
- `Input` and `Answer` are `#[non_exhaustive]`. M6d.5c2 adds to `Input` copy, cut, paste, a
  clipboard's answer and the deletion around the caret. It adds to `Answer` its third state,
  taken and left to the platform, with what the handler writes.
- The seven methods and `Ui::set_event_time` are gone: one way to give input.

**Targets named by the host, one served until M6d.5c3.**

- `TargetId` is a number that the host chooses. `TargetId::MAIN` is the one that every root draws
  on.
- An event for another target is not taken, and changes nothing but the time. Its outputs are
  empty.

**The host gathers the pointer's moves.**

- It gives the moves of a frame as one, the last, at its own time, as the web and Android give
  them. It gives that move before any other event that follows it, and before the frame.
- `slime-app` holds the last move until another event comes for the interface, or the frame
  starts.
- uwui stays eager: it neither defers nor batches.
- A move walks the tree once: the hover and the rest share one hit test.

**What waits for its reader**, the owner's choice:

| Item | Its reader |
|---|---|
| The moves between two frames | a pen's stroke, with the pen's kind of pointer |
| The wheel's phase | a scroll that ends with inertia |
| A field's purpose and hints | a host whose platform reads them, Android's or the web's |
| The pointer's entering | none: its first move gives its place |
| Whether the cursor shows | a component that hides it |

**Outputs read for each target, only what changed.** `Ui::outputs(TargetId) -> Outputs` gives:

- `cursor`: the pointer's shape, when it changed.
- `input_method`, `InputMethod::On` or `Off`:
  - `On` each time a field's input starts: another field took the focus, or the field lost the
    focus and took it back before the host read, which ended what it composed;
  - the host then ends what the input method composed and turns it on: off then on, on winit;
  - `Off` once no field holds the focus.
- `input_area`: the caret's box, in the target's physical pixels. It is told with each `On`, and
  whenever it moves.

A host keeps only what its platform needs. `slime-app` keeps whether the input method is allowed,
to forbid it only while it is; and whether an interface spoke, to show its own arrow once the
presentation has none. The outputs are the host's alone: what another reader takes, the host is
not told. A presentation gives the same interface at every call once it gave one, since the window
is told only what changed. `Ui::cursor` and `Ui::text_input` are gone. A program that needs to know
whether a field takes the keys asks the focus, as *Hunna Wood*'s overlay now does.

**Tests give input as a person, behind a feature**, the owner's choice. `host::Person` is a trait
that `Ui` implements, built with uwui's feature `test-input` alone. It gives each input through
`Ui::event`, on the main target, at the interface's latest time, which it does not move. The tests
of uwui, of the editor, of the dev overlay and of the examples turn the feature on through their
dev-dependencies, uwui's own through a dev-dependency on itself, and the benchmarks through theirs:
their 580 calls read as before and reach the one entry, and a host, the game shipped and uwui once
published have one way to give input. A test of time gives its events their time
through `Ui::event`. `EventSource::Simulated` names a test's or a headless run's time, as
`FrameSource::Simulated` does.

**The engine.** `slime-app` gives each event on `TargetId::MAIN`, stamped at reception on the
host's clock. It shows the outputs after each frame drawn.

## Alternatives considered

- **The seven methods kept, each with a target and a time.** One call for each kind, as before.
  But a host translates its platform's events in one match anyway, and seven doors would each
  carry the target, the time and, at M6d.5c2, the answer's third state.
- **uwui gathering the moves itself**, each move recorded and handled before the next event of
  another kind, the frame or a query. Every host would gain, but the challenge found it wrong in
  three ways:
  - queries that take `&self`, an element's state, `Ui::dragging`, the focus, would answer from
    before the moves;
  - the pointer's rest would start at the frame's time, not the move's, against ADR-0227;
  - drags whose result follows the path, a slider's changes, would lose their steps in uwui's own
    tests.
- **The moves between, the wheel's phase, a field's purpose, the entering and the cursor's
  showing built now**, as ADR-0249's column reads: items that no code reads, against
  [ADR-0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md).
- **The input method's state with its field and its caret's box as one value.** It changes at
  each move of the caret, so a host would keep the last field to tell a new input from a moved
  caret, as `slime-app` did. And a field that lost the focus and took it back within one step
  would end its composition without the host knowing.
- **Outputs as the state that holds now**, as `Ui::cursor` gave it: simpler for tests, but each
  host keeps what it last set, and ADR-0248 asks for what changed.
- **`Answer` as a structure of flags**, as the prototype of M6d.5a wrote it: it allows "left to
  the platform" without "taken", a state that means nothing. The three states are an enum, which
  M6d.5c2 completes, and what a copy writes travels beside it.
- **`host::Person` public**, documented as the tests' door: nothing to turn on; but any host could
  give input without its time nor its target, and it would leave with uwui at M6d.11 as published
  API.
- **Tests rewritten on `Ui::event` through constructors local to each test crate.** Each test
  would show the one entry. But some 580 calls would be rewritten, each with an answer to ignore,
  and the same constructors written in five crates.
- **The cut in two, or targets first**: a first part too large to review; or 650 calls of
  `Ui::frame` changed before the events' shape is fixed.

## Consequences

- `uwui::host` holds the host's API: `TargetId`, `Event`, `Input`, `Answer`, `Outputs` and
  `InputMethod`; `Person` with `test-input`. `Ui::event` and `Ui::outputs` reach it.
- Until [M6d.5e](../stage-6d.md#m6d5e--slime-app-a-host-through-the-api-alone) gives uwui the
  lifecycle, `Presentation::ui` must give the same interface at every call once given
  ([`debt.md`](../debt.md)).
- A move of the pointer over the editor's hierarchy of 100,000 entities takes 1.6 µs, against
  3.2 µs before, without allocating
  ([`measurements.md`](../measurements.md#m6d5c1-input-as-the-hosts-events)).
- A host that gives every move pays a hit test for each, about four times a frame at 240 Hz for
  a mouse at 1,000 Hz.
- In the engine, what the editor's viewport measures along the pointer's path sees one move for
  each frame: a move out and back within a frame does not count towards the threshold of a drag,
  nor towards an orbit's length. The raw motion that turns a held cursor is not gathered. A field
  of numbers dragged re-anchors as before, since a change of the modifiers gives the held move
  first.
- Until M6d.5c3, one target is served, and a second's events are not taken. With targets, the
  engine holds one move for each target.
- The debt that M6d.5b left, the times given beside the events, is settled. One frame's time for
  every root waits for M6d.5c3.
- The cost of a move on the lowest machine that the quality bar names is not measured: no such
  machine is set up yet.
- M6d.9a1b3 follows M6d.5c, as ADR-0250 orders it.
