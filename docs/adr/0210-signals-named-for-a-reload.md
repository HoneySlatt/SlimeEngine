# 0210. Signals named for a reload, through serde

- Status: Accepted; amends [0202](0202-how-m6d1-compares-ways-of-writing-an-application.md)
  (M6d.1c cut in three; `serde` added for a type's state) and
  [0205](0205-signals-change-uwuis-interface.md) (a signal's state across a reload of its type
  decided)
- Date: 2026-11-07

## Context

[ADR-0205](0205-signals-change-uwuis-interface.md) chose signals for uwui's state, and left to
M6d.1c how a signal's state migrates across a reload of its type. On M6d.1b's bench, a reload ran
the view's function again in a new runtime. The root scope's signals took their values back in the
order that they were made. A signal added before the others shifted that order: the new signal
took the selection's value, the selection the page's, which its type refused. The selection and
the page were lost. A shift between two signals of one type would give a wrong value without a
word.

No reference migrates a value whose type changed
([uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#a-type-whose-state-migrates)).
Iced, Rune and Dioxus start again. Flutter keeps fields by their names, through Dart's reflection.
In Rust, a value survives a change of its type only through a form that does not depend on its
layout. The engine asks for the same thing: an interface's state kept in a save and rewound
([`uwui-needs.md`](../uwui-needs.md)).

[ADR-0202](0202-how-m6d1-compares-ways-of-writing-an-application.md) gave M6d.1c three questions:
a function's body reloaded, a type whose state migrates, and the interface's executor. At its
start, the milestone held three ADRs, two dependencies, the tool `dx`, and the rivals' reloads
timed. The owner decided on 2026-11-07, one question at a time.

## Decision

**M6d.1c is cut in three:**

- M6d.1c1: signals named, and a type's state migrated through `serde`;
- M6d.1c2: the interface's executor, its waker, its tasks bound to components, and other threads'
  changes gathered into the next frame;
- M6d.1c3: a function's body reloaded through `subsecond`, timed on a prototype of Slatty's size,
  the rivals' reloads beside it.

Names come first. A handle that another thread keeps on a signal must stay valid across a reload,
and the timed reload runs on kept state.

**A signal whose state a reload keeps is named by its author:**
`let selected = RwSignal::kept("selected", || 0);`, and `Stored::kept` for a value that no closure
tracks.

- The name is unique within its scope. Two alike panic when the second is made, at the first run.
- Only rows and dynamic views make scopes; the root's holds the rest.
- A scope's path is the keys, from the window's root, of the element that made it: a row's, a
  dynamic view's. A kept value follows those keys, as the tree's element state does. A grid's
  cells are keyed by their items; a list's rows and an element's children, by their places.
- The first value comes from a closure, which runs only when no reload kept one.
- A signal not named starts again from its first value at each reload. It is never replayed by
  its order.

**The value goes through `serde`.** `kept` asks for `T: Serialize + DeserializeOwned`. Before the
reload, each kept value is written into a `serde_json::Value` in memory, by its fields' names.
After it, the signal of that name and that path reads the value into its type as the new code has
it. A field added takes its `#[serde(default)]`.

- A signal written as a value holds its value: a kept list of signals keeps their values.
- A value that no longer reads as its type starts again, and the runtime notes it. The signals
  that a failed read made are disposed.
- A value that cannot be written, a map keyed by pairs for instance, starts again, and the runtime
  notes it.
- A kept value that no signal took once the reload's frames end, a signal renamed for instance,
  is noted and dropped.
- The form in memory is uwui's own business. M6d.3, which brings `serde` to uwui's documents, may
  change it.

Both crates are already in the lock file, built for `rmcp`: MIT OR Apache-2.0. On the bench's
branch, `slime-benches` depends on them; nothing on `main` changes.

## Alternatives considered

- **A struct of state derived with `serde`**, its fields signals, kept by its type's name: names
  that the compiler checks, and `serde`'s `alias` for a field renamed. But some eight lines more
  for each view, a struct to write beside the ideal code, and first values to declare to `serde`.
- **Names taken from each `let` by an attribute macro on the view's function:** the ideal code
  unchanged. But a procedural macro rewrites the function's body, `syn` and `quote` added; it
  misses the signals made in helpers and closures, and its errors are less clear.
- **No migration**, as in Iced, Rune and Dioxus: a scope starts again when its signals' count or
  types change. No new concept and no dependency. But the bench's loss stays, two signals of one
  type exchanged pass unseen, and the engine's saves get nothing.
- **Replay in the order that signals are made, guarded by their types:** the body reloaded for
  free. But a signal removed and another of the same type added elsewhere in the scope shifts
  silently, and two rules for one thing.
- **The value written with RON:** closer to Rust, its enums and maps keyed by any type. But a new
  crate, for a form in memory that nobody reads.
- **A form of uwui's own,** a serializer and a deserializer written by hand: no crate of format,
  but some 500 to 800 lines that redo what `serde_json::Value` does.
- **No `serde`, a trait of uwui's own:** no dependency, but a trait that doubles `serde`, which
  applications' types derive already.

## Consequences

- An author names what a reload must keep: one string for each such signal, and the rule that the
  others start again. The library's screen changes four lines and adds two concepts: `kept`, and
  that rule.
- A renamed signal loses its value, and the runtime notes it.
- A scope's path moves with the keys of the elements above it. An element added before a dynamic
  view, or a list's rows reordered, gives a kept value to another scope; only a value left where no
  scope is now is noted. A part that keeps a signal, used twice in one scope, panics: it is no
  scope of its own. Keys given by views, and a scope for each part that keeps state, would answer
  both: uwui's first stage decides them.
- `serde` writes no `Instant`: `kept` does not compile for a value that holds one. Such a value
  holds a time from the clock's start instead.
- Names and scopes' places live outside the graph's nodes. A signal not named holds no more than
  before. An open screen makes a few allocations more, named or not: a list's path, a dynamic
  view's, and the maps' growth.
- The figures, timed in the program without the compiler's part, are in
  [uwui's `docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md).
- M6d.1c2 must answer how another thread's handle on a signal survives a reload, which makes new
  signals. M6d.1c3 runs the reload of a body over kept state.
- The engine's saves of an interface's state can rest on the same names. Their format is the
  engine's, decided when a game needs it.
