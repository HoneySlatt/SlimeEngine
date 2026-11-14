# 0262. uwui-accesskit's API: one tree per window, the host's node made when needed

- Status: Accepted; amends
  [0261](0261-each-reader-of-meaning-its-changes-each-target-its-tree-no-number-on-a-screen-readers-action-m6d5d-cut-in-two.md)
  (`slime-color` in the crate's tests, which otherwise depends on no engine crate)
- Date: 2026-11-14

## Context

M6d.5d2 moves `slime-app`'s translation of the interface's meaning into AccessKit's tree into
`uwui-accesskit`, a crate of uwui's
([ADR-0261](0261-each-reader-of-meaning-its-changes-each-target-its-tree-no-number-on-a-screen-readers-action-m6d5d-cut-in-two.md)).
ADR-0261 fixed its name, its licence, `accesskit` re-exported, and the guard on a screen reader's
request. It left the crate's API to this milestone.

The facts, read in the repository on 2026-11-14:

- The translation read uwui's public API alone. What was the engine's in it: the window's title and
  size, `slime-platform`'s window and its three events of screen readers (the tree asked, an
  action, no reader any more).
- AccessKit's adapters, on every platform, give a host the same three events.
- The translation kept the window's title and last size, and built the window's node, the tree's
  root, only when the update held it: a whole update, the top changed, or the window resized.
- The tests name `slime_color::LinearColor` for a color well: uwui names it in its API without
  re-exporting it. `slime-color` leaves with uwui; `uwui-render` already depends on it.

## Decision

**One tree per window.** `ScreenReaderTree::new(target)` holds what one window gives screen
readers, the part of the interface that its target draws. It maps one to one onto an adapter's
events:

- `tree_requested()`: updates are given from now on, the next whole;
- `act(ui, request)`: done only on `TreeId::ROOT`, on an element of its target, and on an action
  that the element takes now (ADR-0261);
- `deactivated(ui)`: no update until a screen reader asks again, and its reader of meaning let go;
- `update(ui, window_changed, window)` after each frame of its target: none while no screen reader
  reads.

The crate keeps whether a screen reader reads, so that a host is a few lines.

**The window's node is the host's, made when needed.** `window` is an `impl FnOnce() -> Node`,
called only when the update holds the node, as before: whole, the top changed, or
`window_changed`, which the host sets when what the node says changed since the last update it
gave, a size or a title. The crate sets its children to the target's top. A host keeps its
adapter, its window's title and size. The update is `#[must_use]`: one that does not reach the
adapter leaves the next naming nodes that AccessKit never received.

**What the tree says of itself.** Its toolkit is `uwui`, its version the crate's.

**Logging and tests.** The crate logs with `tracing`, as uwui's crates do
([ADR-0008](0008-errors-logging-and-assertions.md)): a warning once when the interface refuses a
reader, each request at the debug level. Its tests take `slime-color` as a dev-dependency; the
crate names no other engine crate, which `scripts/check.sh lint` checks, as for `uwui-render`.
These two are the owner's choices of 2026-11-14; the second amends ADR-0261's "the crate depends on
no engine crate", for its tests alone.

## Alternatives considered

- **The window's title and size kept by the crate**, which builds the node, as the translation
  did in `slime-app`: the crate would decide what a window's node says, its role, name and bounds,
  and grow a field for each property that a host adds, a description or a window's state.
- **The node given at each update**, built by the host: a node and a string made for nothing at
  each frame of a window read by a screen reader.
- **The node's changes found by comparing nodes**: the node built at each frame to compare it.
- **Whether a screen reader reads left to the host**: each host would repeat the same flag and
  the same whole update after a request.
- **Free functions over a state the host keeps**: the guard of ADR-0261 needs the tree's target,
  which a host could pass wrong.
- **A tree id given by the host**, for a tree grafted into another toolkit's: no host grafts one
  now; written as a limit in the rustdoc.
- **No logging in the crate**: an API of more for a refusal that comes past 64 readers.
- **The color well's test without `slime-color`**: its color taken from a style that uwui
  computes, a test that depends on a theme's default.

## Consequences

- `slime-app` holds no translation: its `accessibility` module makes the window's node and passes
  `slime-platform`'s events and updates. It depends on `uwui-accesskit` with its feature, no longer
  on `accesskit`.
- Screen readers read the toolkit as `uwui` instead of `SlimeEngine`.
- `scripts/check.sh` checks, in its `headless` part, that the game, built with its default
  features or for shipping, holds neither `uwui-accesskit` nor AccessKit's adapter.
- A host that gives the tree another interface leaves the window's reader of meaning in the one it
  left, which keeps it until it goes; `slime-app` gives one interface for a run.
